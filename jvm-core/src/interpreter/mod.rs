//! Java bytecode interpreter.
//!
//! Implements a stack-based interpreter over the JVM instruction set.
//! The focus is on the subset needed to run Raoh:
//! - Core stack / local-variable operations
//! - Object creation and field access
//! - Method invocation (all four flavours + `invokedynamic`)
//! - Integer / long / reference comparisons and control flow
//! - Native stubs for `java.lang.*` and `java.util.*`

use std::collections::{HashMap, HashSet, VecDeque};
use std::rc::Rc;

#[cfg(target_arch = "wasm32")]
use wasm_bindgen::prelude::*;

use crate::class_file::{
    self, Attribute, BootstrapMethod, ClassFile, ConstantPoolEntry, ExceptionTableEntry,
};
use crate::heap::{JObject, JRef, JValue};

type OwnedJarArchive = zip::ZipArchive<std::io::Cursor<Vec<u8>>>;

/// All execution-time data extracted from a resolved method in a single pass.
/// Returned by [`Vm::resolve_method_exec_info`] to avoid repeated `find_method`
/// calls and to give each field a self-documenting name.
#[derive(Clone)]
pub(super) struct MethodExecInfo {
    /// Internal class name that owns the resolved method.
    pub class_name: String,
    /// Resolved method descriptor (may differ from the call-site descriptor for generics).
    pub descriptor: String,
    /// `Code.max_locals` (0 if the method has no `Code` attribute).
    pub max_locals: usize,
    /// `true` when the method has a `Code` attribute (i.e. is not abstract/native).
    pub has_code: bool,
    /// Raw bytecode.
    pub code: Rc<Vec<u8>>,
    /// Exception handler table.
    pub exception_table: Rc<Vec<ExceptionTableEntry>>,
    /// Shared constant-pool entries (`Rc` for O(1) clone).
    pub cp: Rc<Vec<ConstantPoolEntry>>,
    /// Bootstrap methods from the `BootstrapMethods` attribute.
    pub bootstrap_methods: Rc<Vec<BootstrapMethod>>,
    /// `access_flags` from the method_info entry.
    pub access_flags: u16,
    /// Pre-parsed parameter descriptor tokens reused across frame construction.
    pub param_tokens: Rc<Vec<String>>,
    /// Number of local-variable slots consumed by parameters.
    pub param_slot_count: usize,
    /// Preformatted frame owner string used in diagnostics.
    pub frame_owner: Rc<str>,
}

#[derive(Clone)]
pub(super) struct ResolvedMemberRef {
    pub class_name: String,
    pub member_name: String,
    pub descriptor: String,
    pub arg_count: usize,
    pub returns_void: bool,
}

#[derive(Clone)]
pub(super) struct ResolvedStaticCallSite {
    pub method_name: String,
    pub expected_arg_count: usize,
    pub push_return: bool,
    pub empty_varargs: bool,
    pub method_info: Rc<MethodExecInfo>,
}

#[derive(Clone)]
pub(super) struct ResolvedVirtualCallSite {
    pub dispatch_class: String,
    pub method_name: String,
    pub method_info: Rc<MethodExecInfo>,
}

#[derive(Clone)]
pub(super) struct ReflectFieldInfo {
    pub name: String,
    pub descriptor: String,
    pub type_name: String,
    pub access_flags: u16,
}

#[derive(Clone)]
pub(super) struct ReflectMethodInfo {
    pub name: String,
    pub descriptor: String,
    pub param_types: Vec<String>,
    pub return_type: String,
    pub exception_types: Vec<String>,
    pub access_flags: u16,
}

#[derive(Clone)]
pub(super) struct ReflectConstructorInfo {
    pub descriptor: String,
    pub param_types: Vec<String>,
    pub exception_types: Vec<String>,
    pub access_flags: u16,
}

#[derive(Default)]
struct VmProfileStat {
    count: u64,
    nanos: u128,
}

struct VmProfiler {
    opcode_counts: [u64; 256],
    opcode_nanos: [u128; 256],
    method_stats: HashMap<Rc<str>, VmProfileStat>,
    for_name_stats: HashMap<String, VmProfileStat>,
    top_n: usize,
}

impl VmProfiler {
    fn from_env() -> Option<Self> {
        let enabled = matches!(
            std::env::var("JVM_PROFILE").ok().as_deref(),
            Some("1" | "true" | "TRUE" | "yes" | "YES")
        );
        if !enabled {
            return None;
        }
        let top_n = std::env::var("JVM_PROFILE_TOP")
            .ok()
            .and_then(|s| s.parse::<usize>().ok())
            .filter(|n| *n > 0)
            .unwrap_or(20);
        Some(Self {
            opcode_counts: [0; 256],
            opcode_nanos: [0; 256],
            method_stats: HashMap::new(),
            for_name_stats: HashMap::new(),
            top_n,
        })
    }

    fn record(&mut self, opcode: u8, frame_owner: &Rc<str>, elapsed: std::time::Duration) {
        let idx = usize::from(opcode);
        let nanos = elapsed.as_nanos();
        self.opcode_counts[idx] += 1;
        self.opcode_nanos[idx] += nanos;
        let stat = self.method_stats.entry(Rc::clone(frame_owner)).or_default();
        stat.count += 1;
        stat.nanos += nanos;
    }

    fn record_for_name(&mut self, runtime_name: &str, elapsed: std::time::Duration) {
        let stat = self.for_name_stats.entry(runtime_name.to_owned()).or_default();
        stat.count += 1;
        stat.nanos += elapsed.as_nanos();
    }

    fn report(&self) -> String {
        let mut lines = Vec::new();
        lines.push(format!("vm-profile top={}", self.top_n));

        let mut methods: Vec<_> = self.method_stats.iter().collect();
        methods.sort_by(|a, b| {
            b.1.nanos
                .cmp(&a.1.nanos)
                .then_with(|| a.0.as_ref().cmp(b.0.as_ref()))
        });
        lines.push("methods:".to_owned());
        for (name, stat) in methods.into_iter().take(self.top_n) {
            lines.push(format!(
                "  {:9.3} ms  {:8} ops  {}",
                stat.nanos as f64 / 1_000_000.0,
                stat.count,
                name
            ));
        }

        if !self.for_name_stats.is_empty() {
            let mut for_names: Vec<_> = self.for_name_stats.iter().collect();
            for_names.sort_by(|a, b| b.1.nanos.cmp(&a.1.nanos).then_with(|| a.0.cmp(b.0)));
            lines.push("forName:".to_owned());
            for (name, stat) in for_names.into_iter().take(self.top_n) {
                lines.push(format!(
                    "  {:9.3} ms  {:8} calls  {}",
                    stat.nanos as f64 / 1_000_000.0,
                    stat.count,
                    name
                ));
            }
        }

        let mut opcodes: Vec<_> = self
            .opcode_counts
            .iter()
            .enumerate()
            .filter_map(|(idx, count)| {
                if *count == 0 {
                    None
                } else {
                    Some((idx, *count, self.opcode_nanos[idx]))
                }
            })
            .collect();
        opcodes.sort_by(|a, b| b.2.cmp(&a.2).then_with(|| a.0.cmp(&b.0)));
        lines.push("opcodes:".to_owned());
        for (idx, count, nanos) in opcodes.into_iter().take(self.top_n) {
            lines.push(format!(
                "  0x{idx:02x}  {:9.3} ms  {:8} hits",
                nanos as f64 / 1_000_000.0,
                count
            ));
        }

        lines.push(String::new());
        lines.join("\n")
    }
}

#[cfg(test)]
mod tests {
    use super::{LazyClass, Vm};
    use std::io::{Cursor, Write};

    fn build_misnamed_jar() -> Vec<u8> {
        let mut archive = zip::ZipArchive::new(Cursor::new(include_bytes!("../../tests/test.jar").as_slice()))
            .expect("open test jar");
        let mut class_file = archive.by_name("JarTestEntry.class").expect("JarTestEntry.class");
        let mut class_bytes = Vec::new();
        std::io::Read::read_to_end(&mut class_file, &mut class_bytes).expect("read class bytes");

        let mut jar_bytes = Vec::new();
        {
            let cursor = Cursor::new(&mut jar_bytes);
            let mut writer = zip::ZipWriter::new(cursor);
            let options = zip::write::SimpleFileOptions::default()
                .compression_method(zip::CompressionMethod::Deflated);
            writer.start_file("wrong/Path.class", options).expect("start class entry");
            writer.write_all(&class_bytes).expect("write class entry");
            writer.finish().expect("finish jar");
        }
        jar_bytes
    }

    fn class_entries_in_test_jar() -> Vec<(String, Vec<u8>)> {
        let mut archive = zip::ZipArchive::new(Cursor::new(include_bytes!("../../tests/test.jar").as_slice()))
            .expect("open test jar");
        let mut classes = Vec::new();
        for i in 0..archive.len() {
            let mut file = archive.by_index(i).expect("test jar entry");
            let name = file.name().to_owned();
            let Some(class_name) = name.strip_suffix(".class") else {
                continue;
            };
            if class_name.is_empty() {
                continue;
            }
            let mut bytes = Vec::new();
            std::io::Read::read_to_end(&mut file, &mut bytes).expect("read class bytes");
            classes.push((class_name.to_owned(), bytes));
        }
        classes
    }

    #[test]
    fn jar_classes_stay_pending_until_first_access() {
        let mut vm = Vm::new();
        let count = vm.load_jar(include_bytes!("../../tests/test.jar")).expect("load_jar failed");
        assert!(count > 0, "expected at least one class in test JAR");
        assert!(matches!(vm.classes.get("JarTestEntry"), Some(LazyClass::PendingJarEntry(_))));

        vm.ensure_class_ready("JarTestEntry");

        assert!(matches!(vm.classes.get("JarTestEntry"), Some(LazyClass::Ready(_))));
    }

    #[test]
    fn misnamed_jar_class_uses_entry_path_and_fails_on_parse() {
        let mut vm = Vm::new();
        let count = vm.load_jar(&build_misnamed_jar()).expect("load_jar failed");
        assert_eq!(count, 1, "expected one class in misnamed jar");
        assert!(matches!(vm.classes.get("wrong/Path"), Some(LazyClass::PendingJarEntry(_))));
        assert!(vm.resolve_class("JarTestEntry").is_none(), "must not recover by internal name");

        vm.ensure_class_ready("wrong/Path");

        match vm.classes.get("wrong/Path") {
            Some(LazyClass::ParseError(err)) => {
                assert!(err.contains("Class name mismatch"), "unexpected error: {err}");
                assert!(err.contains("wrong/Path"), "unexpected error: {err}");
                assert!(err.contains("JarTestEntry"), "unexpected error: {err}");
            }
            _ => panic!("expected ParseError for misnamed JAR entry"),
        }
    }

    #[test]
    fn missing_packaged_lookup_leaves_pending_jar_entries_untouched() {
        let class_names: Vec<String> = class_entries_in_test_jar()
            .into_iter()
            .map(|(name, _)| name)
            .collect();
        let mut vm = Vm::new();
        vm.load_jar(include_bytes!("../../tests/test.jar")).expect("load test jar");

        for class_name in &class_names {
            assert!(
                matches!(vm.classes.get(class_name), Some(LazyClass::PendingJarEntry(_))),
                "expected pending jar entry before miss: {class_name}"
            );
        }

        vm.ensure_class_ready("missing/Type");

        for class_name in &class_names {
            assert!(
                matches!(vm.classes.get(class_name), Some(LazyClass::PendingJarEntry(_))),
                "packaged miss must not parse or rewrite pending entry: {class_name}"
            );
        }
        assert!(vm.resolve_class("missing/Type").is_none(), "missing class must remain unresolved");
    }
}

