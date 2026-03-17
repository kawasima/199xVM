//! Integration tests for the 199xVM.
//!
//! These tests load pre-compiled `.class` bundles and verify bytecode execution
//! without requiring a browser or WASM runtime.
//!
//! Prerequisites:
//!   ./build-shim.sh          (builds jdk-shim/bundle.bin)
//!   ./build-test-bundle.sh   (builds test-classes/bundle.bin)

/// Combine shim + app bundles.
fn combined_bundle(shim: &[u8], app: &[u8]) -> Vec<u8> {
    let mut combined = Vec::with_capacity(shim.len() + app.len());
    combined.extend_from_slice(shim);
    combined.extend_from_slice(app);
    combined
}

fn shim_bundle() -> &'static [u8] {
    include_bytes!("../../jdk-shim/bundle.bin")
}

fn test_bundle() -> &'static [u8] {
    include_bytes!("../../test-classes/bundle.bin")
}

fn test_jar() -> &'static [u8] {
    include_bytes!("test.jar")
}

// ---------------------------------------------------------------------------
// Integer.toString via shim bytecode
// ---------------------------------------------------------------------------

#[test]
fn integer_tostring() {
    let bundle = combined_bundle(shim_bundle(), test_bundle());
    let result = jvm_core::run_static_native(
        &bundle,
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
    let bundle = combined_bundle(shim_bundle(), test_bundle());
    let result = jvm_core::run_static_native(
        &bundle,
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
    let bundle = combined_bundle(shim_bundle(), test_bundle());
    let result = jvm_core::run_static_native(
        &bundle,
        "LocalDateTimeNowTest",
        "run",
        "()Ljava/lang/String;",
    );
    assert_eq!(result, "ok");
}

// ---------------------------------------------------------------------------

#[test]
fn arraylist_basics() {
    let bundle = combined_bundle(shim_bundle(), test_bundle());
    let result = jvm_core::run_static_native(
        &bundle,
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
    let bundle = combined_bundle(shim_bundle(), test_bundle());
    let result = jvm_core::run_static_native(
        &bundle,
        "TryCatchTest",
        "run",
        "()Ljava/lang/String;",
    );
    assert_eq!(result, "CAUGHT");
}

#[test]
fn factorial_long() {
    let bundle = combined_bundle(shim_bundle(), test_bundle());
    let result = jvm_core::run_static_native(
        &bundle,
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
    let bundle = combined_bundle(shim_bundle(), test_bundle());
    let result = jvm_core::run_static_native(
        &bundle,
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
    let bundle = combined_bundle(shim_bundle(), test_bundle());
    let result = jvm_core::run_static_native(
        &bundle,
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
    let bundle = combined_bundle(shim_bundle(), test_bundle());
    let result = jvm_core::run_static_native(
        &bundle,
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
    let bundle = combined_bundle(shim_bundle(), test_bundle());
    let result = jvm_core::run_static_native(
        &bundle,
        "SynchronizedTest",
        "run",
        "()Ljava/lang/String;",
    );
    assert_eq!(result, "13");
}

#[test]
fn synchronized_null_monitor_throws_npe() {
    let bundle = combined_bundle(shim_bundle(), test_bundle());
    let result = jvm_core::run_static_native(
        &bundle,
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
    let bundle = combined_bundle(shim_bundle(), test_bundle());
    let result = jvm_core::run_static_native(
        &bundle,
        "ClassLoaderTest",
        "run",
        "()Ljava/lang/String;",
    );
    assert_eq!(result, "cl:ok|forName:ok|loadClass:ok");
}

#[test]
fn classloader_missing_class_throws_cnfe() {
    let bundle = combined_bundle(shim_bundle(), test_bundle());
    let result = jvm_core::run_static_native(
        &bundle,
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
    let bundle = combined_bundle(shim_bundle(), test_bundle());
    let result = jvm_core::run_static_native(
        &bundle,
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
    let bundle = combined_bundle(shim_bundle(), test_bundle());
    let result = jvm_core::run_static_native(
        &bundle,
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
    let bundle = combined_bundle(shim_bundle(), test_bundle());
    let result = jvm_core::run_static_native(
        &bundle,
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
    let bundle = combined_bundle(shim_bundle(), test_bundle());
    let result = jvm_core::run_static_native(
        &bundle,
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
    let bundle = combined_bundle(shim_bundle(), test_bundle());
    let result = jvm_core::run_static_native(
        &bundle,
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
    let bundle = combined_bundle(shim_bundle(), test_bundle());
    let result = jvm_core::run_static_native(
        &bundle,
        "ThreadBasicTest",
        "run",
        "()Ljava/lang/String;",
    );
    assert_eq!(result, "ABC");
}

#[test]
fn monitor_contention_two_threads() {
    let bundle = combined_bundle(shim_bundle(), test_bundle());
    let result = jvm_core::run_static_native(
        &bundle,
        "MonitorContentionTest",
        "run",
        "()Ljava/lang/String;",
    );
    assert_eq!(result, "10");
}

#[test]
fn wait_notify_producer_consumer() {
    let bundle = combined_bundle(shim_bundle(), test_bundle());
    let result = jvm_core::run_static_native(
        &bundle,
        "WaitNotifyTest",
        "run",
        "()Ljava/lang/String;",
    );
    assert_eq!(result, "produced:42,consumed:42");
}

#[test]
fn notify_all_wakes_multiple_waiters() {
    let bundle = combined_bundle(shim_bundle(), test_bundle());
    let result = jvm_core::run_static_native(
        &bundle,
        "NotifyAllTest",
        "run",
        "()Ljava/lang/String;",
    );
    assert_eq!(result, "3");
}

#[test]
fn wait_without_lock_throws_imse() {
    let bundle = combined_bundle(shim_bundle(), test_bundle());
    let result = jvm_core::run_static_native(
        &bundle,
        "WaitWithoutLockTest",
        "run",
        "()Ljava/lang/String;",
    );
    assert_eq!(result, "wait:IMSE,notify:IMSE,notifyAll:IMSE");
}

#[test]
fn reentrant_wait_restores_count() {
    let bundle = combined_bundle(shim_bundle(), test_bundle());
    let result = jvm_core::run_static_native(
        &bundle,
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
    let bundle = combined_bundle(shim_bundle(), test_bundle());
    let result = jvm_core::run_static_native(
        &bundle,
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
    let bundle = combined_bundle(shim_bundle(), test_bundle());
    let result = jvm_core::run_static_native(
        &bundle,
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
    let bundle = combined_bundle(shim_bundle(), test_bundle());
    let result = jvm_core::run_static_native(
        &bundle,
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
    let bundle = combined_bundle(shim_bundle(), test_bundle());
    let result = jvm_core::run_static_native(
        &bundle,
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
    let bundle = combined_bundle(shim_bundle(), test_bundle());
    let result = jvm_core::run_static_native(
        &bundle,
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
    let bundle = combined_bundle(shim_bundle(), test_bundle());
    let result = jvm_core::run_static_native(
        &bundle,
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
    let bundle = combined_bundle(shim_bundle(), test_bundle());
    let result = jvm_core::run_static_native(
        &bundle,
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
    let bundle = combined_bundle(shim_bundle(), test_bundle());
    let result = jvm_core::run_static_native(
        &bundle,
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
    let bundle = combined_bundle(shim_bundle(), test_bundle());
    let result = jvm_core::run_static_native(
        &bundle,
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
    let bundle = combined_bundle(shim_bundle(), test_bundle());
    let result = jvm_core::run_static_native(
        &bundle,
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
    let bundle = combined_bundle(shim_bundle(), test_bundle());
    let result = jvm_core::run_static_native(
        &bundle,
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
    let bundle = combined_bundle(shim_bundle(), test_bundle());
    let result = jvm_core::run_static_native(
        &bundle,
        "PriorityQueueTest",
        "run",
        "()Ljava/lang/String;",
    );
    assert_eq!(result, "10,20,30");
}

// ---------------------------------------------------------------------------
// JAR loader: load classes from a JAR file
// ---------------------------------------------------------------------------

#[test]
fn load_jar_and_run() {
    let mut vm = jvm_core::interpreter::Vm::new();
    jvm_core::load_bundle(&mut vm, shim_bundle());
    let count = vm.load_jar(test_jar()).expect("load_jar failed");
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
    vm.load_jar(test_jar()).expect("load_jar failed");
    assert!(vm.resources.contains_key("resource.txt"), "resource.txt not found");
    let data = vm.resources.get("resource.txt").unwrap();
    let text = std::str::from_utf8(data).unwrap().trim();
    assert_eq!(text, "hello from jar resource");
}

#[test]
fn jar_to_bundle_roundtrip() {
    let bundle = jvm_core::jar_to_bundle_native(test_jar());
    assert!(!bundle.is_empty(), "jar_to_bundle returned empty");
    let result = jvm_core::run_static_native(
        &[shim_bundle(), &bundle].concat(),
        "JarTestEntry",
        "run",
        "()Ljava/lang/String;",
    );
    assert_eq!(result, "jar-ok");
}
