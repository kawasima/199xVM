//! Interpreter benchmarks for Issue #13 performance bottlenecks.
//!
//! Prerequisites:
//!   ./build-shim.sh          (builds jdk-shim/bundle.bin)
//!   ./build-test-bundle.sh   (builds test-classes/bench-bundle.bin)
//!
//! Run:
//!   cargo bench --package jvm-core

use criterion::{criterion_group, criterion_main, Criterion};

fn combined_bundle(shim: &[u8], app: &[u8]) -> Vec<u8> {
    let mut v = Vec::with_capacity(shim.len() + app.len());
    v.extend_from_slice(shim);
    v.extend_from_slice(app);
    v
}

fn shim_bundle() -> &'static [u8] {
    include_bytes!("../../jdk-shim/bundle.bin")
}

fn bench_bundle() -> &'static [u8] {
    include_bytes!("../../test-classes/bench-bundle.bin")
}

fn test_jar() -> &'static [u8] {
    include_bytes!("../../test-classes/test.jar")
}

fn jar_loader_test_jar() -> &'static [u8] {
    include_bytes!("../tests/test.jar")
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

/// Measures O(n) method lookup + constant pool clone per static call.
/// BenchMethodCall.run() calls add(s, i) 1000 times.
fn bench_method_call(c: &mut Criterion) {
    let bundle = combined_bundle(shim_bundle(), bench_bundle());
    c.bench_function("method_call_1000x", |b| {
        b.iter(|| jvm_core::run_static_native(&bundle, "BenchMethodCall", "run", "()I"))
    });
}

/// Measures getstatic/putstatic string key formatting cost.
/// BenchStaticField.run() reads/writes a static int field 1000 times.
fn bench_static_field(c: &mut Criterion) {
    let bundle = combined_bundle(shim_bundle(), bench_bundle());
    c.bench_function("static_field_1000x", |b| {
        b.iter(|| jvm_core::run_static_native(&bundle, "BenchStaticField", "run", "()I"))
    });
}

/// Measures inherited getstatic/putstatic when the symbolic owner differs from the declaring owner.
fn bench_inherited_static_field(c: &mut Criterion) {
    let bundle = combined_bundle(shim_bundle(), bench_bundle());
    c.bench_function("inherited_static_field_1000x", |b| {
        b.iter(|| {
            jvm_core::run_static_native(&bundle, "BenchInheritedStaticField", "run", "()I")
        })
    });
}

/// Measures ldc string constant clone cost.
/// BenchStringLdc.run() loads the string literal "hello" 1000 times.
fn bench_string_ldc(c: &mut Criterion) {
    let bundle = combined_bundle(shim_bundle(), bench_bundle());
    c.bench_function("string_ldc_1000x", |b| {
        b.iter(|| {
            jvm_core::run_static_native(&bundle, "BenchStringLdc", "run", "()Ljava/lang/String;")
        })
    });
}

/// Measures virtual dispatch + interface list rebuild cost.
/// BenchVirtualCall.run() calls adder.add(s, i) via an interface 1000 times.
fn bench_virtual_call(c: &mut Criterion) {
    let bundle = combined_bundle(shim_bundle(), bench_bundle());
    c.bench_function("virtual_call_1000x", |b| {
        b.iter(|| jvm_core::run_static_native(&bundle, "BenchVirtualCall", "run", "()I"))
    });
}

/// Measures repeated Class.getDeclaredMethods() metadata construction.
fn bench_declared_methods(c: &mut Criterion) {
    let bundle = combined_bundle(shim_bundle(), bench_bundle());
    c.bench_function("declared_methods_1000x", |b| {
        b.iter(|| {
            jvm_core::run_static_native(&bundle, "BenchDeclaredMethods", "run", "()I")
        })
    });
}

/// Measures lazy JAR registration plus first execution of a class loaded from the JAR.
fn bench_lazy_jar_load_and_run(c: &mut Criterion) {
    c.bench_function("lazy_jar_load_and_run_entry", |b| {
        b.iter(|| {
            let mut vm = jvm_core::interpreter::Vm::new();
            jvm_core::load_bundle(&mut vm, shim_bundle());
            vm.load_jar(jar_loader_test_jar()).expect("load test jar");
            let result = vm.invoke_static_threaded("JarTestEntry", "run", "()Ljava/lang/String;", vec![]);
            match result {
                Ok(jvm_core::heap::JValue::Ref(Some(r))) => {
                    assert_eq!(r.borrow().as_java_string().unwrap_or_default(), "jar-ok");
                }
                other => panic!("unexpected result: {other:?}"),
            }
        })
    });
}