/// A class entry in the VM's class registry.
///
/// `PendingBytes` and `PendingJarEntry` are promoted to `Ready` on first access,
/// implementing standard ClassLoader lazy-loading semantics for both flat bundles
/// and JAR-backed classes.
#[derive(Debug, Clone)]
pub(in crate::interpreter) struct JarEntryRef {
    pub jar_id: usize,
    pub entry_index: usize,
    pub entry_name: String,
}

pub(in crate::interpreter) enum LazyClass {
    /// Raw bytes not yet parsed.
    PendingBytes(Vec<u8>),
    /// JAR entry that should be decompressed only when first accessed.
    PendingJarEntry(JarEntryRef),
    /// Fully parsed class file.
    Ready(ClassFile),
    /// Bytes were present but could not be parsed (malformed class).
    /// The entry is preserved so callers can distinguish "never registered"
    /// from "registered but broken", and to avoid repeated parse attempts.
    /// The inner `String` holds the original parse error message.
    ParseError(String),
}

mod annotations;
mod bytecode;
mod descriptors;
mod dispatch;
mod frame;
mod invoke;
pub(crate) mod launcher;
mod native_static;
mod native_virtual;
mod reflection;
pub(crate) mod trampoline;

use descriptors::*;

#[cfg(target_arch = "wasm32")]
#[wasm_bindgen]
extern "C" {
    #[wasm_bindgen(js_namespace = console, js_name = log)]
    fn console_log(s: &str);
    #[wasm_bindgen(js_namespace = console, js_name = error)]
    fn console_error(s: &str);
}

// ---------------------------------------------------------------------------
// Thread types
// ---------------------------------------------------------------------------

pub type ThreadId = u64;

/// Process-style stdio handling exposed by the launcher layer.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StdioMode {
    Pipe,
    Ignore,
    Inherit,
}

impl StdioMode {
    pub fn from_spec(spec: &str) -> Result<Self, String> {
        match spec {
            "pipe" => Ok(Self::Pipe),
            "ignore" => Ok(Self::Ignore),
            "inherit" => Ok(Self::Inherit),
            _ => Err(format!("Unsupported stdio mode: {spec}")),
        }
    }
}

/// The execution state of a green thread.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[allow(dead_code)]
pub(crate) enum ThreadState {
    /// Ready to run (or currently running).
    Runnable,
    /// Blocked waiting to acquire an object monitor.
    WaitingOnMonitor(usize),
    /// Waiting on Object.wait() condition.
    WaitingOnCondition(usize),
    /// Waiting for another thread to terminate (Thread.join).
    Joining(ThreadId),
    /// Sleeping (Thread.sleep).
    Sleeping,
    /// Yielded — Thread.yield() requested a voluntary context switch.
    /// The scheduler sets this back to Runnable after switching.
    Yielded,
    /// Terminated — run() method returned or threw.
    Terminated,
}

/// A reentrant object monitor (JVMS §5.4.2.1).
///
/// Each Java object has an associated monitor. A thread can lock the monitor
/// multiple times (reentrant); the monitor is released only when the count
/// drops to zero.
#[allow(dead_code)]
pub(crate) struct Monitor {
    /// The thread that currently owns this monitor, or `None` if unlocked.
    pub owner: Option<ThreadId>,
    /// Reentrant lock count (0 when unlocked).
    pub count: usize,
    /// Threads blocked on `monitorenter` waiting to acquire this monitor.
    pub entry_queue: VecDeque<ThreadId>,
    /// Threads blocked on `Object.wait()`.
    pub wait_queue: VecDeque<ThreadId>,
}

/// Per-thread execution context.
#[allow(dead_code)]
pub(crate) struct ThreadContext {
    pub id: ThreadId,
    pub state: ThreadState,
    /// Pending exception object — set by athrow, consumed by exception handler.
    pub pending_exception: Option<JRef>,
    /// Pending frame to be pushed by the trampoline after an invoke opcode.
    pub pending_frame: Option<trampoline::FrameInfo>,
    /// The thread's own call stack (for green thread scheduling).
    pub call_stack: Vec<trampoline::FrameInfo>,
    /// `<clinit>` frames currently active while the thread's call stack is borrowed out.
    pub active_clinit_stack: Vec<String>,
    /// The java.lang.Thread object associated with this thread.
    pub thread_object: Option<JRef>,
    /// Number of instructions executed in the current time slice.
    pub instruction_count: usize,
    /// Saved monitor reentrant count for Object.wait() — restored after notify.
    pub saved_monitor_count: usize,
}

impl ThreadContext {
    fn new(id: ThreadId) -> Self {
        ThreadContext {
            id,
            state: ThreadState::Runnable,
            pending_exception: None,
            pending_frame: None,
            call_stack: Vec::new(),
            active_clinit_stack: Vec::new(),
            thread_object: None,
            instruction_count: 0,
            saved_monitor_count: 0,
        }
    }
}

/// Round-robin scheduler managing all green threads.
#[allow(dead_code)]
pub(crate) struct Scheduler {
    threads: Vec<ThreadContext>,
    current_thread_idx: usize,
    next_thread_id: ThreadId,
}

/// Maximum instructions per thread before yielding to the next runnable thread.
const TIME_SLICE: usize = 1000;
/// Larger slice used when effectively only one runnable thread exists.
const SINGLE_RUNNABLE_TIME_SLICE: usize = 20_000;

impl Scheduler {
    pub(in crate::interpreter) fn new() -> Self {
        let main_thread = ThreadContext::new(0);
        Scheduler {
            threads: vec![main_thread],
            current_thread_idx: 0,
            next_thread_id: 1,
        }
    }

    /// Get a mutable reference to the currently running thread.
    #[inline]
    pub fn current_thread_mut(&mut self) -> &mut ThreadContext {
        &mut self.threads[self.current_thread_idx]
    }

    /// Get an immutable reference to the currently running thread.
    #[inline]
    pub fn current_thread(&self) -> &ThreadContext {
        &self.threads[self.current_thread_idx]
    }

    /// Spawn a new green thread and return its ID.
    pub fn spawn(&mut self, thread_object: Option<JRef>) -> ThreadId {
        let id = self.next_thread_id;
        self.next_thread_id += 1;
        let mut ctx = ThreadContext::new(id);
        ctx.thread_object = thread_object;
        self.threads.push(ctx);
        id
    }

    /// Get a mutable reference to a thread by ID.
    pub fn thread_mut(&mut self, id: ThreadId) -> Option<&mut ThreadContext> {
        self.threads.iter_mut().find(|t| t.id == id)
    }

    /// Get an immutable reference to a thread by ID.
    pub fn thread(&self, id: ThreadId) -> Option<&ThreadContext> {
        self.threads.iter().find(|t| t.id == id)
    }

    /// Returns true if all threads are terminated.
    pub fn all_terminated(&self) -> bool {
        self.threads.iter().all(|t| t.state == ThreadState::Terminated)
    }

    /// Returns true if only the main thread (id=0) is alive.
    pub fn only_main_alive(&self) -> bool {
        self.threads.iter().all(|t| t.id == 0 || t.state == ThreadState::Terminated)
    }

    /// Advance to the next runnable thread (round-robin).
    /// Returns true if a runnable thread was found.
    pub fn advance(&mut self) -> bool {
        let n = self.threads.len();
        for i in 1..=n {
            let idx = (self.current_thread_idx + i) % n;
            if self.threads[idx].state == ThreadState::Runnable {
                self.current_thread_idx = idx;
                return true;
            }
        }
        false
    }

    /// Check if any Joining threads should be woken because their target terminated.
    pub fn wake_joiners(&mut self) {
        // Collect terminated thread IDs into a HashSet for O(1) lookup.
        let terminated: HashSet<ThreadId> = self.threads.iter()
            .filter(|t| t.state == ThreadState::Terminated)
            .map(|t| t.id)
            .collect();
        // Wake any thread that was Joining on a terminated thread.
        for t in &mut self.threads {
            if let ThreadState::Joining(target_id) = t.state {
                if terminated.contains(&target_id) {
                    t.state = ThreadState::Runnable;
                }
            }
        }
    }

    /// Return the number of runnable threads.
    pub fn runnable_count(&self) -> usize {
        self.threads.iter().filter(|t| t.state == ThreadState::Runnable).count()
    }

    /// Return the total number of threads.
    pub fn thread_count(&self) -> usize {
        self.threads.len()
    }

    /// Reset the current thread index to the main thread (id=0, idx=0).
    pub fn reset_to_main(&mut self) {
        self.current_thread_idx = 0;
    }

    /// Find the thread ID associated with a java.lang.Thread object (by pointer identity).
    pub fn find_thread_id_by_object(&self, thread_obj: &JRef) -> Option<ThreadId> {
        let target_ptr = Rc::as_ptr(thread_obj) as usize;
        self.threads.iter().find_map(|t| {
            if let Some(ref obj) = t.thread_object {
                if Rc::as_ptr(obj) as usize == target_ptr {
                    return Some(t.id);
                }
            }
            None
        })
    }

    /// Return a summary of non-terminated thread states (for error diagnostics).
    pub fn alive_thread_summary(&self) -> String {
        self.threads.iter()
            .filter(|t| t.state != ThreadState::Terminated)
            .map(|t| format!("thread {}={:?}", t.id, t.state))
            .collect::<Vec<_>>()
            .join(", ")
    }
}

// ---------------------------------------------------------------------------
// VM state
// ---------------------------------------------------------------------------

