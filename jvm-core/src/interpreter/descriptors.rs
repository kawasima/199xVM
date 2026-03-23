use crate::class_file::ConstantPoolEntry;
use crate::heap::{JObject, JRef, JValue};

// ---------------------------------------------------------------------------
// Numeric conversion (JVM spec)
// ---------------------------------------------------------------------------

/// JVM spec f2i: NaN→0, clamp to i32 range.
pub(super) fn float_to_int(v: f32) -> i32 {
    if v.is_nan() {
        0
    } else if v >= i32::MAX as f32 {
        i32::MAX
    } else if v <= i32::MIN as f32 {
        i32::MIN
    } else {
        v as i32
    }
}

/// JVM spec f2l: NaN→0, clamp to i64 range.
pub(super) fn float_to_long(v: f32) -> i64 {
    if v.is_nan() {
        0
    } else if v >= i64::MAX as f32 {
        i64::MAX
    } else if v <= i64::MIN as f32 {
        i64::MIN
    } else {
        v as i64
    }
}

/// JVM spec d2i: NaN→0, clamp to i32 range.
pub(super) fn double_to_int(v: f64) -> i32 {
    if v.is_nan() {
        0
    } else if v >= i32::MAX as f64 {
        i32::MAX
    } else if v <= i32::MIN as f64 {
        i32::MIN
    } else {
        v as i32
    }
}

/// JVM spec d2l: NaN→0, clamp to i64 range.
pub(super) fn double_to_long(v: f64) -> i64 {
    if v.is_nan() {
        0
    } else if v >= i64::MAX as f64 {
        i64::MAX
    } else if v <= i64::MIN as f64 {
        i64::MIN
    } else {
        v as i64
    }
}

// ---------------------------------------------------------------------------
// Bytecode operand reading
// ---------------------------------------------------------------------------

pub(super) fn read_i16(code: &[u8], pc: &mut usize) -> i16 {
    let hi = code[*pc] as i8 as i16;
    let lo = code[*pc + 1] as i16;
    *pc += 2;
    (hi << 8) | lo
}

pub(super) fn read_u16(code: &[u8], pc: &mut usize) -> u16 {
    let v = u16::from_be_bytes([code[*pc], code[*pc + 1]]);
    *pc += 2;
    v
}

pub(super) fn read_i32(code: &[u8], pc: &mut usize) -> i32 {
    let v = i32::from_be_bytes([code[*pc], code[*pc + 1], code[*pc + 2], code[*pc + 3]]);
    *pc += 4;
    v
}

// ---------------------------------------------------------------------------
// Constant pool resolution
// ---------------------------------------------------------------------------

pub(super) fn resolve_class_name(cp: &[ConstantPoolEntry], idx: u16) -> String {
    match &cp[idx as usize] {
        ConstantPoolEntry::Class { name_index } => match &cp[*name_index as usize] {
            ConstantPoolEntry::Utf8(s) => s.clone(),
            _ => String::new(),
        },
        _ => String::new(),
    }
}

pub(super) fn resolve_methodref(cp: &[ConstantPoolEntry], idx: u16) -> (String, String, String) {
    let (class_idx, nat_idx) = match &cp[idx as usize] {
        ConstantPoolEntry::Methodref {
            class_index,
            name_and_type_index,
        }
        | ConstantPoolEntry::InterfaceMethodref {
            class_index,
            name_and_type_index,
        } => (*class_index, *name_and_type_index),
        _ => return (String::new(), String::new(), String::new()),
    };
    let class_name = resolve_class_name(cp, class_idx);
    let (name, desc) = match &cp[nat_idx as usize] {
        ConstantPoolEntry::NameAndType {
            name_index,
            descriptor_index,
        } => {
            let n = match &cp[*name_index as usize] {
                ConstantPoolEntry::Utf8(s) => s.clone(),
                _ => String::new(),
            };
            let d = match &cp[*descriptor_index as usize] {
                ConstantPoolEntry::Utf8(s) => s.clone(),
                _ => String::new(),
            };
            (n, d)
        }
        _ => (String::new(), String::new()),
    };
    (class_name, name, desc)
}