/// Measures lazy JAR registration plus first resource inflate from the JAR.
fn bench_lazy_jar_load_and_read_resource(c: &mut Criterion) {
    c.bench_function("lazy_jar_load_and_read_resource", |b| {
        b.iter(|| {
            let mut vm = jvm_core::interpreter::Vm::new();
            vm.load_jar(jar_loader_test_jar()).expect("load test jar");
            let data = vm.read_resource("resource.txt")
                .expect("read resource")
                .expect("resource.txt missing");
            assert_eq!(std::str::from_utf8(&data).expect("utf8").trim(), "hello from jar resource");
        })
    });
}

/// Measures repeated ClassLoader.getResourceAsStream on an already-loaded JAR resource.
fn bench_classloader_get_resource_as_stream(c: &mut Criterion) {
    c.bench_function("classloader_get_resource_as_stream_100x", |b| {
        b.iter(|| {
            let mut vm = jvm_core::interpreter::Vm::new();
            jvm_core::load_bundle(&mut vm, shim_bundle());
            vm.load_jar(test_jar()).expect("load test jar");

            let class_loader = match vm
                .invoke_static("java/lang/ClassLoader", "getSystemClassLoader", "()Ljava/lang/ClassLoader;", vec![])
                .expect("getSystemClassLoader")
            {
                jvm_core::heap::JValue::Ref(Some(r)) => r,
                other => panic!("unexpected class loader result: {other:?}"),
            };
            let name = jvm_core::heap::JValue::Ref(Some(vm.intern_string("resource.txt")));

            for _ in 0..100 {
                let stream = vm
                    .invoke_virtual(
                        class_loader.clone(),
                        "java/lang/ClassLoader",
                        "getResourceAsStream",
                        "(Ljava/lang/String;)Ljava/io/InputStream;",
                        vec![name.clone()],
                    )
                    .expect("getResourceAsStream");
                match stream {
                    jvm_core::heap::JValue::Ref(Some(_)) => {}
                    other => panic!("unexpected stream result: {other:?}"),
                }
            }
        })
    });
}

/// Measures launcher/process execution when the first bytecode triggers a heavy class initializer.
fn bench_process_clinit_launch_to_exit(c: &mut Criterion) {
    let jar_data = framed_jars(&[test_jar()]);
    c.bench_function("process_clinit_launch_to_exit", |b| {
        b.iter(|| {
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

            let exit = pump_process_to_exit(&mut process, 4096, 64);
            let stdout = String::from_utf8(process.take_stdout()).expect("utf8 stdout");
            let stderr = String::from_utf8(process.take_stderr()).expect("utf8 stderr");
            assert_eq!(stdout, "Idone");
            assert!(stderr.is_empty());
            assert_eq!(exit.exit_code, 0);
            assert_eq!(exit.uncaught_exception, None);
        })
    });
}

/// Measures launcher/process execution when class initialization walks a superclass chain.
fn bench_process_super_clinit_chain_launch_to_exit(c: &mut Criterion) {
    let jar_data = framed_jars(&[test_jar()]);
    c.bench_function("process_super_clinit_chain_launch_to_exit", |b| {
        b.iter(|| {
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

            let exit = pump_process_to_exit(&mut process, 4096, 64);
            let stdout = String::from_utf8(process.take_stdout()).expect("utf8 stdout");
            let stderr = String::from_utf8(process.take_stderr()).expect("utf8 stderr");
            assert_eq!(stdout, "BMLbase:mid:leaf");
            assert!(stderr.is_empty());
            assert_eq!(exit.exit_code, 0);
            assert_eq!(exit.uncaught_exception, None);
        })
    });
}

criterion_group!(
    benches,
    bench_method_call,
    bench_static_field,
    bench_inherited_static_field,
    bench_string_ldc,
    bench_virtual_call,
    bench_declared_methods,
    bench_lazy_jar_load_and_run,
    bench_lazy_jar_load_and_read_resource,
    bench_classloader_get_resource_as_stream,
    bench_process_clinit_launch_to_exit,
    bench_process_super_clinit_chain_launch_to_exit
);
criterion_main!(benches);
