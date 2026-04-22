use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

use crate::class_file::Attribute;
use crate::heap::{JavaStringValue, JObject, JRef, JValue, NativePayload};

use super::class_identity::DefineClassError;
use super::LazyClass;
use super::descriptors::*;
use super::native_static::{regex_encode_java_string, regex_full_match_source};

#[cfg(target_arch = "wasm32")]
use super::{console_error, console_log};

/// Convert a UTF-16 code unit index to a UTF-8 byte offset within `s`.
///
/// When the index lands inside a surrogate pair, round up to the next scalar
/// boundary so repeated regex searches can keep making progress.
fn utf16_index_to_byte_offset(s: &str, utf16_idx: usize) -> usize {
    if utf16_idx == 0 {
        return 0;
    }
    let mut byte_offset = 0usize;
    let mut code_units = 0usize;
    for ch in s.chars() {
        byte_offset += ch.len_utf8();
        code_units += ch.len_utf16();
        if code_units >= utf16_idx {
            return byte_offset;
        }
    }
    s.len()
}

/// Convert a UTF-16 code unit index to a UTF-8 byte offset only when it lands
/// on an exact scalar boundary. Returns `None` for indices inside surrogate
/// pairs, where Rust `str` cannot slice losslessly.
fn utf16_index_to_byte_offset_exact(s: &str, utf16_idx: usize) -> Option<usize> {
    let mut byte_offset = 0usize;
    let mut code_units = 0usize;
    if utf16_idx == 0 {
        return Some(0);
    }
    for ch in s.chars() {
        if code_units == utf16_idx {
            return Some(byte_offset);
        }
        let next_code_units = code_units + ch.len_utf16();
        if utf16_idx < next_code_units {
            return None;
        }
        code_units = next_code_units;
        byte_offset += ch.len_utf8();
    }
    (code_units == utf16_idx).then_some(byte_offset)
}

/// Convert a UTF-8 byte offset to a UTF-16 code unit index.
fn byte_offset_to_utf16_index(s: &str, byte_offset: usize) -> usize {
    let mut utf16_idx = 0usize;
    let mut bytes_seen = 0usize;
    for ch in s.chars() {
        if bytes_seen >= byte_offset {
            break;
        }
        bytes_seen += ch.len_utf8();
        utf16_idx += ch.len_utf16();
    }
    utf16_idx
}

fn arg_string_value(arg: &JValue) -> Option<JavaStringValue> {
    match arg {
        JValue::Ref(Some(r)) => r.borrow().as_java_string_value().cloned(),
        _ => None,
    }
}

fn u16_find(haystack: &[u16], needle: &[u16], from: usize) -> Option<usize> {
    let from = from.min(haystack.len());
    if needle.is_empty() {
        return Some(from);
    }
    if needle.len() > haystack.len() || from > haystack.len().saturating_sub(needle.len()) {
        return None;
    }
    let max = haystack.len() - needle.len();
    (from..=max).find(|&i| &haystack[i..i + needle.len()] == needle)
}

fn u16_last_index_of_unit(haystack: &[u16], needle: u16, from: usize) -> Option<usize> {
    if haystack.is_empty() {
        return None;
    }
    let start = from.min(haystack.len().saturating_sub(1));
    (0..=start).rev().find(|&i| haystack[i] == needle)
}

fn u16_rfind(haystack: &[u16], needle: &[u16], from: usize) -> Option<usize> {
    if needle.is_empty() {
        return Some(from.min(haystack.len()));
    }
    if needle.len() > haystack.len() {
        return None;
    }
    let start = from.min(haystack.len() - needle.len());
    (0..=start)
        .rev()
        .find(|&i| &haystack[i..i + needle.len()] == needle)
}

fn u16_replace(haystack: &[u16], needle: &[u16], replacement: &[u16]) -> Vec<u16> {
    if needle.is_empty() {
        return haystack.to_vec();
    }
    let mut out = Vec::with_capacity(haystack.len());
    let mut cursor = 0usize;
    while cursor <= haystack.len().saturating_sub(needle.len()) {
        if &haystack[cursor..cursor + needle.len()] == needle {
            out.extend_from_slice(replacement);
            cursor += needle.len();
        } else {
            out.push(haystack[cursor]);
            cursor += 1;
        }
    }
    out.extend_from_slice(&haystack[cursor..]);
    out
}

fn string_slice_value(value: &JavaStringValue, start: usize, end: usize) -> JavaStringValue {
    if let Some(text) = value.as_str() {
        if let (Some(start_byte), Some(end_byte)) = (
            utf16_index_to_byte_offset_exact(text, start),
            utf16_index_to_byte_offset_exact(text, end),
        ) {
            return JavaStringValue::new(text[start_byte..end_byte].to_owned());
        }
    }
    JavaStringValue::from_utf16(value.slice_utf16(start, end))
}

fn string_index_of_value(haystack: &JavaStringValue, needle: &JavaStringValue, from_index: usize) -> Option<usize> {
    if let (Some(hs), Some(ns)) = (haystack.as_str(), needle.as_str()) {
        let start_byte = utf16_index_to_byte_offset(hs, from_index);
        return hs[start_byte..]
            .find(ns)
            .map(|byte_idx| byte_offset_to_utf16_index(hs, start_byte + byte_idx));
    }
    u16_find(haystack.utf16(), needle.utf16(), from_index)
}

