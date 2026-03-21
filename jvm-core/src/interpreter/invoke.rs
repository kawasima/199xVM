
use crate::heap::{JObject, JRef, JValue, NativePayload};

use super::Vm;
use super::descriptors::*;

impl Vm {
    fn static_native_args_from_info(
        &mut self,
        info: &super::MethodExecInfo,
        args: Vec<JValue>,
    ) -> Vec<JValue> {
        let req = info.param_slot_count;
        let mut locals = vec![JValue::Void; info.max_locals.max(req)];
        let mut li = 0usize;
        for (a, t) in args.into_iter().zip(info.param_tokens.iter()) {
            if li >= locals.len() {
                break;
            }
            locals[li] = self.adapt_value_for_descriptor(t, a);
            li += if t == "J" || t == "D" { 2 } else { 1 };
        }
        let mut native_args = Vec::with_capacity(info.param_tokens.len());
        let mut slot = 0usize;
        for t in info.param_tokens.iter() {
            if slot < locals.len() {
                native_args.push(locals[slot].clone());
            }
            slot += if t == "J" || t == "D" { 2 } else { 1 };
        }
        native_args
    }

    pub(crate) fn invoke_static_native_from_info(
        &mut self,
        info: &super::MethodExecInfo,
        method_name: &str,
        args: Vec<JValue>,
    ) -> Result<JValue, String> {
        let native_args = self.static_native_args_from_info(info, args);
        if let Some(v) = self.native_static(&info.class_name, method_name, &info.descriptor, &native_args) {
            if let Some(err) = self.pending_exception_err() {
                return Err(err);
            }
            return Ok(v);
        }
        Err(format!(
            "Native stub not found for: {}.{method_name}{}",
            info.class_name, info.descriptor
        ))
    }

    /// Execute a static method and return its result.
    /// Uses the trampoline for bytecode methods, falls back to native for stubs.
    pub fn invoke_static(
        &mut self,
        class_name: &str,
        method_name: &str,
        descriptor: &str,
        args: Vec<JValue>,
    ) -> Result<JValue, String> {
        // Normalize descriptor and args (varargs synthesis) once, shared by
        // both the bytecode (trampoline) and native paths.
        let orig_args = args;
        let (resolved_desc, args) = match self.prepare_static_args(class_name, method_name, descriptor, orig_args.clone()) {
            Some(pair) => pair,
            None => {
                // Method flags not found — try native stubs with original args/descriptor.
                if let Some(v) = self.native_static(class_name, method_name, descriptor, &orig_args) {
                    if let Some(err) = self.pending_exception_err() { return Err(err); }
                    return Ok(v);
                }
                return Err(format!("Method not found: {class_name}.{method_name}{descriptor}"));
            }
        };
        let desc = resolved_desc.as_str();

        match self.build_static_frame(class_name, method_name, desc, args.clone(), true)? {
            Some(fi) => {
                let frame_owner = fi.frame_owner.to_string();
                let mut call_stack = vec![fi];
                self.run_trampoline(&mut call_stack)
                    .map_err(|e| format!("{e}\n  at {frame_owner}"))
            }
            None => {
                // Native fallback — args already have varargs synthesis applied.
                if let Some(v) = self.native_static(class_name, method_name, desc, &args) {
                    if let Some(err) = self.pending_exception_err() { return Err(err); }
                    return Ok(v);
                }
                // Try with full locals construction for proper slot mapping.
                let info = match self.resolve_method_exec_info(class_name, method_name, desc) {
                    Some(info) => info,
                    None => return Err(format!("Method not found: {class_name}.{method_name}{desc}")),
                };
                if !info.has_code {
                    return self.invoke_static_native_from_info(&info, method_name, args);
                }
                Err(format!("Method not found: {}.{method_name}{}", info.class_name, info.descriptor))
            }
        }
    }

    /// Execute an instance method with invokespecial semantics.
    pub fn invoke_special(
        &mut self,
        this: JRef,
        class_name: &str,
        method_name: &str,
        descriptor: &str,
        args: Vec<JValue>,
    ) -> Result<JValue, String> {
        match self.build_special_frame_inner(this.clone(), class_name, method_name, descriptor, args.clone(), true)? {
            Some(fi) => {
                let frame_owner = fi.frame_owner.to_string();
                let mut call_stack = vec![fi];
                self.run_trampoline(&mut call_stack)
                    .map_err(|e| format!("{e}\n  at {frame_owner}"))
            }
            None => {
                // Native fallback.
                let resolved = self
                    .resolve_method_signature(class_name, method_name, descriptor)
                    .map(|(resolved, _)| resolved)
                    .unwrap_or_else(|| descriptor.to_owned());
                let desc = resolved.as_str();
                if let Some(v) = self.native_virtual(&this, class_name, method_name, desc, &args) {
                    if let Some(err) = self.pending_exception_err() { return Err(err); }
                    return Ok(v);
                }
                Err(format!("Special method not found: {class_name}.{method_name}{desc}"))
            }
        }
    }

