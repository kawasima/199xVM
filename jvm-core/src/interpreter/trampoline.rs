use std::rc::Rc;

use crate::class_file::{BootstrapMethod, ConstantPoolEntry, ExceptionTableEntry};
use crate::heap::{JObject, JRef, JValue};

use super::descriptors::*;
use super::frame::*;
use super::Vm;

// ---------------------------------------------------------------------------
// Data types
// ---------------------------------------------------------------------------

/// All data needed to execute or resume a method frame.
pub(crate) struct FrameInfo {
    pub frame: Frame,
    pub code: Vec<u8>,
    pub cp: Rc<Vec<ConstantPoolEntry>>,
    pub frame_owner: String,
    pub bootstrap_methods: Vec<BootstrapMethod>,
    pub exception_table: Vec<ExceptionTableEntry>,
    /// Whether to push the return value onto the caller's operand stack.
    pub push_return: bool,
    /// Pending concat recipe state (for StringConcatFactory toString() calls).
    pub concat_state: Option<ConcatState>,
}

/// Saved state for a StringConcatFactory recipe interrupted by toString().
///
/// Currently `concat_state` is never set to `Some` because `dispatch_invokedynamic`
/// still uses the recursive path. This will be wired up when invokedynamic is
/// migrated to the trampoline (Phase 2+), at which point concat recipes that call
/// `toString()` on non-String objects will use this state machine to yield and
/// resume after the toString() frame completes.
#[allow(dead_code)]
pub(crate) struct ConcatState {
    pub recipe_chars: Vec<char>,
    pub args: Vec<JValue>,
    pub arg_types: Vec<char>,
    pub result: String,
    pub char_idx: usize,
    pub arg_idx: usize,
    pub const_idx: usize,
    pub bootstrap_arguments: Vec<u16>,
    pub cp: Rc<Vec<ConstantPoolEntry>>,
}

// ---------------------------------------------------------------------------
// Trampoline loop
// ---------------------------------------------------------------------------

impl Vm {
    /// Run an explicit call stack using a trampoline loop.
    /// Returns the final return value of the bottom-most frame.
    pub(crate) fn run_trampoline(
        &mut self,
        call_stack: &mut Vec<FrameInfo>,
    ) -> Result<JValue, String> {
        loop {
            if call_stack.is_empty() {
                return Ok(JValue::Void);
            }

            let fi = call_stack.last_mut().unwrap();
            if fi.frame.pc >= fi.code.len() {
                // Fell off the end of the method body: this indicates invalid
                // bytecode or an interpreter bug, so surface it as an error.
                return Err(format!(
                    "Execution fell off the end of method {}",
                    fi.frame_owner
                ));
            }

            let opcode_pc = fi.frame.pc;
            let opcode = fi.code[fi.frame.pc];
            fi.frame.pc += 1;

            let result = self.execute_opcode(
                &mut fi.frame, &fi.code, &fi.cp, &fi.frame_owner,
                &fi.bootstrap_methods, &fi.exception_table, opcode,
            );

            match result {
                Ok(Some(ret)) => {
                    // Method returned a value — pop frame.
                    let popped = call_stack.pop().unwrap();
                    if call_stack.is_empty() {
                        return Ok(ret);
                    }
                    let caller = call_stack.last_mut().unwrap();
                    if popped.push_return && !matches!(ret, JValue::Void) {
                        if caller.concat_state.is_some() {
                            // Return from toString() during concat.
                            feed_concat_return(caller, &ret);
                            self.try_resume_concat(call_stack)?;
                        } else {
                            caller.frame.stack.push(ret);
                        }
                    } else if caller.concat_state.is_some() {
                        self.try_resume_concat(call_stack)?;
                    }
                }
                Ok(None) => {
                    // Check if a new frame was posted by an invoke opcode.
                    if let Some(new_fi) = self.pending_frame_mut().take() {
                        call_stack.push(new_fi);
                    }
                }
                Err(err_msg) => {
                    match self.unwind_exception(call_stack, opcode_pc, &err_msg) {
                        Ok(()) => {} // handler found
                        Err(e) => return Err(e),
                    }
                }
            }
        }
    }

