use std::cell::RefCell;
use std::rc::Rc;

use crate::class_file::{BootstrapMethod, ConstantPoolEntry};
use crate::heap::{JavaStringValue, JObject, JRef, JValue, NativePayload};

use super::Vm;
use super::class_identity::ClassId;
use super::cp_cache::{CpCache, CpCacheEntry, ResolvedMethodEntry};
use super::descriptors::*;
use super::frame::*;
use super::trampoline::FrameInfo;

fn push_utf16_str(out: &mut Vec<u16>, s: &str) {
    out.extend(s.encode_utf16());
}

fn push_java_string_value(out: &mut Vec<u16>, value: &JavaStringValue) {
    out.extend_from_slice(value.utf16());
}

impl Vm {
    // -----------------------------------------------------------------------
    // Trampoline-compatible dispatch methods.
    // These pop args from the operand stack, then either:
    //   - Build a FrameInfo and store it in self.pending_frame (bytecode method)
    //   - Call native inline and push the result onto the frame's stack
    // Returns Ok(None) always (the result is either pushed inline or deferred).
    // -----------------------------------------------------------------------

    pub(super) fn dispatch_static(
        &mut self,
        cp: &[ConstantPoolEntry],
        cache: &CpCache,
        idx: u16,
        caller_class_id: Option<ClassId>,
        frame: &mut Frame,
    ) -> Result<Option<JValue>, String> {
        // ---- FAST PATH: check cpCache for a previously resolved method ----
        {
            let cb = cache.borrow();
            if let Some(Some(CpCacheEntry::Method(entry))) = cb.get(idx as usize) {
                if entry.has_code {
                    // Bytecode method — build frame directly, no String clones needed.
                    let args = pop_args(frame, entry.arg_slot_count);
                    let fi = self.build_frame_from_cache(entry, args, !entry.is_void);
                    *self.pending_frame_mut() = Some(fi);
                    return Ok(None);
                } else {
                    // Native method — extract data before releasing borrow.
                    let owner = entry.owner_class.clone();
                    let mname = entry.method_name.clone();
                    let desc = entry.descriptor.clone();
                    let is_void = entry.is_void;
                    let arg_slot_count = entry.arg_slot_count;
                    drop(cb);
                    let args = pop_args(frame, arg_slot_count);
                    let result = self.invoke_static(&owner, &mname, &desc, args)?;
                    if !is_void { frame.stack.push(result); }
                    return Ok(None);
                }
            }
        }

        // ---- SLOW PATH: first invocation — resolve and populate cache ----
        let (class_name, method_name, descriptor) = resolve_methodref_ref(cp, idx);
        let resolved_target = match caller_class_id {
            Some(caller) => Some(self.resolve_method_reference(caller, cp, idx)?),
            None => None,
        };
        let resolved_class_id = resolved_target.as_ref().map(|target| target.owner_class_id);
        let resolved_class_name = resolved_target
            .as_ref()
            .map(|target| target.owner_class.clone())
            .unwrap_or_else(|| class_name.to_owned());
        let resolved_method_name = resolved_target
            .as_ref()
            .map(|target| target.name.as_str())
            .unwrap_or(method_name);
        let resolved_descriptor = resolved_target
            .as_ref()
            .map(|target| target.descriptor.as_str())
            .unwrap_or(descriptor);
        if resolved_target
            .as_ref()
            .map(|target| target.access_flags & 0x0008 == 0)
            .unwrap_or(false)
        {
            let detail = format!("{resolved_class_name}.{resolved_method_name}{resolved_descriptor}");
            self.throw_incompatible_class_change_error(&detail);
            return Err(format!("java/lang/IncompatibleClassChangeError: {detail}"));
        }
        self.ensure_class_init(&resolved_class_name)?;
        let n_args = count_args(resolved_descriptor);
        let args = pop_args(frame, n_args);

        // Normalize descriptor and args (varargs synthesis) before branching.
        let orig_args = args.clone();
        let (desc, args) = if resolved_class_id.is_some() {
            (resolved_descriptor.to_owned(), args)
        } else {
            match self.prepare_static_args(&resolved_class_name, resolved_method_name, resolved_descriptor, args) {
                Some(pair) => pair,
                None => {
                    // Method flags not found — fall back to invoke_static with original args.
                    let result = self.invoke_static(&resolved_class_name, resolved_method_name, resolved_descriptor, orig_args)?;
                    if !matches!(result, JValue::Void) { frame.stack.push(result); }
                    return Ok(None);
                }
            }
        };

        let push_return = !desc.ends_with(")V");
        // Resolve method exec info once (used for both frame building and cache population).
        let exec_info = resolved_class_id
            .and_then(|class_id| self.resolve_method_exec_info_for_class_id(class_id, resolved_method_name, &desc))
            .or_else(|| self.resolve_method_exec_info(&resolved_class_name, resolved_method_name, &desc));
        match exec_info {
            Some(info) if info.has_code => {
                // Bytecode method — build frame and populate cache.
                let fi = self.build_static_frame_from_exec_info(&info, resolved_method_name, &desc, args, push_return);
                self.populate_static_method_cache(cache, idx, resolved_method_name, &desc, &info);
                *self.pending_frame_mut() = Some(fi);
                Ok(None)
            }
            _ => {
                // Native or unresolved — cache and fall back to invoke_static.
                self.populate_static_native_cache(cache, idx, resolved_class_id, &resolved_class_name, resolved_method_name, &desc);
                let result = self.invoke_static(&resolved_class_name, resolved_method_name, &desc, args)?;
                if !matches!(result, JValue::Void) {
                    frame.stack.push(result);
                }
                Ok(None)
            }
        }
    }

