//! Java bytecode interpreter.
//!
//! Implements a stack-based interpreter over the JVM instruction set.
//! The focus is on the subset needed to run Raoh:
//! - Core stack / local-variable operations
//! - Object creation and field access
//! - Method invocation (all four flavours + `invokedynamic`)
//! - Integer / long / reference comparisons and control flow
//! - Native stubs for `java.lang.*` and `java.util.*`

use std::collections::{HashMap, HashSet};
use std::rc::Rc;

#[cfg(target_arch = "wasm32")]
use wasm_bindgen::prelude::*;

use crate::class_file::{
    self, Attribute, BootstrapMethod, ClassFile, ConstantPoolEntry, ExceptionTableEntry,
};
use crate::heap::{JObject, JRef, JValue};

/// All execution-time data extracted from a resolved method in a single pass.
/// Returned by [`Vm::resolve_method_exec_info`] to avoid repeated `find_method`
/// calls and to give each field a self-documenting name.
pub(super) struct MethodExecInfo {
    /// Internal class name that owns the resolved method.
    pub class_name: String,
    /// Resolved method descriptor (may differ from the call-site descriptor for generics).
    pub descriptor: String,
    /// `Code.max_locals` (0 if the method has no `Code` attribute).
    pub max_locals: usize,
    /// `true` when the method has a `Code` attribute (i.e. is not abstract/native).
    pub has_code: bool,
    /// Raw bytecode.
    pub code: Vec<u8>,
    /// Exception handler table.
    pub exception_table: Vec<ExceptionTableEntry>,
    /// Shared constant-pool entries (`Rc` for O(1) clone).
    pub cp: Rc<Vec<ConstantPoolEntry>>,
    /// Bootstrap methods from the `BootstrapMethods` attribute.
    pub bootstrap_methods: Vec<BootstrapMethod>,
}

/// A class entry in the VM's class registry.
///
/// `Pending` holds the raw `.class` bytes and is promoted to `Ready` on first access,
/// implementing standard ClassLoader lazy-loading semantics.
pub(in crate::interpreter) enum LazyClass {
    /// Raw bytes not yet parsed.
    Pending(Vec<u8>),
    /// Fully parsed class file.
    Ready(ClassFile),
    /// Bytes were present but could not be parsed (malformed class).
    /// The entry is preserved so callers can distinguish "never registered"
    /// from "registered but broken", and to avoid repeated parse attempts.
    /// The inner `String` holds the original parse error message.
    ParseError(String),
}

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
    /// Class registry: keyed by internal name (`net/unit8/raoh/Result`).
    /// Entries start as `LazyClass::Pending` (raw bytes) and are promoted to
    /// `LazyClass::Ready` (parsed `ClassFile`) on first access.
    pub(in crate::interpreter) classes: HashMap<String, LazyClass>,
    /// Interned strings cache (not strictly required but saves allocations).
    pub(in crate::interpreter) string_pool: HashMap<String, JRef>,
    /// Static field storage keyed by class name → field name.
    /// Avoids allocating a `"ClassName.fieldName"` string on every getstatic/putstatic.
    pub(in crate::interpreter) static_fields: HashMap<String, HashMap<String, JValue>>,
    /// Classes whose `<clinit>` has already been run.
    pub(in crate::interpreter) clinit_done: HashSet<String>,
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
    /// Singleton system ClassLoader instance (created on first access).
    pub(in crate::interpreter) system_classloader: Option<JRef>,
}

impl Vm {
    /// Create an empty VM.
    pub fn new() -> Self {
        Vm {
            classes: HashMap::new(),
            string_pool: HashMap::new(),
            static_fields: HashMap::new(),
            clinit_done: HashSet::new(),
            class_pool: HashMap::new(),
            pending_exception: None,
            stdout_buffer: String::new(),
            stderr_buffer: String::new(),
            system_classloader: None,
        }
    }

    /// Register a pre-parsed class file (always stored as `Ready`).
    pub fn load_class(&mut self, class_file: ClassFile) {
        let name = class_file.constant_pool.class_name(class_file.this_class).to_owned();
        self.classes.insert(name, LazyClass::Ready(class_file));
    }

    /// Register raw `.class` bytes for lazy parsing.
    /// The class is parsed only when first accessed via [`Self::ensure_class_ready`].
    /// If the class is already registered (e.g., as `Ready`), the existing entry is kept.
    pub fn load_lazy(&mut self, name: String, bytes: Vec<u8>) {
        self.classes.entry(name).or_insert(LazyClass::Pending(bytes));
    }