    /// Execute up to `max_steps` instructions on the given call stack.
    /// Returns:
    ///  - `Ok(Some(val))` if the call stack completed (all frames returned).
    ///  - `Ok(None)` if the step limit was reached (thread should yield).
    ///  - `Err(msg)` if an unhandled exception occurred.
    pub(crate) fn run_trampoline_steps(
        &mut self,
        call_stack: &mut Vec<FrameInfo>,
        max_steps: usize,
    ) -> Result<Option<JValue>, String> {
        use super::ThreadState;

        let mut steps = 0;
        loop {
            if call_stack.is_empty() {
                return Ok(Some(JValue::Void));
            }
            if steps >= max_steps {
                return Ok(None); // yield — time slice exhausted
            }
            // If the current thread was blocked (e.g., by join() or sleep()),
            // yield immediately so the scheduler can switch threads.
            if self.scheduler.current_thread().state != ThreadState::Runnable {
                return Ok(None);
            }

            let fi = call_stack.last_mut().unwrap();
            if fi.frame.pc >= fi.code.len() {
                return Err(format!(
                    "Execution fell off the end of method {}",
                    fi.frame_owner
                ));
            }

            let opcode_pc = fi.frame.pc;
            let opcode = fi.code[fi.frame.pc];
            fi.frame.pc += 1;
            steps += 1;

            let result = self.execute_opcode(
                &mut fi.frame, &fi.code, &fi.cp, &fi.frame_owner,
                &fi.bootstrap_methods, &fi.exception_table, opcode,
            );

            match result {
                Ok(Some(ret)) => {
                    let popped = call_stack.pop().unwrap();
                    if call_stack.is_empty() {
                        return Ok(Some(ret));
                    }
                    let caller = call_stack.last_mut().unwrap();
                    if popped.push_return && !matches!(ret, JValue::Void) {
                        if caller.concat_state.is_some() {
                            feed_concat_return(caller, &ret);
                            self.try_resume_concat(call_stack)?;
                        } else {
                            caller.frame.stack.push(ret);
                        }
                    } else if caller.concat_state.is_some() {
                        self.try_resume_concat(call_stack)?;
                    }
                }
                Ok(None) => {
                    if let Some(new_fi) = self.pending_frame_mut().take() {
                        call_stack.push(new_fi);
                    }
                }
                Err(err_msg) => {
                    match self.unwind_exception(call_stack, opcode_pc, &err_msg) {
                        Ok(()) => {}
                        Err(e) => return Err(e),
                    }
                }
            }
        }
    }

    /// Run all green threads to completion using cooperative scheduling.
    /// The main thread (id=0) call_stack must already be populated.
    /// Returns the final return value from the main thread.
    pub(crate) fn run_all_threads(&mut self) -> Result<JValue, String> {
        use super::ThreadState;
        use super::TIME_SLICE;

        loop {
            let current_id = self.scheduler.current_thread().id;
            let state = self.scheduler.current_thread().state.clone();

            match state {
                ThreadState::Runnable => {
                    // Take the call_stack out of ThreadContext for borrowing.
                    let mut call_stack = std::mem::take(
                        &mut self.scheduler.current_thread_mut().call_stack
                    );

                    let result = self.run_trampoline_steps(&mut call_stack, TIME_SLICE);

                    // Put the call_stack back.
                    self.scheduler.current_thread_mut().call_stack = call_stack;

                    match result {
                        Ok(Some(val)) => {
                            // Thread completed.
                            self.scheduler.current_thread_mut().state = ThreadState::Terminated;
                            if current_id == 0 {
                                // Main thread finished — drain remaining threads
                                // then return the result.
                                self.drain_non_main_threads()?;
                                // Restore current thread to main so that
                                // subsequent VM calls (pending_exception,
                                // Thread.currentThread, monitors) see the
                                // correct context.
                                self.scheduler.reset_to_main();
                                return Ok(val);
                            }
                        }
                        Ok(None) => {
                            // Time slice exhausted — yield to next thread.
                        }
                        Err(e) => {
                            // Unhandled exception — terminate thread.
                            self.scheduler.current_thread_mut().state = ThreadState::Terminated;
                            if current_id == 0 {
                                return Err(e);
                            }
                            eprintln!("Thread {} terminated with exception: {e}", current_id);
                        }
                    }
                }
                ThreadState::Terminated => {
                    // Skip terminated threads.
                }
                ThreadState::Yielded | ThreadState::Sleeping => {
                    // Voluntary yield — set back to Runnable so this thread
                    // can be scheduled again after other threads get a turn.
                    self.scheduler.current_thread_mut().state = ThreadState::Runnable;
                }
                ThreadState::Joining(_)
                | ThreadState::WaitingOnMonitor(_) | ThreadState::WaitingOnCondition(_) => {
                    // Blocked — skip to next thread.
                }
            }

            // Wake joiners/sleeping threads before trying to advance.
            self.scheduler.wake_joiners();

            // Advance to next runnable thread.
            if !self.scheduler.advance() {
                // No runnable threads — check for deadlock or all terminated.
                if self.scheduler.all_terminated() {
                    return Ok(JValue::Void);
                }
                // Check if only blocked threads remain (potential deadlock).
                if self.scheduler.runnable_count() == 0 {
                    return Err("Deadlock: no runnable threads".to_owned());
                }
            }
        }
    }

