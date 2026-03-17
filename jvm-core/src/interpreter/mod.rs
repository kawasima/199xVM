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

/// All execution-time data extracted from a resolved method in a single pass.
/// Returned by [`Vm::resolve_method_exec_info`] to avoid repeated `find_method`
/// calls and to give each field a self-documenting name.
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
    pub code: Vec<u8>,
    /// Exception handler table.
    pub exception_table: Vec<ExceptionTableEntry>,
    /// Shared constant-pool entries (`Rc` for O(1) clone).
    pub cp: Rc<Vec<ConstantPoolEntry>>,
    /// Bootstrap methods from the `BootstrapMethods` attribute.
    pub bootstrap_methods: Vec<BootstrapMethod>,
    /// `access_flags` from the method_info entry.
    pub access_flags: u16,
}

/// A class entry in the VM's class registry.
///
/// `Pending` holds the raw `.class` bytes and is promoted to `Ready` on first access,
/// implementing standard ClassLoader lazy-loading semantics.
pub(in crate::interpreter) enum LazyClass {
    /// Raw bytes not yet parsed.
    Pending(Vec<u8>),
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
    /// Class registry: keyed by internal name (`net/unit8/raoh/Result`).
    /// Entries start as `LazyClass::Pending` (raw bytes) and are promoted to
    /// `LazyClass::Ready` (parsed `ClassFile`) on first access.
    pub(in crate::interpreter) classes: HashMap<String, LazyClass>,
    /// Interned strings cache (not strictly required but saves allocations).
    pub(in crate::interpreter) string_pool: HashMap<String, JRef>,
    /// Static field storage keyed by class name → field name.
    /// Avoids allocating a `"ClassName.fieldName"` string on every getstatic/putstatic.
    pub(in crate::interpreter) static_fields: HashMap<String, HashMap<String, JValue>>,
    /// Classes whose `<clinit>` has already been run successfully.
    pub(in crate::interpreter) clinit_done: HashSet<String>,
    /// Classes whose `<clinit>` threw an exception (erroneous state per JVMS §5.5).
    pub(in crate::interpreter) clinit_failed: HashSet<String>,
    /// Canonical Class objects keyed by internal class name or descriptor.
    pub(in crate::interpreter) class_pool: HashMap<String, JRef>,
    /// Buffered `System.out.print` content until newline/println.
    pub(in crate::interpreter) stdout_buffer: String,
    /// Buffered `System.err.print` content until newline/println.
    pub(in crate::interpreter) stderr_buffer: String,
    /// Singleton system ClassLoader instance (created on first access).
    pub(in crate::interpreter) system_classloader: Option<JRef>,
    /// Green thread scheduler.
    pub(in crate::interpreter) scheduler: Scheduler,
    /// Object monitors keyed by object identity (Rc pointer address).
    monitors: HashMap<usize, Monitor>,
    /// Method resolution cache: (class, method_name, descriptor) → owner class name.
    /// Avoids repeated super-chain walks for the same method lookup.
    method_owner_cache: HashMap<(String, String, String), Option<String>>,
    /// Non-class resources from loaded JARs, keyed by path (e.g. "clojure/core.clj").
    pub resources: HashMap<String, Vec<u8>>,
}

