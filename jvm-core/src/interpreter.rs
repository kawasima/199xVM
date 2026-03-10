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

use crate::class_file::{
    Attribute, BootstrapMethod, ClassFile, CodeAttribute, ConstantPoolEntry, MethodInfo,
};
use crate::heap::{JObject, JRef, JValue, NativePayload};

// ---------------------------------------------------------------------------
// VM state
// ---------------------------------------------------------------------------

/// The central virtual machine that holds loaded classes and drives execution.
pub struct Vm {
    /// Loaded class files keyed by internal name (`net/unit8/raoh/Result`).
    classes: HashMap<String, ClassFile>,
    /// Interned strings cache (not strictly required but saves allocations).
    string_pool: HashMap<String, JRef>,
}

impl Vm {
    /// Create an empty VM.
    pub fn new() -> Self {
        Vm { classes: HashMap::new(), string_pool: HashMap::new() }
    }

    /// Register a pre-parsed class file.
    pub fn load_class(&mut self, class_file: ClassFile) {
        let name = class_file.constant_pool.class_name(class_file.this_class).to_owned();
        self.classes.insert(name, class_file);
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
            return self.find_method(&super_name, method_name, descriptor);
        }
        None
    }

    /// Execute a static method and return its result.
    pub fn invoke_static(
        &mut self,
        class_name: &str,
        method_name: &str,
        descriptor: &str,
        args: Vec<JValue>,
    ) -> Result<JValue, String> {
        // Try native implementations first.
        if let Some(v) = self.native_static(class_name, method_name, descriptor, &args) {
            return Ok(v);
        }

        // Look up class/method.
        let (class_name_owned, descriptor_owned) = {
            let (class, method) = self
                .find_method(class_name, method_name, descriptor)
                .ok_or_else(|| format!("Method not found: {class_name}.{method_name}{descriptor}"))?;
            let cn = class.constant_pool.class_name(class.this_class).to_owned();
            let desc = class.constant_pool.utf8(method.descriptor_index).to_owned();
            (cn, desc)
        };

        // Build initial frame.
        let max_locals = {
            let (class, method) = self.find_method(&class_name_owned, method_name, &descriptor_owned).unwrap();
            method.code().map(|c| c.max_locals as usize).unwrap_or(0)
        };
        let mut locals = vec![JValue::Void; max_locals.max(args.len())];
        for (i, a) in args.into_iter().enumerate() {
            locals[i] = a;
        }

        let code = {
            let (_, method) = self.find_method(&class_name_owned, method_name, &descriptor_owned).unwrap();
            method.code().ok_or_else(|| format!("No code in {class_name_owned}.{method_name}"))?.code.clone()
        };
        let cp_entries: Vec<ConstantPoolEntry> = {
            let class = self.classes.get(&class_name_owned).unwrap();
            class.constant_pool.entries.clone()
        };
        let bootstrap_methods: Vec<BootstrapMethod> = {
            let class = self.classes.get(&class_name_owned).unwrap();
            class.attributes.iter().find_map(|a| {
                if let Attribute::BootstrapMethods(bms) = a {
                    Some(bms.clone())
                } else {
                    None
                }
            }).unwrap_or_default()
        };

        let mut frame = Frame {
            locals,
            stack: Vec::new(),
            pc: 0,
        };

        self.run_frame(&mut frame, &code, &cp_entries, &class_name_owned, &bootstrap_methods)
    }

    /// Execute an instance method (first local = `this` reference).
    pub fn invoke_virtual(
        &mut self,
        this: JRef,
        class_name: &str,
        method_name: &str,
        descriptor: &str,
        args: Vec<JValue>,
    ) -> Result<JValue, String> {
        // Use the actual runtime class of `this` for virtual dispatch.
        let runtime_class = this.borrow().class_name.clone();

        // Try native implementations first.
        if let Some(v) = self.native_virtual(&this, &runtime_class, method_name, descriptor, &args) {
            return Ok(v);
        }

        // Resolve method starting from the runtime class.
        let resolve_class = if self.classes.contains_key(&runtime_class) {
            runtime_class.clone()
        } else {
            class_name.to_owned()
        };

        let (class_name_owned, descriptor_owned) = {
            let (class, method) = self
                .find_method(&resolve_class, method_name, descriptor)
                .ok_or_else(|| {
                    format!("Virtual method not found: {resolve_class}.{method_name}{descriptor}")
                })?;
            let cn = class.constant_pool.class_name(class.this_class).to_owned();
            let desc = class.constant_pool.utf8(method.descriptor_index).to_owned();
            (cn, desc)
        };

        let max_locals = {
            let (_, method) = self.find_method(&class_name_owned, method_name, &descriptor_owned).unwrap();
            method.code().map(|c| c.max_locals as usize).unwrap_or(0)
        };

        // `this` goes into local[0], then arguments.
        let total = max_locals.max(args.len() + 1);
        let mut locals = vec![JValue::Void; total];
        locals[0] = JValue::Ref(Some(this));
        for (i, a) in args.into_iter().enumerate() {
            locals[i + 1] = a;
        }

        let code = {
            let (_, method) = self.find_method(&class_name_owned, method_name, &descriptor_owned).unwrap();
            method.code().ok_or_else(|| format!("No code in {class_name_owned}.{method_name}"))?.code.clone()
        };
        let cp_entries: Vec<ConstantPoolEntry> = {
            let class = self.classes.get(&class_name_owned).unwrap();
            class.constant_pool.entries.clone()
        };
        let bootstrap_methods: Vec<BootstrapMethod> = {
            let class = self.classes.get(&class_name_owned).unwrap();
            class.attributes.iter().find_map(|a| {
                if let Attribute::BootstrapMethods(bms) = a {
                    Some(bms.clone())
                } else {
                    None
                }
            }).unwrap_or_default()
        };

        let mut frame = Frame {
            locals,
            stack: Vec::new(),
            pc: 0,
        };

        self.run_frame(&mut frame, &code, &cp_entries, &class_name_owned, &bootstrap_methods)
    }

    // ------------------------------------------------------------------
    // Core interpreter loop
    // ------------------------------------------------------------------

    fn run_frame(
        &mut self,
        frame: &mut Frame,
        code: &[u8],
        cp: &[ConstantPoolEntry],
        class_name: &str,
        bootstrap_methods: &[BootstrapMethod],
    ) -> Result<JValue, String> {
        macro_rules! cp_get {
            ($idx:expr) => {
                &cp[$idx as usize]
            };
        }

        loop {
            let opcode = code[frame.pc];
            frame.pc += 1;

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
                    let hi = code[frame.pc] as i16;
                    let lo = code[frame.pc + 1] as i16;
                    frame.pc += 2;
                    frame.stack.push(JValue::Int(((hi << 8) | lo) as i32));
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
                    let idx = frame.stack.pop().unwrap().as_int() as usize;
                    let arr_ref = frame.stack.pop().unwrap();
                    if let Some(r) = arr_ref.as_ref() {
                        let elem = match &r.borrow().native {
                            NativePayload::Array(v) => v[idx].clone(),
                            _ => JValue::Ref(None),
                        };
                        frame.stack.push(elem);
                    } else {
                        return Err("NullPointerException: aaload".to_owned());
                    }
                }
                0x33 => { // baload
                    let idx = frame.stack.pop().unwrap().as_int() as usize;
                    let arr_ref = frame.stack.pop().unwrap();
                    if let Some(r) = arr_ref.as_ref() {
                        let elem = match &r.borrow().native {
                            NativePayload::ByteArray(v) => JValue::Int(v[idx] as i32),
                            _ => JValue::Int(0),
                        };
                        frame.stack.push(elem);
                    } else {
                        return Err("NullPointerException: baload".to_owned());
                    }
                }
                0x2e => { // iaload
                    let idx = frame.stack.pop().unwrap().as_int() as usize;
                    let arr_ref = frame.stack.pop().unwrap();
                    if let Some(r) = arr_ref.as_ref() {
                        let elem = match &r.borrow().native {
                            NativePayload::IntArray(v) => JValue::Int(v[idx]),
                            _ => JValue::Int(0),
                        };
                        frame.stack.push(elem);
                    } else {
                        return Err("NullPointerException: iaload".to_owned());
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
                    let idx = frame.stack.pop().unwrap().as_int() as usize;
                    let arr_ref = frame.stack.pop().unwrap();
                    if let Some(r) = arr_ref.as_ref() {
                        if let NativePayload::Array(ref mut v) = r.borrow_mut().native {
                            while v.len() <= idx { v.push(JValue::Ref(None)); }
                            v[idx] = val;
                        }
                    }
                }

                // ---- Stack manipulation ----
                0x57 => { frame.stack.pop(); }                                           // pop
                0x58 => { frame.stack.pop(); frame.stack.pop(); }                        // pop2
                0x59 => { let v = frame.stack.last().unwrap().clone(); frame.stack.push(v); } // dup
                0x5a => { // dup_x1
                    let v1 = frame.stack.pop().unwrap();
                    let v2 = frame.stack.pop().unwrap();
                    frame.stack.push(v1.clone());
                    frame.stack.push(v2);
                    frame.stack.push(v1);
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
                0x6c => { let b = frame.stack.pop().unwrap().as_int(); let a = frame.stack.pop().unwrap().as_int(); frame.stack.push(JValue::Int(a.wrapping_div(b))); } // idiv
                0x70 => { let b = frame.stack.pop().unwrap().as_int(); let a = frame.stack.pop().unwrap().as_int(); frame.stack.push(JValue::Int(a.wrapping_rem(b))); } // irem
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
                0x94 => { let b = frame.stack.pop().unwrap().as_long(); let a = frame.stack.pop().unwrap().as_long(); frame.stack.push(JValue::Int(a.cmp(&b) as i32)); } // lcmp

                // ---- Conversions ----
                0x85 => { let v = frame.stack.pop().unwrap().as_int(); frame.stack.push(JValue::Long(v as i64)); } // i2l
                0x86 => { let v = frame.stack.pop().unwrap().as_int(); frame.stack.push(JValue::Float(v as f32)); } // i2f
                0x87 => { let v = frame.stack.pop().unwrap().as_int(); frame.stack.push(JValue::Double(v as f64)); } // i2d
                0x88 => { let v = frame.stack.pop().unwrap().as_long() as i32; frame.stack.push(JValue::Int(v)); } // l2i
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
                0xac => return Ok(frame.stack.pop().unwrap()), // ireturn
                0xad => return Ok(frame.stack.pop().unwrap()), // lreturn
                0xae => return Ok(frame.stack.pop().unwrap()), // freturn
                0xaf => return Ok(frame.stack.pop().unwrap()), // dreturn
                0xb0 => return Ok(frame.stack.pop().unwrap()), // areturn
                0xb1 => return Ok(JValue::Void),               // return

                // ---- Field access ----
                0xb2 => { // getstatic
                    let idx = read_u16(code, &mut frame.pc);
                    let v = self.resolve_static_field(cp, idx)?;
                    frame.stack.push(v);
                }
                0xb3 => { // putstatic
                    frame.stack.pop(); // discard for now
                    frame.pc += 2;
                }
                0xb4 => { // getfield
                    let idx = read_u16(code, &mut frame.pc);
                    let obj_ref = frame.stack.pop().unwrap();
                    let v = self.resolve_instance_field(cp, idx, &obj_ref)?;
                    frame.stack.push(v);
                }
                0xb5 => { // putfield
                    let idx = read_u16(code, &mut frame.pc);
                    let val = frame.stack.pop().unwrap();
                    let obj_ref = frame.stack.pop().unwrap();
                    self.set_instance_field(cp, idx, &obj_ref, val)?;
                }

                // ---- Method invocation ----
                0xb6 => { // invokevirtual
                    let idx = read_u16(code, &mut frame.pc);
                    let result = self.dispatch_virtual(cp, idx, frame)?;
                    if !matches!(result, JValue::Void) { frame.stack.push(result); }
                }
                0xb7 => { // invokespecial
                    let idx = read_u16(code, &mut frame.pc);
                    let result = self.dispatch_special(cp, idx, frame)?;
                    if !matches!(result, JValue::Void) { frame.stack.push(result); }
                }
                0xb8 => { // invokestatic
                    let idx = read_u16(code, &mut frame.pc);
                    let result = self.dispatch_static(cp, idx, frame)?;
                    if !matches!(result, JValue::Void) { frame.stack.push(result); }
                }
                0xb9 => { // invokeinterface
                    let idx = read_u16(code, &mut frame.pc);
                    frame.pc += 2; // count + 0
                    let result = self.dispatch_interface(cp, idx, frame)?;
                    if !matches!(result, JValue::Void) { frame.stack.push(result); }
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
                    let class_name = resolve_class_name(cp, idx);
                    let obj = JObject::new(class_name);
                    frame.stack.push(JValue::Ref(Some(obj)));
                }
                0xbc => { // newarray
                    let atype = code[frame.pc]; frame.pc += 1;
                    let count = frame.stack.pop().unwrap().as_int() as usize;
                    let arr = match atype {
                        8 => JObject::new_array("[B", vec![JValue::Int(0); count]),
                        10 => JObject::new_array("[I", vec![JValue::Int(0); count]),
                        11 => JObject::new_array("[J", vec![JValue::Long(0); count]),
                        _ => JObject::new_array("[Ljava/lang/Object;", vec![JValue::Ref(None); count]),
                    };
                    frame.stack.push(JValue::Ref(Some(arr)));
                }
                0xbd => { // anewarray
                    let idx = read_u16(code, &mut frame.pc);
                    let elem_class = resolve_class_name(cp, idx);
                    let count = frame.stack.pop().unwrap().as_int() as usize;
                    let arr = JObject::new_array(
                        format!("[L{elem_class};"),
                        vec![JValue::Ref(None); count],
                    );
                    frame.stack.push(JValue::Ref(Some(arr)));
                }
                0xbe => { // arraylength
                    let arr_ref = frame.stack.pop().unwrap();
                    let len = match arr_ref.as_ref() {
                        Some(r) => match &r.borrow().native {
                            NativePayload::Array(v) => v.len() as i32,
                            NativePayload::ByteArray(v) => v.len() as i32,
                            NativePayload::IntArray(v) => v.len() as i32,
                            _ => 0,
                        },
                        None => return Err("NullPointerException: arraylength".to_owned()),
                    };
                    frame.stack.push(JValue::Int(len));
                }

                // ---- instanceof / checkcast ----
                0xc0 => { // checkcast
                    frame.pc += 2; // index (ignored for now)
                    // value stays on stack unchanged
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
                    let msg = match exc.as_ref() {
                        Some(r) => format!("Exception: {}", r.borrow().class_name),
                        None => "NullPointerException (athrow null)".to_owned(),
                    };
                    return Err(msg);
                }

                // ---- monitorenter / monitorexit (no-ops in single-threaded context) ----
                0xc2 | 0xc3 => { frame.stack.pop(); }

                other => {
                    return Err(format!(
                        "Unimplemented opcode 0x{other:02x} at pc {}",
                        frame.pc - 1
                    ));
                }
            }
        }
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
                let s = match &cp[*string_index as usize] {
                    ConstantPoolEntry::Utf8(s) => s.clone(),
                    _ => String::new(),
                };
                let obj = self.intern_string(s);
                frame.stack.push(JValue::Ref(Some(obj)));
            }
            ConstantPoolEntry::Class { name_index } => {
                let name = match &cp[*name_index as usize] {
                    ConstantPoolEntry::Utf8(s) => s.clone(),
                    _ => String::new(),
                };
                // Return a Class object stand-in with the class name stored.
                let obj = JObject::new_string(name);
                frame.stack.push(JValue::Ref(Some(obj)));
            }
            other => {
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
        let (class_name, field_name, _descriptor) = resolve_fieldref(cp, idx);
        // Well-known static fields.
        match (class_name.as_str(), field_name.as_str()) {
            ("java/lang/System", "out") => Ok(JValue::Ref(Some(JObject::new("java/io/PrintStream")))),
            ("java/lang/System", "err") => Ok(JValue::Ref(Some(JObject::new("java/io/PrintStream")))),
            _ => Ok(JValue::Ref(None)),
        }
    }

    fn resolve_instance_field(
        &mut self,
        cp: &[ConstantPoolEntry],
        idx: u16,
        obj_ref: &JValue,
    ) -> Result<JValue, String> {
        let (_, field_name, _) = resolve_fieldref(cp, idx);
        match obj_ref.as_ref() {
            Some(r) => {
                Ok(r.borrow().fields.get(&field_name).cloned().unwrap_or(JValue::Ref(None)))
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

    fn dispatch_static(
        &mut self,
        cp: &[ConstantPoolEntry],
        idx: u16,
        frame: &mut Frame,
    ) -> Result<JValue, String> {
        let (class_name, method_name, descriptor) = resolve_methodref(cp, idx);
        let n_args = count_args(&descriptor);
        let args = pop_args(frame, n_args);
        self.invoke_static(&class_name, &method_name, &descriptor, args)
    }

    fn dispatch_virtual(
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
            JValue::Ref(None) => Err(format!("NullPointerException: invokevirtual {class_name}.{method_name}")),
            _ => Err("Expected reference for invokevirtual".to_owned()),
        }
    }

    fn dispatch_special(
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
            JValue::Ref(None) => Err(format!("NullPointerException: invokespecial {class_name}.{method_name}")),
            _ => Err("Expected reference for invokespecial".to_owned()),
        }
    }

    fn dispatch_interface(
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
            JValue::Ref(None) => Err(format!("NullPointerException: invokeinterface {class_name}.{method_name}")),
            _ => Err("Expected reference for invokeinterface".to_owned()),
        }
    }

    /// Handle `invokedynamic` — currently supports the three bootstrap methods
    /// used by Raoh: LambdaMetafactory, StringConcatFactory, SwitchBootstraps.
    fn dispatch_invokedynamic(
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

        let bm = &bootstrap_methods[bm_index as usize];
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

                // The implementation method is in bootstrap argument 1 (index 1).
                // We store it as a lambda stub that will be resolved when called.
                let impl_handle_idx = bm.bootstrap_arguments.get(1).copied();

                let lambda = JObject::new_lambda(move |_args: Vec<JValue>| {
                    // Simplified: lambdas in Raoh are usually short decoder wrappers.
                    // Full implementation would invoke the impl method handle.
                    JValue::Ref(None)
                });
                Ok(JValue::Ref(Some(lambda)))
            }

            "java/lang/invoke/StringConcatFactory" => {
                // Pop arguments based on dynamic descriptor.
                let n_args = count_args(&descriptor);
                let args = pop_args(frame, n_args);
                let mut result = String::new();
                for a in &args {
                    match a {
                        JValue::Int(v) => result.push_str(&v.to_string()),
                        JValue::Long(v) => result.push_str(&v.to_string()),
                        JValue::Float(v) => result.push_str(&v.to_string()),
                        JValue::Double(v) => result.push_str(&v.to_string()),
                        JValue::Ref(Some(r)) => {
                            if let Some(s) = r.borrow().as_java_string() {
                                result.push_str(s);
                            } else {
                                result.push_str(&r.borrow().class_name);
                            }
                        }
                        _ => {}
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
    // ------------------------------------------------------------------

    /// Handle static native methods. Returns `None` if not a known native.
    fn native_static(
        &mut self,
        class_name: &str,
        method_name: &str,
        _descriptor: &str,
        args: &[JValue],
    ) -> Option<JValue> {
        match (class_name, method_name) {
            ("java/lang/Integer", "valueOf") => {
                let v = args[0].as_int();
                let obj = JObject::new("java/lang/Integer");
                obj.borrow_mut().fields.insert("value".to_owned(), JValue::Int(v));
                Some(JValue::Ref(Some(obj)))
            }
            ("java/lang/Integer", "parseInt") => {
                if let Some(s_ref) = args[0].as_ref() {
                    if let Some(s) = s_ref.borrow().as_java_string() {
                        let v = s.parse::<i32>().unwrap_or(0);
                        return Some(JValue::Int(v));
                    }
                }
                Some(JValue::Int(0))
            }
            ("java/lang/Long", "valueOf") => {
                let v = args[0].as_long();
                let obj = JObject::new("java/lang/Long");
                obj.borrow_mut().fields.insert("value".to_owned(), JValue::Long(v));
                Some(JValue::Ref(Some(obj)))
            }
            ("java/lang/Boolean", "valueOf") => {
                let v = args[0].as_int();
                let obj = JObject::new("java/lang/Boolean");
                obj.borrow_mut().fields.insert("value".to_owned(), JValue::Int(v));
                Some(JValue::Ref(Some(obj)))
            }
            ("java/util/Objects", "requireNonNull") => Some(args[0].clone()),
            ("java/util/Objects", "requireNonNullElse") => {
                if args[0].is_null() { Some(args[1].clone()) } else { Some(args[0].clone()) }
            }
            ("java/util/List", "of") | ("java/util/Arrays", "asList") => {
                let arr = JObject::new_array("java/util/List", args.to_vec());
                Some(JValue::Ref(Some(arr)))
            }
            ("java/util/Map", "of") => {
                let obj = JObject::new("java/util/HashMap");
                Some(JValue::Ref(Some(obj)))
            }
            ("java/util/Optional", "empty") => {
                let obj = JObject::new("java/util/Optional");
                obj.borrow_mut().fields.insert("value".to_owned(), JValue::Ref(None));
                Some(JValue::Ref(Some(obj)))
            }
            ("java/util/Optional", "of") | ("java/util/Optional", "ofNullable") => {
                let obj = JObject::new("java/util/Optional");
                obj.borrow_mut().fields.insert("value".to_owned(), args[0].clone());
                Some(JValue::Ref(Some(obj)))
            }
            _ => None,
        }
    }

    /// Handle instance native methods. Returns `None` if not a known native.
    fn native_virtual(
        &mut self,
        this: &JRef,
        class_name: &str,
        method_name: &str,
        _descriptor: &str,
        args: &[JValue],
    ) -> Option<JValue> {
        let cn = this.borrow().class_name.clone();
        match (cn.as_str(), method_name) {
            (_, "toString") => {
                let s = match &this.borrow().native {
                    NativePayload::JavaString(s) => s.clone(),
                    _ => format!("{}@{}", this.borrow().class_name, 0),
                };
                Some(JValue::Ref(Some(JObject::new_string(s))))
            }
            ("java/lang/Integer", "intValue") | ("java/lang/Integer", "longValue") => {
                Some(this.borrow().fields.get("value").cloned().unwrap_or(JValue::Int(0)))
            }
            ("java/lang/Long", "longValue") => {
                Some(this.borrow().fields.get("value").cloned().unwrap_or(JValue::Long(0)))
            }
            ("java/lang/Boolean", "booleanValue") => {
                Some(this.borrow().fields.get("value").cloned().unwrap_or(JValue::Int(0)))
            }
            ("java/io/PrintStream", "println") | ("java/io/PrintStream", "print") => {
                // Discard output — the playground will capture it differently.
                Some(JValue::Void)
            }
            ("java/util/ArrayList", "add") | ("java/util/ArrayList", "addAll") => {
                // Simplified: accept but don't store.
                Some(JValue::Int(1))
            }
            ("java/util/ArrayList", "size") | ("java/util/ArrayList", "isEmpty") => {
                Some(JValue::Int(0))
            }
            _ => None,
        }
    }

    /// Check if `runtime_class` is an instance of `target_class` (by name).
    fn is_instance_of(&self, runtime_class: &str, target_class: &str) -> bool {
        if runtime_class == target_class { return true; }
        // Check loaded class hierarchy.
        if let Some(class) = self.classes.get(runtime_class) {
            // Check interfaces.
            for &iface_idx in &class.interfaces {
                let iface_name = class.constant_pool.class_name(iface_idx);
                if iface_name == target_class { return true; }
            }
            // Check super class.
            if class.super_class != 0 {
                let super_name = class.constant_pool.class_name(class.super_class).to_owned();
                if super_name != "java/lang/Object" && self.is_instance_of(&super_name, target_class) {
                    return true;
                }
            }
        }
        false
    }
}

// ---------------------------------------------------------------------------
// Stack frame
// ---------------------------------------------------------------------------

struct Frame {
    locals: Vec<JValue>,
    stack: Vec<JValue>,
    pc: usize,
}

// ---------------------------------------------------------------------------
// Utility functions
// ---------------------------------------------------------------------------

fn read_i16(code: &[u8], pc: &mut usize) -> i16 {
    let hi = code[*pc] as i8 as i16;
    let lo = code[*pc + 1] as i16;
    *pc += 2;
    (hi << 8) | lo
}

fn read_u16(code: &[u8], pc: &mut usize) -> u16 {
    let v = u16::from_be_bytes([code[*pc], code[*pc + 1]]);
    *pc += 2;
    v
}

fn read_i32(code: &[u8], pc: &mut usize) -> i32 {
    let v = i32::from_be_bytes([code[*pc], code[*pc + 1], code[*pc + 2], code[*pc + 3]]);
    *pc += 4;
    v
}

fn resolve_class_name(cp: &[ConstantPoolEntry], idx: u16) -> String {
    match &cp[idx as usize] {
        ConstantPoolEntry::Class { name_index } => {
            match &cp[*name_index as usize] {
                ConstantPoolEntry::Utf8(s) => s.clone(),
                _ => String::new(),
            }
        }
        _ => String::new(),
    }
}

fn resolve_methodref(cp: &[ConstantPoolEntry], idx: u16) -> (String, String, String) {
    let (class_idx, nat_idx) = match &cp[idx as usize] {
        ConstantPoolEntry::Methodref { class_index, name_and_type_index }
        | ConstantPoolEntry::InterfaceMethodref { class_index, name_and_type_index } => {
            (*class_index, *name_and_type_index)
        }
        _ => return (String::new(), String::new(), String::new()),
    };
    let class_name = resolve_class_name(cp, class_idx);
    let (name, desc) = match &cp[nat_idx as usize] {
        ConstantPoolEntry::NameAndType { name_index, descriptor_index } => {
            let n = match &cp[*name_index as usize] { ConstantPoolEntry::Utf8(s) => s.clone(), _ => String::new() };
            let d = match &cp[*descriptor_index as usize] { ConstantPoolEntry::Utf8(s) => s.clone(), _ => String::new() };
            (n, d)
        }
        _ => (String::new(), String::new()),
    };
    (class_name, name, desc)
}

fn resolve_fieldref(cp: &[ConstantPoolEntry], idx: u16) -> (String, String, String) {
    let (class_idx, nat_idx) = match &cp[idx as usize] {
        ConstantPoolEntry::Fieldref { class_index, name_and_type_index } => {
            (*class_index, *name_and_type_index)
        }
        _ => return (String::new(), String::new(), String::new()),
    };
    let class_name = resolve_class_name(cp, class_idx);
    let (name, desc) = match &cp[nat_idx as usize] {
        ConstantPoolEntry::NameAndType { name_index, descriptor_index } => {
            let n = match &cp[*name_index as usize] { ConstantPoolEntry::Utf8(s) => s.clone(), _ => String::new() };
            let d = match &cp[*descriptor_index as usize] { ConstantPoolEntry::Utf8(s) => s.clone(), _ => String::new() };
            (n, d)
        }
        _ => (String::new(), String::new()),
    };
    (class_name, name, desc)
}

/// Count the number of method arguments from a JVM method descriptor.
/// E.g. `"(ILjava/lang/String;Z)V"` → 3
fn count_args(descriptor: &str) -> usize {
    let mut count = 0usize;
    let mut chars = descriptor.chars().peekable();
    if chars.next() != Some('(') { return 0; }
    loop {
        match chars.next() {
            Some(')') | None => break,
            Some('L') => {
                // Skip until ';'
                for c in chars.by_ref() { if c == ';' { break; } }
                count += 1;
            }
            Some('[') => {
                // Array prefix — peek next to decide if it consumes another token.
                if chars.peek() == Some(&'L') {
                    chars.next();
                    for c in chars.by_ref() { if c == ';' { break; } }
                } else {
                    chars.next(); // primitive after [
                }
                count += 1;
            }
            Some('J') | Some('D') => count += 1, // long/double (take 2 stack slots but 1 arg)
            Some(_) => count += 1,
        }
    }
    count
}

/// Pop `n` arguments from the operand stack, returned in call order.
fn pop_args(frame: &mut Frame, n: usize) -> Vec<JValue> {
    let mut args: Vec<JValue> = (0..n).map(|_| frame.stack.pop().unwrap_or(JValue::Void)).collect();
    args.reverse();
    args
}

/// Compare two `JValue`s by reference identity.
fn refs_equal(a: &JValue, b: &JValue) -> bool {
    match (a, b) {
        (JValue::Ref(None), JValue::Ref(None)) => true,
        (JValue::Ref(Some(ra)), JValue::Ref(Some(rb))) => Rc::ptr_eq(ra, rb),
        _ => false,
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
        assert_eq!(count_args("(Ljava/lang/Object;Lnet/unit8/raoh/Path;)Lnet/unit8/raoh/Result;"), 2);
    }
}
