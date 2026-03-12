use std::rc::Rc;

use crate::heap::JValue;

pub(crate) struct Frame {
    pub locals: Vec<JValue>,
    pub stack: Vec<JValue>,
    pub pc: usize,
}

/// Pop `n` arguments from the operand stack, returned in call order.
pub(super) fn pop_args(frame: &mut Frame, n: usize) -> Vec<JValue> {
    let mut args: Vec<JValue> = (0..n).map(|_| frame.stack.pop().unwrap_or(JValue::Void)).collect();
    args.reverse();
    args
}

pub(super) fn is_category2(v: &JValue) -> bool {
    matches!(v, JValue::Long(_) | JValue::Double(_))
}

/// Convert a Java array index (`i32`) to `usize`, returning `ArrayIndexOutOfBoundsException`
/// for negative values. Callers must still check upper bounds (use `.get(idx)` on the array).
pub(super) fn array_index(idx_i: i32) -> Result<usize, String> {
    if idx_i < 0 {
        Err(format!("java/lang/ArrayIndexOutOfBoundsException: Index {idx_i} out of bounds"))
    } else {
        Ok(idx_i as usize)
    }
}

/// Return an `ArrayIndexOutOfBoundsException` error for the given index.
pub(super) fn array_oob(idx_i: i32) -> String {
    format!("java/lang/ArrayIndexOutOfBoundsException: Index {idx_i} out of bounds")
}

/// Compare two `JValue`s by reference identity.
pub(super) fn refs_equal(a: &JValue, b: &JValue) -> bool {
    match (a, b) {
        (JValue::Ref(None), JValue::Ref(None)) => true,
        (JValue::Ref(Some(ra)), JValue::Ref(Some(rb))) => Rc::ptr_eq(ra, rb),
        _ => false,
    }
}