    /// Ensure the named class is fully parsed (`Ready`).
    /// If the entry is `Pending`, parses it in place and promotes it to `Ready`.
    /// On parse failure the entry is set to `ParseError` so the failure is
    /// diagnosable and repeated parse attempts are avoided.
    /// Does nothing if the class is already `Ready`, `ParseError`, or not registered.
    pub(in crate::interpreter) fn ensure_class_ready(&mut self, name: &str) {
        // Only act when the entry is Pending; skip Ready / ParseError / missing.
        if !matches!(self.classes.get(name), Some(LazyClass::Pending(_))) {
            return;
        }
        if let Some(LazyClass::Pending(bytes)) = self.classes.remove(name) {
            match class_file::parse(&bytes) {
                Ok(cf) => { self.classes.insert(name.to_owned(), LazyClass::Ready(cf)); }
                Err(e) => {
                    eprintln!("Warning: failed to parse class '{name}': {e}");
                    self.classes.insert(name.to_owned(), LazyClass::ParseError(e.to_string()));
                }
            }
        }
    }

    /// Return a reference to a parsed class.
    /// Caller must have called `ensure_class_ready` first (or know the class is already Ready).
    pub(in crate::interpreter) fn get_class(&self, name: &str) -> Option<&ClassFile> {
        match self.classes.get(name)? {
            LazyClass::Ready(cf) => Some(cf),
            LazyClass::Pending(_) | LazyClass::ParseError(_) => None,
        }
    }

