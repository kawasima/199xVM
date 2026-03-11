//! Simple heap using reference-counted objects.
//!
//! Each [`JObject`] is represented as a reference-counted smart pointer.
//! This avoids a full GC implementation while being sufficient for short-lived
//! decoder invocations like Raoh.

use std::collections::HashMap;
use std::rc::Rc;
use std::cell::RefCell;

/// A Java value that can appear on the operand stack or in a local variable slot.
#[derive(Debug, Clone)]
pub enum JValue {
    Void,
    Int(i32),
    Long(i64),
    Float(f32),
    Double(f64),
    /// Reference to a heap-allocated object (or null).
    Ref(Option<JRef>),
    /// Return address (used by `jsr`/`ret`, rare in modern bytecode).
    ReturnAddress(u32),
}

impl JValue {
    /// Unwrap as `i32` or panic.
    pub fn as_int(&self) -> i32 {
        match self {
            JValue::Int(v) => *v,
            other => panic!("Expected Int, got {other:?}"),
        }
    }

    /// Unwrap as `i64` or panic.
    pub fn as_long(&self) -> i64 {
        match self {
            JValue::Long(v) => *v,
            other => panic!("Expected Long, got {other:?}"),
        }
    }

    /// Unwrap as `f32` or panic.
    pub fn as_float(&self) -> f32 {
        match self {
            JValue::Float(v) => *v,
            other => panic!("Expected Float, got {other:?}"),
        }
    }

    /// Unwrap as `f64` or panic.
    pub fn as_double(&self) -> f64 {
        match self {
            JValue::Double(v) => *v,
            other => panic!("Expected Double, got {other:?}"),
        }
    }

    /// Unwrap as object reference (may be null).
    /// `Int(0)` is treated as null because uninitialized local slots and
    /// some bytecode paths (e.g. `iconst_0` used as `aconst_null` equivalent)
    /// may leave an Int(0) where a reference is expected.
    pub fn as_ref(&self) -> Option<&JRef> {
        match self {
            JValue::Ref(r) => r.as_ref(),
            JValue::Void | JValue::Int(0) => None,
            other => panic!("Expected Ref, got {other:?}"),
        }
    }

    /// Returns `true` if this is a null reference.
    pub fn is_null(&self) -> bool {
        matches!(self, JValue::Ref(None))
    }
}

/// A reference-counted handle to a heap object.
pub type JRef = Rc<RefCell<JObject>>;

/// A heap-allocated Java object.
#[derive(Debug)]
pub struct JObject {
    /// The fully-qualified internal class name (e.g. `"java/lang/String"`).
    pub class_name: String,
    /// Instance fields keyed by field name.
    pub fields: HashMap<String, JValue>,
    /// Underlying native payload for special types (String content, arrays, etc.).
    pub native: NativePayload,
}

/// Native backing storage for built-in types.
pub enum NativePayload {
    None,
    /// `java.lang.String` content.
    JavaString(String),
    /// Object array (`[Ljava/lang/Object;` etc.).
    Array(Vec<JValue>),
    /// Byte/char/int primitive arrays.
    ByteArray(Vec<u8>),
    IntArray(Vec<i32>),
    LongArray(Vec<i64>),
    /// A Rust closure captured as a lambda stand-in.
    Lambda(Rc<dyn Fn(Vec<JValue>) -> JValue>),
    /// A lambda backed by a bytecode method handle.
    ///
    /// When the functional interface method is invoked, the VM looks up
    /// `impl_class::impl_method(impl_desc)` and prepends `captured` to the arguments.
    BytecodeLambda {
        impl_class: String,
        impl_method: String,
        impl_desc: String,
        /// JVM reference_kind: 5=invokeVirtual, 6=invokeStatic, 7=invokeSpecial, etc.
        ref_kind: u8,
        captured: Vec<JValue>,
    },
}

impl std::fmt::Debug for NativePayload {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            NativePayload::None => write!(f, "None"),
            NativePayload::JavaString(s) => write!(f, "JavaString({s:?})"),
            NativePayload::Array(v) => write!(f, "Array(len={})", v.len()),
            NativePayload::ByteArray(v) => write!(f, "ByteArray(len={})", v.len()),
            NativePayload::IntArray(v) => write!(f, "IntArray(len={})", v.len()),
            NativePayload::LongArray(v) => write!(f, "LongArray(len={})", v.len()),
            NativePayload::Lambda(_) => write!(f, "Lambda(...)"),
            NativePayload::BytecodeLambda { impl_class, impl_method, .. } => {
                write!(f, "BytecodeLambda({impl_class}::{impl_method})")
            }
        }
    }
}

impl JObject {
    /// Create a plain Java object with the given class name.
    pub fn new(class_name: impl Into<String>) -> JRef {
        Rc::new(RefCell::new(JObject {
            class_name: class_name.into(),
            fields: HashMap::new(),
            native: NativePayload::None,
        }))
    }

    /// Create a `java.lang.String` backed by a Rust `String`.
    pub fn new_string(s: impl Into<String>) -> JRef {
        Rc::new(RefCell::new(JObject {
            class_name: "java/lang/String".to_owned(),
            fields: HashMap::new(),
            native: NativePayload::JavaString(s.into()),
        }))
    }

    /// Create an object array.
    pub fn new_array(class_name: impl Into<String>, elements: Vec<JValue>) -> JRef {
        Rc::new(RefCell::new(JObject {
            class_name: class_name.into(),
            fields: HashMap::new(),
            native: NativePayload::Array(elements),
        }))
    }

    /// Create a lambda/closure object.
    pub fn new_lambda(f: impl Fn(Vec<JValue>) -> JValue + 'static) -> JRef {
        Rc::new(RefCell::new(JObject {
            class_name: "$$Lambda".to_owned(),
            fields: HashMap::new(),
            native: NativePayload::Lambda(Rc::new(f)),
        }))
    }

    /// Get the string content if this is a `java.lang.String`.
    pub fn as_java_string(&self) -> Option<&str> {
        match &self.native {
            NativePayload::JavaString(s) => Some(s),
            _ => None,
        }
    }
}