    /// After the main thread finishes, run remaining non-daemon threads
    /// to completion (or timeout).
    fn drain_non_main_threads(&mut self) -> Result<(), String> {
        use super::ThreadState;
        use super::TIME_SLICE;

        // Simple bound to prevent infinite drain.
        let max_iterations = 100_000;
        let mut iterations = 0;

        while !self.scheduler.only_main_alive() && iterations < max_iterations {
            iterations += 1;
            self.scheduler.wake_joiners();

            if !self.scheduler.advance() {
                break;
            }

            let state = self.scheduler.current_thread().state.clone();
            match state {
                ThreadState::Yielded | ThreadState::Sleeping => {
                    self.scheduler.current_thread_mut().state = ThreadState::Runnable;
                }
                ThreadState::Runnable => {}
                _ => continue,
            }

            let mut call_stack = std::mem::take(
                &mut self.scheduler.current_thread_mut().call_stack
            );
            let result = self.run_trampoline_steps(&mut call_stack, TIME_SLICE);
            self.scheduler.current_thread_mut().call_stack = call_stack;

            match result {
                Ok(Some(_)) | Err(_) => {
                    self.scheduler.current_thread_mut().state = ThreadState::Terminated;
                }
                Ok(None) => {}
            }
        }
        if !self.scheduler.only_main_alive() {
            let summary = self.scheduler.alive_thread_summary();
            if self.scheduler.runnable_count() == 0 {
                return Err(format!("Thread drain deadlock: all remaining threads blocked [{}]", summary));
            }
            return Err(format!("Thread drain timeout after {} iterations: [{}]", max_iterations, summary));
        }
        Ok(())
    }

    /// Search for an exception handler up the call stack.
    fn unwind_exception(
        &mut self,
        call_stack: &mut Vec<FrameInfo>,
        initial_opcode_pc: usize,
        err_msg: &str,
    ) -> Result<(), String> {
        let mut first = true;
        let mut trace = String::new();
        while let Some(fi) = call_stack.last_mut() {
            fi.concat_state = None;
            let pc = if first { initial_opcode_pc } else { fi.frame.pc.saturating_sub(1) };
            first = false;

            if let Some((handler_pc, exc_obj)) = self.find_exception_handler(
                &fi.frame, &fi.exception_table, &fi.cp, pc, err_msg,
            ) {
                fi.frame.stack.clear();
                fi.frame.stack.push(exc_obj);
                fi.frame.pc = handler_pc;
                return Ok(());
            }
            if trace.is_empty() {
                trace.push_str(err_msg);
            }
            trace.push_str("\n  at ");
            trace.push_str(&fi.frame_owner);
            call_stack.pop();
        }
        Err(if trace.is_empty() { err_msg.to_owned() } else { trace })
    }

