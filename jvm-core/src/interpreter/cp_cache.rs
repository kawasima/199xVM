use std::cell::RefCell;
use std::rc::Rc;

use crate::class_file::{BootstrapMethod, ConstantPoolEntry, ExceptionTableEntry};

use super::class_identity::ClassId;

/// A resolved static/special method entry ready for frame construction.
/// All resolution work (owner lookup, code extraction, descriptor parsing)
/// is done once and stored here for subsequent invocations.
pub(crate) struct ResolvedMethodEntry {
    /// Loader-scoped owner class identity when known.
    pub owner_class_id: Option<ClassId>,
    /// Legacy owner class name used when loader-scoped identity is not available.
    pub owner_class: String,
    /// Pre-extracted code bytes.
    pub code: Rc<Vec<u8>>,
    /// Exception table from the Code attribute.
    pub exception_table: Rc<Vec<ExceptionTableEntry>>,
    /// max_locals from the Code attribute.
    pub max_locals: usize,
    /// Number of argument slots (pre-counted from descriptor).
    pub arg_slot_count: usize,
    /// Method access_flags.
    pub access_flags: u16,
    /// Whether the method has a Code attribute (false = native).
    pub has_code: bool,
    /// Shared constant pool of the owning class.
    pub cp: Rc<Vec<ConstantPoolEntry>>,
    /// cpCache of the owning class (for frame construction).
    pub cache: CpCache,
    /// Bootstrap methods from the owning class.
    pub bootstrap_methods: Rc<Vec<BootstrapMethod>>,
    /// The resolved descriptor (may differ from call-site for generics).
    pub descriptor: String,
    /// Pre-parsed parameter type tokens (for local slot setup).
    pub param_tokens: Vec<String>,
    /// Whether the method returns void.
    pub is_void: bool,
    /// Whether the method is ACC_VARARGS (reserved for future varargs fast path).
    #[allow(dead_code)]
    pub is_varargs: bool,
    /// The method name (for frame_owner formatting).
    pub method_name: String,
}

/// A resolved field entry for field access bytecodes.
pub(crate) struct ResolvedFieldEntry {
    /// Loader-scoped class identity that owns the resolved field.
    pub owner_class_id: ClassId,
    /// Legacy class name that owns the field after hierarchy traversal.
    /// Static field storage remains name-keyed until Phase 5.
    pub owner_class: String,
    /// Field name.
    pub field_name: String,
    /// Field descriptor (for default value computation).
    pub field_descriptor: String,
    /// Field access flags used to validate static/instance opcode compatibility.
    pub access_flags: u16,
}

/// A cpCache entry: resolved method or field.
pub(crate) enum CpCacheEntry {
    Method(ResolvedMethodEntry),
    Field(ResolvedFieldEntry),
}

/// Per-constant-pool cache, indexed by cp entry index.
/// `None` means not yet resolved; `Some` means resolved and ready.
pub(crate) type CpCache = Rc<RefCell<Vec<Option<CpCacheEntry>>>>;

/// Create a new empty cpCache of the given size.
pub(crate) fn new_cp_cache(size: usize) -> CpCache {
    Rc::new(RefCell::new((0..size).map(|_| None).collect()))
}
