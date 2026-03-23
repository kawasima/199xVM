//! Java bytecode interpreter.
//!
//! Implements a stack-based interpreter over the JVM instruction set.
//! The focus is on the subset needed to run Raoh:
//! - Core stack / local-variable operations
//! - Object creation and field access
//! - Method invocation (all four flavours + `invokedynamic`)
//! - Integer / long / reference comparisons and control flow
//! - Native stubs for `java.lang.*` and `java.util.*`

use std::collections::VecDeque;
use std::rc::Rc;

#[cfg(target_arch = "wasm32")]
use wasm_bindgen::prelude::*;

use crate::class_file::{
    self, Attribute, BootstrapMethod, ClassFile, ConstantPoolEntry, ExceptionTableEntry,
};
use crate::collections::{HashMap, HashSet};
use crate::heap::{JObject, JRef, JValue};

type OwnedJarArchive = zip::ZipArchive<std::io::Cursor<Vec<u8>>>;

/// All execution-time data extracted from a resolved method in a single pass.
/// Returned by [`Vm::resolve_method_exec_info`] to avoid repeated `find_method`
/// calls and to give each field a self-documenting name.
#[derive(Clone)]
pub(super) struct MethodExecInfo {
    /// Stable VM-local class identifier for the declaring class.
    pub class_id: ClassId,
    /// Internal class name that owns the resolved method.
    pub class_name: String,
    /// Resolved method descriptor (may differ from the call-site descriptor for generics).
    pub descriptor: String,
    /// Parameter-only descriptor prefix used for relaxed signature matching.
    pub parameter_descriptor: Rc<str>,
    /// Number of source-level arguments in the descriptor.
    pub arg_count: usize,
    /// Whether the method is marked ACC_VARARGS.
    pub is_varargs: bool,
    /// `Code.max_stack` (0 if the method has no `Code` attribute).
    pub max_stack: usize,
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
    /// Precomputed local-slot width for each parameter token (1 for most, 2 for long/double).
    pub param_slot_steps: Rc<Vec<usize>>,
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
    pub instance_field_slot: Option<usize>,
}

#[derive(Clone)]
struct ClassRuntimeMetadata {
    access_flags: u16,
    #[allow(dead_code)]
    super_name: Option<String>,
    super_id: Option<ClassId>,
    #[allow(dead_code)]
    interface_names: Rc<Vec<String>>,
    interface_ids: Rc<Vec<ClassId>>,
    declared_methods: HashMap<(String, String), Rc<MethodExecInfo>>,
    declared_methods_by_name: HashMap<String, Vec<Rc<MethodExecInfo>>>,
    class_initializer: Option<Rc<MethodExecInfo>>,
}