    /// Build a FrameInfo from a pre-resolved MethodExecInfo (avoids double resolution).
    pub(super) fn build_static_frame_from_exec_info(
        &mut self,
        info: &super::MethodExecInfo,
        method_name: &str,
        descriptor: &str,
        args: Vec<JValue>,
        push_return: bool,
    ) -> FrameInfo {
        let (param_tokens, _) = Self::parse_method_descriptor_tokens(descriptor);
        let req: usize = param_tokens.iter()
            .map(|t| if t == "J" || t == "D" { 2 } else { 1 })
            .sum();
        let mut locals = vec![JValue::Void; info.max_locals.max(req)];
        let mut li = 0usize;
        for (a, t) in args.into_iter().zip(param_tokens.iter()) {
            if li >= locals.len() { break; }
            locals[li] = self.adapt_value_for_descriptor(t, a);
            li += if t == "J" || t == "D" { 2 } else { 1 };
        }
        let fo = format!("{}.{method_name}{}", info.class_name, info.descriptor);
        let synchronized_monitor = if info.access_flags & 0x0020 != 0 {
            let class_obj = self.class_object(&info.class_name);
            self.monitor_enter(&class_obj);
            Some(class_obj)
        } else {
            None
        };
        FrameInfo {
            class_id: info.class_id,
            frame: Frame { locals, stack: Vec::new(), pc: 0 },
            code: info.code.clone(),
            cp: Rc::clone(&info.cp),
            cache: Rc::clone(&info.cache),
            frame_owner: fo,
            bootstrap_methods: info.bootstrap_methods.clone(),
            exception_table: info.exception_table.clone(),
            push_return,
            concat_state: None,
            lambda_return_adapt: None,
            synchronized_monitor,
        }
    }

    /// Build a FrameInfo directly from a cached ResolvedMethodEntry (zero resolution).
    fn build_frame_from_cache(&mut self, entry: &ResolvedMethodEntry, args: Vec<JValue>, push_return: bool) -> FrameInfo {
        let req: usize = entry.param_tokens.iter()
            .map(|t| if t == "J" || t == "D" { 2 } else { 1 })
            .sum();
        let mut locals = vec![JValue::Void; entry.max_locals.max(req)];
        let mut li = 0usize;
        for (a, t) in args.into_iter().zip(entry.param_tokens.iter()) {
            if li >= locals.len() { break; }
            locals[li] = self.adapt_value_for_descriptor(t, a);
            li += if t == "J" || t == "D" { 2 } else { 1 };
        }
        let fo = format!("{}.{}{}", entry.owner_class, entry.method_name, entry.descriptor);
        let synchronized_monitor = if entry.access_flags & 0x0020 != 0 {
            let class_obj = self.class_object(&entry.owner_class);
            self.monitor_enter(&class_obj);
            Some(class_obj)
        } else {
            None
        };
        FrameInfo {
            class_id: entry.owner_class_id,
            frame: Frame { locals, stack: Vec::new(), pc: 0 },
            code: (*entry.code).clone(),
            cp: Rc::clone(&entry.cp),
            cache: Rc::clone(&entry.cache),
            frame_owner: fo,
            bootstrap_methods: (*entry.bootstrap_methods).to_vec(),
            exception_table: (*entry.exception_table).to_vec(),
            push_return,
            concat_state: None,
            lambda_return_adapt: None,
            synchronized_monitor,
        }
    }

    /// Populate the cpCache with a resolved bytecode method entry.
    fn populate_static_method_cache(
        &self,
        cache: &CpCache,
        idx: u16,
        method_name: &str,
        descriptor: &str,
        info: &super::MethodExecInfo,
    ) {
        let (param_tokens, _) = Self::parse_method_descriptor_tokens(descriptor);
        let entry = ResolvedMethodEntry {
            owner_class_id: info.class_id,
            owner_class: info.class_name.clone(),
            code: Rc::new(info.code.clone()),
            exception_table: Rc::new(info.exception_table.clone()),
            max_locals: info.max_locals,
            arg_slot_count: count_args(descriptor),
            access_flags: info.access_flags,
            has_code: info.has_code,
            cp: Rc::clone(&info.cp),
            cache: Rc::clone(&info.cache),
            bootstrap_methods: Rc::new(info.bootstrap_methods.clone()),
            descriptor: descriptor.to_owned(),
            param_tokens,
            is_void: descriptor.ends_with(")V"),
            is_varargs: info.access_flags & 0x0080 != 0,
            method_name: method_name.to_owned(),
        };
        cache.borrow_mut()[idx as usize] = Some(CpCacheEntry::Method(entry));
    }