    /// Continue a StringConcatFactory recipe. May push new frames for toString().
    fn try_resume_concat(
        &mut self,
        call_stack: &mut Vec<FrameInfo>,
    ) -> Result<(), String> {
        loop {
            // Check if there's a concat state to process.
            {
                let fi = call_stack.last().unwrap();
                if fi.concat_state.is_none() {
                    return Ok(());
                }
            }

            // Process one character at a time, breaking out when we need to yield.
            enum Action {
                Continue,
                NativeToString(JRef, String),
                Finished(String),
            }

            let action = {
                let fi = call_stack.last_mut().unwrap();
                let cs = fi.concat_state.as_mut().unwrap();

                if cs.char_idx >= cs.recipe_chars.len() {
                    // Recipe finished.
                    Action::Finished(std::mem::take(&mut cs.result))
                } else {
                    let ch = cs.recipe_chars[cs.char_idx];
                    cs.char_idx += 1;

                    if ch == '\x01' {
                        let mut need_tostring: Option<(JRef, String)> = None;
                        if let Some(a) = cs.args.get(cs.arg_idx) {
                            let is_bool = cs.arg_types.get(cs.arg_idx) == Some(&'Z');
                            match a {
                                JValue::Int(v) if is_bool => cs.result.push_str(if *v != 0 { "true" } else { "false" }),
                                JValue::Int(v) => cs.result.push_str(&v.to_string()),
                                JValue::Long(v) => cs.result.push_str(&v.to_string()),
                                JValue::Float(v) => cs.result.push_str(&v.to_string()),
                                JValue::Double(v) => cs.result.push_str(&v.to_string()),
                                JValue::Ref(Some(r)) => {
                                    if let Some(s) = r.borrow().as_java_string() {
                                        cs.result.push_str(s);
                                    } else {
                                        need_tostring = Some((r.clone(), r.borrow().class_name.clone()));
                                    }
                                }
                                JValue::Ref(None) => cs.result.push_str("null"),
                                _ => {}
                            }
                        }
                        cs.arg_idx += 1;
                        if let Some((r, cn)) = need_tostring {
                            Action::NativeToString(r, cn)
                        } else {
                            Action::Continue
                        }
                    } else if ch == '\x02' {
                        expand_concat_const(cs);
                        cs.const_idx += 1;
                        Action::Continue
                    } else {
                        cs.result.push(ch);
                        Action::Continue
                    }
                }
            }; // borrows dropped here

            match action {
                Action::Continue => continue,
                Action::NativeToString(r, cn) => {
                    // Try bytecode frame first.
                    let frame_opt = self.build_virtual_frame_inner(
                        r.clone(), &cn, "toString",
                        "()Ljava/lang/String;", vec![], true,
                    )?;
                    if let Some(frame_info) = frame_opt {
                        call_stack.push(frame_info);
                        return Ok(());
                    }
                    // Native toString.
                    let s = if let Some(v) = self.native_virtual(
                        &r, &cn, "toString", "()Ljava/lang/String;", &[],
                    ) {
                        if let JValue::Ref(Some(sr)) = v {
                            sr.borrow().as_java_string().unwrap_or("").to_owned()
                        } else {
                            cn.clone()
                        }
                    } else {
                        cn.clone()
                    };
                    let fi = call_stack.last_mut().unwrap();
                    let cs = fi.concat_state.as_mut().unwrap();
                    cs.result.push_str(&s);
                }
                Action::Finished(result_str) => {
                    let fi = call_stack.last_mut().unwrap();
                    fi.concat_state = None;
                    fi.frame.stack.push(JValue::Ref(Some(JObject::new_string(result_str))));
                    return Ok(());
                }
            }
        }
    }

    // -----------------------------------------------------------------------
    // Frame builders — resolve method, build locals, return FrameInfo.
    // Returns None if the method is native (caller must handle).
    // -----------------------------------------------------------------------

    /// Resolve descriptor and apply varargs synthesis for a static call.
    /// Returns `(resolved_descriptor, adjusted_args)`, or `None` if the method
    /// cannot be found at all (no flags).
    pub(crate) fn prepare_static_args(
        &mut self,
        class_name: &str,
        method_name: &str,
        descriptor: &str,
        args: Vec<JValue>,
    ) -> Option<(String, Vec<JValue>)> {
        let method_flags = self.find_method_flags(class_name, method_name, descriptor);
        let resolved_descriptor = if method_flags.is_some() {
            descriptor.to_owned()
        } else {
            self.find_method_real_descriptor(class_name, method_name, descriptor)
                .unwrap_or_else(|| descriptor.to_owned())
        };
        let descriptor_r = resolved_descriptor.as_str();
        let method_flags = if method_flags.is_none() {
            self.find_method_flags(class_name, method_name, descriptor_r)
        } else {
            method_flags
        };
        if method_flags.is_none() {
            return None;
        }

        let mut args = args;
        let expected = count_args(descriptor_r);
        if args.len() < expected {
            if method_flags.map(|f| f & 0x0080 != 0).unwrap_or(false) && expected - args.len() == 1 {
                args.push(JValue::Ref(Some(JObject::new_array("[Ljava/lang/Object;", vec![]))));
            }
        }
        Some((resolved_descriptor, args))
    }

