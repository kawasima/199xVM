//! Focused integration tests for the ClassId/class-lifecycle refactor.

fn shim_bundle() -> &'static [u8] {
    include_bytes!("../../jdk-shim/bundle.bin")
}

fn test_jar() -> &'static [u8] {
    include_bytes!("../../test-classes/test.jar")
}

fn run_jar_test(class: &str, method: &str, descriptor: &str) -> String {
    let mut vm = jvm_core::interpreter::Vm::new();
    jvm_core::load_bundle(&mut vm, shim_bundle());
    vm.load_jar(test_jar()).expect("failed to load test JAR");
    let result = vm.invoke_static_threaded(class, method, descriptor, vec![]);
    vm.flush_printstreams();
    match result {
        Ok(v) => jvalue_to_string(&v),
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

#[test]
fn class_for_name_array_and_repeat_init() {
    let result = run_jar_test("ClassForNameArrayInitTest", "run", "()Ljava/lang/String;");
    assert_eq!(result, "array-ok|1");
}

#[test]
fn static_access_triggers_class_init_once() {
    let result = run_jar_test("StaticAccessInitTest", "run", "()Ljava/lang/String;");
    assert_eq!(result, "41|42|1");
}

#[test]
fn find_loaded_class_requires_actual_loaded_state() {
    let result = run_jar_test("FindLoadedClassStateTest", "run", "()Ljava/lang/String;");
    assert_eq!(result, "true|true|true");
}

#[test]
fn lifecycle_profile_reports_class_counters() {
    let mut vm = jvm_core::interpreter::Vm::new();
    vm.enable_profiler();
    jvm_core::load_bundle(&mut vm, shim_bundle());
    vm.load_jar(test_jar()).expect("failed to load test JAR");
    vm.reset_profiler();

    let result = vm
        .invoke_static_threaded("StaticAccessInitTest", "run", "()Ljava/lang/String;", vec![])
        .expect("run StaticAccessInitTest");
    assert_eq!(jvalue_to_string(&result), "41|42|1");

    let report = vm.take_profile_report().expect("profile report");
    println!("{report}");
    assert!(
        report.contains("class.identity_lookup")
            || report.contains("class.identity_lookup.fast"),
        "missing identity lookup counter:\n{report}"
    );
    assert!(
        report.contains("class.prepare.transition"),
        "missing prepare transition counter:\n{report}"
    );
    assert!(
        report.contains("class.init.check"),
        "missing init check counter:\n{report}"
    );
    assert!(
        report.contains("class.init.clinit_run"),
        "missing clinit execution counter:\n{report}"
    );
}

#[test]
fn class_mirror_identity_and_counters_are_reported() {
    let mut vm = jvm_core::interpreter::Vm::new();
    vm.enable_profiler();
    jvm_core::load_bundle(&mut vm, shim_bundle());
    vm.load_jar(test_jar()).expect("failed to load test JAR");
    vm.reset_profiler();

    let result = vm
        .invoke_static_threaded("ClassMirrorIdentityTest", "run", "()Ljava/lang/String;", vec![])
        .expect("run ClassMirrorIdentityTest");
    assert_eq!(
        jvalue_to_string(&result),
        "true|ClassMirrorIdentityTest$Base|1|true|true"
    );

    let report = vm.take_profile_report().expect("profile report");
    println!("{report}");
    assert!(
        report.contains("class.mirror.pool.")
            || report.contains("class.mirror.id."),
        "missing class mirror counters:\n{report}"
    );
}

#[test]
fn repeated_for_name_profile_shows_hot_path_hits() {
    let mut vm = jvm_core::interpreter::Vm::new();
    vm.enable_profiler();
    jvm_core::load_bundle(&mut vm, shim_bundle());
    vm.load_jar(test_jar()).expect("failed to load test JAR");
    vm.reset_profiler();

    let result = vm
        .invoke_static_threaded(
            "RepeatedForNameProfileTest",
            "run",
            "()Ljava/lang/String;",
            vec![],
        )
        .expect("run RepeatedForNameProfileTest");
    assert_eq!(jvalue_to_string(&result), "true|1");

    let report = vm.take_profile_report().expect("profile report");
    println!("{report}");
    assert!(
        report.contains("RepeatedForNameProfileTest$Target"),
        "missing forName stats for target class:\n{report}"
    );
    assert!(
        report.contains("class.mirror.pool.hit"),
        "missing mirror pool hit counter:\n{report}"
    );
    assert!(
        report.contains("class.init.clinit_run"),
        "missing class init execution counter:\n{report}"
    );
}