    /// Populate the cpCache with a resolved native method entry.
    fn populate_static_native_cache(
        &self,
        cache: &CpCache,
        idx: u16,
        class_id: Option<ClassId>,
        class_name: &str,
        method_name: &str,
        descriptor: &str,
    ) {
        let (param_tokens, _) = Self::parse_method_descriptor_tokens(descriptor);
        let entry = ResolvedMethodEntry {
            owner_class_id: class_id,
            owner_class: class_name.to_owned(),
            code: Rc::new(Vec::new()),
            exception_table: Rc::new(Vec::new()),
            max_locals: 0,
            arg_slot_count: count_args(descriptor),
            access_flags: 0,
            has_code: false,
            cp: Rc::new(Vec::new()),
            cache: Rc::new(RefCell::new(Vec::new())),
            bootstrap_methods: Rc::new(Vec::new()),
            descriptor: descriptor.to_owned(),
            param_tokens,
            is_void: descriptor.ends_with(")V"),
            is_varargs: false,
            method_name: method_name.to_owned(),
        };
        cache.borrow_mut()[idx as usize] = Some(CpCacheEntry::Method(entry));
    }

    pub(super) fn dispatch_virtual(
        &mut self,
        cp: &[ConstantPoolEntry],
        idx: u16,
        frame: &mut Frame,
    ) -> Result<Option<JValue>, String> {
        let (class_name, method_name, descriptor) = resolve_methodref_ref(cp, idx);
        let n_args = count_args(descriptor);
        let args = pop_args(frame, n_args);
        let this_val = frame.stack.pop().unwrap();
        match this_val {
            JValue::Ref(Some(r)) => {
                let push_return = !descriptor.ends_with(")V");
                self.dispatch_virtual_on_ref(r, class_name, method_name, descriptor, args, push_return, frame)
            }
            JValue::Ref(None) => Err(format!("NullPointerException: invokevirtual {class_name}.{method_name}{descriptor}")),
            other => Err(format!(
                "Expected reference for invokevirtual {class_name}.{method_name}{descriptor}, got {other:?}"
            )),
        }
    }

    /// Shared logic for virtual dispatch (used by dispatch_virtual and dispatch_interface).
    fn dispatch_virtual_on_ref(
        &mut self,
        r: JRef,
        class_name: &str,
        method_name: &str,
        descriptor: &str,
        args: Vec<JValue>,
        push_return: bool,
        frame: &mut Frame,
    ) -> Result<Option<JValue>, String> {
        // Fast-path: intercept Object.wait/notify/notifyAll directly to avoid
        // re-entering invoke_virtual's recursive path, which doesn't check
        // thread state for yielding.
        // This handles the normal invokevirtual bytecode path. Errors are
        // returned as Err (propagated to the trampoline's exception handler).
        // The native_virtual path (for invoke_virtual recursive fallback)
        // handles errors differently via pending_exception.
        match (method_name, descriptor) {
            ("wait", "()V") | ("wait", "(J)V") => {
                // Note: wait(long) timeout is intentionally ignored — waits indefinitely.
                if let Err(e) = self.monitor_wait(&r) {
                    return Err(e);
                }
                return Ok(None);
            }
            ("notify", "()V") => {
                if let Err(e) = self.monitor_notify(&r) {
                    return Err(e);
                }
                return Ok(None);
            }
            ("notifyAll", "()V") => {
                if let Err(e) = self.monitor_notify_all(&r) {
                    return Err(e);
                }
                return Ok(None);
            }
            _ => {}
        }

        match self.build_virtual_frame_inner(r.clone(), class_name, method_name, descriptor, args.clone(), push_return)? {
            Some(fi) => {
                *self.pending_frame_mut() = Some(fi);
                Ok(None)
            }
            None => {
                // Try to handle BytecodeLambda SAM dispatch via the trampoline
                // (instead of the recursive invoke_virtual path which uses
                // run_trampoline — the non-time-sliced variant that ignores
                // thread state changes like WaitingOnCondition).
                if let Some(fi) = self.try_build_lambda_sam_frame(&r, method_name, descriptor, args.clone(), push_return)? {
                    *self.pending_frame_mut() = Some(fi);
                    return Ok(None);
                }
                // Native, NativePayload::Lambda (Rust closure), or unresolved —
                // fall back to recursive invoke_virtual. This runs outside the
                // time-sliced trampoline, so thread state changes (e.g.
                // WaitingOnCondition) won't cause a yield. This is acceptable
                // because Rust closures don't call Java wait/notify.
                let result = self.invoke_virtual(r, class_name, method_name, descriptor, args)?;
                if class_name == "java/lang/reflect/Method" && method_name == "invoke" {
                    if let Some(err) = self.pending_exception_err() {
                        return Err(err);
                    }
                }
                if !matches!(result, JValue::Void) {
                    frame.stack.push(result);
                }
                Ok(None)
            }
        }
    }

