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

criterion_group!(
    benches,
    bench_method_call,
    bench_static_field,
    bench_string_ldc,
    bench_virtual_call
);
criterion_main!(benches);