pub(super) fn resolve_fieldref(cp: &[ConstantPoolEntry], idx: u16) -> (String, String, String) {
    let (class_idx, nat_idx) = match &cp[idx as usize] {
        ConstantPoolEntry::Fieldref {
            class_index,
            name_and_type_index,
        } => (*class_index, *name_and_type_index),
        _ => return (String::new(), String::new(), String::new()),
    };
    let class_name = resolve_class_name(cp, class_idx);
    let (name, desc) = match &cp[nat_idx as usize] {
        ConstantPoolEntry::NameAndType {
            name_index,
            descriptor_index,
        } => {
            let n = match &cp[*name_index as usize] {
                ConstantPoolEntry::Utf8(s) => s.clone(),
                _ => String::new(),
            };
            let d = match &cp[*descriptor_index as usize] {
                ConstantPoolEntry::Utf8(s) => s.clone(),
                _ => String::new(),
            };
            (n, d)
        }
        _ => (String::new(), String::new()),
    };
    (class_name, name, desc)
}

// ---------------------------------------------------------------------------
// Descriptor utilities
// ---------------------------------------------------------------------------

/// Return the default zero-value for a JVM field descriptor.
pub(super) fn default_value_for_descriptor(desc: &str) -> JValue {
    match desc.as_bytes().first() {
        Some(b'I') | Some(b'B') | Some(b'C') | Some(b'S') | Some(b'Z') => JValue::Int(0),
        Some(b'J') => JValue::Long(0),
        Some(b'F') => JValue::Float(0.0),
        Some(b'D') => JValue::Double(0.0),
        _ => JValue::Ref(None), // Object types default to null
    }
}

/// Extract a class name from a JVM field descriptor.
pub(super) fn descriptor_to_class_name(desc: &str) -> Option<String> {
    match desc.as_bytes().first()? {
        b'L' => {
            let inner = &desc[1..desc.len().checked_sub(1).unwrap_or(1)];
            Some(inner.to_string())
        }
        b'[' => Some(desc.to_string()),
        _ => None,
    }
}

/// Count the number of method arguments from a JVM method descriptor.
pub(super) fn count_args(descriptor: &str) -> usize {
    let mut count = 0usize;
    let mut chars = descriptor.chars().peekable();
    if chars.next() != Some('(') {
        return 0;
    }
    loop {
        match chars.peek().copied() {
            Some(')') | None => break,
            Some('L') => {
                chars.next(); // consume 'L'
                for c in chars.by_ref() {
                    if c == ';' {
                        break;
                    }
                }
                count += 1;
            }
            Some('[') => {
                // Consume all leading '[' (multi-dimensional arrays)
                while chars.peek() == Some(&'[') {
                    chars.next();
                }
                // Consume the element type
                if chars.peek() == Some(&'L') {
                    chars.next(); // consume 'L'
                    for c in chars.by_ref() {
                        if c == ';' {
                            break;
                        }
                    }
                } else {
                    chars.next(); // consume primitive type char
                }
                count += 1;
            }
            Some(_) => {
                chars.next();
                count += 1;
            }
        }
    }
    count
}

/// Extract the first character of each argument type in a method descriptor.
pub(super) fn arg_type_chars(descriptor: &str) -> Vec<char> {
    let mut types = Vec::new();
    let mut chars = descriptor.chars().peekable();
    if chars.next() != Some('(') {
        return types;
    }
    loop {
        match chars.peek().copied() {
            Some(')') | None => break,
            Some('L') => {
                chars.next(); // consume 'L'
                for c in chars.by_ref() {
                    if c == ';' {
                        break;
                    }
                }
                types.push('L');
            }
            Some('[') => {
                // Consume all leading '[' (multi-dimensional arrays)
                while chars.peek() == Some(&'[') {
                    chars.next();
                }
                // Consume the element type
                if chars.peek() == Some(&'L') {
                    chars.next(); // consume 'L'
                    for c in chars.by_ref() {
                        if c == ';' {
                            break;
                        }
                    }
                } else {
                    chars.next(); // consume primitive type char
                }
                types.push('[');
            }
            Some(c) => {
                chars.next();
                types.push(c);
            }
        }
    }
    types
}

pub(super) fn method_return_descriptor(descriptor: &str) -> Option<&str> {
    descriptor.split_once(')').map(|(_, ret)| ret)
}

pub(super) fn is_reference_descriptor(desc: &str) -> bool {
    matches!(desc.as_bytes().first(), Some(b'L' | b'['))
}

// ---------------------------------------------------------------------------
// Vm methods for type conversion and descriptor handling
// ---------------------------------------------------------------------------

