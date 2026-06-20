//! Java bytecode interpreter.
//!
//! Implements a stack-based interpreter over the JVM instruction set.
//! The focus is on the subset needed to run Raoh:
//! - Core stack / local-variable operations
//! - Object creation and field access
//! - Method invocation (all four flavours + `invokedynamic`)
//! - Integer / long / reference comparisons and control flow
//! - Native stubs for `java.lang.*` and `java.util.*`

use std::cell::RefCell;
use std::collections::{HashMap, HashSet, VecDeque};
use std::rc::{Rc, Weak};

#[cfg(target_arch = "wasm32")]
use wasm_bindgen::prelude::*;

use crate::class_file::{
    self, Attribute, BootstrapMethod, ClassFile, ConstantPoolEntry, ExceptionTableEntry,
};
use crate::heap::{JavaStringValue, JObject, JRef, JValue};
use class_identity::{ClassId, ClassIdentityRegistry, ClassRecord, DefineClassError, LoaderId};

type OwnedJarArchive = zip::ZipArchive<std::io::Cursor<Vec<u8>>>;

const ACC_PUBLIC: u16 = 0x0001;
const ACC_PRIVATE: u16 = 0x0002;
const ACC_STATIC: u16 = 0x0008;
const ACC_INTERFACE: u16 = 0x0200;
const ACC_ABSTRACT: u16 = 0x0400;

/// All execution-time data extracted from a resolved method in a single pass.
/// Returned by [`Vm::resolve_method_exec_info`] to avoid repeated `find_method`
/// calls and to give each field a self-documenting name.
pub(super) struct MethodExecInfo {
    /// Loader-scoped class identity that owns the resolved method.
    pub class_id: Option<ClassId>,
    /// Internal class name that owns the resolved method.
    pub class_name: String,
    /// Resolved method descriptor (may differ from the call-site descriptor for generics).
    pub descriptor: String,
    /// `Code.max_locals` (0 if the method has no `Code` attribute).
    pub max_locals: usize,
    /// `true` when the method has a `Code` attribute (i.e. is not abstract/native).
    pub has_code: bool,
    /// Raw bytecode.
    pub code: Vec<u8>,
    /// Exception handler table.
    pub exception_table: Vec<ExceptionTableEntry>,
    /// Shared constant-pool entries (`Rc` for O(1) clone).
    pub cp: Rc<Vec<ConstantPoolEntry>>,
    /// Per-constant-pool resolution cache from the owning class.
    pub cache: cp_cache::CpCache,
    /// Bootstrap methods from the `BootstrapMethods` attribute.
    pub bootstrap_methods: Vec<BootstrapMethod>,
    /// `access_flags` from the method_info entry.
    pub access_flags: u16,
}

/// Loader-aware field resolution result for bytecode Fieldref entries.
pub(super) struct ResolvedFieldTarget {
    pub owner_class_id: ClassId,
    pub owner_class: String,
    pub name: String,
    pub descriptor: String,
    pub access_flags: u16,
}

/// Loader-aware method resolution result for bytecode Methodref entries.
pub(super) struct ResolvedMethodTarget {
    pub owner_class_id: ClassId,
    pub owner_class: String,
    pub name: String,
    pub descriptor: String,
    pub access_flags: u16,
}

#[derive(Clone, Eq, Hash, PartialEq)]
struct RegexCacheKey {
    pattern: String,
    flags: i32,
}

struct RegexCache {
    capacity: usize,
    order: VecDeque<RegexCacheKey>,
    entries: HashMap<RegexCacheKey, regex::Regex>,
}

impl RegexCache {
    fn new(capacity: usize) -> Self {
        Self {
            capacity,
            order: VecDeque::new(),
            entries: HashMap::new(),
        }
    }

    fn compile(&mut self, pattern: &str, flags: i32) -> Option<regex::Regex> {
        let key = RegexCacheKey {
            pattern: pattern.to_owned(),
            flags,
        };
        if let Some(regex) = self.entries.get(&key).cloned() {
            self.touch(&key);
            return Some(regex);
        }

        let regex = regex::Regex::new(pattern).ok()?;
        if self.capacity != 0 {
            while self.entries.len() >= self.capacity {
                let Some(lru_key) = self.order.pop_front() else {
                    break;
                };
                self.entries.remove(&lru_key);
            }
            self.order.push_back(key.clone());
            self.entries.insert(key, regex.clone());
        }
        Some(regex)
    }

    fn touch(&mut self, key: &RegexCacheKey) {
        if let Some(pos) = self.order.iter().position(|existing| existing == key) {
            self.order.remove(pos);
        }
        self.order.push_back(key.clone());
    }
}

#[cfg(test)]
mod tests {
    use super::{LazyClass, RegexCache, RegexCacheKey, Vm};
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

