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

fn framed_jars(jars: &[&[u8]]) -> Vec<u8> {
    let mut out = Vec::new();
    for jar in jars {
        let len = jar.len() as u32;
        out.extend_from_slice(&len.to_be_bytes());
        out.extend_from_slice(jar);
    }
    out
}

fn pump_process_to_exit(
    process: &mut jvm_core::JvmProcess,
    max_iters: usize,
    pump_rounds: usize,
) -> jvm_core::ProcessExit {
    for _ in 0..max_iters {
        match process.pump(pump_rounds) {
            jvm_core::ProcessState::Running => {}
            jvm_core::ProcessState::WaitingForInput => {
                panic!("process unexpectedly blocked on stdin");
            }
            jvm_core::ProcessState::Exited => {
                return process.exit().cloned().expect("process exit");
            }
        }
    }
    panic!("process did not exit after {max_iters} iterations with pump_rounds={pump_rounds}");
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
fn try_catch_checked_exception() {
    let result = run_jar_test(
        "TryCatchCheckedExceptionTest",
        "run",
        "()Ljava/lang/String;",
    );
    assert_eq!(result, "java.io.FileNotFoundException");
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
// AccessController overloads
// ---------------------------------------------------------------------------

#[test]
fn access_controller_action_overloads() {
    let result = run_jar_test(
        "AccessControllerShimTest",
        "runActionOverloads",
        "()Ljava/lang/String;",
    );
    assert_eq!(result, "one|two|three|true");
}

#[test]
fn access_controller_exception_wrapper_reaches_process_boundary() {
    let jar_data = framed_jars(&[test_jar()]);
    let mut process = jvm_core::launch_classpath_main_native(
        shim_bundle(),
        &jar_data,
        "AccessControllerExceptionProcessMain",
        &[],
        jvm_core::StdioMode::Ignore,
        jvm_core::StdioMode::Pipe,
        jvm_core::StdioMode::Pipe,
    )
    .expect("launch classpath main");

    let exit = pump_process_to_exit(&mut process, 4096, 64);
    let stdout = String::from_utf8(process.take_stdout()).unwrap();
    let stderr = String::from_utf8(process.take_stderr()).unwrap();

    assert!(stdout.is_empty());
    assert!(stderr.is_empty());
    assert_eq!(exit.exit_code, 1);
    let uncaught = exit
        .uncaught_exception
        .expect("expected uncaught PrivilegedActionException at process boundary");
    assert!(uncaught.contains("java/security/PrivilegedActionException"));
    assert!(uncaught.contains("java/io/IOException: boom"));
}

#[test]
fn system_exit_sets_process_exit_code_without_uncaught_exception() {
    let jar_data = framed_jars(&[test_jar()]);
    let mut process = jvm_core::launch_classpath_main_native(
        shim_bundle(),
        &jar_data,
        "SystemExitProcessMain",
        &[],
        jvm_core::StdioMode::Ignore,
        jvm_core::StdioMode::Pipe,
        jvm_core::StdioMode::Pipe,
    )
    .expect("launch classpath main");

    let exit = pump_process_to_exit(&mut process, 4096, 64);
    let stdout = String::from_utf8(process.take_stdout()).unwrap();
    let stderr = String::from_utf8(process.take_stderr()).unwrap();

    assert!(stdout.is_empty());
    assert!(stderr.is_empty());
    assert_eq!(exit.exit_code, 7);
    assert_eq!(exit.uncaught_exception, None);
}

// ---------------------------------------------------------------------------
// Random / SecureRandom shim behavior and Collections.shuffle(Random)
// ---------------------------------------------------------------------------

#[test]
fn seeded_random_shims() {
    let result = run_jar_test(
        "RandomShimTest",
        "runSeededRandom",
        "()Ljava/lang/String;",
    );
    assert_eq!(result, "true|true|true|[2, 4, 5, 1, 3]");
}

#[test]
fn secure_random_shim_api() {
    let result = run_jar_test(
        "RandomShimTest",
        "runSecureRandomApi",
        "()Ljava/lang/String;",
    );
    assert_eq!(result, "true|true|true");
}

#[test]
fn stack_shim_api() {
    let result = run_jar_test(
        "StackShimTest",
        "run",
        "()Ljava/lang/String;",
    );
    assert_eq!(result, "alpha|beta|beta|2|beta|false|alpha|true");
}

// ---------------------------------------------------------------------------
// Double.parseDouble / Float.parseFloat shim behavior
// ---------------------------------------------------------------------------

#[test]
fn parse_number_shims() {
    let result = run_jar_test(
        "ParseNumbersTest",
        "run",
        "()Ljava/lang/String;",
    );
    assert_eq!(result, "3.5|2.25|true|true|true");
}

#[test]
fn floating_wrapper_instance_predicates() {
    let result = run_jar_test(
        "DoubleInstanceMethodsTest",
        "run",
        "()Ljava/lang/String;",
    );
    assert_eq!(result, "true|true|true|true");
}

#[test]
fn objects_null_predicates() {
    let result = run_jar_test(
        "ObjectsPredicatesTest",
        "run",
        "()Ljava/lang/String;",
    );
    assert_eq!(result, "true|false|false|true");
}

#[test]
fn integer_bit_intrinsics() {
    let result = run_jar_test(
        "IntegerBitIntrinsicsTest",
        "run",
        "()Ljava/lang/String;",
    );
    assert_eq!(
        result,
        "16|27|4|32|1|67305985|32|59|4|256|1|578437695752307201"
    );
}

// ---------------------------------------------------------------------------
// getstatic/putstatic must resolve the declaring owner, not just the symbolic one
// ---------------------------------------------------------------------------

#[test]
fn inherited_static_field_uses_declaring_owner() {
    let result = run_jar_test(
        "InheritedStaticFieldTest",
        "run",
        "()Ljava/lang/String;",
    );
    assert_eq!(result, "1|7|7");
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

#[test]
fn classloader_resource_streams_are_independent() {
    let mut vm = jvm_core::interpreter::Vm::new();
    jvm_core::load_bundle(&mut vm, shim_bundle());
    vm.load_jar(jar_loader_test_jar()).expect("load test jar");

    let class_loader = match vm
        .invoke_static("java/lang/ClassLoader", "getSystemClassLoader", "()Ljava/lang/ClassLoader;", vec![])
        .expect("getSystemClassLoader")
    {
        jvm_core::heap::JValue::Ref(Some(r)) => r,
        other => panic!("unexpected class loader result: {other:?}"),
    };
    let arg = jvm_core::heap::JValue::Ref(Some(vm.intern_string("resource.txt")));

    let left = match vm
        .invoke_virtual(
            class_loader.clone(),
            "java/lang/ClassLoader",
            "getResourceAsStream",
            "(Ljava/lang/String;)Ljava/io/InputStream;",
            vec![arg.clone()],
        )
        .expect("left stream")
    {
        jvm_core::heap::JValue::Ref(Some(r)) => r,
        other => panic!("unexpected left stream result: {other:?}"),
    };
    let right = match vm
        .invoke_virtual(
            class_loader,
            "java/lang/ClassLoader",
            "getResourceAsStream",
            "(Ljava/lang/String;)Ljava/io/InputStream;",
            vec![arg],
        )
        .expect("right stream")
    {
        jvm_core::heap::JValue::Ref(Some(r)) => r,
        other => panic!("unexpected right stream result: {other:?}"),
    };

    assert!(!std::rc::Rc::ptr_eq(&left, &right));
    let left_buf = match left.borrow().fields.get("buf") {
        Some(jvm_core::heap::JValue::Ref(Some(r))) => r.clone(),
        other => panic!("unexpected left buf: {other:?}"),
    };
    let right_buf = match right.borrow().fields.get("buf") {
        Some(jvm_core::heap::JValue::Ref(Some(r))) => r.clone(),
        other => panic!("unexpected right buf: {other:?}"),
    };
    assert!(std::rc::Rc::ptr_eq(&left_buf, &right_buf));
    assert_eq!(left.borrow().fields.get("pos").map(|v| v.as_int()), Some(0));
    assert_eq!(right.borrow().fields.get("pos").map(|v| v.as_int()), Some(0));
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
// JVMS §5.5: class init only pulls in superinterfaces with concrete instance methods
// ---------------------------------------------------------------------------

#[test]
fn class_init_skips_plain_superinterfaces() {
    let result = run_jar_test(
        "InterfaceClassInitSelectionTest",
        "run",
        "()Ljava/lang/String;",
    );
    assert_eq!(result, "D");
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

#[test]
fn interface_monomorphic_cache_updates_when_receiver_changes() {
    let result = run_jar_test(
        "InterfaceMonomorphicDispatchTest",
        "run",
        "()Ljava/lang/String;",
    );
    assert_eq!(result, "A|B|A");
}

#[test]
fn virtual_monomorphic_cache_updates_when_receiver_changes() {
    let result = run_jar_test(
        "VirtualMonomorphicDispatchTest",
        "run",
        "()Ljava/lang/String;",
    );
    assert_eq!(result, "A|B|A");
}

#[test]
fn reflection_includes_interface_default_methods() {
    let result = run_jar_test(
        "ReflectionInterfaceDefaultMethodTest",
        "run",
        "()Ljava/lang/String;",
    );
    assert_eq!(result, "iface|true");
}

#[test]
fn reflection_method_arrays_are_cached_but_not_aliased() {
    let result = run_jar_test(
        "ReflectionMethodArrayIsolationTest",
        "run",
        "()Ljava/lang/String;",
    );
    assert_eq!(result, "true|true|true");
}

#[test]
fn reflection_invocation_target_exception_exposes_cause() {
    let result = run_jar_test(
        "ReflectionInvocationCauseTest",
        "run",
        "()Ljava/lang/String;",
    );
    assert_eq!(
        result,
        "java.lang.reflect.InvocationTargetException|ReflectionInvocationCauseTest$Boom:boom"
    );
}

#[test]
fn reflection_invoke_exception_can_be_unwrapped_and_caught() {
    let result = run_jar_test(
        "ReflectionInvocationCatchUnwrapTest",
        "run",
        "()Ljava/lang/String;",
    );
    assert_eq!(result, "ReflectionInvocationCatchUnwrapTest$Cookies:wrapped");
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
fn thread_get_stack_trace_shim() {
    let result = run_jar_test(
        "ThreadStackTraceShimTest",
        "run",
        "()Ljava/lang/String;",
    );
    assert_eq!(result, "len=0");
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
fn method_handle_shims() {
    let result = run_jar_test("MethodHandleShimTest", "run", "()Ljava/lang/String;");
    assert_eq!(result, "canAccess|()V|(I[Ljava/lang/Object;)Ljava/lang/String;|(Ljava/lang/Object;J)I");
}

#[test]
fn regex_capture_groups() {
    let result = run_jar_test("RegexGroupsTest", "run", "()Ljava/lang/String;");
    assert_eq!(result, "3|1.12.0|1|12|0");
}

#[test]
fn regex_find_escaped_parens() {
    let result = run_jar_test("RegexFindEscapedParensTest", "run", "()Ljava/lang/String;");
    assert_eq!(result, "true|Wrong number of args (0) passed to: :kw|false");
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
    assert!(vm.has_resource("resource.txt"), "resource.txt not found");
    assert!(!vm.resources.contains_key("resource.txt"),
        "resource.txt should remain compressed until first access");
    let data = vm.read_resource("resource.txt")
        .expect("read_resource failed")
        .expect("resource.txt missing");
    assert!(vm.resources.contains_key("resource.txt"),
        "resource.txt should be cached after first access");
    let text = std::str::from_utf8(&data).unwrap().trim();
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

#[test]
fn launcher_process_pipe_roundtrip() {
    let jar_data = framed_jars(&[test_jar()]);
    let mut process = jvm_core::launch_classpath_main_native(
        shim_bundle(),
        &jar_data,
        "ProcessLauncherEchoMain",
        &["pipe".to_owned()],
        jvm_core::StdioMode::Pipe,
        jvm_core::StdioMode::Pipe,
        jvm_core::StdioMode::Pipe,
    )
    .expect("launch classpath main");

    let mut state = "running";
    for _ in 0..128 {
        match process.pump(64) {
            jvm_core::ProcessState::Running => {}
            jvm_core::ProcessState::WaitingForInput => {
                state = "waiting";
                break;
            }
            jvm_core::ProcessState::Exited => {
                state = "exited";
                break;
            }
        }
    }
    assert_eq!(state, "waiting", "process did not block on stdin");
    assert_eq!(String::from_utf8(process.take_stdout()).unwrap(), "pipe>");
    assert!(process.take_stderr().is_empty());

    process.write_stdin(b"abc!");

    state = "running";
    for _ in 0..128 {
        match process.pump(64) {
            jvm_core::ProcessState::Running | jvm_core::ProcessState::WaitingForInput => {}
            jvm_core::ProcessState::Exited => {
                state = "exited";
                break;
            }
        }
    }
    assert_eq!(state, "exited", "process did not exit after stdin payload");
    assert_eq!(String::from_utf8(process.take_stdout()).unwrap(), "abc");
    assert_eq!(String::from_utf8(process.take_stderr()).unwrap(), "bang");

    let exit = process.exit().expect("process exit");
    assert_eq!(exit.exit_code, 0);
    assert_eq!(exit.uncaught_exception, None);
}

#[test]
fn launcher_process_pipe_eof() {
    let jar_data = framed_jars(&[test_jar()]);
    let mut process = jvm_core::launch_classpath_main_native(
        shim_bundle(),
        &jar_data,
        "ProcessLauncherEchoMain",
        &["eof".to_owned()],
        jvm_core::StdioMode::Pipe,
        jvm_core::StdioMode::Pipe,
        jvm_core::StdioMode::Pipe,
    )
    .expect("launch classpath main");

    for _ in 0..128 {
        if matches!(process.pump(64), jvm_core::ProcessState::WaitingForInput) {
            break;
        }
    }
    assert_eq!(String::from_utf8(process.take_stdout()).unwrap(), "eof>");

    process.close_stdin();

    let mut exited = false;
    for _ in 0..128 {
        if matches!(process.pump(64), jvm_core::ProcessState::Exited) {
            exited = true;
            break;
        }
    }
    assert!(exited, "process did not exit after stdin EOF");
    assert_eq!(String::from_utf8(process.take_stdout()).unwrap(), "<eof>");
    assert!(process.take_stderr().is_empty());

    let exit = process.exit().expect("process exit");
    assert_eq!(exit.exit_code, 0);
    assert_eq!(exit.uncaught_exception, None);
}

#[test]
fn launcher_process_pipe_byte_writes() {
    let jar_data = framed_jars(&[test_jar()]);
    let mut process = jvm_core::launch_classpath_main_native(
        shim_bundle(),
        &jar_data,
        "ProcessLauncherEchoMain",
        &["bytes".to_owned()],
        jvm_core::StdioMode::Ignore,
        jvm_core::StdioMode::Pipe,
        jvm_core::StdioMode::Pipe,
    )
    .expect("launch classpath main");

    let mut exited = false;
    for _ in 0..128 {
        if matches!(process.pump(64), jvm_core::ProcessState::Exited) {
            exited = true;
            break;
        }
    }
    assert!(exited, "process did not exit after raw byte writes");
    assert_eq!(String::from_utf8(process.take_stdout()).unwrap(), "ABC");
    assert_eq!(String::from_utf8(process.take_stderr()).unwrap(), "DEF");

    let exit = process.exit().expect("process exit");
    assert_eq!(exit.exit_code, 0);
    assert_eq!(exit.uncaught_exception, None);
}

#[test]
fn launcher_process_pipe_close_system_streams() {
    let jar_data = framed_jars(&[test_jar()]);
    let mut process = jvm_core::launch_classpath_main_native(
        shim_bundle(),
        &jar_data,
        "ProcessLauncherEchoMain",
        &["close".to_owned()],
        jvm_core::StdioMode::Ignore,
        jvm_core::StdioMode::Pipe,
        jvm_core::StdioMode::Pipe,
    )
    .expect("launch classpath main");

    let mut exited = false;
    for _ in 0..128 {
        if matches!(process.pump(64), jvm_core::ProcessState::Exited) {
            exited = true;
            break;
        }
    }
    assert!(exited, "process did not exit after closing system streams");
    assert_eq!(String::from_utf8(process.take_stdout()).unwrap(), "A");
    assert_eq!(String::from_utf8(process.take_stderr()).unwrap(), "B");

    let exit = process.exit().expect("process exit");
    assert_eq!(exit.exit_code, 0);
    assert_eq!(exit.uncaught_exception, None);
}

#[test]
fn launcher_process_pipe_write_after_close_is_ignored() {
    let jar_data = framed_jars(&[test_jar()]);
    let mut process = jvm_core::launch_classpath_main_native(
        shim_bundle(),
        &jar_data,
        "ProcessLauncherEchoMain",
        &["write-after-close".to_owned()],
        jvm_core::StdioMode::Ignore,
        jvm_core::StdioMode::Pipe,
        jvm_core::StdioMode::Pipe,
    )
    .expect("launch classpath main");

    let mut exited = false;
    for _ in 0..128 {
        if matches!(process.pump(64), jvm_core::ProcessState::Exited) {
            exited = true;
            break;
        }
    }
    assert!(exited, "process did not exit after writing to closed system streams");
    assert_eq!(String::from_utf8(process.take_stdout()).unwrap(), "A");
    assert_eq!(String::from_utf8(process.take_stderr()).unwrap(), "B");

    let exit = process.exit().expect("process exit");
    assert_eq!(exit.exit_code, 0);
    assert_eq!(exit.uncaught_exception, None);
}

#[test]
fn launcher_process_clinit_yields_before_static_side_effects() {
    let jar_data = framed_jars(&[test_jar()]);
    let mut process = jvm_core::launch_classpath_main_native(
        shim_bundle(),
        &jar_data,
        "ClinitYieldProcessMain",
        &[],
        jvm_core::StdioMode::Ignore,
        jvm_core::StdioMode::Pipe,
        jvm_core::StdioMode::Pipe,
    )
    .expect("launch classpath main");

    let first_state = process.pump(1);
    assert_eq!(first_state, jvm_core::ProcessState::Running);
    assert_eq!(String::from_utf8(process.take_stdout()).unwrap(), "");
    assert_eq!(String::from_utf8(process.take_stderr()).unwrap(), "");

    let exit = pump_process_to_exit(&mut process, 4096, 64);
    let stdout = String::from_utf8(process.take_stdout()).unwrap();
    let stderr = String::from_utf8(process.take_stderr()).unwrap();

    assert_eq!(stdout, "Idone");
    assert!(stderr.is_empty());
    assert_eq!(exit.exit_code, 0);
    assert_eq!(exit.uncaught_exception, None);
}

#[test]
fn launcher_process_superclass_clinit_chain_yields_before_static_side_effects() {
    let jar_data = framed_jars(&[test_jar()]);
    let mut process = jvm_core::launch_classpath_main_native(
        shim_bundle(),
        &jar_data,
        "ClinitChainYieldProcessMain",
        &[],
        jvm_core::StdioMode::Ignore,
        jvm_core::StdioMode::Pipe,
        jvm_core::StdioMode::Pipe,
    )
    .expect("launch classpath main");

    let first_state = process.pump(1);
    assert_eq!(first_state, jvm_core::ProcessState::Running);
    assert_eq!(String::from_utf8(process.take_stdout()).unwrap(), "");
    assert_eq!(String::from_utf8(process.take_stderr()).unwrap(), "");

    let exit = pump_process_to_exit(&mut process, 4096, 64);
    let stdout = String::from_utf8(process.take_stdout()).unwrap();
    let stderr = String::from_utf8(process.take_stderr()).unwrap();

    assert_eq!(stdout, "BMLbase:mid:leaf");
    assert!(stderr.is_empty());
    assert_eq!(exit.exit_code, 0);
    assert_eq!(exit.uncaught_exception, None);
}

#[test]
fn launcher_process_rejects_invalid_classpath_jar() {
    let jar_data = framed_jars(&[b"not-a-jar"]);
    let err = match jvm_core::launch_classpath_main_native(
        shim_bundle(),
        &jar_data,
        "ProcessLauncherEchoMain",
        &[],
        jvm_core::StdioMode::Pipe,
        jvm_core::StdioMode::Pipe,
        jvm_core::StdioMode::Pipe,
    ) {
        Ok(_) => panic!("launch should fail for invalid classpath jar"),
        Err(err) => err,
    };
    assert!(
        err.contains("launchClasspathMain failed during classpath load: Failed to load classpath JAR #0"),
        "unexpected error: {err}"
    );
}

#[test]
fn printstream_non_marker_still_uses_underlying_stream() {
    let result = run_jar_test("PrintStreamNonMarkerTest", "run", "()Ljava/lang/String;");
    assert_eq!(result, "AB\\n|true");
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
