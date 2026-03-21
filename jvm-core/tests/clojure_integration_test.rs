//! Dedicated slow-path Clojure validation tests.
//!
//! These are intentionally separate from the default JVM integration suite so
//! they do not depend on `test-classes/test.jar` or `make test-bundle`.

const UPSTREAM_NAMESPACES: &[&str] = &[
    "clojure.test-clojure.atoms",
    "clojure.test-clojure.logic",
    "clojure.test-clojure.try-catch",
];

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

fn clj_smoke_artifact_hint() -> &'static str {
    "Run `make clj-smoke-bundle-docker` (preferred) or `./build-clj-smoke.sh` first"
}

fn selected_upstream_namespaces_from_runner() -> Vec<String> {
    let runner = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../test-sources/clojure/src/upstream/runner.clj");
    let source = std::fs::read_to_string(&runner)
        .unwrap_or_else(|e| panic!("read {}: {e}", runner.display()));
    let mut namespaces = Vec::new();
    for token in source.split_whitespace() {
        let token = token.trim_matches(|c: char| matches!(c, '\'' | '[' | ']' | '(' | ')' ));
        if token.starts_with("clojure.test-clojure.") && !namespaces.iter().any(|ns| ns == token) {
            namespaces.push(token.to_owned());
        }
    }
    namespaces
}

// Keep the upstream diagnostic lane responsive: per-test timeouts should fire
// under a minute even after increasing the single-thread interpreter time slice.
const UPSTREAM_MAX_PUMPS: usize = 4_096;
const UPSTREAM_PUMP_ROUNDS: usize = 128;
const UPSTREAM_MAX_ELAPSED_SECS: u64 = 58;

#[derive(Debug)]
struct PumpTimeout {
    max_pumps: usize,
    pump_rounds: usize,
    max_elapsed: std::time::Duration,
    elapsed: std::time::Duration,
}

fn pump_process_to_exit(
    process: &mut jvm_core::JvmProcess,
    max_pumps: usize,
    pump_rounds: usize,
    max_elapsed: std::time::Duration,
) -> Result<jvm_core::ProcessExit, PumpTimeout> {
    let start = std::time::Instant::now();
    for _ in 0..max_pumps {
        if start.elapsed() >= max_elapsed {
            return Err(PumpTimeout {
                max_pumps,
                pump_rounds,
                max_elapsed,
                elapsed: start.elapsed(),
            });
        }
        match process.pump(pump_rounds) {
            jvm_core::ProcessState::Running => {}
            jvm_core::ProcessState::WaitingForInput => {
                panic!("process unexpectedly blocked on stdin");
            }
            jvm_core::ProcessState::Exited => {
                return Ok(process.exit().cloned().expect("process exit"));
            }
        }
    }
    Err(PumpTimeout {
        max_pumps,
        pump_rounds,
        max_elapsed,
        elapsed: start.elapsed(),
    })
}

fn upstream_max_elapsed() -> std::time::Duration {
    let secs = std::env::var("UPSTREAM_MAX_ELAPSED_SECS")
        .ok()
        .and_then(|s| s.parse::<u64>().ok())
        .filter(|secs| *secs > 0)
        .unwrap_or(UPSTREAM_MAX_ELAPSED_SECS);
    std::time::Duration::from_secs(secs)
}

fn upstream_log_output() -> bool {
    matches!(
        std::env::var("UPSTREAM_LOG_OUTPUT").ok().as_deref(),
        Some("1" | "true" | "TRUE" | "yes" | "YES")
    )
}

