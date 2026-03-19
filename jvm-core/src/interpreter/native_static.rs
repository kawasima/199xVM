use std::rc::Rc;
use crate::heap::{JObject, JValue, NativePayload};

/// Returns true if `regex` ends with an unescaped `$` anchor.
/// A `$` is escaped when preceded by an odd number of backslashes.
fn ends_with_unescaped_dollar(regex: &str) -> bool {
    if !regex.ends_with('$') {
        return false;
    }
    let trailing_backslashes = regex
        .chars()
        .rev()
        .skip(1)
        .take_while(|&c| c == '\\')
        .count();
    trailing_backslashes % 2 == 0
}

/// Perform a full-string regex match as Java's `Matcher.matches()` requires.
///
/// Always wraps the pattern with `^(?:...)$` to enforce full-string semantics.
/// To avoid `^(?:^...$)$` (which breaks Rust's regex engine), a leading `^`
/// and a trailing unescaped `$` are stripped before wrapping.  This correctly
/// handles alternations like `^foo$|bar$` — the stripped form `foo$|bar` is
/// re-anchored as `^(?:foo$|bar)$`, so `"xxbar"` no longer matches.
pub(super) fn regex_full_match(regex: &str, input: &str) -> bool {
    if regex == ".*" {
        return true;
    }
    let stripped_start = regex.strip_prefix('^').unwrap_or(regex);
    let stripped = if ends_with_unescaped_dollar(stripped_start) {
        &stripped_start[..stripped_start.len() - 1]
    } else {
        stripped_start
    };
    let anchored = format!("^(?:{stripped})$");
    regex::Regex::new(&anchored)
        .map(|re| re.is_match(input))
        .unwrap_or_else(|e| {
            eprintln!("Unsupported regex pattern '{regex}': {e}");
            false
        })
}