/// The central virtual machine that holds loaded classes and drives execution.
pub struct Vm {
    /// Class registry: keyed by internal name (`net/unit8/raoh/Result`).
    /// Entries start as `LazyClass::PendingBytes`/`PendingJarEntry` and are promoted to
    /// `LazyClass::Ready` (parsed `ClassFile`) on first access.
    pub(in crate::interpreter) classes: HashMap<String, LazyClass>,
    /// Interned strings cache (not strictly required but saves allocations).
    pub(in crate::interpreter) string_pool: HashMap<String, JRef>,
    /// Static field storage keyed by class name → field name.
    /// Avoids allocating a `"ClassName.fieldName"` string on every getstatic/putstatic.
    pub(in crate::interpreter) static_fields: HashMap<String, HashMap<String, JValue>>,
    /// Classes whose `<clinit>` has already been run successfully.
    pub(in crate::interpreter) clinit_done: HashSet<String>,
    /// Classes whose initialization is currently owned by a thread (JVMS §5.5 step 6).
    clinit_owners: HashMap<String, ThreadId>,
    /// Classes whose prerequisites are being initialized before their own `<clinit>` is posted.
    pub(in crate::interpreter) clinit_pending: HashSet<String>,
    /// Classes whose own `<clinit>` frame is posted or currently executing.
    pub(in crate::interpreter) clinit_running: HashSet<String>,
    /// Classes whose `<clinit>` threw an exception (erroneous state per JVMS §5.5).
    pub(in crate::interpreter) clinit_failed: HashSet<String>,
    /// `<clinit>` frames currently executing on the synchronous `run_trampoline` path.
    sync_clinit_stack: Vec<String>,
    /// Canonical Class objects keyed by internal class name or descriptor.
    pub(in crate::interpreter) class_pool: HashMap<String, JRef>,
    /// Buffered `System.out.print` content until newline/println.
    pub(in crate::interpreter) stdout_buffer: String,
    /// Buffered `System.err.print` content until newline/println.
    pub(in crate::interpreter) stderr_buffer: String,
    /// Process stdin mode.
    pub(in crate::interpreter) stdin_mode: StdioMode,
    /// Process stdout mode.
    pub(in crate::interpreter) stdout_mode: StdioMode,
    /// Process stderr mode.
    pub(in crate::interpreter) stderr_mode: StdioMode,
    /// Pending bytes for `System.out` when stdout is piped.
    pub(in crate::interpreter) stdout_chunks: VecDeque<Vec<u8>>,
    /// Pending bytes for `System.err` when stderr is piped.
    pub(in crate::interpreter) stderr_chunks: VecDeque<Vec<u8>>,
    /// Buffered bytes supplied to the process stdin pipe.
    pub(in crate::interpreter) stdin_bytes: VecDeque<u8>,
    /// Whether the process stdin pipe has reached EOF.
    pub(in crate::interpreter) stdin_closed: bool,
    /// Cached `System.in` object for host-side wakeups.
    pub(in crate::interpreter) system_stdin: Option<JRef>,
    /// Singleton system ClassLoader instance (created on first access).
    pub(in crate::interpreter) system_classloader: Option<JRef>,
    /// Green thread scheduler.
    pub(in crate::interpreter) scheduler: Scheduler,
    /// Object monitors keyed by object identity (Rc pointer address).
    monitors: HashMap<usize, Monitor>,
    /// Method resolution cache: (class, method_name, descriptor) → owner class name.
    /// Avoids repeated super-chain walks for the same method lookup.
    method_owner_cache: HashMap<(String, String, String), Option<String>>,
    /// Resolved method signature cache: requested call site → (real descriptor, access flags).
    /// Avoids repeated relaxed descriptor matching and flags lookups on hot invoke paths.
    method_signature_cache: HashMap<(String, String, String), (String, u16)>,
    /// Cached decoded method refs keyed by `(cp pointer, cp index)`.
    methodref_constant_cache: HashMap<(usize, u16), Rc<ResolvedMemberRef>>,
    /// Cached resolved static call sites keyed by `(cp pointer, cp index)`.
    static_callsite_cache: HashMap<(usize, u16), Rc<ResolvedStaticCallSite>>,
    /// Monomorphic virtual/interface call-site cache keyed by `(cp pointer, cp index)`.
    /// Each entry remembers the most recently seen dispatch class for that call site.
    virtual_callsite_cache: HashMap<(usize, u16), Rc<ResolvedVirtualCallSite>>,
    /// Static field owner cache: symbolic owner/name/descriptor → declaring class name.
    /// Mirrors HotSpot's resolved field entries well enough for repeated getstatic/putstatic.
    static_field_owner_cache: HashMap<(String, String, String), Option<String>>,
    /// Cached decoded field refs keyed by `(cp pointer, cp index)`.
    fieldref_constant_cache: HashMap<(usize, u16), Rc<ResolvedMemberRef>>,
    /// Cached resolved method execution metadata keyed by requested call-site triplet.
    method_exec_info_cache: HashMap<(String, String, String), MethodExecInfo>,
    /// Cached `<clinit>` presence for parsed classes.
    class_initializer_cache: HashMap<String, bool>,
    /// Cached presence of concrete non-static interface methods.
    concrete_interface_method_cache: HashMap<String, bool>,
    /// Cached subtype checks keyed by `(runtime_class, target_class)`.
    instanceof_cache: HashMap<(String, String), bool>,
    /// Cached ordered superinterfaces that must be initialized before a class.
    class_init_superinterface_cache: HashMap<String, Vec<String>>,
    /// Materialized non-class resources from loaded JARs, keyed by path.
    pub resources: HashMap<String, Vec<u8>>,
    /// Shared byte-array objects for resource-backed input streams.
    resource_array_cache: HashMap<String, JRef>,
    /// Cached declared-field metadata, analogous to OpenJDK's ReflectionData fast path.
    reflection_fields_cache: HashMap<String, Rc<Vec<ReflectFieldInfo>>>,
    /// Cached declared-method metadata, analogous to OpenJDK's ReflectionData fast path.
    reflection_methods_cache: HashMap<String, Rc<Vec<ReflectMethodInfo>>>,
    /// Cached declared-constructor metadata, analogous to OpenJDK's ReflectionData fast path.
    reflection_ctors_cache: HashMap<String, Rc<Vec<ReflectConstructorInfo>>>,
    /// Non-class resources that still point at compressed JAR entries.
    pending_resources: HashMap<String, JarEntryRef>,
    /// Parsed ZIP archives kept alive so lazy entry reads do not re-scan the central directory.
    jar_archives: Vec<OwnedJarArchive>,
    profiler: Option<VmProfiler>,
}

impl Vm {
    /// Create an empty VM with a main thread.
    pub fn new() -> Self {
        Vm {
            classes: HashMap::new(),
            string_pool: HashMap::new(),
            static_fields: HashMap::new(),
            clinit_done: HashSet::new(),
            clinit_owners: HashMap::new(),
            clinit_pending: HashSet::new(),
            clinit_running: HashSet::new(),
            clinit_failed: HashSet::new(),
            sync_clinit_stack: Vec::new(),
            class_pool: HashMap::new(),
            stdout_buffer: String::new(),
            stderr_buffer: String::new(),
            stdin_mode: StdioMode::Pipe,
            stdout_mode: StdioMode::Inherit,
            stderr_mode: StdioMode::Inherit,
            stdout_chunks: VecDeque::new(),
            stderr_chunks: VecDeque::new(),
            stdin_bytes: VecDeque::new(),
            stdin_closed: false,
            system_stdin: None,
            system_classloader: None,
            scheduler: Scheduler::new(),
            monitors: HashMap::new(),
            method_owner_cache: HashMap::new(),
            method_signature_cache: HashMap::new(),
            methodref_constant_cache: HashMap::new(),
            static_callsite_cache: HashMap::new(),
            virtual_callsite_cache: HashMap::new(),
            static_field_owner_cache: HashMap::new(),
            fieldref_constant_cache: HashMap::new(),
            method_exec_info_cache: HashMap::new(),
            class_initializer_cache: HashMap::new(),
            concrete_interface_method_cache: HashMap::new(),
            instanceof_cache: HashMap::new(),
            class_init_superinterface_cache: HashMap::new(),
            resources: HashMap::new(),
            resource_array_cache: HashMap::new(),
            reflection_fields_cache: HashMap::new(),
            reflection_methods_cache: HashMap::new(),
            reflection_ctors_cache: HashMap::new(),
            pending_resources: HashMap::new(),
            jar_archives: Vec::new(),
            profiler: VmProfiler::from_env(),
        }
    }

    pub(in crate::interpreter) fn effective_time_slice(&self) -> usize {
        if self.scheduler.runnable_count() <= 1 {
            SINGLE_RUNNABLE_TIME_SLICE
        } else {
            TIME_SLICE
        }
    }

    fn invalidate_resolution_caches(&mut self) {
        self.method_owner_cache.clear();
        self.method_signature_cache.clear();
        self.methodref_constant_cache.clear();
        self.static_callsite_cache.clear();
        self.virtual_callsite_cache.clear();
        self.static_field_owner_cache.clear();
        self.fieldref_constant_cache.clear();
        self.method_exec_info_cache.clear();
        self.instanceof_cache.clear();
    }

    fn invalidate_class_caches(&mut self, name: &str) {
        self.invalidate_resolution_caches();
        self.class_initializer_cache.remove(name);
        self.concrete_interface_method_cache.remove(name);
        self.class_init_superinterface_cache.remove(name);
        self.reflection_fields_cache.remove(name);
        self.reflection_methods_cache.remove(name);
        self.reflection_ctors_cache.remove(name);
    }

    fn read_jar_entry(&mut self, entry: &JarEntryRef) -> Result<Vec<u8>, String> {
        let archive = self.jar_archives.get_mut(entry.jar_id)
            .ok_or_else(|| format!("Missing JAR backing store for {}", entry.entry_name))?;
        let mut file = archive.by_index(entry.entry_index)
            .map_err(|e| format!("ZIP entry error for {}: {e}", entry.entry_name))?;
        if file.name() != entry.entry_name {
            return Err(format!(
                "ZIP entry mismatch at index {}: expected {}, found {}",
                entry.entry_index,
                entry.entry_name,
                file.name()
            ));
        }
        let mut buf = Vec::with_capacity(file.size() as usize);
        std::io::Read::read_to_end(&mut file, &mut buf)
            .map_err(|e| format!("Read error for {}: {e}", entry.entry_name))?;
        Ok(buf)
    }

    /// Get the object identity key for monitor operations.
    ///
    /// Uses the `Rc` pointer address as a stable identity. This is safe as long
    /// as the `Rc` is alive — which is guaranteed because the caller holds a
    /// reference. In Phase 4+, we may switch to a per-object unique ID to avoid
    /// address reuse after deallocation.
    fn object_id(obj: &JRef) -> usize {
        Rc::as_ptr(obj) as *const () as usize
    }

    /// Acquire the monitor for the given object (monitorenter).
    /// In a single-threaded context, this always succeeds immediately.
    /// With multiple threads, the current thread may block.
    pub(in crate::interpreter) fn monitor_enter(&mut self, obj: &JRef) {
        let id = Self::object_id(obj);
        let thread_id = self.scheduler.current_thread().id;
        let monitor = self.monitors.entry(id).or_insert(Monitor {
            owner: None,
            count: 0,
            entry_queue: VecDeque::new(),
            wait_queue: VecDeque::new(),
        });
        match monitor.owner {
            None => {
                // Unlocked — acquire.
                monitor.owner = Some(thread_id);
                monitor.count = 1;
            }
            Some(owner) if owner == thread_id => {
                // Reentrant — increment count.
                monitor.count += 1;
            }
            Some(_) => {
                // Owned by another thread — block until released.
                monitor.entry_queue.push_back(thread_id);
                // Transition current thread to WaitingOnMonitor so the
                // scheduler yields and switches to another thread.
                self.scheduler.current_thread_mut().state = ThreadState::WaitingOnMonitor(id);
            }
        }
    }

    /// Release the monitor for the given object (monitorexit).
    /// Returns Err if the current thread does not own the monitor.
    pub(in crate::interpreter) fn monitor_exit(&mut self, obj: &JRef) -> Result<(), String> {
        let id = Self::object_id(obj);
        let thread_id = self.scheduler.current_thread().id;
        let mut remove_monitor = false;
        let mut wake_thread: Option<ThreadId> = None;
        {
            let monitor = match self.monitors.get_mut(&id) {
                Some(m) => m,
                None => return Err("java/lang/IllegalMonitorStateException: monitor not entered".to_owned()),
            };
            if monitor.owner != Some(thread_id) {
                return Err("java/lang/IllegalMonitorStateException: current thread is not owner".to_owned());
            }
            monitor.count -= 1;
            if monitor.count == 0 {
                let has_waiters = !monitor.entry_queue.is_empty() || !monitor.wait_queue.is_empty();
                if !has_waiters {
                    monitor.owner = None;
                    // Fully unlocked with no waiters — remove to avoid unbounded growth
                    // and stale monitor state from address reuse.
                    remove_monitor = true;
                } else if let Some(waiting_id) = monitor.entry_queue.pop_front() {
                    // Transfer ownership directly to the next waiter.
                    monitor.owner = Some(waiting_id);
                    // Default count = 1 for normal monitorenter waiters.
                    // Overridden below for wait()-woken threads.
                    monitor.count = 1;
                    wake_thread = Some(waiting_id);
                } else {
                    monitor.owner = None;
                }
            }
        }
        if remove_monitor {
            self.monitors.remove(&id);
        }
        // Set the woken thread to Runnable (outside the monitor borrow).
        if let Some(wid) = wake_thread {
            // Check if the woken thread needs saved_monitor_count restored.
            let restore_count = self.scheduler.thread(wid).and_then(|t| {
                if matches!(t.state, ThreadState::WaitingOnCondition(_)) && t.saved_monitor_count > 0 {
                    Some(t.saved_monitor_count)
                } else {
                    None
                }
            });
            if let Some(count) = restore_count {
                if let Some(m) = self.monitors.get_mut(&id) {
                    m.count = count;
                }
                if let Some(t) = self.scheduler.thread_mut(wid) {
                    t.saved_monitor_count = 0;
                }
            }
            if let Some(t) = self.scheduler.thread_mut(wid) {
                t.state = ThreadState::Runnable;
            }
        }
        Ok(())
    }

