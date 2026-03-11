//! Java bytecode interpreter.
//!
//! Implements a stack-based interpreter over the JVM instruction set.
//! The focus is on the subset needed to run Raoh:
//! - Core stack / local-variable operations
//! - Object creation and field access
//! - Method invocation (all four flavours + `invokedynamic`)
//! - Integer / long / reference comparisons and control flow
//! - Native stubs for `java.lang.*` and `java.util.*`

use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

#[cfg(target_arch = "wasm32")]
use wasm_bindgen::prelude::*;

use crate::class_file::{
    Attribute, BootstrapMethod, ClassFile, ConstantPoolEntry, ExceptionTableEntry, MethodInfo,
};
use crate::heap::{JObject, JRef, JValue, NativePayload};

#[cfg(target_arch = "wasm32")]
#[wasm_bindgen]
extern "C" {
    #[wasm_bindgen(js_namespace = console, js_name = log)]
    fn console_log(s: &str);
    #[wasm_bindgen(js_namespace = console, js_name = error)]
    fn console_error(s: &str);
}

// ---------------------------------------------------------------------------
// VM state
// ---------------------------------------------------------------------------

/// The central virtual machine that holds loaded classes and drives execution.
pub struct Vm {
    /// Loaded class files keyed by internal name (`net/unit8/raoh/Result`).
    classes: HashMap<String, ClassFile>,
    /// Interned strings cache (not strictly required but saves allocations).
    string_pool: HashMap<String, JRef>,
    /// Static field storage keyed by "ClassName.fieldName".
    static_fields: HashMap<String, JValue>,
    /// Canonical Class objects keyed by internal class name or descriptor.
    class_pool: HashMap<String, JRef>,
    /// Pending exception object — set by athrow, consumed by exception handler.
    /// This preserves the full exception object (with message, cause, fields)
    /// across the Err(String) propagation path.
    pending_exception: Option<JRef>,
    /// Buffered `System.out.print` content until newline/println.
    stdout_buffer: String,
    /// Buffered `System.err.print` content until newline/println.
    stderr_buffer: String,
}

impl Vm {
    /// Create an empty VM.
    pub fn new() -> Self {
        Vm {
            classes: HashMap::new(),
            string_pool: HashMap::new(),
            static_fields: HashMap::new(),
            class_pool: HashMap::new(),
            pending_exception: None,
            stdout_buffer: String::new(),
            stderr_buffer: String::new(),
        }
    }

    /// Register a pre-parsed class file.
    pub fn load_class(&mut self, class_file: ClassFile) {
        let name = class_file.constant_pool.class_name(class_file.this_class).to_owned();
        self.classes.insert(name, class_file);
    }

    /// Flush buffered PrintStream output (`print` without trailing `println`).
    pub fn flush_printstreams(&mut self) {
        if !self.stdout_buffer.is_empty() {
            Self::emit_host_line(false, &self.stdout_buffer);
            self.stdout_buffer.clear();
        }
        if !self.stderr_buffer.is_empty() {
            Self::emit_host_line(true, &self.stderr_buffer);
            self.stderr_buffer.clear();
        }
    }

    /// Intern a Java string (returns same `JRef` for equal content).
    pub fn intern_string(&mut self, s: impl Into<String>) -> JRef {
        let s = s.into();
        if let Some(r) = self.string_pool.get(&s) {
            return Rc::clone(r);
        }
        let r = JObject::new_string(s.clone());
        self.string_pool.insert(s, Rc::clone(&r));
        r
    }

    fn pending_exception_err(&self) -> Option<String> {
        self.pending_exception.as_ref().map(|r| {
            let b = r.borrow();
            let mut s = format!("Exception: {}", b.class_name);
            if let Some(JValue::Ref(Some(msg_ref))) = b.fields.get("detailMessage") {
                if let Some(msg) = msg_ref.borrow().as_java_string() {
                    if !msg.is_empty() {
                        s.push_str(": ");
                        s.push_str(msg);
                    }
                }
            }
            s
        })
    }

    fn class_object(&mut self, internal_name: impl Into<String>) -> JRef {
        let internal_name = internal_name.into();
        if let Some(r) = self.class_pool.get(&internal_name) {
            return Rc::clone(r);
        }
        let obj = JObject::new("java/lang/Class");
        obj.borrow_mut().fields.insert(
            "__name_internal".to_owned(),
            JValue::Ref(Some(self.intern_string(internal_name.clone()))),
        );
        self.class_pool.insert(internal_name, Rc::clone(&obj));
        obj
    }

    /// Look up a loaded class by internal name.
    pub fn class(&self, name: &str) -> Option<&ClassFile> {
        self.classes.get(name)
    }