    /// Build a FrameInfo for a static invocation. Returns None if native.
    /// Caller should use `prepare_static_args` first to normalize descriptor/args.
    pub(crate) fn build_static_frame(
        &mut self,
        class_name: &str,
        method_name: &str,
        descriptor: &str,
        args: Vec<JValue>,
        push_return: bool,
    ) -> Result<Option<FrameInfo>, String> {
        let info = self.resolve_method_exec_info(class_name, method_name, descriptor)
            .ok_or_else(|| format!("Method not found: {class_name}.{method_name}{descriptor}"))?;
        if !info.has_code { return Ok(None); }

        let (param_tokens, _) = Self::parse_method_descriptor_tokens(descriptor);
        let req: usize = param_tokens.iter().map(|t| if t == "J" || t == "D" { 2 } else { 1 }).sum();
        let mut locals = vec![JValue::Void; info.max_locals.max(req)];
        let mut li = 0usize;
        for (a, t) in args.into_iter().zip(param_tokens.iter()) {
            if li >= locals.len() { break; }
            locals[li] = self.adapt_value_for_descriptor(t, a);
            li += if t == "J" || t == "D" { 2 } else { 1 };
        }

        let fo = format!("{}.{method_name}{}", info.class_name, info.descriptor);
        Ok(Some(FrameInfo {
            frame: Frame { locals, stack: Vec::new(), pc: 0 },
            code: info.code, cp: info.cp, frame_owner: fo,
            bootstrap_methods: info.bootstrap_methods, exception_table: info.exception_table,
            push_return, concat_state: None,
        }))
    }

    /// Inner helper for building a virtual frame (no lambda handling).
    /// Returns None if the method is native, a lambda, or not found in bytecode.
    pub(crate) fn build_virtual_frame_inner(
        &mut self,
        this: JRef,
        class_name: &str,
        method_name: &str,
        descriptor: &str,
        args: Vec<JValue>,
        push_return: bool,
    ) -> Result<Option<FrameInfo>, String> {
        // Pure Rust lambda closures are handled inline — not via frames.
        let is_bytecode_lambda;
        {
            let borrow = this.borrow();
            if matches!(borrow.native, crate::heap::NativePayload::Lambda(_)) {
                return Ok(None);
            }
            is_bytecode_lambda = matches!(borrow.native, crate::heap::NativePayload::BytecodeLambda { .. });
        }

        let runtime_class = this.borrow().class_name.clone();
        // For bytecode lambdas, resolve on the interface class (class_name) so that
        // default methods are found.  For normal objects, use the runtime class.
        let resolve_class = if is_bytecode_lambda {
            class_name.to_owned()
        } else if self.classes.contains_key(&runtime_class) {
            runtime_class.clone()
        } else {
            class_name.to_owned()
        };

        let resolved = if self.method_exists(&resolve_class, method_name, descriptor) {
            descriptor.to_owned()
        } else {
            self.find_method_real_descriptor(&resolve_class, method_name, descriptor)
                .unwrap_or_else(|| descriptor.to_owned())
        };
        let desc = resolved.as_str();

        if !self.method_exists(&resolve_class, method_name, desc) {
            return Ok(None);
        }

        let info = self.resolve_method_exec_info(&resolve_class, method_name, desc).unwrap();

        if info.access_flags & 0x0400 != 0 {
            // For bytecode lambdas, abstract methods (the SAM) should be handled
            // by invoke_virtual's SAM dispatch, not as an error.
            if is_bytecode_lambda {
                return Ok(None);
            }
            let exc = JObject::new("java/lang/AbstractMethodError");
            let ms = format!("{}.{method_name}{}", info.class_name, info.descriptor);
            let msg = self.intern_string(ms.clone());
            exc.borrow_mut().fields.insert("detailMessage".to_owned(), JValue::Ref(Some(msg)));
            *self.pending_exception_mut() = Some(exc);
            return Err(format!("java/lang/AbstractMethodError: {ms}"));
        }

        if !info.has_code { return Ok(None); }

        let (param_tokens, _) = Self::parse_method_descriptor_tokens(desc);
        let req = 1 + param_tokens.iter().map(|t| if t == "J" || t == "D" { 2 } else { 1 }).sum::<usize>();
        let total = info.max_locals.max(req);
        let mut locals = vec![JValue::Void; total];
        locals[0] = JValue::Ref(Some(this));
        let mut li = 1usize;
        for (a, t) in args.into_iter().zip(param_tokens.iter()) {
            if li >= locals.len() { break; }
            locals[li] = self.adapt_value_for_descriptor(t, a);
            li += if t == "J" || t == "D" { 2 } else { 1 };
        }

        let fo = format!("{}.{method_name}{}", info.class_name, info.descriptor);
        Ok(Some(FrameInfo {
            frame: Frame { locals, stack: Vec::new(), pc: 0 },
            code: info.code, cp: info.cp, frame_owner: fo,
            bootstrap_methods: info.bootstrap_methods, exception_table: info.exception_table,
            push_return, concat_state: None,
        }))
    }