    /// Execute an instance method (virtual dispatch).
    pub fn invoke_virtual(
        &mut self,
        this: JRef,
        class_name: &str,
        method_name: &str,
        descriptor: &str,
        args: Vec<JValue>,
    ) -> Result<JValue, String> {
        let runtime_class = this.borrow().class_name.clone();

        // RecordMethod dispatch — ObjectMethods bootstrap for toString/equals/hashCode.
        let record_method_info = match &this.borrow().native {
            NativePayload::RecordMethod { method, class_simple_name, component_names, getters } => {
                Some((method.clone(), class_simple_name.clone(), component_names.clone(), getters.clone()))
            }
            _ => None,
        };
        if let Some((rm_method, class_simple_name, component_names, getters)) = record_method_info {
            if method_name == rm_method {
                match rm_method.as_str() {
                    "toString" => {
                        // args[0] = the record instance
                        let recv = args.into_iter().next().unwrap_or(JValue::Ref(None));
                        if let JValue::Ref(Some(recv_ref)) = recv {
                            let mut parts = Vec::new();
                            for ((getter_class, getter_method, getter_desc), comp_name) in getters.iter().zip(component_names.iter()) {
                                let val = self.invoke_virtual(recv_ref.clone(), getter_class, getter_method, getter_desc, vec![])?;
                                let s = self.jvalue_to_string(val)?;
                                parts.push(format!("{comp_name}={s}"));
                            }
                            let result = format!("{}[{}]", class_simple_name, parts.join(", "));
                            return Ok(JValue::Ref(Some(JObject::new_string(result))));
                        }
                        return Ok(JValue::Ref(Some(JObject::new_string(format!("{}[]", class_simple_name)))));
                    }
                    "equals" => {
                        // args[0] = this record, args[1] = other object
                        let mut iter = args.into_iter();
                        let recv = iter.next().unwrap_or(JValue::Ref(None));
                        let other = iter.next().unwrap_or(JValue::Ref(None));
                        let (recv_ref, other_ref) = match (recv, other) {
                            (JValue::Ref(Some(a)), JValue::Ref(Some(b))) => (a, b),
                            _ => return Ok(JValue::Int(0)),
                        };
                        // Must be same class.
                        if recv_ref.borrow().class_name != other_ref.borrow().class_name {
                            return Ok(JValue::Int(0));
                        }
                        for (getter_class, getter_method, getter_desc) in &getters {
                            let v1 = self.invoke_virtual(recv_ref.clone(), getter_class, getter_method, getter_desc, vec![])?;
                            let v2 = self.invoke_virtual(other_ref.clone(), getter_class, getter_method, getter_desc, vec![])?;
                            if !self.jvalue_equals(&v1, &v2)? {
                                return Ok(JValue::Int(0));
                            }
                        }
                        return Ok(JValue::Int(1));
                    }
                    "hashCode" => {
                        // args[0] = the record instance
                        let recv = args.into_iter().next().unwrap_or(JValue::Ref(None));
                        if let JValue::Ref(Some(recv_ref)) = recv {
                            let mut hash: i32 = 0;
                            for (getter_class, getter_method, getter_desc) in &getters {
                                let val = self.invoke_virtual(recv_ref.clone(), getter_class, getter_method, getter_desc, vec![])?;
                                let comp_hash = self.jvalue_hash(&val)?;
                                hash = hash.wrapping_mul(31).wrapping_add(comp_hash);
                            }
                            return Ok(JValue::Int(hash));
                        }
                        return Ok(JValue::Int(0));
                    }
                    _ => {}
                }
            }
        }

        // Lambda dispatch — handle inline.
        let lambda_info = match &this.borrow().native {
            NativePayload::BytecodeLambda { sam_method, sam_desc, impl_class, impl_method, impl_desc, ref_kind, captured } => {
                Some((sam_method.clone(), sam_desc.clone(), impl_class.clone(), impl_method.clone(), impl_desc.clone(), *ref_kind, captured.clone()))
            }
            NativePayload::Lambda(f) => {
                return Ok(f(args));
            }
            _ => None,
        };
        if let Some((sam_method, sam_desc_str, impl_class, impl_method, impl_desc, ref_kind, captured)) = lambda_info {
            let sam_arg_count = count_args(&sam_desc_str);
            let call_arg_count = count_args(descriptor);
            if method_name == sam_method && call_arg_count == sam_arg_count {
                let mut full_args = captured;
                full_args.extend(args);
                let invoked = if ref_kind == 5 || ref_kind == 7 || ref_kind == 9 {
                    if full_args.is_empty() {
                        return Err("Lambda invoke_virtual: missing receiver argument".to_owned());
                    }
                    let recv = full_args.remove(0);
                    match recv {
                        JValue::Ref(Some(r)) => self.invoke_virtual(r, &impl_class, &impl_method, &impl_desc, full_args),
                        _ => Err(format!("Lambda invoke_virtual: expected Ref for this, got {recv:?}")),
                    }
                } else if ref_kind == 8 {
                    // newinvokespecial — constructor reference (e.g. Age::new)
                    self.ensure_class_init(&impl_class)?;
                    let obj = JObject::new(&impl_class);
                    self.invoke_special(obj.clone(), &impl_class, &impl_method, &impl_desc, full_args)?;
                    Ok(JValue::Ref(Some(obj)))
                } else {
                    self.invoke_static(&impl_class, &impl_method, &impl_desc, full_args)
                }?;
                return self.adapt_lambda_return(descriptor, &impl_desc, invoked);
            }
        }

        // Try to build a bytecode frame.
        match self.build_virtual_frame_inner(this.clone(), class_name, method_name, descriptor, args.clone(), true)? {
            Some(fi) => {
                let frame_owner = fi.frame_owner.to_string();
                let mut call_stack = vec![fi];
                self.run_trampoline(&mut call_stack)
                    .map_err(|e| format!("{e}\n  at {frame_owner}"))
            }
            None => {
                // Native fallback.
                let resolve_class = if self.classes.contains_key(&runtime_class) {
                    runtime_class.clone()
                } else {
                    class_name.to_owned()
                };
                let resolved = self
                    .resolve_method_signature(&resolve_class, method_name, descriptor)
                    .map(|(resolved, _)| resolved)
                    .unwrap_or_else(|| descriptor.to_owned());
                let desc = resolved.as_str();
                if let Some(v) = self.native_virtual(&this, &runtime_class, method_name, desc, &args) {
                    if let Some(err) = self.pending_exception_err() { return Err(err); }
                    return Ok(v);
                }
                Err(format!("Virtual method not found: {resolve_class}.{method_name}{desc} (runtime={runtime_class})"))
            }
        }
    }