    /// Find a method by name and descriptor in a class (including super-chain).
    pub fn find_method<'a>(
        &'a self,
        class_name: &str,
        method_name: &str,
        descriptor: &str,
    ) -> Option<(&'a ClassFile, &'a MethodInfo)> {
        let class = self.classes.get(class_name)?;
        for m in &class.methods {
            let n = class.constant_pool.utf8(m.name_index);
            let d = class.constant_pool.utf8(m.descriptor_index);
            if n == method_name && d == descriptor {
                return Some((class, m));
            }
        }
        // Walk super class.
        if class.super_class != 0 {
            let super_name = class.constant_pool.class_name(class.super_class).to_owned();
            if let Some(result) = self.find_method(&super_name, method_name, descriptor) {
                return Some(result);
            }
        }
        // Walk interfaces (for default methods).
        let iface_names: Vec<String> = class.interfaces.iter()
            .map(|&idx| class.constant_pool.class_name(idx).to_owned())
            .collect();
        for iface_name in iface_names {
            if let Some(result) = self.find_method(&iface_name, method_name, descriptor) {
                return Some(result);
            }
        }
        None
    }

    /// Like find_method but with relaxed matching when the compiler emits generic types.
    /// Match priority:
    ///   1. Exact param types match (ignoring return type)
    ///   2. Same argument count match (ignoring both param types and return type)
    ///   3. Varargs method (ACC_VARARGS) whose non-varargs param count <= call arg count
    /// Returns the real descriptor string of the matched method.
    fn find_method_real_descriptor(
        &self,
        class_name: &str,
        method_name: &str,
        descriptor: &str,
    ) -> Option<String> {
        let param_part = descriptor.split(')').next().unwrap_or("(");
        let arg_count = count_args(descriptor);
        let class = self.classes.get(class_name)?;
        let mut arg_count_match: Option<String> = None;
        let mut varargs_match: Option<String> = None;
        for m in &class.methods {
            let n = class.constant_pool.utf8(m.name_index);
            let d = class.constant_pool.utf8(m.descriptor_index);
            if n != method_name { continue; }
            let d_param = d.split(')').next().unwrap_or("(");
            // Priority 1: exact param match
            if d_param == param_part {
                return Some(d.to_owned());
            }
            // Priority 2: same arg count
            if arg_count_match.is_none() && count_args(d) == arg_count {
                arg_count_match = Some(d.to_owned());
            }
            // Priority 3: varargs method (ACC_VARARGS = 0x0080)
            // A varargs call with 0 extra args passes an empty array as last param,
            // so arg count from call site may be less than the method's param count.
            if varargs_match.is_none() && (m.access_flags & 0x0080 != 0) {
                let method_param_count = count_args(d);
                // varargs: fixed params = method_param_count - 1, array counts as 1
                let fixed = method_param_count.saturating_sub(1);
                if arg_count >= fixed {
                    varargs_match = Some(d.to_owned());
                }
            }
        }
        if arg_count_match.is_some() { return arg_count_match; }
        if varargs_match.is_some() { return varargs_match; }
        // Walk super class.
        if class.super_class != 0 {
            let super_name = class.constant_pool.class_name(class.super_class).to_owned();
            if let Some(result) = self.find_method_real_descriptor(&super_name, method_name, descriptor) {
                return Some(result);
            }
        }
        // Walk interfaces.
        let iface_names: Vec<String> = class.interfaces.iter()
            .map(|&idx| class.constant_pool.class_name(idx).to_owned())
            .collect();
        for iface_name in iface_names {
            if let Some(result) = self.find_method_real_descriptor(&iface_name, method_name, descriptor) {
                return Some(result);
            }
        }
        None
    }

    /// Execute a static method and return its result.
    pub fn invoke_static(
        &mut self,
        class_name: &str,
        method_name: &str,
        descriptor: &str,
        args: Vec<JValue>,
    ) -> Result<JValue, String> {
        // Try bytecode first if the class is loaded; fall back to native stubs.
        let found = self.find_method(class_name, method_name, descriptor).is_some();
        if !found {
            if let Some(v) = self.native_static(class_name, method_name, descriptor, &args) {
                if let Some(err) = self.pending_exception_err() {
                    return Err(err);
                }
                return Ok(v);
            }
        }

        // Resolve the actual descriptor: exact match first, then param-only fallback.
        // The compiler may emit a generic return type (e.g. Ljava/lang/Object;) for
        // wildcard-imported methods whose real return type is more specific.
        let resolved_descriptor = if self.find_method(class_name, method_name, descriptor).is_some() {
            descriptor.to_owned()
        } else {
            self.find_method_real_descriptor(class_name, method_name, descriptor)
                .unwrap_or_else(|| descriptor.to_owned())
        };
        let descriptor = resolved_descriptor.as_str();

        // If the resolved method is varargs (ACC_VARARGS = 0x0080), pad args with an
        // empty array when the call site passes fewer arguments than the method expects.
        let mut args = args;
        let expected_arg_count = count_args(descriptor);
        if args.len() < expected_arg_count {
            let is_varargs = self.find_method(class_name, method_name, descriptor)
                .map(|(_, m)| m.access_flags & 0x0080 != 0)
                .unwrap_or(false);
            if is_varargs {
                // Push empty Object[] for the missing varargs parameter
                while args.len() < expected_arg_count {
                    args.push(JValue::Ref(Some(JObject::new_array("[Ljava/lang/Object;", vec![]))));
                }
            }
        }

        // Look up class/method.
        let (class_name_owned, descriptor_owned) = {
            let (class, method) = self
                .find_method(class_name, method_name, descriptor)
                .ok_or_else(|| format!("Method not found: {class_name}.{method_name}{descriptor}"))?;
            let cn = class.constant_pool.class_name(class.this_class).to_owned();
            let desc = class.constant_pool.utf8(method.descriptor_index).to_owned();
            (cn, desc)
        };

        // Build initial frame.
        let max_locals = {
            let (_class, method) = self.find_method(&class_name_owned, method_name, &descriptor_owned).unwrap();
            method.code().map(|c| c.max_locals as usize).unwrap_or(0)
        };
        let (param_tokens, _) = Self::parse_method_descriptor_tokens(descriptor);
        let required_slots: usize = param_tokens
            .iter()
            .map(|t| if t == "J" || t == "D" { 2 } else { 1 })
            .sum();
        let mut locals = vec![JValue::Void; max_locals.max(required_slots)];
        let mut local_idx = 0usize;
        for (a, t) in args.into_iter().zip(param_tokens.iter()) {
            if local_idx >= locals.len() {
                break;
            }
            locals[local_idx] = a;
            local_idx += if t == "J" || t == "D" { 2 } else { 1 };
        }

        // If method has no code (native), fall back to native stubs.
        let has_code = {
            let (_, method) = self.find_method(&class_name_owned, method_name, &descriptor_owned).unwrap();
            method.code().is_some()
        };
        if !has_code {
            let (param_tokens, _) = Self::parse_method_descriptor_tokens(&descriptor_owned);
            let mut native_args = Vec::with_capacity(param_tokens.len());
            let mut slot = 0usize;
            for t in &param_tokens {
                if slot < locals.len() {
                    native_args.push(locals[slot].clone());
                }
                slot += if t == "J" || t == "D" { 2 } else { 1 };
            }
            if let Some(v) = self.native_static(&class_name_owned, method_name, &descriptor_owned, &native_args) {
                if let Some(err) = self.pending_exception_err() {
                    return Err(err);
                }
                return Ok(v);
            }
            return Err(format!("No code (native) in {class_name_owned}.{method_name}{descriptor_owned}"));
        }

        let (code, exception_table) = {
            let (_, method) = self.find_method(&class_name_owned, method_name, &descriptor_owned).unwrap();
            let ca = method.code().unwrap();
            (ca.code.clone(), ca.exception_table.clone())
        };
        let cp_entries: Vec<ConstantPoolEntry> = {
            let class = self.classes.get(&class_name_owned).unwrap();
            class.constant_pool.entries.clone()
        };
        let bootstrap_methods: Vec<BootstrapMethod> = {
            let class = self.classes.get(&class_name_owned).unwrap();
            class.attributes.iter().find_map(|a| {
                if let Attribute::BootstrapMethods(bms) = a {
                    Some(bms.clone())
                } else {
                    None
                }
            }).unwrap_or_default()
        };

        let mut frame = Frame {
            locals,
            stack: Vec::new(),
            pc: 0,
        };

        let frame_owner = format!("{class_name_owned}.{method_name}{descriptor_owned}");
        self.run_frame(&mut frame, &code, &cp_entries, &frame_owner, &bootstrap_methods, &exception_table)
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
        let resolved_descriptor = if self.find_method(class_name, method_name, descriptor).is_some() {
            descriptor.to_owned()
        } else {
            self.find_method_real_descriptor(class_name, method_name, descriptor)
                .unwrap_or_else(|| descriptor.to_owned())
        };
        let descriptor = resolved_descriptor.as_str();

        // Resolve from the specified class, not the runtime class.
        let found = self.find_method(class_name, method_name, descriptor).is_some();
        if !found {
            if let Some(v) = self.native_virtual(&this, class_name, method_name, descriptor, &args) {
                return Ok(v);
            }
            return Err(format!("Special method not found: {class_name}.{method_name}{descriptor}"));
        }

        let (class_name_owned, descriptor_owned) = {
            let (class, method) = self.find_method(class_name, method_name, descriptor).unwrap();
            let cn = class.constant_pool.class_name(class.this_class).to_owned();
            let desc = class.constant_pool.utf8(method.descriptor_index).to_owned();
            (cn, desc)
        };

        let max_locals = {
            let (_, method) = self.find_method(&class_name_owned, method_name, &descriptor_owned).unwrap();
            method.code().map(|c| c.max_locals as usize).unwrap_or(0)
        };

        let (param_tokens, _) = Self::parse_method_descriptor_tokens(descriptor);
        let required_slots = 1 + param_tokens
            .iter()
            .map(|t| if t == "J" || t == "D" { 2 } else { 1 })
            .sum::<usize>();
        let total = max_locals.max(required_slots);
        let mut locals = vec![JValue::Void; total];
        locals[0] = JValue::Ref(Some(this.clone()));
        let mut local_idx = 1usize;
        for (a, t) in args.into_iter().zip(param_tokens.iter()) {
            if local_idx >= locals.len() {
                break;
            }
            locals[local_idx] = a;
            local_idx += if t == "J" || t == "D" { 2 } else { 1 };
        }

        // If method has no code (native), fall back to native stubs.
        let has_code = {
            let (_, method) = self.find_method(&class_name_owned, method_name, &descriptor_owned).unwrap();
            method.code().is_some()
        };
        if !has_code {
            let virt_args: Vec<JValue> = locals[1..].iter()
                .filter(|v| !matches!(v, JValue::Void))
                .cloned()
                .collect();
            if let Some(v) = self.native_virtual(&this, &class_name_owned, method_name, &descriptor_owned, &virt_args) {
                return Ok(v);
            }
            return Err(format!("No code (native) in {class_name_owned}.{method_name}{descriptor_owned}"));
        }

        let (code, exception_table) = {
            let (_, method) = self.find_method(&class_name_owned, method_name, &descriptor_owned).unwrap();
            let ca = method.code().unwrap();
            (ca.code.clone(), ca.exception_table.clone())
        };
        let cp_entries: Vec<ConstantPoolEntry> = {
            let class = self.classes.get(&class_name_owned).unwrap();
            class.constant_pool.entries.clone()
        };
        let bootstrap_methods: Vec<BootstrapMethod> = {
            let class = self.classes.get(&class_name_owned).unwrap();
            class.attributes.iter().find_map(|a| {
                if let Attribute::BootstrapMethods(bms) = a {
                    Some(bms.clone())
                } else {
                    None
                }
            }).unwrap_or_default()
        };

        let mut frame = Frame {
            locals,
            stack: Vec::new(),
            pc: 0,
        };

        let frame_owner = format!("{class_name_owned}.{method_name}{descriptor_owned}");
        self.run_frame(&mut frame, &code, &cp_entries, &frame_owner, &bootstrap_methods, &exception_table)
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

        // If this is a BytecodeLambda, invoke its implementation method directly.
        if runtime_class == "$$Lambda" {
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
            if let Some((sam_method, sam_desc, impl_class, impl_method, impl_desc, ref_kind, captured)) = lambda_info {
                // A lambda object should intercept only its SAM call.
                // Default methods on the interface (e.g. Decoder.decode(Object))
                // must be dispatched normally via the interface bytecode.
                if method_name != sam_method || descriptor != sam_desc {
                    // Fall through to regular method resolution below.
                } else {
                let mut full_args = captured;
                full_args.extend(args);
                // ref_kind 5 = invokeVirtual, 7 = invokeSpecial, 9 = invokeInterface
                // ref_kind 6 = invokeStatic
                let invoked = if ref_kind == 5 || ref_kind == 7 || ref_kind == 9 {
                    // First captured arg is `this` for instance methods.
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
        }

        // Resolve method starting from the runtime class.
        let resolve_class = if self.classes.contains_key(&runtime_class) {
            runtime_class.clone()
        } else {
            class_name.to_owned()
        };

        // Resolve descriptor: exact match first, then param-only fallback for generic return types.
        let resolved_descriptor = if self.find_method(&resolve_class, method_name, descriptor).is_some() {
            descriptor.to_owned()
        } else {
            self.find_method_real_descriptor(&resolve_class, method_name, descriptor)
                .unwrap_or_else(|| descriptor.to_owned())
        };
        let descriptor = resolved_descriptor.as_str();

        let found = self.find_method(&resolve_class, method_name, descriptor).is_some();
        if !found {
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

        let (class_name_owned, descriptor_owned) = {
            let (class, method) = self
                .find_method(&resolve_class, method_name, descriptor)
                .unwrap();
            let cn = class.constant_pool.class_name(class.this_class).to_owned();
            let desc = class.constant_pool.utf8(method.descriptor_index).to_owned();
            (cn, desc)
        };

        let max_locals = {
            let (_, method) = self.find_method(&class_name_owned, method_name, &descriptor_owned).unwrap();
            method.code().map(|c| c.max_locals as usize).unwrap_or(0)
        };

        // `this` goes into local[0], then arguments.
        let (param_tokens, _) = Self::parse_method_descriptor_tokens(descriptor);
        let required_slots = 1 + param_tokens
            .iter()
            .map(|t| if t == "J" || t == "D" { 2 } else { 1 })
            .sum::<usize>();
        let total = max_locals.max(required_slots);
        let mut locals = vec![JValue::Void; total];
        locals[0] = JValue::Ref(Some(this));
        let mut local_idx = 1usize;
        for (a, t) in args.into_iter().zip(param_tokens.iter()) {
            if local_idx >= locals.len() {
                break;
            }
            locals[local_idx] = a;
            local_idx += if t == "J" || t == "D" { 2 } else { 1 };
        }

        // If method has no code (native), fall back to native stubs.
        let has_code = {
            let (_, method) = self.find_method(&class_name_owned, method_name, &descriptor_owned).unwrap();
            method.code().is_some()
        };
        if !has_code {
            // Extract `this` back from locals[0].
            let this_ref = match &locals[0] {
                JValue::Ref(Some(r)) => r.clone(),
                _ => return Err(format!("No code (native) in {class_name_owned}.{method_name}{descriptor_owned}")),
            };
            let virt_args: Vec<JValue> = locals[1..].iter()
                .filter(|v| !matches!(v, JValue::Void))
                .cloned()
                .collect();
            if let Some(v) = self.native_virtual(&this_ref, &class_name_owned, method_name, &descriptor_owned, &virt_args) {
                if let Some(err) = self.pending_exception_err() {
                    return Err(err);
                }
                return Ok(v);
            }
            return Err(format!("No code (native) in {class_name_owned}.{method_name}{descriptor_owned}"));
        }

        let (code, exception_table) = {
            let (_, method) = self.find_method(&class_name_owned, method_name, &descriptor_owned).unwrap();
            let ca = method.code().unwrap();
            (ca.code.clone(), ca.exception_table.clone())
        };
        let cp_entries: Vec<ConstantPoolEntry> = {
            let class = self.classes.get(&class_name_owned).unwrap();
            class.constant_pool.entries.clone()
        };
        let bootstrap_methods: Vec<BootstrapMethod> = {
            let class = self.classes.get(&class_name_owned).unwrap();
            class.attributes.iter().find_map(|a| {
                if let Attribute::BootstrapMethods(bms) = a {
                    Some(bms.clone())
                } else {
                    None
                }
            }).unwrap_or_default()
        };

        let mut frame = Frame {
            locals,
            stack: Vec::new(),
            pc: 0,
        };

        let frame_owner = format!("{class_name_owned}.{method_name}{descriptor_owned}");
        self.run_frame(&mut frame, &code, &cp_entries, &frame_owner, &bootstrap_methods, &exception_table)
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

    fn run_frame(
        &mut self,
        frame: &mut Frame,
        code: &[u8],
        cp: &[ConstantPoolEntry],
        class_name: &str,
        bootstrap_methods: &[BootstrapMethod],
        exception_table: &[ExceptionTableEntry],
    ) -> Result<JValue, String> {
        loop {
            if frame.pc >= code.len() {
                return Err(format!("Execution fell off end of method in {class_name}"));
            }
            let opcode_pc = frame.pc; // PC of the current instruction (for exception table lookup)
            let opcode = code[frame.pc];
            frame.pc += 1;

            // Wrap opcode execution to catch exceptions and search exception_table.
            let result = self.execute_opcode(frame, code, cp, class_name, bootstrap_methods, exception_table, opcode);
            match result {
                Ok(Some(ret)) => return Ok(ret),  // method returned a value
                Ok(None) => continue,              // opcode executed, continue loop
                Err(err_msg) => {
                    // Try to find an exception handler in the exception_table.
                    if let Some((handler_pc, exc_obj)) = self.find_exception_handler(
                        frame, exception_table, cp, opcode_pc, &err_msg,
                    ) {
                        frame.stack.clear();
                        frame.stack.push(exc_obj);
                        frame.pc = handler_pc;
                        continue;
                    }
                    return Err(err_msg);
                }
            }
        }
    }

    /// Search exception_table for a matching handler.
    /// Returns (handler_pc, exception_object) if found.
    fn find_exception_handler(
        &mut self,
        _frame: &Frame,
        exception_table: &[ExceptionTableEntry],
        cp: &[ConstantPoolEntry],
        throw_pc: usize,
        err_msg: &str,
    ) -> Option<(usize, JValue)> {
        // Extract exception class name from error message if it matches our format.
        let exc_class = if let Some(rest) = err_msg.strip_prefix("Exception: ") {
            rest.split(':').next().unwrap_or(rest).trim()
        } else if err_msg.starts_with("NullPointerException") {
            "java/lang/NullPointerException"
        } else if err_msg.starts_with("ClassCastException") {
            "java/lang/ClassCastException"
        } else if err_msg.contains("IndexOutOfBoundsException") {
            "java/lang/IndexOutOfBoundsException"
        } else if err_msg.contains("ArithmeticException") {
            "java/lang/ArithmeticException"
        } else if err_msg.contains("StackOverflowError") {
            "java/lang/StackOverflowError"
        } else if err_msg.starts_with("UnsupportedOperationException") {
            "java/lang/UnsupportedOperationException"
        } else {
            // Last resort: treat any error as java/lang/RuntimeException so
            // catch(Exception e) / catch-all can still handle it.
            "java/lang/RuntimeException"
        };

        for entry in exception_table {
            let start = entry.start_pc as usize;
            let end = entry.end_pc as usize;
            if throw_pc < start || throw_pc >= end {
                continue;
            }
            // catch_type == 0 means catch-all (finally).
            if entry.catch_type == 0 {
                let exc_obj = self.take_or_create_exception(exc_class, err_msg);
                return Some((entry.handler_pc as usize, exc_obj));
            }
            // Resolve catch_type to class name and check if exception is instance.
            let catch_class = resolve_class_name(cp, entry.catch_type);
            if exc_class == catch_class || self.is_instance_of(exc_class, &catch_class) {
                let exc_obj = self.take_or_create_exception(exc_class, err_msg);
                return Some((entry.handler_pc as usize, exc_obj));
            }
        }
        // No handler found — do NOT clear pending_exception here; it must survive
        // propagation through intermediate frames until a handler is found upstream.
        None
    }

    /// Take the pending exception object if set, or create a new one.
    fn take_or_create_exception(&mut self, exc_class: &str, err_msg: &str) -> JValue {
        if let Some(r) = self.pending_exception.take() {
            JValue::Ref(Some(r))
        } else {
            // Create exception object with the message stored as a field.
            let exc = JObject::new(exc_class);
            // Store the error message in a "detailMessage" field (matches JDK Throwable).
            // Strip the "Exception: classname" prefix to get just the meaningful message.
            let msg_str = err_msg.strip_prefix("Exception: ")
                .and_then(|s| {
                    // After stripping "Exception: ", the remainder is class name.
                    // If there's a ": " after the class name, extract the actual message.
                    s.find(": ").map(|i| &s[i + 2..])
                })
                .unwrap_or(err_msg);
            exc.borrow_mut().fields.insert(
                "detailMessage".to_owned(),
                JValue::Ref(Some(JObject::new_string(msg_str))),
            );
            JValue::Ref(Some(exc))
        }
    }

    /// Execute a single opcode. Returns:
    /// - Ok(Some(value)) if the method returns
    /// - Ok(None) if execution should continue
    /// - Err(msg) if an exception was thrown
    fn execute_opcode(
        &mut self,
        frame: &mut Frame,
        code: &[u8],
        cp: &[ConstantPoolEntry],
        class_name: &str,
        bootstrap_methods: &[BootstrapMethod],
        _exception_table: &[ExceptionTableEntry],
        opcode: u8,
    ) -> Result<Option<JValue>, String> {
            match opcode {
                // ---- Constants ----
                0x00 => {} // nop
                0x01 => frame.stack.push(JValue::Ref(None)), // aconst_null
                0x02 => frame.stack.push(JValue::Int(-1)),   // iconst_m1
                0x03 => frame.stack.push(JValue::Int(0)),    // iconst_0
                0x04 => frame.stack.push(JValue::Int(1)),    // iconst_1
                0x05 => frame.stack.push(JValue::Int(2)),    // iconst_2
                0x06 => frame.stack.push(JValue::Int(3)),    // iconst_3
                0x07 => frame.stack.push(JValue::Int(4)),    // iconst_4
                0x08 => frame.stack.push(JValue::Int(5)),    // iconst_5
                0x09 => frame.stack.push(JValue::Long(0)),   // lconst_0
                0x0a => frame.stack.push(JValue::Long(1)),   // lconst_1
                0x0b => frame.stack.push(JValue::Float(0.0)),// fconst_0
                0x0c => frame.stack.push(JValue::Float(1.0)),// fconst_1
                0x0d => frame.stack.push(JValue::Float(2.0)),// fconst_2
                0x0e => frame.stack.push(JValue::Double(0.0)),// dconst_0
                0x0f => frame.stack.push(JValue::Double(1.0)),// dconst_1

                0x10 => { // bipush
                    let b = code[frame.pc] as i8;
                    frame.pc += 1;
                    frame.stack.push(JValue::Int(b as i32));
                }
                0x11 => { // sipush
                    let hi = code[frame.pc] as i16;
                    let lo = code[frame.pc + 1] as i16;
                    frame.pc += 2;
                    frame.stack.push(JValue::Int(((hi << 8) | lo) as i32));
                }
                0x12 => { // ldc
                    let idx = code[frame.pc] as u16;
                    frame.pc += 1;
                    self.push_ldc(frame, cp, idx);
                }
                0x13 | 0x14 => { // ldc_w / ldc2_w
                    let idx = u16::from_be_bytes([code[frame.pc], code[frame.pc + 1]]);
                    frame.pc += 2;
                    self.push_ldc(frame, cp, idx);
                }

                // ---- Loads ----
                0x15 => { let i = code[frame.pc] as usize; frame.pc += 1; frame.stack.push(frame.locals[i].clone()); } // iload
                0x16 => { let i = code[frame.pc] as usize; frame.pc += 1; frame.stack.push(frame.locals[i].clone()); } // lload
                0x17 => { let i = code[frame.pc] as usize; frame.pc += 1; frame.stack.push(frame.locals[i].clone()); } // fload
                0x18 => { let i = code[frame.pc] as usize; frame.pc += 1; frame.stack.push(frame.locals[i].clone()); } // dload
                0x19 => { let i = code[frame.pc] as usize; frame.pc += 1; frame.stack.push(frame.locals[i].clone()); } // aload

                0x1a..=0x1d => { let i = (opcode - 0x1a) as usize; frame.stack.push(frame.locals[i].clone()); } // iload_0..3
                0x1e..=0x21 => { let i = (opcode - 0x1e) as usize; frame.stack.push(frame.locals[i].clone()); } // lload_0..3
                0x22..=0x25 => { let i = (opcode - 0x22) as usize; frame.stack.push(frame.locals[i].clone()); } // fload_0..3
                0x26..=0x29 => { let i = (opcode - 0x26) as usize; frame.stack.push(frame.locals[i].clone()); } // dload_0..3
                0x2a..=0x2d => { let i = (opcode - 0x2a) as usize; frame.stack.push(frame.locals[i].clone()); } // aload_0..3

                // ---- Array loads ----
                0x32 => { // aaload
                    let idx = frame.stack.pop().unwrap().as_int() as usize;
                    let arr_ref = frame.stack.pop().unwrap();
                    if let Some(r) = arr_ref.as_ref() {
                        let elem = match &r.borrow().native {
                            NativePayload::Array(v) => v[idx].clone(),
                            _ => JValue::Ref(None),
                        };
                        frame.stack.push(elem);
                    } else {
                        return Err("NullPointerException: aaload".to_owned());
                    }
                }
                0x33 => { // baload
                    let idx = frame.stack.pop().unwrap().as_int() as usize;
                    let arr_ref = frame.stack.pop().unwrap();
                    if let Some(r) = arr_ref.as_ref() {
                        let elem = match &r.borrow().native {
                            NativePayload::ByteArray(v) => JValue::Int(v[idx] as i32),
                            NativePayload::Array(v) => {
                                let b = v.get(idx).map(|x| x.as_int() as i8).unwrap_or(0);
                                JValue::Int(b as i32)
                            }
                            _ => JValue::Int(0),
                        };
                        frame.stack.push(elem);
                    } else {
                        return Err("NullPointerException: baload".to_owned());
                    }
                }
                0x2e => { // iaload
                    let idx = frame.stack.pop().unwrap().as_int() as usize;
                    let arr_ref = frame.stack.pop().unwrap();
                    if let Some(r) = arr_ref.as_ref() {
                        let elem = match &r.borrow().native {
                            NativePayload::IntArray(v) => JValue::Int(v[idx]),
                            NativePayload::Array(v) => v.get(idx).cloned().unwrap_or(JValue::Int(0)),
                            _ => JValue::Int(0),
                        };
                        frame.stack.push(elem);
                    } else {
                        return Err("NullPointerException: iaload".to_owned());
                    }
                }
                0x2f | 0x30 | 0x31 | 0x35 => { // laload, faload, daload, saload
                    let idx = frame.stack.pop().unwrap().as_int() as usize;
                    let arr_ref = frame.stack.pop().unwrap();
                    if let Some(r) = arr_ref.as_ref() {
                        let elem = match &r.borrow().native {
                            NativePayload::Array(v) => v.get(idx).cloned().unwrap_or(JValue::Int(0)),
                            NativePayload::LongArray(v) => JValue::Long(v[idx]),
                            NativePayload::IntArray(v) => JValue::Int(v[idx]),
                            _ => JValue::Int(0),
                        };
                        frame.stack.push(elem);
                    } else {
                        return Err("NullPointerException: array load".to_owned());
                    }
                }

                // ---- Stores ----
                0x36 => { let i = code[frame.pc] as usize; frame.pc += 1; let v = frame.stack.pop().unwrap(); frame.locals[i] = v; } // istore
                0x37 => { let i = code[frame.pc] as usize; frame.pc += 1; let v = frame.stack.pop().unwrap(); frame.locals[i] = v; } // lstore
                0x38 => { let i = code[frame.pc] as usize; frame.pc += 1; let v = frame.stack.pop().unwrap(); frame.locals[i] = v; } // fstore
                0x39 => { let i = code[frame.pc] as usize; frame.pc += 1; let v = frame.stack.pop().unwrap(); frame.locals[i] = v; } // dstore
                0x3a => { let i = code[frame.pc] as usize; frame.pc += 1; let v = frame.stack.pop().unwrap(); frame.locals[i] = v; } // astore

                0x3b..=0x3e => { let i = (opcode - 0x3b) as usize; let v = frame.stack.pop().unwrap(); frame.locals[i] = v; } // istore_0..3
                0x3f..=0x42 => { let i = (opcode - 0x3f) as usize; let v = frame.stack.pop().unwrap(); frame.locals[i] = v; } // lstore_0..3
                0x43..=0x46 => { let i = (opcode - 0x43) as usize; let v = frame.stack.pop().unwrap(); frame.locals[i] = v; } // fstore_0..3
                0x47..=0x4a => { let i = (opcode - 0x47) as usize; let v = frame.stack.pop().unwrap(); frame.locals[i] = v; } // dstore_0..3
                0x4b..=0x4e => { let i = (opcode - 0x4b) as usize; let v = frame.stack.pop().unwrap(); frame.locals[i] = v; } // astore_0..3

                // ---- Array stores ----
                0x53 => { // aastore
                    let val = frame.stack.pop().unwrap();
                    let idx = frame.stack.pop().unwrap().as_int() as usize;
                    let arr_ref = frame.stack.pop().unwrap();
                    if let Some(r) = arr_ref.as_ref() {
                        if let NativePayload::Array(ref mut v) = r.borrow_mut().native {
                            while v.len() <= idx { v.push(JValue::Ref(None)); }
                            v[idx] = val;
                        }
                    }
                }

                0x4f => { // iastore
                    let val = frame.stack.pop().unwrap();
                    let idx = frame.stack.pop().unwrap().as_int() as usize;
                    let arr_ref = frame.stack.pop().unwrap();
                    if let Some(r) = arr_ref.as_ref() {
                        match r.borrow_mut().native {
                            NativePayload::Array(ref mut v) => {
                                while v.len() <= idx { v.push(JValue::Int(0)); }
                                v[idx] = val;
                            }
                            NativePayload::IntArray(ref mut v) => {
                                if idx < v.len() { v[idx] = val.as_int(); }
                            }
                            _ => {}
                        }
                    }
                }
                0x55 => { // castore (char array store — treated same as iastore)
                    let val = frame.stack.pop().unwrap();
                    let idx = frame.stack.pop().unwrap().as_int() as usize;
                    let arr_ref = frame.stack.pop().unwrap();
                    if let Some(r) = arr_ref.as_ref() {
                        if let NativePayload::Array(ref mut v) = r.borrow_mut().native {
                            while v.len() <= idx { v.push(JValue::Int(0)); }
                            v[idx] = val;
                        }
                    }
                }
                0x50 | 0x51 | 0x52 | 0x56 => { // lastore, fastore, dastore, sastore
                    let val = frame.stack.pop().unwrap();
                    let idx = frame.stack.pop().unwrap().as_int() as usize;
                    let arr_ref = frame.stack.pop().unwrap();
                    if let Some(r) = arr_ref.as_ref() {
                        match r.borrow_mut().native {
                            NativePayload::Array(ref mut v) => {
                                while v.len() <= idx { v.push(JValue::Int(0)); }
                                v[idx] = val;
                            }
                            NativePayload::LongArray(ref mut v) => {
                                if idx < v.len() { v[idx] = val.as_long(); }
                            }
                            _ => {}
                        }
                    }
                }
                0x54 => { // bastore
                    let val = frame.stack.pop().unwrap().as_int() as u8;
                    let idx = frame.stack.pop().unwrap().as_int() as usize;
                    let arr_ref = frame.stack.pop().unwrap();
                    if let Some(r) = arr_ref.as_ref() {
                        match r.borrow_mut().native {
                            NativePayload::ByteArray(ref mut v) => {
                                if idx < v.len() { v[idx] = val; }
                            }
                            NativePayload::Array(ref mut v) => {
                                while v.len() <= idx { v.push(JValue::Int(0)); }
                                v[idx] = JValue::Int(val as i32);
                            }
                            _ => {}
                        }
                    }
                }
                0x34 => { // caload (char array load)
                    let idx = frame.stack.pop().unwrap().as_int() as usize;
                    let arr_ref = frame.stack.pop().unwrap();
                    let val = match arr_ref.as_ref() {
                        Some(r) => match &r.borrow().native {
                            NativePayload::Array(v) => v.get(idx).cloned().unwrap_or(JValue::Int(0)),
                            _ => JValue::Int(0),
                        },
                        None => return Err("NullPointerException: caload".to_owned()),
                    };
                    frame.stack.push(val);
                }

                // ---- Stack manipulation ----
                0x57 => { frame.stack.pop(); }                                           // pop
                0x58 => { // pop2
                    let v1 = frame.stack.pop().unwrap();
                    if !is_category2(&v1) {
                        frame.stack.pop();
                    }
                }
                0x59 => { let v = frame.stack.last().unwrap().clone(); frame.stack.push(v); } // dup
                0x5a => { // dup_x1
                    let v1 = frame.stack.pop().unwrap();
                    let v2 = frame.stack.pop().unwrap();
                    frame.stack.push(v1.clone());
                    frame.stack.push(v2);
                    frame.stack.push(v1);
                }
                0x5b => { // dup_x2
                    let v1 = frame.stack.pop().unwrap();
                    let v2 = frame.stack.pop().unwrap();
                    if is_category2(&v2) {
                        // Form 2: ..., value2(cat2), value1(cat1) -> ..., value1, value2, value1
                        frame.stack.push(v1.clone());
                        frame.stack.push(v2);
                        frame.stack.push(v1);
                    } else {
                        // Form 1: ..., value3, value2, value1 (all cat1)
                        let v3 = frame.stack.pop().unwrap();
                        frame.stack.push(v1.clone());
                        frame.stack.push(v3);
                        frame.stack.push(v2);
                        frame.stack.push(v1);
                    }
                }
                0x5c => { // dup2
                    let v1 = frame.stack.pop().unwrap();
                    if is_category2(&v1) {
                        // Form 2: ..., value1(cat2) -> ..., value1, value1
                        frame.stack.push(v1.clone());
                        frame.stack.push(v1);
                    } else {
                        // Form 1: ..., value2, value1 (both cat1)
                        let v2 = frame.stack.pop().unwrap();
                        frame.stack.push(v2.clone());
                        frame.stack.push(v1.clone());
                        frame.stack.push(v2);
                        frame.stack.push(v1);
                    }
                }
                0x5d => { // dup2_x1
                    let v1 = frame.stack.pop().unwrap();
                    if is_category2(&v1) {
                        // Form 2: ..., value2(cat1), value1(cat2) -> ..., value1, value2, value1
                        let v2 = frame.stack.pop().unwrap();
                        frame.stack.push(v1.clone());
                        frame.stack.push(v2);
                        frame.stack.push(v1);
                    } else {
                        // Form 1: ..., value3, value2, value1 (all cat1)
                        let v2 = frame.stack.pop().unwrap();
                        let v3 = frame.stack.pop().unwrap();
                        frame.stack.push(v2.clone());
                        frame.stack.push(v1.clone());
                        frame.stack.push(v3);
                        frame.stack.push(v2);
                        frame.stack.push(v1);
                    }
                }
                0x5e => { // dup2_x2
                    let v1 = frame.stack.pop().unwrap();
                    if is_category2(&v1) {
                        let v2 = frame.stack.pop().unwrap();
                        if is_category2(&v2) {
                            // Form 4: ..., value2(cat2), value1(cat2) -> ..., value1, value2, value1
                            frame.stack.push(v1.clone());
                            frame.stack.push(v2);
                            frame.stack.push(v1);
                        } else {
                            // Form 3: ..., value3(cat1), value2(cat1), value1(cat2)
                            let v3 = frame.stack.pop().unwrap();
                            frame.stack.push(v1.clone());
                            frame.stack.push(v3);
                            frame.stack.push(v2);
                            frame.stack.push(v1);
                        }
                    } else {
                        let v2 = frame.stack.pop().unwrap(); // cat1 expected
                        let v3 = frame.stack.pop().unwrap();
                        if is_category2(&v3) {
                            // Form 2: ..., value3(cat2), value2(cat1), value1(cat1)
                            frame.stack.push(v2.clone());
                            frame.stack.push(v1.clone());
                            frame.stack.push(v3);
                            frame.stack.push(v2);
                            frame.stack.push(v1);
                        } else {
                            // Form 1: ..., value4, value3, value2, value1 (all cat1)
                            let v4 = frame.stack.pop().unwrap();
                            frame.stack.push(v2.clone());
                            frame.stack.push(v1.clone());
                            frame.stack.push(v4);
                            frame.stack.push(v3);
                            frame.stack.push(v2);
                            frame.stack.push(v1);
                        }
                    }
                }
                0x5f => { // swap
                    let v1 = frame.stack.pop().unwrap();
                    let v2 = frame.stack.pop().unwrap();
                    frame.stack.push(v1);
                    frame.stack.push(v2);
                }

                // ---- Arithmetic (int) ----
                0x60 => { let b = frame.stack.pop().unwrap().as_int(); let a = frame.stack.pop().unwrap().as_int(); frame.stack.push(JValue::Int(a.wrapping_add(b))); } // iadd
                0x64 => { let b = frame.stack.pop().unwrap().as_int(); let a = frame.stack.pop().unwrap().as_int(); frame.stack.push(JValue::Int(a.wrapping_sub(b))); } // isub
                0x68 => { let b = frame.stack.pop().unwrap().as_int(); let a = frame.stack.pop().unwrap().as_int(); frame.stack.push(JValue::Int(a.wrapping_mul(b))); } // imul
                0x6c => { let b = frame.stack.pop().unwrap().as_int(); let a = frame.stack.pop().unwrap().as_int(); frame.stack.push(JValue::Int(a.wrapping_div(b))); } // idiv
                0x70 => { let b = frame.stack.pop().unwrap().as_int(); let a = frame.stack.pop().unwrap().as_int(); frame.stack.push(JValue::Int(a.wrapping_rem(b))); } // irem
                0x74 => { let a = frame.stack.pop().unwrap().as_int(); frame.stack.push(JValue::Int(a.wrapping_neg())); } // ineg
                0x7e => { let b = frame.stack.pop().unwrap().as_int(); let a = frame.stack.pop().unwrap().as_int(); frame.stack.push(JValue::Int(a & b)); } // iand
                0x80 => { let b = frame.stack.pop().unwrap().as_int(); let a = frame.stack.pop().unwrap().as_int(); frame.stack.push(JValue::Int(a | b)); } // ior
                0x82 => { let b = frame.stack.pop().unwrap().as_int(); let a = frame.stack.pop().unwrap().as_int(); frame.stack.push(JValue::Int(a ^ b)); } // ixor
                0x78 => { let b = frame.stack.pop().unwrap().as_int() & 0x1f; let a = frame.stack.pop().unwrap().as_int(); frame.stack.push(JValue::Int(a << b)); } // ishl
                0x7a => { let b = frame.stack.pop().unwrap().as_int() & 0x1f; let a = frame.stack.pop().unwrap().as_int(); frame.stack.push(JValue::Int(a >> b)); } // ishr
                0x7c => { let b = frame.stack.pop().unwrap().as_int() & 0x1f; let a = frame.stack.pop().unwrap().as_int(); frame.stack.push(JValue::Int(((a as u32) >> b) as i32)); } // iushr

                // ---- Arithmetic (long) ----
                0x61 => { let b = frame.stack.pop().unwrap().as_long(); let a = frame.stack.pop().unwrap().as_long(); frame.stack.push(JValue::Long(a.wrapping_add(b))); } // ladd
                0x65 => { let b = frame.stack.pop().unwrap().as_long(); let a = frame.stack.pop().unwrap().as_long(); frame.stack.push(JValue::Long(a.wrapping_sub(b))); } // lsub
                0x69 => { let b = frame.stack.pop().unwrap().as_long(); let a = frame.stack.pop().unwrap().as_long(); frame.stack.push(JValue::Long(a.wrapping_mul(b))); } // lmul
                0x6d => { let b = frame.stack.pop().unwrap().as_long(); let a = frame.stack.pop().unwrap().as_long(); frame.stack.push(JValue::Long(a.wrapping_div(b))); } // ldiv
                0x71 => { let b = frame.stack.pop().unwrap().as_long(); let a = frame.stack.pop().unwrap().as_long(); frame.stack.push(JValue::Long(a.wrapping_rem(b))); } // lrem
                0x75 => { let a = frame.stack.pop().unwrap().as_long(); frame.stack.push(JValue::Long(a.wrapping_neg())); } // lneg
                0x7f => { let b = frame.stack.pop().unwrap().as_long(); let a = frame.stack.pop().unwrap().as_long(); frame.stack.push(JValue::Long(a & b)); } // land
                0x81 => { let b = frame.stack.pop().unwrap().as_long(); let a = frame.stack.pop().unwrap().as_long(); frame.stack.push(JValue::Long(a | b)); } // lor
                0x83 => { let b = frame.stack.pop().unwrap().as_long(); let a = frame.stack.pop().unwrap().as_long(); frame.stack.push(JValue::Long(a ^ b)); } // lxor
                0x79 => { let b = frame.stack.pop().unwrap().as_int() & 0x3f; let a = frame.stack.pop().unwrap().as_long(); frame.stack.push(JValue::Long(a << b)); } // lshl
                0x7b => { let b = frame.stack.pop().unwrap().as_int() & 0x3f; let a = frame.stack.pop().unwrap().as_long(); frame.stack.push(JValue::Long(a >> b)); } // lshr
                0x7d => { let b = frame.stack.pop().unwrap().as_int() & 0x3f; let a = frame.stack.pop().unwrap().as_long(); frame.stack.push(JValue::Long(((a as u64) >> b) as i64)); } // lushr
                0x94 => { // lcmp
                    let b = frame.stack.pop().unwrap().as_long();
                    let a = frame.stack.pop().unwrap().as_long();
                    let v = if a < b { -1 } else if a == b { 0 } else { 1 };
                    frame.stack.push(JValue::Int(v));
                }

                // ---- Arithmetic (float) ----
                0x62 => { let b = frame.stack.pop().unwrap().as_float(); let a = frame.stack.pop().unwrap().as_float(); frame.stack.push(JValue::Float(a + b)); } // fadd
                0x66 => { let b = frame.stack.pop().unwrap().as_float(); let a = frame.stack.pop().unwrap().as_float(); frame.stack.push(JValue::Float(a - b)); } // fsub
                0x6a => { let b = frame.stack.pop().unwrap().as_float(); let a = frame.stack.pop().unwrap().as_float(); frame.stack.push(JValue::Float(a * b)); } // fmul
                0x6e => { let b = frame.stack.pop().unwrap().as_float(); let a = frame.stack.pop().unwrap().as_float(); frame.stack.push(JValue::Float(a / b)); } // fdiv
                0x72 => { let b = frame.stack.pop().unwrap().as_float(); let a = frame.stack.pop().unwrap().as_float(); frame.stack.push(JValue::Float(a % b)); } // frem
                0x76 => { let a = frame.stack.pop().unwrap().as_float(); frame.stack.push(JValue::Float(-a)); } // fneg
                0x95 => { // fcmpl (NaN → -1)
                    let b = frame.stack.pop().unwrap().as_float();
                    let a = frame.stack.pop().unwrap().as_float();
                    // If either is NaN, none of >, ==, < are true → falls to else (-1).
                    frame.stack.push(JValue::Int(if a > b { 1 } else if a == b { 0 } else if a < b { -1 } else { -1 }));
                }
                0x96 => { // fcmpg (NaN → 1)
                    let b = frame.stack.pop().unwrap().as_float();
                    let a = frame.stack.pop().unwrap().as_float();
                    // If either is NaN, none of >, ==, < are true → falls to else (1).
                    frame.stack.push(JValue::Int(if a > b { 1 } else if a == b { 0 } else if a < b { -1 } else { 1 }));
                }

                // ---- Arithmetic (double) ----
                0x63 => { let b = frame.stack.pop().unwrap().as_double(); let a = frame.stack.pop().unwrap().as_double(); frame.stack.push(JValue::Double(a + b)); } // dadd
                0x67 => { let b = frame.stack.pop().unwrap().as_double(); let a = frame.stack.pop().unwrap().as_double(); frame.stack.push(JValue::Double(a - b)); } // dsub
                0x6b => { let b = frame.stack.pop().unwrap().as_double(); let a = frame.stack.pop().unwrap().as_double(); frame.stack.push(JValue::Double(a * b)); } // dmul
                0x6f => { let b = frame.stack.pop().unwrap().as_double(); let a = frame.stack.pop().unwrap().as_double(); frame.stack.push(JValue::Double(a / b)); } // ddiv
                0x73 => { let b = frame.stack.pop().unwrap().as_double(); let a = frame.stack.pop().unwrap().as_double(); frame.stack.push(JValue::Double(a % b)); } // drem
                0x77 => { let a = frame.stack.pop().unwrap().as_double(); frame.stack.push(JValue::Double(-a)); } // dneg
                0x97 => { // dcmpl (NaN → -1)
                    let b = frame.stack.pop().unwrap().as_double();
                    let a = frame.stack.pop().unwrap().as_double();
                    // If either is NaN, none of >, ==, < are true → falls to else (-1).
                    frame.stack.push(JValue::Int(if a > b { 1 } else if a == b { 0 } else if a < b { -1 } else { -1 }));
                }
                0x98 => { // dcmpg (NaN → 1)
                    let b = frame.stack.pop().unwrap().as_double();
                    let a = frame.stack.pop().unwrap().as_double();
                    // If either is NaN, none of >, ==, < are true → falls to else (1).
                    frame.stack.push(JValue::Int(if a > b { 1 } else if a == b { 0 } else if a < b { -1 } else { 1 }));
                }

                // ---- Conversions ----
                0x85 => { let v = frame.stack.pop().unwrap().as_int(); frame.stack.push(JValue::Long(v as i64)); } // i2l
                0x86 => { let v = frame.stack.pop().unwrap().as_int(); frame.stack.push(JValue::Float(v as f32)); } // i2f
                0x87 => { let v = frame.stack.pop().unwrap().as_int(); frame.stack.push(JValue::Double(v as f64)); } // i2d
                0x88 => { let v = frame.stack.pop().unwrap().as_long() as i32; frame.stack.push(JValue::Int(v)); } // l2i
                0x89 => { let v = frame.stack.pop().unwrap().as_long(); frame.stack.push(JValue::Float(v as f32)); } // l2f
                0x8a => { let v = frame.stack.pop().unwrap().as_long(); frame.stack.push(JValue::Double(v as f64)); } // l2d
                0x8b => { let v = frame.stack.pop().unwrap().as_float(); frame.stack.push(JValue::Int(float_to_int(v))); } // f2i
                0x8c => { let v = frame.stack.pop().unwrap().as_float(); frame.stack.push(JValue::Long(float_to_long(v))); } // f2l
                0x8d => { let v = frame.stack.pop().unwrap().as_float(); frame.stack.push(JValue::Double(v as f64)); } // f2d
                0x8e => { let v = frame.stack.pop().unwrap().as_double(); frame.stack.push(JValue::Int(double_to_int(v))); } // d2i
                0x8f => { let v = frame.stack.pop().unwrap().as_double(); frame.stack.push(JValue::Long(double_to_long(v))); } // d2l
                0x90 => { let v = frame.stack.pop().unwrap().as_double(); frame.stack.push(JValue::Float(v as f32)); } // d2f
                0x91 => { let v = frame.stack.pop().unwrap().as_int() as i8; frame.stack.push(JValue::Int(v as i32)); } // i2b
                0x92 => { let v = frame.stack.pop().unwrap().as_int() as u16; frame.stack.push(JValue::Int(v as i32)); } // i2c
                0x93 => { let v = frame.stack.pop().unwrap().as_int() as i16; frame.stack.push(JValue::Int(v as i32)); } // i2s

                // ---- iinc ----
                0x84 => {
                    let idx = code[frame.pc] as usize;
                    let c = code[frame.pc + 1] as i8;
                    frame.pc += 2;
                    if let JValue::Int(ref mut v) = frame.locals[idx] {
                        *v = v.wrapping_add(c as i32);
                    }
                }

                // ---- Comparisons / branches (int) ----
                0x99 => { let off = read_i16(code, &mut frame.pc); let v = frame.stack.pop().unwrap().as_int(); if v == 0 { frame.pc = (frame.pc as i32 - 3 + off as i32) as usize; } } // ifeq
                0x9a => { let off = read_i16(code, &mut frame.pc); let v = frame.stack.pop().unwrap().as_int(); if v != 0 { frame.pc = (frame.pc as i32 - 3 + off as i32) as usize; } } // ifne
                0x9b => { let off = read_i16(code, &mut frame.pc); let v = frame.stack.pop().unwrap().as_int(); if v < 0  { frame.pc = (frame.pc as i32 - 3 + off as i32) as usize; } } // iflt
                0x9c => { let off = read_i16(code, &mut frame.pc); let v = frame.stack.pop().unwrap().as_int(); if v >= 0 { frame.pc = (frame.pc as i32 - 3 + off as i32) as usize; } } // ifge
                0x9d => { let off = read_i16(code, &mut frame.pc); let v = frame.stack.pop().unwrap().as_int(); if v > 0  { frame.pc = (frame.pc as i32 - 3 + off as i32) as usize; } } // ifgt
                0x9e => { let off = read_i16(code, &mut frame.pc); let v = frame.stack.pop().unwrap().as_int(); if v <= 0 { frame.pc = (frame.pc as i32 - 3 + off as i32) as usize; } } // ifle

                0x9f => { let off = read_i16(code, &mut frame.pc); let b = frame.stack.pop().unwrap().as_int(); let a = frame.stack.pop().unwrap().as_int(); if a == b { frame.pc = (frame.pc as i32 - 3 + off as i32) as usize; } } // if_icmpeq
                0xa0 => { let off = read_i16(code, &mut frame.pc); let b = frame.stack.pop().unwrap().as_int(); let a = frame.stack.pop().unwrap().as_int(); if a != b { frame.pc = (frame.pc as i32 - 3 + off as i32) as usize; } } // if_icmpne
                0xa1 => { let off = read_i16(code, &mut frame.pc); let b = frame.stack.pop().unwrap().as_int(); let a = frame.stack.pop().unwrap().as_int(); if a < b  { frame.pc = (frame.pc as i32 - 3 + off as i32) as usize; } } // if_icmplt
                0xa2 => { let off = read_i16(code, &mut frame.pc); let b = frame.stack.pop().unwrap().as_int(); let a = frame.stack.pop().unwrap().as_int(); if a >= b { frame.pc = (frame.pc as i32 - 3 + off as i32) as usize; } } // if_icmpge
                0xa3 => { let off = read_i16(code, &mut frame.pc); let b = frame.stack.pop().unwrap().as_int(); let a = frame.stack.pop().unwrap().as_int(); if a > b  { frame.pc = (frame.pc as i32 - 3 + off as i32) as usize; } } // if_icmpgt
                0xa4 => { let off = read_i16(code, &mut frame.pc); let b = frame.stack.pop().unwrap().as_int(); let a = frame.stack.pop().unwrap().as_int(); if a <= b { frame.pc = (frame.pc as i32 - 3 + off as i32) as usize; } } // if_icmple

                // ---- Reference comparisons ----
                0xa5 => { // if_acmpeq
                    let off = read_i16(code, &mut frame.pc);
                    let b = frame.stack.pop().unwrap();
                    let a = frame.stack.pop().unwrap();
                    if refs_equal(&a, &b) { frame.pc = (frame.pc as i32 - 3 + off as i32) as usize; }
                }
                0xa6 => { // if_acmpne
                    let off = read_i16(code, &mut frame.pc);
                    let b = frame.stack.pop().unwrap();
                    let a = frame.stack.pop().unwrap();
                    if !refs_equal(&a, &b) { frame.pc = (frame.pc as i32 - 3 + off as i32) as usize; }
                }
                0xc6 => { // ifnull
                    let off = read_i16(code, &mut frame.pc);
                    let v = frame.stack.pop().unwrap();
                    if v.is_null() { frame.pc = (frame.pc as i32 - 3 + off as i32) as usize; }
                }
                0xc7 => { // ifnonnull
                    let off = read_i16(code, &mut frame.pc);
                    let v = frame.stack.pop().unwrap();
                    if !v.is_null() { frame.pc = (frame.pc as i32 - 3 + off as i32) as usize; }
                }

                // ---- Unconditional jump ----
                0xa7 => { // goto
                    let off = read_i16(code, &mut frame.pc);
                    frame.pc = (frame.pc as i32 - 3 + off as i32) as usize;
                }
                0xc8 => { // goto_w
                    let off = read_i32(code, &mut frame.pc);
                    frame.pc = (frame.pc as i32 - 5 + off) as usize;
                }

                // ---- tableswitch ----
                0xaa => {
                    let base_pc = frame.pc - 1;
                    // Skip padding to align on 4-byte boundary.
                    while frame.pc % 4 != 0 { frame.pc += 1; }
                    let default_off = read_i32(code, &mut frame.pc);
                    let low = read_i32(code, &mut frame.pc);
                    let high = read_i32(code, &mut frame.pc);
                    let count = (high - low + 1) as usize;
                    let offsets: Vec<i32> = (0..count).map(|_| read_i32(code, &mut frame.pc)).collect();
                    let key = frame.stack.pop().unwrap().as_int();
                    let off = if key >= low && key <= high {
                        offsets[(key - low) as usize]
                    } else {
                        default_off
                    };
                    frame.pc = (base_pc as i32 + off) as usize;
                }

                // ---- lookupswitch ----
                0xab => {
                    let base_pc = frame.pc - 1;
                    while frame.pc % 4 != 0 { frame.pc += 1; }
                    let default_off = read_i32(code, &mut frame.pc);
                    let npairs = read_i32(code, &mut frame.pc) as usize;
                    let pairs: Vec<(i32, i32)> = (0..npairs)
                        .map(|_| (read_i32(code, &mut frame.pc), read_i32(code, &mut frame.pc)))
                        .collect();
                    let key = frame.stack.pop().unwrap().as_int();
                    let off = pairs.iter().find(|(k, _)| *k == key).map(|(_, v)| *v).unwrap_or(default_off);
                    frame.pc = (base_pc as i32 + off) as usize;
                }

                // ---- Returns ----
                0xac => return Ok(Some(frame.stack.pop().unwrap())), // ireturn
                0xad => return Ok(Some(frame.stack.pop().unwrap())), // lreturn
                0xae => return Ok(Some(frame.stack.pop().unwrap())), // freturn
                0xaf => return Ok(Some(frame.stack.pop().unwrap())), // dreturn
                0xb0 => return Ok(Some(frame.stack.pop().unwrap())), // areturn
                0xb1 => return Ok(Some(JValue::Void)),               // return

                // ---- Field access ----
                0xb2 => { // getstatic
                    let idx = read_u16(code, &mut frame.pc);
                    let v = self.resolve_static_field(cp, idx)?;
                    frame.stack.push(v);
                }
                0xb3 => { // putstatic
                    let idx = read_u16(code, &mut frame.pc);
                    let val = frame.stack.pop().unwrap_or(JValue::Void);
                    let (cls, fld, _) = resolve_fieldref(cp, idx);
                    // Per JVMS §5.5: putstatic triggers class initialization.
                    self.ensure_class_init(&cls)?;
                    self.static_fields.insert(format!("{cls}.{fld}"), val);
                }
                0xb4 => { // getfield
                    let idx = read_u16(code, &mut frame.pc);
                    let (_, gf_field_name, _) = resolve_fieldref(cp, idx);
                    let obj_ref = frame.stack.pop().unwrap_or_else(|| {
                        panic!("getfield {gf_field_name}: empty stack in {class_name}")
                    });
                    if matches!(obj_ref, JValue::Void) {
                        return Err(format!(
                            "getfield {gf_field_name}: expected Ref on stack, got Void in {class_name}"
                        ));
                    }
                    let v = self.resolve_instance_field(cp, idx, &obj_ref)?;
                    frame.stack.push(v);
                }
                0xb5 => { // putfield
                    let idx = read_u16(code, &mut frame.pc);
                    let val = frame.stack.pop().unwrap_or(JValue::Void);
                    let obj_ref = frame.stack.pop().unwrap_or(JValue::Void);
                    if matches!(obj_ref, JValue::Void) {
                        let (_, pf_field_name, _) = resolve_fieldref(cp, idx);
                        return Err(format!(
                            "putfield {pf_field_name}: expected Ref on stack, got Void in {class_name}"
                        ));
                    }
                    self.set_instance_field(cp, idx, &obj_ref, val)?;
                }

                // ---- Method invocation ----
                0xb6 => { // invokevirtual
                    let idx = read_u16(code, &mut frame.pc);
                    let result = self.dispatch_virtual(cp, idx, frame).map_err(|e| {
                        if e.starts_with("NullPointerException") {
                            format!("{e} in {class_name}")
                        } else {
                            e
                        }
                    })?;
                    if !matches!(result, JValue::Void) { frame.stack.push(result); }
                }
                0xb7 => { // invokespecial
                    let idx = read_u16(code, &mut frame.pc);
                    let result = self.dispatch_special(cp, idx, frame).map_err(|e| {
                        if e.starts_with("NullPointerException") {
                            format!("{e} in {class_name}")
                        } else {
                            e
                        }
                    })?;
                    if !matches!(result, JValue::Void) { frame.stack.push(result); }
                }
                0xb8 => { // invokestatic
                    let idx = read_u16(code, &mut frame.pc);
                    let result = self.dispatch_static(cp, idx, frame)?;
                    if !matches!(result, JValue::Void) { frame.stack.push(result); }
                }
                0xb9 => { // invokeinterface
                    let idx = read_u16(code, &mut frame.pc);
                    frame.pc += 2; // count + 0
                    let result = self.dispatch_interface(cp, idx, frame).map_err(|e| {
                        if e.starts_with("NullPointerException") {
                            format!("{e} in {class_name}")
                        } else {
                            e
                        }
                    })?;
                    if !matches!(result, JValue::Void) { frame.stack.push(result); }
                }
                0xba => { // invokedynamic
                    let idx = read_u16(code, &mut frame.pc);
                    frame.pc += 2; // reserved bytes
                    // NOTE: JVMS §6.5.invokedynamic says CallSite should be cached per instruction.
                    // Our VM doesn't use CallSite indirection, so caching isn't applicable here.
                    // This is a performance concern only; correctness is unaffected.
                    let result = self.dispatch_invokedynamic(cp, idx, frame, class_name, bootstrap_methods)?;
                    if !matches!(result, JValue::Void) { frame.stack.push(result); }
                }

                // ---- Object creation ----
                0xbb => { // new
                    let idx = read_u16(code, &mut frame.pc);
                    let new_class = resolve_class_name(cp, idx);
                    // Run <clinit> for the class being instantiated.
                    self.ensure_class_init(&new_class)?;
                    let obj = if self.classes.contains_key(&new_class) {
                        // Class is loaded (bytecode available) — use plain object.
                        JObject::new(new_class)
                    } else {
                        match new_class.as_str() {
                            // JDK collection types backed by Array payload (no shim loaded).
                            "java/util/ArrayList" | "java/util/LinkedList" =>
                                JObject::new_array(new_class, vec![]),
                            _ => JObject::new(new_class),
                        }
                    };
                    frame.stack.push(JValue::Ref(Some(obj)));
                }
                0xbc => { // newarray
                    let atype = code[frame.pc]; frame.pc += 1;
                    let count = frame.stack.pop().unwrap().as_int() as usize;
                    let arr = match atype {
                        4 => JObject::new_array("[Z", vec![JValue::Int(0); count]),   // boolean
                        5 => JObject::new_array("[C", vec![JValue::Int(0); count]),   // char
                        6 => JObject::new_array("[F", vec![JValue::Float(0.0); count]), // float
                        7 => JObject::new_array("[D", vec![JValue::Double(0.0); count]), // double
                        8 => JObject::new_array("[B", vec![JValue::Int(0); count]),   // byte
                        9 => JObject::new_array("[S", vec![JValue::Int(0); count]),   // short
                        10 => JObject::new_array("[I", vec![JValue::Int(0); count]),  // int
                        11 => JObject::new_array("[J", vec![JValue::Long(0); count]), // long
                        _ => JObject::new_array("[Ljava/lang/Object;", vec![JValue::Ref(None); count]),
                    };
                    frame.stack.push(JValue::Ref(Some(arr)));
                }
                0xbd => { // anewarray
                    let idx = read_u16(code, &mut frame.pc);
                    let elem_class = resolve_class_name(cp, idx);
                    let count = frame.stack.pop().unwrap().as_int() as usize;
                    let arr = JObject::new_array(
                        format!("[L{elem_class};"),
                        vec![JValue::Ref(None); count],
                    );
                    frame.stack.push(JValue::Ref(Some(arr)));
                }
                0xc5 => { // multianewarray
                    let idx = read_u16(code, &mut frame.pc);
                    let dimensions = code[frame.pc] as usize;
                    frame.pc += 1;
                    let class_name_str = resolve_class_name(cp, idx);
                    let mut dim_sizes = Vec::with_capacity(dimensions);
                    for _ in 0..dimensions {
                        dim_sizes.push(frame.stack.pop().unwrap().as_int() as usize);
                    }
                    dim_sizes.reverse();
                    let arr = self.create_multi_array(&class_name_str, &dim_sizes, 0);
                    frame.stack.push(JValue::Ref(Some(arr)));
                }
                0xbe => { // arraylength
                    let arr_ref = frame.stack.pop().unwrap();
                    let len = match arr_ref.as_ref() {
                        Some(r) => match &r.borrow().native {
                            NativePayload::Array(v) => v.len() as i32,
                            NativePayload::ByteArray(v) => v.len() as i32,
                            NativePayload::IntArray(v) => v.len() as i32,
                            NativePayload::LongArray(v) => v.len() as i32,
                            _ => 0,
                        },
                        None => return Err("NullPointerException: arraylength".to_owned()),
                    };
                    frame.stack.push(JValue::Int(len));
                }

                // ---- instanceof / checkcast ----
                0xc0 => { // checkcast — per JVMS §6.5.checkcast
                    let idx = read_u16(code, &mut frame.pc);
                    let target_class = resolve_class_name(cp, idx);
                    // Peek at top of stack (don't pop — value stays if check passes).
                    let obj = frame.stack.last().unwrap();
                    match obj.as_ref() {
                        None => {} // null passes checkcast
                        Some(r) => {
                            let cn = r.borrow().class_name.clone();
                            if !self.is_instance_of(&cn, &target_class) {
                                return Err(format!(
                                    "ClassCastException: {} cannot be cast to {}",
                                    cn.replace('/', "."),
                                    target_class.replace('/', ".")
                                ));
                            }
                        }
                    }
                }
                0xc1 => { // instanceof
                    let idx = read_u16(code, &mut frame.pc);
                    let target_class = resolve_class_name(cp, idx);
                    let obj = frame.stack.pop().unwrap();
                    let is_instance = match obj.as_ref() {
                        None => false,
                        Some(r) => {
                            let cn = r.borrow().class_name.clone();
                            self.is_instance_of(&cn, &target_class)
                        }
                    };
                    frame.stack.push(JValue::Int(is_instance as i32));
                }

                // ---- wide prefix ----
                0xc4 => {
                    let sub = code[frame.pc]; frame.pc += 1;
                    let local_idx = read_u16(code, &mut frame.pc) as usize;
                    match sub {
                        0x15 | 0x16 | 0x17 | 0x18 | 0x19 => { frame.stack.push(frame.locals[local_idx].clone()); }
                        0x36 | 0x37 | 0x38 | 0x39 | 0x3a => { let v = frame.stack.pop().unwrap(); frame.locals[local_idx] = v; }
                        0x84 => { let c = read_i16(code, &mut frame.pc); if let JValue::Int(ref mut v) = frame.locals[local_idx] { *v = v.wrapping_add(c as i32); } }
                        _ => return Err(format!("Unsupported wide sub-opcode: 0x{sub:02x}")),
                    }
                }

                // ---- athrow ----
                0xbf => {
                    let exc = frame.stack.pop().unwrap();
                    let (msg, exc_ref) = match exc {
                        JValue::Ref(Some(r)) => {
                            let detail = r
                                .borrow()
                                .fields
                                .get("detailMessage")
                                .and_then(|v| v.as_ref())
                                .and_then(|s| s.borrow().as_java_string().map(|x| x.to_owned()));
                            let msg = format!(
                                "Exception: {}{} at {}:pc{}",
                                r.borrow().class_name,
                                detail
                                    .as_ref()
                                    .map(|d| format!(": {d}"))
                                    .unwrap_or_default(),
                                class_name,
                                frame.pc.saturating_sub(1)
                            );
                            (msg, Some(r))
                        }
                        JValue::Ref(None) => {
                            let npe = JObject::new("java/lang/NullPointerException");
                            (
                                format!(
                                    "Exception: java/lang/NullPointerException at {}:pc{}",
                                    class_name,
                                    frame.pc.saturating_sub(1)
                                ),
                                Some(npe),
                            )
                        }
                        _ => (
                            format!(
                                "Exception: java/lang/RuntimeException at {}:pc{}",
                                class_name,
                                frame.pc.saturating_sub(1)
                            ),
                            None,
                        ),
                    };
                    if let Some(r) = exc_ref {
                        self.pending_exception = Some(r);
                    }
                    return Err(msg);
                }

                // ---- monitorenter / monitorexit (no-ops in single-threaded context) ----
                0xc2 | 0xc3 => { frame.stack.pop(); }

                other => {
                    return Err(format!(
                        "Unimplemented opcode 0x{other:02x} at pc {}",
                        frame.pc - 1
                    ));
                }
            }
            Ok(None)
    }

    // ------------------------------------------------------------------
    // Opcode helpers
    // ------------------------------------------------------------------

    fn push_ldc(&mut self, frame: &mut Frame, cp: &[ConstantPoolEntry], idx: u16) {
        match &cp[idx as usize] {
            ConstantPoolEntry::Integer(v) => frame.stack.push(JValue::Int(*v)),
            ConstantPoolEntry::Float(v) => frame.stack.push(JValue::Float(*v)),
            ConstantPoolEntry::Long(v) => frame.stack.push(JValue::Long(*v)),
            ConstantPoolEntry::Double(v) => frame.stack.push(JValue::Double(*v)),
            ConstantPoolEntry::String { string_index } => {
                let s = match &cp[*string_index as usize] {
                    ConstantPoolEntry::Utf8(s) => s.clone(),
                    _ => String::new(),
                };
                let obj = self.intern_string(s);
                frame.stack.push(JValue::Ref(Some(obj)));
            }
            ConstantPoolEntry::Class { name_index } => {
                let name = match &cp[*name_index as usize] {
                    ConstantPoolEntry::Utf8(s) => s.clone(),
                    _ => String::new(),
                };
                let obj = self.class_object(name);
                frame.stack.push(JValue::Ref(Some(obj)));
            }
            _other => {
                // MethodHandle, MethodType — push null as placeholder.
                frame.stack.push(JValue::Ref(None));
            }
        }
    }

    fn resolve_static_field(
        &mut self,
        cp: &[ConstantPoolEntry],
        idx: u16,
    ) -> Result<JValue, String> {
        let (class_name, field_name, descriptor) = resolve_fieldref(cp, idx);
        // Run <clinit> if not yet done (initialises static fields via putstatic).
        self.ensure_class_init(&class_name.clone())?;
        // Search this class and its super-class chain for the static field (JVMS §5.4.3.2).
        if let Some(v) = self.resolve_static_field_in_hierarchy(&class_name, &field_name) {
            return Ok(v);
        }
        // Well-known JDK static fields that cannot be initialised via <clinit>
        // because the JDK classes are not in the bundle.
        match (class_name.as_str(), field_name.as_str()) {
            ("java/lang/System", "out") => {
                let key = format!("{class_name}.{field_name}");
                if let Some(v) = self.static_fields.get(&key) {
                    return Ok(v.clone());
                }
                let v = JValue::Ref(Some(JObject::new_print_stream(false)));
                self.static_fields.insert(key, v.clone());
                Ok(v)
            }
            ("java/lang/System", "err") => {
                let key = format!("{class_name}.{field_name}");
                if let Some(v) = self.static_fields.get(&key) {
                    return Ok(v.clone());
                }
                let v = JValue::Ref(Some(JObject::new_print_stream(true)));
                self.static_fields.insert(key, v.clone());
                Ok(v)
            }
            _ => Ok(default_value_for_descriptor(&descriptor)),
        }
    }

    /// Walk the class hierarchy to find a static field value.
    fn resolve_static_field_in_hierarchy(&self, class_name: &str, field_name: &str) -> Option<JValue> {
        // Check this class first.
        let key = format!("{class_name}.{field_name}");
        if let Some(v) = self.static_fields.get(&key) {
            return Some(v.clone());
        }
        // Check super class.
        if let Some(class) = self.classes.get(class_name) {
            if class.super_class != 0 {
                let super_name = class.constant_pool.class_name(class.super_class).to_owned();
                if let Some(v) = self.resolve_static_field_in_hierarchy(&super_name, field_name) {
                    return Some(v);
                }
            }
            // Check interfaces.
            let iface_names: Vec<String> = class.interfaces.iter()
                .map(|&idx| class.constant_pool.class_name(idx).to_owned())
                .collect();
            for iface_name in iface_names {
                if let Some(v) = self.resolve_static_field_in_hierarchy(&iface_name, field_name) {
                    return Some(v);
                }
            }
        }
        None
    }

    fn resolve_instance_field(
        &mut self,
        cp: &[ConstantPoolEntry],
        idx: u16,
        obj_ref: &JValue,
    ) -> Result<JValue, String> {
        let (_, field_name, field_desc) = resolve_fieldref(cp, idx);
        match obj_ref.as_ref() {
            Some(r) => {
                let default = default_value_for_descriptor(&field_desc);
                Ok(r.borrow().fields.get(&field_name).cloned().unwrap_or(default))
            }
            None => Err(format!("NullPointerException: getfield {field_name}")),
        }
    }

    fn set_instance_field(
        &mut self,
        cp: &[ConstantPoolEntry],
        idx: u16,
        obj_ref: &JValue,
        val: JValue,
    ) -> Result<(), String> {
        let (_, field_name, _) = resolve_fieldref(cp, idx);
        match obj_ref.as_ref() {
            Some(r) => { r.borrow_mut().fields.insert(field_name, val); Ok(()) }
            None => Err(format!("NullPointerException: putfield {field_name}")),
        }
    }

    fn dispatch_static(
        &mut self,
        cp: &[ConstantPoolEntry],
        idx: u16,
        frame: &mut Frame,
    ) -> Result<JValue, String> {
        let (class_name, method_name, descriptor) = resolve_methodref(cp, idx);
        // Per JVMS §5.5: invokestatic triggers class initialization.
        self.ensure_class_init(&class_name)?;
        let n_args = count_args(&descriptor);
        let args = pop_args(frame, n_args);
        self.invoke_static(&class_name, &method_name, &descriptor, args)
    }

    fn dispatch_virtual(
        &mut self,
        cp: &[ConstantPoolEntry],
        idx: u16,
        frame: &mut Frame,
    ) -> Result<JValue, String> {
        let (class_name, method_name, descriptor) = resolve_methodref(cp, idx);
        let n_args = count_args(&descriptor);
        let args = pop_args(frame, n_args);
        let this_val = frame.stack.pop().unwrap();
        match this_val {
            JValue::Ref(Some(r)) => self.invoke_virtual(r, &class_name, &method_name, &descriptor, args),
            JValue::Ref(None) => Err(format!("NullPointerException: invokevirtual {class_name}.{method_name}{descriptor}")),
            other => Err(format!(
                "Expected reference for invokevirtual {class_name}.{method_name}{descriptor}, got {other:?}"
            )),
        }
    }

    fn dispatch_special(
        &mut self,
        cp: &[ConstantPoolEntry],
        idx: u16,
        frame: &mut Frame,
    ) -> Result<JValue, String> {
        let (class_name, method_name, descriptor) = resolve_methodref(cp, idx);
        let n_args = count_args(&descriptor);
        let args = pop_args(frame, n_args);
        let this_val = frame.stack.pop().unwrap();
        match this_val {
            JValue::Ref(Some(r)) => {
                // invokespecial does NOT do virtual dispatch — it calls the exact class
                // specified in the constant pool. This is critical for super.<init>() calls.
                if method_name == "<init>" {
                    // String constructors must be handled natively since String
                    // content is managed by NativePayload::JavaString.
                    if class_name == "java/lang/String" {
                        let s = self.string_from_init_args(&descriptor, &args, &r);
                        r.borrow_mut().native = NativePayload::JavaString(s);
                        return Ok(JValue::Void);
                    }
                    let has_method = self.find_method(&class_name, &method_name, &descriptor).is_some();
                    if !has_method {
                        // Constructor not in bundle — no-op fallback.
                        return Ok(JValue::Void);
                    }
                }
                // Use invoke_special (non-virtual) instead of invoke_virtual.
                self.invoke_special(r, &class_name, &method_name, &descriptor, args)
            }
            JValue::Ref(None) => Err(format!("NullPointerException: invokespecial {class_name}.{method_name}{descriptor}")),
            other => Err(format!(
                "Expected reference for invokespecial {class_name}.{method_name}{descriptor}, got {other:?}"
            )),
        }
    }

    fn dispatch_interface(
        &mut self,
        cp: &[ConstantPoolEntry],
        idx: u16,
        frame: &mut Frame,
    ) -> Result<JValue, String> {
        let (class_name, method_name, descriptor) = resolve_methodref(cp, idx);
        let n_args = count_args(&descriptor);
        let args = pop_args(frame, n_args);

        // Static interface methods (e.g. List.of()) have no receiver on the stack.
        // Detect by checking if the method exists as a static method in the interface class.
        let is_static = self.find_method(&class_name, &method_name, &descriptor)
            .map(|(_, m)| m.access_flags & 0x0008 != 0)
            .unwrap_or(false);
        if is_static {
            return self.invoke_static(&class_name, &method_name, &descriptor, args);
        }

        let this_val = frame.stack.pop().unwrap();
        match this_val {
            JValue::Ref(Some(r)) => self.invoke_virtual(r, &class_name, &method_name, &descriptor, args),
            JValue::Ref(None) => Err(format!("NullPointerException: invokeinterface {class_name}.{method_name}{descriptor}")),
            other => Err(format!(
                "Expected reference for invokeinterface {class_name}.{method_name}{descriptor}, got {other:?}"
            )),
        }
    }

    /// Handle `invokedynamic` — currently supports the three bootstrap methods
    /// used by Raoh: LambdaMetafactory, StringConcatFactory, SwitchBootstraps.
    fn dispatch_invokedynamic(
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

        let (_method_name, descriptor) = match &cp[nat_index as usize] {
            ConstantPoolEntry::NameAndType { name_index, descriptor_index } => {
                let n = match &cp[*name_index as usize] { ConstantPoolEntry::Utf8(s) => s.clone(), _ => String::new() };
                let d = match &cp[*descriptor_index as usize] { ConstantPoolEntry::Utf8(s) => s.clone(), _ => String::new() };
                (n, d)
            }
            other => return Err(format!("Expected NameAndType at cp[{nat_index}], got {other:?}")),
        };

        let bm = &bootstrap_methods[bm_index as usize];
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
                // Resolve it to (class, method, descriptor) so the VM can invoke it later.
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

                let sam_desc = bm.bootstrap_arguments.get(0).and_then(|&arg_idx| {
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
                            sam_method: _method_name,
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

                let mut result = String::new();
                let mut arg_idx = 0;
                for ch in recipe.chars() {
                    if ch == '\x01' {
                        // Substitute argument — call toString() for objects.
                        if let Some(a) = args.get(arg_idx) {
                            match a {
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
                        // \u0002 = constant from bootstrap args (skip for now)
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
                // Unknown bootstrap — push null.
                Ok(JValue::Ref(None))
            }
        }
    }

    // ------------------------------------------------------------------
    // Native method stubs
    // ------------------------------------------------------------------

    /// Handle static native methods. Returns `None` if not a known native.
    fn native_static(
        &mut self,
        _class_name: &str,
        _method_name: &str,
        _descriptor: &str,
        _args: &[JValue],
    ) -> Option<JValue> {
        match (_class_name, _method_name, _descriptor) {
            ("java/lang/Class", "forName0", "(Ljava/lang/String;)Ljava/lang/Class;") => {
                let runtime_name = _args
                    .first()
                    .and_then(|v| v.as_ref())
                    .and_then(|r| r.borrow().as_java_string().map(|s| s.to_owned()))?;
                let internal = Self::class_internal_name_from_runtime_name(&runtime_name);
                Some(JValue::Ref(Some(self.class_object(internal))))
            }
            ("java/lang/System", "currentTimeMillis", "()J") => {
                let ms = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .ok()
                    .map(|d| d.as_millis() as i64)
                    .unwrap_or(0);
                Some(JValue::Long(ms))
            }
            ("java/lang/System", "identityHashCode", "(Ljava/lang/Object;)I") => {
                let hc = _args
                    .first()
                    .and_then(|v| v.as_ref())
                    .map(|r| {
                        let ptr = Rc::as_ptr(r) as usize;
                        (ptr as u64 as u32) as i32
                    })
                    .unwrap_or(0);
                Some(JValue::Int(hc))
            }
            (
                "java/lang/System",
                "arraycopy",
                "(Ljava/lang/Object;ILjava/lang/Object;II)V",
            ) => {
                let src = _args.first().and_then(|v| v.as_ref()).cloned()?;
                let src_pos = _args.get(1).map(|v| v.as_int().max(0) as usize).unwrap_or(0);
                let dst = _args.get(2).and_then(|v| v.as_ref()).cloned()?;
                let dst_pos = _args.get(3).map(|v| v.as_int().max(0) as usize).unwrap_or(0);
                let len = _args.get(4).map(|v| v.as_int().max(0) as usize).unwrap_or(0);

                let src_snapshot = {
                    let src_b = src.borrow();
                    match &src_b.native {
                        NativePayload::Array(v) => Some(v.clone()),
                        NativePayload::ByteArray(v) => {
                            Some(v.iter().map(|b| JValue::Int(*b as i32)).collect())
                        }
                        NativePayload::IntArray(v) => {
                            Some(v.iter().map(|i| JValue::Int(*i)).collect())
                        }
                        NativePayload::LongArray(v) => {
                            Some(v.iter().map(|l| JValue::Long(*l)).collect())
                        }
                        _ => None,
                    }
                };

                if let Some(src_vals) = src_snapshot {
                    let src_end = src_pos.saturating_add(len).min(src_vals.len());
                    let count = src_end.saturating_sub(src_pos);
                    let mut dst_b = dst.borrow_mut();
                    match &mut dst_b.native {
                        NativePayload::Array(v) => {
                            if dst_pos < v.len() && count > 0 {
                                let dst_end = dst_pos.saturating_add(count).min(v.len());
                                let write_count = dst_end.saturating_sub(dst_pos);
                                let copy = &src_vals[src_pos..src_pos + write_count];
                                v[dst_pos..dst_pos + write_count].clone_from_slice(copy);
                            }
                        }
                        NativePayload::ByteArray(v) => {
                            if dst_pos < v.len() && count > 0 {
                                let dst_end = dst_pos.saturating_add(count).min(v.len());
                                let write_count = dst_end.saturating_sub(dst_pos);
                                for i in 0..write_count {
                                    v[dst_pos + i] = src_vals[src_pos + i].as_int() as u8;
                                }
                            }
                        }
                        NativePayload::IntArray(v) => {
                            if dst_pos < v.len() && count > 0 {
                                let dst_end = dst_pos.saturating_add(count).min(v.len());
                                let write_count = dst_end.saturating_sub(dst_pos);
                                for i in 0..write_count {
                                    v[dst_pos + i] = src_vals[src_pos + i].as_int();
                                }
                            }
                        }
                        NativePayload::LongArray(v) => {
                            if dst_pos < v.len() && count > 0 {
                                let dst_end = dst_pos.saturating_add(count).min(v.len());
                                let write_count = dst_end.saturating_sub(dst_pos);
                                for i in 0..write_count {
                                    v[dst_pos + i] = src_vals[src_pos + i].as_long();
                                }
                            }
                        }
                        _ => {}
                    }
                }
                Some(JValue::Void)
            }
            ("java/lang/reflect/Array", "newInstance", "(Ljava/lang/Class;I)Ljava/lang/Object;") => {
                let component = _args
                    .first()
                    .and_then(|v| v.as_ref())
                    .and_then(|c| self.class_internal_name_from_obj(c))
                    .unwrap_or_else(|| "java/lang/Object".to_owned());
                let len = _args.get(1).map(|v| v.as_int().max(0) as usize).unwrap_or(0);
                let arr = match component.as_str() {
                    "boolean" => JObject::new_array("[Z", vec![JValue::Int(0); len]),
                    "byte" => JObject::new_array("[B", vec![JValue::Int(0); len]),
                    "char" => JObject::new_array("[C", vec![JValue::Int(0); len]),
                    "short" => JObject::new_array("[S", vec![JValue::Int(0); len]),
                    "int" => JObject::new_array("[I", vec![JValue::Int(0); len]),
                    "long" => JObject::new_array("[J", vec![JValue::Long(0); len]),
                    "float" => JObject::new_array("[F", vec![JValue::Float(0.0); len]),
                    "double" => JObject::new_array("[D", vec![JValue::Double(0.0); len]),
                    _ if component.starts_with('[') => {
                        JObject::new_array(format!("[{component}"), vec![JValue::Ref(None); len])
                    }
                    _ => JObject::new_array(format!("[L{component};"), vec![JValue::Ref(None); len]),
                };
                Some(JValue::Ref(Some(arr)))
            }
            ("java/lang/reflect/Array", "newInstance", "(Ljava/lang/Class;[I)Ljava/lang/Object;") => {
                let component = _args
                    .first()
                    .and_then(|v| v.as_ref())
                    .and_then(|c| self.class_internal_name_from_obj(c))
                    .unwrap_or_else(|| "java/lang/Object".to_owned());
                let dims = _args
                    .get(1)
                    .and_then(|v| v.as_ref())
                    .and_then(|r| match &r.borrow().native {
                        NativePayload::Array(v) => Some(v.iter().map(|x| x.as_int().max(0) as usize).collect::<Vec<_>>()),
                        NativePayload::IntArray(v) => Some(v.iter().map(|x| (*x).max(0) as usize).collect::<Vec<_>>()),
                        _ => None,
                    })
                    .unwrap_or_default();
                if dims.is_empty() {
                    return Some(JValue::Ref(None));
                }
                let base_desc = match component.as_str() {
                    "boolean" => "Z".to_owned(),
                    "byte" => "B".to_owned(),
                    "char" => "C".to_owned(),
                    "short" => "S".to_owned(),
                    "int" => "I".to_owned(),
                    "long" => "J".to_owned(),
                    "float" => "F".to_owned(),
                    "double" => "D".to_owned(),
                    _ if component.starts_with('[') => component,
                    _ => format!("L{component};"),
                };
                let desc = format!("{}{}", "[".repeat(dims.len()), base_desc);
                Some(JValue::Ref(Some(self.create_multi_array(&desc, &dims, 0))))
            }
            ("java/lang/reflect/Array", "getLength", "(Ljava/lang/Object;)I") => {
                let len = _args
                    .first()
                    .and_then(|v| v.as_ref())
                    .and_then(|r| match &r.borrow().native {
                        NativePayload::Array(v) => Some(v.len() as i32),
                        NativePayload::ByteArray(v) => Some(v.len() as i32),
                        NativePayload::IntArray(v) => Some(v.len() as i32),
                        NativePayload::LongArray(v) => Some(v.len() as i32),
                        _ => None,
                    })
                    .unwrap_or(0);
                Some(JValue::Int(len))
            }
            ("java/lang/reflect/Array", "get", "(Ljava/lang/Object;I)Ljava/lang/Object;") => {
                let idx = _args.get(1).map(|v| v.as_int().max(0) as usize).unwrap_or(0);
                let v = _args
                    .first()
                    .and_then(|x| x.as_ref())
                    .and_then(|r| match &r.borrow().native {
                        NativePayload::Array(v) => v.get(idx).cloned(),
                        NativePayload::ByteArray(v) => v.get(idx).map(|x| JValue::Int(*x as i32)),
                        NativePayload::IntArray(v) => v.get(idx).map(|x| JValue::Int(*x)),
                        NativePayload::LongArray(v) => v.get(idx).map(|x| JValue::Long(*x)),
                        _ => None,
                    })
                    .unwrap_or(JValue::Ref(None));
                Some(self.wrap_primitive_value(v))
            }
            ("java/lang/reflect/Array", "set", "(Ljava/lang/Object;ILjava/lang/Object;)V") => {
                let idx = _args.get(1).map(|v| v.as_int().max(0) as usize).unwrap_or(0);
                let value = _args.get(2).cloned().unwrap_or(JValue::Ref(None));
                if let Some(r) = _args.first().and_then(|x| x.as_ref()) {
                    let mut arr = r.borrow_mut();
                    match &mut arr.native {
                        NativePayload::Array(v) => {
                            if idx < v.len() {
                                v[idx] = value;
                            }
                        }
                        NativePayload::ByteArray(v) => {
                            if idx < v.len() {
                                let iv = self.adapt_value_for_descriptor("B", value).as_int();
                                v[idx] = iv as u8;
                            }
                        }
                        NativePayload::IntArray(v) => {
                            if idx < v.len() {
                                let iv = self.adapt_value_for_descriptor("I", value).as_int();
                                v[idx] = iv;
                            }
                        }
                        NativePayload::LongArray(v) => {
                            if idx < v.len() {
                                let lv = self.adapt_value_for_descriptor("J", value).as_long();
                                v[idx] = lv;
                            }
                        }
                        _ => {}
                    }
                }
                Some(JValue::Void)
            }
            _ => None,
        }
    }

    /// Handle instance native methods. Returns `None` if not a known native.
    /// Extract a Rust String from String constructor arguments.
    fn string_from_init_args(&self, descriptor: &str, args: &[JValue], _this: &JRef) -> String {
        match descriptor {
            "()V" => String::new(),
            "([C)V" => {
                // String(char[])
                if let Some(r) = args.first().and_then(|a| a.as_ref()) {
                    if let NativePayload::Array(chars) = &r.borrow().native {
                        chars.iter().map(|v| char::from(v.as_int() as u8 as u32 as u8)).collect()
                    } else { String::new() }
                } else { String::new() }
            }
            "([CII)V" => {
                // String(char[], offset, count)
                if let Some(r) = args.first().and_then(|a| a.as_ref()) {
                    let offset = args.get(1).map(|a| a.as_int() as usize).unwrap_or(0);
                    let count = args.get(2).map(|a| a.as_int() as usize).unwrap_or(0);
                    if let NativePayload::Array(chars) = &r.borrow().native {
                        chars[offset..offset + count].iter()
                            .map(|v| {
                                let code = v.as_int() as u32;
                                char::from_u32(code).unwrap_or('?')
                            })
                            .collect()
                    } else { String::new() }
                } else { String::new() }
            }
            "([B)V" => {
                // String(byte[])
                if let Some(r) = args.first().and_then(|a| a.as_ref()) {
                    if let NativePayload::Array(bytes) = &r.borrow().native {
                        bytes.iter().map(|v| v.as_int() as u8 as char).collect()
                    } else { String::new() }
                } else { String::new() }
            }
            "([BII)V" | "([BIILjava/lang/String;)V" | "([BIILjava/nio/charset/Charset;)V" => {
                if let Some(r) = args.first().and_then(|a| a.as_ref()) {
                    let offset = args.get(1).map(|a| a.as_int().max(0) as usize).unwrap_or(0);
                    let count = args.get(2).map(|a| a.as_int().max(0) as usize).unwrap_or(0);
                    if let NativePayload::Array(bytes) = &r.borrow().native {
                        let end = offset.saturating_add(count).min(bytes.len());
                        bytes[offset.min(bytes.len())..end]
                            .iter()
                            .map(|v| v.as_int() as u8 as char)
                            .collect()
                    } else { String::new() }
                } else { String::new() }
            }
            "([BLjava/lang/String;)V" | "([BLjava/nio/charset/Charset;)V" => {
                if let Some(r) = args.first().and_then(|a| a.as_ref()) {
                    if let NativePayload::Array(bytes) = &r.borrow().native {
                        bytes.iter().map(|v| v.as_int() as u8 as char).collect()
                    } else { String::new() }
                } else { String::new() }
            }
            "([BIII)V" => {
                if let Some(r) = args.first().and_then(|a| a.as_ref()) {
                    let _hibyte = args.get(1).map(|a| a.as_int()).unwrap_or(0);
                    let offset = args.get(2).map(|a| a.as_int().max(0) as usize).unwrap_or(0);
                    let count = args.get(3).map(|a| a.as_int().max(0) as usize).unwrap_or(0);
                    if let NativePayload::Array(bytes) = &r.borrow().native {
                        let end = offset.saturating_add(count).min(bytes.len());
                        bytes[offset.min(bytes.len())..end]
                            .iter()
                            .map(|v| v.as_int() as u8 as char)
                            .collect()
                    } else { String::new() }
                } else { String::new() }
            }
            "(Ljava/lang/String;)V" => {
                // String(String) — copy constructor
                if let Some(r) = args.first().and_then(|a| a.as_ref()) {
                    r.borrow().as_java_string().unwrap_or("").to_owned()
                } else { String::new() }
            }
            _ => String::new(),
        }
    }

    fn printstream_text_for(&mut self, value: &JValue) -> String {
        match value {
            JValue::Void => "void".to_owned(),
            JValue::Int(i) => i.to_string(),
            JValue::Long(l) => l.to_string(),
            JValue::Float(f) => f.to_string(),
            JValue::Double(d) => d.to_string(),
            JValue::Ref(None) => "null".to_owned(),
            JValue::Ref(Some(r)) => {
                if let Some(s) = r.borrow().as_java_string() {
                    return s.to_owned();
                }
                let class_name = r.borrow().class_name.clone();
                match self.invoke_virtual(r.clone(), &class_name, "toString", "()Ljava/lang/String;", vec![]) {
                    Ok(JValue::Ref(Some(sref))) => sref.borrow().as_java_string().unwrap_or("").to_owned(),
                    _ => format!("{class_name}@obj"),
                }
            }
            JValue::ReturnAddress(a) => format!("ret:{a}"),
        }
    }

    fn emit_host_line(is_err: bool, line: &str) {
        #[cfg(target_arch = "wasm32")]
        {
            if is_err {
                console_error(line);
            } else {
                console_log(line);
            }
        }
        #[cfg(not(target_arch = "wasm32"))]
        {
            if is_err {
                eprintln!("{line}");
            } else {
                println!("{line}");
            }
        }
    }

    fn write_printstream(&mut self, is_err: bool, text: &str, newline: bool) {
        let buf = if is_err {
            &mut self.stderr_buffer
        } else {
            &mut self.stdout_buffer
        };
        buf.push_str(text);
        while let Some(pos) = buf.find('\n') {
            let line = buf[..pos].to_owned();
            Self::emit_host_line(is_err, &line);
            buf.drain(..=pos);
        }
        if newline {
            Self::emit_host_line(is_err, buf);
            buf.clear();
        }
    }

    fn class_internal_name_from_obj(&self, class_obj: &JRef) -> Option<String> {
        class_obj
            .borrow()
            .fields
            .get("__name_internal")
            .and_then(|v| v.as_ref())
            .and_then(|r| r.borrow().as_java_string().map(|s| s.to_owned()))
    }

    fn class_display_name(internal_name: &str) -> String {
        if internal_name.starts_with('[') {
            internal_name.replace('/', ".")
        } else {
            internal_name.replace('/', ".")
        }
    }

    fn class_internal_name_from_runtime_name(name: &str) -> String {
        if name.starts_with('[') {
            name.replace('.', "/")
        } else {
            name.replace('.', "/")
        }
    }

    fn descriptor_to_runtime_class_name(desc: &str) -> String {
        match desc.as_bytes().first().copied() {
            Some(b'B') => "byte".to_owned(),
            Some(b'C') => "char".to_owned(),
            Some(b'D') => "double".to_owned(),
            Some(b'F') => "float".to_owned(),
            Some(b'I') => "int".to_owned(),
            Some(b'J') => "long".to_owned(),
            Some(b'S') => "short".to_owned(),
            Some(b'Z') => "boolean".to_owned(),
            Some(b'V') => "void".to_owned(),
            Some(b'L') => desc
                .strip_prefix('L')
                .and_then(|s| s.strip_suffix(';'))
                .unwrap_or("java/lang/Object")
                .to_owned(),
            Some(b'[') => desc.to_owned(),
            _ => "java/lang/Object".to_owned(),
        }
    }

    fn parse_method_descriptor(desc: &str) -> (Vec<String>, String) {
        let mut params = Vec::new();
        let bytes = desc.as_bytes();
        let mut i = 0usize;
        if bytes.get(i) != Some(&b'(') {
            return (params, "void".to_owned());
        }
        i += 1;
        while let Some(&b) = bytes.get(i) {
            if b == b')' {
                i += 1;
                break;
            }
            let start = i;
            while bytes.get(i) == Some(&b'[') {
                i += 1;
            }
            match bytes.get(i).copied() {
                Some(b'L') => {
                    i += 1;
                    while bytes.get(i) != Some(&b';') && i < bytes.len() {
                        i += 1;
                    }
                    if i < bytes.len() {
                        i += 1;
                    }
                }
                Some(_) => i += 1,
                None => break,
            }
            let token = &desc[start..i];
            params.push(Self::descriptor_to_runtime_class_name(token));
        }
        let ret = if i <= desc.len() {
            Self::descriptor_to_runtime_class_name(&desc[i..])
        } else {
            "void".to_owned()
        };
        (params, ret)
    }

    fn parse_method_descriptor_tokens(desc: &str) -> (Vec<String>, String) {
        let mut params = Vec::new();
        let bytes = desc.as_bytes();
        let mut i = 0usize;
        if bytes.get(i) != Some(&b'(') {
            return (params, "V".to_owned());
        }
        i += 1;
        while let Some(&b) = bytes.get(i) {
            if b == b')' {
                i += 1;
                break;
            }
            let start = i;
            while bytes.get(i) == Some(&b'[') {
                i += 1;
            }
            match bytes.get(i).copied() {
                Some(b'L') => {
                    i += 1;
                    while bytes.get(i) != Some(&b';') && i < bytes.len() {
                        i += 1;
                    }
                    if i < bytes.len() {
                        i += 1;
                    }
                }
                Some(_) => i += 1,
                None => break,
            }
            params.push(desc[start..i].to_owned());
        }
        let ret = if i <= desc.len() { desc[i..].to_owned() } else { "V".to_owned() };
        (params, ret)
    }

    fn wrap_primitive_value(&self, value: JValue) -> JValue {
        match value {
            JValue::Int(i) => {
                let obj = JObject::new("java/lang/Integer");
                obj.borrow_mut().fields.insert("value".to_owned(), JValue::Int(i));
                JValue::Ref(Some(obj))
            }
            JValue::Long(l) => {
                let obj = JObject::new("java/lang/Long");
                obj.borrow_mut().fields.insert("value".to_owned(), JValue::Long(l));
                JValue::Ref(Some(obj))
            }
            JValue::Float(f) => {
                let obj = JObject::new("java/lang/Float");
                obj.borrow_mut().fields.insert("value".to_owned(), JValue::Float(f));
                JValue::Ref(Some(obj))
            }
            JValue::Double(d) => {
                let obj = JObject::new("java/lang/Double");
                obj.borrow_mut().fields.insert("value".to_owned(), JValue::Double(d));
                JValue::Ref(Some(obj))
            }
            other => other,
        }
    }

    fn unwrap_boxed_primitive(&self, value: &JValue) -> Option<JValue> {
        let r = value.as_ref()?;
        let obj = r.borrow();
        match obj.class_name.as_str() {
            "java/lang/Integer" | "java/lang/Byte" | "java/lang/Short" | "java/lang/Character" => {
                obj.fields.get("value").cloned().map(|v| match v {
                    JValue::Int(i) => JValue::Int(i),
                    _ => JValue::Int(0),
                })
            }
            "java/lang/Boolean" => {
                obj.fields.get("value").cloned().map(|v| match v {
                    JValue::Int(i) => JValue::Int(if i == 0 { 0 } else { 1 }),
                    _ => JValue::Int(0),
                })
            }
            "java/lang/Long" => obj.fields.get("value").cloned().map(|v| match v {
                JValue::Long(l) => JValue::Long(l),
                JValue::Int(i) => JValue::Long(i as i64),
                _ => JValue::Long(0),
            }),
            "java/lang/Float" => obj.fields.get("value").cloned().map(|v| match v {
                JValue::Float(f) => JValue::Float(f),
                JValue::Double(d) => JValue::Float(d as f32),
                JValue::Int(i) => JValue::Float(i as f32),
                _ => JValue::Float(0.0),
            }),
            "java/lang/Double" => obj.fields.get("value").cloned().map(|v| match v {
                JValue::Double(d) => JValue::Double(d),
                JValue::Float(f) => JValue::Double(f as f64),
                JValue::Int(i) => JValue::Double(i as f64),
                _ => JValue::Double(0.0),
            }),
            _ => None,
        }
    }

    fn adapt_value_for_descriptor(&self, desc: &str, value: JValue) -> JValue {
        match desc.as_bytes().first().copied() {
            Some(b'Z') | Some(b'B') | Some(b'C') | Some(b'S') | Some(b'I') => {
                match value {
                    JValue::Int(i) => JValue::Int(i),
                    JValue::Long(l) => JValue::Int(l as i32),
                    JValue::Float(f) => JValue::Int(f as i32),
                    JValue::Double(d) => JValue::Int(d as i32),
                    JValue::Ref(_) => self.unwrap_boxed_primitive(&value).map(|v| self.adapt_value_for_descriptor("I", v)).unwrap_or(JValue::Int(0)),
                    _ => JValue::Int(0),
                }
            }
            Some(b'J') => match value {
                JValue::Long(l) => JValue::Long(l),
                JValue::Int(i) => JValue::Long(i as i64),
                JValue::Ref(_) => self.unwrap_boxed_primitive(&value).map(|v| self.adapt_value_for_descriptor("J", v)).unwrap_or(JValue::Long(0)),
                _ => JValue::Long(0),
            },
            Some(b'F') => match value {
                JValue::Float(f) => JValue::Float(f),
                JValue::Double(d) => JValue::Float(d as f32),
                JValue::Int(i) => JValue::Float(i as f32),
                JValue::Long(l) => JValue::Float(l as f32),
                JValue::Ref(_) => self.unwrap_boxed_primitive(&value).map(|v| self.adapt_value_for_descriptor("F", v)).unwrap_or(JValue::Float(0.0)),
                _ => JValue::Float(0.0),
            },
            Some(b'D') => match value {
                JValue::Double(d) => JValue::Double(d),
                JValue::Float(f) => JValue::Double(f as f64),
                JValue::Int(i) => JValue::Double(i as f64),
                JValue::Long(l) => JValue::Double(l as f64),
                JValue::Ref(_) => self.unwrap_boxed_primitive(&value).map(|v| self.adapt_value_for_descriptor("D", v)).unwrap_or(JValue::Double(0.0)),
                _ => JValue::Double(0.0),
            },
            Some(b'L') | Some(b'[') => match value {
                JValue::Ref(_) => value,
                primitive => self.wrap_primitive_value(primitive),
            },
            _ => value,
        }
    }

    fn collect_reflection_args(&self, args_array: Option<&JRef>) -> Vec<JValue> {
        if let Some(arr) = args_array {
            if let NativePayload::Array(v) = &arr.borrow().native {
                return v.clone();
            }
        }
        Vec::new()
    }

    fn raise_invocation_target_exception(&mut self, message: &str) {
        let cause = self.pending_exception.take().unwrap_or_else(|| {
            let exc = JObject::new("java/lang/RuntimeException");
            exc.borrow_mut().fields.insert(
                "detailMessage".to_owned(),
                JValue::Ref(Some(JObject::new_string(message.to_owned()))),
            );
            exc
        });
        let ite = JObject::new("java/lang/reflect/InvocationTargetException");
        ite.borrow_mut().fields.insert("target".to_owned(), JValue::Ref(Some(cause)));
        self.pending_exception = Some(ite);
    }

    fn make_class_array(&mut self, class_names: Vec<String>) -> JRef {
        let values = class_names
            .into_iter()
            .map(|n| JValue::Ref(Some(self.class_object(n))))
            .collect();
        JObject::new_array("[Ljava/lang/Class;", values)
    }

    fn build_reflect_field(&mut self, owner: &str, name: &str, desc: &str, access_flags: u16) -> JRef {
        let obj = JObject::new("java/lang/reflect/Field");
        {
            let mut o = obj.borrow_mut();
            o.fields.insert("clazz".to_owned(), JValue::Ref(Some(self.class_object(owner.to_owned()))));
            o.fields.insert("name".to_owned(), JValue::Ref(Some(self.intern_string(name.to_owned()))));
            o.fields.insert("__descriptor".to_owned(), JValue::Ref(Some(self.intern_string(desc.to_owned()))));
            o.fields.insert(
                "type".to_owned(),
                JValue::Ref(Some(self.class_object(Self::descriptor_to_runtime_class_name(desc)))),
            );
            o.fields.insert("modifiers".to_owned(), JValue::Int(i32::from(access_flags)));
        }
        obj
    }

    fn build_reflect_method(
        &mut self,
        owner: &str,
        name: &str,
        method_desc: &str,
        access_flags: u16,
        exceptions: Vec<String>,
    ) -> JRef {
        let (param_names, return_name) = Self::parse_method_descriptor(method_desc);
        let param_array = self.make_class_array(param_names);
        let ex_array = self.make_class_array(exceptions);
        let obj = JObject::new("java/lang/reflect/Method");
        {
            let mut o = obj.borrow_mut();
            o.fields.insert("clazz".to_owned(), JValue::Ref(Some(self.class_object(owner.to_owned()))));
            o.fields.insert("name".to_owned(), JValue::Ref(Some(self.intern_string(name.to_owned()))));
            o.fields.insert("__descriptor".to_owned(), JValue::Ref(Some(self.intern_string(method_desc.to_owned()))));
            o.fields.insert(
                "returnType".to_owned(),
                JValue::Ref(Some(self.class_object(return_name))),
            );
            o.fields
                .insert("parameterTypes".to_owned(), JValue::Ref(Some(param_array)));
            o.fields
                .insert("exceptionTypes".to_owned(), JValue::Ref(Some(ex_array)));
            o.fields.insert("modifiers".to_owned(), JValue::Int(i32::from(access_flags)));
        }
        obj
    }

    fn build_reflect_constructor(
        &mut self,
        owner: &str,
        method_desc: &str,
        access_flags: u16,
        exceptions: Vec<String>,
    ) -> JRef {
        let (param_names, _) = Self::parse_method_descriptor(method_desc);
        let param_array = self.make_class_array(param_names);
        let ex_array = self.make_class_array(exceptions);
        let obj = JObject::new("java/lang/reflect/Constructor");
        {
            let mut o = obj.borrow_mut();
            o.fields.insert("clazz".to_owned(), JValue::Ref(Some(self.class_object(owner.to_owned()))));
            o.fields.insert("__descriptor".to_owned(), JValue::Ref(Some(self.intern_string(method_desc.to_owned()))));
            o.fields
                .insert("parameterTypes".to_owned(), JValue::Ref(Some(param_array)));
            o.fields
                .insert("exceptionTypes".to_owned(), JValue::Ref(Some(ex_array)));
            o.fields.insert("modifiers".to_owned(), JValue::Int(i32::from(access_flags)));
        }
        obj
    }

    fn build_reflect_record_component(&mut self, owner: &str, name: &str, desc: &str) -> JRef {
        let obj = JObject::new("java/lang/reflect/RecordComponent");
        {
            let mut o = obj.borrow_mut();
            o.fields.insert("clazz".to_owned(), JValue::Ref(Some(self.class_object(owner.to_owned()))));
            o.fields.insert("name".to_owned(), JValue::Ref(Some(self.intern_string(name.to_owned()))));
            o.fields.insert("__descriptor".to_owned(), JValue::Ref(Some(self.intern_string(desc.to_owned()))));
            o.fields.insert(
                "type".to_owned(),
                JValue::Ref(Some(self.class_object(Self::descriptor_to_runtime_class_name(desc)))),
            );
        }
        obj
    }

    fn read_u8(data: &[u8], p: &mut usize) -> Option<u8> {
        let b = *data.get(*p)?;
        *p += 1;
        Some(b)
    }

    fn read_u16(data: &[u8], p: &mut usize) -> Option<u16> {
        let hi = *data.get(*p)? as u16;
        let lo = *data.get(*p + 1)? as u16;
        *p += 2;
        Some((hi << 8) | lo)
    }

    fn skip_annotation_element_value(data: &[u8], p: &mut usize) -> Option<()> {
        let tag = Self::read_u8(data, p)?;
        match tag as char {
            'B' | 'C' | 'D' | 'F' | 'I' | 'J' | 'S' | 'Z' | 's' => {
                let _ = Self::read_u16(data, p)?;
            }
            'e' => {
                let _ = Self::read_u16(data, p)?;
                let _ = Self::read_u16(data, p)?;
            }
            'c' => {
                let _ = Self::read_u16(data, p)?;
            }
            '@' => {
                Self::skip_annotation(data, p)?;
            }
            '[' => {
                let n = Self::read_u16(data, p)? as usize;
                for _ in 0..n {
                    Self::skip_annotation_element_value(data, p)?;
                }
            }
            _ => return None,
        }
        Some(())
    }

    fn skip_annotation(data: &[u8], p: &mut usize) -> Option<()> {
        let _type_index = Self::read_u16(data, p)?;
        let pairs = Self::read_u16(data, p)? as usize;
        for _ in 0..pairs {
            let _name_index = Self::read_u16(data, p)?;
            Self::skip_annotation_element_value(data, p)?;
        }
        Some(())
    }

    fn parse_runtime_visible_annotation_types(
        attrs: &[Attribute],
        cp: &crate::class_file::ConstantPool,
    ) -> Vec<String> {
        let mut types = Vec::new();
        for attr in attrs {
            let (name, data) = match attr {
                Attribute::Unknown { name, data } => (name, data),
                _ => continue,
            };
            if name != "RuntimeVisibleAnnotations" {
                continue;
            }
            let mut p = 0usize;
            let n = match Self::read_u16(data, &mut p) {
                Some(v) => v as usize,
                None => continue,
            };
            for _ in 0..n {
                let type_index = match Self::read_u16(data, &mut p) {
                    Some(v) => v,
                    None => break,
                };
                let desc = cp.utf8(type_index).to_owned();
                if let Some(inner) = desc.strip_prefix('L').and_then(|s| s.strip_suffix(';')) {
                    types.push(inner.to_owned());
                }
                let pairs = match Self::read_u16(data, &mut p) {
                    Some(v) => v as usize,
                    None => break,
                };
                for _ in 0..pairs {
                    if Self::read_u16(data, &mut p).is_none() {
                        break;
                    }
                    if Self::skip_annotation_element_value(data, &mut p).is_none() {
                        break;
                    }
                }
            }
        }
        types
    }

    fn cp_const_to_jvalue(&mut self, cp: &crate::class_file::ConstantPool, idx: u16) -> Option<JValue> {
        match cp.get(idx) {
            ConstantPoolEntry::Integer(v) => Some(JValue::Int(*v)),
            ConstantPoolEntry::Long(v) => Some(JValue::Long(*v)),
            ConstantPoolEntry::Float(v) => Some(JValue::Float(*v)),
            ConstantPoolEntry::Double(v) => Some(JValue::Double(*v)),
            ConstantPoolEntry::String { string_index } => {
                let s = cp.utf8(*string_index).to_owned();
                Some(JValue::Ref(Some(self.intern_string(s))))
            }
            ConstantPoolEntry::Utf8(s) => Some(JValue::Ref(Some(self.intern_string(s.clone())))),
            _ => None,
        }
    }

    fn parse_annotation_element_value(
        &mut self,
        data: &[u8],
        p: &mut usize,
        cp: &crate::class_file::ConstantPool,
    ) -> Option<JValue> {
        let tag = Self::read_u8(data, p)? as char;
        match tag {
            'B' | 'C' | 'D' | 'F' | 'I' | 'J' | 'S' | 'Z' | 's' => {
                let const_idx = Self::read_u16(data, p)?;
                self.cp_const_to_jvalue(cp, const_idx)
            }
            'e' => {
                let type_name_index = Self::read_u16(data, p)?;
                let const_name_index = Self::read_u16(data, p)?;
                let t = cp.utf8(type_name_index);
                let c = cp.utf8(const_name_index);
                Some(JValue::Ref(Some(self.intern_string(format!("{t}.{c}")))))
            }
            'c' => {
                let class_info_index = Self::read_u16(data, p)?;
                let desc = cp.utf8(class_info_index);
                Some(JValue::Ref(Some(self.class_object(Self::descriptor_to_runtime_class_name(desc)))))
            }
            '@' => {
                let ann = self.parse_one_runtime_visible_annotation(data, p, cp)?;
                Some(JValue::Ref(Some(ann)))
            }
            '[' => {
                let n = Self::read_u16(data, p)? as usize;
                let mut vals = Vec::with_capacity(n);
                for _ in 0..n {
                    vals.push(self.parse_annotation_element_value(data, p, cp).unwrap_or(JValue::Ref(None)));
                }
                Some(JValue::Ref(Some(JObject::new_array("[Ljava/lang/Object;", vals))))
            }
            _ => None,
        }
    }

    fn parse_one_runtime_visible_annotation(
        &mut self,
        data: &[u8],
        p: &mut usize,
        cp: &crate::class_file::ConstantPool,
    ) -> Option<JRef> {
        let type_index = Self::read_u16(data, p)?;
        let desc = cp.utf8(type_index).to_owned();
        let ann_class = desc.strip_prefix('L')?.strip_suffix(';')?.to_owned();
        let pairs = Self::read_u16(data, p)? as usize;
        let ann_obj = JObject::new(ann_class.clone());
        {
            let mut o = ann_obj.borrow_mut();
            o.fields.insert(
                "__ann_type".to_owned(),
                JValue::Ref(Some(self.class_object(ann_class))),
            );
        }
        for _ in 0..pairs {
            let name_index = Self::read_u16(data, p)?;
            let name = cp.utf8(name_index).to_owned();
            let value = self
                .parse_annotation_element_value(data, p, cp)
                .unwrap_or(JValue::Ref(None));
            ann_obj
                .borrow_mut()
                .fields
                .insert(format!("__ann_{name}"), value);
        }
        Some(ann_obj)
    }

    fn parse_runtime_visible_annotations(
        &mut self,
        attrs: &[Attribute],
        cp: &crate::class_file::ConstantPool,
    ) -> Vec<JRef> {
        let mut anns = Vec::new();
        for attr in attrs {
            let (name, data) = match attr {
                Attribute::Unknown { name, data } => (name, data),
                _ => continue,
            };
            if name != "RuntimeVisibleAnnotations" {
                continue;
            }
            let mut p = 0usize;
            let n = match Self::read_u16(data, &mut p) {
                Some(v) => v as usize,
                None => continue,
            };
            for _ in 0..n {
                if let Some(ann) = self.parse_one_runtime_visible_annotation(data, &mut p, cp) {
                    anns.push(ann);
                } else {
                    break;
                }
            }
        }
        anns
    }

    fn parse_runtime_visible_parameter_annotation_types(
        attrs: &[Attribute],
        cp: &crate::class_file::ConstantPool,
        param_count: usize,
    ) -> Vec<Vec<String>> {
        let mut out = vec![Vec::new(); param_count];
        for attr in attrs {
            let (name, data) = match attr {
                Attribute::Unknown { name, data } => (name, data),
                _ => continue,
            };
            if name != "RuntimeVisibleParameterAnnotations" {
                continue;
            }
            let mut p = 0usize;
            let declared = match Self::read_u8(data, &mut p) {
                Some(v) => v as usize,
                None => continue,
            };
            for i in 0..declared {
                let n = match Self::read_u16(data, &mut p) {
                    Some(v) => v as usize,
                    None => break,
                };
                let mut ann_types = Vec::new();
                for _ in 0..n {
                    let type_index = match Self::read_u16(data, &mut p) {
                        Some(v) => v,
                        None => break,
                    };
                    let desc = cp.utf8(type_index).to_owned();
                    if let Some(inner) = desc.strip_prefix('L').and_then(|s| s.strip_suffix(';')) {
                        ann_types.push(inner.to_owned());
                    }
                    let pairs = match Self::read_u16(data, &mut p) {
                        Some(v) => v as usize,
                        None => break,
                    };
                    for _ in 0..pairs {
                        if Self::read_u16(data, &mut p).is_none() {
                            break;
                        }
                        if Self::skip_annotation_element_value(data, &mut p).is_none() {
                            break;
                        }
                    }
                }
                if i < out.len() {
                    out[i] = ann_types;
                }
            }
        }
        out
    }

    fn parse_runtime_visible_parameter_annotations(
        &mut self,
        attrs: &[Attribute],
        cp: &crate::class_file::ConstantPool,
        param_count: usize,
    ) -> Vec<Vec<JRef>> {
        let mut out = vec![Vec::new(); param_count];
        for attr in attrs {
            let (name, data) = match attr {
                Attribute::Unknown { name, data } => (name, data),
                _ => continue,
            };
            if name != "RuntimeVisibleParameterAnnotations" {
                continue;
            }
            let mut p = 0usize;
            let declared = match Self::read_u8(data, &mut p) {
                Some(v) => v as usize,
                None => continue,
            };
            for i in 0..declared {
                let n = match Self::read_u16(data, &mut p) {
                    Some(v) => v as usize,
                    None => break,
                };
                let mut anns = Vec::new();
                for _ in 0..n {
                    if let Some(ann) = self.parse_one_runtime_visible_annotation(data, &mut p, cp) {
                        anns.push(ann);
                    } else {
                        break;
                    }
                }
                if i < out.len() {
                    out[i] = anns;
                }
            }
        }
        out
    }

    fn build_annotation_array(&self, annotation_types: Vec<String>) -> JValue {
        let vals = annotation_types
            .into_iter()
            .map(|ann| JValue::Ref(Some(JObject::new(ann))))
            .collect();
        JValue::Ref(Some(JObject::new_array(
            "[Ljava/lang/annotation/Annotation;",
            vals,
        )))
    }

    fn build_annotation_ref_array(&self, annotation_refs: Vec<JRef>) -> JValue {
        let vals = annotation_refs
            .into_iter()
            .map(|ann| JValue::Ref(Some(ann)))
            .collect();
        JValue::Ref(Some(JObject::new_array(
            "[Ljava/lang/annotation/Annotation;",
            vals,
        )))
    }

    fn native_virtual(
        &mut self,
        this: &JRef,
        _class_name: &str,
        method_name: &str,
        _descriptor: &str,
        _args: &[JValue],
    ) -> Option<JValue> {
        if _class_name == "java/lang/Object" {
            match method_name {
                "hashCode" => {
                    let ptr = Rc::as_ptr(this) as usize;
                    return Some(JValue::Int((ptr as u64 as u32) as i32));
                }
                "getClass" => {
                    let runtime_class = this.borrow().class_name.clone();
                    return Some(JValue::Ref(Some(self.class_object(runtime_class))));
                }
                _ => {}
            }
        }
        let cn = this.borrow().class_name.clone();
        match (cn.as_str(), method_name) {
            ("java/lang/Class", "getName") => {
                let internal = self
                    .class_internal_name_from_obj(this)
                    .unwrap_or_else(|| "java/lang/Object".to_owned());
                Some(JValue::Ref(Some(self.intern_string(Self::class_display_name(&internal)))))
            }
            ("java/lang/Class", "getModifiers") => {
                let target = self
                    .class_internal_name_from_obj(this)
                    .unwrap_or_else(|| "java/lang/Object".to_owned());
                let mods = self
                    .classes
                    .get(&target)
                    .map(|cf| i32::from(cf.access_flags))
                    .unwrap_or(0);
                Some(JValue::Int(mods))
            }
            ("java/lang/Class", "isInstance") => {
                let target = self
                    .class_internal_name_from_obj(this)
                    .unwrap_or_else(|| "java/lang/Object".to_owned());
                let obj_class = _args
                    .first()
                    .and_then(|v| v.as_ref())
                    .map(|r| r.borrow().class_name.clone());
                let result = match obj_class {
                    Some(rc) => self.is_instance_of(&rc, &target),
                    None => false,
                };
                Some(JValue::Int(if result { 1 } else { 0 }))
            }
            ("java/lang/Class", "isAssignableFrom") => {
                let target = self
                    .class_internal_name_from_obj(this)
                    .unwrap_or_else(|| "java/lang/Object".to_owned());
                let other = _args
                    .first()
                    .and_then(|v| v.as_ref())
                    .and_then(|c| self.class_internal_name_from_obj(c));
                let result = other
                    .as_ref()
                    .map(|o| self.is_instance_of(o, &target))
                    .unwrap_or(false);
                Some(JValue::Int(if result { 1 } else { 0 }))
            }
            ("java/lang/Class", "isInterface") => {
                let target = self
                    .class_internal_name_from_obj(this)
                    .unwrap_or_else(|| "java/lang/Object".to_owned());
                let is_iface = self
                    .classes
                    .get(&target)
                    .map(|cf| (cf.access_flags & 0x0200) != 0)
                    .unwrap_or(false);
                Some(JValue::Int(if is_iface { 1 } else { 0 }))
            }
            ("java/lang/Class", "getComponentType") => {
                let target = self
                    .class_internal_name_from_obj(this)
                    .unwrap_or_else(|| "java/lang/Object".to_owned());
                if !target.starts_with('[') {
                    return Some(JValue::Ref(None));
                }
                let elem = &target[1..];
                let comp = match elem.as_bytes().first().copied() {
                    Some(b'B') => "byte".to_owned(),
                    Some(b'C') => "char".to_owned(),
                    Some(b'D') => "double".to_owned(),
                    Some(b'F') => "float".to_owned(),
                    Some(b'I') => "int".to_owned(),
                    Some(b'J') => "long".to_owned(),
                    Some(b'S') => "short".to_owned(),
                    Some(b'Z') => "boolean".to_owned(),
                    Some(b'[') => elem.to_owned(),
                    Some(b'L') if elem.ends_with(';') => {
                        elem[1..elem.len() - 1].to_owned()
                    }
                    _ => "java/lang/Object".to_owned(),
                };
                Some(JValue::Ref(Some(self.class_object(comp))))
            }
            ("java/lang/Class", "getSuperclass") => {
                let target = self
                    .class_internal_name_from_obj(this)
                    .unwrap_or_else(|| "java/lang/Object".to_owned());
                let super_name = if target.starts_with('[') {
                    Some("java/lang/Object".to_owned())
                } else if let Some(cf) = self.classes.get(&target) {
                    if cf.super_class == 0 {
                        None
                    } else {
                        Some(cf.constant_pool.class_name(cf.super_class).to_owned())
                    }
                } else if matches!(target.as_str(), "byte" | "short" | "int" | "long" | "float" | "double" | "char" | "boolean" | "void") {
                    None
                } else {
                    Some("java/lang/Object".to_owned())
                };
                Some(JValue::Ref(super_name.map(|s| self.class_object(s))))
            }
            ("java/lang/Class", "getInterfaces") => {
                let target = self
                    .class_internal_name_from_obj(this)
                    .unwrap_or_else(|| "java/lang/Object".to_owned());
                let iface_names: Vec<String> = if target.starts_with('[') {
                    vec!["java/lang/Cloneable".to_owned(), "java/io/Serializable".to_owned()]
                } else if let Some(cf) = self.classes.get(&target) {
                    cf.interfaces
                        .iter()
                        .map(|idx| cf.constant_pool.class_name(*idx).to_owned())
                        .collect()
                } else {
                    Vec::new()
                };
                let vals = iface_names
                    .into_iter()
                    .map(|n| JValue::Ref(Some(self.class_object(n))))
                    .collect();
                Some(JValue::Ref(Some(JObject::new_array(
                    "[Ljava/lang/Class;",
                    vals,
                ))))
            }
            ("java/lang/Class", "getEnumConstants") => {
                let target = self
                    .class_internal_name_from_obj(this)
                    .unwrap_or_else(|| "java/lang/Object".to_owned());
                let _ = self.ensure_class_init(&target);
                let key = format!("{target}.$VALUES");
                if let Some(JValue::Ref(Some(arr))) = self.static_fields.get(&key).cloned() {
                    let cloned = match self.invoke_virtual(
                        arr.clone(),
                        "java/lang/Object",
                        "clone",
                        "()Ljava/lang/Object;",
                        vec![],
                    ) {
                        Ok(v) => v,
                        Err(_) => JValue::Ref(Some(arr)),
                    };
                    Some(cloned)
                } else {
                    Some(JValue::Ref(None))
                }
            }
            ("java/lang/Class", "isRecord") => {
                let target = self
                    .class_internal_name_from_obj(this)
                    .unwrap_or_else(|| "java/lang/Object".to_owned());
                let is_record = self
                    .classes
                    .get(&target)
                    .map(|cf| cf.attributes.iter().any(|a| matches!(a, Attribute::Record { .. })))
                    .unwrap_or(false);
                Some(JValue::Int(if is_record { 1 } else { 0 }))
            }
            ("java/lang/Class", "getRecordComponents") => {
                let target = self
                    .class_internal_name_from_obj(this)
                    .unwrap_or_else(|| "java/lang/Object".to_owned());
                let mut comps_meta: Vec<(String, String)> = Vec::new();
                if let Some(cf) = self.classes.get(&target) {
                    for attr in &cf.attributes {
                        if let Attribute::Record { components } = attr {
                            for c in components {
                                let name = cf.constant_pool.utf8(c.name_index).to_owned();
                                let desc = cf.constant_pool.utf8(c.descriptor_index).to_owned();
                                comps_meta.push((name, desc));
                            }
                        }
                    }
                }
                if comps_meta.is_empty() {
                    return Some(JValue::Ref(None));
                }
                let comps = comps_meta
                    .into_iter()
                    .map(|(n, d)| JValue::Ref(Some(self.build_reflect_record_component(&target, &n, &d))))
                    .collect();
                Some(JValue::Ref(Some(JObject::new_array(
                    "[Ljava/lang/reflect/RecordComponent;",
                    comps,
                ))))
            }
            ("java/lang/Class", "getDeclaredAnnotations") => {
                let target = self
                    .class_internal_name_from_obj(this)
                    .unwrap_or_else(|| "java/lang/Object".to_owned());
                let anns = if let Some(cf) = self.classes.get(&target) {
                    let attrs = cf.attributes.clone();
                    let cp_entries = cf.constant_pool.entries.clone();
                    let cp = crate::class_file::ConstantPool { entries: cp_entries };
                    self.parse_runtime_visible_annotations(&attrs, &cp)
                } else {
                    Vec::new()
                };
                Some(self.build_annotation_ref_array(anns))
            }
            ("java/lang/Class", "getDeclaredFields0") | ("java/lang/Class", "getDeclaredFields") => {
                let target = self
                    .class_internal_name_from_obj(this)
                    .unwrap_or_else(|| "java/lang/Object".to_owned());
                let public_only = _args.first().map(|v| v.as_int() != 0).unwrap_or(false);
                let mut out = Vec::new();
                let mut members: Vec<(String, String, u16)> = Vec::new();
                if let Some(cf) = self.classes.get(&target) {
                    for f in &cf.fields {
                        if public_only && (f.access_flags & 0x0001) == 0 {
                            continue;
                        }
                        let name = cf.constant_pool.utf8(f.name_index).to_owned();
                        let desc = cf.constant_pool.utf8(f.descriptor_index).to_owned();
                        members.push((name, desc, f.access_flags));
                    }
                }
                for (name, desc, flags) in members {
                    out.push(JValue::Ref(Some(self.build_reflect_field(&target, &name, &desc, flags))));
                }
                Some(JValue::Ref(Some(JObject::new_array(
                    "[Ljava/lang/reflect/Field;",
                    out,
                ))))
            }
            ("java/lang/Class", "getDeclaredMethods0") | ("java/lang/Class", "getDeclaredMethods") => {
                let target = self
                    .class_internal_name_from_obj(this)
                    .unwrap_or_else(|| "java/lang/Object".to_owned());
                let public_only = _args.first().map(|v| v.as_int() != 0).unwrap_or(false);
                let mut out = Vec::new();
                let mut members: Vec<(String, String, u16, Vec<String>)> = Vec::new();
                if let Some(cf) = self.classes.get(&target) {
                    for m in &cf.methods {
                        if public_only && (m.access_flags & 0x0001) == 0 {
                            continue;
                        }
                        let name = cf.constant_pool.utf8(m.name_index).to_owned();
                        if name == "<init>" || name == "<clinit>" {
                            continue;
                        }
                        let desc = cf.constant_pool.utf8(m.descriptor_index).to_owned();
                        let mut ex = Vec::new();
                        for attr in &m.attributes {
                            if let Attribute::Exceptions { exception_index_table } = attr {
                                ex = exception_index_table
                                    .iter()
                                    .map(|idx| cf.constant_pool.class_name(*idx).to_owned())
                                    .collect();
                            }
                        }
                        members.push((name, desc, m.access_flags, ex));
                    }
                }
                for (name, desc, flags, ex) in members {
                    out.push(JValue::Ref(Some(self.build_reflect_method(
                        &target, &name, &desc, flags, ex,
                    ))));
                }
                Some(JValue::Ref(Some(JObject::new_array(
                    "[Ljava/lang/reflect/Method;",
                    out,
                ))))
            }
            ("java/lang/Class", "getDeclaredConstructors0")
            | ("java/lang/Class", "getDeclaredConstructors") => {
                let target = self
                    .class_internal_name_from_obj(this)
                    .unwrap_or_else(|| "java/lang/Object".to_owned());
                let public_only = _args.first().map(|v| v.as_int() != 0).unwrap_or(false);
                let mut out = Vec::new();
                let mut members: Vec<(String, u16, Vec<String>)> = Vec::new();
                if let Some(cf) = self.classes.get(&target) {
                    for m in &cf.methods {
                        if public_only && (m.access_flags & 0x0001) == 0 {
                            continue;
                        }
                        let name = cf.constant_pool.utf8(m.name_index).to_owned();
                        if name != "<init>" {
                            continue;
                        }
                        let desc = cf.constant_pool.utf8(m.descriptor_index).to_owned();
                        let mut ex = Vec::new();
                        for attr in &m.attributes {
                            if let Attribute::Exceptions { exception_index_table } = attr {
                                ex = exception_index_table
                                    .iter()
                                    .map(|idx| cf.constant_pool.class_name(*idx).to_owned())
                                    .collect();
                            }
                        }
                        members.push((desc, m.access_flags, ex));
                    }
                }
                for (desc, flags, ex) in members {
                    out.push(JValue::Ref(Some(self.build_reflect_constructor(
                        &target, &desc, flags, ex,
                    ))));
                }
                Some(JValue::Ref(Some(JObject::new_array(
                    "[Ljava/lang/reflect/Constructor;",
                    out,
                ))))
            }
            ("java/lang/reflect/Executable", "getParameterAnnotations")
            | ("java/lang/reflect/Method", "getParameterAnnotations")
            | ("java/lang/reflect/Constructor", "getParameterAnnotations") => {
                let runtime_cn = this.borrow().class_name.clone();
                let (owner, method_name, desc, param_count) = if runtime_cn == "java/lang/reflect/Method" {
                    let m = this.borrow();
                    let owner = m.fields.get("clazz")
                        .and_then(|v| v.as_ref())
                        .and_then(|c| self.class_internal_name_from_obj(c))
                        .unwrap_or_else(|| "java/lang/Object".to_owned());
                    let method_name = m.fields.get("name")
                        .and_then(|v| v.as_ref())
                        .and_then(|s| s.borrow().as_java_string().map(|x| x.to_owned()))
                        .unwrap_or_default();
                    let desc = m.fields.get("__descriptor")
                        .and_then(|v| v.as_ref())
                        .and_then(|s| s.borrow().as_java_string().map(|x| x.to_owned()))
                        .unwrap_or_default();
                    let param_count = m.fields.get("parameterTypes")
                        .and_then(|v| v.as_ref())
                        .and_then(|arr| match &arr.borrow().native {
                            NativePayload::Array(v) => Some(v.len()),
                            _ => None,
                        })
                        .unwrap_or(0);
                    (owner, method_name, desc, param_count)
                } else {
                    let c = this.borrow();
                    let owner = c.fields.get("clazz")
                        .and_then(|v| v.as_ref())
                        .and_then(|k| self.class_internal_name_from_obj(k))
                        .unwrap_or_else(|| "java/lang/Object".to_owned());
                    let desc = c.fields.get("__descriptor")
                        .and_then(|v| v.as_ref())
                        .and_then(|s| s.borrow().as_java_string().map(|x| x.to_owned()))
                        .unwrap_or_default();
                    let param_count = c.fields.get("parameterTypes")
                        .and_then(|v| v.as_ref())
                        .and_then(|arr| match &arr.borrow().native {
                            NativePayload::Array(v) => Some(v.len()),
                            _ => None,
                        })
                        .unwrap_or(0);
                    (owner, "<init>".to_owned(), desc, param_count)
                };

                let per_param = if let Some(cf) = self.classes.get(&owner) {
                    if let Some(mi) = cf.methods.iter().find(|m| {
                        cf.constant_pool.utf8(m.name_index) == method_name
                            && cf.constant_pool.utf8(m.descriptor_index) == desc
                    }) {
                        let attrs = mi.attributes.clone();
                        let cp_entries = cf.constant_pool.entries.clone();
                        let cp = crate::class_file::ConstantPool { entries: cp_entries };
                        self.parse_runtime_visible_parameter_annotations(&attrs, &cp, param_count)
                    } else {
                        vec![Vec::new(); param_count]
                    }
                } else {
                    vec![Vec::new(); param_count]
                };
                let outer = per_param
                    .into_iter()
                    .map(|ann_refs| self.build_annotation_ref_array(ann_refs))
                    .collect();
                Some(JValue::Ref(Some(JObject::new_array(
                    "[[Ljava/lang/annotation/Annotation;",
                    outer,
                ))))
            }
            ("java/lang/reflect/Method", "invoke") => {
                let (owner, name, desc, modifiers) = {
                    let m = this.borrow();
                    let owner = m.fields.get("clazz")
                        .and_then(|v| v.as_ref())
                        .and_then(|c| self.class_internal_name_from_obj(c))
                        .unwrap_or_else(|| "java/lang/Object".to_owned());
                    let name = m.fields.get("name")
                        .and_then(|v| v.as_ref())
                        .and_then(|s| s.borrow().as_java_string().map(|x| x.to_owned()))
                        .unwrap_or_default();
                    let desc = m.fields.get("__descriptor")
                        .and_then(|v| v.as_ref())
                        .and_then(|s| s.borrow().as_java_string().map(|x| x.to_owned()))
                        .unwrap_or_else(|| "()Ljava/lang/Object;".to_owned());
                    let modifiers = m.fields.get("modifiers").map(|v| v.as_int()).unwrap_or(0);
                    (owner, name, desc, modifiers)
                };

                let recv = _args.first().cloned().unwrap_or(JValue::Ref(None));
                let arg_array = _args.get(1).and_then(|v| v.as_ref());
                let raw_args = self.collect_reflection_args(arg_array);
                let (param_tokens, ret_token) = Self::parse_method_descriptor_tokens(&desc);
                let mut call_args = Vec::with_capacity(param_tokens.len());
                for (i, p) in param_tokens.iter().enumerate() {
                    let src = raw_args.get(i).cloned().unwrap_or_else(|| default_value_for_descriptor(p));
                    call_args.push(self.adapt_value_for_descriptor(p, src));
                }

                let result = if (modifiers & 0x0008) != 0 {
                    self.invoke_static(&owner, &name, &desc, call_args)
                } else {
                    match recv {
                        JValue::Ref(Some(r)) => self.invoke_virtual(r, &owner, &name, &desc, call_args),
                        _ => Ok(JValue::Ref(None)),
                    }
                };

                let out = match result {
                    Ok(v) => v,
                    Err(e) => {
                        self.raise_invocation_target_exception(&e);
                        return Some(JValue::Ref(None));
                    }
                };
                if ret_token == "V" {
                    Some(JValue::Ref(None))
                } else if !matches!(ret_token.as_bytes().first(), Some(b'L' | b'[')) {
                    Some(self.wrap_primitive_value(out))
                } else {
                    Some(out)
                }
            }
            ("java/lang/reflect/Method", "getDeclaredAnnotations") => {
                let (owner, name, desc) = {
                    let m = this.borrow();
                    let owner = m.fields.get("clazz")
                        .and_then(|v| v.as_ref())
                        .and_then(|c| self.class_internal_name_from_obj(c))
                        .unwrap_or_else(|| "java/lang/Object".to_owned());
                    let name = m.fields.get("name")
                        .and_then(|v| v.as_ref())
                        .and_then(|s| s.borrow().as_java_string().map(|x| x.to_owned()))
                        .unwrap_or_default();
                    let desc = m.fields.get("__descriptor")
                        .and_then(|v| v.as_ref())
                        .and_then(|s| s.borrow().as_java_string().map(|x| x.to_owned()))
                        .unwrap_or_default();
                    (owner, name, desc)
                };
                let anns = if let Some(cf) = self.classes.get(&owner) {
                    if let Some(mi) = cf.methods.iter().find(|m| {
                        cf.constant_pool.utf8(m.name_index) == name && cf.constant_pool.utf8(m.descriptor_index) == desc
                    }) {
                        let attrs = mi.attributes.clone();
                        let cp_entries = cf.constant_pool.entries.clone();
                        let cp = crate::class_file::ConstantPool { entries: cp_entries };
                        self.parse_runtime_visible_annotations(&attrs, &cp)
                    } else {
                        Vec::new()
                    }
                } else {
                    Vec::new()
                };
                Some(self.build_annotation_ref_array(anns))
            }
            ("java/lang/reflect/Constructor", "newInstance") => {
                let (owner, desc) = {
                    let c = this.borrow();
                    let owner = c.fields.get("clazz")
                        .and_then(|v| v.as_ref())
                        .and_then(|k| self.class_internal_name_from_obj(k))
                        .unwrap_or_else(|| "java/lang/Object".to_owned());
                    let desc = c.fields.get("__descriptor")
                        .and_then(|v| v.as_ref())
                        .and_then(|s| s.borrow().as_java_string().map(|x| x.to_owned()))
                        .unwrap_or_else(|| "()V".to_owned());
                    (owner, desc)
                };
                let arg_array = _args.first().and_then(|v| v.as_ref());
                let raw_args = self.collect_reflection_args(arg_array);
                let (param_tokens, _) = Self::parse_method_descriptor_tokens(&desc);
                let mut call_args = Vec::with_capacity(param_tokens.len());
                for (i, p) in param_tokens.iter().enumerate() {
                    let src = raw_args.get(i).cloned().unwrap_or_else(|| default_value_for_descriptor(p));
                    call_args.push(self.adapt_value_for_descriptor(p, src));
                }
                let obj = JObject::new(owner.clone());
                if let Err(e) = self.invoke_virtual(obj.clone(), &owner, "<init>", &desc, call_args) {
                    self.raise_invocation_target_exception(&e);
                    return Some(JValue::Ref(None));
                }
                Some(JValue::Ref(Some(obj)))
            }
            ("java/lang/reflect/Constructor", "getDeclaredAnnotations") => {
                let (owner, desc) = {
                    let c = this.borrow();
                    let owner = c.fields.get("clazz")
                        .and_then(|v| v.as_ref())
                        .and_then(|k| self.class_internal_name_from_obj(k))
                        .unwrap_or_else(|| "java/lang/Object".to_owned());
                    let desc = c.fields.get("__descriptor")
                        .and_then(|v| v.as_ref())
                        .and_then(|s| s.borrow().as_java_string().map(|x| x.to_owned()))
                        .unwrap_or_default();
                    (owner, desc)
                };
                let anns = if let Some(cf) = self.classes.get(&owner) {
                    if let Some(mi) = cf.methods.iter().find(|m| {
                        cf.constant_pool.utf8(m.name_index) == "<init>" && cf.constant_pool.utf8(m.descriptor_index) == desc
                    }) {
                        let attrs = mi.attributes.clone();
                        let cp_entries = cf.constant_pool.entries.clone();
                        let cp = crate::class_file::ConstantPool { entries: cp_entries };
                        self.parse_runtime_visible_annotations(&attrs, &cp)
                    } else {
                        Vec::new()
                    }
                } else {
                    Vec::new()
                };
                Some(self.build_annotation_ref_array(anns))
            }
            ("java/lang/reflect/Field", "get") => {
                let (owner, name, desc, modifiers) = {
                    let f = this.borrow();
                    let owner = f.fields.get("clazz")
                        .and_then(|v| v.as_ref())
                        .and_then(|k| self.class_internal_name_from_obj(k))
                        .unwrap_or_else(|| "java/lang/Object".to_owned());
                    let name = f.fields.get("name")
                        .and_then(|v| v.as_ref())
                        .and_then(|s| s.borrow().as_java_string().map(|x| x.to_owned()))
                        .unwrap_or_default();
                    let desc = f.fields.get("__descriptor")
                        .and_then(|v| v.as_ref())
                        .and_then(|s| s.borrow().as_java_string().map(|x| x.to_owned()))
                        .unwrap_or_else(|| "Ljava/lang/Object;".to_owned());
                    let modifiers = f.fields.get("modifiers").map(|v| v.as_int()).unwrap_or(0);
                    (owner, name, desc, modifiers)
                };

                let raw = if (modifiers & 0x0008) != 0 {
                    self.static_fields
                        .get(&format!("{owner}.{name}"))
                        .cloned()
                        .unwrap_or_else(|| default_value_for_descriptor(&desc))
                } else {
                    match _args.first().and_then(|v| v.as_ref()) {
                        Some(target) => target.borrow().fields.get(&name).cloned().unwrap_or_else(|| default_value_for_descriptor(&desc)),
                        None => JValue::Ref(None),
                    }
                };
                if matches!(desc.as_bytes().first(), Some(b'L' | b'[')) {
                    Some(raw)
                } else {
                    Some(self.wrap_primitive_value(raw))
                }
            }
            ("java/lang/reflect/Field", "set") => {
                let (owner, name, desc, modifiers) = {
                    let f = this.borrow();
                    let owner = f.fields.get("clazz")
                        .and_then(|v| v.as_ref())
                        .and_then(|k| self.class_internal_name_from_obj(k))
                        .unwrap_or_else(|| "java/lang/Object".to_owned());
                    let name = f.fields.get("name")
                        .and_then(|v| v.as_ref())
                        .and_then(|s| s.borrow().as_java_string().map(|x| x.to_owned()))
                        .unwrap_or_default();
                    let desc = f.fields.get("__descriptor")
                        .and_then(|v| v.as_ref())
                        .and_then(|s| s.borrow().as_java_string().map(|x| x.to_owned()))
                        .unwrap_or_else(|| "Ljava/lang/Object;".to_owned());
                    let modifiers = f.fields.get("modifiers").map(|v| v.as_int()).unwrap_or(0);
                    (owner, name, desc, modifiers)
                };
                let val = _args.get(1).cloned().unwrap_or(JValue::Ref(None));
                let adapted = self.adapt_value_for_descriptor(&desc, val);
                if (modifiers & 0x0008) != 0 {
                    self.static_fields.insert(format!("{owner}.{name}"), adapted);
                } else if let Some(target) = _args.first().and_then(|v| v.as_ref()) {
                    target.borrow_mut().fields.insert(name, adapted);
                }
                Some(JValue::Void)
            }
            ("java/lang/reflect/Field", "getDeclaredAnnotations") => {
                let (owner, name, desc) = {
                    let f = this.borrow();
                    let owner = f.fields.get("clazz")
                        .and_then(|v| v.as_ref())
                        .and_then(|k| self.class_internal_name_from_obj(k))
                        .unwrap_or_else(|| "java/lang/Object".to_owned());
                    let name = f.fields.get("name")
                        .and_then(|v| v.as_ref())
                        .and_then(|s| s.borrow().as_java_string().map(|x| x.to_owned()))
                        .unwrap_or_default();
                    let desc = f.fields.get("__descriptor")
                        .and_then(|v| v.as_ref())
                        .and_then(|s| s.borrow().as_java_string().map(|x| x.to_owned()))
                        .unwrap_or_default();
                    (owner, name, desc)
                };
                let anns = if let Some(cf) = self.classes.get(&owner) {
                    if let Some(fi) = cf.fields.iter().find(|f| {
                        cf.constant_pool.utf8(f.name_index) == name && cf.constant_pool.utf8(f.descriptor_index) == desc
                    }) {
                        let attrs = fi.attributes.clone();
                        let cp_entries = cf.constant_pool.entries.clone();
                        let cp = crate::class_file::ConstantPool { entries: cp_entries };
                        self.parse_runtime_visible_annotations(&attrs, &cp)
                    } else {
                        Vec::new()
                    }
                } else {
                    Vec::new()
                };
                Some(self.build_annotation_ref_array(anns))
            }
            ("java/lang/reflect/RecordComponent", "getDeclaredAnnotations") => {
                let (owner, name, desc) = {
                    let rc = this.borrow();
                    let owner = rc.fields.get("clazz")
                        .and_then(|v| v.as_ref())
                        .and_then(|k| self.class_internal_name_from_obj(k))
                        .unwrap_or_else(|| "java/lang/Object".to_owned());
                    let name = rc.fields.get("name")
                        .and_then(|v| v.as_ref())
                        .and_then(|s| s.borrow().as_java_string().map(|x| x.to_owned()))
                        .unwrap_or_default();
                    let desc = rc.fields.get("__descriptor")
                        .and_then(|v| v.as_ref())
                        .and_then(|s| s.borrow().as_java_string().map(|x| x.to_owned()))
                        .unwrap_or_default();
                    (owner, name, desc)
                };
                let ann_src = self.classes.get(&owner).and_then(|cf| {
                    for attr in &cf.attributes {
                        if let Attribute::Record { components } = attr {
                            if let Some(c) = components.iter().find(|c| {
                                cf.constant_pool.utf8(c.name_index) == name
                                    && cf.constant_pool.utf8(c.descriptor_index) == desc
                            }) {
                                let attrs = c.attributes.clone();
                                let cp_entries = cf.constant_pool.entries.clone();
                                let cp = crate::class_file::ConstantPool { entries: cp_entries };
                                return Some((attrs, cp));
                            }
                        }
                    }
                    None
                });
                let anns = if let Some((attrs, cp)) = ann_src {
                    self.parse_runtime_visible_annotations(&attrs, &cp)
                } else {
                    Vec::new()
                };
                Some(self.build_annotation_ref_array(anns))
            }
            ("java/lang/reflect/Field", "getBoolean") => {
                let v = self.native_virtual(this, _class_name, "get", _descriptor, _args)?;
                let i = self.adapt_value_for_descriptor("Z", v).as_int();
                Some(JValue::Int(if i == 0 { 0 } else { 1 }))
            }
            ("java/lang/reflect/Field", "getByte") => {
                let v = self.native_virtual(this, _class_name, "get", _descriptor, _args)?;
                Some(self.adapt_value_for_descriptor("B", v))
            }
            ("java/lang/reflect/Field", "getChar") => {
                let v = self.native_virtual(this, _class_name, "get", _descriptor, _args)?;
                Some(self.adapt_value_for_descriptor("C", v))
            }
            ("java/lang/reflect/Field", "getShort") => {
                let v = self.native_virtual(this, _class_name, "get", _descriptor, _args)?;
                Some(self.adapt_value_for_descriptor("S", v))
            }
            ("java/lang/reflect/Field", "getInt") => {
                let v = self.native_virtual(this, _class_name, "get", _descriptor, _args)?;
                Some(self.adapt_value_for_descriptor("I", v))
            }
            ("java/lang/reflect/Field", "getLong") => {
                let v = self.native_virtual(this, _class_name, "get", _descriptor, _args)?;
                Some(self.adapt_value_for_descriptor("J", v))
            }
            ("java/lang/reflect/Field", "getFloat") => {
                let v = self.native_virtual(this, _class_name, "get", _descriptor, _args)?;
                Some(self.adapt_value_for_descriptor("F", v))
            }
            ("java/lang/reflect/Field", "getDouble") => {
                let v = self.native_virtual(this, _class_name, "get", _descriptor, _args)?;
                Some(self.adapt_value_for_descriptor("D", v))
            }
            ("java/lang/reflect/Field", "setBoolean") => {
                let mut a = vec![_args.first().cloned().unwrap_or(JValue::Ref(None))];
                a.push(self.adapt_value_for_descriptor("Z", _args.get(1).cloned().unwrap_or(JValue::Int(0))));
                self.native_virtual(this, _class_name, "set", _descriptor, &a)
            }
            ("java/lang/reflect/Field", "setByte") => {
                let mut a = vec![_args.first().cloned().unwrap_or(JValue::Ref(None))];
                a.push(self.adapt_value_for_descriptor("B", _args.get(1).cloned().unwrap_or(JValue::Int(0))));
                self.native_virtual(this, _class_name, "set", _descriptor, &a)
            }
            ("java/lang/reflect/Field", "setChar") => {
                let mut a = vec![_args.first().cloned().unwrap_or(JValue::Ref(None))];
                a.push(self.adapt_value_for_descriptor("C", _args.get(1).cloned().unwrap_or(JValue::Int(0))));
                self.native_virtual(this, _class_name, "set", _descriptor, &a)
            }
            ("java/lang/reflect/Field", "setShort") => {
                let mut a = vec![_args.first().cloned().unwrap_or(JValue::Ref(None))];
                a.push(self.adapt_value_for_descriptor("S", _args.get(1).cloned().unwrap_or(JValue::Int(0))));
                self.native_virtual(this, _class_name, "set", _descriptor, &a)
            }
            ("java/lang/reflect/Field", "setInt") => {
                let mut a = vec![_args.first().cloned().unwrap_or(JValue::Ref(None))];
                a.push(self.adapt_value_for_descriptor("I", _args.get(1).cloned().unwrap_or(JValue::Int(0))));
                self.native_virtual(this, _class_name, "set", _descriptor, &a)
            }
            ("java/lang/reflect/Field", "setLong") => {
                let mut a = vec![_args.first().cloned().unwrap_or(JValue::Ref(None))];
                a.push(self.adapt_value_for_descriptor("J", _args.get(1).cloned().unwrap_or(JValue::Long(0))));
                self.native_virtual(this, _class_name, "set", _descriptor, &a)
            }
            ("java/lang/reflect/Field", "setFloat") => {
                let mut a = vec![_args.first().cloned().unwrap_or(JValue::Ref(None))];
                a.push(self.adapt_value_for_descriptor("F", _args.get(1).cloned().unwrap_or(JValue::Float(0.0))));
                self.native_virtual(this, _class_name, "set", _descriptor, &a)
            }
            ("java/lang/reflect/Field", "setDouble") => {
                let mut a = vec![_args.first().cloned().unwrap_or(JValue::Ref(None))];
                a.push(self.adapt_value_for_descriptor("D", _args.get(1).cloned().unwrap_or(JValue::Double(0.0))));
                self.native_virtual(this, _class_name, "set", _descriptor, &a)
            }
            // String native methods — backed by NativePayload::JavaString in Rust.
            ("java/lang/String", "toString") => {
                let s = this.borrow().as_java_string().unwrap_or("").to_owned();
                Some(JValue::Ref(Some(JObject::new_string(s))))
            }
            ("java/lang/String", "length") => {
                let len = this.borrow().as_java_string().map(|s| s.len() as i32).unwrap_or(0);
                Some(JValue::Int(len))
            }
            ("java/lang/String", "charAt") => {
                let idx = _args.first().map(|v| v.as_int() as usize).unwrap_or(0);
                let ch = this.borrow().as_java_string()
                    .and_then(|s| s.chars().nth(idx))
                    .unwrap_or('\0') as i32;
                Some(JValue::Int(ch))
            }
            ("java/lang/String", "isEmpty") => {
                let empty = this.borrow().as_java_string().map(|s| s.is_empty()).unwrap_or(true);
                Some(JValue::Int(if empty { 1 } else { 0 }))
            }
            ("java/lang/String", "equals") => {
                let other_str = _args.first()
                    .and_then(|a| a.as_ref())
                    .and_then(|r| r.borrow().as_java_string().map(|s| s.to_owned()));
                let this_str = this.borrow().as_java_string().map(|s| s.to_owned());
                let eq = match (this_str, other_str) {
                    (Some(a), Some(b)) => a == b,
                    _ => false,
                };
                Some(JValue::Int(if eq { 1 } else { 0 }))
            }
            ("java/lang/String", "hashCode") => {
                let hash = this.borrow().as_java_string().map(|s| {
                    s.chars().fold(0i32, |h, c| h.wrapping_mul(31).wrapping_add(c as i32))
                }).unwrap_or(0);
                Some(JValue::Int(hash))
            }
            ("java/lang/String", "substring") => {
                let begin = _args.first().map(|v| v.as_int() as usize).unwrap_or(0);
                let s = this.borrow().as_java_string().unwrap_or("").to_owned();
                let end = _args.get(1).map(|v| v.as_int() as usize).unwrap_or(s.len());
                let sub: String = s.chars().skip(begin).take(end - begin).collect();
                Some(JValue::Ref(Some(JObject::new_string(sub))))
            }
            ("java/lang/String", "concat") => {
                let a = this.borrow().as_java_string().unwrap_or("").to_owned();
                let b = _args.first()
                    .and_then(|v| v.as_ref())
                    .and_then(|r| r.borrow().as_java_string().map(|s| s.to_owned()))
                    .unwrap_or_default();
                Some(JValue::Ref(Some(JObject::new_string(a + &b))))
            }
            ("java/lang/String", "contains") => {
                let haystack = this.borrow().as_java_string().unwrap_or("").to_owned();
                let needle = _args.first()
                    .and_then(|v| v.as_ref())
                    .and_then(|r| r.borrow().as_java_string().map(|s| s.to_owned()))
                    .unwrap_or_default();
                Some(JValue::Int(if haystack.contains(&needle) { 1 } else { 0 }))
            }
            ("java/lang/String", "startsWith") => {
                let s = this.borrow().as_java_string().unwrap_or("").to_owned();
                let prefix = _args.first()
                    .and_then(|v| v.as_ref())
                    .and_then(|r| r.borrow().as_java_string().map(|s| s.to_owned()))
                    .unwrap_or_default();
                Some(JValue::Int(if s.starts_with(&prefix) { 1 } else { 0 }))
            }
            ("java/lang/String", "endsWith") => {
                let s = this.borrow().as_java_string().unwrap_or("").to_owned();
                let suffix = _args.first()
                    .and_then(|v| v.as_ref())
                    .and_then(|r| r.borrow().as_java_string().map(|s| s.to_owned()))
                    .unwrap_or_default();
                Some(JValue::Int(if s.ends_with(&suffix) { 1 } else { 0 }))
            }
            ("java/lang/String", "indexOf") => {
                let s = this.borrow().as_java_string().unwrap_or("").to_owned();
                let idx = match _args.first() {
                    Some(JValue::Ref(Some(r))) => {
                        let needle = r.borrow().as_java_string().unwrap_or("").to_owned();
                        s.find(&needle).map(|i| i as i32).unwrap_or(-1)
                    }
                    Some(JValue::Int(ch)) => {
                        let c = char::from_u32(*ch as u32).unwrap_or('\0');
                        s.find(c).map(|i| i as i32).unwrap_or(-1)
                    }
                    _ => -1,
                };
                Some(JValue::Int(idx))
            }
            ("java/lang/String", "lastIndexOf") => {
                let s = this.borrow().as_java_string().unwrap_or("").to_owned();
                let ch = _args.first().map(|v| v.as_int()).unwrap_or(0);
                let c = char::from_u32(ch as u32).unwrap_or('\0');
                let idx = s.rfind(c).map(|i| i as i32).unwrap_or(-1);
                Some(JValue::Int(idx))
            }
            ("java/lang/String", "trim") => {
                let s = this.borrow().as_java_string().unwrap_or("").trim().to_owned();
                Some(JValue::Ref(Some(JObject::new_string(s))))
            }
            ("java/lang/String", "toLowerCase") => {
                let s = this.borrow().as_java_string().unwrap_or("").to_lowercase();
                Some(JValue::Ref(Some(JObject::new_string(s))))
            }
            ("java/lang/String", "toUpperCase") => {
                let s = this.borrow().as_java_string().unwrap_or("").to_uppercase();
                Some(JValue::Ref(Some(JObject::new_string(s))))
            }
            ("java/lang/String", "toCharArray") => {
                let s = this.borrow().as_java_string().unwrap_or("").to_owned();
                let chars: Vec<JValue> = s.chars().map(|c| JValue::Int(c as i32)).collect();
                Some(JValue::Ref(Some(JObject::new_array("[C", chars))))
            }
            ("java/lang/String", "getBytes") => {
                let s = this.borrow().as_java_string().unwrap_or("").to_owned();
                let bytes: Vec<JValue> = s.bytes().map(|b| JValue::Int(b as i32)).collect();
                Some(JValue::Ref(Some(JObject::new_array("[B", bytes))))
            }
            (c, "clone") if c == "java/lang/Object" || c.starts_with('[') => {
                let src = this.borrow();
                let mut fields = HashMap::new();
                for (k, v) in &src.fields {
                    fields.insert(k.clone(), v.clone());
                }
                let native = match &src.native {
                    NativePayload::None => NativePayload::None,
                    NativePayload::JavaString(s) => NativePayload::JavaString(s.clone()),
                    NativePayload::Array(v) => NativePayload::Array(v.clone()),
                    NativePayload::ByteArray(v) => NativePayload::ByteArray(v.clone()),
                    NativePayload::IntArray(v) => NativePayload::IntArray(v.clone()),
                    NativePayload::LongArray(v) => NativePayload::LongArray(v.clone()),
                    NativePayload::PrintStream(is_err) => NativePayload::PrintStream(*is_err),
                    NativePayload::Lambda(f) => NativePayload::Lambda(f.clone()),
                    NativePayload::BytecodeLambda {
                        sam_method,
                        sam_desc,
                        impl_class,
                        impl_method,
                        impl_desc,
                        ref_kind,
                        captured,
                    } =>
                        NativePayload::BytecodeLambda {
                            sam_method: sam_method.clone(),
                            sam_desc: sam_desc.clone(),
                            impl_class: impl_class.clone(),
                            impl_method: impl_method.clone(),
                            impl_desc: impl_desc.clone(),
                            ref_kind: *ref_kind,
                            captured: captured.clone(),
                        },
                };
                let cloned = Rc::new(RefCell::new(crate::heap::JObject {
                    class_name: src.class_name.clone(),
                    fields,
                    native,
                }));
                Some(JValue::Ref(Some(cloned)))
            }
            // PrintStream native bridge.
            ("java/io/PrintStream", "println") | ("java/io/PrintStream", "print") => {
                let is_err = matches!(this.borrow().native, NativePayload::PrintStream(true));
                let text = _args.first().map(|v| self.printstream_text_for(v)).unwrap_or_default();
                self.write_printstream(is_err, &text, method_name == "println");
                Some(JValue::Void)
            }
            // System.arraycopy — native
            ("java/lang/System", "arraycopy") => {
                // Handled separately if needed.
                None
            }
            _ => None,
        }
    }

    /// Run `<clinit>` for a class if it hasn't been initialized yet.
    /// Per JVMS §5.5: Before a class is initialized, its direct superclass must
    /// be initialized first (recursively), and any superinterfaces that declare
    /// default methods must also be initialized.
    fn ensure_class_init(&mut self, class_name: &str) -> Result<(), String> {
        let key = format!("{class_name}.<clinit>done");
        if self.static_fields.contains_key(&key) {
            return Ok(());
        }
        // Mark as initialized before running to prevent recursion.
        self.static_fields.insert(key, JValue::Int(1));

        // Initialize super class first (JVMS §5.5 step 7).
        let (super_name, iface_names) = if let Some(class) = self.classes.get(class_name) {
            let sup = if class.super_class != 0 {
                let s = class.constant_pool.class_name(class.super_class).to_owned();
                if s != "java/lang/Object" { Some(s) } else { None }
            } else {
                None
            };
            // JVMS §5.5 step 8: initialize superinterfaces that declare default methods.
            let ifaces: Vec<String> = class.interfaces.iter()
                .map(|&idx| class.constant_pool.class_name(idx).to_owned())
                .collect();
            (sup, ifaces)
        } else {
            (None, vec![])
        };
        if let Some(s) = super_name {
            self.ensure_class_init(&s)?;
        }
        for iface in iface_names {
            self.ensure_class_init(&iface)?;
        }

        // Check if the class has a <clinit> method.
        let has_clinit = self.find_method(class_name, "<clinit>", "()V").is_some();
        if has_clinit {
            self.invoke_static(class_name, "<clinit>", "()V", vec![])?;
        }
        Ok(())
    }

    /// Check if `runtime_class` is an instance of `target_class` (by name).
    /// Handles array types per JVMS §6.5.instanceof / §6.5.checkcast.
    /// Recursively create a multi-dimensional array for `multianewarray`.
    fn create_multi_array(&self, desc: &str, sizes: &[usize], depth: usize) -> JRef {
        let count = sizes[depth];
        if depth + 1 >= sizes.len() {
            // Innermost dimension — create a flat array.
            let elem = if desc.ends_with("[I") || desc.ends_with("[B") || desc.ends_with("[C") || desc.ends_with("[S") || desc.ends_with("[Z") {
                JValue::Int(0)
            } else if desc.ends_with("[J") {
                JValue::Long(0)
            } else if desc.ends_with("[F") {
                JValue::Float(0.0)
            } else if desc.ends_with("[D") {
                JValue::Double(0.0)
            } else {
                JValue::Ref(None)
            };
            JObject::new_array(desc, vec![elem; count])
        } else {
            // Create sub-arrays.
            let sub_desc = &desc[1..]; // strip one '['
            let elements: Vec<JValue> = (0..count)
                .map(|_| JValue::Ref(Some(self.create_multi_array(sub_desc, sizes, depth + 1))))
                .collect();
            JObject::new_array(desc, elements)
        }
    }

    fn is_instance_of(&self, runtime_class: &str, target_class: &str) -> bool {
        if runtime_class == target_class { return true; }
        // java/lang/Object is a supertype of everything.
        if target_class == "java/lang/Object" { return true; }

        // Array type rules (JVMS §6.5.checkcast):
        //   array → Object: true (handled above)
        //   array → Cloneable / Serializable: true
        //   T[] → S[]: recursively check T against S
        if runtime_class.starts_with('[') {
            if target_class == "java/lang/Cloneable" || target_class == "java/io/Serializable" {
                return true;
            }
            if target_class.starts_with('[') {
                // Both are arrays: compare component types.
                let rc = &runtime_class[1..];
                let tc = &target_class[1..];
                // Extract component class names from descriptors.
                let rc_class = descriptor_to_class_name(rc);
                let tc_class = descriptor_to_class_name(tc);
                if let (Some(r), Some(t)) = (rc_class, tc_class) {
                    return self.is_instance_of(&r, &t);
                }
                // Primitive arrays: must be same type (already handled by == check above).
                return false;
            }
            return false;
        }

        // Check loaded class hierarchy.
        if let Some(class) = self.classes.get(runtime_class) {
            // Check interfaces (recursively).
            for &iface_idx in &class.interfaces {
                let iface_name = class.constant_pool.class_name(iface_idx);
                if self.is_instance_of(iface_name, target_class) { return true; }
            }
            // Check super class.
            if class.super_class != 0 {
                let super_name = class.constant_pool.class_name(class.super_class).to_owned();
                if self.is_instance_of(&super_name, target_class) {
                    return true;
                }
            }
        }
        false
    }
}

// ---------------------------------------------------------------------------
// Stack frame
// ---------------------------------------------------------------------------

struct Frame {
    locals: Vec<JValue>,
    stack: Vec<JValue>,
    pc: usize,
}

// ---------------------------------------------------------------------------
// Utility functions
// ---------------------------------------------------------------------------

/// JVM spec f2i: NaN→0, clamp to i32 range.
fn float_to_int(v: f32) -> i32 {
    if v.is_nan() { 0 }
    else if v >= i32::MAX as f32 { i32::MAX }
    else if v <= i32::MIN as f32 { i32::MIN }
    else { v as i32 }
}

/// JVM spec f2l: NaN→0, clamp to i64 range.
fn float_to_long(v: f32) -> i64 {
    if v.is_nan() { 0 }
    else if v >= i64::MAX as f32 { i64::MAX }
    else if v <= i64::MIN as f32 { i64::MIN }
    else { v as i64 }
}

/// JVM spec d2i: NaN→0, clamp to i32 range.
fn double_to_int(v: f64) -> i32 {
    if v.is_nan() { 0 }
    else if v >= i32::MAX as f64 { i32::MAX }
    else if v <= i32::MIN as f64 { i32::MIN }
    else { v as i32 }
}

/// JVM spec d2l: NaN→0, clamp to i64 range.
fn double_to_long(v: f64) -> i64 {
    if v.is_nan() { 0 }
    else if v >= i64::MAX as f64 { i64::MAX }
    else if v <= i64::MIN as f64 { i64::MIN }
    else { v as i64 }
}

fn read_i16(code: &[u8], pc: &mut usize) -> i16 {
    let hi = code[*pc] as i8 as i16;
    let lo = code[*pc + 1] as i16;
    *pc += 2;
    (hi << 8) | lo
}

fn read_u16(code: &[u8], pc: &mut usize) -> u16 {
    let v = u16::from_be_bytes([code[*pc], code[*pc + 1]]);
    *pc += 2;
    v
}

fn read_i32(code: &[u8], pc: &mut usize) -> i32 {
    let v = i32::from_be_bytes([code[*pc], code[*pc + 1], code[*pc + 2], code[*pc + 3]]);
    *pc += 4;
    v
}

fn resolve_class_name(cp: &[ConstantPoolEntry], idx: u16) -> String {
    match &cp[idx as usize] {
        ConstantPoolEntry::Class { name_index } => {
            match &cp[*name_index as usize] {
                ConstantPoolEntry::Utf8(s) => s.clone(),
                _ => String::new(),
            }
        }
        _ => String::new(),
    }
}

fn resolve_methodref(cp: &[ConstantPoolEntry], idx: u16) -> (String, String, String) {
    let (class_idx, nat_idx) = match &cp[idx as usize] {
        ConstantPoolEntry::Methodref { class_index, name_and_type_index }
        | ConstantPoolEntry::InterfaceMethodref { class_index, name_and_type_index } => {
            (*class_index, *name_and_type_index)
        }
        _ => return (String::new(), String::new(), String::new()),
    };
    let class_name = resolve_class_name(cp, class_idx);
    let (name, desc) = match &cp[nat_idx as usize] {
        ConstantPoolEntry::NameAndType { name_index, descriptor_index } => {
            let n = match &cp[*name_index as usize] { ConstantPoolEntry::Utf8(s) => s.clone(), _ => String::new() };
            let d = match &cp[*descriptor_index as usize] { ConstantPoolEntry::Utf8(s) => s.clone(), _ => String::new() };
            (n, d)
        }
        _ => (String::new(), String::new()),
    };
    (class_name, name, desc)
}

fn resolve_fieldref(cp: &[ConstantPoolEntry], idx: u16) -> (String, String, String) {
    let (class_idx, nat_idx) = match &cp[idx as usize] {
        ConstantPoolEntry::Fieldref { class_index, name_and_type_index } => {
            (*class_index, *name_and_type_index)
        }
        _ => return (String::new(), String::new(), String::new()),
    };
    let class_name = resolve_class_name(cp, class_idx);
    let (name, desc) = match &cp[nat_idx as usize] {
        ConstantPoolEntry::NameAndType { name_index, descriptor_index } => {
            let n = match &cp[*name_index as usize] { ConstantPoolEntry::Utf8(s) => s.clone(), _ => String::new() };
            let d = match &cp[*descriptor_index as usize] { ConstantPoolEntry::Utf8(s) => s.clone(), _ => String::new() };
            (n, d)
        }
        _ => (String::new(), String::new()),
    };
    (class_name, name, desc)
}

/// Return the default zero-value for a JVM field descriptor.
fn default_value_for_descriptor(desc: &str) -> JValue {
    match desc.as_bytes().first() {
        Some(b'I') | Some(b'B') | Some(b'C') | Some(b'S') | Some(b'Z') => JValue::Int(0),
        Some(b'J') => JValue::Long(0),
        Some(b'F') => JValue::Float(0.0),
        Some(b'D') => JValue::Double(0.0),
        _ => JValue::Ref(None), // Object types default to null
    }
}

/// Extract a class name from a JVM field descriptor.
/// `Ljava/lang/String;` → Some("java/lang/String")
/// `[Ljava/lang/String;` → Some("[Ljava/lang/String;") (preserves array)
/// Primitive descriptors (I, B, etc.) → None
fn descriptor_to_class_name(desc: &str) -> Option<String> {
    match desc.as_bytes().first()? {
        b'L' => {
            // Strip 'L' prefix and ';' suffix.
            let inner = &desc[1..desc.len().checked_sub(1).unwrap_or(1)];
            Some(inner.to_string())
        }
        b'[' => {
            // Array descriptor: treat as class name as-is for recursive checks.
            Some(desc.to_string())
        }
        _ => None, // Primitive type — not a class.
    }
}

/// Count the number of method arguments from a JVM method descriptor.
/// E.g. `"(ILjava/lang/String;Z)V"` → 3
fn count_args(descriptor: &str) -> usize {
    let mut count = 0usize;
    let mut chars = descriptor.chars().peekable();
    if chars.next() != Some('(') { return 0; }
    loop {
        match chars.next() {
            Some(')') | None => break,
            Some('L') => {
                // Skip until ';'
                for c in chars.by_ref() { if c == ';' { break; } }
                count += 1;
            }
            Some('[') => {
                // Array prefix — peek next to decide if it consumes another token.
                if chars.peek() == Some(&'L') {
                    chars.next();
                    for c in chars.by_ref() { if c == ';' { break; } }
                } else {
                    chars.next(); // primitive after [
                }
                count += 1;
            }
            Some('J') | Some('D') => count += 1, // long/double (take 2 stack slots but 1 arg)
            Some(_) => count += 1,
        }
    }
    count
}

fn method_return_descriptor(descriptor: &str) -> Option<&str> {
    descriptor.split_once(')').map(|(_, ret)| ret)
}

fn is_reference_descriptor(desc: &str) -> bool {
    matches!(desc.as_bytes().first(), Some(b'L' | b'['))
}

/// Pop `n` arguments from the operand stack, returned in call order.
fn pop_args(frame: &mut Frame, n: usize) -> Vec<JValue> {
    let mut args: Vec<JValue> = (0..n).map(|_| frame.stack.pop().unwrap_or(JValue::Void)).collect();
    args.reverse();
    args
}

fn is_category2(v: &JValue) -> bool {
    matches!(v, JValue::Long(_) | JValue::Double(_))
}

/// Compare two `JValue`s by reference identity.
fn refs_equal(a: &JValue, b: &JValue) -> bool {
    match (a, b) {
        (JValue::Ref(None), JValue::Ref(None)) => true,
        (JValue::Ref(Some(ra)), JValue::Ref(Some(rb))) => Rc::ptr_eq(ra, rb),
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_count_args() {
        assert_eq!(count_args("()V"), 0);
        assert_eq!(count_args("(I)V"), 1);
        assert_eq!(count_args("(ILjava/lang/String;Z)V"), 3);
        assert_eq!(count_args("(Ljava/lang/Object;Lnet/unit8/raoh/Path;)Lnet/unit8/raoh/Result;"), 2);
    }
}
