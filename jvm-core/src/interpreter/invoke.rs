
use crate::heap::{JObject, JRef, JValue, NativePayload};

use super::Vm;
use super::descriptors::*;
use super::frame::*;

impl Vm {
    /// Execute a static method and return its result.
    pub fn invoke_static(
        &mut self,
        class_name: &str,
        method_name: &str,
        descriptor: &str,
        args: Vec<JValue>,
    ) -> Result<JValue, String> {
        // Resolve descriptor: exact match first, then param-only fallback for generic return types.
        let method_flags = self.find_method_flags(class_name, method_name, descriptor);
        let resolved_descriptor = if method_flags.is_some() {
            descriptor.to_owned()
        } else {
            self.find_method_real_descriptor(class_name, method_name, descriptor)
                .unwrap_or_else(|| descriptor.to_owned())
        };
        let descriptor = resolved_descriptor.as_str();

        // Re-check with the resolved descriptor if it changed, then try native stubs.
        let method_flags = if method_flags.is_none() {
            self.find_method_flags(class_name, method_name, descriptor)
        } else {
            method_flags
        };
        if method_flags.is_none() {
            if let Some(v) = self.native_static(class_name, method_name, descriptor, &args) {
                if let Some(err) = self.pending_exception_err() {
                    return Err(err);
                }
                return Ok(v);
            }
            return Err(format!("Method not found: {class_name}.{method_name}{descriptor}"));
        }

        // If the resolved method is varargs (ACC_VARARGS = 0x0080), synthesize a single
        // empty array argument when the call site omits the trailing varargs parameter.
        let mut args = args;
        let expected_arg_count = count_args(descriptor);
        if args.len() < expected_arg_count {
            let is_varargs = method_flags.map(|f| f & 0x0080 != 0).unwrap_or(false);
            if is_varargs && expected_arg_count - args.len() == 1 {
                args.push(JValue::Ref(Some(JObject::new_array("[Ljava/lang/Object;", vec![]))));
            }
        }

        // Resolve method and extract all execution info in one pass.
        let info = self
            .resolve_method_exec_info(class_name, method_name, descriptor)
            .ok_or_else(|| format!("Method not found: {class_name}.{method_name}{descriptor}"))?;

        // Build initial frame.
        let (param_tokens, _) = Self::parse_method_descriptor_tokens(descriptor);
        let required_slots: usize = param_tokens
            .iter()
            .map(|t| if t == "J" || t == "D" { 2 } else { 1 })
            .sum();
        let mut locals = vec![JValue::Void; info.max_locals.max(required_slots)];
        let mut local_idx = 0usize;
        for (a, t) in args.into_iter().zip(param_tokens.iter()) {
            if local_idx >= locals.len() {
                break;
            }
            locals[local_idx] = self.adapt_value_for_descriptor(t, a);
            local_idx += if t == "J" || t == "D" { 2 } else { 1 };
        }

        // If method has no code (native), fall back to native stubs.
        if !info.has_code {
            let (param_tokens, _) = Self::parse_method_descriptor_tokens(&info.descriptor);
            let mut native_args = Vec::with_capacity(param_tokens.len());
            let mut slot = 0usize;
            for t in &param_tokens {
                if slot < locals.len() {
                    native_args.push(locals[slot].clone());
                }
                slot += if t == "J" || t == "D" { 2 } else { 1 };
            }
            if let Some(v) = self.native_static(&info.class_name, method_name, &info.descriptor, &native_args) {
                if let Some(err) = self.pending_exception_err() {
                    return Err(err);
                }
                return Ok(v);
            }
            return Err(format!("No code (native) in {}.{method_name}{}", info.class_name, info.descriptor));
        }

        let mut frame = Frame {
            locals,
            stack: Vec::new(),
            pc: 0,
        };

        let frame_owner = format!("{}.{method_name}{}", info.class_name, info.descriptor);
        self.run_frame(&mut frame, &info.code, &*info.cp, &frame_owner, &info.bootstrap_methods, &info.exception_table)
            .map_err(|e| format!("{e}\n  at {frame_owner}"))
    }

