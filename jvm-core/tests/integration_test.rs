//! Integration tests for the 199xVM.
//!
//! These tests load pre-compiled test classes from a JAR file and verify
//! bytecode execution without requiring a browser or WASM runtime.
//!
//! Prerequisites:
//!   ./build-shim.sh          (builds jdk-shim/bundle.bin)
//!   ./build-test-bundle.sh   (builds test-classes/test.jar)

fn shim_bundle() -> &'static [u8] {
    include_bytes!("../../jdk-shim/bundle.bin")
}

fn test_jar() -> &'static [u8] {
    include_bytes!("../../test-classes/test.jar")
}

fn jar_loader_test_jar() -> &'static [u8] {
    include_bytes!("test.jar")
}

/// Run a test class from the test JAR via the JAR loader.
fn run_jar_test(class: &str, method: &str, descriptor: &str) -> String {
    let mut vm = jvm_core::interpreter::Vm::new();
    jvm_core::load_bundle(&mut vm, shim_bundle());
    vm.load_jar(test_jar()).expect("failed to load test JAR");
    let result = vm.invoke_static_threaded(class, method, descriptor, vec![]);
    vm.flush_printstreams();
    match result {
        Ok(v) => {
            if let jvm_core::heap::JValue::Ref(Some(ref r)) = v {
                let is_str = matches!(r.borrow().native, jvm_core::heap::NativePayload::JavaString(_));
                if !is_str {
                    let cn = r.borrow().class_name.clone();
                    if let Ok(s) = vm.invoke_virtual(r.clone(), &cn, "toString", "()Ljava/lang/String;", vec![]) {
                        return jvalue_to_string(&s);
                    }
                }
            }
            jvalue_to_string(&v)
        }
        Err(e) => format!("ERROR: {e}"),
    }
}

fn jvalue_to_string(v: &jvm_core::heap::JValue) -> String {
    match v {
        jvm_core::heap::JValue::Void => "void".to_owned(),
        jvm_core::heap::JValue::Int(i) => i.to_string(),
        jvm_core::heap::JValue::Long(l) => l.to_string(),
        jvm_core::heap::JValue::Float(f) => f.to_string(),
        jvm_core::heap::JValue::Double(d) => d.to_string(),
        jvm_core::heap::JValue::Ref(None) => "null".to_owned(),
        jvm_core::heap::JValue::Ref(Some(r)) => {
            let obj = r.borrow();
            match &obj.native {
                jvm_core::heap::NativePayload::JavaString(s) => s.clone(),
                _ => format!("{}@obj", obj.class_name),
            }
        }
        jvm_core::heap::JValue::ReturnAddress(a) => format!("ret:{a}"),
    }
}

// ---------------------------------------------------------------------------
// Integer.toString via shim bytecode
// ---------------------------------------------------------------------------

#[test]
fn integer_tostring() {
    let result = run_jar_test(
        "IntToStringTest",
        "run",
        "()Ljava/lang/String;",
    );
    assert_eq!(result, "42");
}

// ---------------------------------------------------------------------------
// String concatenation: "OK: " + Integer.valueOf(42)
// ---------------------------------------------------------------------------

#[test]
fn string_concat() {
    let result = run_jar_test(
        "StringConcatTest",
        "run",
        "()Ljava/lang/String;",
    );
    assert_eq!(result, "OK: 42");
}

// ---------------------------------------------------------------------------
// ArrayList basic operations
// ---------------------------------------------------------------------------
// java.time.LocalDateTime.now() — regression for missing Clock shim
// ---------------------------------------------------------------------------

#[test]
fn local_datetime_now() {
    let result = run_jar_test(
        "LocalDateTimeNowTest",
        "run",
        "()Ljava/lang/String;",
    );
    assert_eq!(result, "ok");
}

// ---------------------------------------------------------------------------

#[test]
fn arraylist_basics() {
    let result = run_jar_test(
        "ListTest",
        "run",
        "()Ljava/lang/String;",
    );
    assert_eq!(result, "2: hello world");
}

// ---------------------------------------------------------------------------
// try/catch: NullPointerException caught in handler
// ---------------------------------------------------------------------------

