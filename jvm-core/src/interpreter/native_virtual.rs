use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

use crate::class_file::Attribute;
use crate::heap::{JObject, JRef, JValue, NativePayload};

use super::descriptors::*;

#[cfg(target_arch = "wasm32")]
use super::{console_error, console_log};

/// Convert a Java char-index to a UTF-8 byte offset within `s`.
/// Returns `s.len()` when `char_idx` is beyond the end of the string.
fn char_to_byte_offset(s: &str, char_idx: usize) -> usize {
    if char_idx == 0 {
        return 0;
    }
    s.char_indices().nth(char_idx).map(|(b, _)| b).unwrap_or(s.len())
}

impl super::Vm {
    /// Extract a Rust `String` from `java/lang/String` constructor arguments based on the method descriptor.
    /// Returns an empty string if the descriptor is not recognized or arguments are invalid.
    pub(super) fn string_from_init_args(&self, descriptor: &str, args: &[JValue], _this: &JRef) -> String {
        match descriptor {
            "()V" => String::new(),
            "([C)V" => {
                // String(char[])
                if let Some(r) = args.first().and_then(|a| a.as_ref()) {
                    if let NativePayload::Array(chars) = &r.borrow().native {
                        chars.iter().map(|v| {
                            let code = v.as_int() as u32;
                            char::from_u32(code).unwrap_or('?')
                        }).collect()
                    } else { String::new() }
                } else { String::new() }
            }
            "([CII)V" => {
                // String(char[], offset, count)
                if let Some(r) = args.first().and_then(|a| a.as_ref()) {
                    let offset = args.get(1).map(|a| a.as_int().max(0) as usize).unwrap_or(0);
                    let count = args.get(2).map(|a| a.as_int().max(0) as usize).unwrap_or(0);
                    if let NativePayload::Array(chars) = &r.borrow().native {
                        let end = offset.saturating_add(count).min(chars.len());
                        chars[offset.min(chars.len())..end].iter()
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

    pub(super) fn printstream_text_for(&mut self, value: &JValue) -> String {
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

    pub(super) fn emit_host_line(is_err: bool, line: &str) {
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

    pub(super) fn write_printstream(&mut self, is_err: bool, text: &str, newline: bool) {
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

    pub(super) fn native_virtual(
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
            ("java/util/regex/Pattern", "matcher") => {
                let input = _args
                    .first()
                    .and_then(|v| v.as_ref())
                    .and_then(|r| r.borrow().as_java_string().map(|s| s.to_owned()))
                    .unwrap_or_default();
                let m = JObject::new("java/util/regex/Matcher");
                m.borrow_mut().fields.insert("__pattern".to_owned(), JValue::Ref(Some(this.clone())));
                m.borrow_mut().fields.insert("__input".to_owned(), JValue::Ref(Some(self.intern_string(input))));
                Some(JValue::Ref(Some(m)))
            }
            ("java/util/regex/Matcher", "matches") => {
                let (regex, input) = {
                    let mb = this.borrow();
                    let regex = mb.fields.get("__pattern")
                        .and_then(|v| v.as_ref())
                        .and_then(|p| p.borrow().fields.get("__regex").cloned())
                        .and_then(|v| v.as_ref().cloned())
                        .and_then(|s| s.borrow().as_java_string().map(|x| x.to_owned()))
                        .unwrap_or_default();
                    let input = mb.fields.get("__input")
                        .and_then(|v| v.as_ref())
                        .and_then(|s| s.borrow().as_java_string().map(|x| x.to_owned()))
                        .unwrap_or_default();
                    (regex, input)
                };
                let ok = if regex == ".*" { true } else { regex == input };
                Some(JValue::Int(if ok { 1 } else { 0 }))
            }
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
                let len = this.borrow().as_java_string().map(|s| s.chars().count() as i32).unwrap_or(0);
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
                let s = this.borrow().as_java_string().unwrap_or("").to_owned();
                let char_len = s.chars().count();
                let begin = (_args.first().map(|v| v.as_int() as usize).unwrap_or(0)).min(char_len);
                let end = (_args.get(1).map(|v| v.as_int() as usize).unwrap_or(char_len)).min(char_len).max(begin);
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
                // fromIndex (char-index): default 0
                let from_char = _args.get(1).map(|v| v.as_int().max(0) as usize).unwrap_or(0);
                let from_byte = char_to_byte_offset(&s, from_char);
                let search_str = &s[from_byte..];
                let idx = match _args.first() {
                    Some(JValue::Ref(Some(r))) => {
                        let needle = r.borrow().as_java_string().unwrap_or("").to_owned();
                        search_str.find(needle.as_str()).map(|byte_pos| {
                            s[..from_byte + byte_pos].chars().count() as i32
                        }).unwrap_or(-1)
                    }
                    Some(JValue::Int(ch)) => {
                        let c = char::from_u32(*ch as u32).unwrap_or('\0');
                        search_str.find(c).map(|byte_pos| {
                            s[..from_byte + byte_pos].chars().count() as i32
                        }).unwrap_or(-1)
                    }
                    _ => -1,
                };
                Some(JValue::Int(idx))
            }
            ("java/lang/String", "lastIndexOf") => {
                let s = this.borrow().as_java_string().unwrap_or("").to_owned();
                // fromIndex (char-index): default = end of string
                let char_len = s.chars().count();
                let from_char = _args.get(1).map(|v| (v.as_int() as usize).min(char_len)).unwrap_or(char_len);
                let from_byte = char_to_byte_offset(&s, from_char);
                let search_str = &s[..from_byte];
                let idx = match _args.first() {
                    Some(JValue::Ref(Some(r))) => {
                        let needle = r.borrow().as_java_string().unwrap_or("").to_owned();
                        search_str.rfind(needle.as_str()).map(|byte_pos| {
                            s[..byte_pos].chars().count() as i32
                        }).unwrap_or(-1)
                    }
                    Some(JValue::Int(ch)) => {
                        let c = char::from_u32(*ch as u32).unwrap_or('\0');
                        search_str.rfind(c).map(|byte_pos| {
                            s[..byte_pos].chars().count() as i32
                        }).unwrap_or(-1)
                    }
                    _ => -1,
                };
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
            _ => None,
        }
    }
}
