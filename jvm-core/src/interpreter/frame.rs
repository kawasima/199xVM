use std::rc::Rc;

use crate::heap::JValue;

pub(super) struct Frame {
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

/// Compare two `JValue`s by reference identity.
pub(super) fn refs_equal(a: &JValue, b: &JValue) -> bool {
    match (a, b) {
        (JValue::Ref(None), JValue::Ref(None)) => true,
        (JValue::Ref(Some(ra)), JValue::Ref(Some(rb))) => Rc::ptr_eq(ra, rb),
        _ => false,
    }
}