    pub(super) fn dispatch_special(
        &mut self,
        cp: &[ConstantPoolEntry],
        idx: u16,
        caller_class_id: Option<ClassId>,
        frame: &mut Frame,
    ) -> Result<Option<JValue>, String> {
        let (class_name, method_name, descriptor) = resolve_methodref_ref(cp, idx);
        let resolved_target = match caller_class_id {
            Some(caller) => Some(self.resolve_method_reference(caller, cp, idx)?),
            None => None,
        };
        let resolved_class_name = resolved_target
            .as_ref()
            .map(|target| target.owner_class.as_str())
            .unwrap_or(class_name);
        let resolved_method_name = resolved_target
            .as_ref()
            .map(|target| target.name.as_str())
            .unwrap_or(method_name);
        let resolved_descriptor = resolved_target
            .as_ref()
            .map(|target| target.descriptor.as_str())
            .unwrap_or(descriptor);
        let n_args = count_args(resolved_descriptor);
        let args = pop_args(frame, n_args);
        let this_val = frame.stack.pop().unwrap();
        match this_val {
            JValue::Ref(Some(r)) => {
                if method_name == "<init>" {
                    if resolved_class_name == "java/lang/String" {
                        let s = self.string_from_init_args(resolved_descriptor, &args, &r);
                        r.borrow_mut().native = NativePayload::JavaString(s);
                        return Ok(None); // void
                    }
                    if resolved_target.is_none() && !self.method_exists(class_name, method_name, descriptor) {
                        return Ok(None); // legacy no-op for non-loader-aware helper paths
                    }
                }
                let push_return = !resolved_descriptor.ends_with(")V");
                match self.build_special_frame_inner(
                    r.clone(),
                    resolved_target.as_ref().map(|target| target.owner_class_id),
                    resolved_class_name,
                    resolved_method_name,
                    resolved_descriptor,
                    args.clone(),
                    push_return,
                )? {
                    Some(fi) => {
                        *self.pending_frame_mut() = Some(fi);
                        Ok(None)
                    }
                    None => {
                        let result = self.invoke_special(
                            r,
                            resolved_class_name,
                            resolved_method_name,
                            resolved_descriptor,
                            args,
                        )?;
                        if !matches!(result, JValue::Void) {
                            frame.stack.push(result);
                        }
                        Ok(None)
                    }
                }
            }
            JValue::Ref(None) => Err(format!("NullPointerException: invokespecial {class_name}.{method_name}{descriptor}")),
            other => Err(format!(
                "Expected reference for invokespecial {class_name}.{method_name}{descriptor}, got {other:?}"
            )),
        }
    }

    pub(super) fn dispatch_interface(
        &mut self,
        cp: &[ConstantPoolEntry],
        idx: u16,
        caller_class_id: Option<ClassId>,
        frame: &mut Frame,
    ) -> Result<Option<JValue>, String> {
        let (class_name, method_name, descriptor) = resolve_methodref_ref(cp, idx);
        let resolved_target = match caller_class_id {
            Some(caller) => Some(self.resolve_method_reference(caller, cp, idx)?),
            None => None,
        };
        let resolved_class_name = resolved_target
            .as_ref()
            .map(|target| target.owner_class.as_str())
            .unwrap_or(class_name);
        let resolved_method_name = resolved_target
            .as_ref()
            .map(|target| target.name.as_str())
            .unwrap_or(method_name);
        let resolved_descriptor = resolved_target
            .as_ref()
            .map(|target| target.descriptor.as_str())
            .unwrap_or(descriptor);
        let n_args = count_args(resolved_descriptor);
        let args = pop_args(frame, n_args);

        let is_static = resolved_target
            .as_ref()
            .map(|target| target.access_flags & 0x0008 != 0)
            .or_else(|| self.find_method_flags(resolved_class_name, resolved_method_name, resolved_descriptor)
                .map(|flags| flags & 0x0008 != 0))
            .unwrap_or(false);
        if is_static {
            let push_return = !resolved_descriptor.ends_with(")V");
            match self.build_static_frame(
                resolved_class_name,
                resolved_method_name,
                resolved_descriptor,
                args.clone(),
                push_return,
            )? {
                Some(fi) => {
                    *self.pending_frame_mut() = Some(fi);
                    return Ok(None);
                }
                None => {
                    let result = self.invoke_static(
                        resolved_class_name,
                        resolved_method_name,
                        resolved_descriptor,
                        args,
                    )?;
                    if !matches!(result, JValue::Void) {
                        frame.stack.push(result);
                    }
                    return Ok(None);
                }
            }
        }

        let this_val = frame.stack.pop().unwrap();
        match this_val {
            JValue::Ref(Some(r)) => {
                let push_return = !resolved_descriptor.ends_with(")V");
                self.dispatch_virtual_on_ref(
                    r,
                    resolved_class_name,
                    resolved_method_name,
                    resolved_descriptor,
                    args,
                    push_return,
                    frame,
                )
            }
            JValue::Ref(None) => Err(format!("NullPointerException: invokeinterface {class_name}.{method_name}{descriptor}")),
            other => Err(format!(
                "Expected reference for invokeinterface {class_name}.{method_name}{descriptor}, got {other:?}"
            )),
        }
    }