    /// Execute an instance method with invokespecial semantics.
    /// Resolves from the specified class (not the runtime class).
    pub fn invoke_special(
        &mut self,
        this: JRef,
        class_name: &str,
        method_name: &str,
        descriptor: &str,
        args: Vec<JValue>,
    ) -> Result<JValue, String> {
        // Resolve descriptor: exact match first, then param-only fallback for generic return types.
        let resolved_descriptor = if self.method_exists(class_name, method_name, descriptor) {
            descriptor.to_owned()
        } else {
            self.find_method_real_descriptor(class_name, method_name, descriptor)
                .unwrap_or_else(|| descriptor.to_owned())
        };
        let descriptor = resolved_descriptor.as_str();

        // Resolve from the specified class, not the runtime class.
        if !self.method_exists(class_name, method_name, descriptor) {
            if let Some(v) = self.native_virtual(&this, class_name, method_name, descriptor, &args) {
                if let Some(err) = self.pending_exception_err() {
                    return Err(err);
                }
                return Ok(v);
            }
            return Err(format!("Special method not found: {class_name}.{method_name}{descriptor}"));
        }

        // Resolve method and extract all execution info in one pass.
        let info = self
            .resolve_method_exec_info(class_name, method_name, descriptor)
            .unwrap();

        // JVMS §5.4.3.3: invoking an abstract method throws AbstractMethodError.
        if let Some(flags) = self.find_method_flags(&info.class_name, method_name, &info.descriptor) {
            if flags & 0x0400 != 0 {
                let exc = crate::heap::JObject::new("java/lang/AbstractMethodError");
                let msg_str = format!("{}.{method_name}{}", info.class_name, info.descriptor);
                let msg = self.intern_string(msg_str);
                exc.borrow_mut().fields.insert("detailMessage".to_owned(), crate::heap::JValue::Ref(Some(msg)));
                self.pending_exception = Some(exc);
                return Err(format!("AbstractMethodError: {}.{method_name}{}", info.class_name, info.descriptor));
            }
        }

        let (param_tokens, _) = Self::parse_method_descriptor_tokens(descriptor);
        let required_slots = 1 + param_tokens
            .iter()
            .map(|t| if t == "J" || t == "D" { 2 } else { 1 })
            .sum::<usize>();
        let total = info.max_locals.max(required_slots);
        let mut locals = vec![JValue::Void; total];
        locals[0] = JValue::Ref(Some(this.clone()));
        let mut local_idx = 1usize;
        for (a, t) in args.into_iter().zip(param_tokens.iter()) {
            if local_idx >= locals.len() {
                break;
            }
            locals[local_idx] = self.adapt_value_for_descriptor(t, a);
            local_idx += if t == "J" || t == "D" { 2 } else { 1 };
        }

        // If method has no code (native), fall back to native stubs.
        if !info.has_code {
            let virt_args: Vec<JValue> = locals[1..].iter()
                .filter(|v| !matches!(v, JValue::Void))
                .cloned()
                .collect();
            if let Some(v) = self.native_virtual(&this, &info.class_name, method_name, &info.descriptor, &virt_args) {
                if let Some(err) = self.pending_exception_err() {
                    return Err(err);
                }
                return Ok(v);
            }
            return Err(format!("No code (native) in {}.{method_name}{}", info.class_name, info.descriptor));
        }

        let mut frame = Frame {
            locals,
            stack: Vec::new(),
            pc: 0,
        };

        let frame_owner = format!("{}.{method_name}{}", info.class_name, info.descriptor);
        self.run_frame(&mut frame, &info.code, &*info.cp, &frame_owner, &info.bootstrap_methods, &info.exception_table)
            .map_err(|e| format!("{e}\n  at {frame_owner}"))
    }