use super::{ResolvedMemberRef, ResolvedStaticCallSite, Vm};

impl Vm {
    pub(super) fn resolve_classref_cached(
        &mut self,
        cp: &[ConstantPoolEntry],
        idx: u16,
    ) -> std::rc::Rc<str> {
        self.increment_profile_counter("cp.class.resolve");
        let cache_key = (cp.as_ptr() as usize, idx);
        if let Some(cached) = self.classref_constant_cache.get(&cache_key) {
            return cached.clone();
        }
        let resolved = std::rc::Rc::<str>::from(resolve_class_name(cp, idx));
        self.classref_constant_cache
            .insert(cache_key, resolved.clone());
        resolved
    }

    pub(super) fn resolve_methodref_cached(
        &mut self,
        cp: &[ConstantPoolEntry],
        idx: u16,
    ) -> std::rc::Rc<ResolvedMemberRef> {
        self.increment_profile_counter("cp.method.resolve");
        let cache_key = (cp.as_ptr() as usize, idx);
        if let Some(cached) = self.methodref_constant_cache.get(&cache_key) {
            return cached.clone();
        }
        let (class_name, member_name, descriptor) = resolve_methodref(cp, idx);
        let resolved = std::rc::Rc::new(ResolvedMemberRef {
            class_name,
            member_name,
            arg_count: count_args(&descriptor),
            returns_void: descriptor.ends_with(")V"),
            descriptor,
            instance_field_slot: None,
        });
        self.methodref_constant_cache
            .insert(cache_key, resolved.clone());
        resolved
    }

    pub(super) fn resolve_fieldref_cached(
        &mut self,
        cp: &[ConstantPoolEntry],
        idx: u16,
    ) -> std::rc::Rc<ResolvedMemberRef> {
        self.increment_profile_counter("cp.field.resolve");
        let cache_key = (cp.as_ptr() as usize, idx);
        if let Some(cached) = self.fieldref_constant_cache.get(&cache_key) {
            return cached.clone();
        }
        let (class_name, member_name, descriptor) = resolve_fieldref(cp, idx);
        let instance_field_slot =
            self.resolve_instance_field_slot(&class_name, &member_name, &descriptor);
        let resolved = std::rc::Rc::new(ResolvedMemberRef {
            class_name,
            member_name,
            arg_count: 0,
            returns_void: false,
            descriptor,
            instance_field_slot,
        });
        self.fieldref_constant_cache
            .insert(cache_key, resolved.clone());
        resolved
    }

    pub(super) fn resolve_static_callsite_cached(
        &mut self,
        cp: &[ConstantPoolEntry],
        idx: u16,
    ) -> Option<std::rc::Rc<ResolvedStaticCallSite>> {
        let cache_key = (cp.as_ptr() as usize, idx);
        if let Some(cached) = self.static_callsite_cache.get(&cache_key) {
            return Some(cached.clone());
        }
        let member = self.resolve_methodref_cached(cp, idx);
        let (resolved_descriptor, method_flags) = self.resolve_method_signature(
            &member.class_name,
            &member.member_name,
            &member.descriptor,
        )?;
        let method_info = std::rc::Rc::new(self.resolve_method_exec_info(
            &member.class_name,
            &member.member_name,
            &resolved_descriptor,
        )?);
        let expected_arg_count = method_info.param_tokens.len();
        let resolved = std::rc::Rc::new(ResolvedStaticCallSite {
            method_name: member.member_name.clone(),
            call_arg_count: member.arg_count,
            expected_arg_count: method_info.arg_count,
            push_return: !method_info.returns_void,
            empty_varargs: method_flags & 0x0080 != 0
                && member.arg_count < expected_arg_count
                && expected_arg_count - member.arg_count == 1,
            method_info,
        });
        self.static_callsite_cache
            .insert(cache_key, resolved.clone());
        Some(resolved)
    }