#[derive(Clone)]
struct ClassInitPrerequisites {
    super_id: Option<ClassId>,
    interface_ids: Rc<Vec<ClassId>>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ClassInitStage {
    Planning,
    Running,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ClassInitMode {
    Sync,
    Schedule,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ClassInitOutcome {
    Completed,
    Yield,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(crate) struct ClassId(usize);

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
struct LoaderId(u32);

const VM_DEFAULT_LOADER_ID: LoaderId = LoaderId(0);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ClassLifecycleState {
    Loading,
    Loaded,
    Prepared,
    Initializing {
        owner: ThreadId,
        stage: ClassInitStage,
    },
    Initialized,
    Erroneous,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ClassTerminalError {
    Parse,
    Initialization,
}

pub(in crate::interpreter) enum ClassSource {
    PendingBytes(Vec<u8>),
    PendingJarEntry(JarEntryRef),
    Ready(ClassFile),
    ParseError(String),
}

struct ClassRecord {
    #[allow(dead_code)]
    defining_loader: LoaderId,
    binary_name: String,
    source: ClassSource,
    lifecycle: ClassLifecycleState,
    runtime_metadata: Option<Rc<ClassRuntimeMetadata>>,
    init_prerequisites: Option<ClassInitPrerequisites>,
    terminal_error: Option<ClassTerminalError>,
}

#[cfg(test)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ClassSourceKind {
    PendingBytes,
    PendingJarEntry,
    Ready,
    ParseError,
}

#[derive(Clone)]
struct InstanceFieldLayout {
    default_values: Rc<Vec<JValue>>,
    slot_lookup: HashMap<(ClassId, String, String), usize>,
    native_name_slots: HashMap<String, usize>,
    native_name_counts: HashMap<String, usize>,
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
    class_init_stats: HashMap<String, VmProfileStat>,
    phase_stats: HashMap<&'static str, VmProfileStat>,
    counters: HashMap<&'static str, u64>,
    top_n: usize,
}

impl VmProfiler {
    fn new(top_n: usize) -> Self {
        Self {
            opcode_counts: [0; 256],
            opcode_nanos: [0; 256],
            method_stats: HashMap::default(),
            for_name_stats: HashMap::default(),
            class_init_stats: HashMap::default(),
            phase_stats: HashMap::default(),
            counters: HashMap::default(),
            top_n,
        }
    }

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
        Some(Self::new(top_n))
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
        let stat = self
            .for_name_stats
            .entry(runtime_name.to_owned())
            .or_default();
        stat.count += 1;
        stat.nanos += elapsed.as_nanos();
    }

    fn record_class_init(&mut self, class_name: &str, elapsed: std::time::Duration) {
        let stat = self.class_init_stats.entry(class_name.to_owned()).or_default();
        stat.count += 1;
        stat.nanos += elapsed.as_nanos();
    }

    fn record_phase(&mut self, phase_name: &'static str, elapsed: std::time::Duration) {
        let stat = self.phase_stats.entry(phase_name).or_default();
        stat.count += 1;
        stat.nanos += elapsed.as_nanos();
    }

    fn increment_counter(&mut self, counter_name: &'static str) {
        *self.counters.entry(counter_name).or_default() += 1;
    }

    fn clear(&mut self) {
        let top_n = self.top_n;
        *self = Self::new(top_n);
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

        if !self.class_init_stats.is_empty() {
            let mut class_inits: Vec<_> = self.class_init_stats.iter().collect();
            class_inits.sort_by(|a, b| b.1.nanos.cmp(&a.1.nanos).then_with(|| a.0.cmp(b.0)));
            lines.push("class-init:".to_owned());
            for (name, stat) in class_inits.into_iter().take(self.top_n) {
                lines.push(format!(
                    "  {:9.3} ms  {:8} calls  {}",
                    stat.nanos as f64 / 1_000_000.0,
                    stat.count,
                    name
                ));
            }
        }

        if !self.phase_stats.is_empty() {
            let mut phases: Vec<_> = self.phase_stats.iter().collect();
            phases.sort_by(|a, b| b.1.nanos.cmp(&a.1.nanos).then_with(|| a.0.cmp(b.0)));
            lines.push("phases:".to_owned());
            for (name, stat) in phases.into_iter().take(self.top_n) {
                lines.push(format!(
                    "  {:9.3} ms  {:8} calls  {}",
                    stat.nanos as f64 / 1_000_000.0,
                    stat.count,
                    name
                ));
            }
        }

        if !self.counters.is_empty() {
            let mut counters: Vec<_> = self.counters.iter().collect();
            counters.sort_by(|a, b| a.0.cmp(b.0));
            lines.push("counters:".to_owned());
            for (name, value) in counters {
                lines.push(format!("  {:8}  {}", value, name));
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
    use super::{ClassSourceKind, Vm};
    use std::io::{Cursor, Write};

    fn build_misnamed_jar() -> Vec<u8> {
        let mut archive = zip::ZipArchive::new(Cursor::new(
            include_bytes!("../../tests/test.jar").as_slice(),
        ))
        .expect("open test jar");
        let mut class_file = archive
            .by_name("JarTestEntry.class")
            .expect("JarTestEntry.class");
        let mut class_bytes = Vec::new();
        std::io::Read::read_to_end(&mut class_file, &mut class_bytes).expect("read class bytes");

        let mut jar_bytes = Vec::new();
        {
            let cursor = Cursor::new(&mut jar_bytes);
            let mut writer = zip::ZipWriter::new(cursor);
            let options = zip::write::SimpleFileOptions::default()
                .compression_method(zip::CompressionMethod::Deflated);
            writer
                .start_file("wrong/Path.class", options)
                .expect("start class entry");
            writer.write_all(&class_bytes).expect("write class entry");
            writer.finish().expect("finish jar");
        }
        jar_bytes
    }

    fn class_entries_in_test_jar() -> Vec<(String, Vec<u8>)> {
        let mut archive = zip::ZipArchive::new(Cursor::new(
            include_bytes!("../../tests/test.jar").as_slice(),
        ))
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
        let count = vm
            .load_jar(include_bytes!("../../tests/test.jar"))
            .expect("load_jar failed");
        assert!(count > 0, "expected at least one class in test JAR");
        assert!(matches!(
            vm.class_source_kind("JarTestEntry"),
            Some(ClassSourceKind::PendingJarEntry)
        ));

        vm.ensure_class_ready("JarTestEntry");

        assert!(matches!(
            vm.class_source_kind("JarTestEntry"),
            Some(ClassSourceKind::Ready)
        ));
    }

    #[test]
    fn misnamed_jar_class_uses_entry_path_and_fails_on_parse() {
        let mut vm = Vm::new();
        let count = vm.load_jar(&build_misnamed_jar()).expect("load_jar failed");
        assert_eq!(count, 1, "expected one class in misnamed jar");
        assert!(matches!(
            vm.class_source_kind("wrong/Path"),
            Some(ClassSourceKind::PendingJarEntry)
        ));
        assert!(
            vm.resolve_class("JarTestEntry").is_none(),
            "must not recover by internal name"
        );

        vm.ensure_class_ready("wrong/Path");

        match vm.class_parse_error("wrong/Path") {
            Some(err) => {
                assert!(
                    err.contains("Class name mismatch"),
                    "unexpected error: {err}"
                );
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
        vm.load_jar(include_bytes!("../../tests/test.jar"))
            .expect("load test jar");

        for class_name in &class_names {
            assert!(
                matches!(
                    vm.class_source_kind(class_name),
                    Some(ClassSourceKind::PendingJarEntry)
                ),
                "expected pending jar entry before miss: {class_name}"
            );
        }

        vm.ensure_class_ready("missing/Type");

        for class_name in &class_names {
            assert!(
                matches!(
                    vm.class_source_kind(class_name),
                    Some(ClassSourceKind::PendingJarEntry)
                ),
                "packaged miss must not parse or rewrite pending entry: {class_name}"
            );
        }
        assert!(
            vm.resolve_class("missing/Type").is_none(),
            "missing class must remain unresolved"
        );
    }

    #[test]
    fn ensure_class_loaded_by_name_reports_parse_failure() {
        let mut vm = Vm::new();
        vm.load_lazy("broken/Type".to_owned(), b"not-a-class".to_vec());

        let err = vm
            .ensure_class_loaded_by_name("broken/Type")
            .expect_err("invalid class bytes must return Err");

        assert!(
            !err.is_empty(),
            "expected non-empty parse/load error"
        );
        assert!(matches!(
            vm.class_source_kind("broken/Type"),
            Some(ClassSourceKind::ParseError)
        ));
    }
}

#[derive(Debug, Clone)]
pub(in crate::interpreter) struct JarEntryRef {
    pub jar_id: usize,
    pub entry_index: usize,
    pub entry_name: String,
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
    pub active_clinit_stack: Vec<ClassId>,
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
        self.threads
            .iter()
            .all(|t| t.state == ThreadState::Terminated)
    }

    /// Returns true if only the main thread (id=0) is alive.
    pub fn only_main_alive(&self) -> bool {
        self.threads
            .iter()
            .all(|t| t.id == 0 || t.state == ThreadState::Terminated)
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
        let terminated: HashSet<ThreadId> = self
            .threads
            .iter()
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
        self.threads
            .iter()
            .filter(|t| t.state == ThreadState::Runnable)
            .count()
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
        self.threads
            .iter()
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
    /// Stable class records indexed by `ClassId`.
    class_records: Vec<ClassRecord>,
    /// Identity index: `(defining_loader, binary_name) -> ClassId`.
    class_identity_index: HashMap<LoaderId, HashMap<String, ClassId>>,
    /// Interned strings cache (not strictly required but saves allocations).
    pub(in crate::interpreter) string_pool: HashMap<String, JRef>,
    /// Static field storage keyed by `ClassId` → field name.
    pub(in crate::interpreter) static_fields: HashMap<ClassId, HashMap<String, JValue>>,
    /// `<clinit>` frames currently executing on the synchronous `run_trampoline` path.
    sync_clinit_stack: Vec<ClassId>,
    /// Canonical `java/lang/Class` mirrors for registered classes.
    class_mirror_pool: HashMap<ClassId, JRef>,
    /// Canonical Class objects keyed by internal class name or descriptor.
    /// Used for arrays, primitives, and any mirror not yet tied to a `ClassId`.
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
    /// Method resolution cache: (class id, method_name, descriptor) → owner class id.
    /// Avoids repeated super-chain walks for the same method lookup.
    method_owner_cache: HashMap<(ClassId, String, String), Option<ClassId>>,
    /// Resolved method signature cache: requested call site → (real descriptor, access flags).
    /// Avoids repeated relaxed descriptor matching and flags lookups on hot invoke paths.
    method_signature_cache: HashMap<(ClassId, String, String), (String, u16)>,
    /// Cached decoded method refs keyed by `(cp pointer, cp index)`.
    methodref_constant_cache: HashMap<(usize, u16), Rc<ResolvedMemberRef>>,
    /// Cached decoded class refs keyed by `(cp pointer, cp index)`.
    classref_constant_cache: HashMap<(usize, u16), Rc<str>>,
    /// Static field owner cache: symbolic owner/name/descriptor → declaring class id.
    /// Mirrors HotSpot's resolved field entries well enough for repeated getstatic/putstatic.
    static_field_owner_cache: HashMap<(ClassId, String, String), Option<ClassId>>,
    /// Cached decoded field refs keyed by `(cp pointer, cp index)`.
    fieldref_constant_cache: HashMap<(usize, u16), Rc<ResolvedMemberRef>>,
    /// Cached resolved method execution metadata keyed by requested call-site triplet.
    method_exec_info_cache: HashMap<(ClassId, String, String), MethodExecInfo>,
    /// Cached presence of concrete non-static interface methods.
    concrete_interface_method_cache: HashMap<ClassId, bool>,
    /// Cached inherited instance-field counts used to pre-size object field storage.
    instance_field_capacity_cache: HashMap<ClassId, usize>,
    /// Cached declared instance-field layouts used by bytecode getfield/putfield.
    instance_field_layout_cache: HashMap<ClassId, Rc<InstanceFieldLayout>>,
    /// Cached resolved instance-field slots keyed by `(symbolic owner, name, descriptor)`.
    instance_field_slot_cache: HashMap<(ClassId, String, String), Option<usize>>,
    /// Cached subtype checks keyed by `(runtime_class, target_class)`.
    instanceof_cache: HashMap<(ClassId, ClassId), bool>,
    /// Cached ordered superinterfaces that must be initialized before a class.
    class_init_superinterface_cache: HashMap<ClassId, Rc<Vec<ClassId>>>,
    /// Materialized non-class resources from loaded JARs, keyed by path.
    pub resources: HashMap<String, Vec<u8>>,
    /// Shared byte-array objects for resource-backed input streams.
    /// Cached declared-field metadata, analogous to OpenJDK's ReflectionData fast path.
    reflection_fields_cache: HashMap<ClassId, Rc<Vec<ReflectFieldInfo>>>,
    /// Cached declared-method metadata, analogous to OpenJDK's ReflectionData fast path.
    reflection_methods_cache: HashMap<ClassId, Rc<Vec<ReflectMethodInfo>>>,
    /// Cached declared-constructor metadata, analogous to OpenJDK's ReflectionData fast path.
    reflection_ctors_cache: HashMap<ClassId, Rc<Vec<ReflectConstructorInfo>>>,
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
            class_records: Vec::new(),
            class_identity_index: HashMap::default(),
            string_pool: HashMap::default(),
            static_fields: HashMap::default(),
            sync_clinit_stack: Vec::new(),
            class_mirror_pool: HashMap::default(),
            class_pool: HashMap::default(),
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
            monitors: HashMap::default(),
            method_owner_cache: HashMap::default(),
            method_signature_cache: HashMap::default(),
            methodref_constant_cache: HashMap::default(),
            classref_constant_cache: HashMap::default(),
            static_field_owner_cache: HashMap::default(),
            fieldref_constant_cache: HashMap::default(),
            method_exec_info_cache: HashMap::default(),
            concrete_interface_method_cache: HashMap::default(),
            instance_field_capacity_cache: HashMap::default(),
            instance_field_layout_cache: HashMap::default(),
            instance_field_slot_cache: HashMap::default(),
            instanceof_cache: HashMap::default(),
            class_init_superinterface_cache: HashMap::default(),
            resources: HashMap::default(),
            reflection_fields_cache: HashMap::default(),
            reflection_methods_cache: HashMap::default(),
            reflection_ctors_cache: HashMap::default(),
            pending_resources: HashMap::default(),
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

    fn class_id_for_name(&self, name: &str) -> Option<ClassId> {
        self.class_identity_index
            .get(&VM_DEFAULT_LOADER_ID)
            .and_then(|by_name| by_name.get(name))
            .copied()
    }

    fn tracked_class_id_for_name(&mut self, name: &str) -> Option<ClassId> {
        self.increment_profile_counter("class.identity_lookup.fast");
        let started = self.profiler.as_ref().map(|_| std::time::Instant::now());
        let result = self.class_id_for_name(name);
        self.record_class_identity_lookup("class.identity_lookup.fast", started);
        result
    }

    fn record_class_identity_lookup(
        &mut self,
        phase_name: &'static str,
        started: Option<std::time::Instant>,
    ) {
        if let Some(started) = started {
            self.record_profile_phase_sample(phase_name, started.elapsed());
        }
    }

    fn lookup_class_id(&mut self, loader: LoaderId, binary_name: &str) -> Option<ClassId> {
        self.increment_profile_counter("class.identity_lookup");
        let started = self.profiler.as_ref().map(|_| std::time::Instant::now());
        let result = self
            .class_identity_index
            .get(&loader)
            .and_then(|by_name| by_name.get(binary_name))
            .copied();
        self.record_class_identity_lookup("class.identity_lookup", started);
        result
    }

    fn class_record(&self, class_id: ClassId) -> Option<&ClassRecord> {
        self.class_records.get(class_id.0)
    }

    fn class_record_mut(&mut self, class_id: ClassId) -> Option<&mut ClassRecord> {
        self.class_records.get_mut(class_id.0)
    }

    fn class_name(&self, class_id: ClassId) -> Option<&str> {
        self.class_record(class_id).map(|record| record.binary_name.as_str())
    }

    fn object_runtime_class_id(&mut self, obj: &JRef) -> Option<ClassId> {
        if let Some(class_id) = obj.borrow().class_id {
            self.increment_profile_counter("object.class_id.hit");
            return Some(class_id);
        }
        let class_name = obj.borrow().class_name.clone();
        let class_id = self.tracked_class_id_for_name(&class_name)?;
        self.increment_profile_counter("object.class_id.memoize");
        obj.borrow_mut().class_id = Some(class_id);
        Some(class_id)
    }

    fn class_mirror_id_from_obj(&mut self, class_obj: &JRef) -> Option<ClassId> {
        if let Some(class_id) = class_obj.borrow().represented_class_id {
            self.increment_profile_counter("class.mirror.id.hit");
            return Some(class_id);
        }
        let internal_name = class_obj
            .borrow()
            .fields
            .get("__name_internal")
            .and_then(|v| v.as_ref())
            .and_then(|r| r.borrow().as_java_string().map(|s| s.to_owned()))?;
        let class_id = self.tracked_class_id_for_name(&internal_name)?;
        self.increment_profile_counter("class.mirror.id.memoize");
        class_obj.borrow_mut().represented_class_id = Some(class_id);
        self.class_mirror_pool
            .entry(class_id)
            .or_insert_with(|| Rc::clone(class_obj));
        Some(class_id)
    }

    fn class_target_from_mirror(&mut self, class_obj: &JRef) -> Option<(Option<ClassId>, String)> {
        if let Some(class_id) = self.class_mirror_id_from_obj(class_obj) {
            if let Some(class_name) = self.class_name(class_id) {
                return Some((Some(class_id), class_name.to_owned()));
            }
        }
        let internal_name = class_obj
            .borrow()
            .fields
            .get("__name_internal")
            .and_then(|v| v.as_ref())
            .and_then(|r| r.borrow().as_java_string().map(|s| s.to_owned()))?;
        Some((None, internal_name))
    }

    fn allocate_loaded_object(
        &self,
        class_id: ClassId,
        field_capacity: usize,
        field_slots: Vec<JValue>,
    ) -> JRef {
        let class_name = self.class_name(class_id).unwrap_or_default().to_owned();
        let obj = JObject::with_field_capacity_and_slots(class_name, field_capacity, field_slots);
        obj.borrow_mut().class_id = Some(class_id);
        obj
    }

    fn static_field_value_by_id(&self, class_id: ClassId, field_name: &str) -> Option<&JValue> {
        self.static_fields
            .get(&class_id)
            .and_then(|fields| fields.get(field_name))
    }

    fn static_field_value(&mut self, class_name: &str, field_name: &str) -> Option<JValue> {
        let class_id = self.tracked_class_id_for_name(class_name)?;
        self.static_field_value_by_id(class_id, field_name).cloned()
    }

    fn static_field_owner_id_or_self(
        &mut self,
        class_name: &str,
        field_name: &str,
        descriptor: &str,
    ) -> Option<ClassId> {
        let class_id = self.tracked_class_id_for_name(class_name)?;
        Some(
            self.find_static_field_owner_id(class_id, field_name, descriptor)
                .unwrap_or(class_id),
        )
    }

    fn set_static_field_value_by_id(
        &mut self,
        class_id: ClassId,
        field_name: impl Into<String>,
        value: JValue,
    ) {
        self.static_fields
            .entry(class_id)
            .or_default()
            .insert(field_name.into(), value);
    }

    fn set_static_field_value(
        &mut self,
        class_name: &str,
        field_name: impl Into<String>,
        value: JValue,
    ) {
        let Some(class_id) = self.tracked_class_id_for_name(class_name) else {
            return;
        };
        self.set_static_field_value_by_id(class_id, field_name, value);
    }

    fn extend_static_field_values(&mut self, class_id: ClassId, fields: HashMap<String, JValue>) {
        if fields.is_empty() {
            return;
        }
        self.static_fields.entry(class_id).or_default().extend(fields);
    }

    fn parsed_class(&self, class_id: ClassId) -> Option<&ClassFile> {
        match &self.class_record(class_id)?.source {
            ClassSource::Ready(cf) => Some(cf),
            ClassSource::PendingBytes(_)
            | ClassSource::PendingJarEntry(_)
            | ClassSource::ParseError(_) => None,
        }
    }

    fn class_parse_error(&self, class_name: &str) -> Option<&str> {
        let class_id = self.class_id_for_name(class_name)?;
        match &self.class_record(class_id)?.source {
            ClassSource::ParseError(err) => Some(err.as_str()),
            ClassSource::PendingBytes(_)
            | ClassSource::PendingJarEntry(_)
            | ClassSource::Ready(_) => None,
        }
    }

    #[cfg(test)]
    fn class_source_kind(&self, class_name: &str) -> Option<ClassSourceKind> {
        let class_id = self.class_id_for_name(class_name)?;
        match &self.class_record(class_id)?.source {
            ClassSource::PendingBytes(_) => Some(ClassSourceKind::PendingBytes),
            ClassSource::PendingJarEntry(_) => Some(ClassSourceKind::PendingJarEntry),
            ClassSource::Ready(_) => Some(ClassSourceKind::Ready),
            ClassSource::ParseError(_) => Some(ClassSourceKind::ParseError),
        }
    }

    fn lifecycle_for_source(source: &ClassSource) -> ClassLifecycleState {
        match source {
            ClassSource::PendingBytes(_) | ClassSource::PendingJarEntry(_) => {
                ClassLifecycleState::Loading
            }
            ClassSource::Ready(_) => ClassLifecycleState::Loaded,
            ClassSource::ParseError(_) => ClassLifecycleState::Erroneous,
        }
    }

    fn class_is_loaded(&self, class_id: ClassId) -> bool {
        matches!(
            self.class_record(class_id).map(|record| &record.source),
            Some(ClassSource::Ready(_))
        )
    }

    fn register_class_source(
        &mut self,
        defining_loader: LoaderId,
        binary_name: String,
        source: ClassSource,
        replace_existing: bool,
    ) -> ClassId {
        if let Some(class_id) = self.lookup_class_id(defining_loader, &binary_name) {
            if !replace_existing {
                return class_id;
            }
            self.invalidate_class_caches(&binary_name);
            let lifecycle = Self::lifecycle_for_source(&source);
            if let Some(record) = self.class_record_mut(class_id) {
                record.source = source;
                record.lifecycle = lifecycle;
                record.runtime_metadata = None;
                record.init_prerequisites = None;
                record.terminal_error = None;
            }
            return class_id;
        }

        self.invalidate_class_caches(&binary_name);
        let class_id = ClassId(self.class_records.len());
        self.class_records.push(ClassRecord {
            defining_loader,
            binary_name: binary_name.clone(),
            lifecycle: Self::lifecycle_for_source(&source),
            source,
            runtime_metadata: None,
            init_prerequisites: None,
            terminal_error: None,
        });
        self.class_identity_index
            .entry(defining_loader)
            .or_default()
            .insert(binary_name, class_id);
        class_id
    }

    fn invalidate_resolution_caches(&mut self) {
        self.method_owner_cache.clear();
        self.method_signature_cache.clear();
        self.methodref_constant_cache.clear();
        self.classref_constant_cache.clear();
        self.static_field_owner_cache.clear();
        self.fieldref_constant_cache.clear();
        self.method_exec_info_cache.clear();
        self.instanceof_cache.clear();
        self.instance_field_slot_cache.clear();
    }

    fn invalidate_class_caches(&mut self, name: &str) {
        self.invalidate_resolution_caches();
        if let Some(class_id) = self.class_id_for_name(name) {
            self.static_fields.remove(&class_id);
            self.concrete_interface_method_cache.remove(&class_id);
            self.instance_field_capacity_cache.remove(&class_id);
            self.instance_field_layout_cache.remove(&class_id);
            self.class_init_superinterface_cache.remove(&class_id);
            self.reflection_fields_cache.remove(&class_id);
            self.reflection_methods_cache.remove(&class_id);
            self.reflection_ctors_cache.remove(&class_id);
        }
    }

    fn read_jar_entry(&mut self, entry: &JarEntryRef) -> Result<Vec<u8>, String> {
        let started = self.profiler.as_ref().map(|_| std::time::Instant::now());
        let buf = {
            let archive = self
                .jar_archives
                .get_mut(entry.jar_id)
                .ok_or_else(|| format!("Missing JAR backing store for {}", entry.entry_name))?;
            let mut file = archive
                .by_index(entry.entry_index)
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
            buf
        };
        if let Some(started) = started {
            self.record_profile_phase_sample("jar.read_entry", started.elapsed());
        }
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
                None => {
                    return Err(
                        "java/lang/IllegalMonitorStateException: monitor not entered".to_owned(),
                    )
                }
            };
            if monitor.owner != Some(thread_id) {
                return Err(
                    "java/lang/IllegalMonitorStateException: current thread is not owner"
                        .to_owned(),
                );
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
                if matches!(t.state, ThreadState::WaitingOnCondition(_))
                    && t.saved_monitor_count > 0
                {
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
                None => {
                    return Err(
                        "java/lang/IllegalMonitorStateException: object not locked".to_owned()
                    )
                }
            };
            if monitor.owner != Some(thread_id) {
                return Err(
                    "java/lang/IllegalMonitorStateException: current thread is not owner"
                        .to_owned(),
                );
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
            None => {
                return Err("java/lang/IllegalMonitorStateException: object not locked".to_owned())
            }
        };
        if monitor.owner != Some(thread_id) {
            return Err(
                "java/lang/IllegalMonitorStateException: current thread is not owner".to_owned(),
            );
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
            None => {
                return Err("java/lang/IllegalMonitorStateException: object not locked".to_owned())
            }
        };
        if monitor.owner != Some(thread_id) {
            return Err(
                "java/lang/IllegalMonitorStateException: current thread is not owner".to_owned(),
            );
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
                    let restore_count =
                        self.scheduler
                            .thread(waiting_id)
                            .and_then(|t| match t.state {
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
    pub(in crate::interpreter) fn init_vm_throwable(
        &mut self,
        exc: &JRef,
        detail_message: Option<JRef>,
    ) {
        let stack_trace = JObject::new_array("[Ljava/lang/StackTraceElement;", vec![]);
        self.set_object_field_value(exc, "detailMessage", JValue::Ref(detail_message));
        self.set_object_field_value(exc, "cause", JValue::Ref(Some(Rc::clone(exc))));
        self.set_object_field_value(exc, "stackTrace", JValue::Ref(Some(stack_trace)));
        self.set_object_field_value(exc, "suppressedExceptions", JValue::Ref(None));
    }

    pub(in crate::interpreter) fn new_vm_exception(
        &mut self,
        class_name: &str,
        detail_message: Option<JRef>,
    ) -> JRef {
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
    pub(in crate::interpreter) fn thread_start(
        &mut self,
        thread_obj: JRef,
    ) -> Result<ThreadId, String> {
        // Reject double-start: check if a ThreadContext already exists for this object.
        if self.find_thread_id_by_object(&thread_obj).is_some() {
            return Err("java/lang/IllegalThreadStateException: thread already started".to_owned());
        }

        let id = self.scheduler.spawn(Some(Rc::clone(&thread_obj)));

        // Build a frame for `run()V` on the Thread object.
        let class_name = thread_obj.borrow().class_name.clone();
        let fi =
            self.build_virtual_frame_inner(thread_obj, &class_name, "run", "()V", vec![], false)?;
        match fi {
            Some(frame_info) => {
                self.scheduler
                    .thread_mut(id)
                    .unwrap()
                    .call_stack
                    .push(frame_info);
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
        let thread_name = self.intern_string("main");
        self.set_object_field_value(&obj, "tid", JValue::Int(0));
        self.set_object_field_value(&obj, "name", JValue::Ref(Some(thread_name)));
        self.set_object_field_value(&obj, "priority", JValue::Int(5));
        self.set_object_field_value(&obj, "daemon", JValue::Int(0));
        self.scheduler.current_thread_mut().thread_object = Some(Rc::clone(&obj));
        obj
    }

    /// Find the thread ID associated with a java.lang.Thread object.
    pub(in crate::interpreter) fn find_thread_id_by_object(
        &self,
        thread_obj: &JRef,
    ) -> Option<ThreadId> {
        self.scheduler.find_thread_id_by_object(thread_obj)
    }

    /// Check if a thread (identified by its java.lang.Thread object) is alive.
    pub(in crate::interpreter) fn thread_is_alive(&self, thread_obj: &JRef) -> bool {
        if let Some(id) = self.find_thread_id_by_object(thread_obj) {
            self.scheduler
                .thread(id)
                .map(|t| t.state != ThreadState::Terminated)
                .unwrap_or(false)
        } else {
            false
        }
    }

    /// Register a pre-parsed class file (always stored as `Ready`).
    pub fn load_class(&mut self, class_file: ClassFile) {
        let name = class_file
            .constant_pool
            .class_name(class_file.this_class)
            .to_owned();
        let seeded = self.extract_constant_value_fields(&class_file);
        let class_id = self.register_class_source(
            VM_DEFAULT_LOADER_ID,
            name,
            ClassSource::Ready(class_file),
            true,
        );
        self.extend_static_field_values(class_id, seeded);
    }

    fn constant_value_as_jvalue(
        &mut self,
        constant_pool: &class_file::ConstantPool,
        descriptor: &str,
        constant_index: u16,
    ) -> Option<JValue> {
        match constant_pool.get(constant_index) {
            ConstantPoolEntry::Integer(v) => Some(JValue::Int(*v)),
            ConstantPoolEntry::Float(v) => Some(JValue::Float(*v)),
            ConstantPoolEntry::Long(v) => Some(JValue::Long(*v)),
            ConstantPoolEntry::Double(v) => Some(JValue::Double(*v)),
            ConstantPoolEntry::String { string_index } if descriptor == "Ljava/lang/String;" => {
                Some(JValue::Ref(Some(
                    self.intern_string(constant_pool.utf8(*string_index)),
                )))
            }
            _ => None,
        }
    }

    fn extract_constant_value_fields(&mut self, class_file: &ClassFile) -> HashMap<String, JValue> {
        let mut seeded = HashMap::default();
        for field in &class_file.fields {
            if field.access_flags & 0x0008 == 0 {
                continue;
            }
            let constant_index = field.attributes.iter().find_map(|attr| match attr {
                Attribute::ConstantValue {
                    constantvalue_index,
                } => Some(*constantvalue_index),
                _ => None,
            });
            let Some(constant_index) = constant_index else {
                continue;
            };
            let field_name = class_file.constant_pool.utf8(field.name_index).to_owned();
            let descriptor = class_file.constant_pool.utf8(field.descriptor_index);
            let Some(value) = self.constant_value_as_jvalue(
                &class_file.constant_pool,
                descriptor,
                constant_index,
            ) else {
                continue;
            };
            seeded.insert(field_name, value);
        }
        seeded
    }

    /// Register raw `.class` bytes for lazy parsing.
    /// The class is parsed only when first accessed via [`Self::ensure_class_loaded_by_name`].
    /// If the class is already registered (e.g., as `Ready`), the existing entry is kept.
    pub fn load_lazy(&mut self, name: String, bytes: Vec<u8>) {
        self.register_class_source(
            VM_DEFAULT_LOADER_ID,
            name,
            ClassSource::PendingBytes(bytes),
            false,
        );
    }

    fn load_lazy_jar_entry(&mut self, name: String, entry: JarEntryRef) {
        self.register_class_source(
            VM_DEFAULT_LOADER_ID,
            name,
            ClassSource::PendingJarEntry(entry),
            false,
        );
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
        let mut archive =
            zip::ZipArchive::new(reader).map_err(|e| format!("Invalid JAR/ZIP: {e}"))?;
        let jar_id = self.jar_archives.len();
        let mut class_entries = Vec::new();
        let mut resource_entries = Vec::new();
        let mut count = 0;
        for i in 0..archive.len() {
            let file = archive
                .by_index(i)
                .map_err(|e| format!("ZIP entry error: {e}"))?;
            let name = file.name().to_owned();
            let entry = JarEntryRef {
                jar_id,
                entry_index: i,
                entry_name: name.clone(),
            };
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
            self.pending_resources.insert(name, entry);
        }
        Ok(count)
    }

    fn ensure_class_loaded(&mut self, class_id: ClassId) -> Result<(), String> {
        self.increment_profile_counter("class.ensure_loaded");
        let pending = match self.class_record(class_id).map(|record| &record.source) {
            Some(ClassSource::PendingBytes(bytes)) => Some(ClassSource::PendingBytes(bytes.clone())),
            Some(ClassSource::PendingJarEntry(entry)) => {
                Some(ClassSource::PendingJarEntry(entry.clone()))
            }
            Some(ClassSource::Ready(_)) => return Ok(()),
            Some(ClassSource::ParseError(err)) => return Err(err.clone()),
            None => return Err(format!("missing class record {}", class_id.0)),
        };

        let binary_name = self
            .class_record(class_id)
            .map(|record| record.binary_name.clone())
            .unwrap_or_default();
        let started = self.profiler.as_ref().map(|_| std::time::Instant::now());
        let result = match pending.expect("pending source") {
            ClassSource::PendingBytes(bytes) => {
                let parse_started = self.profiler.as_ref().map(|_| std::time::Instant::now());
                let parsed = class_file::parse(&bytes).map_err(|e| e.to_string());
                if let Some(parse_started) = parse_started {
                    self.record_profile_phase_sample("class.parse", parse_started.elapsed());
                }
                parsed
            }
            ClassSource::PendingJarEntry(entry) => match self.read_jar_entry(&entry) {
                Ok(bytes) => {
                    let parse_started = self.profiler.as_ref().map(|_| std::time::Instant::now());
                    let parsed = class_file::parse(&bytes);
                    if let Some(parse_started) = parse_started {
                        self.record_profile_phase_sample("class.parse", parse_started.elapsed());
                    }
                    match parsed {
                        Ok(cf) => {
                            let actual_name = cf.constant_pool.class_name(cf.this_class);
                            if actual_name == binary_name {
                                Ok(cf)
                            } else {
                                Err(format!(
                                    "Class name mismatch for {}: expected {}, found {}",
                                    entry.entry_name, binary_name, actual_name
                                ))
                            }
                        }
                        Err(e) => Err(e.to_string()),
                    }
                }
                Err(e) => Err(e),
            },
            ClassSource::Ready(_) | ClassSource::ParseError(_) => unreachable!(),
        };
        match result {
            Ok(cf) => {
                let seeded = self.extract_constant_value_fields(&cf);
                if let Some(record) = self.class_record_mut(class_id) {
                    record.source = ClassSource::Ready(cf);
                    record.lifecycle = ClassLifecycleState::Loaded;
                    record.terminal_error = None;
                }
                self.extend_static_field_values(class_id, seeded);
                self.increment_profile_counter("class.load.transition");
                if let Some(started) = started {
                    self.record_profile_phase_sample("class.ensure_loaded", started.elapsed());
                }
                Ok(())
            }
            Err(err) => {
                eprintln!("Warning: failed to parse class '{binary_name}': {err}");
                if let Some(record) = self.class_record_mut(class_id) {
                    record.source = ClassSource::ParseError(err.clone());
                    record.lifecycle = ClassLifecycleState::Erroneous;
                    record.terminal_error = Some(ClassTerminalError::Parse);
                }
                self.increment_profile_counter("class.load.error");
                if let Some(started) = started {
                    self.record_profile_phase_sample("class.ensure_loaded", started.elapsed());
                }
                Err(err)
            }
        }
    }

    pub(in crate::interpreter) fn ensure_class_loaded_by_name(
        &mut self,
        name: &str,
    ) -> Result<Option<ClassId>, String> {
        let Some(class_id) = self.tracked_class_id_for_name(name) else {
            return Ok(None);
        };
        self.ensure_class_loaded(class_id)?;
        Ok(Some(class_id))
    }

    fn ensure_class_prepared(&mut self, class_id: ClassId) -> Result<(), String> {
        self.increment_profile_counter("class.ensure_prepared");
        self.ensure_class_loaded(class_id)?;
        let already_prepared = self
            .class_record(class_id)
            .map(|record| {
                record.runtime_metadata.is_some()
                    || matches!(
                        record.lifecycle,
                        ClassLifecycleState::Prepared
                            | ClassLifecycleState::Initializing { .. }
                            | ClassLifecycleState::Initialized
                    )
                    || (matches!(record.lifecycle, ClassLifecycleState::Erroneous)
                        && matches!(record.terminal_error, Some(ClassTerminalError::Initialization)))
            })
            .unwrap_or(false);
        if already_prepared {
            self.increment_profile_counter("class.prepare.hit");
            return Ok(());
        }

        let started = self.profiler.as_ref().map(|_| std::time::Instant::now());
        let metadata = self
            .build_class_runtime_metadata(class_id)
            .ok_or_else(|| format!("missing class metadata for {class_id:?}"))?;
        if let Some(record) = self.class_record_mut(class_id) {
            record.runtime_metadata = Some(metadata);
            if !matches!(
                record.lifecycle,
                ClassLifecycleState::Initializing { .. }
                    | ClassLifecycleState::Initialized
                    | ClassLifecycleState::Erroneous
            ) {
                record.lifecycle = ClassLifecycleState::Prepared;
            }
        }
        self.increment_profile_counter("class.prepare.transition");
        if let Some(started) = started {
            self.record_profile_phase_sample("class.ensure_prepared", started.elapsed());
        }
        Ok(())
    }

    /// Backwards-compatible wrapper for paths that only needed lazy parsing before.
    pub(in crate::interpreter) fn ensure_class_ready(&mut self, name: &str) {
        let Ok(Some(class_id)) = self.ensure_class_loaded_by_name(name) else {
            return;
        };
        let _ = self.ensure_class_prepared(class_id);
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

    /// Return a reference to a parsed class.
    /// Caller must have called `ensure_class_loaded_by_name` first (or know the class is loaded).
    pub(in crate::interpreter) fn get_class(&self, name: &str) -> Option<&ClassFile> {
        self.class_id_for_name(name).and_then(|class_id| self.parsed_class(class_id))
    }

    /// Ensure class is ready and return a reference to it.
    pub(in crate::interpreter) fn resolve_class(&mut self, name: &str) -> Option<&ClassFile> {
        match self.ensure_class_loaded_by_name(name) {
            Ok(Some(_)) => {}
            Ok(None) | Err(_) => return None,
        }
        self.get_class(name)
    }

    fn build_class_runtime_metadata(&mut self, class_id: ClassId) -> Option<Rc<ClassRuntimeMetadata>> {
        let started = self.profiler.as_ref().map(|_| std::time::Instant::now());
        let (
            class_name_out,
            access_flags,
            super_name,
            interface_names,
            cp,
            bootstrap_methods,
            methods,
        ) = {
            let class = self.parsed_class(class_id)?;
            let class_name_out = class.constant_pool.class_name(class.this_class).to_owned();
            let super_name = if class.super_class != 0 {
                Some(class.constant_pool.class_name(class.super_class).to_owned())
            } else {
                None
            };
            let interface_names = Rc::new(
                class.interfaces
                    .iter()
                    .map(|&idx| class.constant_pool.class_name(idx).to_owned())
                    .collect::<Vec<_>>(),
            );
            let cp = Rc::clone(&class.constant_pool.entries);
            let bootstrap_methods = class
                .attributes
                .iter()
                .find_map(|a| {
                    if let Attribute::BootstrapMethods(bms) = a {
                        Some(Rc::new(bms.clone()))
                    } else {
                        None
                    }
                })
                .unwrap_or_else(|| Rc::new(Vec::new()));
            let methods = class
                .methods
                .iter()
                .map(|method| {
                    let name = class.constant_pool.utf8(method.name_index).to_owned();
                    let descriptor = class.constant_pool.utf8(method.descriptor_index).to_owned();
                    let access_flags = method.access_flags;
                    let (max_stack, max_locals, has_code, code, exception_table) =
                        if let Some(ca) = method.code() {
                            (
                                ca.max_stack as usize,
                                ca.max_locals as usize,
                                true,
                                Rc::new(ca.code.clone()),
                                Rc::new(ca.exception_table.clone()),
                            )
                        } else {
                            (0, 0, false, Rc::new(Vec::new()), Rc::new(Vec::new()))
                        };
                    (name, descriptor, access_flags, max_stack, max_locals, has_code, code, exception_table)
                })
                .collect::<Vec<_>>();
            (
                class_name_out,
                class.access_flags,
                super_name,
                interface_names,
                cp,
                bootstrap_methods,
                methods,
            )
        };

        let mut declared_methods = HashMap::default();
        let mut declared_methods_by_name: HashMap<String, Vec<Rc<MethodExecInfo>>> =
            HashMap::default();
        let mut class_initializer = None;
        let super_id = super_name
            .as_deref()
            .and_then(|name| self.class_id_for_name(name));
        let interface_ids = Rc::new(
            interface_names
                .iter()
                .filter_map(|name| self.class_id_for_name(name))
                .collect::<Vec<_>>(),
        );

        for (
            method_name,
            descriptor,
            method_access_flags,
            max_stack,
            max_locals,
            has_code,
            code,
            exception_table,
        ) in methods
        {
            let (param_tokens, _) = Self::parse_method_descriptor_tokens(&descriptor);
            let parameter_descriptor =
                Rc::<str>::from(descriptor.split(')').next().unwrap_or("(").to_owned());
            let param_slot_steps: Vec<usize> = param_tokens
                .iter()
                .map(|t| if t == "J" || t == "D" { 2 } else { 1 })
                .collect();
            let param_slot_count = param_slot_steps.iter().sum();
            let info = Rc::new(MethodExecInfo {
                class_id,
                class_name: class_name_out.clone(),
                descriptor: descriptor.clone(),
                parameter_descriptor,
                arg_count: param_tokens.len(),
                is_varargs: method_access_flags & 0x0080 != 0,
                max_stack,
                access_flags: method_access_flags,
                max_locals,
                has_code,
                code,
                exception_table,
                cp: Rc::clone(&cp),
                bootstrap_methods: Rc::clone(&bootstrap_methods),
                param_tokens: Rc::new(param_tokens),
                param_slot_steps: Rc::new(param_slot_steps),
                param_slot_count,
                frame_owner: Rc::<str>::from(format!(
                    "{class_name_out}.{method_name}{descriptor}"
                )),
            });
            if method_name == "<clinit>" && descriptor == "()V" {
                class_initializer = Some(info.clone());
            }
            declared_methods.insert((method_name.clone(), descriptor), info.clone());
            declared_methods_by_name
                .entry(method_name)
                .or_default()
                .push(info);
        }

        let metadata = Rc::new(ClassRuntimeMetadata {
            access_flags,
            super_name,
            super_id,
            interface_names,
            interface_ids,
            declared_methods,
            declared_methods_by_name,
            class_initializer,
        });
        if let Some(started) = started {
            self.record_profile_phase_sample("class.runtime_metadata", started.elapsed());
        }
        Some(metadata)
    }

    fn class_runtime_metadata_by_id(
        &mut self,
        class_id: ClassId,
    ) -> Option<Rc<ClassRuntimeMetadata>> {
        self.ensure_class_prepared(class_id).ok()?;
        self.class_record(class_id)
            .and_then(|record| record.runtime_metadata.clone())
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

    pub(in crate::interpreter) fn increment_profile_counter(&mut self, counter_name: &'static str) {
        if let Some(profiler) = self.profiler.as_mut() {
            profiler.increment_counter(counter_name);
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

    pub fn enable_profiler(&mut self) {
        if self.profiler.is_none() {
            self.profiler = Some(VmProfiler::new(20));
        }
    }

    pub fn reset_profiler(&mut self) {
        if let Some(profiler) = self.profiler.as_mut() {
            profiler.clear();
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

    pub(in crate::interpreter) fn record_class_init_sample(
        &mut self,
        class_name: &str,
        elapsed: std::time::Duration,
    ) {
        if let Some(profiler) = self.profiler.as_mut() {
            profiler.record_class_init(class_name, elapsed);
        }
    }

    pub(in crate::interpreter) fn record_profile_phase_sample(
        &mut self,
        phase_name: &'static str,
        elapsed: std::time::Duration,
    ) {
        if let Some(profiler) = self.profiler.as_mut() {
            profiler.record_phase(phase_name, elapsed);
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
                    b.fields
                        .get("cause")
                        .and_then(|v| v.as_ref())
                        .filter(|cause| !Rc::ptr_eq(cause, r))
                        .cloned()
                        .or_else(|| {
                            b.fields
                                .get("target")
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
        self.scheduler
            .current_thread()
            .pending_exception
            .as_ref()
            .map(|r| self.format_exception_ref(r))
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
        let exc =
            self.new_vm_exception_message("java/lang/NoClassDefFoundError", name.replace('/', "."));
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
        self.set_object_field_value(&exc, "detailMessage", JValue::Ref(Some(msg)));
        *self.pending_exception_mut() = Some(exc);
    }

    /// Set `pending_exception` to a `ClassFormatError` carrying the parse error message.
    /// Used when a class record is marked with a parse failure.
    pub(in crate::interpreter) fn throw_class_format_error(&mut self, parse_msg: &str) {
        let exc = self.new_vm_exception_message("java/lang/ClassFormatError", parse_msg);
        *self.pending_exception_mut() = Some(exc);
    }

    fn class_object_by_id(&mut self, class_id: ClassId) -> JRef {
        if let Some(r) = self.class_mirror_pool.get(&class_id).cloned() {
            self.increment_profile_counter("class.mirror.pool.hit");
            return r;
        }
        let internal_name = self.class_name(class_id).unwrap_or_default().to_owned();
        if let Some(r) = self.class_pool.get(&internal_name).cloned() {
            self.increment_profile_counter("class.mirror.pool.memoize");
            r.borrow_mut().represented_class_id = Some(class_id);
            self.class_mirror_pool.insert(class_id, Rc::clone(&r));
            return r;
        }
        let obj = JObject::new("java/lang/Class");
        obj.borrow_mut().represented_class_id = Some(class_id);
        let internal_name_ref = self.intern_string(internal_name.clone());
        self.set_object_field_value(
            &obj,
            "__name_internal",
            JValue::Ref(Some(internal_name_ref)),
        );
        self.class_mirror_pool.insert(class_id, Rc::clone(&obj));
        self.class_pool.insert(internal_name, Rc::clone(&obj));
        self.increment_profile_counter("class.mirror.pool.create");
        obj
    }

    fn class_object(&mut self, internal_name: impl Into<String>) -> JRef {
        let internal_name = internal_name.into();
        if let Some(class_id) = self.tracked_class_id_for_name(&internal_name) {
            return self.class_object_by_id(class_id);
        }
        if let Some(r) = self.class_pool.get(&internal_name) {
            return Rc::clone(r);
        }
        let obj = JObject::new("java/lang/Class");
        let internal_name_ref = self.intern_string(internal_name.clone());
        self.set_object_field_value(
            &obj,
            "__name_internal",
            JValue::Ref(Some(internal_name_ref)),
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
        let class_id = self.tracked_class_id_for_name(class_name)?;
        self.find_method_flags_id(class_id, method_name, descriptor)
    }

    fn find_method_flags_id(
        &mut self,
        class_id: ClassId,
        method_name: &str,
        descriptor: &str,
    ) -> Option<u16> {
        let metadata = self.class_runtime_metadata_by_id(class_id)?;
        if let Some(candidates) = metadata.declared_methods_by_name.get(method_name) {
            for info in candidates {
                if info.descriptor == descriptor {
                    return Some(info.access_flags);
                }
            }
        }
        if let Some(super_id) = metadata.super_id {
            if let Some(f) = self.find_method_flags_id(super_id, method_name, descriptor) {
                return Some(f);
            }
        }
        for &iface_id in metadata.interface_ids.iter() {
            if let Some(f) = self.find_method_flags_id(iface_id, method_name, descriptor) {
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
        let class_id = self.tracked_class_id_for_name(class_name)?;
        let cache_key = (
            class_id,
            method_name.to_owned(),
            descriptor.to_owned(),
        );
        if let Some(cached) = self.method_exec_info_cache.get(&cache_key) {
            return Some(cached.clone());
        }
        let owner_id = self.find_method_owner_id(class_id, method_name, descriptor)?;
        let metadata = self.class_runtime_metadata_by_id(owner_id)?;
        let info = metadata
            .declared_methods
            .get(&(method_name.to_owned(), descriptor.to_owned()))?
            .clone();
        let info = info.as_ref().clone();
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
        let class_id = self.tracked_class_id_for_name(class_name)?;
        self.find_method_owner_id(class_id, method_name, descriptor)
            .and_then(|owner_id| self.class_name(owner_id).map(str::to_owned))
    }

    fn find_method_owner_id(
        &mut self,
        class_id: ClassId,
        method_name: &str,
        descriptor: &str,
    ) -> Option<ClassId> {
        let cache_key = (
            class_id,
            method_name.to_owned(),
            descriptor.to_owned(),
        );
        if let Some(cached) = self.method_owner_cache.get(&cache_key) {
            return *cached;
        }
        let metadata = self.class_runtime_metadata_by_id(class_id)?;
        let mut result = None;
        if let Some(candidates) = metadata.declared_methods_by_name.get(method_name) {
            if candidates.iter().any(|info| info.descriptor == descriptor) {
                result = Some(class_id);
            }
        }
        if result.is_none() {
            if let Some(super_id) = metadata.super_id {
                result = self.find_method_owner_id(super_id, method_name, descriptor);
            }
        }
        if result.is_none() {
            for &iface_id in metadata.interface_ids.iter() {
                if let Some(owner) = self.find_method_owner_id(iface_id, method_name, descriptor) {
                    result = Some(owner);
                    break;
                }
            }
        }
        // Only cache positive results — negative lookups (None) may become valid
        // after new classes are registered via load_lazy/load_class.
        if result.is_some() {
            self.method_owner_cache.insert(cache_key, result.clone());
        }
        result
    }

    /// Returns `true` if the named method exists in the class hierarchy.
    /// Used to check method existence before dispatch without borrowing ClassFile data.
    pub(in crate::interpreter) fn method_exists(
        &mut self,
        class_name: &str,
        method_name: &str,
        descriptor: &str,
    ) -> bool {
        self.find_method_owner(class_name, method_name, descriptor)
            .is_some()
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
        let class_id = self.tracked_class_id_for_name(class_name)?;
        let cache_key = (
            class_id,
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
        self.method_signature_cache
            .insert(cache_key, result.clone());
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
        let class_id = self.tracked_class_id_for_name(class_name)?;
        self.find_method_real_descriptor_id(class_id, method_name, descriptor)
    }

    fn find_method_real_descriptor_id(
        &mut self,
        class_id: ClassId,
        method_name: &str,
        descriptor: &str,
    ) -> Option<String> {
        let param_part = descriptor.split(')').next().unwrap_or("(");
        let arg_count = count_args(descriptor);
        let metadata = self.class_runtime_metadata_by_id(class_id)?;
        let mut arg_count_match: Option<String> = None;
        let mut varargs_match: Option<String> = None;
        let candidates = metadata.declared_methods_by_name.get(method_name);
        for info in candidates.into_iter().flatten() {
            if info.parameter_descriptor.as_ref() == param_part {
                return Some(info.descriptor.clone());
            }
            if arg_count_match.is_none() && info.arg_count == arg_count {
                arg_count_match = Some(info.descriptor.clone());
            }
            if varargs_match.is_none() && info.is_varargs {
                let fixed = info.arg_count.saturating_sub(1);
                if arg_count >= fixed {
                    varargs_match = Some(info.descriptor.clone());
                }
            }
        }
        if arg_count_match.is_some() {
            return arg_count_match;
        }
        if varargs_match.is_some() {
            return varargs_match;
        }
        if let Some(super_id) = metadata.super_id {
            if let Some(result) =
                self.find_method_real_descriptor_id(super_id, method_name, descriptor)
            {
                return Some(result);
            }
        }
        for &iface_id in metadata.interface_ids.iter() {
            if let Some(result) =
                self.find_method_real_descriptor_id(iface_id, method_name, descriptor)
            {
                return Some(result);
            }
        }
        None
    }

    // ------------------------------------------------------------------

    pub(super) fn instance_field_capacity(&mut self, class_name: &str) -> usize {
        let Some(class_id) = self.tracked_class_id_for_name(class_name) else {
            return 0;
        };
        if let Some(cached) = self.instance_field_capacity_cache.get(&class_id) {
            return *cached;
        }
        let capacity = self.instance_field_layout_id(class_id).default_values.len();
        self.instance_field_capacity_cache
            .insert(class_id, capacity);
        capacity
    }

    fn instance_field_layout(&mut self, class_name: &str) -> Rc<InstanceFieldLayout> {
        let Some(class_id) = self.tracked_class_id_for_name(class_name) else {
            return Self::empty_instance_field_layout();
        };
        self.instance_field_layout_id(class_id)
    }

    fn empty_instance_field_layout() -> Rc<InstanceFieldLayout> {
        Rc::new(InstanceFieldLayout {
            default_values: Rc::new(Vec::new()),
            slot_lookup: HashMap::default(),
            native_name_slots: HashMap::default(),
            native_name_counts: HashMap::default(),
        })
    }

    fn instance_field_layout_id(&mut self, class_id: ClassId) -> Rc<InstanceFieldLayout> {
        if let Some(cached) = self.instance_field_layout_cache.get(&class_id) {
            return cached.clone();
        }
        if self.ensure_class_prepared(class_id).is_err() {
            return Self::empty_instance_field_layout();
        }
        let super_id = self
            .class_runtime_metadata_by_id(class_id)
            .and_then(|metadata| metadata.super_id);
        let own_fields = self
            .parsed_class(class_id)
            .map(|class| {
                class
                    .fields
                    .iter()
                    .filter(|field| field.access_flags & 0x0008 == 0)
                    .map(|field| {
                        (
                            class.constant_pool.utf8(field.name_index).to_owned(),
                            class.constant_pool.utf8(field.descriptor_index).to_owned(),
                        )
                    })
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();

        let mut default_values = Vec::new();
        let mut slot_lookup = HashMap::default();
        let mut native_name_slots = HashMap::default();
        let mut native_name_counts = HashMap::default();

        if let Some(super_id) = super_id {
            let super_layout = self.instance_field_layout_id(super_id);
            default_values.extend(super_layout.default_values.iter().cloned());
            slot_lookup = super_layout.slot_lookup.clone();
            native_name_slots = super_layout.native_name_slots.clone();
            native_name_counts = super_layout.native_name_counts.clone();
        }

        for (field_name, descriptor) in own_fields {
            let slot = default_values.len();
            default_values.push(default_value_for_descriptor(&descriptor));
            slot_lookup.insert(
                (
                    class_id,
                    field_name.clone(),
                    descriptor.clone(),
                ),
                slot,
            );
            let count = native_name_counts.entry(field_name.clone()).or_insert(0);
            *count += 1;
            if *count == 1 {
                native_name_slots.insert(field_name, slot);
            } else {
                native_name_slots.remove(&field_name);
            }
        }

        let layout = Rc::new(InstanceFieldLayout {
            default_values: Rc::new(default_values),
            slot_lookup,
            native_name_slots,
            native_name_counts,
        });
        self.instance_field_layout_cache
            .insert(class_id, layout.clone());
        layout
    }

    fn resolve_instance_field_slot(
        &mut self,
        class_name: &str,
        field_name: &str,
        descriptor: &str,
    ) -> Option<usize> {
        let class_id = self.tracked_class_id_for_name(class_name)?;
        self.resolve_instance_field_slot_id(class_id, field_name, descriptor)
    }

    fn resolve_instance_field_slot_id(
        &mut self,
        class_id: ClassId,
        field_name: &str,
        descriptor: &str,
    ) -> Option<usize> {
        let cache_key = (
            class_id,
            field_name.to_owned(),
            descriptor.to_owned(),
        );
        if let Some(cached) = self.instance_field_slot_cache.get(&cache_key) {
            return *cached;
        }
        if self.ensure_class_prepared(class_id).is_err() {
            return None;
        }
        let declares_here = self
            .parsed_class(class_id)
            .map(|class| {
                class.fields.iter().any(|field| {
                    (field.access_flags & 0x0008) == 0
                        && class.constant_pool.utf8(field.name_index) == field_name
                        && class.constant_pool.utf8(field.descriptor_index) == descriptor
                })
            })
            .unwrap_or(false);
        let super_id = self
            .class_runtime_metadata_by_id(class_id)
            .and_then(|metadata| metadata.super_id);
        let resolved = if declares_here {
            self.instance_field_layout_id(class_id)
                .slot_lookup
                .get(&cache_key)
                .copied()
        } else if let Some(super_id) = super_id {
            self.resolve_instance_field_slot_id(super_id, field_name, descriptor)
        } else {
            None
        };
        self.instance_field_slot_cache.insert(cache_key, resolved);
        resolved
    }

    fn named_instance_field_slot_id(
        &mut self,
        class_id: ClassId,
        field_name: &str,
    ) -> Option<usize> {
        self.instance_field_layout_id(class_id)
            .native_name_slots
            .get(field_name)
            .copied()
    }

    pub(in crate::interpreter) fn get_object_field_value(
        &mut self,
        obj: &JRef,
        field_name: &str,
    ) -> Option<JValue> {
        let borrow = obj.borrow();
        let class_id = borrow.class_id;
        drop(borrow);
        let slot = if let Some(class_id) = class_id {
            self.named_instance_field_slot_id(class_id, field_name)
        } else {
            self.object_runtime_class_id(obj)
                .and_then(|class_id| self.named_instance_field_slot_id(class_id, field_name))
        };
        let borrow = obj.borrow();
        if let Some(slot) = slot {
            if let Some(value) = borrow.field_slots.get(slot).cloned() {
                return Some(value);
            }
        }
        borrow.fields.get(field_name).cloned()
    }

    pub(in crate::interpreter) fn set_object_field_value(
        &mut self,
        obj: &JRef,
        field_name: &str,
        value: JValue,
    ) {
        let borrow = obj.borrow_mut();
        let class_id = borrow.class_id;
        drop(borrow);
        let slot = if let Some(class_id) = class_id {
            self.named_instance_field_slot_id(class_id, field_name)
        } else {
            self.object_runtime_class_id(obj)
                .and_then(|class_id| self.named_instance_field_slot_id(class_id, field_name))
        };
        let mut borrow = obj.borrow_mut();
        if let Some(slot) = slot {
            if let Some(slot_value) = borrow.field_slots.get_mut(slot) {
                *slot_value = value.clone();
            }
        }
        borrow.fields.insert(field_name.to_owned(), value);
    }

    fn declares_concrete_interface_method_id(&mut self, class_id: ClassId) -> bool {
        if let Some(cached) = self.concrete_interface_method_cache.get(&class_id) {
            return *cached;
        }
        let _ = self.ensure_class_prepared(class_id);
        let has_concrete_method = self
            .parsed_class(class_id)
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
            .insert(class_id, has_concrete_method);
        has_concrete_method
    }

    fn collect_class_init_superinterfaces_id(
        &mut self,
        interface_id: ClassId,
        seen: &mut HashSet<ClassId>,
        ordered: &mut Vec<ClassId>,
    ) {
        if !seen.insert(interface_id) {
            return;
        }
        let _ = self.ensure_class_prepared(interface_id);
        let Some(metadata) = self.class_runtime_metadata_by_id(interface_id) else {
            return;
        };
        if (metadata.access_flags & 0x0200) == 0 {
            return;
        }
        let super_ifaces = metadata.interface_ids.clone();
        for &super_iface in super_ifaces.iter() {
            self.collect_class_init_superinterfaces_id(super_iface, seen, ordered);
        }
        if self.declares_concrete_interface_method_id(interface_id) {
            ordered.push(interface_id);
        }
    }

    fn class_init_superinterfaces_id(&mut self, class_id: ClassId) -> Rc<Vec<ClassId>> {
        if let Some(cached) = self.class_init_superinterface_cache.get(&class_id) {
            return cached.clone();
        }
        let _ = self.ensure_class_prepared(class_id);
        let Some(metadata) = self.class_runtime_metadata_by_id(class_id) else {
            return Rc::new(Vec::new());
        };
        if (metadata.access_flags & 0x0200) != 0 {
            return Rc::new(Vec::new());
        }
        let direct_ifaces = metadata.interface_ids.clone();
        let mut seen = HashSet::default();
        let mut ordered = Vec::new();
        for &iface_id in direct_ifaces.iter() {
            self.collect_class_init_superinterfaces_id(iface_id, &mut seen, &mut ordered);
        }
        let ordered = Rc::new(ordered);
        self.class_init_superinterface_cache
            .insert(class_id, ordered.clone());
        ordered
    }

    fn find_static_field_owner_id(
        &mut self,
        class_id: ClassId,
        field_name: &str,
        descriptor: &str,
    ) -> Option<ClassId> {
        let cache_key = (
            class_id,
            field_name.to_owned(),
            descriptor.to_owned(),
        );
        if let Some(cached) = self.static_field_owner_cache.get(&cache_key) {
            return *cached;
        }
        let result = self.find_static_field_owner_uncached_id(class_id, field_name, descriptor);
        if result.is_some() {
            self.static_field_owner_cache
                .insert(cache_key, result.clone());
        }
        result
    }

    fn find_static_field_owner_uncached_id(
        &mut self,
        class_id: ClassId,
        field_name: &str,
        descriptor: &str,
    ) -> Option<ClassId> {
        let _ = self.ensure_class_prepared(class_id);
        let class = self.parsed_class(class_id)?;
        for field in &class.fields {
            let name = class.constant_pool.utf8(field.name_index);
            let desc = class.constant_pool.utf8(field.descriptor_index);
            if name == field_name && desc == descriptor && (field.access_flags & 0x0008) != 0 {
                return Some(class_id);
            }
        }
        let Some(metadata) = self.class_runtime_metadata_by_id(class_id) else {
            return None;
        };
        for &iface_id in metadata.interface_ids.iter() {
            if let Some(owner) = self.find_static_field_owner_id(iface_id, field_name, descriptor) {
                return Some(owner);
            }
        }
        if let Some(super_id) = metadata.super_id {
            if let Some(owner) = self.find_static_field_owner_id(super_id, field_name, descriptor) {
                return Some(owner);
            }
        }
        None
    }

    fn class_lifecycle_state_by_id(&self, class_id: ClassId) -> Option<ClassLifecycleState> {
        self.class_record(class_id)
            .map(|record| record.lifecycle)
    }

    fn is_class_initialized_id(&self, class_id: ClassId) -> bool {
        matches!(
            self.class_lifecycle_state_by_id(class_id),
            Some(ClassLifecycleState::Initialized)
        )
    }

    fn is_class_init_failed_id(&self, class_id: ClassId) -> bool {
        self.class_record(class_id)
            .map(|record| {
                matches!(record.terminal_error, Some(ClassTerminalError::Initialization))
            })
            .unwrap_or(false)
    }

    pub(crate) fn mark_class_init_done_id(&mut self, class_id: ClassId) {
        if let Some(record) = self.class_record_mut(class_id) {
            record.lifecycle = ClassLifecycleState::Initialized;
            record.terminal_error = None;
        }
    }

    pub(crate) fn mark_class_init_failed_id(&mut self, class_id: ClassId) {
        if let Some(record) = self.class_record_mut(class_id) {
            record.lifecycle = ClassLifecycleState::Erroneous;
            record.terminal_error = Some(ClassTerminalError::Initialization);
        }
        self.increment_profile_counter("class.init.failure");
    }

    fn current_thread_id(&self) -> ThreadId {
        self.scheduler.current_thread().id
    }

    fn class_init_owner_id(&self, class_id: ClassId) -> Option<ThreadId> {
        match self.class_lifecycle_state_by_id(class_id) {
            Some(ClassLifecycleState::Initializing { owner, .. }) => Some(owner),
            _ => None,
        }
    }

    fn begin_class_init_id(&mut self, class_id: ClassId) {
        let owner = self.current_thread_id();
        if let Some(record) = self.class_record_mut(class_id) {
            record.lifecycle = ClassLifecycleState::Initializing {
                owner,
                stage: ClassInitStage::Planning,
            };
            record.terminal_error = None;
        }
    }

    fn mark_class_init_running_id(&mut self, class_id: ClassId) {
        let owner = self
            .class_init_owner_id(class_id)
            .unwrap_or_else(|| self.current_thread_id());
        if let Some(record) = self.class_record_mut(class_id) {
            record.lifecycle = ClassLifecycleState::Initializing {
                owner,
                stage: ClassInitStage::Running,
            };
        }
    }

    pub(crate) fn push_sync_clinit_frame(&mut self, fi: &trampoline::FrameInfo) {
        if let Some(class_id) = fi.class_initializer_owner {
            self.sync_clinit_stack.push(class_id);
        }
    }

    pub(crate) fn pop_sync_clinit_frame(&mut self, fi: &trampoline::FrameInfo) {
        let Some(class_id) = fi.class_initializer_owner else {
            return;
        };
        if matches!(self.sync_clinit_stack.last(), Some(last) if *last == class_id) {
            self.sync_clinit_stack.pop();
            return;
        }
        if let Some(pos) = self
            .sync_clinit_stack
            .iter()
            .rposition(|active| *active == class_id)
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
        if let Some(class_id) = fi.class_initializer_owner {
            self.scheduler
                .current_thread_mut()
                .active_clinit_stack
                .push(class_id);
        }
    }

    pub(crate) fn pop_thread_clinit_frame(&mut self, fi: &trampoline::FrameInfo) {
        let Some(class_id) = fi.class_initializer_owner else {
            return;
        };
        let stack = &mut self.scheduler.current_thread_mut().active_clinit_stack;
        if matches!(stack.last(), Some(last) if *last == class_id) {
            stack.pop();
            return;
        }
        if let Some(pos) = stack.iter().rposition(|active| *active == class_id) {
            stack.remove(pos);
        }
    }

    pub(crate) fn register_thread_clinit_frames(&mut self, call_stack: &[trampoline::FrameInfo]) {
        for fi in call_stack {
            self.push_thread_clinit_frame(fi);
        }
    }

    pub(crate) fn unregister_thread_clinit_frames(&mut self, call_stack: &[trampoline::FrameInfo]) {
        for fi in call_stack.iter().rev() {
            self.pop_thread_clinit_frame(fi);
        }
    }

    fn is_class_initializer_active_on_current_stack(&self, class_id: ClassId) -> bool {
        if self
            .sync_clinit_stack
            .iter()
            .any(|active| *active == class_id)
        {
            return true;
        }
        self.scheduler
            .current_thread()
            .active_clinit_stack
            .iter()
            .any(|active| *active == class_id)
    }

    fn class_init_prerequisites(&mut self, class_id: ClassId) -> ClassInitPrerequisites {
        if let Some(cached) = self
            .class_record(class_id)
            .and_then(|record| record.init_prerequisites.clone())
        {
            return cached;
        }
        let Some(metadata) = self.class_runtime_metadata_by_id(class_id) else {
            let prerequisites = ClassInitPrerequisites {
                super_id: None,
                interface_ids: Rc::new(Vec::new()),
            };
            if let Some(record) = self.class_record_mut(class_id) {
                record.init_prerequisites = Some(prerequisites.clone());
            }
            return prerequisites;
        };
        if (metadata.access_flags & 0x0200) != 0 {
            let prerequisites = ClassInitPrerequisites {
                super_id: None,
                interface_ids: Rc::new(Vec::new()),
            };
            if let Some(record) = self.class_record_mut(class_id) {
                record.init_prerequisites = Some(prerequisites.clone());
            }
            return prerequisites;
        }
        let super_id = metadata
            .super_id
            .filter(|super_id| self.class_name(*super_id) != Some("java/lang/Object"));
        let prerequisites = ClassInitPrerequisites {
            super_id,
            interface_ids: self.class_init_superinterfaces_id(class_id),
        };
        if let Some(record) = self.class_record_mut(class_id) {
            record.init_prerequisites = Some(prerequisites.clone());
        }
        prerequisites
    }

    fn class_init_depends_on_id(
        &mut self,
        class_id: ClassId,
        prerequisite_id: ClassId,
        visited: &mut HashSet<ClassId>,
    ) -> bool {
        if !visited.insert(class_id) {
            return false;
        }
        let prerequisites = self.class_init_prerequisites(class_id);
        if prerequisites.super_id == Some(prerequisite_id) {
            return true;
        }
        if prerequisites.interface_ids.iter().any(|&iface| iface == prerequisite_id) {
            return true;
        }
        if let Some(super_id) = prerequisites.super_id {
            if self.class_init_depends_on_id(super_id, prerequisite_id, visited) {
                return true;
            }
        }
        for &iface_id in prerequisites.interface_ids.iter() {
            if self.class_init_depends_on_id(iface_id, prerequisite_id, visited) {
                return true;
            }
        }
        false
    }

    fn current_stack_is_in_class_init_prerequisite_of(&mut self, class_id: ClassId) -> bool {
        let mut active = self.sync_clinit_stack.clone();
        active.extend(
            self.scheduler
                .current_thread()
                .active_clinit_stack
                .iter()
                .cloned(),
        );
        active
            .into_iter()
            .any(|active_class| {
                let mut visited = HashSet::default();
                self.class_init_depends_on_id(class_id, active_class, &mut visited)
            })
    }

    fn throw_no_class_def_found_for_failed_init(
        &mut self,
        class_name: &str,
    ) -> Result<ClassInitOutcome, String> {
        self.throw_no_class_def_found(class_name);
        Err(format!("java/lang/NoClassDefFoundError: {class_name}"))
    }

    fn throw_no_class_def_found_for_failed_init_id(
        &mut self,
        class_id: ClassId,
    ) -> Result<ClassInitOutcome, String> {
        let class_name = self.class_name(class_id).unwrap_or_default().to_owned();
        self.throw_no_class_def_found_for_failed_init(&class_name)
    }

    fn wrap_class_init_failure_id(
        &mut self,
        class_id: ClassId,
        err: String,
    ) -> Result<ClassInitOutcome, String> {
        let cause = self.pending_exception_mut().take();
        let eiie =
            self.new_vm_exception_message("java/lang/ExceptionInInitializerError", err.clone());
        if let Some(c) = cause {
            eiie.borrow_mut()
                .fields
                .insert("cause".to_owned(), JValue::Ref(Some(c)));
        }
        *self.pending_exception_mut() = Some(eiie);
        self.mark_class_init_failed_id(class_id);
        Err("java/lang/ExceptionInInitializerError".to_owned())
    }

    fn ensure_class_init_inner(
        &mut self,
        class_name: &str,
        mode: ClassInitMode,
    ) -> Result<ClassInitOutcome, String> {
        let Some(class_id) = self.tracked_class_id_for_name(class_name) else {
            return Ok(ClassInitOutcome::Completed);
        };
        self.ensure_class_init_inner_id(class_id, mode)
    }

    fn ensure_class_init_inner_id(
        &mut self,
        class_id: ClassId,
        mode: ClassInitMode,
    ) -> Result<ClassInitOutcome, String> {
        self.increment_profile_counter("class.init.check");
        let started = self.profiler.as_ref().map(|_| std::time::Instant::now());
        let result = (|| {
            if self.is_class_initialized_id(class_id) {
                return Ok(ClassInitOutcome::Completed);
            }
            if self.is_class_init_failed_id(class_id) {
                return self.throw_no_class_def_found_for_failed_init_id(class_id);
            }
            if self.is_class_initializer_active_on_current_stack(class_id) {
                return Ok(ClassInitOutcome::Completed);
            }
            self.ensure_class_prepared(class_id)?;

            let current_thread_id = self.current_thread_id();
            match self.class_lifecycle_state_by_id(class_id) {
                Some(ClassLifecycleState::Initializing { owner, .. })
                    if owner != current_thread_id =>
                {
                    return match mode {
                        ClassInitMode::Sync => Ok(ClassInitOutcome::Completed),
                        ClassInitMode::Schedule => Ok(ClassInitOutcome::Yield),
                    };
                }
                Some(ClassLifecycleState::Initializing {
                    stage: ClassInitStage::Running,
                    ..
                }) => {
                    if mode == ClassInitMode::Schedule {
                        return Ok(ClassInitOutcome::Yield);
                    }
                }
                Some(ClassLifecycleState::Initialized) => return Ok(ClassInitOutcome::Completed),
                Some(ClassLifecycleState::Erroneous) if self.is_class_init_failed_id(class_id) => {
                    return self.throw_no_class_def_found_for_failed_init_id(class_id);
                }
                _ => {
                    self.increment_profile_counter("class.init.start");
                    self.begin_class_init_id(class_id);
                }
            }

            let prerequisites = self.class_init_prerequisites(class_id);
            if let Some(super_id) = prerequisites.super_id {
                match self.ensure_class_init_inner_id(super_id, mode) {
                    Ok(ClassInitOutcome::Completed) => {
                        if mode == ClassInitMode::Schedule && !self.is_class_initialized_id(super_id)
                        {
                            if self.current_stack_is_in_class_init_prerequisite_of(class_id) {
                                return Ok(ClassInitOutcome::Completed);
                            }
                            return Ok(ClassInitOutcome::Yield);
                        }
                    }
                    Ok(ClassInitOutcome::Yield) => {
                        if self.current_stack_is_in_class_init_prerequisite_of(class_id) {
                            return Ok(ClassInitOutcome::Completed);
                        }
                        return Ok(ClassInitOutcome::Yield);
                    }
                    Err(err) => {
                        self.mark_class_init_failed_id(class_id);
                        return Err(err);
                    }
                }
            }
            for &iface_id in prerequisites.interface_ids.iter() {
                match self.ensure_class_init_inner_id(iface_id, mode) {
                    Ok(ClassInitOutcome::Completed) => {
                        if mode == ClassInitMode::Schedule && !self.is_class_initialized_id(iface_id)
                        {
                            if self.current_stack_is_in_class_init_prerequisite_of(class_id) {
                                return Ok(ClassInitOutcome::Completed);
                            }
                            return Ok(ClassInitOutcome::Yield);
                        }
                    }
                    Ok(ClassInitOutcome::Yield) => {
                        if self.current_stack_is_in_class_init_prerequisite_of(class_id) {
                            return Ok(ClassInitOutcome::Completed);
                        }
                        return Ok(ClassInitOutcome::Yield);
                    }
                    Err(err) => {
                        self.mark_class_init_failed_id(class_id);
                        return Err(err);
                    }
                }
            }

            let class_initializer = self
                .class_runtime_metadata_by_id(class_id)
                .and_then(|metadata| metadata.class_initializer.clone());
            let Some(class_initializer) = class_initializer else {
                self.mark_class_init_done_id(class_id);
                self.increment_profile_counter("class.init.complete");
                return Ok(ClassInitOutcome::Completed);
            };

            self.mark_class_init_running_id(class_id);
            self.increment_profile_counter("class.init.clinit_run");
            match mode {
                ClassInitMode::Sync => {
                    if class_initializer.has_code {
                        let fi = self.build_static_frame_from_info(
                            class_initializer.as_ref(),
                            vec![],
                            false,
                            true,
                        );
                        let frame_owner = fi.frame_owner.to_string();
                        let mut call_stack = vec![fi];
                        if let Err(err) = self
                            .run_trampoline(&mut call_stack)
                            .map(|_| JValue::Void)
                            .map_err(|e| format!("{e}\n  at {frame_owner}"))
                        {
                            return self.wrap_class_init_failure_id(class_id, err);
                        }
                    } else if let Err(err) = self.invoke_static_native_from_info(
                        class_initializer.as_ref(),
                        "<clinit>",
                        vec![],
                    ) {
                        return self.wrap_class_init_failure_id(class_id, err);
                    }
                    self.mark_class_init_done_id(class_id);
                    self.increment_profile_counter("class.init.complete");
                    Ok(ClassInitOutcome::Completed)
                }
                ClassInitMode::Schedule => {
                    if class_initializer.has_code {
                        let fi = self.build_static_frame_from_info(
                            class_initializer.as_ref(),
                            vec![],
                            false,
                            true,
                        );
                        debug_assert!(self.pending_frame_mut().is_none());
                        *self.pending_frame_mut() = Some(fi);
                        self.increment_profile_counter("class.init.yield");
                        Ok(ClassInitOutcome::Yield)
                    } else if let Err(err) = self.invoke_static_native_from_info(
                        class_initializer.as_ref(),
                        "<clinit>",
                        vec![],
                    ) {
                        self.mark_class_init_failed_id(class_id);
                        Err(err)
                    } else {
                        self.mark_class_init_done_id(class_id);
                        self.increment_profile_counter("class.init.complete");
                        Ok(ClassInitOutcome::Completed)
                    }
                }
            }
        })();
        if let Some(started) = started {
            let class_name = self.class_name(class_id).unwrap_or_default().to_owned();
            self.record_class_init_sample(&class_name, started.elapsed());
        }
        result
    }

    /// Run `<clinit>` for a class if it hasn't been initialized yet.
    /// Per JVMS §5.5: Before a class is initialized, its direct superclass must
    /// be initialized first (recursively), and any superinterfaces that declare
    /// concrete non-static methods must also be initialized.
    fn ensure_class_init(&mut self, class_name: &str) -> Result<(), String> {
        let _ = self.ensure_class_init_inner(class_name, ClassInitMode::Sync)?;
        Ok(())
    }

    pub(crate) fn ensure_class_init_or_schedule(
        &mut self,
        class_name: &str,
    ) -> Result<bool, String> {
        Ok(matches!(
            self.ensure_class_init_inner(class_name, ClassInitMode::Schedule)?,
            ClassInitOutcome::Yield
        ))
    }

    /// Recursively create a multi-dimensional array for `multianewarray`.
    fn create_multi_array(&self, desc: &str, sizes: &[usize], depth: usize) -> JRef {
        let count = sizes[depth];
        if depth + 1 >= sizes.len() {
            let elem = if desc.ends_with("[I")
                || desc.ends_with("[B")
                || desc.ends_with("[C")
                || desc.ends_with("[S")
                || desc.ends_with("[Z")
            {
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
    fn is_instance_of_object(&mut self, obj: &JRef, target_class: &str) -> bool {
        if let Some(runtime_id) = self.object_runtime_class_id(obj) {
            if let Some(target_id) = self.tracked_class_id_for_name(target_class) {
                return self.is_instance_of_id(runtime_id, target_id);
            }
        }
        let runtime_class = obj.borrow().class_name.clone();
        self.is_instance_of(&runtime_class, target_class)
    }

    fn is_instance_of_object_id(&mut self, obj: &JRef, target_id: ClassId) -> bool {
        if let Some(runtime_id) = self.object_runtime_class_id(obj) {
            return self.is_instance_of_id(runtime_id, target_id);
        }
        let Some(target_class) = self.class_name(target_id).map(str::to_owned) else {
            return false;
        };
        self.is_instance_of_object(obj, &target_class)
    }

    fn is_instance_of(&mut self, runtime_class: &str, target_class: &str) -> bool {
        if runtime_class == target_class {
            return true;
        }
        if target_class == "java/lang/Object" {
            return true;
        }

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

        let Some(runtime_id) = self.tracked_class_id_for_name(runtime_class) else {
            return false;
        };
        let Some(target_id) = self.tracked_class_id_for_name(target_class) else {
            return false;
        };
        self.is_instance_of_id(runtime_id, target_id)
    }

    fn is_instance_of_id(&mut self, runtime_id: ClassId, target_id: ClassId) -> bool {
        if runtime_id == target_id {
            return true;
        }
        let cache_key = (runtime_id, target_id);
        if let Some(cached) = self.instanceof_cache.get(&cache_key) {
            return *cached;
        }
        let _ = self.ensure_class_prepared(runtime_id);
        let result = self
            .class_runtime_metadata_by_id(runtime_id)
            .map(|metadata| {
                metadata
                    .interface_ids
                    .iter()
                    .any(|&iface_id| self.is_instance_of_id(iface_id, target_id))
                    || metadata
                        .super_id
                        .map(|super_id| self.is_instance_of_id(super_id, target_id))
                        .unwrap_or(false)
            })
            .unwrap_or(false);
        self.instanceof_cache.insert(cache_key, result);
        result
    }
}