impl Vm {
    /// Create an empty VM with a main thread.
    pub fn new() -> Self {
        Vm {
            classes: HashMap::new(),
            string_pool: HashMap::new(),
            static_fields: HashMap::new(),
            clinit_done: HashSet::new(),
            clinit_failed: HashSet::new(),
            class_pool: HashMap::new(),
            stdout_buffer: String::new(),
            stderr_buffer: String::new(),
            system_classloader: None,
            scheduler: Scheduler::new(),
            monitors: HashMap::new(),
            method_owner_cache: HashMap::new(),
            resources: HashMap::new(),
        }
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

    /// Set a pending IllegalMonitorStateException from an error message.
    pub(in crate::interpreter) fn throw_illegal_monitor_state(&mut self, err_msg: &str) {
        let msg = self.intern_string(err_msg);
        let exc = crate::heap::JObject::new("java/lang/IllegalMonitorStateException");
        exc.borrow_mut().fields.insert("detailMessage".to_owned(), JValue::Ref(Some(msg)));
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
        self.classes.insert(name, LazyClass::Ready(class_file));
    }

    /// Register raw `.class` bytes for lazy parsing.
    /// The class is parsed only when first accessed via [`Self::ensure_class_ready`].
    /// If the class is already registered (e.g., as `Ready`), the existing entry is kept.
    pub fn load_lazy(&mut self, name: String, bytes: Vec<u8>) {
        self.classes.entry(name).or_insert(LazyClass::Pending(bytes));
    }

    /// Load classes and resources from a JAR (ZIP) byte array.
    /// `.class` entries are registered via [`Self::load_lazy`]; all other
    /// non-directory entries are stored in the resource map.
    /// Returns the number of classes loaded.
    pub fn load_jar(&mut self, jar_bytes: &[u8]) -> Result<usize, String> {
        use std::io::Cursor;
        let reader = Cursor::new(jar_bytes);
        let mut archive = zip::ZipArchive::new(reader)
            .map_err(|e| format!("Invalid JAR/ZIP: {e}"))?;
        let mut count = 0;
        for i in 0..archive.len() {
            let mut file = archive.by_index(i)
                .map_err(|e| format!("ZIP entry error: {e}"))?;
            let name = file.name().to_owned();
            let mut buf = Vec::with_capacity(file.size() as usize);
            std::io::Read::read_to_end(&mut file, &mut buf)
                .map_err(|e| format!("Read error for {name}: {e}"))?;
            if name.ends_with(".class") {
                if let Some(class_name) = crate::class_file::parse_class_name(&buf) {
                    self.load_lazy(class_name, buf);
                    count += 1;
                }
            } else if !name.ends_with('/') {
                self.resources.insert(name, buf);
            }
        }
        Ok(count)
    }

    /// Ensure the named class is fully parsed (`Ready`).
    /// If the entry is `Pending`, parses it in place and promotes it to `Ready`.
    /// On parse failure the entry is set to `ParseError` so the failure is
    /// diagnosable and repeated parse attempts are avoided.
    /// Does nothing if the class is already `Ready`, `ParseError`, or not registered.
    pub(in crate::interpreter) fn ensure_class_ready(&mut self, name: &str) {
        // Only act when the entry is Pending; skip Ready / ParseError / missing.
        if !matches!(self.classes.get(name), Some(LazyClass::Pending(_))) {
            return;
        }
        if let Some(LazyClass::Pending(bytes)) = self.classes.remove(name) {
            match class_file::parse(&bytes) {
                Ok(cf) => { self.classes.insert(name.to_owned(), LazyClass::Ready(cf)); }
                Err(e) => {
                    eprintln!("Warning: failed to parse class '{name}': {e}");
                    self.classes.insert(name.to_owned(), LazyClass::ParseError(e.to_string()));
                }
            }
        }
    }

    /// Return a reference to a parsed class.
    /// Caller must have called `ensure_class_ready` first (or know the class is already Ready).
    pub(in crate::interpreter) fn get_class(&self, name: &str) -> Option<&ClassFile> {
        match self.classes.get(name)? {
            LazyClass::Ready(cf) => Some(cf),
            LazyClass::Pending(_) | LazyClass::ParseError(_) => None,
        }
    }

    /// Ensure class is ready and return a reference to it.
    pub(in crate::interpreter) fn resolve_class(&mut self, name: &str) -> Option<&ClassFile> {
        self.ensure_class_ready(name);
        self.get_class(name)
    }

    /// Flush buffered PrintStream output (`print` without trailing `println`).
    pub fn flush_printstreams(&mut self) {
        if !self.stdout_buffer.is_empty() {
            Self::emit_host_line(false, &self.stdout_buffer);
            self.stdout_buffer.clear();
        }
        if !self.stderr_buffer.is_empty() {
            Self::emit_host_line(true, &self.stderr_buffer);
            self.stderr_buffer.clear();
        }
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

    fn pending_exception_err(&self) -> Option<String> {
        self.scheduler.current_thread().pending_exception.as_ref().map(|r| {
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
            s
        })
    }

    /// Return (or lazily create) the singleton system ClassLoader instance.
    pub(in crate::interpreter) fn get_or_create_system_classloader(&mut self) -> JRef {
        if let Some(ref cl) = self.system_classloader {
            return Rc::clone(cl);
        }
        let cl = JObject::new("java/net/URLClassLoader");
        self.system_classloader = Some(Rc::clone(&cl));
        cl
    }

    /// Set `pending_exception` to a `NoClassDefFoundError` for `name`.
    /// `name` should be the internal (slash-separated) class name.
    pub(in crate::interpreter) fn throw_no_class_def_found(&mut self, name: &str) {
        let exc = JObject::new("java/lang/NoClassDefFoundError");
        let msg = self.intern_string(name.replace('/', "."));
        exc.borrow_mut().fields.insert("detailMessage".to_owned(), JValue::Ref(Some(msg)));
        *self.pending_exception_mut() = Some(exc);
    }

    /// Set `pending_exception` to a new `ClassNotFoundException` for `name`.
    /// `name` should be the runtime (dot-separated) class name.
    pub(in crate::interpreter) fn throw_class_not_found(&mut self, name: &str) {
        let exc = JObject::new("java/lang/ClassNotFoundException");
        let msg = self.intern_string(name.to_owned());
        exc.borrow_mut().fields.insert("detailMessage".to_owned(), JValue::Ref(Some(msg)));
        *self.pending_exception_mut() = Some(exc);
    }

    /// Set `pending_exception` to a `NullPointerException` with an optional detail message.
    pub(in crate::interpreter) fn throw_null_pointer(&mut self, detail: &str) {
        let exc = JObject::new("java/lang/NullPointerException");
        let msg = self.intern_string(detail.to_owned());
        exc.borrow_mut().fields.insert("detailMessage".to_owned(), JValue::Ref(Some(msg)));
        *self.pending_exception_mut() = Some(exc);
    }

    /// Set `pending_exception` to a `BootstrapMethodError` with a detail message.
    pub(in crate::interpreter) fn throw_bootstrap_method_error(&mut self, detail: &str) {
        let exc = JObject::new("java/lang/BootstrapMethodError");
        let msg = self.intern_string(detail.to_owned());
        exc.borrow_mut().fields.insert("detailMessage".to_owned(), JValue::Ref(Some(msg)));
        *self.pending_exception_mut() = Some(exc);
    }

    /// Set `pending_exception` to a `ClassFormatError` carrying the parse error message.
    /// Used when a class entry exists as `LazyClass::ParseError` (malformed bytecode).
    pub(in crate::interpreter) fn throw_class_format_error(&mut self, parse_msg: &str) {
        let exc = JObject::new("java/lang/ClassFormatError");
        let msg = self.intern_string(parse_msg.to_owned());
        exc.borrow_mut().fields.insert("detailMessage".to_owned(), JValue::Ref(Some(msg)));
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
        let bootstrap_methods = class.attributes.iter().find_map(|a| {
            if let Attribute::BootstrapMethods(bms) = a { Some(bms.clone()) } else { None }
        }).unwrap_or_default();
        Some(MethodExecInfo {
            class_name: class_name_out,
            descriptor: descriptor_out,
            access_flags,
            max_locals,
            has_code,
            code,
            exception_table,
            cp,
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
                let eiie = JObject::new("java/lang/ExceptionInInitializerError");
                let msg = self.intern_string(e.clone());
                eiie.borrow_mut().fields.insert("detailMessage".to_owned(), JValue::Ref(Some(msg)));
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
