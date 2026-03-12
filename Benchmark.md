# Benchmark Guide

Benchmarks measure the interpreter hot paths identified in [Issue #13](https://github.com/kawasima/199xVM/issues/13).

## Prerequisites

Build the JDK shim and benchmark class bundles before running:

```sh
./build-shim.sh          # builds jdk-shim/bundle.bin
./build-test-bundle.sh   # builds test-classes/bench-bundle.bin
```

## Running benchmarks

```sh
cargo bench --package jvm-core
```

Results are written to `target/criterion/` as HTML reports.

## Benchmark scenarios

| Name | Java class | Issue #13 bottleneck |
|---|---|---|
| `method_call_1000x` | `BenchMethodCall.run()` | O(n) method lookup + constant pool clone per static call |
| `static_field_1000x` | `BenchStaticField.run()` | `format!` string allocation on every `getstatic`/`putstatic` |
| `string_ldc_1000x` | `BenchStringLdc.run()` | `String::clone` on every `ldc` string constant |
| `virtual_call_1000x` | `BenchVirtualCall.run()` | Interface virtual dispatch + interface name list rebuild |

Each Java method runs a loop of 1000 iterations so the per-call overhead is amplified and measurable above criterion's noise floor.

## Comparing before and after a fix

Save a named baseline before applying a fix:

```sh
cargo bench --package jvm-core -- --save-baseline before
```

Apply the fix, then compare:

```sh
cargo bench --package jvm-core -- --baseline before
```

criterion will print a percentage change and confidence interval for each benchmark.

## Baseline (2026-03-12, unoptimized interpreter)

| Benchmark | Time (median) |
|---|---|
| `method_call_1000x` | 4.91 ms |
| `static_field_1000x` | 4.15 ms |
| `string_ldc_1000x` | 3.54 ms |
| `virtual_call_1000x` | 5.06 ms |