    /// Inner helper for building an invokespecial frame.
    pub(crate) fn build_special_frame_inner(
        &mut self,
        this: JRef,
        class_name: &str,
        method_name: &str,
        descriptor: &str,
        args: Vec<JValue>,
        push_return: bool,
    ) -> Result<Option<FrameInfo>, String> {
        let resolved = if self.method_exists(class_name, method_name, descriptor) {
            descriptor.to_owned()
        } else {
            self.find_method_real_descriptor(class_name, method_name, descriptor)
                .unwrap_or_else(|| descriptor.to_owned())
        };
        let desc = resolved.as_str();

        if !self.method_exists(class_name, method_name, desc) {
            return Ok(None);
        }

        let info = self.resolve_method_exec_info(class_name, method_name, desc).unwrap();

        if info.access_flags & 0x0400 != 0 {
            let exc = JObject::new("java/lang/AbstractMethodError");
            let ms = format!("{}.{method_name}{}", info.class_name, info.descriptor);
            let msg = self.intern_string(ms.clone());
            exc.borrow_mut().fields.insert("detailMessage".to_owned(), JValue::Ref(Some(msg)));
            *self.pending_exception_mut() = Some(exc);
            return Err(format!("java/lang/AbstractMethodError: {ms}"));
        }

        if !info.has_code { return Ok(None); }

        let (param_tokens, _) = Self::parse_method_descriptor_tokens(desc);
        let req = 1 + param_tokens.iter().map(|t| if t == "J" || t == "D" { 2 } else { 1 }).sum::<usize>();
        let total = info.max_locals.max(req);
        let mut locals = vec![JValue::Void; total];
        locals[0] = JValue::Ref(Some(this));
        let mut li = 1usize;
        for (a, t) in args.into_iter().zip(param_tokens.iter()) {
            if li >= locals.len() { break; }
            locals[li] = self.adapt_value_for_descriptor(t, a);
            li += if t == "J" || t == "D" { 2 } else { 1 };
        }

        let fo = format!("{}.{method_name}{}", info.class_name, info.descriptor);
        Ok(Some(FrameInfo {
            frame: Frame { locals, stack: Vec::new(), pc: 0 },
            code: info.code, cp: info.cp, frame_owner: fo,
            bootstrap_methods: info.bootstrap_methods, exception_table: info.exception_table,
            push_return, concat_state: None,
        }))
    }
}

fn feed_concat_return(fi: &mut FrameInfo, ret: &JValue) {
    if let Some(ref mut cs) = fi.concat_state {
        if let JValue::Ref(Some(sr)) = ret {
            if let Some(s) = sr.borrow().as_java_string() {
                cs.result.push_str(s);
            }
        }
    }
}

fn expand_concat_const(cs: &mut ConcatState) {
    use crate::class_file::ConstantPoolEntry;
    let ba_idx = 1 + cs.const_idx;
    if let Some(&cp_idx) = cs.bootstrap_arguments.get(ba_idx) {
        match cs.cp.get(cp_idx as usize) {
            Some(ConstantPoolEntry::String { string_index }) => {
                if let Some(ConstantPoolEntry::Utf8(s)) = cs.cp.get(*string_index as usize) {
                    cs.result.push_str(s);
                }
            }
            Some(ConstantPoolEntry::Integer(v)) => cs.result.push_str(&v.to_string()),
            Some(ConstantPoolEntry::Long(v)) => cs.result.push_str(&v.to_string()),
            Some(ConstantPoolEntry::Float(v)) => {
                if v.is_infinite() {
                    cs.result.push_str(if *v > 0.0 { "Infinity" } else { "-Infinity" });
                } else if v.is_nan() {
                    cs.result.push_str("NaN");
                } else {
                    cs.result.push_str(&v.to_string());
                }
            }
            Some(ConstantPoolEntry::Double(v)) => {
                if v.is_infinite() {
                    cs.result.push_str(if *v > 0.0 { "Infinity" } else { "-Infinity" });
                } else if v.is_nan() {
                    cs.result.push_str("NaN");
                } else {
                    cs.result.push_str(&v.to_string());
                }
            }
            Some(ConstantPoolEntry::Utf8(s)) => cs.result.push_str(s),
            Some(ConstantPoolEntry::Class { name_index }) => {
                if let Some(ConstantPoolEntry::Utf8(s)) = cs.cp.get(*name_index as usize) {
                    cs.result.push_str(s);
                }
            }
            _ => {}
        }
    }
}