    #[test]
    fn regex_cache_evicts_least_recently_used_entry() {
        let mut cache = RegexCache::new(2);
        cache.compile("a", 0).expect("compile a");
        cache.compile("b", 0).expect("compile b");
        cache.compile("a", 0).expect("touch a");
        cache.compile("c", 0).expect("compile c");

        assert!(cache.entries.contains_key(&RegexCacheKey {
            pattern: "a".to_owned(),
            flags: 0,
        }));
        assert!(cache.entries.contains_key(&RegexCacheKey {
            pattern: "c".to_owned(),
            flags: 0,
        }));
        assert!(!cache.entries.contains_key(&RegexCacheKey {
            pattern: "b".to_owned(),
            flags: 0,
        }));
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
pub(crate) mod class_identity;
pub(crate) mod cp_cache;
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
    /// Legacy class registry: keyed by internal name (`net/unit8/raoh/Result`).
    /// Loader-scoped identity truth lives in `class_identities`; this map remains
    /// as the Phase 1 bytecode storage path until execution is migrated.
    /// Entries start as `LazyClass::PendingBytes`/`PendingJarEntry` and are promoted to
    /// `LazyClass::Ready` (parsed `ClassFile`) on first access.
    pub(in crate::interpreter) classes: HashMap<String, LazyClass>,
    /// Loader-scoped bytecode storage for classes whose plain binary name is not
    /// enough to identify the defining class.
    pub(in crate::interpreter) classes_by_id: HashMap<ClassId, LazyClass>,
    /// Loader-owned class identity records for defining and initiating loaders.
    class_identities: ClassIdentityRegistry,
    /// Java ClassLoader object identity to VM LoaderId side table.
    classloader_ids: HashMap<usize, LoaderId>,
    /// VM LoaderId to Java ClassLoader object side table for Java-visible mirrors.
    classloader_objects: HashMap<LoaderId, JRef>,
    /// Next dynamically assigned LoaderId. Built-in ids occupy 0 and 1.
    next_loader_id: u64,
    /// Interned strings cache keyed by UTF-16 content.
    pub(in crate::interpreter) string_pool: HashMap<JavaStringValue, JRef>,
    /// Static field storage keyed by class name → field name.
    /// Avoids allocating a `"ClassName.fieldName"` string on every getstatic/putstatic.
    pub(in crate::interpreter) static_fields: HashMap<String, HashMap<String, JValue>>,
    /// Classes whose `<clinit>` has already been run successfully.
    pub(in crate::interpreter) clinit_done: HashSet<String>,
    /// Classes whose `<clinit>` threw an exception (erroneous state per JVMS §5.5).
    pub(in crate::interpreter) clinit_failed: HashSet<String>,
    /// Canonical Class objects keyed by loader-scoped class identity.
    pub(in crate::interpreter) class_pool: HashMap<ClassId, JRef>,
    /// Heap object identity to its loader-scoped class identity.
    pub(in crate::interpreter) object_class_ids: HashMap<usize, (Weak<RefCell<JObject>>, ClassId)>,
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
    /// Bounded LRU cache for compiled host-side regular expressions.
    regex_cache: RegexCache,
    /// Materialized non-class resources from loaded JARs, keyed by path.
    pub resources: HashMap<String, Vec<u8>>,
    /// Non-class resources that still point at compressed JAR entries.
    pending_resources: HashMap<String, JarEntryRef>,
    /// Parsed ZIP archives kept alive so lazy entry reads do not re-scan the central directory.
    jar_archives: Vec<OwnedJarArchive>,
}

impl Vm {
    /// Create an empty VM with a main thread.
    pub fn new() -> Self {
        Vm {
            classes: HashMap::new(),
            classes_by_id: HashMap::new(),
            class_identities: ClassIdentityRegistry::new(),
            classloader_ids: HashMap::new(),
            classloader_objects: HashMap::new(),
            next_loader_id: 2,
            string_pool: HashMap::new(),
            static_fields: HashMap::new(),
            clinit_done: HashSet::new(),
            clinit_failed: HashSet::new(),
            class_pool: HashMap::new(),
            object_class_ids: HashMap::new(),
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
            regex_cache: RegexCache::new(64),
            resources: HashMap::new(),
            pending_resources: HashMap::new(),
            jar_archives: Vec::new(),
        }
    }