impl super::Vm {
    /// Extract UTF-16-backed string content from `java/lang/String` constructor arguments.
    pub(super) fn string_from_init_args(&self, descriptor: &str, args: &[JValue], _this: &JRef) -> JavaStringValue {
        match descriptor {
            "()V" => JavaStringValue::from_utf16(Vec::new()),
            "([C)V" => {
                // String(char[])
                if let Some(r) = args.first().and_then(|a| a.as_ref()) {
                    if let NativePayload::Array(chars) = &r.borrow().native {
                        JavaStringValue::from_utf16(
                            chars.iter().map(|v| v.as_int() as u16).collect(),
                        )
                    } else { JavaStringValue::from_utf16(Vec::new()) }
                } else { JavaStringValue::from_utf16(Vec::new()) }
            }
            "([CII)V" => {
                // String(char[], offset, count)
                if let Some(r) = args.first().and_then(|a| a.as_ref()) {
                    let offset = args.get(1).map(|a| a.as_int().max(0) as usize).unwrap_or(0);
                    let count = args.get(2).map(|a| a.as_int().max(0) as usize).unwrap_or(0);
                    if let NativePayload::Array(chars) = &r.borrow().native {
                        let end = offset.saturating_add(count).min(chars.len());
                        JavaStringValue::from_utf16(
                            chars[offset.min(chars.len())..end]
                                .iter()
                                .map(|v| v.as_int() as u16)
                                .collect(),
                        )
                    } else { JavaStringValue::from_utf16(Vec::new()) }
                } else { JavaStringValue::from_utf16(Vec::new()) }
            }
            "([B)V" => {
                // String(byte[])
                if let Some(r) = args.first().and_then(|a| a.as_ref()) {
                    if let NativePayload::Array(bytes) = &r.borrow().native {
                        JavaStringValue::from_utf16(
                            bytes.iter().map(|v| v.as_int() as u8 as u16).collect(),
                        )
                    } else { JavaStringValue::from_utf16(Vec::new()) }
                } else { JavaStringValue::from_utf16(Vec::new()) }
            }
            "([BII)V" | "([BIILjava/lang/String;)V" | "([BIILjava/nio/charset/Charset;)V" => {
                if let Some(r) = args.first().and_then(|a| a.as_ref()) {
                    let offset = args.get(1).map(|a| a.as_int().max(0) as usize).unwrap_or(0);
                    let count = args.get(2).map(|a| a.as_int().max(0) as usize).unwrap_or(0);
                    if let NativePayload::Array(bytes) = &r.borrow().native {
                        let end = offset.saturating_add(count).min(bytes.len());
                        JavaStringValue::from_utf16(
                            bytes[offset.min(bytes.len())..end]
                                .iter()
                                .map(|v| v.as_int() as u8 as u16)
                                .collect(),
                        )
                    } else { JavaStringValue::from_utf16(Vec::new()) }
                } else { JavaStringValue::from_utf16(Vec::new()) }
            }
            "([BLjava/lang/String;)V" | "([BLjava/nio/charset/Charset;)V" => {
                if let Some(r) = args.first().and_then(|a| a.as_ref()) {
                    if let NativePayload::Array(bytes) = &r.borrow().native {
                        JavaStringValue::from_utf16(
                            bytes.iter().map(|v| v.as_int() as u8 as u16).collect(),
                        )
                    } else { JavaStringValue::from_utf16(Vec::new()) }
                } else { JavaStringValue::from_utf16(Vec::new()) }
            }
            "([BIII)V" => {
                if let Some(r) = args.first().and_then(|a| a.as_ref()) {
                    let _hibyte = args.get(1).map(|a| a.as_int()).unwrap_or(0);
                    let offset = args.get(2).map(|a| a.as_int().max(0) as usize).unwrap_or(0);
                    let count = args.get(3).map(|a| a.as_int().max(0) as usize).unwrap_or(0);
                    if let NativePayload::Array(bytes) = &r.borrow().native {
                        let end = offset.saturating_add(count).min(bytes.len());
                        JavaStringValue::from_utf16(
                            bytes[offset.min(bytes.len())..end]
                                .iter()
                                .map(|v| v.as_int() as u8 as u16)
                                .collect(),
                        )
                    } else { JavaStringValue::from_utf16(Vec::new()) }
                } else { JavaStringValue::from_utf16(Vec::new()) }
            }
            "(Ljava/lang/String;)V" => {
                // String(String) — copy constructor
                if let Some(r) = args.first().and_then(|a| a.as_ref()) {
                    r.borrow()
                        .as_java_string_value()
                        .cloned()
                        .unwrap_or_else(|| JavaStringValue::from_utf16(Vec::new()))
                } else { JavaStringValue::from_utf16(Vec::new()) }
            }
            _ => JavaStringValue::from_utf16(Vec::new()),
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

    pub(super) fn write_printstream_bytes(&mut self, is_err: bool, bytes: &[u8]) {
        let mode = if is_err {
            self.stderr_mode
        } else {
            self.stdout_mode
        };
        match mode {
            super::StdioMode::Ignore => {}
            super::StdioMode::Pipe => {
                if !bytes.is_empty() {
                    let chunks = if is_err {
                        &mut self.stderr_chunks
                    } else {
                        &mut self.stdout_chunks
                    };
                    chunks.push_back(bytes.to_vec());
                }
            }
            super::StdioMode::Inherit => {
                if bytes.is_empty() {
                    return;
                }
                let text = String::from_utf8_lossy(bytes);
                let buf = if is_err {
                    &mut self.stderr_buffer
                } else {
                    &mut self.stdout_buffer
                };
                buf.push_str(&text);
                while let Some(pos) = buf.find('\n') {
                    let line = buf[..pos].to_owned();
                    Self::emit_host_line(is_err, &line);
                    buf.drain(..=pos);
                }
            }
        }
    }

    /// Returns true if `class_name` is `java/lang/ClassLoader` or a subclass of it
    /// (i.e., the resolved owner declared the method as a ClassLoader method).
    fn is_classloader_subtype(&mut self, class_name: &str) -> bool {
        let mut visited = std::collections::HashSet::new();
        self.is_classloader_subtype_inner(class_name, &mut visited)
    }

    fn is_classloader_subtype_inner(
        &mut self,
        class_name: &str,
        visited: &mut std::collections::HashSet<String>,
    ) -> bool {
        if class_name == "java/lang/ClassLoader" {
            return true;
        }
        if !visited.insert(class_name.to_owned()) {
            return false; // cycle guard
        }
        self.ensure_class_ready(class_name);
        let super_name = self.get_class(class_name).and_then(|cf| {
            if cf.super_class != 0 {
                Some(cf.constant_pool.class_name(cf.super_class).to_owned())
            } else {
                None
            }
        });
        match super_name {
            Some(s) => self.is_classloader_subtype_inner(&s, visited),
            None => false,
        }
    }

    /// Handle ClassLoader instance methods that must dispatch by resolved owner, not runtime class.
    /// Returns `Some(value)` if the method was handled, `None` to fall through.
    fn native_classloader(
        &mut self,
        this: &JRef,
        method_name: &str,
        args: &[JValue],
    ) -> Option<JValue> {
        match method_name {
            "loadClass" | "findClass" => {
                // A null or missing name argument must surface as NullPointerException.
                // (`defineClass` accepts a null name per JDK spec, so the check is here only.)
                let name_str = match args
                    .first()
                    .and_then(|v| v.as_ref())
                    .and_then(|r| r.borrow().as_java_string().map(|s| s.to_owned()))
                {
                    Some(s) => s,
                    None => {
                        self.throw_null_pointer("name");
                        return Some(JValue::Void);
                    }
                };
                let internal = Self::class_internal_name_from_runtime_name(&name_str);
                self.ensure_class_ready(&internal);
                match self.classes.get(&internal) {
                    Some(LazyClass::Ready(_)) => {}
                    Some(LazyClass::ParseError(msg)) => {
                        let msg = msg.clone();
                        self.throw_class_format_error(&msg);
                        return Some(JValue::Void);
                    }
                    _ => {
                        self.throw_class_not_found(&name_str);
                        return Some(JValue::Void);
                    }
                }
                let initiating_loader = self.loader_id_for_classloader(this);
                let class_id = self
                    .class_id_for_defined(initiating_loader, &internal)
                    .or_else(|| {
                        self.class_id_for_defined(
                            super::class_identity::LoaderId::SYSTEM,
                            &internal,
                        )
                    })
                    .or_else(|| {
                        self.class_id_for_defined(
                            super::class_identity::LoaderId::BOOTSTRAP,
                            &internal,
                        )
                    })
                    .unwrap_or_else(|| {
                        self.register_defined_class(
                            super::class_identity::LoaderId::SYSTEM,
                            internal.clone(),
                        )
                    });
                self.record_initiating_loader(initiating_loader, internal.clone(), class_id);
                Some(JValue::Ref(self.class_object_for_id(class_id)))
            }
            "findLoadedClass" => {
                let name_str = args
                    .first()
                    .and_then(|v| v.as_ref())
                    .and_then(|r| r.borrow().as_java_string().map(|s| s.to_owned()))
                    .unwrap_or_default();
                let internal = Self::class_internal_name_from_runtime_name(&name_str);
                let initiating_loader = self.loader_id_for_classloader(this);
                if let Some(class_id) = self.class_id_for_initiating(initiating_loader, &internal) {
                    Some(JValue::Ref(self.class_object_for_id(class_id)))
                } else {
                    Some(JValue::Ref(None))
                }
            }
            "defineClass" => {
                let explicit_name = args
                    .first()
                    .and_then(|v| v.as_ref())
                    .and_then(|r| r.borrow().as_java_string().map(|s| s.to_owned()));
                // Extract byte[] argument (2nd arg), off (3rd), len (4th).
                // Supports both 4-arg and 5-arg (with ProtectionDomain) variants.
                let byte_array = args.get(1)
                    .and_then(|v| v.as_ref())
                    .and_then(|r| {
                        let obj = r.borrow();
                        match &obj.native {
                            NativePayload::ByteArray(v) => Some(v.clone()),
                            // newarray-created byte[] uses Array of JValue::Int
                            NativePayload::Array(v) => {
                                Some(v.iter().map(|e| e.as_int() as u8).collect())
                            }
                            _ => None,
                        }
                    });
                let off_raw = args.get(2).map(|v| v.as_int()).unwrap_or(0);
                let len_raw = args.get(3).map(|v| v.as_int()).unwrap_or(0);

                if let Some(bytes) = byte_array {
                    if off_raw < 0 || len_raw < 0 || (off_raw as usize) + (len_raw as usize) > bytes.len() {
                        let detail = format!("defineClass: off={off_raw}, len={len_raw}, array length={}", bytes.len());
                        let exc = self.new_vm_exception_message("java/lang/IndexOutOfBoundsException", detail);
                        *self.pending_exception_mut() = Some(exc);
                        return Some(JValue::Void);
                    }
                    let off = off_raw as usize;
                    let len = len_raw as usize;
                    let class_bytes = bytes[off..off + len].to_vec();
                    if let Some(class_name) = crate::class_file::parse_class_name(&class_bytes) {
                        let defining_loader = self.loader_id_for_classloader(this);
                        if let Some(name) = explicit_name {
                            let explicit_internal = Self::class_internal_name_from_runtime_name(&name);
                            if explicit_internal != class_name {
                                self.throw_no_class_def_found(&class_name);
                                return Some(JValue::Void);
                            }
                        }
                        let class_file = match crate::class_file::parse(&class_bytes) {
                            Ok(class_file) => class_file,
                            Err(err) => {
                                self.throw_class_format_error(&err);
                                return Some(JValue::Void);
                            }
                        };
                        let class_id = match self.try_register_defined_class(
                            defining_loader,
                            class_name.clone(),
                        ) {
                            Ok(class_id) => class_id,
                            Err(DefineClassError::Duplicate(_)) => {
                                self.throw_linkage_error(&format!("duplicate class definition: {class_name}"));
                                return Some(JValue::Void);
                            }
                        };
                        self.record_initiating_loader(defining_loader, class_name.clone(), class_id);
                        self.classes
                            .entry(class_name.clone())
                            .or_insert(LazyClass::Ready(class_file));
                        Some(JValue::Ref(self.class_object_for_id(class_id)))
                    } else {
                        self.throw_class_format_error("defineClass: cannot parse class");
                        Some(JValue::Void)
                    }
                } else {
                    self.throw_null_pointer("defineClass: byte array is null");
                    Some(JValue::Void)
                }
            }
            "getResource" => {
                let name = args
                    .first()
                    .and_then(|v| v.as_ref())
                    .and_then(|r| r.borrow().as_java_string().map(|s| s.to_owned()))
                    .unwrap_or_default();
                let normalized = name.strip_prefix('/').unwrap_or(&name);
                if self.has_resource(normalized) {
                    let url = JObject::new("java/net/URL");
                    url.borrow_mut().fields.insert("protocol".to_owned(),
                        JValue::Ref(Some(self.intern_string("bundle"))));
                    url.borrow_mut().fields.insert("host".to_owned(),
                        JValue::Ref(Some(self.intern_string(""))));
                    url.borrow_mut().fields.insert("port".to_owned(), JValue::Int(-1));
                    url.borrow_mut().fields.insert("file".to_owned(),
                        JValue::Ref(Some(self.intern_string(format!("/{normalized}")))));
                    url.borrow_mut().fields.insert("ref".to_owned(), JValue::Ref(None));
                    Some(JValue::Ref(Some(url)))
                } else {
                    Some(JValue::Ref(None))
                }
            }
            "getResourceAsStream" => {
                let name = args
                    .first()
                    .and_then(|v| v.as_ref())
                    .and_then(|r| r.borrow().as_java_string().map(|s| s.to_owned()))
                    .unwrap_or_default();
                let normalized = name.strip_prefix('/').unwrap_or(&name);
                match self.read_resource(normalized) {
                    Ok(Some(data)) => {
                        // Create a [B array with the resource bytes
                        let elems: Vec<JValue> = data.iter().map(|&b| JValue::Int(b as i8 as i32)).collect();
                        let byte_array = JObject::new_array("[B", elems);
                        // Create ByteArrayInputStream via its constructor logic
                        let bais = JObject::new("java/io/ByteArrayInputStream");
                        bais.borrow_mut().fields.insert("buf".to_owned(), JValue::Ref(Some(byte_array)));
                        bais.borrow_mut().fields.insert("pos".to_owned(), JValue::Int(0));
                        bais.borrow_mut().fields.insert("count".to_owned(), JValue::Int(data.len() as i32));
                        bais.borrow_mut().fields.insert("mark".to_owned(), JValue::Int(0));
                        Some(JValue::Ref(Some(bais)))
                    }
                    Ok(None) => Some(JValue::Ref(None)),
                    Err(err) => {
                        self.throw_runtime_exception(&format!(
                            "getResourceAsStream({normalized}): {err}"
                        ));
                        Some(JValue::Void)
                    }
                }
            }
            "findResource" => {
                // Return null — resources are accessed via getResourceAsStream
                Some(JValue::Ref(None))
            }
            "findResources" => {
                // Return empty Enumeration via Collections.emptyEnumeration()
                match self.invoke_static(
                    "java/util/Collections", "emptyEnumeration",
                    "()Ljava/util/Enumeration;", vec![],
                ) {
                    Ok(v) => Some(v),
                    Err(_) => Some(JValue::Ref(None)),
                }
            }
            _ => None,
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
        // java/lang/Object instance methods are inherited by all reference types.
        match method_name {
            "hashCode" if _descriptor == "()I" => {
                if let Some(s) = this.borrow().as_java_string_value() {
                    return Some(JValue::Int(s.hash_code()));
                }
                // For all other objects use identity (pointer address).
                let ptr = Rc::as_ptr(this) as usize;
                return Some(JValue::Int((ptr as u64 as u32) as i32));
            }
            "intern" if _descriptor == "()Ljava/lang/String;" && this.borrow().as_java_string_value().is_some() => {
                return Some(JValue::Ref(self.intern_existing_string_ref(this)));
            }
            "getClass" if _descriptor == "()Ljava/lang/Class;" => {
                let runtime_class = this.borrow().class_name.clone();
                return Some(JValue::Ref(Some(self.class_object(runtime_class))));
            }
            _ => {}
        }
        // ----- Object.wait/notify/notifyAll (inherited by ALL classes) -----
        {
            let result = match (method_name, _descriptor) {
                ("wait", "()V") | ("wait", "(J)V") => Some(self.monitor_wait(this)),
                ("notify", "()V") => Some(self.monitor_notify(this)),
                ("notifyAll", "()V") => Some(self.monitor_notify_all(this)),
                _ => None,
            };
            if let Some(res) = result {
                if let Err(e) = res {
                    self.throw_illegal_monitor_state(&e);
                }
                return Some(JValue::Void);
            }
        }
        if matches!(this.borrow().native, NativePayload::PrintStream(_)) {
            let is_err = matches!(this.borrow().native, NativePayload::PrintStream(true));
            match (method_name, _descriptor) {
                ("nativeBridgeEnabled", "()Z") => return Some(JValue::Int(1)),
                ("nativeFlush", "()V") => {
                    if matches!(
                        if is_err { self.stderr_mode } else { self.stdout_mode },
                        super::StdioMode::Inherit
                    ) {
                        self.flush_printstreams();
                    }
                    return Some(JValue::Void);
                }
                ("nativeWriteByte", "(I)V") => {
                    let byte = _args.first().map(JValue::as_int).unwrap_or(0) as u8;
                    self.write_printstream_bytes(is_err, &[byte]);
                    return Some(JValue::Void);
                }
                ("nativeWriteBytes", "([BII)V") => {
                    let array_ref = match _args.first().and_then(JValue::as_ref) {
                        Some(array_ref) => array_ref,
                        None => {
                            self.throw_null_pointer("PrintStream.write: buf is null");
                            return Some(JValue::Void);
                        }
                    };
                    let bytes = {
                        let array = array_ref.borrow();
                        match &array.native {
                            NativePayload::ByteArray(bytes) => bytes.clone(),
                            NativePayload::Array(values) => {
                                values.iter().map(|value| value.as_int() as u8).collect()
                            }
                            _ => {
                                self.throw_null_pointer("PrintStream.write: buf is null");
                                return Some(JValue::Void);
                            }
                        }
                    };
                    let off = _args.get(1).map(JValue::as_int).unwrap_or(0);
                    let len = _args.get(2).map(JValue::as_int).unwrap_or(0);
                    if off < 0 || len < 0 || (off as usize).saturating_add(len as usize) > bytes.len() {
                        let exc = self.new_vm_exception_message("java/lang/IndexOutOfBoundsException", format!(
                            "PrintStream.write: off={off}, len={len}, array length={}",
                            bytes.len()
                        ));
                        *self.pending_exception_mut() = Some(exc);
                        return Some(JValue::Void);
                    }
                    self.write_printstream_bytes(
                        is_err,
                        &bytes[off as usize..off as usize + len as usize],
                    );
                    return Some(JValue::Void);
                }
                _ => {}
            }
        }
        if _class_name == "java/io/PrintStream" && method_name == "nativeBridgeEnabled" {
            return Some(JValue::Int(0));
        }
        // ----- java.lang.Thread native methods -----
        if this.borrow().class_name == "java/lang/Thread"
            || self.is_instance_of(&this.borrow().class_name.clone(), "java/lang/Thread")
        {
            match (method_name, _descriptor) {
                ("start", "()V") => {
                    match self.thread_start(Rc::clone(this)) {
                        Ok(_) => {}
                        Err(e) => {
                            // Propagate the error as a pending Java exception
                            // so that Java code can observe the failure.
                            let exc = self.new_vm_exception_message("java/lang/RuntimeException", e);
                            *self.pending_exception_mut() = Some(exc);
                        }
                    }
                    return Some(JValue::Void);
                }
                ("join", "()V") => {
                    if let Some(target_id) = self.find_thread_id_by_object(this) {
                        self.thread_join(target_id);
                    }
                    return Some(JValue::Void);
                }
                ("isAlive", "()Z") => {
                    let alive = self.thread_is_alive(this);
                    return Some(JValue::Int(if alive { 1 } else { 0 }));
                }
                _ => {}
            }
        }
        // ClassLoader methods must dispatch on the resolved owner (`_class_name`), not the
        // runtime class of `this`, so that subclasses of ClassLoader also hit these stubs.
        // Guard on method name first to avoid super-chain walks on unrelated calls.
        if matches!(method_name, "loadClass" | "findClass" | "findLoadedClass" | "defineClass" | "getResource" | "getResourceAsStream" | "findResource" | "findResources")
            && self.is_classloader_subtype(_class_name)
        {
            if let Some(v) = self.native_classloader(this, method_name, _args) {
                return Some(v);
            }
        }
        let cn = this.borrow().class_name.clone();
        match (cn.as_str(), method_name) {
            ("java/security/SecureRandom", "nextBytes") => {
                // Fill byte[] argument with cryptographically random bytes via getrandom.
                if let Some(arr_ref) = _args.first().and_then(|v| v.as_ref()) {
                    let mut obj = arr_ref.borrow_mut();
                    if let NativePayload::Array(ref mut elems) = obj.native {
                        let len = elems.len();
                        let mut buf = vec![0u8; len];
                        getrandom::fill(&mut buf).ok();
                        for (i, &b) in buf.iter().enumerate() {
                            elems[i] = JValue::Int(b as i8 as i32);
                        }
                    } else if let NativePayload::ByteArray(ref mut bytes) = obj.native {
                        getrandom::fill(bytes.as_mut_slice()).ok();
                    }
                }
                Some(JValue::Void)
            }
            ("java/util/regex/Pattern", "matcher") => {
                let input = _args
                    .first()
                    .and_then(|v| v.as_ref())
                    .cloned()
                    .unwrap_or_else(|| self.intern_string(""));
                let m = JObject::new("java/util/regex/Matcher");
                m.borrow_mut().fields.insert("__pattern".to_owned(), JValue::Ref(Some(this.clone())));
                m.borrow_mut().fields.insert("__input".to_owned(), JValue::Ref(Some(input)));
                Some(JValue::Ref(Some(m)))
            }
            ("java/util/regex/Matcher", "matches") => {
                let (regex, flags, input_value) = {
                    let mb = this.borrow();
                    // Try bytecode field names first, then native field names
                    let pattern_ref = mb.fields.get("pattern")
                        .or_else(|| mb.fields.get("__pattern"));
                    let (regex, flags) = pattern_ref
                        .and_then(|v| v.as_ref())
                        .map(|p| {
                            let pb = p.borrow();
                            // Try bytecode field name "regex" first, then native "__regex"
                            let regex = pb.fields.get("regex")
                                .or_else(|| pb.fields.get("__regex"))
                                .and_then(|v| v.as_ref().cloned())
                                .and_then(|s| s.borrow().as_java_string_value().cloned())
                                .map(|s| regex_encode_java_string(&s).into_owned())
                                .unwrap_or_default();
                            let flags = pb.fields.get("__flags").map(|v| v.as_int()).unwrap_or(0);
                            (regex, flags)
                        })
                        .unwrap_or_else(|| (String::new(), 0));
                    let input = mb.fields.get("input")
                        .or_else(|| mb.fields.get("__input"))
                        .and_then(|v| v.as_ref())
                        .and_then(|s| s.borrow().as_java_string_value().cloned())
                        .unwrap_or_else(|| JavaStringValue::from_utf16(Vec::new()));
                    (regex, flags, input)
                };
                let input_text = regex_encode_java_string(&input_value);
                // Use captures to extract groups
                let anchored = regex_full_match_source(&regex);
                let re = self.compile_regex_cached(&anchored, flags);
                let caps = re.as_ref().and_then(|r| r.captures(input_text.as_ref()));
                let ok = caps.is_some();
                // Store captured groups in __groups array field + matchStart/matchEnd
                if let Some(caps) = &caps {
                    let mut groups = Vec::new();
                    for i in 0..caps.len() {
                        if let Some(m) = caps.get(i) {
                            let start = byte_offset_to_utf16_index(&input_text, m.start());
                            let end = byte_offset_to_utf16_index(input_text.as_ref(), m.end());
                            groups.push(JValue::Ref(Some(self.intern_string_value(
                                string_slice_value(&input_value, start, end),
                            ))));
                        } else {
                            groups.push(JValue::Ref(None));
                        }
                    }
                    let groups_arr = JObject::new_array("[Ljava/lang/String;", groups);
                    let (ms, me) = caps
                        .get(0)
                        .map(|m| {
                            (
                                byte_offset_to_utf16_index(input_text.as_ref(), m.start()) as i32,
                                byte_offset_to_utf16_index(input_text.as_ref(), m.end()) as i32,
                            )
                        })
                        .unwrap_or((-1, -1));
                    this.borrow_mut().fields.insert("__groups".to_owned(), JValue::Ref(Some(groups_arr)));
                    this.borrow_mut().fields.insert("matchStart".to_owned(), JValue::Int(ms));
                    this.borrow_mut().fields.insert("matchEnd".to_owned(), JValue::Int(me));
                } else {
                    this.borrow_mut().fields.remove("__groups");
                    this.borrow_mut().fields.insert("matchStart".to_owned(), JValue::Int(-1));
                    this.borrow_mut().fields.insert("matchEnd".to_owned(), JValue::Int(-1));
                }
                Some(JValue::Int(if ok { 1 } else { 0 }))
            }
            ("java/util/regex/Matcher", "find") => {
                let (regex, flags, input_value, search_index) = {
                    let mb = this.borrow();
                    let pattern_ref = mb
                        .fields
                        .get("pattern")
                        .or_else(|| mb.fields.get("__pattern"));
                    let (regex, flags) = pattern_ref
                        .and_then(|v| v.as_ref())
                        .map(|p| {
                            let pb = p.borrow();
                            let regex = pb.fields
                                .get("regex")
                                .or_else(|| pb.fields.get("__regex"))
                                .and_then(|v| v.as_ref().cloned())
                                .and_then(|s| s.borrow().as_java_string_value().cloned())
                                .map(|s| regex_encode_java_string(&s).into_owned())
                                .unwrap_or_default();
                            let flags = pb.fields.get("__flags").map(|v| v.as_int()).unwrap_or(0);
                            (regex, flags)
                        })
                        .unwrap_or_else(|| (String::new(), 0));
                    let input = mb
                        .fields
                        .get("input")
                        .or_else(|| mb.fields.get("__input"))
                        .and_then(|v| v.as_ref())
                        .and_then(|s| s.borrow().as_java_string_value().cloned())
                        .unwrap_or_else(|| JavaStringValue::from_utf16(Vec::new()));
                    let search_index = mb
                        .fields
                        .get("searchIndex")
                        .map(|v| v.as_int().max(0) as usize)
                        .unwrap_or(0);
                    (regex, flags, input, search_index)
                };
                let input_len = input_value.len_utf16();

                if regex.is_empty() {
                    if search_index > input_len {
                        let mut mb = this.borrow_mut();
                        mb.fields.remove("__groups");
                        mb.fields.insert("matchStart".to_owned(), JValue::Int(-1));
                        mb.fields.insert("matchEnd".to_owned(), JValue::Int(-1));
                        mb.fields.insert(
                            "searchIndex".to_owned(),
                            JValue::Int(input_len.saturating_add(1) as i32),
                        );
                        return Some(JValue::Int(0));
                    }
                    let groups_arr = JObject::new_array(
                        "[Ljava/lang/String;",
                        vec![JValue::Ref(Some(self.intern_string("")))],
                    );
                    let next_search = search_index.saturating_add(1);
                    let mut mb = this.borrow_mut();
                    mb.fields
                        .insert("__groups".to_owned(), JValue::Ref(Some(groups_arr)));
                    mb.fields
                        .insert("matchStart".to_owned(), JValue::Int(search_index as i32));
                    mb.fields
                        .insert("matchEnd".to_owned(), JValue::Int(search_index as i32));
                    mb.fields.insert(
                        "searchIndex".to_owned(),
                        JValue::Int(next_search.min(input_len.saturating_add(1)) as i32),
                    );
                    return Some(JValue::Int(1));
                }

                if search_index > input_len {
                    this.borrow_mut().fields.remove("__groups");
                    this.borrow_mut()
                        .fields
                        .insert("matchStart".to_owned(), JValue::Int(-1));
                    this.borrow_mut()
                        .fields
                        .insert("matchEnd".to_owned(), JValue::Int(-1));
                    this.borrow_mut().fields.insert(
                        "searchIndex".to_owned(),
                        JValue::Int(input_len.saturating_add(1) as i32),
                    );
                    return Some(JValue::Int(0));
                }

                let input_text = regex_encode_java_string(&input_value);
                let re = self.compile_regex_cached(&regex, flags);
                let start_byte = utf16_index_to_byte_offset(input_text.as_ref(), search_index);
                let hay = &input_text.as_ref()[start_byte..];
                let caps = re.as_ref().and_then(|r| r.captures(hay));

                if let Some(caps) = caps {
                    let mut groups = Vec::new();
                    for i in 0..caps.len() {
                        if let Some(m) = caps.get(i) {
                            let start = byte_offset_to_utf16_index(input_text.as_ref(), start_byte + m.start());
                            let end = byte_offset_to_utf16_index(input_text.as_ref(), start_byte + m.end());
                            groups.push(JValue::Ref(Some(self.intern_string_value(
                                string_slice_value(&input_value, start, end),
                            ))));
                        } else {
                            groups.push(JValue::Ref(None));
                        }
                    }
                    let groups_arr = JObject::new_array("[Ljava/lang/String;", groups);
                    let (ms, me) = caps
                        .get(0)
                        .map(|m| {
                            (
                                byte_offset_to_utf16_index(input_text.as_ref(), start_byte + m.start()) as i32,
                                byte_offset_to_utf16_index(input_text.as_ref(), start_byte + m.end()) as i32,
                            )
                        })
                        .unwrap_or((-1, -1));
                    let next_search = if ms >= 0 && ms == me {
                        (me as usize).saturating_add(1)
                    } else {
                        me.max(0) as usize
                    };
                    let mut mb = this.borrow_mut();
                    mb.fields
                        .insert("__groups".to_owned(), JValue::Ref(Some(groups_arr)));
                    mb.fields.insert("matchStart".to_owned(), JValue::Int(ms));
                    mb.fields.insert("matchEnd".to_owned(), JValue::Int(me));
                    mb.fields.insert(
                        "searchIndex".to_owned(),
                        JValue::Int(next_search.min(input_len.saturating_add(1)) as i32),
                    );
                    return Some(JValue::Int(1));
                }

                let mut mb = this.borrow_mut();
                mb.fields.remove("__groups");
                mb.fields.insert("matchStart".to_owned(), JValue::Int(-1));
                mb.fields.insert("matchEnd".to_owned(), JValue::Int(-1));
                mb.fields.insert(
                    "searchIndex".to_owned(),
                    JValue::Int(input_len.saturating_add(1) as i32),
                );
                Some(JValue::Int(0))
            }
            ("java/util/regex/Matcher", "group") => {
                // group(int) — return captured group from __groups array
                let idx = _args.first().map(|v| v.as_int().max(0) as usize).unwrap_or(0);
                let mb = this.borrow();
                if let Some(JValue::Ref(Some(groups_ref))) = mb.fields.get("__groups") {
                    if let NativePayload::Array(groups) = &groups_ref.borrow().native {
                        if let Some(g) = groups.get(idx) {
                            return Some(g.clone());
                        }
                    }
                }
                // Fallback: group 0 from __match fields
                if idx == 0 {
                    let start = mb.fields.get("matchStart").map(|v| v.as_int()).unwrap_or(-1);
                    let end = mb.fields.get("matchEnd").map(|v| v.as_int()).unwrap_or(-1);
                    let input = mb.fields.get("input")
                        .or_else(|| mb.fields.get("__input"))
                        .and_then(|v| v.as_ref())
                        .and_then(|s| s.borrow().as_java_string_value().cloned())
                        .unwrap_or_else(|| JavaStringValue::from_utf16(Vec::new()));
                    drop(mb);
                    if start >= 0 && end >= 0 && (end as usize) <= input.len_utf16() {
                        return Some(JValue::Ref(Some(JObject::new_string_value(
                            string_slice_value(&input, start as usize, end as usize),
                        ))));
                    }
                } else {
                    drop(mb);
                }
                Some(JValue::Ref(None))
            }
            ("java/lang/Class", "getName") => {
                let internal = self
                    .class_internal_name_from_obj(this)
                    .unwrap_or_else(|| "java/lang/Object".to_owned());
                Some(JValue::Ref(Some(self.intern_string(Self::class_display_name(&internal)))))
            }
            ("java/lang/Class", "getClassLoader") => {
                let loader = this
                    .borrow()
                    .fields
                    .get("__class_loader")
                    .and_then(|v| v.as_ref().cloned());
                Some(JValue::Ref(loader))
            }
            ("java/lang/Class", "getModifiers") => {
                let target = self
                    .class_internal_name_from_obj(this)
                    .unwrap_or_else(|| "java/lang/Object".to_owned());
                self.ensure_class_ready(&target);
                let mods = self.get_class(&target).map(|cf| i32::from(cf.access_flags)).unwrap_or(0);
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
                self.ensure_class_ready(&target);
                let is_iface = self.get_class(&target).map(|cf| (cf.access_flags & 0x0200) != 0).unwrap_or(false);
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
                self.ensure_class_ready(&target);
                let super_name = if target.starts_with('[') {
                    Some("java/lang/Object".to_owned())
                } else if let Some(cf) = self.get_class(&target) {
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
                self.ensure_class_ready(&target);
                let iface_names: Vec<String> = if target.starts_with('[') {
                    vec!["java/lang/Cloneable".to_owned(), "java/io/Serializable".to_owned()]
                } else if let Some(cf) = self.get_class(&target) {
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
                if let Some(JValue::Ref(Some(arr))) = self.static_fields.get(&target).and_then(|m| m.get("$VALUES")).cloned() {
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
                self.ensure_class_ready(&target);
                let is_record = self.get_class(&target).map(|cf| cf.attributes.iter().any(|a| matches!(a, Attribute::Record { .. }))).unwrap_or(false);
                Some(JValue::Int(if is_record { 1 } else { 0 }))
            }
            ("java/lang/Class", "getRecordComponents") => {
                let target = self
                    .class_internal_name_from_obj(this)
                    .unwrap_or_else(|| "java/lang/Object".to_owned());
                let mut comps_meta: Vec<(String, String)> = Vec::new();
                self.ensure_class_ready(&target);
                if let Some(cf) = self.get_class(&target) {
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
                self.ensure_class_ready(&target);
                let anns = if let Some(cf) = self.get_class(&target) {
                    let attrs = cf.attributes.clone();
                    let cp = cf.constant_pool.clone();
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
                self.ensure_class_ready(&target);
                if let Some(cf) = self.get_class(&target) {
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
                self.ensure_class_ready(&target);
                if let Some(cf) = self.get_class(&target) {
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
                self.ensure_class_ready(&target);
                if let Some(cf) = self.get_class(&target) {
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

                self.ensure_class_ready(&owner);
                let per_param = if let Some(cf) = self.get_class(&owner) {
                    if let Some(mi) = cf.methods.iter().find(|m| {
                        cf.constant_pool.utf8(m.name_index) == method_name
                            && cf.constant_pool.utf8(m.descriptor_index) == desc
                    }) {
                        let attrs = mi.attributes.clone();
                        let cp = cf.constant_pool.clone();
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
                    Some(self.wrap_primitive_value_for_descriptor(&ret_token, out))
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
                self.ensure_class_ready(&owner);
                let anns = if let Some(cf) = self.get_class(&owner) {
                    if let Some(mi) = cf.methods.iter().find(|m| {
                        cf.constant_pool.utf8(m.name_index) == name && cf.constant_pool.utf8(m.descriptor_index) == desc
                    }) {
                        let attrs = mi.attributes.clone();
                        let cp = cf.constant_pool.clone();
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
                self.ensure_class_ready(&owner);
                let anns = if let Some(cf) = self.get_class(&owner) {
                    if let Some(mi) = cf.methods.iter().find(|m| {
                        cf.constant_pool.utf8(m.name_index) == "<init>" && cf.constant_pool.utf8(m.descriptor_index) == desc
                    }) {
                        let attrs = mi.attributes.clone();
                        let cp = cf.constant_pool.clone();
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
                        .get(&owner).and_then(|m| m.get(&name))
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
                    Some(self.wrap_primitive_value_for_descriptor(&desc, raw))
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
                    self.static_fields.entry(owner).or_default().insert(name, adapted);
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
                self.ensure_class_ready(&owner);
                let anns = if let Some(cf) = self.get_class(&owner) {
                    if let Some(fi) = cf.fields.iter().find(|f| {
                        cf.constant_pool.utf8(f.name_index) == name && cf.constant_pool.utf8(f.descriptor_index) == desc
                    }) {
                        let attrs = fi.attributes.clone();
                        let cp = cf.constant_pool.clone();
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
                self.ensure_class_ready(&owner);
                let ann_src = self.get_class(&owner).and_then(|cf| {
                    for attr in &cf.attributes {
                        if let Attribute::Record { components } = attr {
                            if let Some(c) = components.iter().find(|c| {
                                cf.constant_pool.utf8(c.name_index) == name
                                    && cf.constant_pool.utf8(c.descriptor_index) == desc
                            }) {
                                let attrs = c.attributes.clone();
                                let cp = cf.constant_pool.clone();
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
                Some(JValue::Ref(Some(Rc::clone(this))))
            }
            ("java/lang/String", "length") => {
                let len = this
                    .borrow()
                    .as_java_string_value()
                    .map(|s| s.len_utf16() as i32)
                    .unwrap_or(0);
                Some(JValue::Int(len))
            }
            ("java/lang/String", "charAt") => {
                let idx = _args.first().map(|v| v.as_int() as usize).unwrap_or(0);
                let ch = this
                    .borrow()
                    .as_java_string_value()
                    .and_then(|s| s.code_unit_at(idx))
                    .unwrap_or(0) as i32;
                Some(JValue::Int(ch))
            }
            ("java/lang/String", "isEmpty") => {
                let empty = this
                    .borrow()
                    .as_java_string_value()
                    .map(|s| s.len_utf16() == 0)
                    .unwrap_or(true);
                Some(JValue::Int(if empty { 1 } else { 0 }))
            }
            ("java/lang/String", "equals") => {
                let this_borrow = this.borrow();
                let eq = this_borrow
                    .as_java_string_value()
                    .map(|this_value| {
                        _args
                            .first()
                            .and_then(|arg| match arg {
                                JValue::Ref(Some(other_ref)) => Some(other_ref),
                                _ => None,
                            })
                            .map(|other_ref| {
                                let other_borrow = other_ref.borrow();
                                other_borrow
                                    .as_java_string_value()
                                    .map(|other_value| this_value == other_value)
                                    .unwrap_or(false)
                            })
                            .unwrap_or(false)
                    })
                    .unwrap_or(false);
                Some(JValue::Int(if eq { 1 } else { 0 }))
            }
            ("java/lang/String", "hashCode") => {
                let hash = this
                    .borrow()
                    .as_java_string_value()
                    .map(JavaStringValue::hash_code)
                    .unwrap_or(0);
                Some(JValue::Int(hash))
            }
            ("java/lang/String", "substring") => {
                let this_borrow = this.borrow();
                let value = this_borrow.as_java_string_value();
                let len = value.map(JavaStringValue::len_utf16).unwrap_or(0);
                let begin = (_args.first().map(|v| v.as_int() as usize).unwrap_or(0)).min(len);
                let end = (_args.get(1).map(|v| v.as_int() as usize).unwrap_or(len))
                    .min(len)
                    .max(begin);
                let result = value
                    .map(|value| string_slice_value(value, begin, end))
                    .unwrap_or_else(|| JavaStringValue::from_utf16(Vec::new()));
                Some(JValue::Ref(Some(JObject::new_string_value(result))))
            }
            ("java/lang/String", "concat") => {
                let this_borrow = this.borrow();
                let result = if let Some(left) = this_borrow.as_java_string_value() {
                    if let Some(arg_ref) = _args.first().and_then(|arg| arg.as_ref()) {
                        let arg_borrow = arg_ref.borrow();
                        if let Some(right) = arg_borrow.as_java_string_value() {
                            left.concat(right)
                        } else {
                            left.clone()
                        }
                    } else {
                        left.clone()
                    }
                } else {
                    JavaStringValue::from_utf16(Vec::new())
                };
                Some(JValue::Ref(Some(JObject::new_string_value(result))))
            }
            ("java/lang/String", "contains") => {
                let this_borrow = this.borrow();
                let found = this_borrow
                    .as_java_string_value()
                    .map(|haystack| {
                        _args
                            .first()
                            .and_then(|arg| match arg {
                                JValue::Ref(Some(needle_ref)) => Some(needle_ref),
                                _ => None,
                            })
                            .map(|needle_ref| {
                                let needle_borrow = needle_ref.borrow();
                                needle_borrow
                                    .as_java_string_value()
                                    .map(|needle| string_index_of_value(haystack, needle, 0).is_some())
                                    .unwrap_or(false)
                            })
                            .unwrap_or(false)
                    })
                    .unwrap_or(false);
                Some(JValue::Int(if found { 1 } else { 0 }))
            }
            ("java/lang/String", "startsWith") => {
                let this_borrow = this.borrow();
                let s = this_borrow.as_java_string_utf16().unwrap_or(&[]);
                let prefix = _args
                    .first()
                    .and_then(arg_string_value);
                let ok = if let Some(prefix) = prefix {
                    s.len() >= prefix.len_utf16() && s[..prefix.len_utf16()] == prefix.utf16()[..]
                } else {
                    false
                };
                Some(JValue::Int(if ok { 1 } else { 0 }))
            }
            ("java/lang/String", "endsWith") => {
                let this_borrow = this.borrow();
                let s = this_borrow.as_java_string_utf16().unwrap_or(&[]);
                let suffix = _args
                    .first()
                    .and_then(arg_string_value);
                let ok = if let Some(suffix) = suffix {
                    s.len() >= suffix.len_utf16()
                        && s[s.len() - suffix.len_utf16()..] == suffix.utf16()[..]
                } else {
                    false
                };
                Some(JValue::Int(if ok { 1 } else { 0 }))
            }
            ("java/lang/String", "indexOf") => {
                let this_borrow = this.borrow();
                let value = this_borrow.as_java_string_value();
                let from_index = _args
                    .get(1)
                    .map(|v| v.as_int().max(0) as usize)
                    .unwrap_or(0);
                let idx = match _args.first() {
                    Some(arg) => {
                        match arg {
                            JValue::Ref(Some(needle_ref)) => {
                                let needle_borrow = needle_ref.borrow();
                                if let (Some(haystack), Some(needle)) =
                                    (value, needle_borrow.as_java_string_value())
                                {
                                    string_index_of_value(haystack, needle, from_index)
                                        .map(|i| i as i32)
                                        .unwrap_or(-1)
                                } else {
                                    -1
                                }
                            }
                            JValue::Int(ch) => {
                                let needle = *ch as u16;
                                let units = value.map(JavaStringValue::utf16).unwrap_or(&[]);
                                (from_index.min(units.len())..units.len())
                                    .find(|&i| units[i] == needle)
                                    .map(|i| i as i32)
                                    .unwrap_or(-1)
                            }
                            _ => -1,
                        }
                    }
                    None => -1,
                };
                Some(JValue::Int(idx))
            }
            ("java/lang/String", "lastIndexOf") => {
                let this_borrow = this.borrow();
                let value = this_borrow.as_java_string_value();
                let units = value.map(JavaStringValue::utf16).unwrap_or(&[]);
                let from_index = _args
                    .get(1)
                    .map(|v| v.as_int().max(0) as usize)
                    .unwrap_or_else(|| units.len().saturating_sub(1));
                let idx = match _args.first() {
                    Some(arg) => {
                        match arg {
                            JValue::Ref(Some(needle_ref)) => {
                                let needle_borrow = needle_ref.borrow();
                                if let Some(needle) = needle_borrow.as_java_string_value() {
                                    u16_rfind(units, needle.utf16(), from_index)
                                        .map(|i| i as i32)
                                        .unwrap_or(-1)
                                } else {
                                    -1
                                }
                            }
                            JValue::Int(ch) => {
                                u16_last_index_of_unit(units, *ch as u16, from_index)
                                    .map(|i| i as i32)
                                    .unwrap_or(-1)
                            }
                            _ => -1,
                        }
                    }
                    None => -1,
                };
                Some(JValue::Int(idx))
            }
            ("java/lang/String", "trim") => {
                let s = this
                    .borrow()
                    .java_string_to_string_lossy()
                    .unwrap_or_default()
                    .trim()
                    .to_owned();
                Some(JValue::Ref(Some(JObject::new_string(s))))
            }
            ("java/lang/String", "toLowerCase") => {
                let s = this
                    .borrow()
                    .java_string_to_string_lossy()
                    .unwrap_or_default()
                    .to_lowercase();
                Some(JValue::Ref(Some(JObject::new_string(s))))
            }
            ("java/lang/String", "toUpperCase") => {
                let s = this
                    .borrow()
                    .java_string_to_string_lossy()
                    .unwrap_or_default()
                    .to_uppercase();
                Some(JValue::Ref(Some(JObject::new_string(s))))
            }
            ("java/lang/String", "toCharArray") => {
                let chars: Vec<JValue> = this
                    .borrow()
                    .as_java_string_utf16()
                    .map(|s| s.iter().map(|c| JValue::Int(i32::from(*c))).collect())
                    .unwrap_or_default();
                Some(JValue::Ref(Some(JObject::new_array("[C", chars))))
            }
            ("java/lang/String", "getBytes") => {
                let s = this
                    .borrow()
                    .java_string_to_string_lossy()
                    .unwrap_or_default();
                let bytes: Vec<JValue> = s.bytes().map(|b| JValue::Int(b as i32)).collect();
                Some(JValue::Ref(Some(JObject::new_array("[B", bytes))))
            }
            ("java/lang/String", "replace") => {
                let s = this
                    .borrow()
                    .as_java_string_value()
                    .cloned()
                    .unwrap_or_else(|| JavaStringValue::from_utf16(Vec::new()));
                if _args.len() >= 2 {
                    // replace(char, char)
                    if let (JValue::Int(old_c), JValue::Int(new_c)) = (&_args[0], &_args[1]) {
                        let result: Vec<u16> = s
                            .utf16()
                            .iter()
                            .map(|c| if *c == (*old_c as u16) { *new_c as u16 } else { *c })
                            .collect();
                        Some(JValue::Ref(Some(JObject::new_string_utf16(result))))
                    } else {
                        // replace(CharSequence, CharSequence) — null args throw NPE per JDK spec
                        let old_ref = _args[0].as_ref();
                        let new_ref = _args[1].as_ref();
                        if old_ref.is_none() || new_ref.is_none() {
                            self.throw_null_pointer("String.replace: null argument");
                            return Some(JValue::Void);
                        }
                        let old_str = old_ref
                            .and_then(|_| arg_string_value(&_args[0]))
                            .unwrap_or_else(|| JavaStringValue::from_utf16(Vec::new()));
                        let new_str = new_ref
                            .and_then(|_| arg_string_value(&_args[1]))
                            .unwrap_or_else(|| JavaStringValue::from_utf16(Vec::new()));
                        let result = u16_replace(s.utf16(), old_str.utf16(), new_str.utf16());
                        Some(JValue::Ref(Some(JObject::new_string_utf16(result))))
                    }
                } else {
                    Some(JValue::Ref(Some(Rc::clone(this))))
                }
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
                    NativePayload::ProcessPipeInputStream => NativePayload::ProcessPipeInputStream,
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
                    NativePayload::RecordMethod { method, class_simple_name, component_names, getters } =>
                        NativePayload::RecordMethod {
                            method: method.clone(),
                            class_simple_name: class_simple_name.clone(),
                            component_names: component_names.clone(),
                            getters: getters.clone(),
                        },
                };
                let cloned = Rc::new(RefCell::new(crate::heap::JObject {
                    class_name: src.class_name.clone(),
                    fields,
                    native,
                }));
                Some(JValue::Ref(Some(cloned)))
            }
            ("java/io/ProcessPipeInputStream", "read0") if _descriptor == "()I" => {
                Some(JValue::Int(self.stdin_read_byte()))
            }
            ("java/io/ProcessPipeInputStream", "available0") if _descriptor == "()I" => {
                Some(JValue::Int(self.stdin_available()))
            }
            ("java/io/ProcessPipeInputStream", "close0") if _descriptor == "()V" => {
                self.close_stdin();
                Some(JValue::Void)
            }
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::super::{JarEntryRef, Vm};
    use crate::heap::{JObject, JValue};

    #[test]
    fn get_resource_as_stream_raises_runtime_exception_on_lazy_read_error() {
        let mut vm = Vm::new();
        vm.load_jar(include_bytes!("../../tests/test.jar")).expect("load test jar");
        vm.pending_resources.insert(
            "broken.txt".to_owned(),
            JarEntryRef {
                jar_id: 0,
                entry_index: 999,
                entry_name: "broken.txt".to_owned(),
            },
        );

        let arg = JValue::Ref(Some(JObject::new_string("broken.txt")));
        let classloader = JObject::new("java/lang/ClassLoader");
        let result = vm.native_classloader(&classloader, "getResourceAsStream", &[arg]);

        assert!(matches!(result, Some(JValue::Void)));
        let err = vm.pending_exception_err().expect("pending exception");
        assert!(err.contains("java/lang/RuntimeException"), "unexpected error: {err}");
        assert!(err.contains("broken.txt"), "unexpected error: {err}");
    }
}