    /// Object.wait() — release monitor, add to wait_queue, block.
    ///
    /// JVMS semantics: the current thread must own the monitor. The reentrant
    /// count is saved, the monitor is fully released, and the thread is added
    /// to the wait_queue. After notify, the thread re-enters the entry_queue
    /// and must re-acquire the monitor before resuming.
    pub(in crate::interpreter) fn monitor_wait(&mut self, obj: &JRef) -> Result<(), String> {
        let id = Self::object_id(obj);
        let thread_id = self.scheduler.current_thread().id;

        let saved_count = {
            let monitor = match self.monitors.get_mut(&id) {
                Some(m) => m,
                None => return Err("java/lang/IllegalMonitorStateException: object not locked".to_owned()),
            };
            if monitor.owner != Some(thread_id) {
                return Err("java/lang/IllegalMonitorStateException: current thread is not owner".to_owned());
            }
            // Save reentrant count and fully release.
            let saved = monitor.count;
            monitor.count = 0;
            monitor.owner = None;
            monitor.wait_queue.push_back(thread_id);

            // If there are threads blocked on entry, transfer ownership to the first one.
            // If the waiter was previously wait()-woken, restore its saved reentrant count.
            if let Some(waiting_id) = monitor.entry_queue.pop_front() {
                if let Some(t) = self.scheduler.thread_mut(waiting_id) {
                    if let ThreadState::WaitingOnCondition(wait_obj_id) = t.state {
                        if wait_obj_id == id && t.saved_monitor_count > 0 {
                            monitor.count = t.saved_monitor_count;
                            t.saved_monitor_count = 0;
                        } else {
                            monitor.count = 1;
                        }
                    } else {
                        monitor.count = 1;
                    }
                    monitor.owner = Some(waiting_id);
                    t.state = ThreadState::Runnable;
                }
            }

            saved
        };

        // Save the reentrant count and block the current thread.
        let current = self.scheduler.current_thread_mut();
        current.saved_monitor_count = saved_count;
        current.state = ThreadState::WaitingOnCondition(id);
        Ok(())
    }

    /// Object.notify() — move one thread from wait_queue to entry_queue.
    pub(in crate::interpreter) fn monitor_notify(&mut self, obj: &JRef) -> Result<(), String> {
        let id = Self::object_id(obj);
        let thread_id = self.scheduler.current_thread().id;

        let monitor = match self.monitors.get_mut(&id) {
            Some(m) => m,
            None => return Err("java/lang/IllegalMonitorStateException: object not locked".to_owned()),
        };
        if monitor.owner != Some(thread_id) {
            return Err("java/lang/IllegalMonitorStateException: current thread is not owner".to_owned());
        }
        // Move one waiter from wait_queue to entry_queue.
        if let Some(waiter_id) = monitor.wait_queue.pop_front() {
            monitor.entry_queue.push_back(waiter_id);
        }
        Ok(())
    }

    /// Object.notifyAll() — move all threads from wait_queue to entry_queue.
    pub(in crate::interpreter) fn monitor_notify_all(&mut self, obj: &JRef) -> Result<(), String> {
        let id = Self::object_id(obj);
        let thread_id = self.scheduler.current_thread().id;

        let monitor = match self.monitors.get_mut(&id) {
            Some(m) => m,
            None => return Err("java/lang/IllegalMonitorStateException: object not locked".to_owned()),
        };
        if monitor.owner != Some(thread_id) {
            return Err("java/lang/IllegalMonitorStateException: current thread is not owner".to_owned());
        }
        // Move all waiters to entry_queue.
        while let Some(waiter_id) = monitor.wait_queue.pop_front() {
            monitor.entry_queue.push_back(waiter_id);
        }
        Ok(())
    }

    /// Host-side notifyAll for process pipes.
    ///
    /// Unlike `Object.notifyAll()`, this has no Java monitor ownership check.
    /// It exists so the host can wake `System.in` readers after appending bytes
    /// or closing stdin.
    pub(in crate::interpreter) fn host_notify_all(&mut self, obj: &JRef) {
        let id = Self::object_id(obj);
        let mut wake_thread: Option<ThreadId> = None;
        if let Some(monitor) = self.monitors.get_mut(&id) {
            while let Some(waiter_id) = monitor.wait_queue.pop_front() {
                monitor.entry_queue.push_back(waiter_id);
            }
            if monitor.owner.is_none() {
                if let Some(waiting_id) = monitor.entry_queue.pop_front() {
                    monitor.owner = Some(waiting_id);
                    let restore_count = self.scheduler.thread(waiting_id).and_then(|t| match t.state {
                        ThreadState::WaitingOnCondition(wait_obj_id)
                            if wait_obj_id == id && t.saved_monitor_count > 0 =>
                        {
                            Some(t.saved_monitor_count)
                        }
                        _ => None,
                    });
                    monitor.count = restore_count.unwrap_or(1);
                    wake_thread = Some(waiting_id);
                }
            }
        }
        if let Some(wid) = wake_thread {
            if let Some(t) = self.scheduler.thread_mut(wid) {
                t.saved_monitor_count = 0;
                t.state = ThreadState::Runnable;
            }
        }
    }

    /// Initialize the inherited Throwable state for VM-created exceptions.
    ///
    /// These exceptions are allocated directly by the VM without running Java
    /// constructors, so we must populate the fields that Throwable methods
    /// assume are always initialized.
    pub(in crate::interpreter) fn init_vm_throwable(&mut self, exc: &JRef, detail_message: Option<JRef>) {
        let stack_trace = JObject::new_array("[Ljava/lang/StackTraceElement;", vec![]);
        let mut obj = exc.borrow_mut();
        obj.fields.insert("detailMessage".to_owned(), JValue::Ref(detail_message));
        obj.fields.insert("cause".to_owned(), JValue::Ref(Some(Rc::clone(exc))));
        obj.fields.insert("stackTrace".to_owned(), JValue::Ref(Some(stack_trace)));
        obj.fields.insert("suppressedExceptions".to_owned(), JValue::Ref(None));
    }

    pub(in crate::interpreter) fn new_vm_exception(&mut self, class_name: &str, detail_message: Option<JRef>) -> JRef {
        let exc = JObject::new(class_name);
        self.init_vm_throwable(&exc, detail_message);
        exc
    }

    pub(in crate::interpreter) fn new_vm_exception_message(
        &mut self,
        class_name: &str,
        detail_message: impl Into<String>,
    ) -> JRef {
        let msg = self.intern_string(detail_message);
        self.new_vm_exception(class_name, Some(msg))
    }

    /// Set a pending IllegalMonitorStateException from an error message.
    pub(in crate::interpreter) fn throw_illegal_monitor_state(&mut self, err_msg: &str) {
        let exc = self.new_vm_exception_message("java/lang/IllegalMonitorStateException", err_msg);
        *self.pending_exception_mut() = Some(exc);
    }

    /// Mutable access to the current thread's pending exception.
    #[inline]
    pub(in crate::interpreter) fn pending_exception_mut(&mut self) -> &mut Option<JRef> {
        &mut self.scheduler.current_thread_mut().pending_exception
    }

    /// Mutable access to the current thread's pending frame.
    #[inline]
    pub(crate) fn pending_frame_mut(&mut self) -> &mut Option<trampoline::FrameInfo> {
        &mut self.scheduler.current_thread_mut().pending_frame
    }

    /// Spawn a new green thread that will execute the `run()` method of the
    /// given java.lang.Thread object. Returns the new thread's ID.
    pub(in crate::interpreter) fn thread_start(&mut self, thread_obj: JRef) -> Result<ThreadId, String> {
        // Reject double-start: check if a ThreadContext already exists for this object.
        if self.find_thread_id_by_object(&thread_obj).is_some() {
            return Err("java/lang/IllegalThreadStateException: thread already started".to_owned());
        }

        let id = self.scheduler.spawn(Some(Rc::clone(&thread_obj)));

        // Build a frame for `run()V` on the Thread object.
        let class_name = thread_obj.borrow().class_name.clone();
        let fi = self.build_virtual_frame_inner(
            thread_obj, &class_name, "run", "()V", vec![], false,
        )?;
        match fi {
            Some(frame_info) => {
                self.scheduler.thread_mut(id).unwrap().call_stack.push(frame_info);
            }
            None => {
                // run() is not found in bytecode — this shouldn't happen for Thread
                // subclasses, but handle gracefully by marking terminated.
                self.scheduler.thread_mut(id).unwrap().state = ThreadState::Terminated;
            }
        }
        Ok(id)
    }

    /// Block the current thread until the target thread terminates.
    pub(in crate::interpreter) fn thread_join(&mut self, target_id: ThreadId) {
        // If target is already terminated, no-op.
        if let Some(target) = self.scheduler.thread(target_id) {
            if target.state == ThreadState::Terminated {
                return;
            }
        }
        let current = self.scheduler.current_thread_mut();
        current.state = ThreadState::Joining(target_id);
    }

    /// Get the java.lang.Thread object for the current thread.
    /// Creates one lazily for the main thread if it doesn't exist.
    pub(in crate::interpreter) fn current_thread_object(&mut self) -> JRef {
        if let Some(ref obj) = self.scheduler.current_thread().thread_object {
            return Rc::clone(obj);
        }
        // Main thread — create a Thread object lazily.
        // Use tid=0 to avoid collision with Thread.nextId which starts at 1.
        let obj = JObject::new("java/lang/Thread");
        {
            let mut b = obj.borrow_mut();
            b.fields.insert("tid".to_owned(), JValue::Int(0));
            b.fields.insert("name".to_owned(), JValue::Ref(Some(self.intern_string("main"))));
            b.fields.insert("priority".to_owned(), JValue::Int(5));
            b.fields.insert("daemon".to_owned(), JValue::Int(0));
        }
        self.scheduler.current_thread_mut().thread_object = Some(Rc::clone(&obj));
        obj
    }

    /// Find the thread ID associated with a java.lang.Thread object.
    pub(in crate::interpreter) fn find_thread_id_by_object(&self, thread_obj: &JRef) -> Option<ThreadId> {
        self.scheduler.find_thread_id_by_object(thread_obj)
    }

    /// Check if a thread (identified by its java.lang.Thread object) is alive.
    pub(in crate::interpreter) fn thread_is_alive(&self, thread_obj: &JRef) -> bool {
        if let Some(id) = self.find_thread_id_by_object(thread_obj) {
            self.scheduler.thread(id)
                .map(|t| t.state != ThreadState::Terminated)
                .unwrap_or(false)
        } else {
            false
        }
    }

