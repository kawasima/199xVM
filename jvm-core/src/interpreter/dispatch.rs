use std::cell::RefCell;
use std::rc::Rc;

use crate::class_file::{BootstrapMethod, ConstantPoolEntry};
use crate::heap::{JObject, JRef, JValue, NativePayload};

use super::Vm;
use super::descriptors::*;
use super::frame::*;
use super::trampoline::FrameInfo;


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
        idx: u16,
        frame: &mut Frame,
    ) -> Result<Option<JValue>, String> {
        let (class_name, method_name, descriptor) = resolve_methodref(cp, idx);
        self.ensure_class_init(&class_name)?;
        let n_args = count_args(&descriptor);
        let args = pop_args(frame, n_args);

        // Normalize descriptor and args (varargs synthesis) before branching.
        let orig_args = args.clone();
        let (desc, args) = match self.prepare_static_args(&class_name, &method_name, &descriptor, args) {
            Some(pair) => pair,
            None => {
                // Method flags not found — fall back to invoke_static with original args.
                let result = self.invoke_static(&class_name, &method_name, &descriptor, orig_args)?;
                if !matches!(result, JValue::Void) { frame.stack.push(result); }
                return Ok(None);
            }
        };

        let push_return = !desc.ends_with(")V");
        match self.build_static_frame(&class_name, &method_name, &desc, args.clone(), push_return)? {
            Some(fi) => {
                *self.pending_frame_mut() = Some(fi);
                Ok(None)
            }
            None => {
                // Native fallback — args already have varargs synthesis applied.
                let result = self.invoke_static(&class_name, &method_name, &desc, args)?;
                if !matches!(result, JValue::Void) {
                    frame.stack.push(result);
                }
                Ok(None)
            }
        }
    }

    pub(super) fn dispatch_virtual(
        &mut self,
        cp: &[ConstantPoolEntry],
        idx: u16,
        frame: &mut Frame,
    ) -> Result<Option<JValue>, String> {
        let (class_name, method_name, descriptor) = resolve_methodref(cp, idx);
        let n_args = count_args(&descriptor);
        let args = pop_args(frame, n_args);
        let this_val = frame.stack.pop().unwrap();
        match this_val {
            JValue::Ref(Some(r)) => {
                let push_return = !descriptor.ends_with(")V");
                self.dispatch_virtual_on_ref(r, &class_name, &method_name, &descriptor, args, push_return, frame)
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
                // Native or non-lambda — fall back to recursive invoke_virtual.
                let result = self.invoke_virtual(r, class_name, method_name, descriptor, args)?;
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
        frame: &mut Frame,
    ) -> Result<Option<JValue>, String> {
        let (class_name, method_name, descriptor) = resolve_methodref(cp, idx);
        let n_args = count_args(&descriptor);
        let args = pop_args(frame, n_args);
        let this_val = frame.stack.pop().unwrap();
        match this_val {
            JValue::Ref(Some(r)) => {
                if method_name == "<init>" {
                    if class_name == "java/lang/String" {
                        let s = self.string_from_init_args(&descriptor, &args, &r);
                        r.borrow_mut().native = NativePayload::JavaString(s);
                        return Ok(None); // void
                    }
                    let has_method = self.method_exists(&class_name, &method_name, &descriptor);
                    if !has_method {
                        return Ok(None); // no-op
                    }
                }
                let push_return = !descriptor.ends_with(")V");
                match self.build_special_frame_inner(r.clone(), &class_name, &method_name, &descriptor, args.clone(), push_return)? {
                    Some(fi) => {
                        *self.pending_frame_mut() = Some(fi);
                        Ok(None)
                    }
                    None => {
                        let result = self.invoke_special(r, &class_name, &method_name, &descriptor, args)?;
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
        frame: &mut Frame,
    ) -> Result<Option<JValue>, String> {
        let (class_name, method_name, descriptor) = resolve_methodref(cp, idx);
        let n_args = count_args(&descriptor);
        let args = pop_args(frame, n_args);

        let is_static = self.find_method_flags(&class_name, &method_name, &descriptor)
            .map(|flags| flags & 0x0008 != 0)
            .unwrap_or(false);
        if is_static {
            let push_return = !descriptor.ends_with(")V");
            match self.build_static_frame(&class_name, &method_name, &descriptor, args.clone(), push_return)? {
                Some(fi) => {
                    *self.pending_frame_mut() = Some(fi);
                    return Ok(None);
                }
                None => {
                    let result = self.invoke_static(&class_name, &method_name, &descriptor, args)?;
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
                let push_return = !descriptor.ends_with(")V");
                self.dispatch_virtual_on_ref(r, &class_name, &method_name, &descriptor, args, push_return, frame)
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
                let mut result = String::new();
                let mut arg_idx = 0;
                let mut const_idx = 0usize;
                for ch in recipe.chars() {
                    if ch == '\x01' {
                        // Substitute argument — call toString() for objects.
                        if let Some(a) = args.get(arg_idx) {
                            let is_bool = arg_types.get(arg_idx) == Some(&'Z');
                            match a {
                                JValue::Int(v) if is_bool => result.push_str(if *v != 0 { "true" } else { "false" }),
                                JValue::Int(v) => result.push_str(&v.to_string()),
                                JValue::Long(v) => result.push_str(&v.to_string()),
                                JValue::Float(v) => result.push_str(&v.to_string()),
                                JValue::Double(v) => result.push_str(&v.to_string()),
                                JValue::Ref(Some(r)) => {
                                    if let Some(s) = r.borrow().as_java_string() {
                                        result.push_str(s);
                                    } else {
                                        // Call toString() on the object.
                                        match self.invoke_virtual(r.clone(), &r.borrow().class_name.clone(), "toString", "()Ljava/lang/String;", vec![]) {
                                            Ok(JValue::Ref(Some(sr))) => {
                                                if let Some(s) = sr.borrow().as_java_string() {
                                                    result.push_str(s);
                                                }
                                            }
                                            _ => result.push_str(&r.borrow().class_name),
                                        }
                                    }
                                }
                                JValue::Ref(None) => result.push_str("null"),
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
                                        result.push_str(s);
                                    }
                                }
                                Some(ConstantPoolEntry::Integer(v)) => result.push_str(&v.to_string()),
                                Some(ConstantPoolEntry::Long(v)) => result.push_str(&v.to_string()),
                                Some(ConstantPoolEntry::Float(v)) => {
                                    // Use Java-compatible formatting: finite values via Rust,
                                    // but infinities/NaN must match Java's Float.toString output.
                                    if v.is_infinite() {
                                        result.push_str(if *v > 0.0 { "Infinity" } else { "-Infinity" });
                                    } else if v.is_nan() {
                                        result.push_str("NaN");
                                    } else {
                                        result.push_str(&v.to_string());
                                    }
                                }
                                Some(ConstantPoolEntry::Double(v)) => {
                                    if v.is_infinite() {
                                        result.push_str(if *v > 0.0 { "Infinity" } else { "-Infinity" });
                                    } else if v.is_nan() {
                                        result.push_str("NaN");
                                    } else {
                                        result.push_str(&v.to_string());
                                    }
                                }
                                Some(ConstantPoolEntry::Utf8(s)) => result.push_str(s),
                                Some(ConstantPoolEntry::Class { name_index }) => {
                                    if let Some(ConstantPoolEntry::Utf8(s)) = cp.get(*name_index as usize) {
                                        result.push_str(s);
                                    }
                                }
                                Some(ConstantPoolEntry::MethodHandle { .. })
                                | Some(ConstantPoolEntry::MethodType { .. }) => {
                                    // Stable debug representation for unsupported handle/type constants.
                                    if let Some(entry) = cp.get(cp_idx as usize) {
                                        result.push_str(&format!("{entry:?}"));
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
                        result.push(ch);
                    }
                }
                Ok(JValue::Ref(Some(JObject::new_string(result))))
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