    /// Ensure class is ready and return a reference to it.
    pub(in crate::interpreter) fn resolve_class(&mut self, name: &str) -> Option<&ClassFile> {
        self.ensure_class_ready(name);
        self.get_class(name)
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
        use std::collections::hash_map::Entry;
        let s = s.into();
        match self.string_pool.entry(s) {
            Entry::Occupied(e) => Rc::clone(e.get()),
            Entry::Vacant(e) => {
                let jobj = JObject::new_string(e.key().clone());
                Rc::clone(e.insert(jobj))
            }
        }
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

    /// Return (or lazily create) the singleton system ClassLoader instance.
    pub(in crate::interpreter) fn get_or_create_system_classloader(&mut self) -> JRef {
        if let Some(ref cl) = self.system_classloader {
            return Rc::clone(cl);
        }
        let cl = JObject::new("java/lang/ClassLoader");
        self.system_classloader = Some(Rc::clone(&cl));
        cl
    }

    /// Set `pending_exception` to a new `ClassNotFoundException` for `name`.
    /// `name` should be the runtime (dot-separated) class name.
    pub(in crate::interpreter) fn throw_class_not_found(&mut self, name: &str) {
        let exc = JObject::new("java/lang/ClassNotFoundException");
        exc.borrow_mut().fields.insert(
            "detailMessage".to_owned(),
            JValue::Ref(Some(self.intern_string(name.to_owned()))),
        );
        self.pending_exception = Some(exc);
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

    /// Look up a loaded class by internal name (triggers lazy parse if needed).
    pub fn class(&mut self, name: &str) -> Option<&ClassFile> {
        self.resolve_class(name)
    }

    /// Find the `access_flags` of a method by name and descriptor in a class
    /// (including super-chain). Returns `None` if the method is not found.
    ///
    /// This is the lightweight variant used by invoke paths to decide dispatch
    /// strategy before calling `resolve_method_exec_info`.
    pub fn find_method_flags(
        &mut self,
        class_name: &str,
        method_name: &str,
        descriptor: &str,
    ) -> Option<u16> {
        self.ensure_class_ready(class_name);
        let class = self.get_class(class_name)?;
        for m in &class.methods {
            let n = class.constant_pool.utf8(m.name_index);
            let d = class.constant_pool.utf8(m.descriptor_index);
            if n == method_name && d == descriptor {
                return Some(m.access_flags);
            }
        }
        // Walk super class.
        let super_name = if class.super_class != 0 {
            Some(class.constant_pool.class_name(class.super_class).to_owned())
        } else {
            None
        };
        let iface_names: Vec<String> = class.interfaces.iter()
            .map(|&idx| class.constant_pool.class_name(idx).to_owned())
            .collect();
        if let Some(super_name) = super_name {
            if let Some(f) = self.find_method_flags(&super_name, method_name, descriptor) {
                return Some(f);
            }
        }
        for iface_name in &iface_names {
            if let Some(f) = self.find_method_flags(iface_name, method_name, descriptor) {
                return Some(f);
            }
        }
        None
    }

    /// Resolve a method and extract all execution-time data in a single pass.
    ///
    /// This avoids repeated method-lookup calls and eliminates the full clone of
    /// the constant pool that was previously needed to release the borrow on `self`.
    pub(super) fn resolve_method_exec_info(
        &mut self,
        class_name: &str,
        method_name: &str,
        descriptor: &str,
    ) -> Option<MethodExecInfo> {
        // Find the class that owns the method (following super/interface chain).
        let owner = self.find_method_owner(class_name, method_name, descriptor)?;
        self.ensure_class_ready(&owner);
        let class = self.get_class(&owner)?;
        // Find the method within the owning class.
        let method_idx = class.methods.iter().position(|m| {
            class.constant_pool.utf8(m.name_index) == method_name
                && class.constant_pool.utf8(m.descriptor_index) == descriptor
        })?;
        let class_name_out = class.constant_pool.class_name(class.this_class).to_owned();
        let descriptor_out = class.constant_pool.utf8(class.methods[method_idx].descriptor_index).to_owned();
        let (max_locals, has_code, code, exception_table) =
            if let Some(ca) = class.methods[method_idx].code() {
                (ca.max_locals as usize, true, ca.code.clone(), ca.exception_table.clone())
            } else {
                (0, false, vec![], vec![])
            };
        let cp = Rc::clone(&class.constant_pool.entries);
        let bootstrap_methods = class.attributes.iter().find_map(|a| {
            if let Attribute::BootstrapMethods(bms) = a { Some(bms.clone()) } else { None }
        }).unwrap_or_default();
        Some(MethodExecInfo {
            class_name: class_name_out,
            descriptor: descriptor_out,
            max_locals,
            has_code,
            code,
            exception_table,
            cp,
            bootstrap_methods,
        })
    }

    /// Find the name of the class that owns a given method (super-chain walk).
    /// Returns the canonical class name, or `None` if not found.
    fn find_method_owner(
        &mut self,
        class_name: &str,
        method_name: &str,
        descriptor: &str,
    ) -> Option<String> {
        self.ensure_class_ready(class_name);
        let class = self.get_class(class_name)?;
        for m in &class.methods {
            let n = class.constant_pool.utf8(m.name_index);
            let d = class.constant_pool.utf8(m.descriptor_index);
            if n == method_name && d == descriptor {
                return Some(class.constant_pool.class_name(class.this_class).to_owned());
            }
        }
        let super_name = if class.super_class != 0 {
            Some(class.constant_pool.class_name(class.super_class).to_owned())
        } else {
            None
        };
        let iface_names: Vec<String> = class.interfaces.iter()
            .map(|&idx| class.constant_pool.class_name(idx).to_owned())
            .collect();
        if let Some(super_name) = super_name {
            if let Some(owner) = self.find_method_owner(&super_name, method_name, descriptor) {
                return Some(owner);
            }
        }
        for iface_name in &iface_names {
            if let Some(owner) = self.find_method_owner(iface_name, method_name, descriptor) {
                return Some(owner);
            }
        }
        None
    }

    /// Returns `true` if the named method exists in the class hierarchy.
    /// Used to check method existence before dispatch without borrowing ClassFile data.
    pub(in crate::interpreter) fn method_exists(
        &mut self,
        class_name: &str,
        method_name: &str,
        descriptor: &str,
    ) -> bool {
        self.find_method_owner(class_name, method_name, descriptor).is_some()
    }

    /// Like find_method but with relaxed matching when the compiler emits generic types.
    /// Match priority:
    ///   1. Exact param types match (ignoring return type)
    ///   2. Same argument count match (ignoring both param types and return type)
    ///   3. Varargs method (ACC_VARARGS) whose non-varargs param count <= call arg count
    /// Returns the real descriptor string of the matched method.
    pub(in crate::interpreter) fn find_method_real_descriptor(
        &mut self,
        class_name: &str,
        method_name: &str,
        descriptor: &str,
    ) -> Option<String> {
        self.ensure_class_ready(class_name);
        let param_part = descriptor.split(')').next().unwrap_or("(");
        let arg_count = count_args(descriptor);
        let class = self.get_class(class_name)?;
        let mut arg_count_match: Option<String> = None;
        let mut varargs_match: Option<String> = None;
        for m in &class.methods {
            let n = class.constant_pool.utf8(m.name_index);
            let d = class.constant_pool.utf8(m.descriptor_index);
            if n != method_name { continue; }
            let d_param = d.split(')').next().unwrap_or("(");
            if d_param == param_part {
                return Some(d.to_owned());
            }
            if arg_count_match.is_none() && count_args(d) == arg_count {
                arg_count_match = Some(d.to_owned());
            }
            if varargs_match.is_none() && (m.access_flags & 0x0080 != 0) {
                let method_param_count = count_args(d);
                let fixed = method_param_count.saturating_sub(1);
                if arg_count >= fixed {
                    varargs_match = Some(d.to_owned());
                }
            }
        }
        if arg_count_match.is_some() { return arg_count_match; }
        if varargs_match.is_some() { return varargs_match; }
        let super_name = self.get_class(class_name)
            .filter(|c| c.super_class != 0)
            .map(|c| c.constant_pool.class_name(c.super_class).to_owned());
        let iface_names: Vec<String> = self.get_class(class_name)
            .map(|c| c.interfaces.iter().map(|&idx| c.constant_pool.class_name(idx).to_owned()).collect())
            .unwrap_or_default();
        if let Some(super_name) = super_name {
            if let Some(result) = self.find_method_real_descriptor(&super_name, method_name, descriptor) {
                return Some(result);
            }
        }
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
        if self.clinit_done.contains(class_name) {
            return Ok(());
        }
        // Mark as initialized before running to prevent recursion.
        self.clinit_done.insert(class_name.to_owned());

        // Ensure the class is parsed first.
        self.ensure_class_ready(class_name);

        // Initialize super class first (JVMS §5.5 step 7).
        let (super_name, iface_names) = if let Some(class) = self.get_class(class_name) {
            let sup = if class.super_class != 0 {
                let s = class.constant_pool.class_name(class.super_class).to_owned();
                if s != "java/lang/Object" { Some(s) } else { None }
            } else {
                None
            };
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

        // Check if THIS class (not superclasses) has a <clinit> method.
        // <clinit> is not inherited, so we must not walk the super-chain here —
        // doing so would re-execute a superclass <clinit> that was already run.
        self.ensure_class_ready(class_name);
        let has_clinit = self.get_class(class_name).map(|cf| {
            cf.methods.iter().any(|m| {
                cf.constant_pool.utf8(m.name_index) == "<clinit>"
                    && cf.constant_pool.utf8(m.descriptor_index) == "()V"
            })
        }).unwrap_or(false);
        if has_clinit {
            self.invoke_static(class_name, "<clinit>", "()V", vec![])?;
        }
        Ok(())
    }

    /// Recursively create a multi-dimensional array for `multianewarray`.
    fn create_multi_array(&self, desc: &str, sizes: &[usize], depth: usize) -> JRef {
        let count = sizes[depth];
        if depth + 1 >= sizes.len() {
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
            let sub_desc = &desc[1..];
            let elements: Vec<JValue> = (0..count)
                .map(|_| JValue::Ref(Some(self.create_multi_array(sub_desc, sizes, depth + 1))))
                .collect();
            JObject::new_array(desc, elements)
        }
    }

    /// Check if `runtime_class` is an instance of `target_class` (by name).
    /// Handles array types per JVMS §6.5.instanceof / §6.5.checkcast.
    fn is_instance_of(&mut self, runtime_class: &str, target_class: &str) -> bool {
        if runtime_class == target_class { return true; }
        if target_class == "java/lang/Object" { return true; }

        if runtime_class.starts_with('[') {
            if target_class == "java/lang/Cloneable" || target_class == "java/io/Serializable" {
                return true;
            }
            if target_class.starts_with('[') {
                let rc = &runtime_class[1..];
                let tc = &target_class[1..];
                let rc_class = descriptor_to_class_name(rc);
                let tc_class = descriptor_to_class_name(tc);
                if let (Some(r), Some(t)) = (rc_class, tc_class) {
                    return self.is_instance_of(&r, &t);
                }
                return false;
            }
            return false;
        }

        self.ensure_class_ready(runtime_class);
        let (iface_names, super_name) = if let Some(class) = self.get_class(runtime_class) {
            let ifaces: Vec<String> = class.interfaces.iter()
                .map(|&idx| class.constant_pool.class_name(idx).to_owned())
                .collect();
            let sup = if class.super_class != 0 {
                Some(class.constant_pool.class_name(class.super_class).to_owned())
            } else {
                None
            };
            (ifaces, sup)
        } else {
            return false;
        };
        for iface_name in &iface_names {
            if self.is_instance_of(iface_name, target_class) { return true; }
        }
        if let Some(super_name) = super_name {
            if self.is_instance_of(&super_name, target_class) {
                return true;
            }
        }
        false
    }
}