    /// Try to build a FrameInfo for a BytecodeLambda's SAM dispatch.
    /// Returns Ok(Some(fi)) if successful, Ok(None) if not a lambda or SAM doesn't match.
    fn try_build_lambda_sam_frame(
        &mut self,
        r: &JRef,
        method_name: &str,
        descriptor: &str,
        args: Vec<JValue>,
        push_return: bool,
    ) -> Result<Option<FrameInfo>, String> {
        let lambda_info = {
            let borrow = r.borrow();
            match &borrow.native {
                NativePayload::BytecodeLambda {
                    sam_method, sam_desc, impl_class, impl_method, impl_desc, ref_kind, captured,
                } => {
                    let sam_arg_count = count_args(sam_desc);
                    let call_arg_count = count_args(descriptor);
                    if method_name == sam_method.as_str() && call_arg_count == sam_arg_count {
                        Some((
                            impl_class.clone(), impl_method.clone(), impl_desc.clone(),
                            *ref_kind, captured.clone(),
                        ))
                    } else {
                        None
                    }
                }
                _ => None,
            }
        };
        let Some((impl_class, impl_method, impl_desc, ref_kind, captured)) = lambda_info else {
            return Ok(None);
        };

        let mut full_args = captured;
        full_args.extend(args);

        let adapt = Some((descriptor.to_owned(), impl_desc.clone()));

        let mut fi = if ref_kind == 5 || ref_kind == 7 || ref_kind == 9 {
            // Virtual/interface dispatch on receiver.
            if full_args.is_empty() {
                return Err("Lambda SAM dispatch: missing receiver argument".to_owned());
            }
            let recv = full_args.remove(0);
            match recv {
                JValue::Ref(Some(recv_ref)) => {
                    self.build_virtual_frame_inner(
                        recv_ref, &impl_class, &impl_method, &impl_desc, full_args, push_return,
                    )
                }
                _ => Err(format!("Lambda SAM dispatch: expected Ref for receiver, got {recv:?}")),
            }
        } else if ref_kind == 8 {
            // newinvokespecial — constructor reference (e.g. Age::new).
            // Fall through to recursive invoke_virtual path which handles this.
            return Ok(None);
        } else {
            // Static dispatch.
            self.ensure_class_init(&impl_class)?;
            self.build_static_frame(&impl_class, &impl_method, &impl_desc, full_args, push_return)
        }?;

        // Attach return-type adaptation info so the trampoline can box
        // primitive returns when the SAM expects a reference type.
        if let Some(ref mut frame_info) = fi {
            frame_info.lambda_return_adapt = adapt;
        }
        Ok(fi)
    }