    /// Register a pre-parsed class file (always stored as `Ready`).
    pub fn load_class(&mut self, class_file: ClassFile) {
        let name = class_file.constant_pool.class_name(class_file.this_class).to_owned();
        self.invalidate_class_caches(&name);
        self.classes.insert(name, LazyClass::Ready(class_file));
    }

    /// Register raw `.class` bytes for lazy parsing.
    /// The class is parsed only when first accessed via [`Self::ensure_class_ready`].
    /// If the class is already registered (e.g., as `Ready`), the existing entry is kept.
    pub fn load_lazy(&mut self, name: String, bytes: Vec<u8>) {
        if !self.classes.contains_key(&name) {
            self.invalidate_class_caches(&name);
            self.classes.insert(name, LazyClass::PendingBytes(bytes));
        }
    }

    fn load_lazy_jar_entry(&mut self, name: String, entry: JarEntryRef) {
        if !self.classes.contains_key(&name) {
            self.invalidate_class_caches(&name);
            self.classes.insert(name, LazyClass::PendingJarEntry(entry));
        }
    }

    /// Load classes and resources from a JAR (ZIP) byte array.
    /// `.class` entries are registered lazily from their ZIP entry metadata; all other
    /// non-directory entries are recorded and decompressed only on first access.
    ///
    /// The ZIP entry path is treated as the canonical class name (`pkg/Foo.class` -> `pkg/Foo`).
    /// We intentionally do not scan class bodies to recover mismatched internal names:
    /// such JARs are nonstandard, and keeping that fallback would add decompression and
    /// parsing work to ordinary lazy-loading paths.
    /// Returns the number of classes loaded.
    pub fn load_jar(&mut self, jar_bytes: &[u8]) -> Result<usize, String> {
        use std::io::Cursor;
        let reader = Cursor::new(jar_bytes.to_vec());
        let mut archive = zip::ZipArchive::new(reader)
            .map_err(|e| format!("Invalid JAR/ZIP: {e}"))?;
        let jar_id = self.jar_archives.len();
        let mut class_entries = Vec::new();
        let mut resource_entries = Vec::new();
        let mut count = 0;
        for i in 0..archive.len() {
            let file = archive.by_index(i)
                .map_err(|e| format!("ZIP entry error: {e}"))?;
            let name = file.name().to_owned();
            let entry = JarEntryRef { jar_id, entry_index: i, entry_name: name.clone() };
            if let Some(class_name) = name.strip_suffix(".class") {
                if !class_name.is_empty() {
                    class_entries.push((class_name.to_owned(), entry));
                    count += 1;
                }
            } else if !name.ends_with('/') {
                resource_entries.push((name, entry));
            }
        }
        self.jar_archives.push(archive);
        for (class_name, entry) in class_entries {
            self.load_lazy_jar_entry(class_name, entry);
        }
        for (name, entry) in resource_entries {
            self.resources.remove(&name);
            self.resource_array_cache.remove(&name);
            self.pending_resources.insert(name, entry);
        }
        Ok(count)
    }

    /// Ensure the named class is fully parsed (`Ready`).
    /// If the entry is pending, parses it in place and promotes it to `Ready`.
    /// On parse failure the entry is set to `ParseError` so the failure is
    /// diagnosable and repeated parse attempts are avoided.
    /// Does nothing if the class is already `Ready`, `ParseError`, or not registered.
    pub(in crate::interpreter) fn ensure_class_ready(&mut self, name: &str) {
        if !matches!(
            self.classes.get(name),
            Some(LazyClass::PendingBytes(_) | LazyClass::PendingJarEntry(_))
        ) {
            return;
        }
        let pending = self.classes.remove(name);
        let result = match pending {
            Some(LazyClass::PendingBytes(bytes)) => class_file::parse(&bytes).map_err(|e| e.to_string()),
            Some(LazyClass::PendingJarEntry(entry)) => match self.read_jar_entry(&entry) {
                Ok(bytes) => match class_file::parse(&bytes) {
                    Ok(cf) => {
                        let actual_name = cf.constant_pool.class_name(cf.this_class);
                        if actual_name == name {
                            Ok(cf)
                        } else {
                            // We deliberately reject classes whose internal name disagrees with
                            // the ZIP entry path. Recovering them would require a fallback scan
                            // that penalizes the normal lazy-loading hot path for a nonstandard JAR.
                            Err(format!(
                                "Class name mismatch for {}: expected {}, found {}",
                                entry.entry_name, name, actual_name
                            ))
                        }
                    }
                    Err(e) => Err(e.to_string()),
                },
                Err(e) => Err(e),
            },
            Some(other) => {
                self.classes.insert(name.to_owned(), other);
                return;
            }
            None => return,
        };
        match result {
            Ok(cf) => { self.classes.insert(name.to_owned(), LazyClass::Ready(cf)); }
            Err(e) => {
                eprintln!("Warning: failed to parse class '{name}': {e}");
                self.classes.insert(name.to_owned(), LazyClass::ParseError(e));
            }
        }
    }

    pub fn has_resource(&self, name: &str) -> bool {
        let normalized = name.strip_prefix('/').unwrap_or(name);
        self.resources.contains_key(normalized) || self.pending_resources.contains_key(normalized)
    }

    pub fn read_resource(&mut self, name: &str) -> Result<Option<Vec<u8>>, String> {
        let normalized = name.strip_prefix('/').unwrap_or(name);
        if let Some(data) = self.resources.get(normalized) {
            return Ok(Some(data.clone()));
        }
        let Some(entry) = self.pending_resources.get(normalized).cloned() else {
            return Ok(None);
        };
        let data = self.read_jar_entry(&entry)?;
        self.pending_resources.remove(normalized);
        self.resources.insert(normalized.to_owned(), data.clone());
        Ok(Some(data))
    }

    pub(in crate::interpreter) fn resource_byte_array(
        &mut self,
        name: &str,
    ) -> Result<Option<JRef>, String> {
        let normalized = name.strip_prefix('/').unwrap_or(name);
        if let Some(array) = self.resource_array_cache.get(normalized) {
            return Ok(Some(array.clone()));
        }
        let Some(data) = self.read_resource(normalized)? else {
            return Ok(None);
        };
        let array = JObject::new_byte_array(data);
        self.resource_array_cache
            .insert(normalized.to_owned(), array.clone());
        Ok(Some(array))
    }

    /// Return a reference to a parsed class.
    /// Caller must have called `ensure_class_ready` first (or know the class is already Ready).
    pub(in crate::interpreter) fn get_class(&self, name: &str) -> Option<&ClassFile> {
        match self.classes.get(name)? {
            LazyClass::Ready(cf) => Some(cf),
            LazyClass::PendingBytes(_) | LazyClass::PendingJarEntry(_) | LazyClass::ParseError(_) => None,
        }
    }

    /// Ensure class is ready and return a reference to it.
    pub(in crate::interpreter) fn resolve_class(&mut self, name: &str) -> Option<&ClassFile> {
        self.ensure_class_ready(name);
        self.get_class(name)
    }

    /// Flush buffered PrintStream output (`print` without trailing `println`).
    pub fn flush_printstreams(&mut self) {
        if self.stdout_mode == StdioMode::Inherit && !self.stdout_buffer.is_empty() {
            Self::emit_host_line(false, &self.stdout_buffer);
            self.stdout_buffer.clear();
        }
        if self.stderr_mode == StdioMode::Inherit && !self.stderr_buffer.is_empty() {
            Self::emit_host_line(true, &self.stderr_buffer);
            self.stderr_buffer.clear();
        }
    }

    pub(in crate::interpreter) fn record_profile_sample(
        &mut self,
        opcode: u8,
        frame_owner: &Rc<str>,
        elapsed: std::time::Duration,
    ) {
        if let Some(profiler) = self.profiler.as_mut() {
            profiler.record(opcode, frame_owner, elapsed);
        }
    }

    pub fn profile_report(&self) -> Option<String> {
        let profiler = self.profiler.as_ref()?;
        let report = profiler.report();
        if report.is_empty() {
            None
        } else {
            Some(report)
        }
    }

    pub fn take_profile_report(&mut self) -> Option<String> {
        let profiler = self.profiler.take()?;
        let report = profiler.report();
        if report.is_empty() {
            None
        } else {
            Some(report)
        }
    }

    pub fn write_profile_report_if_enabled(&mut self) {
        let Some(report) = self.profile_report() else {
            return;
        };
        self.write_printstream_bytes(true, report.as_bytes());
    }

    pub(in crate::interpreter) fn record_for_name_sample(
        &mut self,
        runtime_name: &str,
        elapsed: std::time::Duration,
    ) {
        if let Some(profiler) = self.profiler.as_mut() {
            profiler.record_for_name(runtime_name, elapsed);
        }
    }

    pub fn set_stdio_modes(&mut self, stdin: StdioMode, stdout: StdioMode, stderr: StdioMode) {
        self.stdin_mode = stdin;
        self.stdout_mode = stdout;
        self.stderr_mode = stderr;
        self.stdin_closed = matches!(stdin, StdioMode::Ignore);
        self.stdin_bytes.clear();
        self.stdout_chunks.clear();
        self.stderr_chunks.clear();
        self.stdout_buffer.clear();
        self.stderr_buffer.clear();
    }

    pub fn write_stdin(&mut self, bytes: &[u8]) {
        if matches!(self.stdin_mode, StdioMode::Ignore) || self.stdin_closed {
            return;
        }
        self.stdin_bytes.extend(bytes.iter().copied());
        if let Some(stdin) = self.system_stdin.clone() {
            self.host_notify_all(&stdin);
        }
    }

    pub fn close_stdin(&mut self) {
        self.stdin_closed = true;
        if let Some(stdin) = self.system_stdin.clone() {
            self.host_notify_all(&stdin);
        }
    }

    pub fn take_stdout(&mut self) -> Vec<u8> {
        let total: usize = self.stdout_chunks.iter().map(|chunk| chunk.len()).sum();
        let mut out = Vec::with_capacity(total);
        while let Some(chunk) = self.stdout_chunks.pop_front() {
            out.extend_from_slice(&chunk);
        }
        out
    }

    pub fn take_stderr(&mut self) -> Vec<u8> {
        let total: usize = self.stderr_chunks.iter().map(|chunk| chunk.len()).sum();
        let mut out = Vec::with_capacity(total);
        while let Some(chunk) = self.stderr_chunks.pop_front() {
            out.extend_from_slice(&chunk);
        }
        out
    }

    pub(in crate::interpreter) fn stdin_read_byte(&mut self) -> i32 {
        if let Some(byte) = self.stdin_bytes.pop_front() {
            return i32::from(byte);
        }
        if self.stdin_closed {
            -1
        } else {
            -2
        }
    }

    pub(in crate::interpreter) fn stdin_available(&self) -> i32 {
        self.stdin_bytes.len().min(i32::MAX as usize) as i32
    }

    pub(in crate::interpreter) fn is_waiting_on_stdin(&self) -> bool {
        let Some(stdin) = &self.system_stdin else {
            return false;
        };
        let stdin_id = Self::object_id(stdin);
        self.scheduler
            .threads
            .iter()
            .any(|t| matches!(t.state, ThreadState::WaitingOnCondition(id) if id == stdin_id))
    }

    /// Intern a Java string (returns same `JRef` for equal content).
    pub fn intern_string(&mut self, s: impl Into<String>) -> JRef {
        use std::collections::hash_map::Entry;
        let s = s.into();
        match self.string_pool.entry(s) {
            Entry::Occupied(e) => Rc::clone(e.get()),
            Entry::Vacant(e) => {
                let jobj = JObject::new_string(e.key().clone());
                Rc::clone(e.insert(jobj))
            }
        }
    }