#[test]
fn try_catch_npe() {
    let result = run_jar_test(
        "TryCatchTest",
        "run",
        "()Ljava/lang/String;",
    );
    assert_eq!(result, "CAUGHT");
}

#[test]
fn factorial_long() {
    let result = run_jar_test(
        "Factorial",
        "run",
        "()Ljava/lang/String;",
    );
    assert_eq!(result, "10!=3628800 15!=1307674368000");
}

// ---------------------------------------------------------------------------
// Arrays.copyOf(int[], int)
// ---------------------------------------------------------------------------

#[test]
fn arrays_copy_of_int() {
    let result = run_jar_test(
        "ArraysCopyOfTest",
        "run",
        "()Ljava/lang/String;",
    );
    assert_eq!(result, "5:1,3,0");
}

// ---------------------------------------------------------------------------
// Stream.reduce(BinaryOperator) returning Optional
// ---------------------------------------------------------------------------

#[test]
fn stream_reduce_optional() {
    let result = run_jar_test(
        "StreamReduceTest",
        "run",
        "()Ljava/lang/String;",
    );
    assert_eq!(result, "abc:false");
}

// ---------------------------------------------------------------------------
// Lambda SAM dispatch respects argument arity (overloaded interface methods)
// ---------------------------------------------------------------------------

#[test]
fn lambda_overload_arity() {
    let result = run_jar_test(
        "LambdaOverloadTest",
        "run",
        "()Ljava/lang/String;",
    );
    assert_eq!(result, "HELLO!|HI?");
}

// ---------------------------------------------------------------------------
// Synchronized blocks (monitorenter / monitorexit)
// ---------------------------------------------------------------------------

#[test]
fn synchronized_blocks() {
    let result = run_jar_test(
        "SynchronizedTest",
        "run",
        "()Ljava/lang/String;",
    );
    assert_eq!(result, "13");
}

#[test]
fn synchronized_null_monitor_throws_npe() {
    let result = run_jar_test(
        "SynchronizedNullTest",
        "run",
        "()Ljava/lang/String;",
    );
    assert_eq!(result, "npe-ok");
}

// ---------------------------------------------------------------------------
// ClassLoader API: getSystemClassLoader, Class.forName, loadClass
// ---------------------------------------------------------------------------

#[test]
fn classloader_api() {
    let result = run_jar_test(
        "ClassLoaderTest",
        "run",
        "()Ljava/lang/String;",
    );
    assert_eq!(result, "cl:ok|forName:ok|loadClass:ok");
}

#[test]
fn classloader_missing_class_throws_cnfe() {
    let result = run_jar_test(
        "ClassLoaderNegativeTest",
        "run",
        "()Ljava/lang/String;",
    );
    assert_eq!(result, "ClassNotFoundException:com.example.NonExistentClass");
}

// ---------------------------------------------------------------------------
// JVMS §5.5: ExceptionInInitializerError when <clinit> throws
// ---------------------------------------------------------------------------

#[test]
fn clinit_exception_wrapped_in_eiie() {
    let result = run_jar_test(
        "ClinitExceptionTest",
        "run",
        "()Ljava/lang/String;",
    );
    assert_eq!(result, "ExceptionInInitializerError");
}

// ---------------------------------------------------------------------------
// JVMS §5.4.3.3: AbstractMethodError does not fire for concrete implementations
// ---------------------------------------------------------------------------

#[test]
fn concrete_interface_method_no_abstract_method_error() {
    let result = run_jar_test(
        "AbstractMethodTest",
        "run",
        "()Ljava/lang/String;",
    );
    assert_eq!(result, "hello");
}

// ---------------------------------------------------------------------------
// java.lang.ref queue bookkeeping
// ---------------------------------------------------------------------------

#[test]
fn reference_queue_basics() {
    let result = run_jar_test(
        "ReferenceQueueTest",
        "run",
        "()Ljava/lang/String;",
    );
    assert_eq!(result, "ok");
}

// ---------------------------------------------------------------------------
// JVMS §5.5: erroneous-state — second access throws NoClassDefFoundError
// ---------------------------------------------------------------------------