    pub(super) fn resolve_special_callsite_cached(
        &mut self,
        cp: &[ConstantPoolEntry],
        idx: u16,
    ) -> Option<std::rc::Rc<super::ResolvedSpecialCallSite>> {
        let cache_key = (cp.as_ptr() as usize, idx);
        if let Some(cached) = self.special_callsite_cache.get(&cache_key) {
            return Some(cached.clone());
        }
        let member = self.resolve_methodref_cached(cp, idx);
        let (resolved_descriptor, _) = self.resolve_method_signature(
            &member.class_name,
            &member.member_name,
            &member.descriptor,
        )?;
        let method_info = std::rc::Rc::new(self.resolve_method_exec_info(
            &member.class_name,
            &member.member_name,
            &resolved_descriptor,
        )?);
        let resolved = std::rc::Rc::new(super::ResolvedSpecialCallSite {
            method_name: member.member_name.clone(),
            push_return: !method_info.returns_void,
            no_effect_constructor: member.member_name == "<init>"
                && self.is_no_effect_constructor(&member.class_name, &resolved_descriptor),
            method_info,
        });
        self.special_callsite_cache
            .insert(cache_key, resolved.clone());
        Some(resolved)
    }

    pub(super) fn class_internal_name_from_obj(&mut self, class_obj: &JRef) -> Option<String> {
        self.class_target_from_mirror(class_obj).map(|(_, name)| name)
    }

    pub(super) fn class_display_name(internal_name: &str) -> String {
        internal_name.replace('/', ".")
    }

    pub(super) fn class_internal_name_from_runtime_name(name: &str) -> String {
        name.replace('.', "/")
    }