    pub(in crate::interpreter) fn format_exception_ref(&self, r: &JRef) -> String {
        fn format_exception_chain(r: &JRef, seen: &mut Vec<usize>, depth: usize) -> String {
            let ptr = Rc::as_ptr(r) as usize;
            if seen.contains(&ptr) {
                return "<cycle>".to_owned();
            }
            seen.push(ptr);

            let (mut s, next) = {
                let b = r.borrow();
                let mut s = format!("Exception: {}", b.class_name);
                if let Some(JValue::Ref(Some(msg_ref))) = b.fields.get("detailMessage") {
                    if let Some(msg) = msg_ref.borrow().as_java_string() {
                        if !msg.is_empty() {
                            s.push_str(": ");
                            s.push_str(msg);
                        }
                    }
                }
                let next = if depth < 8 {
                    b.fields.get("cause")
                        .and_then(|v| v.as_ref())
                        .filter(|cause| !Rc::ptr_eq(cause, r))
                        .cloned()
                        .or_else(|| {
                            b.fields.get("target")
                                .and_then(|v| v.as_ref())
                                .filter(|target| !Rc::ptr_eq(target, r))
                                .cloned()
                        })
                } else {
                    None
                };
                (s, next)
            };

            if let Some(next_ref) = next {
                s.push_str(" | cause: ");
                s.push_str(&format_exception_chain(&next_ref, seen, depth + 1));
            }
            s
        }

        format_exception_chain(r, &mut Vec::new(), 0)
    }

    fn pending_exception_err(&self) -> Option<String> {
        self.scheduler.current_thread().pending_exception.as_ref().map(|r| self.format_exception_ref(r))
    }

    /// Return (or lazily create) the singleton system ClassLoader instance.
    pub(in crate::interpreter) fn get_or_create_system_classloader(&mut self) -> JRef {
        if let Some(ref cl) = self.system_classloader {
            return Rc::clone(cl);
        }
        let cl = JObject::new("java/lang/ClassLoader");
        self.system_classloader = Some(Rc::clone(&cl));
        cl
    }

    /// Set `pending_exception` to a `NoClassDefFoundError` for `name`.
    /// `name` should be the internal (slash-separated) class name.
    pub(in crate::interpreter) fn throw_no_class_def_found(&mut self, name: &str) {
        let exc = self.new_vm_exception_message("java/lang/NoClassDefFoundError", name.replace('/', "."));
        *self.pending_exception_mut() = Some(exc);
    }

    /// Set `pending_exception` to a new `ClassNotFoundException` for `name`.
    /// `name` should be the runtime (dot-separated) class name.
    pub(in crate::interpreter) fn throw_class_not_found(&mut self, name: &str) {
        let exc = self.new_vm_exception_message("java/lang/ClassNotFoundException", name);
        *self.pending_exception_mut() = Some(exc);
    }

    /// Set `pending_exception` to a `NullPointerException` with an optional detail message.
    pub(in crate::interpreter) fn throw_null_pointer(&mut self, detail: &str) {
        let exc = self.new_vm_exception_message("java/lang/NullPointerException", detail);
        *self.pending_exception_mut() = Some(exc);
    }

    /// Set `pending_exception` to a `BootstrapMethodError` with a detail message.
    pub(in crate::interpreter) fn throw_bootstrap_method_error(&mut self, detail: &str) {
        let exc = self.new_vm_exception_message("java/lang/BootstrapMethodError", detail);
        *self.pending_exception_mut() = Some(exc);
    }

    /// Set `pending_exception` to a `RuntimeException` with a detail message.
    pub(in crate::interpreter) fn throw_runtime_exception(&mut self, detail: &str) {
        let exc = JObject::new("java/lang/RuntimeException");
        let msg = self.intern_string(detail.to_owned());
        exc.borrow_mut().fields.insert("detailMessage".to_owned(), JValue::Ref(Some(msg)));
        *self.pending_exception_mut() = Some(exc);
    }

    /// Set `pending_exception` to a `ClassFormatError` carrying the parse error message.
    /// Used when a class entry exists as `LazyClass::ParseError` (malformed bytecode).
    pub(in crate::interpreter) fn throw_class_format_error(&mut self, parse_msg: &str) {
        let exc = self.new_vm_exception_message("java/lang/ClassFormatError", parse_msg);
        *self.pending_exception_mut() = Some(exc);
    }

    fn class_object(&mut self, internal_name: impl Into<String>) -> JRef {
        let internal_name = internal_name.into();
        if let Some(r) = self.class_pool.get(&internal_name) {
            return Rc::clone(r);
        }
        let obj = JObject::new("java/lang/Class");
        obj.borrow_mut().fields.insert(
            "__name_internal".to_owned(),
            JValue::Ref(Some(self.intern_string(internal_name.clone()))),
        );
        self.class_pool.insert(internal_name, Rc::clone(&obj));
        obj
    }

    /// Look up a loaded class by internal name (triggers lazy parse if needed).
    pub fn class(&mut self, name: &str) -> Option<&ClassFile> {
        self.resolve_class(name)
    }

    /// Find the `access_flags` of a method by name and descriptor in a class
    /// (including super-chain). Returns `None` if the method is not found.
    ///
    /// This is the lightweight variant used by invoke paths to decide dispatch
    /// strategy before calling `resolve_method_exec_info`.
    pub fn find_method_flags(
        &mut self,
        class_name: &str,
        method_name: &str,
        descriptor: &str,
    ) -> Option<u16> {
        self.ensure_class_ready(class_name);
        let class = self.get_class(class_name)?;
        for m in &class.methods {
            let n = class.constant_pool.utf8(m.name_index);
            let d = class.constant_pool.utf8(m.descriptor_index);
            if n == method_name && d == descriptor {
                return Some(m.access_flags);
            }
        }
        // Resolve super/interface names while we still hold the borrow, then release it.
        // String allocation happens only here (not on the fast path where method is found above).
        let super_name: Option<String> = if class.super_class != 0 {
            Some(class.constant_pool.class_name(class.super_class).to_owned())
        } else {
            None
        };
        let iface_names: Vec<String> = class.interfaces.iter()
            .map(|&idx| class.constant_pool.class_name(idx).to_owned())
            .collect();
        // borrow on `class` ends here
        if let Some(super_name) = super_name {
            if let Some(f) = self.find_method_flags(&super_name, method_name, descriptor) {
                return Some(f);
            }
        }
        for iface_name in &iface_names {
            if let Some(f) = self.find_method_flags(iface_name, method_name, descriptor) {
                return Some(f);
            }
        }
        None
    }

    /// Resolve a method and extract all execution-time data in a single pass.
    ///
    /// This avoids repeated method-lookup calls and eliminates the full clone of
    /// the constant pool that was previously needed to release the borrow on `self`.
    pub(super) fn resolve_method_exec_info(
        &mut self,
        class_name: &str,
        method_name: &str,
        descriptor: &str,
    ) -> Option<MethodExecInfo> {
        let cache_key = (
            class_name.to_owned(),
            method_name.to_owned(),
            descriptor.to_owned(),
        );
        if let Some(cached) = self.method_exec_info_cache.get(&cache_key) {
            return Some(cached.clone());
        }
        // Find the class that owns the method (following super/interface chain).
        let owner = self.find_method_owner(class_name, method_name, descriptor)?;
        self.ensure_class_ready(&owner);
        let class = self.get_class(&owner)?;
        // Find the method within the owning class.
        let method_idx = class.methods.iter().position(|m| {
            class.constant_pool.utf8(m.name_index) == method_name
                && class.constant_pool.utf8(m.descriptor_index) == descriptor
        })?;
        let class_name_out = class.constant_pool.class_name(class.this_class).to_owned();
        let descriptor_out = class.constant_pool.utf8(class.methods[method_idx].descriptor_index).to_owned();
        let access_flags = class.methods[method_idx].access_flags;
        let (max_locals, has_code, code, exception_table) =
            if let Some(ca) = class.methods[method_idx].code() {
                (
                    ca.max_locals as usize,
                    true,
                    Rc::new(ca.code.clone()),
                    Rc::new(ca.exception_table.clone()),
                )
            } else {
                (0, false, Rc::new(Vec::new()), Rc::new(Vec::new()))
            };
        let cp = Rc::clone(&class.constant_pool.entries);
        let bootstrap_methods = class.attributes.iter().find_map(|a| {
            if let Attribute::BootstrapMethods(bms) = a {
                Some(Rc::new(bms.clone()))
            } else {
                None
            }
        }).unwrap_or_else(|| Rc::new(Vec::new()));
        let (param_tokens, _) = Self::parse_method_descriptor_tokens(&descriptor_out);
        let param_slot_count = param_tokens
            .iter()
            .map(|t| if t == "J" || t == "D" { 2 } else { 1 })
            .sum();
        let info = MethodExecInfo {
            class_name: class_name_out,
            descriptor: descriptor_out.clone(),
            access_flags,
            max_locals,
            has_code,
            code,
            exception_table,
            cp,
            bootstrap_methods,
            param_tokens: Rc::new(param_tokens),
            param_slot_count,
            frame_owner: Rc::<str>::from(format!("{owner}.{method_name}{descriptor_out}")),
        };
        self.method_exec_info_cache.insert(cache_key, info.clone());
        Some(info)
    }

    /// Find the name of the class that owns a given method (super-chain walk).
    /// Returns the canonical class name, or `None` if not found.
    fn find_method_owner(
        &mut self,
        class_name: &str,
        method_name: &str,
        descriptor: &str,
    ) -> Option<String> {
        let cache_key = (class_name.to_owned(), method_name.to_owned(), descriptor.to_owned());
        if let Some(cached) = self.method_owner_cache.get(&cache_key) {
            return cached.clone();
        }
        let result = self.find_method_owner_uncached(class_name, method_name, descriptor);
        // Only cache positive results — negative lookups (None) may become valid
        // after new classes are registered via load_lazy/load_class.
        if result.is_some() {
            self.method_owner_cache.insert(cache_key, result.clone());
        }
        result
    }

    fn find_method_owner_uncached(
        &mut self,
        class_name: &str,
        method_name: &str,
        descriptor: &str,
    ) -> Option<String> {
        self.ensure_class_ready(class_name);
        let class = self.get_class(class_name)?;
        for m in &class.methods {
            let n = class.constant_pool.utf8(m.name_index);
            let d = class.constant_pool.utf8(m.descriptor_index);
            if n == method_name && d == descriptor {
                return Some(class.constant_pool.class_name(class.this_class).to_owned());
            }
        }
        // Resolve names while holding the borrow; allocation is skipped on the fast path.
        let super_name: Option<String> = if class.super_class != 0 {
            Some(class.constant_pool.class_name(class.super_class).to_owned())
        } else {
            None
        };
        let iface_names: Vec<String> = class.interfaces.iter()
            .map(|&idx| class.constant_pool.class_name(idx).to_owned())
            .collect();
        // borrow on `class` ends here
        if let Some(super_name) = super_name {
            if let Some(owner) = self.find_method_owner(&super_name, method_name, descriptor) {
                return Some(owner);
            }
        }
        for iface_name in &iface_names {
            if let Some(owner) = self.find_method_owner(iface_name, method_name, descriptor) {
                return Some(owner);
            }
        }
        None
    }

