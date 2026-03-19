
use crate::class_file::{BootstrapMethod, ConstantPoolEntry, ExceptionTableEntry};
use crate::heap::{JObject, JValue, NativePayload};

use super::Vm;
use super::descriptors::*;
use super::frame::*;

impl Vm {
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
        let exc_class = if err_msg.starts_with("java/") || err_msg.starts_with("javax/") {
            err_msg.split(':').next().unwrap_or(err_msg).trim()
        } else if let Some(rest) = err_msg.strip_prefix("Exception: ") {
            rest.split(':').next().unwrap_or(rest).trim()
        } else if err_msg.starts_with("NullPointerException") {
            "java/lang/NullPointerException"
        } else if err_msg.starts_with("ClassCastException") {
            "java/lang/ClassCastException"
        } else if err_msg.contains("ArithmeticException") {
            "java/lang/ArithmeticException"
        } else if err_msg.contains("StackOverflowError") {
            "java/lang/StackOverflowError"
        } else if err_msg.starts_with("UnsupportedOperationException") {
            "java/lang/UnsupportedOperationException"
        } else if err_msg.contains("IndexOutOfBoundsException") {
            "java/lang/IndexOutOfBoundsException"
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
        if let Some(r) = self.pending_exception_mut().take() {
            JValue::Ref(Some(r))
        } else {
            // Store the error message in a "detailMessage" field (matches JDK Throwable).
            // Strip the "Exception: classname" prefix to get just the meaningful message.
            let msg_str = err_msg.strip_prefix("Exception: ")
                .and_then(|s| {
                    // After stripping "Exception: ", the remainder is class name.
                    // If there's a ": " after the class name, extract the actual message.
                    s.find(": ").map(|i| &s[i + 2..])
                })
                .unwrap_or(err_msg);
            let exc = self.new_vm_exception(exc_class, Some(JObject::new_string(msg_str)));
            JValue::Ref(Some(exc))
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
                        let elem = match &r.borrow().native {
                            NativePayload::Array(v) => v.get(idx).cloned()
                                .ok_or_else(|| array_oob(idx_i))?,
                            _ => JValue::Ref(None),
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
                            NativePayload::Array(v) => JValue::Int(
                                v.get(idx).ok_or_else(|| array_oob(idx_i))?.as_int() as i8 as i32
                            ),
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
                    let v = self.resolve_static_field(cp, idx)?;
                    frame.stack.push(v);
                }
                0xb3 => { // putstatic
                    let idx = read_u16(code, &mut frame.pc);
                    let val = frame.stack.pop().unwrap_or(JValue::Void);
                    let (cls, fld, _) = resolve_fieldref(cp, idx);
                    // Per JVMS §5.5: putstatic triggers class initialization.
                    self.ensure_class_init(&cls)?;
                    self.static_fields.entry(cls).or_default().insert(fld, val);
                }
                0xb4 => { // getfield
                    let idx = read_u16(code, &mut frame.pc);
                    let (_, gf_field_name, _) = resolve_fieldref(cp, idx);
                    let obj_ref = frame.stack.pop()
                        .ok_or_else(|| format!("getfield {gf_field_name}: empty stack in {class_name}"))?;
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
                // dispatch_* methods now return Ok(None) always. They either:
                //   (a) store a FrameInfo in self.pending_frame for the trampoline, or
                //   (b) execute native inline and push the result onto frame.stack.
                0xb6 => { // invokevirtual
                    let idx = read_u16(code, &mut frame.pc);
                    self.dispatch_virtual(cp, idx, frame).map_err(|e| {
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
                    self.dispatch_static(cp, idx, frame)?;
                }
                0xb9 => { // invokeinterface
                    let idx = read_u16(code, &mut frame.pc);
                    frame.pc += 2; // count + 0
                    self.dispatch_interface(cp, idx, frame).map_err(|e| {
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
                    let new_class = resolve_class_name(cp, idx);
                    // Run <clinit> for the class being instantiated.
                    self.ensure_class_init(&new_class)?;
                    // A ParseError entry means the class was registered but malformed —
                    // surface consistently as ClassFormatError (same as Class.forName0 path).
                    if matches!(self.classes.get(&new_class), Some(super::LazyClass::ParseError(_))) {
                        self.throw_class_format_error(&new_class);
                        return Err(format!("java/lang/ClassFormatError: malformed class file for {new_class}"));
                    }
                    let obj = if self.get_class(&new_class).is_some() {
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
                    let elem_class = resolve_class_name(cp, idx);
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
                    let class_name_str = resolve_class_name(cp, idx);
                    let mut dim_sizes = Vec::with_capacity(dimensions);
                    for _ in 0..dimensions {
                        let n = frame.stack.pop().unwrap().as_int();
                        if n < 0 {
                            return Err(format!("java/lang/NegativeArraySizeException: {n}"));
                        }
                        dim_sizes.push(n as usize);
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
        self.ensure_class_init(&class_name)?;
        // Search this class and its super-class chain for the static field (JVMS §5.4.3.2).
        if let Some(v) = self.resolve_static_field_in_hierarchy(&class_name, &field_name) {
            return Ok(v);
        }
        // Well-known JDK static fields that cannot be initialised via <clinit>
        // because the JDK classes are not in the bundle.
        match (class_name.as_str(), field_name.as_str()) {
            ("java/lang/System", "out") => {
                if let Some(v) = self.static_fields.get("java/lang/System").and_then(|m| m.get("out")) {
                    return Ok(v.clone());
                }
                let v = JValue::Ref(Some(JObject::new_print_stream(false)));
                self.static_fields.entry(class_name).or_default().insert(field_name, v.clone());
                Ok(v)
            }
            ("java/lang/System", "err") => {
                if let Some(v) = self.static_fields.get("java/lang/System").and_then(|m| m.get("err")) {
                    return Ok(v.clone());
                }
                let v = JValue::Ref(Some(JObject::new_print_stream(true)));
                self.static_fields.entry(class_name).or_default().insert(field_name, v.clone());
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
                self.static_fields.entry(class_name).or_default().insert(field_name, v.clone());
                Ok(v)
            }
            _ => Ok(default_value_for_descriptor(&descriptor)),
        }
    }

    /// Walk the class hierarchy to find a static field value.
    fn resolve_static_field_in_hierarchy(&mut self, class_name: &str, field_name: &str) -> Option<JValue> {
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

}