#[test]
fn clinit_erroneous_state_throws_ncdfe_on_second_access() {
    let result = run_jar_test(
        "ClinitErroneousStateTest",
        "run",
        "()Ljava/lang/String;",
    );
    assert_eq!(result, "EIIE,NCDFE");
}

// ---------------------------------------------------------------------------
// JVMS §6.5: invokeinterface dispatches to interface default method
// ---------------------------------------------------------------------------

#[test]
fn interface_default_method_dispatch() {
    let result = run_jar_test(
        "InterfaceDefaultMethodTest",
        "run",
        "()Ljava/lang/String;",
    );
    assert_eq!(result, "I am Thing");
}

// ---------------------------------------------------------------------------
// Green threads: Thread.start() / Thread.join()
// ---------------------------------------------------------------------------

#[test]
fn thread_start_join_basic() {
    let result = run_jar_test(
        "ThreadBasicTest",
        "run",
        "()Ljava/lang/String;",
    );
    assert_eq!(result, "ABC");
}

#[test]
fn monitor_contention_two_threads() {
    let result = run_jar_test(
        "MonitorContentionTest",
        "run",
        "()Ljava/lang/String;",
    );
    assert_eq!(result, "10");
}

#[test]
fn wait_notify_producer_consumer() {
    let result = run_jar_test(
        "WaitNotifyTest",
        "run",
        "()Ljava/lang/String;",
    );
    assert_eq!(result, "produced:42,consumed:42");
}

#[test]
fn notify_all_wakes_multiple_waiters() {
    let result = run_jar_test(
        "NotifyAllTest",
        "run",
        "()Ljava/lang/String;",
    );
    assert_eq!(result, "3");
}

#[test]
fn wait_without_lock_throws_imse() {
    let result = run_jar_test(
        "WaitWithoutLockTest",
        "run",
        "()Ljava/lang/String;",
    );
    assert_eq!(result, "wait:IMSE,notify:IMSE,notifyAll:IMSE");
}

#[test]
fn reentrant_wait_restores_count() {
    let result = run_jar_test(
        "ReentrantWaitTest",
        "run",
        "()Ljava/lang/String;",
    );
    assert_eq!(result, "ok");
}

// ---------------------------------------------------------------------------
// ACC_SYNCHRONIZED methods
// ---------------------------------------------------------------------------

#[test]
fn synchronized_methods() {
    let result = run_jar_test(
        "SynchronizedMethodTest",
        "run",
        "()Ljava/lang/String;",
    );
    assert_eq!(result, "3,2,200,2");
}

// ---------------------------------------------------------------------------
// Compact source files (implicit classes)
// ---------------------------------------------------------------------------

#[test]
fn compact_source_file() {
    let result = run_jar_test(
        "CompactTest",
        "run",
        "()Ljava/lang/String;",
    );
    assert_eq!(result, "ok");
}

// ---------------------------------------------------------------------------
// Lambda default method dispatch
// ---------------------------------------------------------------------------

#[test]
fn lambda_default_method() {
    let result = run_jar_test(
        "LambdaDefaultMethodTest",
        "run",
        "()Ljava/lang/String;",
    );
    assert_eq!(result, "hello:default");
}

// ---------------------------------------------------------------------------
// java.net: URLEncoder, URLDecoder, URI
// ---------------------------------------------------------------------------

#[test]
fn net_test() {
    let result = run_jar_test(
        "NetTest",
        "run",
        "()Ljava/lang/String;",
    );
    assert_eq!(result, "hello+world|hello world|example.com");
}

// ---------------------------------------------------------------------------
// java.net.URL: parsing and URI.toURL() round-trip
// ---------------------------------------------------------------------------

#[test]
fn url_test() {
    let result = run_jar_test(
        "URLTest",
        "run",
        "()Ljava/lang/String;",
    );
    assert_eq!(result, "https|example.com|8080|/path|q=1|frag|example.com");
}

// ---------------------------------------------------------------------------
// Matcher.matches(): already-anchored patterns and escaped \$ edge case
// ---------------------------------------------------------------------------

