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
    fn native_classloader(&mut self, method_name: &str, args: &[JValue]) -> Option<JValue> {
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
                let class_id = match self.ensure_class_loaded_by_name(&internal) {
                    Some(class_id) => class_id,
                    None => {
                        self.throw_class_not_found(&name_str);
                        return Some(JValue::Void);
                    }
                };
                if let Some(msg) = self.class_parse_error(&internal).map(str::to_owned) {
                    self.throw_class_format_error(&msg);
                    return Some(JValue::Void);
                }
                Some(JValue::Ref(Some(self.class_object_by_id(class_id))))
            }
            "findLoadedClass" => {
                let name_str = args
                    .first()
                    .and_then(|v| v.as_ref())
                    .and_then(|r| r.borrow().as_java_string().map(|s| s.to_owned()))
                    .unwrap_or_default();
                let internal = Self::class_internal_name_from_runtime_name(&name_str);
                if let Some(class_id) = self.tracked_class_id_for_name(&internal) {
                    Some(JValue::Ref(Some(self.class_object_by_id(class_id))))
                } else {
                    Some(JValue::Ref(None))
                }
            }
            "defineClass" => {
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
                        self.load_lazy(class_name.clone(), class_bytes);
                        let Some(class_id) = self.ensure_class_loaded_by_name(&class_name) else {
                            self.throw_class_not_found(&class_name);
                            return Some(JValue::Void);
                        };
                        if let Some(msg) = self.class_parse_error(&class_name).map(str::to_owned) {
                            self.throw_class_format_error(&msg);
                            return Some(JValue::Void);
                        }
                        Some(JValue::Ref(Some(self.class_object_by_id(class_id))))
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
                // Strings have value-based equality: use Java's string hash algorithm.
                if let Some(s) = this.borrow().as_java_string().map(|s| s.to_owned()) {
                    let hash = s.chars().fold(0i32, |h, c| h.wrapping_mul(31).wrapping_add(c as i32));
                    return Some(JValue::Int(hash));
                }
                // For all other objects use identity (pointer address).
                let ptr = Rc::as_ptr(this) as usize;
                return Some(JValue::Int((ptr as u64 as u32) as i32));
            }
            "intern" if _descriptor == "()Ljava/lang/String;" && this.borrow().as_java_string().is_some() => {
                return Some(JValue::Ref(Some(this.clone())));
            }
            "getClass" if _descriptor == "()Ljava/lang/Class;" => {
                if let Some(class_id) = self.object_runtime_class_id(this) {
                    return Some(JValue::Ref(Some(self.class_object_by_id(class_id))));
                }
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
            || self.is_instance_of_object(this, "java/lang/Thread")
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
            if let Some(v) = self.native_classloader(method_name, _args) {
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
                    // Try bytecode field names first, then native field names
                    let pattern_ref = mb.fields.get("pattern")
                        .or_else(|| mb.fields.get("__pattern"));
                    let regex = pattern_ref
                        .and_then(|v| v.as_ref())
                        .and_then(|p| {
                            let pb = p.borrow();
                            // Try bytecode field name "regex" first, then native "__regex"
                            pb.fields.get("regex")
                                .or_else(|| pb.fields.get("__regex"))
                                .and_then(|v| v.as_ref().cloned())
                                .and_then(|s| s.borrow().as_java_string().map(|x| x.to_owned()))
                        })
                        .unwrap_or_default();
                    let input = mb.fields.get("input")
                        .or_else(|| mb.fields.get("__input"))
                        .and_then(|v| v.as_ref())
                        .and_then(|s| s.borrow().as_java_string().map(|x| x.to_owned()))
                        .unwrap_or_default();
                    (regex, input)
                };
                // Use captures to extract groups
                let anchored = format!("^(?:{regex})$");
                let re = regex::Regex::new(&anchored).ok();
                let caps = re.as_ref().and_then(|r| r.captures(&input));
                let ok = caps.is_some();
                // Store captured groups in __groups array field + matchStart/matchEnd
                if let Some(caps) = &caps {
                    let mut groups = Vec::new();
                    for i in 0..caps.len() {
                        if let Some(m) = caps.get(i) {
                            groups.push(JValue::Ref(Some(self.intern_string(m.as_str()))));
                        } else {
                            groups.push(JValue::Ref(None));
                        }
                    }
                    let groups_arr = JObject::new_array("[Ljava/lang/String;", groups);
                    let (ms, me) = caps.get(0).map(|m| (m.start() as i32, m.end() as i32)).unwrap_or((-1, -1));
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
                        .and_then(|s| s.borrow().as_java_string().map(|x| x.to_owned()))
                        .unwrap_or_default();
                    drop(mb);
                    if start >= 0 && end >= 0 && (end as usize) <= input.len() {
                        return Some(JValue::Ref(Some(self.intern_string(&input[start as usize..end as usize]))));
                    }
                } else {
                    drop(mb);
                }
                Some(JValue::Ref(None))
            }
            ("java/lang/Class", "getName") => {
                let internal = self
                    .class_target_from_mirror(this)
                    .map(|(_, name)| name)
                    .unwrap_or_else(|| "java/lang/Object".to_owned());
                Some(JValue::Ref(Some(
                    self.intern_string(Self::class_display_name(&internal)),
                )))
            }
            ("java/lang/Class", "getModifiers") => {
                let (target_id, target) = self
                    .class_target_from_mirror(this)
                    .unwrap_or((None, "java/lang/Object".to_owned()));
                if let Some(target_id) = target_id {
                    let _ = self.ensure_class_prepared(target_id);
                    let mods = self
                        .parsed_class(target_id)
                        .map(|cf| i32::from(cf.access_flags))
                        .unwrap_or(0);
                    return Some(JValue::Int(mods));
                }
                self.ensure_class_ready(&target);
                let mods = self
                    .get_class(&target)
                    .map(|cf| i32::from(cf.access_flags))
                    .unwrap_or(0);
                Some(JValue::Int(mods))
            }
            ("java/lang/Class", "isInstance") => {
                let (target_id, target) = self
                    .class_target_from_mirror(this)
                    .unwrap_or((None, "java/lang/Object".to_owned()));
                let result = match _args.first().and_then(|v| v.as_ref()) {
                    Some(obj) => target_id
                        .map(|class_id| self.is_instance_of_object_id(obj, class_id))
                        .unwrap_or_else(|| self.is_instance_of_object(obj, &target)),
                    None => false,
                };
                Some(JValue::Int(if result { 1 } else { 0 }))
            }
            ("java/lang/Class", "isAssignableFrom") => {
                let (target_id, target) = self
                    .class_target_from_mirror(this)
                    .unwrap_or((None, "java/lang/Object".to_owned()));
                let other_class = _args.first().and_then(|v| v.as_ref());
                let result = if let (Some(target_id), Some(other_class)) = (target_id, other_class) {
                    self.class_target_from_mirror(other_class)
                        .and_then(|(other_id, other_name)| {
                            other_id
                                .map(|other_id| self.is_instance_of_id(other_id, target_id))
                                .or_else(|| Some(self.is_instance_of(&other_name, &target)))
                        })
                        .unwrap_or_else(|| {
                            self.class_internal_name_from_obj(other_class)
                                .map(|other| self.is_instance_of(&other, &target))
                                .unwrap_or(false)
                        })
                } else {
                    other_class
                        .and_then(|c| self.class_internal_name_from_obj(c))
                        .map(|other| self.is_instance_of(&other, &target))
                        .unwrap_or(false)
                };
                Some(JValue::Int(if result { 1 } else { 0 }))
            }
            ("java/lang/Class", "isInterface") => {
                let (target_id, target) = self
                    .class_target_from_mirror(this)
                    .unwrap_or((None, "java/lang/Object".to_owned()));
                if let Some(target_id) = target_id {
                    let _ = self.ensure_class_prepared(target_id);
                    let is_iface = self
                        .parsed_class(target_id)
                        .map(|cf| (cf.access_flags & 0x0200) != 0)
                        .unwrap_or(false);
                    return Some(JValue::Int(if is_iface { 1 } else { 0 }));
                }
                self.ensure_class_ready(&target);
                let is_iface = self
                    .get_class(&target)
                    .map(|cf| (cf.access_flags & 0x0200) != 0)
                    .unwrap_or(false);
                Some(JValue::Int(if is_iface { 1 } else { 0 }))
            }
            ("java/lang/Class", "getComponentType") => {
                let target = self
                    .class_target_from_mirror(this)
                    .map(|(_, name)| name)
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
                let (target_id, target) = self
                    .class_target_from_mirror(this)
                    .unwrap_or((None, "java/lang/Object".to_owned()));
                if let Some(target_id) = target_id {
                    let super_id = self
                        .class_runtime_metadata_by_id(target_id)
                        .and_then(|metadata| metadata.super_id);
                    return Some(JValue::Ref(super_id.map(|id| self.class_object_by_id(id))));
                }
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
                let (target_id, target) = self
                    .class_target_from_mirror(this)
                    .unwrap_or((None, "java/lang/Object".to_owned()));
                if let Some(target_id) = target_id {
                    let interface_ids = self
                        .class_runtime_metadata_by_id(target_id)
                        .map(|metadata| metadata.interface_ids.clone())
                        .unwrap_or_default();
                    let vals = interface_ids
                        .iter()
                        .map(|&id| JValue::Ref(Some(self.class_object_by_id(id))))
                        .collect();
                    return Some(JValue::Ref(Some(JObject::new_array(
                        "[Ljava/lang/Class;",
                        vals,
                    ))));
                }
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
                let (target_id, target) = self
                    .class_target_from_mirror(this)
                    .unwrap_or((None, "java/lang/Object".to_owned()));
                if let Some(target_id) = target_id {
                    let _ = self.ensure_class_init(&target);
                    if let Some(JValue::Ref(Some(arr))) =
                        self.static_field_value_by_id(target_id, "$VALUES").cloned()
                    {
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
                        return Some(cloned);
                    }
                    return Some(JValue::Ref(None));
                }
                let _ = self.ensure_class_init(&target);
                if let Some(JValue::Ref(Some(arr))) = self.static_field_value(&target, "$VALUES")
                {
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
                let (target_id, target) = self
                    .class_target_from_mirror(this)
                    .unwrap_or((None, "java/lang/Object".to_owned()));
                if let Some(target_id) = target_id {
                    let _ = self.ensure_class_prepared(target_id);
                    let is_record = self
                        .parsed_class(target_id)
                        .map(|cf| {
                            cf.attributes
                                .iter()
                                .any(|a| matches!(a, Attribute::Record { .. }))
                        })
                        .unwrap_or(false);
                    return Some(JValue::Int(if is_record { 1 } else { 0 }));
                }
                self.ensure_class_ready(&target);
                let is_record = self
                    .get_class(&target)
                    .map(|cf| {
                        cf.attributes
                            .iter()
                            .any(|a| matches!(a, Attribute::Record { .. }))
                    })
                    .unwrap_or(false);
                Some(JValue::Int(if is_record { 1 } else { 0 }))
            }
            ("java/lang/Class", "getRecordComponents") => {
                let (target_id, target) = self
                    .class_target_from_mirror(this)
                    .unwrap_or((None, "java/lang/Object".to_owned()));
                let mut comps_meta: Vec<(String, String)> = Vec::new();
                if let Some(target_id) = target_id {
                    let _ = self.ensure_class_prepared(target_id);
                    if let Some(cf) = self.parsed_class(target_id) {
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
                } else {
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
                let (target_id, target) = self
                    .class_target_from_mirror(this)
                    .unwrap_or((None, "java/lang/Object".to_owned()));
                let anns = if let Some(target_id) = target_id {
                    let _ = self.ensure_class_prepared(target_id);
                    if let Some(cf) = self.parsed_class(target_id) {
                        let attrs = cf.attributes.clone();
                        let cp_entries = cf.constant_pool.entries.clone();
                        let cp = crate::class_file::ConstantPool { entries: cp_entries };
                        self.parse_runtime_visible_annotations(&attrs, &cp)
                    } else {
                        Vec::new()
                    }
                } else {
                    self.ensure_class_ready(&target);
                    if let Some(cf) = self.get_class(&target) {
                        let attrs = cf.attributes.clone();
                        let cp_entries = cf.constant_pool.entries.clone();
                        let cp = crate::class_file::ConstantPool { entries: cp_entries };
                        self.parse_runtime_visible_annotations(&attrs, &cp)
                    } else {
                        Vec::new()
                    }
                };
                Some(self.build_annotation_ref_array(anns))
            }
            ("java/lang/Class", "getDeclaredFields0") | ("java/lang/Class", "getDeclaredFields") => {
                let public_only = _args.first().map(|v| v.as_int() != 0).unwrap_or(false);
                let (target_id, target) = self
                    .class_target_from_mirror(this)
                    .unwrap_or((None, "java/lang/Object".to_owned()));
                let infos = target_id
                    .map(|class_id| self.declared_field_infos_id(class_id))
                    .unwrap_or_else(|| self.declared_field_infos(&target));
                let mut out = Vec::new();
                for info in infos.iter() {
                    if public_only && (info.access_flags & 0x0001) == 0 {
                        continue;
                    }
                    out.push(JValue::Ref(Some(
                        self.build_reflect_field_from_info(&target, info),
                    )));
                }
                Some(JValue::Ref(Some(JObject::new_array(
                    "[Ljava/lang/reflect/Field;",
                    out,
                ))))
            }
            ("java/lang/Class", "getDeclaredMethods0") | ("java/lang/Class", "getDeclaredMethods") => {
                let public_only = _args.first().map(|v| v.as_int() != 0).unwrap_or(false);
                let (target_id, target) = self
                    .class_target_from_mirror(this)
                    .unwrap_or((None, "java/lang/Object".to_owned()));
                let infos = target_id
                    .map(|class_id| self.declared_method_infos_id(class_id))
                    .unwrap_or_else(|| self.declared_method_infos(&target));
                let mut out = Vec::new();
                for info in infos.iter() {
                    if public_only && (info.access_flags & 0x0001) == 0 {
                        continue;
                    }
                    out.push(JValue::Ref(Some(
                        self.build_reflect_method_from_info(&target, info),
                    )));
                }
                Some(JValue::Ref(Some(JObject::new_array(
                    "[Ljava/lang/reflect/Method;",
                    out,
                ))))
            }
            ("java/lang/Class", "getDeclaredConstructors0")
            | ("java/lang/Class", "getDeclaredConstructors") => {
                let public_only = _args.first().map(|v| v.as_int() != 0).unwrap_or(false);
                let (target_id, target) = self
                    .class_target_from_mirror(this)
                    .unwrap_or((None, "java/lang/Object".to_owned()));
                let infos = target_id
                    .map(|class_id| self.declared_constructor_infos_id(class_id))
                    .unwrap_or_else(|| self.declared_constructor_infos(&target));
                let mut out = Vec::new();
                for info in infos.iter() {
                    if public_only && (info.access_flags & 0x0001) == 0 {
                        continue;
                    }
                    out.push(JValue::Ref(Some(
                        self.build_reflect_constructor_from_info(&target, info),
                    )));
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
                self.ensure_class_ready(&owner);
                let anns = if let Some(cf) = self.get_class(&owner) {
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
                    self.static_field_value(&owner, &name)
                        .unwrap_or_else(|| default_value_for_descriptor(&desc))
                } else {
                    match _args.first().and_then(|v| v.as_ref()) {
                        Some(target) => self
                            .get_object_field_value(target, &name)
                            .unwrap_or_else(|| default_value_for_descriptor(&desc)),
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
                    self.set_static_field_value(&owner, name, adapted);
                } else if let Some(target) = _args.first().and_then(|v| v.as_ref()) {
                    self.set_object_field_value(target, &name, adapted);
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
                self.ensure_class_ready(&owner);
                let ann_src = self.get_class(&owner).and_then(|cf| {
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
                // fromIndex (char-index): default = end of string.
                // JDK semantics: search backwards starting AT fromIndex (inclusive),
                // so slice up to the byte offset of fromIndex+1.
                let char_len = s.chars().count();
                let from_char = _args.get(1).map(|v| (v.as_int() as usize).min(char_len)).unwrap_or(char_len);
                let from_byte = char_to_byte_offset(&s, from_char.saturating_add(1).min(char_len));
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
            ("java/lang/String", "replace") => {
                let s = this.borrow().as_java_string().unwrap_or("").to_owned();
                if _args.len() >= 2 {
                    // replace(char, char)
                    if let (JValue::Int(old_c), JValue::Int(new_c)) = (&_args[0], &_args[1]) {
                        let old_ch = char::from_u32(*old_c as u32).unwrap_or('\u{FFFD}');
                        let new_ch = char::from_u32(*new_c as u32).unwrap_or('\u{FFFD}');
                        let result = s.replace(old_ch, &new_ch.to_string());
                        Some(JValue::Ref(Some(self.intern_string(result))))
                    } else {
                        // replace(CharSequence, CharSequence) — null args throw NPE per JDK spec
                        let old_ref = _args[0].as_ref();
                        let new_ref = _args[1].as_ref();
                        if old_ref.is_none() || new_ref.is_none() {
                            self.throw_null_pointer("String.replace: null argument");
                            return Some(JValue::Void);
                        }
                        let old_str = old_ref.unwrap().borrow().as_java_string().unwrap_or("").to_owned();
                        let new_str = new_ref.unwrap().borrow().as_java_string().unwrap_or("").to_owned();
                        let result = s.replace(&old_str, &new_str);
                        Some(JValue::Ref(Some(self.intern_string(result))))
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
                    class_id: src.class_id,
                    represented_class_id: src.represented_class_id,
                    field_slots: src.field_slots.clone(),
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
        let result = vm.native_classloader("getResourceAsStream", &[arg]);

        assert!(matches!(result, Some(JValue::Void)));
        let err = vm.pending_exception_err().expect("pending exception");
        assert!(err.contains("java/lang/RuntimeException"), "unexpected error: {err}");
        assert!(err.contains("broken.txt"), "unexpected error: {err}");
    }
}
