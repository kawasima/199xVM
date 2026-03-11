//! Java bytecode interpreter.
//!
//! Implements a stack-based interpreter over the JVM instruction set.
//! The focus is on the subset needed to run Raoh:
//! - Core stack / local-variable operations
//! - Object creation and field access
//! - Method invocation (all four flavours + `invokedynamic`)
//! - Integer / long / reference comparisons and control flow
//! - Native stubs for `java.lang.*` and `java.util.*`

use std::collections::HashMap;
use std::rc::Rc;

#[cfg(target_arch = "wasm32")]
use wasm_bindgen::prelude::*;

use crate::class_file::{
    ClassFile, MethodInfo,
};
use crate::heap::{JObject, JRef, JValue};

mod annotations;
mod bytecode;
mod descriptors;
mod dispatch;
mod frame;
mod invoke;
mod native_static;
mod native_virtual;
mod reflection;

use descriptors::*;

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
    pub(in crate::interpreter) classes: HashMap<String, ClassFile>,
    /// Interned strings cache (not strictly required but saves allocations).
    pub(in crate::interpreter) string_pool: HashMap<String, JRef>,
    /// Static field storage keyed by "ClassName.fieldName".
    pub(in crate::interpreter) static_fields: HashMap<String, JValue>,
    /// Canonical Class objects keyed by internal class name or descriptor.
    pub(in crate::interpreter) class_pool: HashMap<String, JRef>,
    /// Pending exception object — set by athrow, consumed by exception handler.
    /// This preserves the full exception object (with message, cause, fields)
    /// across the Err(String) propagation path.
    pub(in crate::interpreter) pending_exception: Option<JRef>,
    /// Buffered `System.out.print` content until newline/println.
    pub(in crate::interpreter) stdout_buffer: String,
    /// Buffered `System.err.print` content until newline/println.
    pub(in crate::interpreter) stderr_buffer: String,
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

    // ------------------------------------------------------------------

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

    /// Check if `runtime_class` is an instance of `target_class` (by name).
    /// Handles array types per JVMS §6.5.instanceof / §6.5.checkcast.
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

