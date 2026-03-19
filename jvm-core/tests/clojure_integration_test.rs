//! Dedicated slow-path Clojure validation tests.
//!
//! These are intentionally separate from the default JVM integration suite so
//! they do not depend on `test-classes/test.jar` or `make test-bundle`.

fn shim_bundle() -> &'static [u8] {
    include_bytes!("../../jdk-shim/bundle.bin")
}

fn framed_jars_from_paths(paths: &[&std::path::Path]) -> Vec<u8> {
    let mut out = Vec::new();
    for path in paths {
        let jar = std::fs::read(path)
            .unwrap_or_else(|e| panic!("read {}: {e}", path.display()));
        let len = jar.len() as u32;
        out.extend_from_slice(&len.to_be_bytes());
        out.extend_from_slice(&jar);
    }
    out
}

fn read_jar_list(path: &std::path::Path) -> Vec<std::path::PathBuf> {
    let base_dir = path.parent().unwrap_or_else(|| std::path::Path::new("."));
    std::fs::read_to_string(path)
        .unwrap_or_else(|e| panic!("read {}: {e}", path.display()))
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .map(|line| {
            let jar_path = std::path::PathBuf::from(line);
            if jar_path.is_absolute() {
                jar_path
            } else {
                base_dir.join(jar_path)
            }
        })
        .collect()
}

fn load_jars_from_list(vm: &mut jvm_core::interpreter::Vm, path: &std::path::Path) {
    for jar_path in read_jar_list(path) {
        let data = std::fs::read(&jar_path)
            .unwrap_or_else(|e| panic!("read {}: {e}", jar_path.display()));
        vm.load_jar(&data)
            .unwrap_or_else(|e| panic!("load {}: {e}", jar_path.display()));
    }
}

fn pump_process_to_exit(
    process: &mut jvm_core::JvmProcess,
    max_pumps: usize,
) -> jvm_core::ProcessExit {
    for _ in 0..max_pumps {
        match process.pump(256) {
            jvm_core::ProcessState::Running => {}
            jvm_core::ProcessState::WaitingForInput => {
                panic!("process unexpectedly blocked on stdin");
            }
            jvm_core::ProcessState::Exited => {
                return process.exit().cloned().expect("process exit");
            }
        }
    }
    panic!("process did not exit after {max_pumps} pump iterations");
}

#[test]
#[ignore] // Slow (~44s). Run explicitly: cargo test --package jvm-core --test clojure_integration_test clojure_smoke -- --ignored --exact
fn clojure_smoke() {
    let smoke_jar = std::path::Path::new("../clj-smoke/smoke.jar");
    let jars_list = std::path::Path::new("../clj-smoke/clojure-jars.txt");
    if !smoke_jar.exists() || !jars_list.exists() {
        panic!("Run ./build-clj-smoke.sh first");
    }

    let mut vm = jvm_core::interpreter::Vm::new();
    jvm_core::load_bundle(&mut vm, shim_bundle());

    let smoke_data = std::fs::read(smoke_jar).expect("read smoke.jar");
    vm.load_jar(&smoke_data).expect("load smoke.jar");
    load_jars_from_list(&mut vm, jars_list);

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

#[test]
#[ignore] // Slow. Run explicitly: cargo test --package jvm-core --test clojure_integration_test clojure_smoke_upstream_subset -- --ignored --exact
fn clojure_smoke_upstream_subset() {
    let upstream_jar = std::path::Path::new("../clj-smoke/upstream-tests.jar");
    let jars_list = std::path::Path::new("../clj-smoke/clojure-jars.txt");
    if !upstream_jar.exists() || !jars_list.exists() {
        panic!("Run ./build-clj-smoke.sh first");
    }

    let mut classpath = read_jar_list(jars_list);
    classpath.push(upstream_jar.to_path_buf());
    let classpath_refs: Vec<&std::path::Path> = classpath.iter().map(|path| path.as_path()).collect();
    let jar_data = framed_jars_from_paths(&classpath_refs);

    let mut process = jvm_core::launch_classpath_main_native(
        shim_bundle(),
        &jar_data,
        "ClojureUpstreamTestEntry",
        &[],
        jvm_core::StdioMode::Ignore,
        jvm_core::StdioMode::Pipe,
        jvm_core::StdioMode::Pipe,
    )
    .expect("launch upstream Clojure tests");

    let exit = pump_process_to_exit(&mut process, 32_768);
    let stdout = String::from_utf8(process.take_stdout()).expect("utf8 stdout");
    let stderr = String::from_utf8(process.take_stderr()).expect("utf8 stderr");

    assert_eq!(
        exit.uncaught_exception,
        None,
        "unexpected uncaught exception\nstdout:\n{stdout}\nstderr:\n{stderr}"
    );
    assert_eq!(
        exit.exit_code,
        0,
        "upstream Clojure tests failed\nstdout:\n{stdout}\nstderr:\n{stderr}"
    );
    assert!(
        stdout.lines().any(|line| line.starts_with("ok namespaces=")),
        "missing upstream success marker\nstdout:\n{stdout}\nstderr:\n{stderr}"
    );
}