    pub(super) fn compile_regex_cached(&mut self, pattern: &str, flags: i32) -> Option<regex::Regex> {
        self.regex_cache.compile(pattern, flags)
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

    pub(in crate::interpreter) fn record_object_class_id(&mut self, obj: &JRef, class_id: ClassId) {
        self.object_class_ids.insert(Self::object_id(obj), (Rc::downgrade(obj), class_id));
    }

    pub(in crate::interpreter) fn class_id_for_object(&self, obj: &JRef) -> Option<ClassId> {
        let (recorded, class_id) = self.object_class_ids.get(&Self::object_id(obj))?;
        recorded
            .upgrade()
            .filter(|existing| Rc::ptr_eq(existing, obj))
            .map(|_| *class_id)
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

    pub(crate) fn register_defined_class(
        &mut self,
        defining_loader: LoaderId,
        internal_name: impl Into<String>,
    ) -> ClassId {
        self.class_identities.register_defined_class(defining_loader, internal_name)
    }

    pub(crate) fn try_register_defined_class(
        &mut self,
        defining_loader: LoaderId,
        internal_name: impl Into<String>,
    ) -> Result<ClassId, DefineClassError> {
        self.class_identities
            .try_register_defined_class(defining_loader, internal_name)
    }

    pub(crate) fn record_initiating_loader(
        &mut self,
        initiating_loader: LoaderId,
        lookup_name: impl Into<String>,
        class_id: ClassId,
    ) {
        self.class_identities.record_initiating_loader(initiating_loader, lookup_name, class_id);
    }

    pub(crate) fn class_id_for_defined(
        &self,
        defining_loader: LoaderId,
        internal_name: &str,
    ) -> Option<ClassId> {
        self.class_identities.class_id_for_defined(defining_loader, internal_name)
    }

    pub(crate) fn class_id_for_initiating(
        &self,
        initiating_loader: LoaderId,
        lookup_name: &str,
    ) -> Option<ClassId> {
        self.class_identities.class_id_for_initiating(initiating_loader, lookup_name)
    }

    pub(crate) fn class_record(&self, class_id: ClassId) -> Option<&ClassRecord> {
        self.class_identities.class_record(class_id)
    }

    pub(crate) fn loader_id_for_classloader(&mut self, classloader: &JRef) -> LoaderId {
        let object_id = Self::object_id(classloader);
        if let Some(loader_id) = self.classloader_ids.get(&object_id) {
            return *loader_id;
        }
        let loader_id = LoaderId::new(self.next_loader_id);
        self.next_loader_id += 1;
        self.classloader_ids.insert(object_id, loader_id);
        self.classloader_objects.insert(loader_id, Rc::clone(classloader));
        loader_id
    }

    /// Register a pre-parsed class file (always stored as `Ready`).
    pub fn load_class(&mut self, class_file: ClassFile) {
        self.load_class_with_loader(LoaderId::SYSTEM, class_file);
    }

    pub(crate) fn load_class_with_loader(
        &mut self,
        defining_loader: LoaderId,
        class_file: ClassFile,
    ) {
        let name = class_file.constant_pool.class_name(class_file.this_class).to_owned();
        let class_id = self.register_defined_class(defining_loader, name.clone());
        self.record_initiating_loader(defining_loader, name.clone(), class_id);
        self.classes_by_id.insert(class_id, LazyClass::Ready(class_file.clone()));
        self.classes.insert(name, LazyClass::Ready(class_file));
    }

    /// Register raw `.class` bytes for lazy parsing.
    /// The class is parsed only when first accessed via [`Self::ensure_class_ready`].
    /// If the class is already registered (e.g., as `Ready`), the existing entry is kept.
    pub fn load_lazy(&mut self, name: String, bytes: Vec<u8>) {
        self.load_lazy_with_loader(LoaderId::SYSTEM, name, bytes);
    }

    pub(crate) fn load_lazy_with_loader(
        &mut self,
        defining_loader: LoaderId,
        name: String,
        bytes: Vec<u8>,
    ) -> ClassId {
        let class_id = self.register_defined_class(defining_loader, name.clone());
        self.record_initiating_loader(defining_loader, name.clone(), class_id);
        self.classes_by_id
            .entry(class_id)
            .or_insert_with(|| LazyClass::PendingBytes(bytes.clone()));
        self.classes.entry(name).or_insert(LazyClass::PendingBytes(bytes));
        class_id
    }

    fn load_lazy_jar_entry(&mut self, name: String, entry: JarEntryRef) {
        let class_id = self.register_defined_class(LoaderId::SYSTEM, name.clone());
        self.record_initiating_loader(LoaderId::SYSTEM, name.clone(), class_id);
        self.classes_by_id
            .entry(class_id)
            .or_insert_with(|| LazyClass::PendingJarEntry(entry.clone()));
        self.classes.entry(name).or_insert(LazyClass::PendingJarEntry(entry));
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

    pub(in crate::interpreter) fn ensure_class_ready_by_id(&mut self, class_id: ClassId) {
        if !matches!(
            self.classes_by_id.get(&class_id),
            Some(LazyClass::PendingBytes(_) | LazyClass::PendingJarEntry(_))
        ) {
            return;
        }
        let pending = self.classes_by_id.remove(&class_id);
        let expected_name = self
            .class_record(class_id)
            .map(|record| record.internal_name.clone())
            .unwrap_or_default();
        let result = match pending {
            Some(LazyClass::PendingBytes(bytes)) => class_file::parse(&bytes).map_err(|e| e.to_string()),
            Some(LazyClass::PendingJarEntry(entry)) => match self.read_jar_entry(&entry) {
                Ok(bytes) => match class_file::parse(&bytes) {
                    Ok(cf) => {
                        let actual_name = cf.constant_pool.class_name(cf.this_class);
                        if actual_name == expected_name {
                            Ok(cf)
                        } else {
                            Err(format!(
                                "Class name mismatch for {}: expected {}, found {}",
                                entry.entry_name, expected_name, actual_name
                            ))
                        }
                    }
                    Err(e) => Err(e.to_string()),
                },
                Err(e) => Err(e),
            },
            Some(other) => {
                self.classes_by_id.insert(class_id, other);
                return;
            }
            None => return,
        };
        match result {
            Ok(cf) => {
                self.classes_by_id.insert(class_id, LazyClass::Ready(cf));
            }
            Err(e) => {
                eprintln!("Warning: failed to parse class '{expected_name}': {e}");
                self.classes_by_id.insert(class_id, LazyClass::ParseError(e));
            }
        }
    }

    pub(in crate::interpreter) fn get_class_by_id(&self, class_id: ClassId) -> Option<&ClassFile> {
        if let Some(entry) = self.classes_by_id.get(&class_id) {
            return match entry {
                LazyClass::Ready(cf) => Some(cf),
                LazyClass::PendingBytes(_) | LazyClass::PendingJarEntry(_) | LazyClass::ParseError(_) => None,
            };
        }
        let name = &self.class_record(class_id)?.internal_name;
        self.get_class(name)
    }

    pub(in crate::interpreter) fn resolve_class_by_id(&mut self, class_id: ClassId) -> Option<&ClassFile> {
        self.ensure_class_ready_by_id(class_id);
        if self.classes_by_id.contains_key(&class_id) {
            return self.get_class_by_id(class_id);
        }
        let name = self.class_record(class_id)?.internal_name.clone();
        self.resolve_class(&name)
    }

    pub(in crate::interpreter) fn resolve_class_for_class_object(
        &mut self,
        class_object: &JRef,
        internal_name: &str,
    ) -> Option<&ClassFile> {
        if let Some(class_id) = self.class_id_from_class_object(class_object) {
            return self.resolve_class_by_id(class_id);
        }
        self.resolve_class(internal_name)
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

    /// Intern a Java string (returns same `JRef` for equal UTF-16 content).
    pub fn intern_string(&mut self, s: impl Into<String>) -> JRef {
        self.intern_string_value(JavaStringValue::new(s))
    }

    pub fn intern_string_utf16(&mut self, utf16: Vec<u16>) -> JRef {
        self.intern_string_value(JavaStringValue::from_utf16(utf16))
    }

    pub fn intern_string_value(&mut self, value: JavaStringValue) -> JRef {
        use std::collections::hash_map::Entry;
        match self.string_pool.entry(value.clone()) {
            Entry::Occupied(e) => Rc::clone(e.get()),
            Entry::Vacant(e) => {
                let jobj = JObject::new_string_value(value);
                Rc::clone(e.insert(jobj))
            }
        }
    }

    pub fn intern_existing_string_ref(&mut self, string_ref: &JRef) -> Option<JRef> {
        use std::collections::hash_map::Entry;

        let value = string_ref.borrow().as_java_string_value().cloned()?;
        match self.string_pool.entry(value) {
            Entry::Occupied(e) => Some(Rc::clone(e.get())),
            Entry::Vacant(e) => {
                e.insert(Rc::clone(string_ref));
                Some(Rc::clone(string_ref))
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
                    if let Some(msg) = msg_ref.borrow().java_string_to_string_lossy() {
                        if !msg.is_empty() {
                            s.push_str(": ");
                            s.push_str(&msg);
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

    pub(in crate::interpreter) fn pending_exception_err(&self) -> Option<String> {
        self.scheduler.current_thread().pending_exception.as_ref().map(|r| self.format_exception_ref(r))
    }

    /// Return (or lazily create) the singleton system ClassLoader instance.
    pub(in crate::interpreter) fn get_or_create_system_classloader(&mut self) -> JRef {
        if let Some(ref cl) = self.system_classloader {
            return Rc::clone(cl);
        }
        let cl = JObject::new("java/lang/ClassLoader");
        let object_id = Self::object_id(&cl);
        self.classloader_ids.insert(object_id, LoaderId::SYSTEM);
        self.classloader_objects.insert(LoaderId::SYSTEM, Rc::clone(&cl));
        self.system_classloader = Some(Rc::clone(&cl));
        cl
    }

    /// Set `pending_exception` to a `NoClassDefFoundError` for `name`.
    /// `name` should be the internal (slash-separated) class name.
    pub(in crate::interpreter) fn throw_no_class_def_found(&mut self, name: &str) {
        let exc = self.new_vm_exception_message("java/lang/NoClassDefFoundError", name.replace('/', "."));
        *self.pending_exception_mut() = Some(exc);
    }

    pub(in crate::interpreter) fn throw_no_such_field_error(&mut self, detail: &str) {
        let exc = self.new_vm_exception_message("java/lang/NoSuchFieldError", detail);
        *self.pending_exception_mut() = Some(exc);
    }

    pub(in crate::interpreter) fn throw_no_such_method_error(&mut self, detail: &str) {
        let exc = self.new_vm_exception_message("java/lang/NoSuchMethodError", detail);
        *self.pending_exception_mut() = Some(exc);
    }

    pub(in crate::interpreter) fn throw_incompatible_class_change_error(&mut self, detail: &str) {
        let exc = self.new_vm_exception_message("java/lang/IncompatibleClassChangeError", detail);
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

    /// Set `pending_exception` to a `LinkageError` carrying a detail message.
    pub(in crate::interpreter) fn throw_linkage_error(&mut self, detail: &str) {
        let exc = self.new_vm_exception_message("java/lang/LinkageError", detail);
        *self.pending_exception_mut() = Some(exc);
    }

    fn class_object(&mut self, internal_name: impl Into<String>) -> JRef {
        let internal_name = internal_name.into();
        let class_id = self.class_id_for_mirror_name(&internal_name);
        self.class_object_for_id(class_id)
            .expect("class identity was just registered")
    }

    fn class_id_for_mirror_name(&mut self, internal_name: &str) -> ClassId {
        self.class_id_for_defined(LoaderId::SYSTEM, internal_name)
            .or_else(|| self.class_id_for_defined(LoaderId::BOOTSTRAP, internal_name))
            .unwrap_or_else(|| {
                let defining_loader = if Self::is_vm_defined_mirror_name(internal_name) {
                    LoaderId::BOOTSTRAP
                } else {
                    LoaderId::SYSTEM
                };
                self.register_defined_class(defining_loader, internal_name.to_owned())
            })
    }

    fn is_vm_defined_mirror_name(internal_name: &str) -> bool {
        internal_name.starts_with('[')
            || matches!(
                internal_name,
                "boolean" | "byte" | "char" | "short" | "int" | "long" | "float" | "double" | "void"
            )
    }

    fn class_loader_ref_for_mirror(&mut self, defining_loader: LoaderId) -> Option<JRef> {
        if defining_loader == LoaderId::BOOTSTRAP {
            return None;
        }
        if defining_loader == LoaderId::SYSTEM {
            return Some(self.get_or_create_system_classloader());
        }
        self.classloader_objects.get(&defining_loader).cloned()
    }

    pub(crate) fn class_object_for_id(&mut self, class_id: ClassId) -> Option<JRef> {
        if let Some(r) = self.class_pool.get(&class_id) {
            return Some(Rc::clone(r));
        }
        let record = self.class_record(class_id)?.clone();
        let loader_ref = self.class_loader_ref_for_mirror(record.defining_loader);
        let internal_name = self.intern_string(record.internal_name);
        let binary_name = self.intern_string(record.binary_name);
        let obj = JObject::new("java/lang/Class");
        {
            let mut obj_ref = obj.borrow_mut();
            obj_ref
                .fields
                .insert("__name_internal".to_owned(), JValue::Ref(Some(internal_name)));
            obj_ref
                .fields
                .insert("__name".to_owned(), JValue::Ref(Some(binary_name)));
            obj_ref
                .fields
                .insert("__class_loader".to_owned(), JValue::Ref(loader_ref));
        }
        self.class_pool.insert(class_id, Rc::clone(&obj));
        Some(obj)
    }

    pub(crate) fn class_id_from_class_object(&self, class_object: &JRef) -> Option<ClassId> {
        self.class_pool.iter().find_map(|(class_id, existing)| {
            if Rc::ptr_eq(existing, class_object) {
                Some(*class_id)
            } else {
                None
            }
        })
    }

    pub(crate) fn resolve_symbolic_class(
        &mut self,
        caller_class_id: ClassId,
        cp: &[ConstantPoolEntry],
        idx: u16,
    ) -> Result<ClassId, String> {
        let name = match cp.get(idx as usize) {
            Some(ConstantPoolEntry::Class { name_index }) => match cp.get(*name_index as usize) {
                Some(ConstantPoolEntry::Utf8(name)) => name.clone(),
                _ => return Err(format!("java/lang/NoClassDefFoundError: invalid class reference #{idx}")),
            },
            _ => return Err(format!("java/lang/NoClassDefFoundError: invalid class reference #{idx}")),
        };
        self.resolve_symbolic_class_name(caller_class_id, &name)
    }

    pub(crate) fn resolve_symbolic_class_name(
        &mut self,
        caller_class_id: ClassId,
        internal_name: &str,
    ) -> Result<ClassId, String> {
        if internal_name.starts_with('[') {
            return self.resolve_array_class(caller_class_id, internal_name);
        }

        let initiating_loader = self
            .class_record(caller_class_id)
            .map(|record| record.defining_loader)
            .unwrap_or(LoaderId::SYSTEM);
        if let Some(class_id) = self.class_id_for_initiating(initiating_loader, internal_name) {
            return Ok(class_id);
        }

        if let Some(loader_object) = self.classloader_objects.get(&initiating_loader).cloned() {
            let binary_name_ref = self.intern_string(internal_name.replace('/', "."));
            let loader_class_name = loader_object.borrow().class_name.clone();
            match self.invoke_virtual(
                loader_object,
                &loader_class_name,
                "loadClass",
                "(Ljava/lang/String;)Ljava/lang/Class;",
                vec![JValue::Ref(Some(binary_name_ref))],
            ) {
                Ok(JValue::Ref(Some(class_object))) => {
                    if let Some(class_id) = self.class_id_from_class_object(&class_object) {
                        self.record_initiating_loader(initiating_loader, internal_name.to_owned(), class_id);
                        return Ok(class_id);
                    }
                }
                Ok(_) => {}
                Err(err) => {
                    if !err.contains("java/lang/ClassNotFoundException") {
                        return Err(err);
                    }
                }
            }
            self.throw_no_class_def_found(internal_name);
            return Err(format!("java/lang/NoClassDefFoundError: {internal_name}"));
        }

        self.ensure_class_ready(internal_name);
        if matches!(self.classes.get(internal_name), Some(LazyClass::ParseError(_))) {
            self.throw_class_format_error(internal_name);
            return Err(format!("java/lang/ClassFormatError: malformed class file for {internal_name}"));
        }
        let class_id = self
            .class_id_for_defined(initiating_loader, internal_name)
            .or_else(|| self.class_id_for_defined(LoaderId::SYSTEM, internal_name))
            .or_else(|| self.class_id_for_defined(LoaderId::BOOTSTRAP, internal_name));
        if let Some(class_id) = class_id {
            self.record_initiating_loader(initiating_loader, internal_name.to_owned(), class_id);
            Ok(class_id)
        } else {
            self.throw_no_class_def_found(internal_name);
            Err(format!("java/lang/NoClassDefFoundError: {internal_name}"))
        }
    }

    fn resolve_array_class(
        &mut self,
        caller_class_id: ClassId,
        descriptor: &str,
    ) -> Result<ClassId, String> {
        let defining_loader = match descriptor.as_bytes().get(1).copied() {
            Some(b'Z' | b'B' | b'C' | b'S' | b'I' | b'J' | b'F' | b'D') => LoaderId::BOOTSTRAP,
            Some(b'L') => {
                let component = descriptor
                    .strip_prefix("[L")
                    .and_then(|s| s.strip_suffix(';'))
                    .ok_or_else(|| format!("java/lang/NoClassDefFoundError: {descriptor}"))?;
                let component_id = self.resolve_symbolic_class_name(caller_class_id, component)?;
                self.class_record(component_id)
                    .map(|record| record.defining_loader)
                    .unwrap_or(LoaderId::SYSTEM)
            }
            Some(b'[') => {
                let component_id = self.resolve_array_class(caller_class_id, &descriptor[1..])?;
                self.class_record(component_id)
                    .map(|record| record.defining_loader)
                    .unwrap_or(LoaderId::BOOTSTRAP)
            }
            _ => return Err(format!("java/lang/NoClassDefFoundError: {descriptor}")),
        };
        let class_id = self.register_defined_class(defining_loader, descriptor.to_owned());
        self.record_initiating_loader(defining_loader, descriptor.to_owned(), class_id);

        let initiating_loader = self
            .class_record(caller_class_id)
            .map(|record| record.defining_loader)
            .unwrap_or(LoaderId::BOOTSTRAP);
        if initiating_loader != defining_loader {
            self.record_initiating_loader(initiating_loader, descriptor.to_owned(), class_id);
        }

        Ok(class_id)
    }

    pub(in crate::interpreter) fn array_descriptor_for_component(component: &str) -> String {
        if component.starts_with('[') {
            format!("[{component}")
        } else {
            format!("[L{component};")
        }
    }

    pub(in crate::interpreter) fn resolve_array_class_for_component(
        &mut self,
        caller_class_id: ClassId,
        component_class_id: ClassId,
    ) -> Result<ClassId, String> {
        let component_record = self
            .class_record(component_class_id)
            .cloned()
            .ok_or_else(|| "java/lang/NoClassDefFoundError: invalid array component".to_owned())?;
        let descriptor = Self::array_descriptor_for_component(&component_record.internal_name);
        let class_id = self.register_defined_class(component_record.defining_loader, descriptor.clone());
        self.record_initiating_loader(component_record.defining_loader, descriptor.clone(), class_id);
        let initiating_loader = self
            .class_record(caller_class_id)
            .map(|record| record.defining_loader)
            .unwrap_or(LoaderId::BOOTSTRAP);
        if initiating_loader != component_record.defining_loader {
            self.record_initiating_loader(initiating_loader, descriptor, class_id);
        }
        Ok(class_id)
    }

    pub(in crate::interpreter) fn new_object_for_class_id(
        &mut self,
        class_id: ClassId,
    ) -> Result<JRef, String> {
        let class_name = self
            .class_record(class_id)
            .map(|record| record.internal_name.clone())
            .ok_or_else(|| "java/lang/NoClassDefFoundError: invalid resolved class".to_owned())?;
        let obj = JObject::new(class_name);
        self.record_object_class_id(&obj, class_id);
        Ok(obj)
    }

    pub(in crate::interpreter) fn new_array_for_class_id(
        &mut self,
        class_id: ClassId,
        elements: Vec<JValue>,
    ) -> Result<JRef, String> {
        let class_name = self
            .class_record(class_id)
            .map(|record| record.internal_name.clone())
            .ok_or_else(|| "java/lang/NoClassDefFoundError: invalid array class".to_owned())?;
        let obj = JObject::new_array(class_name, elements);
        self.record_object_class_id(&obj, class_id);
        Ok(obj)
    }

    pub(in crate::interpreter) fn primitive_array_class_id(
        &mut self,
        descriptor: &str,
    ) -> ClassId {
        let class_id = self.register_defined_class(LoaderId::BOOTSTRAP, descriptor.to_owned());
        self.record_initiating_loader(LoaderId::BOOTSTRAP, descriptor.to_owned(), class_id);
        class_id
    }

    /// Look up a loaded class by internal name (triggers lazy parse if needed).
    pub fn class(&mut self, name: &str) -> Option<&ClassFile> {
        self.resolve_class(name)
    }

    fn class_is_interface_by_id(&mut self, class_id: ClassId) -> bool {
        self.ensure_class_ready_by_id(class_id);
        self.get_class_by_id(class_id)
            .map(|class| class.access_flags & ACC_INTERFACE != 0)
            .unwrap_or(false)
    }

    fn member_name_and_descriptor(
        cp: &[ConstantPoolEntry],
        name_and_type_index: u16,
    ) -> Option<(String, String)> {
        let ConstantPoolEntry::NameAndType { name_index, descriptor_index } =
            cp.get(name_and_type_index as usize)?
        else {
            return None;
        };
        let name = match cp.get(*name_index as usize)? {
            ConstantPoolEntry::Utf8(name) => name.clone(),
            _ => return None,
        };
        let descriptor = match cp.get(*descriptor_index as usize)? {
            ConstantPoolEntry::Utf8(descriptor) => descriptor.clone(),
            _ => return None,
        };
        Some((name, descriptor))
    }

    pub(super) fn resolve_field_reference(
        &mut self,
        caller_class_id: ClassId,
        cp: &[ConstantPoolEntry],
        idx: u16,
    ) -> Result<ResolvedFieldTarget, String> {
        let (class_index, name_and_type_index) = match cp.get(idx as usize) {
            Some(ConstantPoolEntry::Fieldref { class_index, name_and_type_index }) => {
                (*class_index, *name_and_type_index)
            }
            _ => return Err(format!("java/lang/NoSuchFieldError: invalid field reference #{idx}")),
        };
        let (name, descriptor) = Self::member_name_and_descriptor(cp, name_and_type_index)
            .ok_or_else(|| format!("java/lang/NoSuchFieldError: invalid field reference #{idx}"))?;
        let referenced_class_id = self.resolve_symbolic_class(caller_class_id, cp, class_index)?;
        self.find_field_owner_by_class_id(referenced_class_id, &name, &descriptor)
            .ok_or_else(|| {
                let referenced_class = self
                    .class_record(referenced_class_id)
                    .map(|record| record.internal_name.clone())
                    .unwrap_or_else(|| "<invalid>".to_owned());
                let detail = format!("{referenced_class}.{name}:{descriptor}");
                self.throw_no_such_field_error(&detail);
                format!("java/lang/NoSuchFieldError: {detail}")
            })
    }

    pub(super) fn resolve_method_reference(
        &mut self,
        caller_class_id: ClassId,
        cp: &[ConstantPoolEntry],
        idx: u16,
    ) -> Result<ResolvedMethodTarget, String> {
        let (class_index, name_and_type_index, is_interface_ref) = match cp.get(idx as usize) {
            Some(ConstantPoolEntry::Methodref { class_index, name_and_type_index }) => {
                (*class_index, *name_and_type_index, false)
            }
            Some(ConstantPoolEntry::InterfaceMethodref { class_index, name_and_type_index }) => {
                (*class_index, *name_and_type_index, true)
            }
            _ => return Err(format!("java/lang/NoSuchMethodError: invalid method reference #{idx}")),
        };
        let (name, descriptor) = Self::member_name_and_descriptor(cp, name_and_type_index)
            .ok_or_else(|| format!("java/lang/NoSuchMethodError: invalid method reference #{idx}"))?;
        let referenced_class_id = self.resolve_symbolic_class(caller_class_id, cp, class_index)?;
        let is_interface = self.class_is_interface_by_id(referenced_class_id);
        if is_interface_ref != is_interface {
            let referenced_class = self
                .class_record(referenced_class_id)
                .map(|record| record.internal_name.clone())
                .unwrap_or_else(|| "<invalid>".to_owned());
            let detail = if is_interface_ref {
                format!("InterfaceMethodref resolved to non-interface {referenced_class}")
            } else {
                format!("Methodref resolved to interface {referenced_class}")
            };
            self.throw_incompatible_class_change_error(&detail);
            return Err(format!("java/lang/IncompatibleClassChangeError: {detail}"));
        }

        let target = if is_interface_ref {
            self.find_interface_method_owner_by_class_id(referenced_class_id, &name, &descriptor)
        } else {
            self.find_method_owner_by_class_id(referenced_class_id, &name, &descriptor)
        };
        target.ok_or_else(|| {
            let referenced_class = self
                .class_record(referenced_class_id)
                .map(|record| record.internal_name.clone())
                .unwrap_or_else(|| "<invalid>".to_owned());
            let detail = format!("{referenced_class}.{name}{descriptor}");
            self.throw_no_such_method_error(&detail);
            format!("java/lang/NoSuchMethodError: {detail}")
        })
    }

    pub(super) fn find_field_owner_by_class_id(
        &mut self,
        class_id: ClassId,
        field_name: &str,
        descriptor: &str,
    ) -> Option<ResolvedFieldTarget> {
        self.ensure_class_ready_by_id(class_id);
        let class = self.get_class_by_id(class_id)?;
        for field in &class.fields {
            let name = class.constant_pool.utf8(field.name_index);
            let desc = class.constant_pool.utf8(field.descriptor_index);
            if name == field_name && desc == descriptor {
                let owner_class = class.constant_pool.class_name(class.this_class).to_owned();
                return Some(ResolvedFieldTarget {
                    owner_class_id: class_id,
                    owner_class,
                    name: name.to_owned(),
                    descriptor: desc.to_owned(),
                    access_flags: field.access_flags,
                });
            }
        }
        let cp = Rc::clone(&class.constant_pool.entries);
        let super_class = class.super_class;
        let interfaces = class.interfaces.clone();

        for interface_index in interfaces {
            if let Ok(interface_id) = self.resolve_symbolic_class(class_id, &cp, interface_index) {
                if let Some(target) = self.find_field_owner_by_class_id(interface_id, field_name, descriptor) {
                    return Some(target);
                }
            }
        }
        if super_class != 0 {
            if let Ok(super_id) = self.resolve_symbolic_class(class_id, &cp, super_class) {
                if let Some(target) = self.find_field_owner_by_class_id(super_id, field_name, descriptor) {
                    return Some(target);
                }
            }
        }
        None
    }

    pub(super) fn find_method_owner_by_class_id(
        &mut self,
        class_id: ClassId,
        method_name: &str,
        descriptor: &str,
    ) -> Option<ResolvedMethodTarget> {
        if let Some(target) = self.declared_method_by_class_id(class_id, method_name, descriptor) {
            return Some(target);
        }
        let class = self.get_class_by_id(class_id)?;
        let cp = Rc::clone(&class.constant_pool.entries);
        let super_class = class.super_class;

        if super_class != 0 {
            if let Ok(super_id) = self.resolve_symbolic_class(class_id, &cp, super_class) {
                if let Some(target) =
                    self.find_method_owner_by_class_id(super_id, method_name, descriptor)
                {
                    return Some(target);
                }
            }
        }
        let candidates = self.superinterface_method_candidates(class_id, method_name, descriptor);
        self.choose_superinterface_method_candidate(candidates)
    }

    fn find_interface_method_owner_by_class_id(
        &mut self,
        interface_id: ClassId,
        method_name: &str,
        descriptor: &str,
    ) -> Option<ResolvedMethodTarget> {
        if let Some(target) =
            self.declared_method_by_class_id(interface_id, method_name, descriptor)
        {
            return Some(target);
        }
        if let Some(target) = self.object_public_instance_method(method_name, descriptor) {
            return Some(target);
        }

        let candidates =
            self.superinterface_method_candidates(interface_id, method_name, descriptor);
        self.choose_superinterface_method_candidate(candidates)
    }

    fn choose_superinterface_method_candidate(
        &mut self,
        candidates: Vec<ResolvedMethodTarget>,
    ) -> Option<ResolvedMethodTarget> {
        let mut concrete_maximally_specific_index = None;
        let mut concrete_maximally_specific_count = 0;
        for (index, candidate) in candidates.iter().enumerate() {
            let shadowed_by_subinterface = candidates.iter().any(|other| {
                other.owner_class_id != candidate.owner_class_id
                    && self.interface_extends_by_id(other.owner_class_id, candidate.owner_class_id)
            });
            if !shadowed_by_subinterface && candidate.access_flags & ACC_ABSTRACT == 0 {
                concrete_maximally_specific_count += 1;
                concrete_maximally_specific_index.get_or_insert(index);
            }
        }
        if concrete_maximally_specific_count == 1 {
            if let Some(index) = concrete_maximally_specific_index {
                return candidates.into_iter().nth(index);
            }
        }

        candidates.into_iter().next()
    }

    fn declared_method_by_class_id(
        &mut self,
        class_id: ClassId,
        method_name: &str,
        descriptor: &str,
    ) -> Option<ResolvedMethodTarget> {
        self.ensure_class_ready_by_id(class_id);
        let class = self.get_class_by_id(class_id)?;
        for method in &class.methods {
            let name = class.constant_pool.utf8(method.name_index);
            let desc = class.constant_pool.utf8(method.descriptor_index);
            if name == method_name && desc == descriptor {
                let owner_class = class.constant_pool.class_name(class.this_class).to_owned();
                return Some(ResolvedMethodTarget {
                    owner_class_id: class_id,
                    owner_class,
                    name: name.to_owned(),
                    descriptor: desc.to_owned(),
                    access_flags: method.access_flags,
                });
            }
        }
        None
    }

    fn object_public_instance_method(
        &mut self,
        method_name: &str,
        descriptor: &str,
    ) -> Option<ResolvedMethodTarget> {
        let object_id = self.loaded_class_id_by_name("java/lang/Object")?;
        let target = self.declared_method_by_class_id(object_id, method_name, descriptor)?;
        if Self::is_public_instance_method(target.access_flags) {
            Some(target)
        } else {
            None
        }
    }

    fn loaded_class_id_by_name(&mut self, internal_name: &str) -> Option<ClassId> {
        let existing = self
            .class_id_for_defined(LoaderId::SYSTEM, internal_name)
            .or_else(|| self.class_id_for_defined(LoaderId::BOOTSTRAP, internal_name));
        if existing.is_some() {
            return existing;
        }
        self.resolve_class(internal_name)?;
        self.class_id_for_defined(LoaderId::SYSTEM, internal_name)
            .or_else(|| self.class_id_for_defined(LoaderId::BOOTSTRAP, internal_name))
    }

    fn superinterface_method_candidates(
        &mut self,
        class_or_interface_id: ClassId,
        method_name: &str,
        descriptor: &str,
    ) -> Vec<ResolvedMethodTarget> {
        let mut candidates = Vec::new();
        let mut seen = HashSet::new();
        for direct_interface_id in self.direct_interface_ids(class_or_interface_id) {
            self.collect_interface_method_candidates(
                direct_interface_id,
                method_name,
                descriptor,
                &mut seen,
                &mut candidates,
            );
        }
        candidates
    }

    fn collect_interface_method_candidates(
        &mut self,
        interface_id: ClassId,
        method_name: &str,
        descriptor: &str,
        seen: &mut HashSet<ClassId>,
        candidates: &mut Vec<ResolvedMethodTarget>,
    ) {
        if !seen.insert(interface_id) {
            return;
        }
        if let Some(target) =
            self.declared_method_by_class_id(interface_id, method_name, descriptor)
        {
            if Self::is_inherited_interface_method_candidate(target.access_flags) {
                candidates.push(target);
            }
        }
        for superinterface_id in self.direct_interface_ids(interface_id) {
            self.collect_interface_method_candidates(
                superinterface_id,
                method_name,
                descriptor,
                seen,
                candidates,
            );
        }
    }

    fn interface_extends_by_id(
        &mut self,
        child_interface_id: ClassId,
        ancestor_interface_id: ClassId,
    ) -> bool {
        let mut seen = HashSet::new();
        self.interface_extends_by_id_inner(child_interface_id, ancestor_interface_id, &mut seen)
    }

    fn interface_extends_by_id_inner(
        &mut self,
        child_interface_id: ClassId,
        ancestor_interface_id: ClassId,
        seen: &mut HashSet<ClassId>,
    ) -> bool {
        if !seen.insert(child_interface_id) {
            return false;
        }
        for direct_interface_id in self.direct_interface_ids(child_interface_id) {
            if direct_interface_id == ancestor_interface_id
                || self.interface_extends_by_id_inner(
                    direct_interface_id,
                    ancestor_interface_id,
                    seen,
                )
            {
                return true;
            }
        }
        false
    }

    fn direct_interface_ids(&mut self, class_id: ClassId) -> Vec<ClassId> {
        self.ensure_class_ready_by_id(class_id);
        let Some(class) = self.get_class_by_id(class_id) else {
            return Vec::new();
        };
        let cp = Rc::clone(&class.constant_pool.entries);
        let interfaces = class.interfaces.clone();
        interfaces
            .into_iter()
            .filter_map(|interface_index| {
                self.resolve_symbolic_class(class_id, &cp, interface_index)
                    .ok()
            })
            .collect()
    }

    fn is_public_instance_method(access_flags: u16) -> bool {
        access_flags & ACC_PUBLIC != 0 && access_flags & ACC_STATIC == 0
    }

    fn is_inherited_interface_method_candidate(access_flags: u16) -> bool {
        access_flags & (ACC_PRIVATE | ACC_STATIC) == 0
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
                (ca.max_locals as usize, true, ca.code.clone(), ca.exception_table.clone())
            } else {
                (0, false, vec![], vec![])
            };
        let cp = Rc::clone(&class.constant_pool.entries);
        let cache = Rc::clone(&class.constant_pool.cache);
        let bootstrap_methods = class.attributes.iter().find_map(|a| {
            if let Attribute::BootstrapMethods(bms) = a { Some(bms.clone()) } else { None }
        }).unwrap_or_default();
        let class_id = self
            .class_id_for_defined(LoaderId::SYSTEM, &class_name_out)
            .or_else(|| self.class_id_for_defined(LoaderId::BOOTSTRAP, &class_name_out));
        Some(MethodExecInfo {
            class_id,
            class_name: class_name_out,
            descriptor: descriptor_out,
            access_flags,
            max_locals,
            has_code,
            code,
            exception_table,
            cp,
            cache,
            bootstrap_methods,
        })
    }

    pub(super) fn resolve_method_exec_info_for_class_id(
        &mut self,
        class_id: ClassId,
        method_name: &str,
        descriptor: &str,
    ) -> Option<MethodExecInfo> {
        let owner = self.find_method_owner_by_class_id(class_id, method_name, descriptor)?;
        self.ensure_class_ready_by_id(owner.owner_class_id);
        let class = self.get_class_by_id(owner.owner_class_id)?;
        let method_idx = class.methods.iter().position(|m| {
            class.constant_pool.utf8(m.name_index) == method_name
                && class.constant_pool.utf8(m.descriptor_index) == descriptor
        })?;
        let class_name_out = class.constant_pool.class_name(class.this_class).to_owned();
        let descriptor_out = class.constant_pool.utf8(class.methods[method_idx].descriptor_index).to_owned();
        let access_flags = class.methods[method_idx].access_flags;
        let (max_locals, has_code, code, exception_table) =
            if let Some(ca) = class.methods[method_idx].code() {
                (ca.max_locals as usize, true, ca.code.clone(), ca.exception_table.clone())
            } else {
                (0, false, vec![], vec![])
            };
        let cp = Rc::clone(&class.constant_pool.entries);
        let cache = Rc::clone(&class.constant_pool.cache);
        let bootstrap_methods = class.attributes.iter().find_map(|a| {
            if let Attribute::BootstrapMethods(bms) = a { Some(bms.clone()) } else { None }
        }).unwrap_or_default();
        Some(MethodExecInfo {
            class_id: Some(owner.owner_class_id),
            class_name: class_name_out,
            descriptor: descriptor_out,
            access_flags,
            max_locals,
            has_code,
            code,
            exception_table,
            cp,
            cache,
            bootstrap_methods,
        })
    }

    /// Find the name of the class that owns a given method (super-chain walk).
    /// Returns the canonical class name, or `None` if not found.
    fn find_method_owner(
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
        // borrow on `class` ends here
        if let Some(super_name) = super_name {
            if let Some(owner) = self.find_method_owner(&super_name, method_name, descriptor) {
                return Some(owner);
            }
        }
        let class_id = self.loaded_class_id_by_name(class_name)?;
        let candidates = self.superinterface_method_candidates(class_id, method_name, descriptor);
        self.choose_superinterface_method_candidate(candidates)
            .map(|target| target.owner_class)
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

    /// Run `<clinit>` for a class if it hasn't been initialized yet.
    /// Per JVMS §5.5: Before a class is initialized, its direct superclass must
    /// be initialized first (recursively), and any superinterfaces that declare
    /// default methods must also be initialized.
    fn ensure_class_init(&mut self, class_name: &str) -> Result<(), String> {
        if self.clinit_done.contains(class_name) {
            return Ok(());
        }
        // JVMS §5.5: if <clinit> previously failed, the class is in an erroneous state;
        // subsequent uses must throw NoClassDefFoundError.
        if self.clinit_failed.contains(class_name) {
            self.throw_no_class_def_found(class_name);
            return Err(format!("java/lang/NoClassDefFoundError: {class_name}"));
        }
        // Mark as initialized before running to prevent recursion.
        self.clinit_done.insert(class_name.to_owned());

        // Ensure the class is parsed first.
        self.ensure_class_ready(class_name);

        // Initialize super class first (JVMS §5.5 step 7).
        let (super_name, iface_names) = if let Some(class) = self.get_class(class_name) {
            let sup = if class.super_class != 0 {
                let s = class.constant_pool.class_name(class.super_class).to_owned();
                if s != "java/lang/Object" { Some(s) } else { None }
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
            self.ensure_class_init(&s)?;
        }
        for iface in iface_names {
            self.ensure_class_init(&iface)?;
        }

        // Check if THIS class (not superclasses) has a <clinit> method.
        // <clinit> is not inherited, so we must not walk the super-chain here —
        // doing so would re-execute a superclass <clinit> that was already run.
        self.ensure_class_ready(class_name);
        let has_clinit = self.get_class(class_name).map(|cf| {
            cf.methods.iter().any(|m| {
                cf.constant_pool.utf8(m.name_index) == "<clinit>"
                    && cf.constant_pool.utf8(m.descriptor_index) == "()V"
            })
        }).unwrap_or(false);
        if has_clinit {
            // JVMS §5.5: if <clinit> throws, wrap in ExceptionInInitializerError.
            if let Err(e) = self.invoke_static(class_name, "<clinit>", "()V", vec![]) {
                // Preserve the original exception object as the "cause" field.
                let cause = self.pending_exception_mut().take();
                let eiie = self.new_vm_exception_message("java/lang/ExceptionInInitializerError", e.clone());
                if let Some(c) = cause {
                    eiie.borrow_mut().fields.insert("cause".to_owned(), JValue::Ref(Some(c)));
                }
                *self.pending_exception_mut() = Some(eiie);
                // Remove from clinit_done so subsequent uses hit the clinit_failed path.
                self.clinit_done.remove(class_name);
                self.clinit_failed.insert(class_name.to_owned());
                // Return an error string that encodes the wrapped exception type so that
                // find_exception_handler sees ExceptionInInitializerError, not the original cause.
                #[cfg(target_arch = "wasm32")]
                console_error(&format!("[clinit-fail] {class_name}: {e}"));
                #[cfg(not(target_arch = "wasm32"))]
                eprintln!("[clinit-fail] {class_name}: {e}");
                return Err("java/lang/ExceptionInInitializerError".to_owned());
            }
        }
        Ok(())
    }

    /// Recursively create a multi-dimensional array for `multianewarray`.
    pub(in crate::interpreter) fn create_multi_array_for_class_id(
        &mut self,
        class_id: ClassId,
        sizes: &[usize],
    ) -> Result<JRef, String> {
        let record = self
            .class_record(class_id)
            .cloned()
            .ok_or_else(|| "java/lang/NoClassDefFoundError: invalid array class".to_owned())?;
        Ok(self.create_multi_array(&record.internal_name, record.defining_loader, sizes, 0))
    }

    pub(in crate::interpreter) fn create_multi_array_for_descriptor(
        &mut self,
        descriptor: &str,
        defining_loader: LoaderId,
        sizes: &[usize],
    ) -> JRef {
        self.create_multi_array(descriptor, defining_loader, sizes, 0)
    }

    fn create_multi_array(
        &mut self,
        desc: &str,
        defining_loader: LoaderId,
        sizes: &[usize],
        depth: usize,
    ) -> JRef {
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
            let obj = JObject::new_array(desc, vec![elem; count]);
            let class_id = self.register_defined_class(defining_loader, desc.to_owned());
            self.record_object_class_id(&obj, class_id);
            obj
        } else {
            let sub_desc = &desc[1..];
            let elements: Vec<JValue> = (0..count)
                .map(|_| {
                    JValue::Ref(Some(self.create_multi_array(
                        sub_desc,
                        defining_loader,
                        sizes,
                        depth + 1,
                    )))
                })
                .collect();
            let obj = JObject::new_array(desc, elements);
            let class_id = self.register_defined_class(defining_loader, desc.to_owned());
            self.record_object_class_id(&obj, class_id);
            obj
        }
    }

    /// Check if `runtime_class` is an instance of `target_class` (by name).
    /// Handles array types per JVMS §6.5.instanceof / §6.5.checkcast.
    fn is_instance_of(&mut self, runtime_class: &str, target_class: &str) -> bool {
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