    /// Handle `invokedynamic` — currently supports the three bootstrap methods
    /// used by Raoh: LambdaMetafactory, StringConcatFactory, SwitchBootstraps.
    pub(super) fn dispatch_invokedynamic(
        &mut self,
        cp: &[ConstantPoolEntry],
        idx: u16,
        frame: &mut Frame,
        _class_name: &str,
        bootstrap_methods: &[BootstrapMethod],
    ) -> Result<JValue, String> {
        let (bm_index, nat_index) = match &cp[idx as usize] {
            ConstantPoolEntry::InvokeDynamic { bootstrap_method_attr_index, name_and_type_index } => {
                (*bootstrap_method_attr_index, *name_and_type_index)
            }
            other => return Err(format!("Expected InvokeDynamic at cp[{idx}], got {other:?}")),
        };

        let (method_name, descriptor) = match &cp[nat_index as usize] {
            ConstantPoolEntry::NameAndType { name_index, descriptor_index } => {
                let n = match &cp[*name_index as usize] { ConstantPoolEntry::Utf8(s) => s.clone(), _ => String::new() };
                let d = match &cp[*descriptor_index as usize] { ConstantPoolEntry::Utf8(s) => s.clone(), _ => String::new() };
                (n, d)
            }
            other => return Err(format!("Expected NameAndType at cp[{nat_index}], got {other:?}")),
        };

        let bm = bootstrap_methods.get(bm_index as usize)
            .ok_or_else(|| format!(
                "Invalid bootstrap method index {bm_index} ({} bootstrap methods available)",
                bootstrap_methods.len()
            ))?;
        let bm_ref_idx = bm.bootstrap_method_ref;
        let bm_class = match &cp[bm_ref_idx as usize] {
            ConstantPoolEntry::MethodHandle { reference_index, .. } => {
                match &cp[*reference_index as usize] {
                    ConstantPoolEntry::Methodref { class_index, .. } => {
                        match &cp[*class_index as usize] {
                            ConstantPoolEntry::Class { name_index } => {
                                match &cp[*name_index as usize] {
                                    ConstantPoolEntry::Utf8(s) => s.clone(),
                                    _ => String::new(),
                                }
                            }
                            _ => String::new(),
                        }
                    }
                    _ => String::new(),
                }
            }
            _ => String::new(),
        };

        match bm_class.as_str() {
            "java/lang/invoke/LambdaMetafactory" => {
                // Capture free variables from the stack (captured args come from descriptor).
                let n_captured = count_args(&descriptor);
                let captured = pop_args(frame, n_captured);

                // Bootstrap argument 1 is the implementation MethodHandle.
                // Resolve it to (ref_kind, class, method, descriptor).
                let impl_info = bm.bootstrap_arguments.get(1).and_then(|&arg_idx| {
                    match cp.get(arg_idx as usize)? {
                        ConstantPoolEntry::MethodHandle { reference_kind, reference_index } => {
                            let rk = *reference_kind;
                            match cp.get(*reference_index as usize)? {
                                ConstantPoolEntry::Methodref { class_index, name_and_type_index }
                                | ConstantPoolEntry::InterfaceMethodref { class_index, name_and_type_index } => {
                                    let cls = match cp.get(*class_index as usize)? {
                                        ConstantPoolEntry::Class { name_index } => {
                                            match cp.get(*name_index as usize)? {
                                                ConstantPoolEntry::Utf8(s) => s.clone(),
                                                _ => return None,
                                            }
                                        }
                                        _ => return None,
                                    };
                                    let (mname, mdesc) = match cp.get(*name_and_type_index as usize)? {
                                        ConstantPoolEntry::NameAndType { name_index, descriptor_index } => {
                                            let n = match cp.get(*name_index as usize)? {
                                                ConstantPoolEntry::Utf8(s) => s.clone(), _ => return None,
                                            };
                                            let d = match cp.get(*descriptor_index as usize)? {
                                                ConstantPoolEntry::Utf8(s) => s.clone(), _ => return None,
                                            };
                                            (n, d)
                                        }
                                        _ => return None,
                                    };
                                    Some((rk, cls, mname, mdesc))
                                }
                                _ => None,
                            }
                        }
                        _ => None,
                    }
                });

                let sam_desc = bm.bootstrap_arguments.first().and_then(|&arg_idx| {
                    match cp.get(arg_idx as usize)? {
                        ConstantPoolEntry::MethodType { descriptor_index } => {
                            match cp.get(*descriptor_index as usize)? {
                                ConstantPoolEntry::Utf8(s) => Some(s.clone()),
                                _ => None,
                            }
                        }
                        _ => None,
                    }
                }).unwrap_or_default();

                let lambda = if let Some((ref_kind, impl_class, impl_method, impl_desc)) = impl_info {
                    let obj = Rc::new(RefCell::new(JObject {
                        class_name: "$$Lambda".to_owned(),
                        fields: std::collections::HashMap::new(),
                        native: NativePayload::BytecodeLambda {
                            sam_method: method_name,
                            sam_desc,
                            impl_class,
                            impl_method,
                            impl_desc,
                            ref_kind,
                            captured,
                        },
                    }));
                    obj
                } else {
                    JObject::new_lambda(|_| JValue::Ref(None))
                };
                Ok(JValue::Ref(Some(lambda)))
            }

            "java/lang/invoke/StringConcatFactory" => {
                // Pop arguments based on dynamic descriptor.
                let n_args = count_args(&descriptor);
                let args = pop_args(frame, n_args);

                // Extract the recipe string from bootstrap arguments.
                // The recipe uses \u0001 as placeholders for arguments.
                let recipe = if !bm.bootstrap_arguments.is_empty() {
                    match &cp[bm.bootstrap_arguments[0] as usize] {
                        ConstantPoolEntry::String { string_index } => {
                            match &cp[*string_index as usize] {
                                ConstantPoolEntry::Utf8(s) => s.clone(),
                                _ => "\x01".repeat(n_args),
                            }
                        }
                        ConstantPoolEntry::Utf8(s) => s.clone(),
                        _ => "\x01".repeat(n_args),
                    }
                } else {
                    "\x01".repeat(n_args)
                };

                let arg_types = arg_type_chars(&descriptor);
                let mut result = Vec::new();
                let mut arg_idx = 0;
                let mut const_idx = 0usize;
                for ch in recipe.chars() {
                    if ch == '\x01' {
                        // Substitute argument — call toString() for objects.
                        if let Some(a) = args.get(arg_idx) {
                            let is_bool = arg_types.get(arg_idx) == Some(&'Z');
                            match a {
                                JValue::Int(v) if is_bool => push_utf16_str(&mut result, if *v != 0 { "true" } else { "false" }),
                                JValue::Int(v) if arg_types.get(arg_idx) == Some(&'C') => {
                                    result.push(*v as u16);
                                }
                                JValue::Int(v) => push_utf16_str(&mut result, &v.to_string()),
                                JValue::Long(v) => push_utf16_str(&mut result, &v.to_string()),
                                JValue::Float(v) => push_utf16_str(&mut result, &v.to_string()),
                                JValue::Double(v) => push_utf16_str(&mut result, &v.to_string()),
                                JValue::Ref(Some(r)) => {
                                    if let Some(s) = r.borrow().as_java_string_value().cloned() {
                                        push_java_string_value(&mut result, &s);
                                    } else {
                                        // Call toString() on the object.
                                        match self.invoke_virtual(r.clone(), &r.borrow().class_name.clone(), "toString", "()Ljava/lang/String;", vec![]) {
                                            Ok(JValue::Ref(Some(sr))) => {
                                                if let Some(s) = sr.borrow().as_java_string_value().cloned() {
                                                    push_java_string_value(&mut result, &s);
                                                } else if let Some(s) = sr.borrow().java_string_to_string_lossy() {
                                                    push_utf16_str(&mut result, &s);
                                                }
                                            }
                                            _ => push_utf16_str(&mut result, &r.borrow().class_name),
                                        }
                                    }
                                }
                                JValue::Ref(None) => push_utf16_str(&mut result, "null"),
                                _ => {}
                            }
                        }
                        arg_idx += 1;
                    } else if ch == '\x02' {
                        // \u0002 = compile-time constant from bootstrap args (index 1+).
                        // bm.bootstrap_arguments[0] is the recipe; constants start at [1].
                        let ba_idx = 1 + const_idx;
                        match bm.bootstrap_arguments.get(ba_idx) {
                            Some(&cp_idx) => match cp.get(cp_idx as usize) {
                                Some(ConstantPoolEntry::String { string_index }) => {
                                    if let Some(ConstantPoolEntry::Utf8(s)) = cp.get(*string_index as usize) {
                                        push_utf16_str(&mut result, s);
                                    }
                                }
                                Some(ConstantPoolEntry::Integer(v)) => push_utf16_str(&mut result, &v.to_string()),
                                Some(ConstantPoolEntry::Long(v)) => push_utf16_str(&mut result, &v.to_string()),
                                Some(ConstantPoolEntry::Float(v)) => {
                                    // Use Java-compatible formatting: finite values via Rust,
                                    // but infinities/NaN must match Java's Float.toString output.
                                    if v.is_infinite() {
                                        push_utf16_str(&mut result, if *v > 0.0 { "Infinity" } else { "-Infinity" });
                                    } else if v.is_nan() {
                                        push_utf16_str(&mut result, "NaN");
                                    } else {
                                        push_utf16_str(&mut result, &v.to_string());
                                    }
                                }
                                Some(ConstantPoolEntry::Double(v)) => {
                                    if v.is_infinite() {
                                        push_utf16_str(&mut result, if *v > 0.0 { "Infinity" } else { "-Infinity" });
                                    } else if v.is_nan() {
                                        push_utf16_str(&mut result, "NaN");
                                    } else {
                                        push_utf16_str(&mut result, &v.to_string());
                                    }
                                }
                                Some(ConstantPoolEntry::Utf8(s)) => push_utf16_str(&mut result, s),
                                Some(ConstantPoolEntry::Class { name_index }) => {
                                    if let Some(ConstantPoolEntry::Utf8(s)) = cp.get(*name_index as usize) {
                                        push_utf16_str(&mut result, s);
                                    }
                                }
                                Some(ConstantPoolEntry::MethodHandle { .. })
                                | Some(ConstantPoolEntry::MethodType { .. }) => {
                                    // Stable debug representation for unsupported handle/type constants.
                                    if let Some(entry) = cp.get(cp_idx as usize) {
                                        push_utf16_str(&mut result, &format!("{entry:?}"));
                                    }
                                }
                                Some(other) => {
                                    let detail = format!("unsupported \\x02 constant in StringConcatFactory recipe: {other:?}");
                                    self.throw_bootstrap_method_error(&detail);
                                    return Err(format!("java/lang/BootstrapMethodError: {detail}"));
                                }
                                None => {
                                    let detail = format!("invalid CP index {cp_idx} in StringConcatFactory recipe");
                                    self.throw_bootstrap_method_error(&detail);
                                    return Err(format!("java/lang/BootstrapMethodError: {detail}"));
                                }
                            },
                            None => {} // no constant at this index — emit nothing
                        }
                        const_idx += 1;
                    } else {
                        let mut buf = [0u16; 2];
                        let encoded = ch.encode_utf16(&mut buf);
                        result.extend_from_slice(encoded);
                    }
                }
                Ok(JValue::Ref(Some(JObject::new_string_utf16(result))))
            }

            "java/lang/runtime/SwitchBootstraps" | "java/lang/invoke/SwitchBootstraps" => {
                // typeSwitch: pop an object and an int index, push matched case index.
                let n_args = count_args(&descriptor);
                let args = pop_args(frame, n_args);
                // args[0] = object to switch on, args[1] = restart index (int)
                let obj = args.first().cloned().unwrap_or(JValue::Ref(None));
                let case_classes: Vec<String> = bm.bootstrap_arguments.iter().map(|&arg_idx| {
                    match &cp[arg_idx as usize] {
                        ConstantPoolEntry::Class { name_index } => {
                            match &cp[*name_index as usize] {
                                ConstantPoolEntry::Utf8(s) => s.clone(),
                                _ => String::new(),
                            }
                        }
                        _ => String::new(),
                    }
                }).collect();

                let matched_idx = match obj.as_ref() {
                    None => -1i32, // null → default case
                    Some(r) => {
                        let runtime_class = r.borrow().class_name.clone();
                        case_classes.iter().position(|c| self.is_instance_of(&runtime_class, c))
                            .map(|i| i as i32)
                            .unwrap_or(-1)
                    }
                };
                Ok(JValue::Int(matched_idx))
            }

            "java/lang/runtime/ObjectMethods" => {
                // ObjectMethods bootstrap — used by Java records for toString/equals/hashCode.
                //
                // Bootstrap arguments layout (JVMS §6.5 + JDK source):
                //   [0] = CONSTANT_Class   — the record class
                //   [1] = CONSTANT_String  — component names, semicolon-separated
                //   [2..] = MethodHandle   — one getter per component (invokeVirtual, ref_kind=5)
                //
                // The dynamic `method_name` is "toString", "equals", or "hashCode".
                // We return a pseudo-lambda (RecordMethod payload) that the VM
                // dispatches when the method is invoked on a record instance.
                let n_args = count_args(&descriptor);
                let _captured = pop_args(frame, n_args);

                // Extract record class name from bootstrap arg[0].
                let record_class = bm.bootstrap_arguments.first().and_then(|&idx| {
                    match cp.get(idx as usize)? {
                        ConstantPoolEntry::Class { name_index } => {
                            match cp.get(*name_index as usize)? {
                                ConstantPoolEntry::Utf8(s) => Some(s.clone()),
                                _ => None,
                            }
                        }
                        _ => None,
                    }
                }).unwrap_or_default();

                // Simple class name (after last '/') for toString output.
                let class_simple_name = record_class.split('/').last().unwrap_or(&record_class).to_owned();

                // Component names from bootstrap arg[1] (semicolon-separated string).
                let component_names: Vec<String> = bm.bootstrap_arguments.get(1).and_then(|&idx| {
                    match cp.get(idx as usize)? {
                        ConstantPoolEntry::String { string_index } => {
                            match cp.get(*string_index as usize)? {
                                ConstantPoolEntry::Utf8(s) => Some(s.clone()),
                                _ => None,
                            }
                        }
                        ConstantPoolEntry::Utf8(s) => Some(s.clone()),
                        _ => None,
                    }
                }).map(|s| if s.is_empty() { vec![] } else { s.split(';').map(|c| c.to_owned()).collect() })
                  .unwrap_or_default();

                // Getter MethodHandles from bootstrap args[2..].
                let getters: Vec<(String, String, String)> = bm.bootstrap_arguments.iter().skip(2).filter_map(|&arg_idx| {
                    match cp.get(arg_idx as usize)? {
                        ConstantPoolEntry::MethodHandle { reference_index, .. } => {
                            match cp.get(*reference_index as usize)? {
                                ConstantPoolEntry::Methodref { class_index, name_and_type_index } => {
                                    let cls = match cp.get(*class_index as usize)? {
                                        ConstantPoolEntry::Class { name_index } => {
                                            match cp.get(*name_index as usize)? {
                                                ConstantPoolEntry::Utf8(s) => s.clone(),
                                                _ => return None,
                                            }
                                        }
                                        _ => return None,
                                    };
                                    let (mname, mdesc) = match cp.get(*name_and_type_index as usize)? {
                                        ConstantPoolEntry::NameAndType { name_index, descriptor_index } => {
                                            let n = match cp.get(*name_index as usize)? {
                                                ConstantPoolEntry::Utf8(s) => s.clone(), _ => return None,
                                            };
                                            let d = match cp.get(*descriptor_index as usize)? {
                                                ConstantPoolEntry::Utf8(s) => s.clone(), _ => return None,
                                            };
                                            (n, d)
                                        }
                                        _ => return None,
                                    };
                                    Some((cls, mname, mdesc))
                                }
                                _ => None,
                            }
                        }
                        _ => None,
                    }
                }).collect();

                let obj = Rc::new(RefCell::new(JObject {
                    class_name: "$$RecordMethod".to_owned(),
                    fields: std::collections::HashMap::new(),
                    native: NativePayload::RecordMethod {
                        method: method_name,
                        class_simple_name,
                        component_names,
                        getters,
                    },
                }));
                Ok(JValue::Ref(Some(obj)))
            }

            _ => {
                // Unknown bootstrap class — throw BootstrapMethodError per JVMS §6.5.
                let detail = format!("unknown bootstrap class: {bm_class}");
                self.throw_bootstrap_method_error(&detail);
                Err(format!("java/lang/BootstrapMethodError: {detail}"))
            }
        }
    }

    // ------------------------------------------------------------------
    // Native method stubs
}
