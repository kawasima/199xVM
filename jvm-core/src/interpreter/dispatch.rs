use std::cell::RefCell;
use std::rc::Rc;

use crate::class_file::{BootstrapMethod, ConstantPoolEntry};
use crate::heap::{JObject, JValue, NativePayload};

use super::Vm;
use super::descriptors::*;
use super::frame::*;

impl Vm {
    pub(super) fn dispatch_static(
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

    pub(super) fn dispatch_virtual(
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
            other => {
                Err(format!(
                    "Expected reference for invokevirtual {class_name}.{method_name}{descriptor}, got {other:?}"
                ))
            }
        }
    }

    pub(super) fn dispatch_special(
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

    pub(super) fn dispatch_interface(
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
}