    pub(in crate::interpreter) fn adapt_lambda_return(
        &mut self,
        sam_descriptor: &str,
        impl_descriptor: &str,
        value: JValue,
    ) -> Result<JValue, String> {
        let Some(sam_ret) = method_return_descriptor(sam_descriptor) else {
            return Ok(value);
        };
        if !is_reference_descriptor(sam_ret) || matches!(value, JValue::Ref(_) | JValue::Void) {
            return Ok(value);
        }
        let Some(impl_ret) = method_return_descriptor(impl_descriptor) else {
            return Ok(value);
        };
        self.box_primitive_for_lambda(impl_ret, value)
    }

    fn box_primitive_for_lambda(&mut self, impl_return_desc: &str, value: JValue) -> Result<JValue, String> {
        match impl_return_desc {
            "Z" => self.invoke_static("java/lang/Boolean", "valueOf", "(Z)Ljava/lang/Boolean;", vec![value]),
            "B" => self.invoke_static("java/lang/Byte", "valueOf", "(B)Ljava/lang/Byte;", vec![value]),
            "S" => self.invoke_static("java/lang/Short", "valueOf", "(S)Ljava/lang/Short;", vec![value]),
            "C" => self.invoke_static("java/lang/Character", "valueOf", "(C)Ljava/lang/Character;", vec![value]),
            "I" => self.invoke_static("java/lang/Integer", "valueOf", "(I)Ljava/lang/Integer;", vec![value]),
            "J" => self.invoke_static("java/lang/Long", "valueOf", "(J)Ljava/lang/Long;", vec![value]),
            "F" => self.invoke_static("java/lang/Float", "valueOf", "(F)Ljava/lang/Float;", vec![value]),
            "D" => self.invoke_static("java/lang/Double", "valueOf", "(D)Ljava/lang/Double;", vec![value]),
            _ => Ok(value),
        }
    }

    // ------------------------------------------------------------------
    // Record method helpers
    // ------------------------------------------------------------------

    /// Convert a JValue to its Java string representation, calling toString() for objects.
    pub(in crate::interpreter) fn jvalue_to_string(&mut self, val: JValue) -> Result<String, String> {
        match val {
            JValue::Void => Ok("null".to_owned()),
            JValue::Int(v) => Ok(v.to_string()),
            JValue::Long(v) => Ok(v.to_string()),
            JValue::Float(v) => Ok(v.to_string()),
            JValue::Double(v) => Ok(v.to_string()),
            JValue::Ref(None) => Ok("null".to_owned()),
            JValue::Ref(Some(r)) => {
                if let Some(s) = r.borrow().as_java_string() {
                    return Ok(s.to_owned());
                }
                let class_name = r.borrow().class_name.clone();
                match self.invoke_virtual(r, &class_name, "toString", "()Ljava/lang/String;", vec![])? {
                    JValue::Ref(Some(sr)) => Ok(sr.borrow().as_java_string().unwrap_or("").to_owned()),
                    _ => Ok("null".to_owned()),
                }
            }
            JValue::ReturnAddress(_) => Ok("?".to_owned()),
        }
    }