    pub(super) fn descriptor_to_runtime_class_name(desc: &str) -> String {
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

    pub(super) fn parse_method_descriptor(desc: &str) -> (Vec<String>, String) {
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

    pub(super) fn parse_method_descriptor_tokens(desc: &str) -> (Vec<String>, String) {
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
        let ret = if i <= desc.len() {
            desc[i..].to_owned()
        } else {
            "V".to_owned()
        };
        (params, ret)
    }

    pub(super) fn wrap_primitive_value(&self, value: JValue) -> JValue {
        match value {
            JValue::Int(i) => {
                let obj = JObject::new("java/lang/Integer");
                obj.borrow_mut()
                    .fields
                    .insert("value".to_owned(), JValue::Int(i));
                JValue::Ref(Some(obj))
            }
            JValue::Long(l) => {
                let obj = JObject::new("java/lang/Long");
                obj.borrow_mut()
                    .fields
                    .insert("value".to_owned(), JValue::Long(l));
                JValue::Ref(Some(obj))
            }
            JValue::Float(f) => {
                let obj = JObject::new("java/lang/Float");
                obj.borrow_mut()
                    .fields
                    .insert("value".to_owned(), JValue::Float(f));
                JValue::Ref(Some(obj))
            }
            JValue::Double(d) => {
                let obj = JObject::new("java/lang/Double");
                obj.borrow_mut()
                    .fields
                    .insert("value".to_owned(), JValue::Double(d));
                JValue::Ref(Some(obj))
            }
            other => other,
        }
    }

    pub(super) fn wrap_primitive_value_for_descriptor(&self, desc: &str, value: JValue) -> JValue {
        match desc.as_bytes().first().copied() {
            Some(b'Z') => {
                let i = self.adapt_value_for_descriptor("Z", value).as_int();
                let obj = JObject::new("java/lang/Boolean");
                obj.borrow_mut()
                    .fields
                    .insert("value".to_owned(), JValue::Int(if i == 0 { 0 } else { 1 }));
                JValue::Ref(Some(obj))
            }
            Some(b'B') => {
                let i = self.adapt_value_for_descriptor("B", value).as_int();
                let obj = JObject::new("java/lang/Byte");
                obj.borrow_mut()
                    .fields
                    .insert("value".to_owned(), JValue::Int(i));
                JValue::Ref(Some(obj))
            }
            Some(b'C') => {
                let i = self.adapt_value_for_descriptor("C", value).as_int();
                let obj = JObject::new("java/lang/Character");
                obj.borrow_mut()
                    .fields
                    .insert("value".to_owned(), JValue::Int(i));
                JValue::Ref(Some(obj))
            }
            Some(b'S') => {
                let i = self.adapt_value_for_descriptor("S", value).as_int();
                let obj = JObject::new("java/lang/Short");
                obj.borrow_mut()
                    .fields
                    .insert("value".to_owned(), JValue::Int(i));
                JValue::Ref(Some(obj))
            }
            Some(b'I') => {
                let i = self.adapt_value_for_descriptor("I", value).as_int();
                let obj = JObject::new("java/lang/Integer");
                obj.borrow_mut()
                    .fields
                    .insert("value".to_owned(), JValue::Int(i));
                JValue::Ref(Some(obj))
            }
            Some(b'J') => {
                let l = self.adapt_value_for_descriptor("J", value).as_long();
                let obj = JObject::new("java/lang/Long");
                obj.borrow_mut()
                    .fields
                    .insert("value".to_owned(), JValue::Long(l));
                JValue::Ref(Some(obj))
            }
            Some(b'F') => {
                let f = self.adapt_value_for_descriptor("F", value).as_float();
                let obj = JObject::new("java/lang/Float");
                obj.borrow_mut()
                    .fields
                    .insert("value".to_owned(), JValue::Float(f));
                JValue::Ref(Some(obj))
            }
            Some(b'D') => {
                let d = self.adapt_value_for_descriptor("D", value).as_double();
                let obj = JObject::new("java/lang/Double");
                obj.borrow_mut()
                    .fields
                    .insert("value".to_owned(), JValue::Double(d));
                JValue::Ref(Some(obj))
            }
            _ => value,
        }
    }

    pub(super) fn unwrap_boxed_primitive(&self, value: &JValue) -> Option<JValue> {
        let r = value.as_ref()?;
        let obj = r.borrow();
        match obj.class_name.as_str() {
            "java/lang/Integer" | "java/lang/Byte" | "java/lang/Short" | "java/lang/Character" => {
                obj.fields.get("value").cloned().map(|v| match v {
                    JValue::Int(i) => JValue::Int(i),
                    _ => JValue::Int(0),
                })
            }
            "java/lang/Boolean" => obj.fields.get("value").cloned().map(|v| match v {
                JValue::Int(i) => JValue::Int(if i == 0 { 0 } else { 1 }),
                _ => JValue::Int(0),
            }),
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

    pub(super) fn adapt_value_for_descriptor(&self, desc: &str, value: JValue) -> JValue {
        match desc.as_bytes().first().copied() {
            Some(b'Z') | Some(b'B') | Some(b'C') | Some(b'S') | Some(b'I') => match value {
                JValue::Int(i) => JValue::Int(i),
                JValue::Long(l) => JValue::Int(l as i32),
                JValue::Float(f) => JValue::Int(f as i32),
                JValue::Double(d) => JValue::Int(d as i32),
                JValue::Ref(_) => self
                    .unwrap_boxed_primitive(&value)
                    .map(|v| self.adapt_value_for_descriptor("I", v))
                    .unwrap_or(JValue::Int(0)),
                _ => JValue::Int(0),
            },
            Some(b'J') => match value {
                JValue::Long(l) => JValue::Long(l),
                JValue::Int(i) => JValue::Long(i as i64),
                JValue::Ref(_) => self
                    .unwrap_boxed_primitive(&value)
                    .map(|v| self.adapt_value_for_descriptor("J", v))
                    .unwrap_or(JValue::Long(0)),
                _ => JValue::Long(0),
            },
            Some(b'F') => match value {
                JValue::Float(f) => JValue::Float(f),
                JValue::Double(d) => JValue::Float(d as f32),
                JValue::Int(i) => JValue::Float(i as f32),
                JValue::Long(l) => JValue::Float(l as f32),
                JValue::Ref(_) => self
                    .unwrap_boxed_primitive(&value)
                    .map(|v| self.adapt_value_for_descriptor("F", v))
                    .unwrap_or(JValue::Float(0.0)),
                _ => JValue::Float(0.0),
            },
            Some(b'D') => match value {
                JValue::Double(d) => JValue::Double(d),
                JValue::Float(f) => JValue::Double(f as f64),
                JValue::Int(i) => JValue::Double(i as f64),
                JValue::Long(l) => JValue::Double(l as f64),
                JValue::Ref(_) => self
                    .unwrap_boxed_primitive(&value)
                    .map(|v| self.adapt_value_for_descriptor("D", v))
                    .unwrap_or(JValue::Double(0.0)),
                _ => JValue::Double(0.0),
            },
            Some(b'L') | Some(b'[') => match value {
                JValue::Ref(_) => value,
                primitive => self.wrap_primitive_value(primitive),
            },
            _ => value,
        }
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
        assert_eq!(
            count_args("(Ljava/lang/Object;Lnet/unit8/raoh/Path;)Lnet/unit8/raoh/Result;"),
            2
        );
    }
}