#[test]
fn matcher_test() {
    let result = run_jar_test(
        "MatcherTest",
        "run",
        "()Ljava/lang/String;",
    );
    // true|false|true|true|true|false|false
    // 1: ^foo$ matches "foo"
    // 2: ^foo$ does not match "foobar"
    // 3: email pattern matches "alice@example.com"
    // 4: ^foo\$ matches literal "foo$"
    // 5: non-anchored "foo" matches "foo" (full-string via wrapping)
    // 6: non-anchored "foo" does not match "foobar"
    // 7: alternation ^foo$|bar$ does NOT match "xxbar"
    assert_eq!(result, "true|false|true|true|true|false|false");
}

// ---------------------------------------------------------------------------
// Stream.filter().map().collect(Collectors.toList())
// ---------------------------------------------------------------------------

#[test]
fn stream_collect_filter_map() {
    let result = run_jar_test(
        "StreamCollectTest",
        "run",
        "()Ljava/lang/String;",
    );
    assert_eq!(result, "APPLE,AVOCADO");
}

// ---------------------------------------------------------------------------
// Collectors.joining()
// ---------------------------------------------------------------------------

#[test]
fn stream_collectors_joining() {
    let result = run_jar_test(
        "StreamJoinTest",
        "run",
        "()Ljava/lang/String;",
    );
    assert_eq!(result, "a-b-c");
}

// ---------------------------------------------------------------------------
// IntStream.range().filter().sum()
// ---------------------------------------------------------------------------

#[test]
fn int_stream_range_filter_sum() {
    let result = run_jar_test(
        "IntStreamTest",
        "run",
        "()Ljava/lang/String;",
    );
    assert_eq!(result, "sum=30");
}

// ---------------------------------------------------------------------------
// TreeMap: natural ordering iteration
// ---------------------------------------------------------------------------

#[test]
fn tree_map_natural_ordering() {
    let result = run_jar_test(
        "TreeMapTest",
        "run",
        "()Ljava/lang/String;",
    );
    assert_eq!(result, "apple=1,banana=2,cherry=3");
}

// ---------------------------------------------------------------------------
// TreeSet: sorted iteration
// ---------------------------------------------------------------------------

#[test]
fn tree_set_sorted_iteration() {
    let result = run_jar_test(
        "TreeSetTest",
        "run",
        "()Ljava/lang/String;",
    );
    assert_eq!(result, "10,20,30");
}

// ---------------------------------------------------------------------------
// PriorityQueue: min-heap poll order
// ---------------------------------------------------------------------------

#[test]
fn priority_queue_poll_order() {
    let result = run_jar_test(
        "PriorityQueueTest",
        "run",
        "()Ljava/lang/String;",
    );
    assert_eq!(result, "10,20,30");
}

// ---------------------------------------------------------------------------
// String.replace
// ---------------------------------------------------------------------------

#[test]
fn clojure_bootstrap_shims() {
    let result = run_jar_test("ClojureBootstrapShimsTest", "run", "()Ljava/lang/String;");
    assert_eq!(result, "eqIC|concat|regex|vparse|sysprop|tlocal|atomic|props|charset|replace|double|rwlock|file|bool");
}

#[test]
fn regex_capture_groups() {
    let result = run_jar_test("RegexGroupsTest", "run", "()Ljava/lang/String;");
    assert_eq!(result, "3|1.12.0|1|12|0");
}

#[test]
fn string_replace_char() {
    let result = run_jar_test("StringReplaceTest", "run", "()Ljava/lang/String;");
    assert_eq!(result, "clojure.core_proxy__init");
}

// ---------------------------------------------------------------------------
// JAR loader: load classes from a JAR file
// ---------------------------------------------------------------------------

#[test]
fn load_jar_and_run() {
    let mut vm = jvm_core::interpreter::Vm::new();
    jvm_core::load_bundle(&mut vm, shim_bundle());
    let count = vm.load_jar(jar_loader_test_jar()).expect("load_jar failed");
    assert!(count > 0, "expected at least one class from JAR");
    let result = match vm.invoke_static_threaded(
        "JarTestEntry", "run", "()Ljava/lang/String;", vec![],
    ) {
        Ok(v) => match v {
            jvm_core::heap::JValue::Ref(Some(r)) => {
                r.borrow().as_java_string().unwrap_or_default().to_owned()
            }
            _ => format!("{v:?}"),
        },
        Err(e) => format!("ERROR: {e}"),
    };
    assert_eq!(result, "jar-ok");
}

