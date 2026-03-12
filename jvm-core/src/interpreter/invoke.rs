
use crate::heap::{JRef, JValue, NativePayload};

use super::Vm;
use super::descriptors::*;

impl Vm {
    /// Execute a static method and return its result.
    /// Uses the trampoline for bytecode methods, falls back to native for stubs.
    pub fn invoke_static(
        &mut self,
        class_name: &str,
        method_name: &str,
        descriptor: &str,
        args: Vec<JValue>,
    ) -> Result<JValue, String> {
        match self.build_static_frame(class_name, method_name, descriptor, args.clone(), true)? {
            Some(fi) => {
                let frame_owner = fi.class_name.clone();
                let mut call_stack = vec![fi];
                self.run_trampoline(&mut call_stack)
                    .map_err(|e| format!("{e}\n  at {frame_owner}"))
            }
            None => {
                // Native fallback — use the old inline path.
                self.invoke_static_native(class_name, method_name, descriptor, args)
            }
        }
    }

    /// Native-only path for invoke_static (when build_static_frame returns None).
    fn invoke_static_native(
        &mut self,
        class_name: &str,
        method_name: &str,
        descriptor: &str,
        args: Vec<JValue>,
    ) -> Result<JValue, String> {
        let method_flags = self.find_method_flags(class_name, method_name, descriptor);
        let resolved_descriptor = if method_flags.is_some() {
            descriptor.to_owned()
        } else {
            self.find_method_real_descriptor(class_name, method_name, descriptor)
                .unwrap_or_else(|| descriptor.to_owned())
        };
        let descriptor = resolved_descriptor.as_str();

        if let Some(v) = self.native_static(class_name, method_name, descriptor, &args) {
            if let Some(err) = self.pending_exception_err() { return Err(err); }
            return Ok(v);
        }

        // Method has code but build_static_frame returned None — this means
        // method_flags was None (descriptor mismatch). Try resolving method info
        // for the native path with full locals construction.
        let info = match self.resolve_method_exec_info(class_name, method_name, descriptor) {
            Some(info) => info,
            None => return Err(format!("Method not found: {class_name}.{method_name}{descriptor}")),
        };
        if !info.has_code {
            let (param_tokens, _) = Self::parse_method_descriptor_tokens(&info.descriptor);
            let mut native_args = Vec::with_capacity(param_tokens.len());
            // Build locals to extract native args correctly.
            let req: usize = param_tokens.iter().map(|t| if t == "J" || t == "D" { 2 } else { 1 }).sum();
            let mut locals = vec![JValue::Void; info.max_locals.max(req)];
            let mut li = 0usize;
            for (a, t) in args.into_iter().zip(param_tokens.iter()) {
                if li >= locals.len() { break; }
                locals[li] = self.adapt_value_for_descriptor(t, a);
                li += if t == "J" || t == "D" { 2 } else { 1 };
            }
            let mut slot = 0usize;
            for t in &param_tokens {
                if slot < locals.len() { native_args.push(locals[slot].clone()); }
                slot += if t == "J" || t == "D" { 2 } else { 1 };
            }
            if let Some(v) = self.native_static(&info.class_name, method_name, &info.descriptor, &native_args) {
                if let Some(err) = self.pending_exception_err() { return Err(err); }
                return Ok(v);
            }
        }
        Err(format!("Method not found: {class_name}.{method_name}{descriptor}"))
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
                let frame_owner = fi.class_name.clone();
                let mut call_stack = vec![fi];
                self.run_trampoline(&mut call_stack)
                    .map_err(|e| format!("{e}\n  at {frame_owner}"))
            }
            None => {
                // Native fallback.
                let resolved = if self.method_exists(class_name, method_name, descriptor) {
                    descriptor.to_owned()
                } else {
                    self.find_method_real_descriptor(class_name, method_name, descriptor)
                        .unwrap_or_else(|| descriptor.to_owned())
                };
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
                } else {
                    self.invoke_static(&impl_class, &impl_method, &impl_desc, full_args)
                }?;
                return self.adapt_lambda_return(descriptor, &impl_desc, invoked);
            }
        }

        // Try to build a bytecode frame.
        match self.build_virtual_frame_inner(this.clone(), class_name, method_name, descriptor, args.clone(), true)? {
            Some(fi) => {
                let frame_owner = fi.class_name.clone();
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
                let resolved = if self.method_exists(&resolve_class, method_name, descriptor) {
                    descriptor.to_owned()
                } else {
                    self.find_method_real_descriptor(&resolve_class, method_name, descriptor)
                        .unwrap_or_else(|| descriptor.to_owned())
                };
                let desc = resolved.as_str();
                if let Some(v) = self.native_virtual(&this, &runtime_class, method_name, desc, &args) {
                    if let Some(err) = self.pending_exception_err() { return Err(err); }
                    return Ok(v);
                }
                Err(format!("Virtual method not found: {resolve_class}.{method_name}{desc} (runtime={runtime_class})"))
            }
        }
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