impl super::Vm {
    pub(super) fn native_static(
        &mut self,
        _class_name: &str,
        _method_name: &str,
        _descriptor: &str,
        _args: &[JValue],
    ) -> Option<JValue> {
        match (_class_name, _method_name, _descriptor) {
            ("java/util/regex/Pattern", "compile", "(Ljava/lang/String;)Ljava/util/regex/Pattern;") => {
                let regex = _args
                    .first()
                    .and_then(|v| v.as_ref())
                    .and_then(|r| r.borrow().as_java_string().map(|s| s.to_owned()))
                    .unwrap_or_default();
                let p = JObject::new("java/util/regex/Pattern");
                p.borrow_mut().fields.insert("__regex".to_owned(), JValue::Ref(Some(self.intern_string(regex))));
                p.borrow_mut().fields.insert("__flags".to_owned(), JValue::Int(0));
                Some(JValue::Ref(Some(p)))
            }
            ("java/util/regex/Pattern", "compile", "(Ljava/lang/String;I)Ljava/util/regex/Pattern;") => {
                let regex = _args
                    .first()
                    .and_then(|v| v.as_ref())
                    .and_then(|r| r.borrow().as_java_string().map(|s| s.to_owned()))
                    .unwrap_or_default();
                let flags = _args.get(1).map(|v| v.as_int()).unwrap_or(0);
                let p = JObject::new("java/util/regex/Pattern");
                p.borrow_mut().fields.insert("__regex".to_owned(), JValue::Ref(Some(self.intern_string(regex))));
                p.borrow_mut().fields.insert("__flags".to_owned(), JValue::Int(flags));
                Some(JValue::Ref(Some(p)))
            }
            ("java/util/regex/Pattern", "matches", "(Ljava/lang/String;Ljava/lang/CharSequence;)Z") => {
                let regex = _args
                    .first()
                    .and_then(|v| v.as_ref())
                    .and_then(|r| r.borrow().as_java_string().map(|s| s.to_owned()))
                    .unwrap_or_default();
                let input = _args
                    .get(1)
                    .and_then(|v| v.as_ref())
                    .and_then(|r| r.borrow().as_java_string().map(|s| s.to_owned()))
                    .unwrap_or_default();
                let ok = regex_full_match(&regex, &input);
                Some(JValue::Int(if ok { 1 } else { 0 }))
            }
            ("java/util/regex/Matcher", "nativeMatches", "(Ljava/lang/String;Ljava/lang/String;)Z") => {
                let regex = _args
                    .first()
                    .and_then(|v| v.as_ref())
                    .and_then(|r| r.borrow().as_java_string().map(|s| s.to_owned()))
                    .unwrap_or_default();
                let input = _args
                    .get(1)
                    .and_then(|v| v.as_ref())
                    .and_then(|r| r.borrow().as_java_string().map(|s| s.to_owned()))
                    .unwrap_or_default();
                let ok = regex_full_match(&regex, &input);
                Some(JValue::Int(if ok { 1 } else { 0 }))
            }
            ("java/util/Arrays", "hashCode", "([Ljava/lang/Object;)I") => {
                let Some(arr_ref) = _args.first().and_then(|v| v.as_ref()).cloned() else {
                    return Some(JValue::Int(0));
                };
                let elems: Vec<JValue> = match &arr_ref.borrow().native {
                    NativePayload::Array(v) => v.clone(),
                    _ => return Some(JValue::Int(0)),
                };
                let mut result: i32 = 1;
                for e in elems {
                    let h = match e {
                        JValue::Ref(None) => 0,
                        JValue::Ref(Some(r)) => {
                            let cls = r.borrow().class_name.clone();
                            match self.invoke_virtual(r, &cls, "hashCode", "()I", vec![]) {
                                Ok(JValue::Int(i)) => i,
                                Ok(v) => self.adapt_value_for_descriptor("I", v).as_int(),
                                Err(_) => 0,
                            }
                        }
                        JValue::Int(i) => i,
                        JValue::Long(l) => (l ^ (l >> 32)) as i32,
                        JValue::Float(f) => f.to_bits() as i32,
                        JValue::Double(d) => {
                            let bits = d.to_bits();
                            (bits ^ (bits >> 32)) as i32
                        }
                        _ => 0,
                    };
                    result = result.wrapping_mul(31).wrapping_add(h);
                }
                Some(JValue::Int(result))
            }
            ("java/util/Arrays", "hashCode", "([I)I") => {
                let Some(arr_ref) = _args.first().and_then(|v| v.as_ref()).cloned() else {
                    return Some(JValue::Int(0));
                };
                let mut result: i32 = 1;
                match &arr_ref.borrow().native {
                    NativePayload::IntArray(v) => {
                        for &e in v {
                            result = result.wrapping_mul(31).wrapping_add(e);
                        }
                    }
                    NativePayload::Array(v) => {
                        for e in v {
                            result = result.wrapping_mul(31).wrapping_add(e.as_int());
                        }
                    }
                    _ => return Some(JValue::Int(0)),
                }
                Some(JValue::Int(result))
            }
            ("java/util/Arrays", "hashCode", "([J)I") => {
                let Some(arr_ref) = _args.first().and_then(|v| v.as_ref()).cloned() else {
                    return Some(JValue::Int(0));
                };
                let mut result: i32 = 1;
                match &arr_ref.borrow().native {
                    NativePayload::LongArray(v) => {
                        for &e in v {
                            let h = (e ^ (e >> 32)) as i32;
                            result = result.wrapping_mul(31).wrapping_add(h);
                        }
                    }
                    NativePayload::Array(v) => {
                        for e in v {
                            let lv = e.as_long();
                            let h = (lv ^ (lv >> 32)) as i32;
                            result = result.wrapping_mul(31).wrapping_add(h);
                        }
                    }
                    _ => return Some(JValue::Int(0)),
                }
                Some(JValue::Int(result))
            }
            ("java/util/Arrays", "hashCode", "([B)I") => {
                let Some(arr_ref) = _args.first().and_then(|v| v.as_ref()).cloned() else {
                    return Some(JValue::Int(0));
                };
                let mut result: i32 = 1;
                match &arr_ref.borrow().native {
                    NativePayload::ByteArray(v) => {
                        for &e in v {
                            result = result.wrapping_mul(31).wrapping_add((e as i8) as i32);
                        }
                    }
                    NativePayload::Array(v) => {
                        for e in v {
                            let b = e.as_int() as i8 as i32;
                            result = result.wrapping_mul(31).wrapping_add(b);
                        }
                    }
                    _ => return Some(JValue::Int(0)),
                }
                Some(JValue::Int(result))
            }
            ("java/lang/Class", "getPrimitiveClass", "(Ljava/lang/String;)Ljava/lang/Class;") => {
                let name = match _args
                    .first()
                    .and_then(|v| v.as_ref())
                    .and_then(|r| r.borrow().as_java_string().map(|s| s.to_owned()))
                {
                    Some(n) => n,
                    None => {
                        self.throw_null_pointer("Class.getPrimitiveClass: name is null");
                        return Some(JValue::Void);
                    }
                };
                Some(JValue::Ref(Some(self.class_object(name))))
            }
            ("java/lang/Class", "forName0", "(Ljava/lang/String;)Ljava/lang/Class;") => {
                let runtime_name = match _args
                    .first()
                    .and_then(|v| v.as_ref())
                    .and_then(|r| r.borrow().as_java_string().map(|s| s.to_owned()))
                {
                    Some(n) => n,
                    None => {
                        self.throw_null_pointer("Class.forName: className is null");
                        return Some(JValue::Void);
                    }
                };
                let internal = Self::class_internal_name_from_runtime_name(&runtime_name);
                // Primitive class names are VM-defined synthetic Class objects.
                if matches!(
                    internal.as_str(),
                    "boolean" | "byte" | "char" | "short" | "int" | "long" | "float" | "double" | "void"
                ) {
                    return Some(JValue::Ref(Some(self.class_object(internal))));
                }
                // Array descriptors (e.g. "[I", "[Ljava/lang/String;") are synthetic types
                // not backed by a ClassFile entry — return a class object directly.
                if internal.starts_with('[') {
                    return Some(JValue::Ref(Some(self.class_object(internal))));
                }
                self.ensure_class_ready(&internal);
                match self.classes.get(&internal) {
                    Some(super::LazyClass::Ready(_)) => {}
                    Some(super::LazyClass::ParseError(msg)) => {
                        let msg = msg.clone();
                        self.throw_class_format_error(&msg);
                        return Some(JValue::Void);
                    }
                    _ => {
                        self.throw_class_not_found(&runtime_name);
                        return Some(JValue::Void);
                    }
                }
                Some(JValue::Ref(Some(self.class_object(internal))))
            }
            ("java/lang/Class", "forName1", "(Ljava/lang/String;ZLjava/lang/ClassLoader;)Ljava/lang/Class;") => {
                let runtime_name = match _args
                    .first()
                    .and_then(|v| v.as_ref())
                    .and_then(|r| r.borrow().as_java_string().map(|s| s.to_owned()))
                {
                    Some(n) => n,
                    None => {
                        self.throw_null_pointer("Class.forName: className is null");
                        return Some(JValue::Void);
                    }
                };
                let initialize = _args.get(1).map(|v| v.as_int() != 0).unwrap_or(true);
                let internal = Self::class_internal_name_from_runtime_name(&runtime_name);
                self.ensure_class_ready(&internal);
                match self.classes.get(&internal) {
                    Some(super::LazyClass::Ready(_)) => {}
                    Some(super::LazyClass::ParseError(msg)) => {
                        let msg = msg.clone();
                        self.throw_class_format_error(&msg);
                        return Some(JValue::Void);
                    }
                    _ => {
                        return Some(JValue::Ref(None)); // not found — caller checks null
                    }
                }
                if initialize {
                    if self.ensure_class_init(&internal).is_err() {
                        return Some(JValue::Void);
                    }
                }
                Some(JValue::Ref(Some(self.class_object(internal))))
            }
            ("java/lang/ClassLoader", "getSystemClassLoader", "()Ljava/lang/ClassLoader;") => {
                let cl = self.get_or_create_system_classloader();
                Some(JValue::Ref(Some(cl)))
            }
            ("java/lang/Double", "doubleToLongBits", "(D)J") | ("java/lang/Double", "doubleToRawLongBits", "(D)J") => {
                let d = _args.first().map(|v| v.as_double()).unwrap_or(0.0);
                Some(JValue::Long(d.to_bits() as i64))
            }
            ("java/lang/Double", "longBitsToDouble", "(J)D") => {
                let l = _args.first().map(|v| v.as_long()).unwrap_or(0);
                Some(JValue::Double(f64::from_bits(l as u64)))
            }
            ("java/lang/Float", "floatToRawIntBits", "(F)I") | ("java/lang/Float", "floatToIntBits", "(F)I") => {
                let f = _args.first().map(|v| v.as_float()).unwrap_or(0.0);
                Some(JValue::Int(f.to_bits() as i32))
            }
            ("java/lang/Float", "intBitsToFloat", "(I)F") => {
                let i = _args.first().map(|v| v.as_int()).unwrap_or(0);
                Some(JValue::Float(f32::from_bits(i as u32)))
            }
            ("java/lang/System", "currentTimeMillis", "()J") => {
                #[cfg(target_arch = "wasm32")]
                let ms = js_sys::Date::now() as i64;
                #[cfg(not(target_arch = "wasm32"))]
                let ms = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .ok()
                    .map(|d| d.as_millis() as i64)
                    .unwrap_or(0);
                Some(JValue::Long(ms))
            }
            ("java/lang/System", "nanoTime", "()J") => {
                #[cfg(target_arch = "wasm32")]
                let ns = (js_sys::Date::now() * 1_000_000.0) as i64;
                #[cfg(not(target_arch = "wasm32"))]
                let ns = {
                    use std::sync::OnceLock;
                    static EPOCH: OnceLock<std::time::Instant> = OnceLock::new();
                    let epoch = EPOCH.get_or_init(std::time::Instant::now);
                    epoch.elapsed().as_nanos() as i64
                };
                Some(JValue::Long(ns))
            }
            ("java/lang/System", "initProperties", "(Ljava/util/Properties;)V") => {
                // Populate system properties. The Properties object is passed as first arg.
                let props_ref = _args.first().and_then(|v| v.as_ref()).cloned();
                if let Some(props) = props_ref {
                    let sys_props = [
                        ("os.name", "199xVM"),
                        ("os.arch", "wasm"),
                        ("os.version", "1.0"),
                        ("file.separator", "/"),
                        ("path.separator", ":"),
                        ("line.separator", "\n"),
                        ("file.encoding", "UTF-8"),
                        ("java.version", "25"),
                        ("java.specification.version", "25"),
                        ("java.vm.name", "199xVM"),
                        ("java.vm.specification.version", "25"),
                        ("java.class.path", ""),
                        ("user.dir", "/"),
                        ("user.home", "/"),
                    ];
                    for (key, val) in sys_props {
                        let k = self.intern_string(key);
                        let v = self.intern_string(val);
                        // Call Properties.setProperty(String, String) via bytecode
                        let _ = self.invoke_virtual(
                            props.clone(),
                            "java/util/Properties",
                            "setProperty",
                            "(Ljava/lang/String;Ljava/lang/String;)Ljava/lang/Object;",
                            vec![JValue::Ref(Some(k)), JValue::Ref(Some(v))],
                        );
                    }
                }
                Some(JValue::Void)
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
            // ----- java.lang.Thread -----
            ("java/lang/Thread", "currentThread", "()Ljava/lang/Thread;") => {
                let obj = self.current_thread_object();
                Some(JValue::Ref(Some(obj)))
            }
            ("java/lang/Thread", "yield", "()V") => {
                // Cooperative yield — force immediate context switch by
                // temporarily marking the thread as non-Runnable. The scheduler
                // loop in run_all_threads will set it back to Runnable after
                // switching to another thread.
                use super::ThreadState;
                if self.scheduler.thread_count() > 1 {
                    self.scheduler.current_thread_mut().state = ThreadState::Yielded;
                }
                Some(JValue::Void)
            }
            ("java/lang/Thread", "sleep", "(J)V") => {
                // Validate: negative durations are illegal per JDK spec.
                let millis = _args.first().map(|v| match v {
                    JValue::Long(l) => *l,
                    JValue::Int(i) => *i as i64,
                    _ => 0,
                }).unwrap_or(0);
                if millis < 0 {
                    let msg = self.intern_string("sleep duration must be >= 0");
                    let exc = crate::heap::JObject::new("java/lang/IllegalArgumentException");
                    exc.borrow_mut().fields.insert("detailMessage".to_owned(), JValue::Ref(Some(msg)));
                    *self.pending_exception_mut() = Some(exc);
                    return Some(JValue::Void);
                }
                // In our cooperative model, sleep yields to other threads.
                // A full implementation would track wake-up times.
                use super::ThreadState;
                self.scheduler.current_thread_mut().state = ThreadState::Sleeping;
                Some(JValue::Void)
            }
            ("java/lang/Double", "toString", "(D)Ljava/lang/String;") => {
                let d = _args[0].as_double();
                let s = if d == d.floor() && d.is_finite() && d.abs() < 1e15 {
                    format!("{:.1}", d)
                } else {
                    format!("{}", d)
                };
                Some(JValue::Ref(Some(self.intern_string(s))))
            }
            ("java/lang/Float", "toString", "(F)Ljava/lang/String;") => {
                let f = match &_args[0] {
                    JValue::Float(v) => *v,
                    JValue::Int(v) => f32::from_bits(*v as u32),
                    other => panic!("Expected Float, got {other:?}"),
                };
                let s = if f == f.floor() && f.is_finite() && f.abs() < 1e7 {
                    format!("{:.1}", f)
                } else {
                    format!("{}", f)
                };
                Some(JValue::Ref(Some(self.intern_string(s))))
            }
            // ── java.lang.StrictMath ──────────────────────────────────
            ("java/lang/StrictMath", "sin", "(D)D") => {
                let a = _args[0].as_double();
                Some(JValue::Double(a.sin()))
            }
            ("java/lang/StrictMath", "cos", "(D)D") => {
                let a = _args[0].as_double();
                Some(JValue::Double(a.cos()))
            }
            ("java/lang/StrictMath", "tan", "(D)D") => {
                let a = _args[0].as_double();
                Some(JValue::Double(a.tan()))
            }
            ("java/lang/StrictMath", "asin", "(D)D") => {
                let a = _args[0].as_double();
                Some(JValue::Double(a.asin()))
            }
            ("java/lang/StrictMath", "acos", "(D)D") => {
                let a = _args[0].as_double();
                Some(JValue::Double(a.acos()))
            }
            ("java/lang/StrictMath", "atan", "(D)D") => {
                let a = _args[0].as_double();
                Some(JValue::Double(a.atan()))
            }
            ("java/lang/StrictMath", "exp", "(D)D") => {
                let a = _args[0].as_double();
                Some(JValue::Double(a.exp()))
            }
            ("java/lang/StrictMath", "log", "(D)D") => {
                let a = _args[0].as_double();
                Some(JValue::Double(a.ln()))
            }
            ("java/lang/StrictMath", "log10", "(D)D") => {
                let a = _args[0].as_double();
                Some(JValue::Double(a.log10()))
            }
            ("java/lang/StrictMath", "sqrt", "(D)D") => {
                let a = _args[0].as_double();
                Some(JValue::Double(a.sqrt()))
            }
            ("java/lang/StrictMath", "cbrt", "(D)D") => {
                let a = _args[0].as_double();
                Some(JValue::Double(a.cbrt()))
            }
            ("java/lang/StrictMath", "IEEEremainder", "(DD)D") => {
                let f1 = _args[0].as_double();
                let f2 = _args[1].as_double();
                // IEEE 754 remainder: f1 - f2 * rint(f1/f2), not truncated remainder (%)
                let quotient = f1 / f2;
                Some(JValue::Double(f1 - f2 * quotient.round_ties_even()))
            }
            ("java/lang/StrictMath", "ceil", "(D)D") => {
                let a = _args[0].as_double();
                Some(JValue::Double(a.ceil()))
            }
            ("java/lang/StrictMath", "floor", "(D)D") => {
                let a = _args[0].as_double();
                Some(JValue::Double(a.floor()))
            }
            ("java/lang/StrictMath", "rint", "(D)D") => {
                let a = _args[0].as_double();
                Some(JValue::Double(a.round_ties_even()))
            }
            ("java/lang/StrictMath", "atan2", "(DD)D") => {
                let y = _args[0].as_double();
                let x = _args[1].as_double();
                Some(JValue::Double(y.atan2(x)))
            }
            ("java/lang/StrictMath", "pow", "(DD)D") => {
                let a = _args[0].as_double();
                let b = _args[1].as_double();
                Some(JValue::Double(a.powf(b)))
            }
            ("java/lang/StrictMath", "sinh", "(D)D") => {
                let x = _args[0].as_double();
                Some(JValue::Double(x.sinh()))
            }
            ("java/lang/StrictMath", "cosh", "(D)D") => {
                let x = _args[0].as_double();
                Some(JValue::Double(x.cosh()))
            }
            ("java/lang/StrictMath", "tanh", "(D)D") => {
                let x = _args[0].as_double();
                Some(JValue::Double(x.tanh()))
            }
            ("java/lang/StrictMath", "hypot", "(DD)D") => {
                let x = _args[0].as_double();
                let y = _args[1].as_double();
                Some(JValue::Double(x.hypot(y)))
            }
            ("java/lang/StrictMath", "expm1", "(D)D") => {
                let x = _args[0].as_double();
                Some(JValue::Double(x.exp_m1()))
            }
            ("java/lang/StrictMath", "log1p", "(D)D") => {
                let x = _args[0].as_double();
                Some(JValue::Double(x.ln_1p()))
            }
            _ => None,
        }
    }
}