#[test]
#[ignore] // Slow (~44s). Run explicitly: cargo test --package jvm-core --test clojure_integration_test clojure_smoke -- --ignored --exact
fn clojure_smoke() {
    let smoke_jar = std::path::Path::new("../clj-smoke/smoke.jar");
    let jars_list = std::path::Path::new("../clj-smoke/clojure-jars.txt");
    if !smoke_jar.exists() || !jars_list.exists() {
        panic!("{}", clj_smoke_artifact_hint());
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
fn upstream_runner_namespace_list_matches_expected() {
    let selected = selected_upstream_namespaces_from_runner();
    let expected: Vec<String> = UPSTREAM_NAMESPACES.iter().map(|ns| (*ns).to_owned()).collect();
    assert_eq!(selected, expected, "runner namespace selection changed");
}

fn run_upstream_selector(selector: &str) {
    let upstream_jar = std::path::Path::new("../clj-smoke/upstream-tests.jar");
    let jars_list = std::path::Path::new("../clj-smoke/clojure-jars.txt");
    if !upstream_jar.exists() || !jars_list.exists() {
        panic!("{}", clj_smoke_artifact_hint());
    }

    let mut classpath = read_jar_list(jars_list);
    classpath.push(upstream_jar.to_path_buf());
    let classpath_refs: Vec<&std::path::Path> = classpath.iter().map(|path| path.as_path()).collect();
    let jar_data = framed_jars_from_paths(&classpath_refs);

    let mut process = jvm_core::launch_classpath_main_native(
        shim_bundle(),
        &jar_data,
        "ClojureUpstreamTestEntry",
        &[selector.to_owned()],
        jvm_core::StdioMode::Ignore,
        jvm_core::StdioMode::Pipe,
        jvm_core::StdioMode::Pipe,
    )
    .expect("launch upstream Clojure tests");

    let max_elapsed = upstream_max_elapsed();
    let exit = match pump_process_to_exit(
        &mut process,
        UPSTREAM_MAX_PUMPS,
        UPSTREAM_PUMP_ROUNDS,
        max_elapsed,
    ) {
        Ok(exit) => exit,
        Err(timeout) => {
            let profile = process.profile_report();
            process.kill();
            let stdout = String::from_utf8(process.take_stdout()).expect("utf8 stdout");
            let stderr = String::from_utf8(process.take_stderr()).expect("utf8 stderr");
            let profile_suffix = profile
                .filter(|report| !stderr.contains(report))
                .map(|report| format!("\nprofile:\n{report}"))
                .unwrap_or_default();
            panic!(
                "upstream Clojure tests timed out\nmax_pumps={}\npump_rounds={}\nmax_elapsed={:?}\nelapsed={:?}\nstdout:\n{stdout}\nstderr:\n{stderr}{profile_suffix}",
                timeout.max_pumps,
                timeout.pump_rounds,
                timeout.max_elapsed,
                timeout.elapsed
            );
        }
    };
    let stdout = String::from_utf8(process.take_stdout()).expect("utf8 stdout");
    let stderr = String::from_utf8(process.take_stderr()).expect("utf8 stderr");

    if upstream_log_output() {
        eprintln!("upstream stdout for {selector}:\n{stdout}");
        eprintln!("upstream stderr for {selector}:\n{stderr}");
    }

    assert_eq!(
        exit.uncaught_exception,
        None,
        "unexpected uncaught exception for {selector}\nmax_pumps={UPSTREAM_MAX_PUMPS}\npump_rounds={UPSTREAM_PUMP_ROUNDS}\nmax_elapsed={max_elapsed:?}\nstdout:\n{stdout}\nstderr:\n{stderr}"
    );
    assert_eq!(
        exit.exit_code,
        0,
        "upstream Clojure tests failed for {selector}\nmax_pumps={UPSTREAM_MAX_PUMPS}\npump_rounds={UPSTREAM_PUMP_ROUNDS}\nmax_elapsed={max_elapsed:?}\nstdout:\n{stdout}\nstderr:\n{stderr}"
    );
    assert!(
        stdout.lines().any(|line| line.starts_with("ok namespaces=1")),
        "missing upstream success marker for {selector}\nstdout:\n{stdout}\nstderr:\n{stderr}"
    );
}

macro_rules! upstream_selector_test {
    ($test_name:ident, $selector:literal) => {
        #[test]
        #[ignore]
        fn $test_name() {
            run_upstream_selector($selector);
        }
    };
}

upstream_selector_test!(clojure_upstream_atoms, "clojure.test-clojure.atoms");
upstream_selector_test!(clojure_diag_control, "clojure.test-clojure.control");
upstream_selector_test!(
    clojure_diag_evaluation_eval,
    "clojure.test-clojure.evaluation/Eval"
);
upstream_selector_test!(
    clojure_diag_evaluation_literals,
    "clojure.test-clojure.evaluation/Literals"
);
upstream_selector_test!(
    clojure_diag_evaluation_symbol_resolution_qualified_vars,
    "upstream.evaluation-symbol-resolution/QualifiedVars"
);
upstream_selector_test!(
    clojure_diag_evaluation_symbol_resolution_qualified_classes,
    "upstream.evaluation-symbol-resolution/QualifiedClasses"
);
upstream_selector_test!(
    clojure_diag_evaluation_symbol_resolution_special_forms_a,
    "upstream.evaluation-symbol-resolution/LookupOrderSpecialFormsA"
);
upstream_selector_test!(
    clojure_diag_evaluation_symbol_resolution_special_forms_b,
    "upstream.evaluation-symbol-resolution/LookupOrderSpecialFormsB"
);
upstream_selector_test!(
    clojure_diag_evaluation_symbol_resolution_positive_class_mappings,
    "upstream.evaluation-symbol-resolution/LookupOrderPositiveClassMappings"
);
upstream_selector_test!(
    clojure_diag_evaluation_symbol_resolution_positive_local_binding,
    "upstream.evaluation-symbol-resolution/LookupOrderPositiveLocalBinding"
);
upstream_selector_test!(
    clojure_diag_evaluation_symbol_resolution_positive_current_namespace_var,
    "upstream.evaluation-symbol-resolution/LookupOrderPositiveCurrentNamespaceVar"
);
upstream_selector_test!(
    clojure_diag_evaluation_symbol_resolution_negative,
    "upstream.evaluation-symbol-resolution/LookupOrderNegative"
);
upstream_selector_test!(
    clojure_diag_evaluation_metadata,
    "clojure.test-clojure.evaluation/Metadata"
);
upstream_selector_test!(
    clojure_diag_evaluation_collections,
    "clojure.test-clojure.evaluation/Collections"
);
upstream_selector_test!(
    clojure_diag_evaluation_macros,
    "clojure.test-clojure.evaluation/Macros"
);
upstream_selector_test!(
    clojure_diag_evaluation_loading,
    "clojure.test-clojure.evaluation/Loading"
);
upstream_selector_test!(clojure_diag_fn_bad_arglists, "upstream.fn-bad-arglists");
upstream_selector_test!(clojure_diag_fn_signatures, "upstream.fn-signatures");
upstream_selector_test!(clojure_diag_fn_missing_params, "upstream.fn-missing-params");
upstream_selector_test!(clojure_diag_keywords, "clojure.test-clojure.keywords");
upstream_selector_test!(clojure_upstream_logic, "clojure.test-clojure.logic");
upstream_selector_test!(clojure_diag_macros, "clojure.test-clojure.macros");
upstream_selector_test!(clojure_diag_metadata, "clojure.test-clojure.metadata");
upstream_selector_test!(
    clojure_diag_other_functions,
    "clojure.test-clojure.other-functions"
);
upstream_selector_test!(clojure_diag_predicates, "clojure.test-clojure.predicates");
upstream_selector_test!(clojure_diag_special, "clojure.test-clojure.special");
upstream_selector_test!(clojure_diag_string, "clojure.test-clojure.string");
upstream_selector_test!(clojure_upstream_try_catch, "clojure.test-clojure.try-catch");
upstream_selector_test!(clojure_diag_vectors, "clojure.test-clojure.vectors");