    /// Returns `true` if the named method exists in the class hierarchy.
    /// Used to check method existence before dispatch without borrowing ClassFile data.
    pub(in crate::interpreter) fn method_exists(
        &mut self,
        class_name: &str,
        method_name: &str,
        descriptor: &str,
    ) -> bool {
        self.find_method_owner(class_name, method_name, descriptor).is_some()
    }

    /// Resolve the actual descriptor and access flags for a call site.
    /// This folds exact-match lookup, relaxed descriptor matching, and flags
    /// resolution into one cacheable result.
    pub(in crate::interpreter) fn resolve_method_signature(
        &mut self,
        class_name: &str,
        method_name: &str,
        descriptor: &str,
    ) -> Option<(String, u16)> {
        let cache_key = (
            class_name.to_owned(),
            method_name.to_owned(),
            descriptor.to_owned(),
        );
        if let Some(cached) = self.method_signature_cache.get(&cache_key) {
            return Some(cached.clone());
        }

        let mut resolved_descriptor = descriptor.to_owned();
        let mut flags = self.find_method_flags(class_name, method_name, descriptor);
        if flags.is_none() {
            resolved_descriptor =
                self.find_method_real_descriptor(class_name, method_name, descriptor)?;
            flags = self.find_method_flags(class_name, method_name, &resolved_descriptor);
        }
        let flags = flags?;
        let result = (resolved_descriptor, flags);
        self.method_signature_cache.insert(cache_key, result.clone());
        Some(result)
    }

    /// Like find_method but with relaxed matching when the compiler emits generic types.
    /// Match priority:
    ///   1. Exact param types match (ignoring return type)
    ///   2. Same argument count match (ignoring both param types and return type)
    ///   3. Varargs method (ACC_VARARGS) whose non-varargs param count <= call arg count
    /// Returns the real descriptor string of the matched method.
    pub(in crate::interpreter) fn find_method_real_descriptor(
        &mut self,
        class_name: &str,
        method_name: &str,
        descriptor: &str,
    ) -> Option<String> {
        self.ensure_class_ready(class_name);
        let param_part = descriptor.split(')').next().unwrap_or("(");
        let arg_count = count_args(descriptor);
        let class = self.get_class(class_name)?;
        let mut arg_count_match: Option<String> = None;
        let mut varargs_match: Option<String> = None;
        for m in &class.methods {
            let n = class.constant_pool.utf8(m.name_index);
            let d = class.constant_pool.utf8(m.descriptor_index);
            if n != method_name { continue; }
            let d_param = d.split(')').next().unwrap_or("(");
            if d_param == param_part {
                return Some(d.to_owned());
            }
            if arg_count_match.is_none() && count_args(d) == arg_count {
                arg_count_match = Some(d.to_owned());
            }
            if varargs_match.is_none() && (m.access_flags & 0x0080 != 0) {
                let method_param_count = count_args(d);
                let fixed = method_param_count.saturating_sub(1);
                if arg_count >= fixed {
                    varargs_match = Some(d.to_owned());
                }
            }
        }
        if arg_count_match.is_some() { return arg_count_match; }
        if varargs_match.is_some() { return varargs_match; }
        let super_name = self.get_class(class_name)
            .filter(|c| c.super_class != 0)
            .map(|c| c.constant_pool.class_name(c.super_class).to_owned());
        let iface_names: Vec<String> = self.get_class(class_name)
            .map(|c| c.interfaces.iter().map(|&idx| c.constant_pool.class_name(idx).to_owned()).collect())
            .unwrap_or_default();
        if let Some(super_name) = super_name {
            if let Some(result) = self.find_method_real_descriptor(&super_name, method_name, descriptor) {
                return Some(result);
            }
        }
        for iface_name in iface_names {
            if let Some(result) = self.find_method_real_descriptor(&iface_name, method_name, descriptor) {
                return Some(result);
            }
        }
        None
    }

    // ------------------------------------------------------------------

    fn has_class_initializer(&mut self, class_name: &str) -> bool {
        if let Some(cached) = self.class_initializer_cache.get(class_name) {
            return *cached;
        }
        self.ensure_class_ready(class_name);
        let has_clinit = self
            .get_class(class_name)
            .map(|cf| {
                cf.methods.iter().any(|m| {
                    cf.constant_pool.utf8(m.name_index) == "<clinit>"
                        && cf.constant_pool.utf8(m.descriptor_index) == "()V"
                })
            })
            .unwrap_or(false);
        self.class_initializer_cache
            .insert(class_name.to_owned(), has_clinit);
        has_clinit
    }

    fn declares_concrete_interface_method(&mut self, class_name: &str) -> bool {
        if let Some(cached) = self.concrete_interface_method_cache.get(class_name) {
            return *cached;
        }
        self.ensure_class_ready(class_name);
        let has_concrete_method = self
            .get_class(class_name)
            .map(|cf| {
                cf.methods.iter().any(|m| {
                    let name = cf.constant_pool.utf8(m.name_index);
                    name != "<clinit>"
                        && name != "<init>"
                        && (m.access_flags & 0x0400) == 0
                        && (m.access_flags & 0x0008) == 0
                })
            })
            .unwrap_or(false);
        self.concrete_interface_method_cache
            .insert(class_name.to_owned(), has_concrete_method);
        has_concrete_method
    }

    fn collect_class_init_superinterfaces(
        &mut self,
        interface_name: &str,
        seen: &mut HashSet<String>,
        ordered: &mut Vec<String>,
    ) {
        if !seen.insert(interface_name.to_owned()) {
            return;
        }
        self.ensure_class_ready(interface_name);
        let Some(class) = self.get_class(interface_name) else {
            return;
        };
        if (class.access_flags & 0x0200) == 0 {
            return;
        }
        let super_ifaces: Vec<String> = class
            .interfaces
            .iter()
            .map(|&idx| class.constant_pool.class_name(idx).to_owned())
            .collect();
        for super_iface in super_ifaces {
            self.collect_class_init_superinterfaces(&super_iface, seen, ordered);
        }
        if self.declares_concrete_interface_method(interface_name) {
            ordered.push(interface_name.to_owned());
        }
    }

    /// JVMS §5.5 step 7:
    /// before initializing a class, initialize direct superinterfaces and their
    /// superinterfaces in left-to-right recursive order, but only if the
    /// interface declares at least one non-abstract, non-static method.
    fn class_init_superinterfaces(&mut self, class_name: &str) -> Vec<String> {
        if let Some(cached) = self.class_init_superinterface_cache.get(class_name) {
            return cached.clone();
        }
        self.ensure_class_ready(class_name);
        let Some(class) = self.get_class(class_name) else {
            return Vec::new();
        };
        if (class.access_flags & 0x0200) != 0 {
            return Vec::new();
        }
        let direct_ifaces: Vec<String> = class
            .interfaces
            .iter()
            .map(|&idx| class.constant_pool.class_name(idx).to_owned())
            .collect();
        let mut seen = HashSet::new();
        let mut ordered = Vec::new();
        for iface in direct_ifaces {
            self.collect_class_init_superinterfaces(&iface, &mut seen, &mut ordered);
        }
        self.class_init_superinterface_cache
            .insert(class_name.to_owned(), ordered.clone());
        ordered
    }

    fn find_static_field_owner(
        &mut self,
        class_name: &str,
        field_name: &str,
        descriptor: &str,
    ) -> Option<String> {
        let cache_key = (
            class_name.to_owned(),
            field_name.to_owned(),
            descriptor.to_owned(),
        );
        if let Some(cached) = self.static_field_owner_cache.get(&cache_key) {
            return cached.clone();
        }
        let result = self.find_static_field_owner_uncached(class_name, field_name, descriptor);
        if result.is_some() {
            self.static_field_owner_cache
                .insert(cache_key, result.clone());
        }
        result
    }

    fn find_static_field_owner_uncached(
        &mut self,
        class_name: &str,
        field_name: &str,
        descriptor: &str,
    ) -> Option<String> {
        self.ensure_class_ready(class_name);
        let class = self.get_class(class_name)?;
        for field in &class.fields {
            let name = class.constant_pool.utf8(field.name_index);
            let desc = class.constant_pool.utf8(field.descriptor_index);
            if name == field_name && desc == descriptor && (field.access_flags & 0x0008) != 0 {
                return Some(class.constant_pool.class_name(class.this_class).to_owned());
            }
        }
        let super_name: Option<String> = if class.super_class != 0 {
            Some(class.constant_pool.class_name(class.super_class).to_owned())
        } else {
            None
        };
        let iface_names: Vec<String> = class
            .interfaces
            .iter()
            .map(|&idx| class.constant_pool.class_name(idx).to_owned())
            .collect();
        for iface_name in iface_names {
            if let Some(owner) =
                self.find_static_field_owner(&iface_name, field_name, descriptor)
            {
                return Some(owner);
            }
        }
        if let Some(super_name) = super_name {
            if let Some(owner) =
                self.find_static_field_owner(&super_name, field_name, descriptor)
            {
                return Some(owner);
            }
        }
        None
    }

    pub(crate) fn mark_class_init_done(&mut self, class_name: &str) {
        self.clinit_owners.remove(class_name);
        self.clinit_pending.remove(class_name);
        self.clinit_running.remove(class_name);
        self.clinit_failed.remove(class_name);
        self.clinit_done.insert(class_name.to_owned());
    }

    pub(crate) fn mark_class_init_failed(&mut self, class_name: &str) {
        self.clinit_owners.remove(class_name);
        self.clinit_pending.remove(class_name);
        self.clinit_running.remove(class_name);
        self.clinit_done.remove(class_name);
        self.clinit_failed.insert(class_name.to_owned());
    }

    fn current_thread_id(&self) -> ThreadId {
        self.scheduler.current_thread().id
    }

    fn class_init_owner(&self, class_name: &str) -> Option<ThreadId> {
        self.clinit_owners.get(class_name).copied()
    }

    fn begin_class_init(&mut self, class_name: &str) {
        self.clinit_owners
            .insert(class_name.to_owned(), self.current_thread_id());
    }

    pub(crate) fn push_sync_clinit_frame(&mut self, fi: &trampoline::FrameInfo) {
        if let Some(class_name) = &fi.class_initializer_owner {
            self.sync_clinit_stack.push(class_name.clone());
        }
    }

    pub(crate) fn pop_sync_clinit_frame(&mut self, fi: &trampoline::FrameInfo) {
        let Some(class_name) = &fi.class_initializer_owner else {
            return;
        };
        if matches!(self.sync_clinit_stack.last(), Some(last) if last == class_name) {
            self.sync_clinit_stack.pop();
            return;
        }
        if let Some(pos) = self
            .sync_clinit_stack
            .iter()
            .rposition(|active| active == class_name)
        {
            self.sync_clinit_stack.remove(pos);
        }
    }

    pub(crate) fn register_sync_clinit_frames(&mut self, call_stack: &[trampoline::FrameInfo]) {
        for fi in call_stack {
            self.push_sync_clinit_frame(fi);
        }
    }

    pub(crate) fn unregister_sync_clinit_frames(&mut self, call_stack: &[trampoline::FrameInfo]) {
        for fi in call_stack.iter().rev() {
            self.pop_sync_clinit_frame(fi);
        }
    }

    pub(crate) fn push_thread_clinit_frame(&mut self, fi: &trampoline::FrameInfo) {
        if let Some(class_name) = &fi.class_initializer_owner {
            self.scheduler
                .current_thread_mut()
                .active_clinit_stack
                .push(class_name.clone());
        }
    }

