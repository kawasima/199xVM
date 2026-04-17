
use crate::class_file::{Attribute, BootstrapMethod, ConstantPoolEntry, ExceptionTableEntry};
use crate::heap::{JObject, JValue, NativePayload};

use super::Vm;
use super::cp_cache::{CpCache, CpCacheEntry, ResolvedFieldEntry};
use super::descriptors::*;
use super::frame::*;

impl Vm {
    fn normalize_exception_class_head<'a>(raw: &'a str) -> &'a str {
        let head = raw
            .split(" | ")
            .next()
            .unwrap_or(raw)
            .trim();
        head
            .split(": ")
            .next()
            .unwrap_or(head)
            .split(" at ")
            .next()
            .unwrap_or(head)
            .trim()
    }

    fn exception_class_from_err_msg<'a>(err_msg: &'a str) -> &'a str {
        if err_msg.starts_with("java/") || err_msg.starts_with("javax/") {
            return Self::normalize_exception_class_head(err_msg);
        }
        if let Some(rest) = err_msg.strip_prefix("Exception: ") {
            return Self::normalize_exception_class_head(rest);
        }
        if err_msg.starts_with("NullPointerException") {
            return "java/lang/NullPointerException";
        }
        if err_msg.starts_with("ClassCastException") {
            return "java/lang/ClassCastException";
        }
        if err_msg.contains("ArithmeticException") {
            return "java/lang/ArithmeticException";
        }
        if err_msg.contains("StackOverflowError") {
            return "java/lang/StackOverflowError";
        }
        if err_msg.starts_with("UnsupportedOperationException") {
            return "java/lang/UnsupportedOperationException";
        }
        if err_msg.contains("IndexOutOfBoundsException") {
            return "java/lang/IndexOutOfBoundsException";
        }
        // Last resort: keep execution flowing through catch(Exception).
        "java/lang/RuntimeException"
    }

    pub(super) fn ensure_pending_exception_from_err(&mut self, err_msg: &str) {
        if self.scheduler.current_thread().pending_exception.is_some() {
            return;
        }
        let exc_class = Self::exception_class_from_err_msg(err_msg);
        // Strip "Exception: <class>: " prefix when present.
        let msg_str = err_msg
            .strip_prefix("Exception: ")
            .and_then(|s| s.find(": ").map(|i| &s[i + 2..]))
            .unwrap_or(err_msg);
        let exc = self.new_vm_exception(exc_class, Some(JObject::new_string(msg_str)));
        *self.pending_exception_mut() = Some(exc);
    }

    /// Search exception_table for a matching handler.
    /// Returns (handler_pc, exception_object) if found.
    pub(crate) fn find_exception_handler(
        &mut self,
        _frame: &Frame,
        exception_table: &[ExceptionTableEntry],
        cp: &[ConstantPoolEntry],
        throw_pc: usize,
        err_msg: &str,
    ) -> Option<(usize, JValue)> {
        // Extract exception class name from error message.
        // Preferred format: "java/lang/SomeException: message" — extract the class name directly.
        let exc_class = Self::exception_class_from_err_msg(err_msg);

        for entry in exception_table {
            let start = entry.start_pc as usize;
            let end = entry.end_pc as usize;
            if throw_pc < start || throw_pc >= end {
                continue;
            }
            // catch_type == 0 means catch-all (finally).
            if entry.catch_type == 0 {
                let exc_obj = self.take_or_create_exception(err_msg);
                return Some((entry.handler_pc as usize, exc_obj));
            }
            // Resolve catch_type to class name and check if exception is instance.
            let catch_class = resolve_class_name_ref(cp, entry.catch_type);
            if exc_class == catch_class || self.is_instance_of(exc_class, catch_class) {
                let exc_obj = self.take_or_create_exception(err_msg);
                return Some((entry.handler_pc as usize, exc_obj));
            }
        }
        // No handler found — do NOT clear pending_exception here; it must survive
        // propagation through intermediate frames until a handler is found upstream.
        None
    }

    /// Take the pending exception object if set, or create a new one.
    fn take_or_create_exception(&mut self, err_msg: &str) -> JValue {
        if let Some(r) = self.pending_exception_mut().take() {
            JValue::Ref(Some(r))
        } else {
            self.ensure_pending_exception_from_err(err_msg);
            JValue::Ref(self.pending_exception_mut().take())
        }
    }

    /// Execute a single opcode. Returns:
    /// - Ok(Some(value)) if the method returns
    /// - Ok(None) if execution should continue
    /// - Err(msg) if an exception was thrown
    pub(crate) fn execute_opcode(
        &mut self,
        frame: &mut Frame,
        code: &[u8],
        cp: &[ConstantPoolEntry],
        cache: &CpCache,
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
                    let val = i16::from_be_bytes([code[frame.pc], code[frame.pc + 1]]);
                    frame.pc += 2;
                    frame.stack.push(JValue::Int(val as i32));
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
                    let idx_i = frame.stack.pop().unwrap().as_int();
                    let arr_ref = frame.stack.pop().unwrap();
                    let idx = array_index(idx_i)?;
                    if let Some(r) = arr_ref.as_ref() {
                        let arr = r.borrow();
                        if !arr.class_name.starts_with('[') {
                            return Err("java/lang/ArrayStoreException: aaload on non-array".to_owned());
                        }
                        let elem = match &arr.native {
                            NativePayload::Array(v) => v.get(idx)
                                .cloned()
                                .ok_or_else(|| array_oob(idx_i))?,
                            _ => {
                                return Err("java/lang/ArrayStoreException: aaload on non-reference array".to_owned());
                            }
                        };
                        frame.stack.push(elem);
                    } else {
                        return Err("NullPointerException: aaload".to_owned());
                    }
                }
                0x33 => { // baload
                    let idx_i = frame.stack.pop().unwrap().as_int();
                    let arr_ref = frame.stack.pop().unwrap();
                    let idx = array_index(idx_i)?;
                    if let Some(r) = arr_ref.as_ref() {
                        let elem = match &r.borrow().native {
                            NativePayload::ByteArray(v) => JValue::Int(
                                *v.get(idx).ok_or_else(|| array_oob(idx_i))? as i32
                            ),
                            NativePayload::Array(v) => {
                                let raw = v.get(idx).ok_or_else(|| array_oob(idx_i))?.clone();
                                let narrowed = self.adapt_value_for_descriptor("B", raw).as_int() as i8 as i32;
                                JValue::Int(narrowed)
                            }
                            _ => JValue::Int(0),
                        };
                        frame.stack.push(elem);
                    } else {
                        return Err("NullPointerException: baload".to_owned());
                    }
                }
                0x2e => { // iaload
                    let idx_i = frame.stack.pop().unwrap().as_int();
                    let arr_ref = frame.stack.pop().unwrap();
                    let idx = array_index(idx_i)?;
                    if let Some(r) = arr_ref.as_ref() {
                        let elem = match &r.borrow().native {
                            NativePayload::IntArray(v) => JValue::Int(
                                *v.get(idx).ok_or_else(|| array_oob(idx_i))?
                            ),
                            NativePayload::Array(v) => v.get(idx)
                                .ok_or_else(|| array_oob(idx_i))?.clone(),
                            _ => JValue::Int(0),
                        };
                        frame.stack.push(elem);
                    } else {
                        return Err("NullPointerException: iaload".to_owned());
                    }
                }
                0x2f | 0x30 | 0x31 | 0x35 => { // laload, faload, daload, saload
                    let idx_i = frame.stack.pop().unwrap().as_int();
                    let arr_ref = frame.stack.pop().unwrap();
                    let idx = array_index(idx_i)?;
                    if let Some(r) = arr_ref.as_ref() {
                        let elem = match &r.borrow().native {
                            NativePayload::Array(v) => v.get(idx)
                                .ok_or_else(|| array_oob(idx_i))?.clone(),
                            NativePayload::LongArray(v) => JValue::Long(
                                *v.get(idx).ok_or_else(|| array_oob(idx_i))?
                            ),
                            NativePayload::IntArray(v) => JValue::Int(
                                *v.get(idx).ok_or_else(|| array_oob(idx_i))?
                            ),
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
                    let idx_i = frame.stack.pop().unwrap().as_int();
                    let arr_ref = frame.stack.pop().unwrap();
                    let idx = array_index(idx_i)?;
                    match arr_ref.as_ref() {
                        None => return Err("NullPointerException: aastore".to_owned()),
                        Some(r) => {
                            let (arr_class, arr_len, is_ref_array) = {
                                let arr = r.borrow();
                                let class_name = arr.class_name.clone();
                                match &arr.native {
                                    NativePayload::Array(v) => (class_name, v.len(), true),
                                    NativePayload::ByteArray(v) => (class_name, v.len(), false),
                                    NativePayload::IntArray(v) => (class_name, v.len(), false),
                                    NativePayload::LongArray(v) => (class_name, v.len(), false),
                                    _ => (class_name, 0usize, false),
                                }
                            };
                            if !arr_class.starts_with('[') {
                                return Err("java/lang/ArrayStoreException: aastore on non-array".to_owned());
                            }
                            if !is_ref_array {
                                return Err("java/lang/ArrayStoreException: aastore on primitive array".to_owned());
                            }
                            if idx >= arr_len {
                                return Err(array_oob(idx_i));
                            }

                            // Reference arrays require runtime assignability checks.
                            let component_class = if arr_class.starts_with("[L") {
                                Some(
                                    arr_class
                                        .strip_prefix("[L")
                                        .and_then(|s| s.strip_suffix(';'))
                                        .unwrap_or("java/lang/Object")
                                        .to_owned(),
                                )
                            } else if arr_class.starts_with("[[") {
                                // Component is itself an array descriptor (e.g. [Ljava/lang/String;).
                                descriptor_to_class_name(&arr_class[1..])
                            } else {
                                None
                            };
                            if let Some(component) = component_class {
                                match &val {
                                    JValue::Ref(None) => {}
                                    JValue::Ref(Some(obj)) => {
                                        let runtime = obj.borrow().class_name.clone();
                                        if !self.is_instance_of(&runtime, &component) {
                                            return Err(format!(
                                                "java/lang/ArrayStoreException: {} into {}",
                                                runtime.replace('/', "."),
                                                component.replace('/', ".")
                                            ));
                                        }
                                    }
                                    _ => {
                                        return Err("java/lang/ArrayStoreException: primitive into reference array".to_owned());
                                    }
                                }
                            }

                            if let NativePayload::Array(ref mut v) = r.borrow_mut().native {
                                *v.get_mut(idx).ok_or_else(|| array_oob(idx_i))? = val;
                            }
                        }
                    }
                }

                0x4f => { // iastore
                    let val = frame.stack.pop().unwrap();
                    let idx_i = frame.stack.pop().unwrap().as_int();
                    let arr_ref = frame.stack.pop().unwrap();
                    let idx = array_index(idx_i)?;
                    match arr_ref.as_ref() {
                        None => return Err("NullPointerException: iastore".to_owned()),
                        Some(r) => match r.borrow_mut().native {
                            NativePayload::Array(ref mut v) => {
                                *v.get_mut(idx).ok_or_else(|| array_oob(idx_i))? = val;
                            }
                            NativePayload::IntArray(ref mut v) => {
                                *v.get_mut(idx).ok_or_else(|| array_oob(idx_i))? = val.as_int();
                            }
                            _ => {}
                        },
                    }
                }
                0x55 => { // castore (char array store — treated same as iastore)
                    let val = frame.stack.pop().unwrap();
                    let idx_i = frame.stack.pop().unwrap().as_int();
                    let arr_ref = frame.stack.pop().unwrap();
                    let idx = array_index(idx_i)?;
                    match arr_ref.as_ref() {
                        None => return Err("NullPointerException: castore".to_owned()),
                        Some(r) => {
                            if let NativePayload::Array(ref mut v) = r.borrow_mut().native {
                                *v.get_mut(idx).ok_or_else(|| array_oob(idx_i))? = val;
                            }
                        }
                    }
                }
                0x50 | 0x51 | 0x52 | 0x56 => { // lastore, fastore, dastore, sastore
                    let val = frame.stack.pop().unwrap();
                    let idx_i = frame.stack.pop().unwrap().as_int();
                    let arr_ref = frame.stack.pop().unwrap();
                    let idx = array_index(idx_i)?;
                    match arr_ref.as_ref() {
                        None => return Err("NullPointerException: array store".to_owned()),
                        Some(r) => match r.borrow_mut().native {
                            NativePayload::Array(ref mut v) => {
                                *v.get_mut(idx).ok_or_else(|| array_oob(idx_i))? = val;
                            }
                            NativePayload::LongArray(ref mut v) => {
                                *v.get_mut(idx).ok_or_else(|| array_oob(idx_i))? = val.as_long();
                            }
                            _ => {}
                        },
                    }
                }
                0x54 => { // bastore
                    let val = frame.stack.pop().unwrap().as_int() as u8;
                    let idx_i = frame.stack.pop().unwrap().as_int();
                    let arr_ref = frame.stack.pop().unwrap();
                    let idx = array_index(idx_i)?;
                    match arr_ref.as_ref() {
                        None => return Err("NullPointerException: bastore".to_owned()),
                        Some(r) => match r.borrow_mut().native {
                            NativePayload::ByteArray(ref mut v) => {
                                *v.get_mut(idx).ok_or_else(|| array_oob(idx_i))? = val;
                            }
                            NativePayload::Array(ref mut v) => {
                                *v.get_mut(idx).ok_or_else(|| array_oob(idx_i))? = JValue::Int(val as i32);
                            }
                            _ => {}
                        },
                    }
                }
                0x34 => { // caload (char array load)
                    let idx_i = frame.stack.pop().unwrap().as_int();
                    let arr_ref = frame.stack.pop().unwrap();
                    let idx = array_index(idx_i)?;
                    let val = match arr_ref.as_ref() {
                        Some(r) => match &r.borrow().native {
                            NativePayload::Array(v) => v.get(idx)
                                .ok_or_else(|| array_oob(idx_i))?.clone(),
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
                0x6c => { // idiv
                    let b = frame.stack.pop().unwrap().as_int();
                    if b == 0 {
                        return Err("java/lang/ArithmeticException: / by zero".to_string());
                    }
                    let a = frame.stack.pop().unwrap().as_int();
                    frame.stack.push(JValue::Int(a.wrapping_div(b)));
                }
                0x70 => { // irem
                    let b = frame.stack.pop().unwrap().as_int();
                    if b == 0 {
                        return Err("java/lang/ArithmeticException: / by zero".to_string());
                    }
                    let a = frame.stack.pop().unwrap().as_int();
                    frame.stack.push(JValue::Int(a.wrapping_rem(b)));
                }
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
                0x6d => { // ldiv
                    let b = frame.stack.pop().unwrap().as_long();
                    if b == 0 {
                        return Err("java/lang/ArithmeticException: / by zero".to_string());
                    }
                    let a = frame.stack.pop().unwrap().as_long();
                    frame.stack.push(JValue::Long(a.wrapping_div(b)));
                }
                0x71 => { // lrem
                    let b = frame.stack.pop().unwrap().as_long();
                    if b == 0 {
                        return Err("java/lang/ArithmeticException: / by zero".to_string());
                    }
                    let a = frame.stack.pop().unwrap().as_long();
                    frame.stack.push(JValue::Long(a.wrapping_rem(b)));
                }
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
                    if high < low {
                        return Err(format!("tableswitch: invalid range low={low} high={high}"));
                    }
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
                    // Fast path: check cpCache for resolved field metadata.
                    let cached_field = {
                        let cb = cache.borrow();
                        if let Some(Some(CpCacheEntry::Field(e))) = cb.get(idx as usize) {
                            Some((e.owner_class.clone(), e.field_name.clone()))
                        } else {
                            None
                        }
                    };
                    if let Some((owner, field)) = cached_field {
                        // getstatic triggers class initialization (JVMS §5.5),
                        // even when the field reference is already cached.
                        self.ensure_class_init(&owner)?;
                        if let Some(v) = self.static_fields.get(&owner).and_then(|m| m.get(&field)).cloned() {
                            frame.stack.push(v);
                        } else {
                            // Cached metadata can exist before a value is materialized
                            // (for example during initialization edge cases); resolve
                            // through the slow path instead of returning descriptor default.
                            let v = self.resolve_static_field(cp, idx, class_name)?;
                            frame.stack.push(v);
                        }
                    } else {
                        // Slow path: resolve, push, then populate cache.
                        let v = self.resolve_static_field(cp, idx, class_name)?;
                        frame.stack.push(v.clone());
                        // Populate cache with the resolved field owner.
                        let (declared_owner, fn_, fd) = resolve_fieldref_ref(cp, idx);
                        let mapped_owner =
                            self.remap_static_owner_for_field_access(class_name, declared_owner);
                        let owner = self
                            .find_static_field_owner_class(&mapped_owner, fn_)
                            .unwrap_or(mapped_owner);
                        cache.borrow_mut()[idx as usize] = Some(CpCacheEntry::Field(
                            ResolvedFieldEntry {
                                owner_class: owner,
                                field_name: fn_.to_owned(),
                                field_descriptor: fd.to_owned(),
                            }
                        ));
                    }
                }
                0xb3 => { // putstatic
                    let idx = read_u16(code, &mut frame.pc);
                    let val = frame.stack.pop().unwrap_or(JValue::Void);
                    // Fast path: check cpCache.
                    let cached = {
                        let cb = cache.borrow();
                        match cb.get(idx as usize) {
                            Some(Some(CpCacheEntry::Field(e))) => Some((
                                e.owner_class.clone(), e.field_name.clone(),
                            )),
                            _ => None,
                        }
                    };
                    if let Some((owner, field)) = cached {
                        self.check_final_static_field_write(&owner, &field, None, class_name)?;
                        // putstatic also triggers initialization of the declaring class.
                        self.ensure_class_init(&owner)?;
                        self.static_fields.entry(owner).or_default().insert(field, val);
                    } else {
                        // Slow path.
                        let (declared_owner, fld, fd) = resolve_fieldref_ref(cp, idx);
                        let mapped_owner =
                            self.remap_static_owner_for_field_access(class_name, declared_owner);
                        let resolved =
                            self.find_static_field_info(&mapped_owner, fld, Some(fd));
                        let owner = resolved
                            .as_ref()
                            .map(|(owner, _)| owner.clone())
                            .unwrap_or(mapped_owner);
                        if let Some((resolved_owner, access_flags)) = resolved.as_ref() {
                            self.check_final_static_field_write_resolved(
                                resolved_owner,
                                fld,
                                *access_flags,
                                class_name,
                            )?;
                        }
                        self.ensure_class_init(&owner)?;
                        self.static_fields.entry(owner.clone()).or_default().insert(fld.to_owned(), val);
                        // Populate cache.
                        cache.borrow_mut()[idx as usize] = Some(CpCacheEntry::Field(
                            ResolvedFieldEntry {
                                owner_class: owner,
                                field_name: fld.to_owned(),
                                field_descriptor: fd.to_owned(),
                            }
                        ));
                    }
                }
                0xb4 => { // getfield
                    let idx = read_u16(code, &mut frame.pc);
                    let (_, gf_field_name, _) = resolve_fieldref_ref(cp, idx);
                    let obj_ref = frame.stack.pop()
                        .ok_or_else(|| format!("getfield {gf_field_name}: empty stack in {class_name}"))?;
                    if matches!(obj_ref, JValue::Void) {
                        return Err(format!(
                            "getfield {gf_field_name}: expected Ref on stack, got Void in {class_name}"
                        ));
                    }
                    let v = self.resolve_instance_field(cp, cache, idx, &obj_ref)?;
                    frame.stack.push(v);
                }
                0xb5 => { // putfield
                    let idx = read_u16(code, &mut frame.pc);
                    let val = frame.stack.pop().unwrap_or(JValue::Void);
                    let obj_ref = frame.stack.pop().unwrap_or(JValue::Void);
                    if matches!(obj_ref, JValue::Void) {
                        let (_, pf_field_name, _) = resolve_fieldref_ref(cp, idx);
                        return Err(format!(
                            "putfield {pf_field_name}: expected Ref on stack, got Void in {class_name}"
                        ));
                    }
                    self.set_instance_field(cp, cache, idx, &obj_ref, val)?;
                }

                // ---- Method invocation ----
                // dispatch_* methods now return Ok(None) always. They either:
                //   (a) store a FrameInfo in self.pending_frame for the trampoline, or
                //   (b) execute native inline and push the result onto frame.stack.
                0xb6 => { // invokevirtual
                    let idx = read_u16(code, &mut frame.pc);
                    self.dispatch_virtual(cp, cache, idx, frame).map_err(|e| {
                        if e.starts_with("NullPointerException") { format!("{e} in {class_name}") } else { e }
                    })?;
                }
                0xb7 => { // invokespecial
                    let idx = read_u16(code, &mut frame.pc);
                    self.dispatch_special(cp, idx, frame).map_err(|e| {
                        if e.starts_with("NullPointerException") { format!("{e} in {class_name}") } else { e }
                    })?;
                }
                0xb8 => { // invokestatic
                    let idx = read_u16(code, &mut frame.pc);
                    self.dispatch_static(cp, cache, idx, class_name, frame)?;
                }
                0xb9 => { // invokeinterface
                    let idx = read_u16(code, &mut frame.pc);
                    frame.pc += 2; // count + 0
                    self.dispatch_interface(cp, cache, idx, frame).map_err(|e| {
                        if e.starts_with("NullPointerException") { format!("{e} in {class_name}") } else { e }
                    })?;
                }
                0xba => { // invokedynamic
                    let idx = read_u16(code, &mut frame.pc);
                    frame.pc += 2; // reserved bytes
                    let result = self.dispatch_invokedynamic(cp, idx, frame, class_name, bootstrap_methods)?;
                    if !matches!(result, JValue::Void) { frame.stack.push(result); }
                }

                // ---- Object creation ----
                0xbb => { // new
                    let idx = read_u16(code, &mut frame.pc);
                    let declared_class = resolve_class_name_ref(cp, idx);
                    let mapped_new_class = self.remap_declared_class_for_context(class_name, declared_class);
                    // Run <clinit> for the class being instantiated.
                    self.ensure_class_init(&mapped_new_class)?;
                    // A ParseError entry means the class was registered but malformed —
                    // surface consistently as ClassFormatError (same as Class.forName0 path).
                    if matches!(self.classes.get(&mapped_new_class), Some(super::LazyClass::ParseError(_))) {
                        self.throw_class_format_error(&mapped_new_class);
                        return Err(format!(
                            "java/lang/ClassFormatError: malformed class file for {mapped_new_class}"
                        ));
                    }
                    let obj = if self.get_class(&mapped_new_class).is_some() {
                        // Class is loaded (bytecode available) — use plain object.
                        JObject::new(mapped_new_class.clone())
                    } else {
                        match mapped_new_class.as_str() {
                            // JDK collection types backed by Array payload (no shim loaded).
                            "java/util/ArrayList" | "java/util/LinkedList" =>
                                JObject::new_array(mapped_new_class.clone(), vec![]),
                            _ => JObject::new(mapped_new_class.clone()),
                        }
                    };
                    if self.dynamically_defined_classes.contains(&mapped_new_class) {
                        if let Some(mut class_file) = self.get_class(&mapped_new_class).cloned() {
                            class_file.constant_pool.cache = std::rc::Rc::new(std::cell::RefCell::new(
                                vec![None; class_file.constant_pool.entries.len()],
                            ));
                            obj.borrow_mut().class_snapshot = Some(class_file);
                        }
                    }
                    frame.stack.push(JValue::Ref(Some(obj)));
                }
                0xbc => { // newarray
                    let atype = code[frame.pc]; frame.pc += 1;
                    let count_int = frame.stack.pop().unwrap().as_int();
                    if count_int < 0 {
                        return Err(format!("java/lang/NegativeArraySizeException: {count_int}"));
                    }
                    let count = count_int as usize;
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
                    let elem_class = resolve_class_name_ref(cp, idx);
                    let count_int = frame.stack.pop().unwrap().as_int();
                    if count_int < 0 {
                        return Err(format!("java/lang/NegativeArraySizeException: {count_int}"));
                    }
                    let count = count_int as usize;
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
                    let class_name_str = resolve_class_name_ref(cp, idx);
                    let mut dim_sizes = Vec::with_capacity(dimensions);
                    for _ in 0..dimensions {
                        let n = frame.stack.pop().unwrap().as_int();
                        if n < 0 {
                            return Err(format!("java/lang/NegativeArraySizeException: {n}"));
                        }
                        dim_sizes.push(n as usize);
                    }
                    dim_sizes.reverse();
                    let arr = self.create_multi_array(class_name_str, &dim_sizes, 0);
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
                    let declared_target = resolve_class_name_ref(cp, idx);
                    let target_class =
                        self.remap_declared_class_for_context(class_name, declared_target);
                    // Peek at top of stack (don't pop — value stays if check passes).
                    let obj = frame.stack.last()
                        .ok_or_else(|| "checkcast: empty stack".to_owned())?;
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
                    let declared_target = resolve_class_name_ref(cp, idx);
                    let target_class =
                        self.remap_declared_class_for_context(class_name, declared_target);
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
                            let msg = format!(
                                "{} at {}:pc{}",
                                self.format_exception_ref(&r),
                                class_name,
                                frame.pc.saturating_sub(1)
                            );
                            (msg, Some(r))
                        }
                        JValue::Ref(None) => {
                            let npe = self.new_vm_exception("java/lang/NullPointerException", None);
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
                        *self.pending_exception_mut() = Some(r);
                    }
                    return Err(msg);
                }

                // ---- monitorenter / monitorexit ----
                0xc2 => { // monitorenter
                    let obj_val = match frame.stack.pop() {
                        Some(v) => v,
                        None => return Err("Operand stack underflow in monitorenter".to_owned()),
                    };
                    match obj_val {
                        JValue::Ref(Some(r)) => self.monitor_enter(&r),
                        JValue::Ref(None) => return Err("java/lang/NullPointerException: monitorenter on null".to_owned()),
                        _ => return Err("Internal VM error: monitorenter on non-reference value".to_owned()),
                    }
                }
                0xc3 => { // monitorexit
                    let obj_val = match frame.stack.pop() {
                        Some(v) => v,
                        None => return Err("Operand stack underflow in monitorexit".to_owned()),
                    };
                    match obj_val {
                        JValue::Ref(Some(r)) => self.monitor_exit(&r)?,
                        JValue::Ref(None) => return Err("java/lang/NullPointerException: monitorexit on null".to_owned()),
                        _ => return Err("Internal VM error: monitorexit on non-reference value".to_owned()),
                    }
                }

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
                let s: &str = match &cp[*string_index as usize] {
                    ConstantPoolEntry::Utf8(s) => s.as_str(),
                    _ => "",
                };
                let obj = self.intern_string(s);
                frame.stack.push(JValue::Ref(Some(obj)));
            }
            ConstantPoolEntry::Class { name_index } => {
                let name = match &cp[*name_index as usize] {
                    ConstantPoolEntry::Utf8(s) => s.as_str(),
                    _ => "",
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
        frame_owner: &str,
    ) -> Result<JValue, String> {
        let (declared_class_name, field_name, descriptor) = resolve_fieldref_ref(cp, idx);
        let class_name_owned =
            self.remap_static_owner_for_field_access(frame_owner, declared_class_name);
        let class_name = class_name_owned.as_str();
        // Run <clinit> if not yet done (initialises static fields via putstatic).
        self.ensure_class_init(class_name)?;
        // Search this class and its super-class chain for the static field (JVMS §5.4.3.2).
        if let Some(v) = self.resolve_static_field_in_hierarchy(class_name, field_name) {
            return Ok(v);
        }
        // Some static finals are materialized via ConstantValue rather than <clinit>.
        if let Some(v) = self.resolve_constant_value_static_field(class_name, field_name) {
            return Ok(v);
        }
        // Well-known JDK static fields that cannot be initialised via <clinit>
        // because the JDK classes are not in the bundle.
        match (class_name, field_name) {
            ("java/lang/System", "out") => {
                if let Some(v) = self.static_fields.get("java/lang/System").and_then(|m| m.get("out")) {
                    return Ok(v.clone());
                }
                let v = JValue::Ref(Some(JObject::new_print_stream(false)));
                self.static_fields.entry(class_name.to_owned()).or_default().insert(field_name.to_owned(), v.clone());
                Ok(v)
            }
            ("java/lang/System", "err") => {
                if let Some(v) = self.static_fields.get("java/lang/System").and_then(|m| m.get("err")) {
                    return Ok(v.clone());
                }
                let v = JValue::Ref(Some(JObject::new_print_stream(true)));
                self.static_fields.entry(class_name.to_owned()).or_default().insert(field_name.to_owned(), v.clone());
                Ok(v)
            }
            ("java/lang/System", "in") => {
                if let Some(v) = self.static_fields.get("java/lang/System").and_then(|m| m.get("in")) {
                    if let Some(r) = v.as_ref() {
                        self.system_stdin = Some(r.clone());
                    }
                    return Ok(v.clone());
                }
                let stdin = JObject::new_process_pipe_input_stream();
                let v = JValue::Ref(Some(stdin.clone()));
                self.system_stdin = Some(stdin);
                self.static_fields.entry(class_name.to_owned()).or_default().insert(field_name.to_owned(), v.clone());
                Ok(v)
            }
            _ => Ok(default_value_for_descriptor(descriptor)),
        }
    }

    fn remap_static_owner_for_field_access(
        &self,
        frame_owner: &str,
        declared_class_name: &str,
    ) -> String {
        let Some(current_class) = Self::parse_frame_owner_for_field_access(frame_owner) else {
            return declared_class_name.to_owned();
        };
        if self.class_binary_name_for_lookup(current_class) == declared_class_name {
            return current_class.to_owned();
        }
        declared_class_name.to_owned()
    }

    pub(in crate::interpreter) fn resolve_constant_value_static_field(&mut self, class_name: &str, field_name: &str) -> Option<JValue> {
        self.ensure_class_ready(class_name);
        let (constant_idx, cp_entries, super_name, iface_names) = if let Some(class) = self.get_class(class_name) {
            let constant_idx = class.fields.iter().find_map(|field| {
                if (field.access_flags & 0x0008) == 0 {
                    return None;
                }
                let name = class.constant_pool.utf8(field.name_index);
                if name != field_name {
                    return None;
                }
                field.attributes.iter().find_map(|attr| {
                    if let Attribute::ConstantValue { constantvalue_index } = attr {
                        Some(*constantvalue_index)
                    } else {
                        None
                    }
                })
            });
            let cp_entries = Some(class.constant_pool.entries.clone());
            let sup = if class.super_class != 0 {
                Some(class.constant_pool.class_name(class.super_class).to_owned())
            } else {
                None
            };
            let ifaces = class
                .interfaces
                .iter()
                .map(|&idx| class.constant_pool.class_name(idx).to_owned())
                .collect::<Vec<_>>();
            (constant_idx, cp_entries, sup, ifaces)
        } else {
            (None, None, None, vec![])
        };
        let constant = match (cp_entries.as_deref(), constant_idx) {
            (Some(cp), Some(idx)) => self.constant_value_to_jvalue(cp, idx),
            _ => None,
        };
        if let Some(value) = constant {
            self.static_fields
                .entry(class_name.to_owned())
                .or_default()
                .insert(field_name.to_owned(), value.clone());
            return Some(value);
        }
        if let Some(super_name) = super_name {
            if let Some(v) = self.resolve_constant_value_static_field(&super_name, field_name) {
                return Some(v);
            }
        }
        for iface_name in iface_names {
            if let Some(v) = self.resolve_constant_value_static_field(&iface_name, field_name) {
                return Some(v);
            }
        }
        None
    }

    fn constant_value_to_jvalue(&mut self, cp: &[ConstantPoolEntry], idx: u16) -> Option<JValue> {
        match cp.get(idx as usize)? {
            ConstantPoolEntry::Integer(v) => Some(JValue::Int(*v)),
            ConstantPoolEntry::Long(v) => Some(JValue::Long(*v)),
            ConstantPoolEntry::Float(v) => Some(JValue::Float(*v)),
            ConstantPoolEntry::Double(v) => Some(JValue::Double(*v)),
            ConstantPoolEntry::String { string_index } => {
                let s = match cp.get(*string_index as usize)? {
                    ConstantPoolEntry::Utf8(s) => s.as_str(),
                    _ => return None,
                };
                Some(JValue::Ref(Some(self.intern_string(s))))
            }
            _ => None,
        }
    }

    /// Walk the class hierarchy to find a static field value.
    pub(in crate::interpreter) fn resolve_static_field_in_hierarchy(&mut self, class_name: &str, field_name: &str) -> Option<JValue> {
        // Check this class first.
        if let Some(v) = self.static_fields.get(class_name).and_then(|m| m.get(field_name)) {
            return Some(v.clone());
        }
        // Check super class and interfaces.
        self.ensure_class_ready(class_name);
        let (super_name, iface_names) = if let Some(class) = self.get_class(class_name) {
            let sup = if class.super_class != 0 {
                Some(class.constant_pool.class_name(class.super_class).to_owned())
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
        if let Some(super_name) = super_name {
            if let Some(v) = self.resolve_static_field_in_hierarchy(&super_name, field_name) {
                return Some(v);
            }
        }
        for iface_name in iface_names {
            if let Some(v) = self.resolve_static_field_in_hierarchy(&iface_name, field_name) {
                return Some(v);
            }
        }
        None
    }

    /// Walk the class hierarchy to find which class owns a static field.
    fn find_static_field_owner_class(&mut self, class_name: &str, field_name: &str) -> Option<String> {
        if self.static_fields.get(class_name).and_then(|m| m.get(field_name)).is_some() {
            return Some(class_name.to_owned());
        }
        self.ensure_class_ready(class_name);
        let (super_name, iface_names) = if let Some(class) = self.get_class(class_name) {
            let sup = if class.super_class != 0 {
                Some(class.constant_pool.class_name(class.super_class).to_owned())
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
            if let Some(owner) = self.find_static_field_owner_class(&s, field_name) {
                return Some(owner);
            }
        }
        for iface in iface_names {
            if let Some(owner) = self.find_static_field_owner_class(&iface, field_name) {
                return Some(owner);
            }
        }
        None
    }

    fn check_final_static_field_write(
        &mut self,
        owner: &str,
        field_name: &str,
        descriptor: Option<&str>,
        current_frame_owner: &str,
    ) -> Result<(), String> {
        let Some((resolved_owner, access_flags)) =
            self.find_static_field_info(owner, field_name, descriptor)
        else {
            return Ok(());
        };
        self.check_final_static_field_write_resolved(
            &resolved_owner,
            field_name,
            access_flags,
            current_frame_owner,
        )
    }

    fn check_final_static_field_write_resolved(
        &self,
        resolved_owner: &str,
        field_name: &str,
        access_flags: u16,
        current_frame_owner: &str,
    ) -> Result<(), String> {
        if (access_flags & 0x0010) == 0 {
            return Ok(());
        }
        let illegal_access = || {
            format!(
                "java/lang/IllegalAccessError: Update to static final field {resolved_owner}.{field_name} attempted from {current_frame_owner}"
            )
        };
        let Some(current_class) = Self::parse_frame_owner_for_field_access(current_frame_owner) else {
            return Err(illegal_access());
        };
        if current_class == resolved_owner {
            return Ok(());
        }
        Err(illegal_access())
    }

    fn find_static_field_info(
        &mut self,
        class_name: &str,
        field_name: &str,
        descriptor: Option<&str>,
    ) -> Option<(String, u16)> {
        self.ensure_class_ready(class_name);
        let (found, super_name, iface_names) = if let Some(class) = self.get_class(class_name) {
            let found = class.fields.iter().find_map(|field| {
                if (field.access_flags & 0x0008) == 0 {
                    return None;
                }
                let name = class.constant_pool.utf8(field.name_index);
                let desc = class.constant_pool.utf8(field.descriptor_index);
                if name == field_name && descriptor.map(|expected| expected == desc).unwrap_or(true) {
                    Some((class_name.to_owned(), field.access_flags))
                } else {
                    None
                }
            });
            let sup = if class.super_class != 0 {
                Some(class.constant_pool.class_name(class.super_class).to_owned())
            } else {
                None
            };
            let ifaces = class
                .interfaces
                .iter()
                .map(|&idx| class.constant_pool.class_name(idx).to_owned())
                .collect::<Vec<_>>();
            (found, sup, ifaces)
        } else {
            (None, None, vec![])
        };
        if found.is_some() {
            return found;
        }
        if let Some(super_name) = super_name {
            if let Some(info) = self.find_static_field_info(&super_name, field_name, descriptor) {
                return Some(info);
            }
        }
        for iface_name in iface_names {
            if let Some(info) = self.find_static_field_info(&iface_name, field_name, descriptor) {
                return Some(info);
            }
        }
        None
    }

    fn parse_frame_owner_for_field_access(frame_owner: &str) -> Option<&str> {
        let descriptor_start = frame_owner.find('(')?;
        let method_sep = frame_owner[..descriptor_start].rfind('.')?;
        Some(&frame_owner[..method_sep])
    }

    fn resolve_instance_field(
        &mut self,
        cp: &[ConstantPoolEntry],
        cache: &CpCache,
        idx: u16,
        obj_ref: &JValue,
    ) -> Result<JValue, String> {
        match obj_ref.as_ref() {
            Some(r) => {
                {
                    let cb = cache.borrow();
                    if let Some(Some(CpCacheEntry::Field(e))) = cb.get(idx as usize) {
                        let default = default_value_for_descriptor(&e.field_descriptor);
                        return Ok(r.borrow().fields.get(e.field_name.as_str()).cloned().unwrap_or(default));
                    }
                }
                let (class_name, field_name, field_desc) = resolve_fieldref_ref(cp, idx);
                cache.borrow_mut()[idx as usize] = Some(CpCacheEntry::Field(
                    ResolvedFieldEntry {
                        owner_class: class_name.to_owned(),
                        field_name: field_name.to_owned(),
                        field_descriptor: field_desc.to_owned(),
                    }
                ));
                let default = default_value_for_descriptor(field_desc);
                Ok(r.borrow().fields.get(field_name).cloned().unwrap_or(default))
            }
            None => {
                let (_, field_name, _) = resolve_fieldref_ref(cp, idx);
                Err(format!("NullPointerException: getfield {field_name}"))
            }
        }
    }

    fn set_instance_field(
        &mut self,
        cp: &[ConstantPoolEntry],
        cache: &CpCache,
        idx: u16,
        obj_ref: &JValue,
        val: JValue,
    ) -> Result<(), String> {
        match obj_ref.as_ref() {
            Some(r) => {
                {
                    let cb = cache.borrow();
                    if let Some(Some(CpCacheEntry::Field(e))) = cb.get(idx as usize) {
                        let mut obj = r.borrow_mut();
                        if let Some(slot) = obj.fields.get_mut(e.field_name.as_str()) {
                            *slot = val;
                        } else {
                            obj.fields.insert(e.field_name.clone(), val);
                        }
                        return Ok(());
                    }
                }
                let (class_name, field_name, field_desc) = resolve_fieldref_ref(cp, idx);
                cache.borrow_mut()[idx as usize] = Some(CpCacheEntry::Field(
                    ResolvedFieldEntry {
                        owner_class: class_name.to_owned(),
                        field_name: field_name.to_owned(),
                        field_descriptor: field_desc.to_owned(),
                    }
                ));
                let mut obj = r.borrow_mut();
                if let Some(slot) = obj.fields.get_mut(field_name) {
                    *slot = val;
                } else {
                    obj.fields.insert(field_name.to_owned(), val);
                }
                Ok(())
            }
            None => {
                let (_, field_name, _) = resolve_fieldref_ref(cp, idx);
                Err(format!("NullPointerException: putfield {field_name}"))
            }
        }
    }

}