    /// Execute an instance method (first local = `this` reference).
    pub fn invoke_virtual(
        &mut self,
        this: JRef,
        class_name: &str,
        method_name: &str,
        descriptor: &str,
        args: Vec<JValue>,
    ) -> Result<JValue, String> {
        // Use the actual runtime class of `this` for virtual dispatch.
        let runtime_class = this.borrow().class_name.clone();

        // If receiver carries lambda payload, try direct SAM dispatch.
        let lambda_info = match &this.borrow().native {
            NativePayload::BytecodeLambda { sam_method, sam_desc, impl_class, impl_method, impl_desc, ref_kind, captured } => {
                Some((
                    sam_method.clone(),
                    sam_desc.clone(),
                    impl_class.clone(),
                    impl_method.clone(),
                    impl_desc.clone(),
                    *ref_kind,
                    captured.clone(),
                ))
            }
            NativePayload::Lambda(f) => {
                let result = f(args);
                return Ok(result);
            }
            _ => None,
        };
        if let Some((sam_method, sam_desc_str, impl_class, impl_method, impl_desc, ref_kind, captured)) = lambda_info {
            let sam_arg_count = count_args(&sam_desc_str);
            let call_arg_count = count_args(descriptor);
            if method_name == sam_method && call_arg_count == sam_arg_count {
                let mut full_args = captured;
                full_args.extend(args);
                // ref_kind 5 = invokeVirtual, 7 = invokeSpecial, 9 = invokeInterface
                // ref_kind 6 = invokeStatic
                let invoked = if ref_kind == 5 || ref_kind == 7 || ref_kind == 9 {
                    if full_args.is_empty() {
                        return Err("Lambda invoke_virtual: missing receiver argument".to_owned());
                    }
                    let recv = full_args.remove(0);
                    match recv {
                        JValue::Ref(Some(r)) => self.invoke_virtual(r, &impl_class, &impl_method, &impl_desc, full_args),
                        _ => Err(format!(
                            "Lambda invoke_virtual: expected Ref for this, got {recv:?}"
                        )),
                    }
                } else {
                    self.invoke_static(&impl_class, &impl_method, &impl_desc, full_args)
                }?;
                return self.adapt_lambda_return(descriptor, &impl_desc, invoked);
            }
        }

        // Resolve method starting from the runtime class.
        let resolve_class = if self.classes.contains_key(&runtime_class) {
            runtime_class.clone()
        } else {
            class_name.to_owned()
        };

        // Resolve descriptor: exact match first, then param-only fallback for generic return types.
        let resolved_descriptor = if self.method_exists(&resolve_class, method_name, descriptor) {
            descriptor.to_owned()
        } else {
            self.find_method_real_descriptor(&resolve_class, method_name, descriptor)
                .unwrap_or_else(|| descriptor.to_owned())
        };
        let descriptor = resolved_descriptor.as_str();

        if !self.method_exists(&resolve_class, method_name, descriptor) {
            // Method not in bytecode — try native stubs.
            if let Some(v) = self.native_virtual(&this, &runtime_class, method_name, descriptor, &args) {
                if let Some(err) = self.pending_exception_err() {
                    return Err(err);
                }
                return Ok(v);
            }
            return Err(format!(
                "Virtual method not found: {resolve_class}.{method_name}{descriptor} (runtime={runtime_class})"
            ));
        }

        // Resolve method and extract all execution info in one pass.
        let info = self
            .resolve_method_exec_info(&resolve_class, method_name, descriptor)
            .unwrap();

        // JVMS §5.4.3.3: invoking an abstract method throws AbstractMethodError.
        if let Some(flags) = self.find_method_flags(&info.class_name, method_name, &info.descriptor) {
            if flags & 0x0400 != 0 {
                let exc = crate::heap::JObject::new("java/lang/AbstractMethodError");
                let msg_str = format!("{}.{method_name}{}", info.class_name, info.descriptor);
                let msg = self.intern_string(msg_str);
                exc.borrow_mut().fields.insert("detailMessage".to_owned(), crate::heap::JValue::Ref(Some(msg)));
                self.pending_exception = Some(exc);
                return Err(format!("AbstractMethodError: {}.{method_name}{}", info.class_name, info.descriptor));
            }
        }

        // `this` goes into local[0], then arguments.
        let (param_tokens, _) = Self::parse_method_descriptor_tokens(descriptor);
        let required_slots = 1 + param_tokens
            .iter()
            .map(|t| if t == "J" || t == "D" { 2 } else { 1 })
            .sum::<usize>();
        let total = info.max_locals.max(required_slots);
        let mut locals = vec![JValue::Void; total];
        locals[0] = JValue::Ref(Some(this));
        let mut local_idx = 1usize;
        for (a, t) in args.into_iter().zip(param_tokens.iter()) {
            if local_idx >= locals.len() {
                break;
            }
            locals[local_idx] = self.adapt_value_for_descriptor(t, a);
            local_idx += if t == "J" || t == "D" { 2 } else { 1 };
        }

        // If method has no code (native), fall back to native stubs.
        if !info.has_code {
            let this_ref = match &locals[0] {
                JValue::Ref(Some(r)) => r.clone(),
                _ => return Err(format!("No code (native) in {}.{method_name}{}", info.class_name, info.descriptor)),
            };
            let virt_args: Vec<JValue> = locals[1..].iter()
                .filter(|v| !matches!(v, JValue::Void))
                .cloned()
                .collect();
            if let Some(v) = self.native_virtual(&this_ref, &info.class_name, method_name, &info.descriptor, &virt_args) {
                if let Some(err) = self.pending_exception_err() {
                    return Err(err);
                }
                return Ok(v);
            }
            return Err(format!("No code (native) in {}.{method_name}{}", info.class_name, info.descriptor));
        }

        let mut frame = Frame {
            locals,
            stack: Vec::new(),
            pc: 0,
        };

        let frame_owner = format!("{}.{method_name}{}", info.class_name, info.descriptor);
        self.run_frame(&mut frame, &info.code, &*info.cp, &frame_owner, &info.bootstrap_methods, &info.exception_table)
            .map_err(|e| format!("{e}\n  at {frame_owner}"))
    }

    fn adapt_lambda_return(
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
    // Core interpreter loop
    // ------------------------------------------------------------------

}