    pub(crate) fn pop_thread_clinit_frame(&mut self, fi: &trampoline::FrameInfo) {
        let Some(class_name) = &fi.class_initializer_owner else {
            return;
        };
        let stack = &mut self.scheduler.current_thread_mut().active_clinit_stack;
        if matches!(stack.last(), Some(last) if last == class_name) {
            stack.pop();
            return;
        }
        if let Some(pos) = stack.iter().rposition(|active| active == class_name) {
            stack.remove(pos);
        }
    }

    pub(crate) fn register_thread_clinit_frames(&mut self, call_stack: &[trampoline::FrameInfo]) {
        for fi in call_stack {
            self.push_thread_clinit_frame(fi);
        }
    }

    pub(crate) fn unregister_thread_clinit_frames(
        &mut self,
        call_stack: &[trampoline::FrameInfo],
    ) {
        for fi in call_stack.iter().rev() {
            self.pop_thread_clinit_frame(fi);
        }
    }

    fn is_class_initializer_active_on_current_stack(&self, class_name: &str) -> bool {
        if self
            .sync_clinit_stack
            .iter()
            .any(|active| active == class_name)
        {
            return true;
        }
        self.scheduler
            .current_thread()
            .active_clinit_stack
            .iter()
            .any(|active| active == class_name)
    }

    fn class_init_prerequisites(&mut self, class_name: &str) -> (Option<String>, Vec<String>) {
        self.ensure_class_ready(class_name);
        let Some(class) = self.get_class(class_name) else {
            return (None, Vec::new());
        };
        if (class.access_flags & 0x0200) != 0 {
            return (None, Vec::new());
        }
        let super_name = if class.super_class != 0 {
            let s = class.constant_pool.class_name(class.super_class).to_owned();
            (s != "java/lang/Object").then_some(s)
        } else {
            None
        };
        let iface_names = self.class_init_superinterfaces(class_name);
        (super_name, iface_names)
    }

    fn class_init_depends_on(
        &mut self,
        class_name: &str,
        prerequisite: &str,
        visited: &mut HashSet<String>,
    ) -> bool {
        if !visited.insert(class_name.to_owned()) {
            return false;
        }
        let (super_name, iface_names) = self.class_init_prerequisites(class_name);
        if super_name.as_deref() == Some(prerequisite) {
            return true;
        }
        if iface_names.iter().any(|iface| iface == prerequisite) {
            return true;
        }
        if let Some(super_name) = super_name {
            if self.class_init_depends_on(&super_name, prerequisite, visited) {
                return true;
            }
        }
        for iface in iface_names {
            if self.class_init_depends_on(&iface, prerequisite, visited) {
                return true;
            }
        }
        false
    }

    fn current_stack_is_in_class_init_prerequisite_of(&mut self, class_name: &str) -> bool {
        let mut active = self.sync_clinit_stack.clone();
        active.extend(
            self.scheduler
                .current_thread()
                .active_clinit_stack
                .iter()
                .cloned(),
        );
        active.into_iter().any(|active_class| {
            let mut visited = HashSet::new();
            self.class_init_depends_on(class_name, &active_class, &mut visited)
        })
    }

    /// Run `<clinit>` for a class if it hasn't been initialized yet.
    /// Per JVMS §5.5: Before a class is initialized, its direct superclass must
    /// be initialized first (recursively), and any superinterfaces that declare
    /// concrete non-static methods must also be initialized.
    fn ensure_class_init(&mut self, class_name: &str) -> Result<(), String> {
        if self.clinit_done.contains(class_name) {
            return Ok(());
        }
        if self.clinit_failed.contains(class_name) {
            self.throw_no_class_def_found(class_name);
            return Err(format!("java/lang/NoClassDefFoundError: {class_name}"));
        }
        let current_thread_id = self.current_thread_id();
        if let Some(owner) = self.class_init_owner(class_name) {
            if owner == current_thread_id {
                return Ok(());
            }
            // The synchronous path has no cooperative wait mechanism. In practice
            // it should only see current-thread re-entry, so treat foreign-thread
            // ownership as already in progress and avoid double-starting <clinit>.
            return Ok(());
        }
        self.begin_class_init(class_name);
        self.clinit_pending.insert(class_name.to_owned());

        let (super_name, iface_names) = self.class_init_prerequisites(class_name);
        if let Some(s) = super_name {
            if let Err(err) = self.ensure_class_init(&s) {
                self.mark_class_init_failed(class_name);
                return Err(err);
            }
        }
        for iface in iface_names {
            if let Err(err) = self.ensure_class_init(&iface) {
                self.mark_class_init_failed(class_name);
                return Err(err);
            }
        }

        if !self.has_class_initializer(class_name) {
            self.mark_class_init_done(class_name);
            return Ok(());
        }

        self.clinit_pending.remove(class_name);
        self.clinit_running.insert(class_name.to_owned());
        if let Err(e) = self.invoke_static(class_name, "<clinit>", "()V", vec![]) {
            let cause = self.pending_exception_mut().take();
            let eiie = self.new_vm_exception_message("java/lang/ExceptionInInitializerError", e.clone());
            if let Some(c) = cause {
                eiie.borrow_mut().fields.insert("cause".to_owned(), JValue::Ref(Some(c)));
            }
            *self.pending_exception_mut() = Some(eiie);
            self.mark_class_init_failed(class_name);
            return Err("java/lang/ExceptionInInitializerError".to_owned());
        }
        self.mark_class_init_done(class_name);
        Ok(())
    }

    pub(crate) fn ensure_class_init_or_schedule(&mut self, class_name: &str) -> Result<bool, String> {
        if self.clinit_done.contains(class_name) {
            return Ok(false);
        }
        if self.clinit_failed.contains(class_name) {
            self.throw_no_class_def_found(class_name);
            return Err(format!("java/lang/NoClassDefFoundError: {class_name}"));
        }
        if self.is_class_initializer_active_on_current_stack(class_name) {
            return Ok(false);
        }
        let current_thread_id = self.current_thread_id();
        match self.class_init_owner(class_name) {
            Some(owner) if owner != current_thread_id => return Ok(true),
            Some(_) => {
                if self.clinit_running.contains(class_name) {
                    return Ok(true);
                }
            }
            None => {
                self.begin_class_init(class_name);
                self.clinit_pending.insert(class_name.to_owned());
            }
        }

        let (super_name, iface_names) = self.class_init_prerequisites(class_name);
        if let Some(s) = super_name {
            match self.ensure_class_init_or_schedule(&s) {
                Ok(should_yield) => {
                    if should_yield || !self.clinit_done.contains(&s) {
                        if self.current_stack_is_in_class_init_prerequisite_of(class_name) {
                            return Ok(false);
                        }
                        return Ok(true);
                    }
                }
                Err(err) => {
                    self.mark_class_init_failed(class_name);
                    return Err(err);
                }
            }
        }
        for iface in iface_names {
            match self.ensure_class_init_or_schedule(&iface) {
                Ok(should_yield) => {
                    if should_yield || !self.clinit_done.contains(&iface) {
                        if self.current_stack_is_in_class_init_prerequisite_of(class_name) {
                            return Ok(false);
                        }
                        return Ok(true);
                    }
                }
                Err(err) => {
                    self.mark_class_init_failed(class_name);
                    return Err(err);
                }
            }
        }

        if !self.has_class_initializer(class_name) {
            self.mark_class_init_done(class_name);
            return Ok(false);
        }

        self.clinit_pending.remove(class_name);
        self.clinit_running.insert(class_name.to_owned());
        match self.build_static_frame(class_name, "<clinit>", "()V", vec![], false) {
            Ok(Some(fi)) => {
                debug_assert!(self.pending_frame_mut().is_none());
                *self.pending_frame_mut() = Some(fi);
                Ok(true)
            }
            Ok(None) => {
                if let Err(e) = self.invoke_static(class_name, "<clinit>", "()V", vec![]) {
                    let cause = self.pending_exception_mut().take();
                    let eiie = self.new_vm_exception_message(
                        "java/lang/ExceptionInInitializerError",
                        e.clone(),
                    );
                    if let Some(c) = cause {
                        eiie.borrow_mut().fields.insert("cause".to_owned(), JValue::Ref(Some(c)));
                    }
                    *self.pending_exception_mut() = Some(eiie);
                    self.mark_class_init_failed(class_name);
                    return Err("java/lang/ExceptionInInitializerError".to_owned());
                }
                self.mark_class_init_done(class_name);
                Ok(false)
            }
            Err(err) => {
                self.mark_class_init_failed(class_name);
                Err(err)
            }
        }
    }

    /// Recursively create a multi-dimensional array for `multianewarray`.
    fn create_multi_array(&self, desc: &str, sizes: &[usize], depth: usize) -> JRef {
        let count = sizes[depth];
        if depth + 1 >= sizes.len() {
            let elem = if desc.ends_with("[I") || desc.ends_with("[B") || desc.ends_with("[C") || desc.ends_with("[S") || desc.ends_with("[Z") {
                JValue::Int(0)
            } else if desc.ends_with("[J") {
                JValue::Long(0)
            } else if desc.ends_with("[F") {
                JValue::Float(0.0)
            } else if desc.ends_with("[D") {
                JValue::Double(0.0)
            } else {
                JValue::Ref(None)
            };
            JObject::new_array(desc, vec![elem; count])
        } else {
            let sub_desc = &desc[1..];
            let elements: Vec<JValue> = (0..count)
                .map(|_| JValue::Ref(Some(self.create_multi_array(sub_desc, sizes, depth + 1))))
                .collect();
            JObject::new_array(desc, elements)
        }
    }

    /// Check if `runtime_class` is an instance of `target_class` (by name).
    /// Handles array types per JVMS §6.5.instanceof / §6.5.checkcast.
    fn is_instance_of(&mut self, runtime_class: &str, target_class: &str) -> bool {
        let cache_key = (runtime_class.to_owned(), target_class.to_owned());
        if let Some(cached) = self.instanceof_cache.get(&cache_key) {
            return *cached;
        }
        let result = self.is_instance_of_uncached(runtime_class, target_class);
        self.instanceof_cache.insert(cache_key, result);
        result
    }

    fn is_instance_of_uncached(&mut self, runtime_class: &str, target_class: &str) -> bool {
        if runtime_class == target_class { return true; }
        if target_class == "java/lang/Object" { return true; }

        if runtime_class.starts_with('[') {
            if target_class == "java/lang/Cloneable" || target_class == "java/io/Serializable" {
                return true;
            }
            if target_class.starts_with('[') {
                let rc = &runtime_class[1..];
                let tc = &target_class[1..];
                let rc_class = descriptor_to_class_name(rc);
                let tc_class = descriptor_to_class_name(tc);
                if let (Some(r), Some(t)) = (rc_class, tc_class) {
                    return self.is_instance_of(&r, &t);
                }
                return false;
            }
            return false;
        }

        self.ensure_class_ready(runtime_class);
        let (iface_names, super_name) = if let Some(class) = self.get_class(runtime_class) {
            let ifaces: Vec<String> = class.interfaces.iter()
                .map(|&idx| class.constant_pool.class_name(idx).to_owned())
                .collect();
            let sup = if class.super_class != 0 {
                Some(class.constant_pool.class_name(class.super_class).to_owned())
            } else {
                None
            };
            (ifaces, sup)
        } else {
            return false;
        };
        for iface_name in &iface_names {
            if self.is_instance_of(iface_name, target_class) { return true; }
        }
        if let Some(super_name) = super_name {
            if self.is_instance_of(&super_name, target_class) {
                return true;
            }
        }
        false
    }
}