#[test]
fn load_jar_resource() {
    let mut vm = jvm_core::interpreter::Vm::new();
    jvm_core::load_bundle(&mut vm, shim_bundle());
    vm.load_jar(jar_loader_test_jar()).expect("load_jar failed");
    assert!(vm.resources.contains_key("resource.txt"), "resource.txt not found");
    let data = vm.resources.get("resource.txt").unwrap();
    let text = std::str::from_utf8(data).unwrap().trim();
    assert_eq!(text, "hello from jar resource");
}

#[test]
fn jar_to_bundle_roundtrip() {
    let bundle = jvm_core::jar_to_bundle_native(jar_loader_test_jar());
    assert!(!bundle.is_empty(), "jar_to_bundle returned empty");
    let result = jvm_core::run_static_native(
        &[shim_bundle(), &bundle].concat(),
        "JarTestEntry",
        "run",
        "()Ljava/lang/String;",
    );
    assert_eq!(result, "jar-ok");
}

// ---------------------------------------------------------------------------
// Clojure smoke: AOT-compiled ClojureSmokeEntry.run() → "ok"
// Requires: ./build-clj-smoke.sh (builds clj-smoke/smoke.jar)
// ---------------------------------------------------------------------------

#[test]
#[ignore] // Slow (~44s). Run explicitly: cargo test --package jvm-core clojure_smoke -- --ignored
fn clojure_smoke() {
    let smoke_jar = std::path::Path::new("../clj-smoke/smoke.jar");
    let jars_list = std::path::Path::new("../clj-smoke/clojure-jars.txt");
    if !smoke_jar.exists() || !jars_list.exists() {
        panic!("Run ./build-clj-smoke.sh first");
        return;
    }

    let mut vm = jvm_core::interpreter::Vm::new();
    jvm_core::load_bundle(&mut vm, shim_bundle());

    // Load smoke JAR
    let smoke_data = std::fs::read(smoke_jar).expect("read smoke.jar");
    vm.load_jar(&smoke_data).expect("load smoke.jar");

    // Load Clojure runtime JARs
    let jar_paths = std::fs::read_to_string(jars_list).expect("read jars list");
    for line in jar_paths.lines() {
        let line = line.trim();
        if line.is_empty() { continue; }
        let data = std::fs::read(line).unwrap_or_else(|e| panic!("read {line}: {e}"));
        vm.load_jar(&data).unwrap_or_else(|e| panic!("load {line}: {e}"));
    }

    // Run ClojureSmokeEntry.run() → "ok"
    let result = vm.invoke_static_threaded(
        "ClojureSmokeEntry", "run", "()Ljava/lang/String;", vec![],
    );
    vm.flush_printstreams();
    match result {
        Ok(jvm_core::heap::JValue::Ref(Some(r))) => {
            let s = r.borrow().as_java_string().unwrap_or_default().to_owned();
            assert_eq!(s, "ok", "ClojureSmokeEntry.run() returned {s:?}");
        }
        Ok(other) => panic!("unexpected return: {other:?}"),
        Err(e) => panic!("Clojure smoke failed: {e}"),
    }
}

// ---------------------------------------------------------------------------
// Boolean.TYPE — triggers Class.getPrimitiveClass native stub
// ---------------------------------------------------------------------------

#[test]
fn boolean_type() {
    let result = run_jar_test(
        "BooleanTypeTest",
        "run",
        "()Ljava/lang/String;",
    );
    assert_eq!(result, "boolean");
}

// ---------------------------------------------------------------------------
// IO.println — verifies PrintStream native bridge does not silently fail
// ---------------------------------------------------------------------------

#[test]
fn io_println() {
    let result = run_jar_test(
        "IOPrintlnTest",
        "run",
        "()Ljava/lang/String;",
    );
    assert_eq!(result, "ok");
}