    /// Check structural equality of two JValues (for record equals).
    pub(in crate::interpreter) fn jvalue_equals(&mut self, a: &JValue, b: &JValue) -> Result<bool, String> {
        match (a, b) {
            (JValue::Int(x), JValue::Int(y)) => Ok(x == y),
            (JValue::Long(x), JValue::Long(y)) => Ok(x == y),
            (JValue::Float(x), JValue::Float(y)) => Ok(x == y),
            (JValue::Double(x), JValue::Double(y)) => Ok(x == y),
            (JValue::Ref(None), JValue::Ref(None)) => Ok(true),
            (JValue::Ref(Some(ra)), JValue::Ref(Some(rb))) => {
                // Try string equality first.
                let sa = ra.borrow().as_java_string().map(|s| s.to_owned());
                let sb = rb.borrow().as_java_string().map(|s| s.to_owned());
                if let (Some(sa), Some(sb)) = (sa, sb) {
                    return Ok(sa == sb);
                }
                // Fall back to invoking equals().
                let result = self.invoke_virtual(ra.clone(), &ra.borrow().class_name.clone(), "equals", "(Ljava/lang/Object;)Z", vec![JValue::Ref(Some(rb.clone()))])?;
                Ok(matches!(result, JValue::Int(1)))
            }
            _ => Ok(false),
        }
    }

    /// Compute a hash code for a JValue (for record hashCode).
    pub(in crate::interpreter) fn jvalue_hash(&mut self, val: &JValue) -> Result<i32, String> {
        match val {
            JValue::Int(v) => Ok(*v),
            JValue::Long(v) => Ok((v ^ (v >> 32)) as i32),
            JValue::Float(v) => Ok(v.to_bits() as i32),
            JValue::Double(v) => Ok((v.to_bits() ^ (v.to_bits() >> 32)) as i32),
            JValue::Ref(None) => Ok(0),
            JValue::Ref(Some(r)) => {
                if let Some(s) = r.borrow().as_java_string() {
                    let mut h: i32 = 0;
                    for b in s.bytes() {
                        h = h.wrapping_mul(31).wrapping_add(b as i32);
                    }
                    return Ok(h);
                }
                let class_name = r.borrow().class_name.clone();
                match self.invoke_virtual(r.clone(), &class_name, "hashCode", "()I", vec![])? {
                    JValue::Int(h) => Ok(h),
                    _ => Ok(0),
                }
            }
            _ => Ok(0),
        }
    }

    // ------------------------------------------------------------------
    // Top-level entry point with green thread scheduling
    // ------------------------------------------------------------------

    /// Execute a static method as the top-level entry point with green thread support.
    /// The initial method runs on the main thread. If Thread.start() is called during
    /// execution, spawned threads are scheduled cooperatively.
    pub fn invoke_static_threaded(
        &mut self,
        class_name: &str,
        method_name: &str,
        descriptor: &str,
        args: Vec<JValue>,
    ) -> Result<JValue, String> {
        // Reset scheduler to a clean single-main-thread state so that
        // invoke_static_threaded can be called multiple times on the same Vm.
        // Also clear monitor/lock state tied to previous threads to avoid
        // stale owners/waiters referring to dead thread IDs between runs.
        self.scheduler = super::Scheduler::new();
        self.monitors.clear();

        let orig_args = args;
        let (resolved_desc, args) = match self.prepare_static_args(class_name, method_name, descriptor, orig_args.clone()) {
            Some(pair) => pair,
            None => {
                if let Some(v) = self.native_static(class_name, method_name, descriptor, &orig_args) {
                    if let Some(err) = self.pending_exception_err() { return Err(err); }
                    return Ok(v);
                }
                return Err(format!("Method not found: {class_name}.{method_name}{descriptor}"));
            }
        };
        let desc = resolved_desc.as_str();

        match self.build_static_frame(class_name, method_name, desc, args.clone(), true)? {
            Some(fi) => {
                // Push onto main thread's call_stack and run via scheduler.
                self.scheduler.current_thread_mut().call_stack.push(fi);
                self.run_all_threads()
            }
            None => {
                // Native method — no threading needed.
                if let Some(v) = self.native_static(class_name, method_name, desc, &args) {
                    if let Some(err) = self.pending_exception_err() { return Err(err); }
                    return Ok(v);
                }
                Err(format!(
                    "Native stub not found for: {class_name}.{method_name}{desc}"
                ))
            }
        }
    }
}
