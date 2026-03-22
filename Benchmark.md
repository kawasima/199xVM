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
| `inherited_static_field_1000x` | `BenchInheritedStaticField.run()` | inherited `getstatic`/`putstatic` owner resolution on every access |
| `string_ldc_1000x` | `BenchStringLdc.run()` | `String::clone` on every `ldc` string constant |
| `virtual_call_1000x` | `BenchVirtualCall.run()` | Interface virtual dispatch + interface name list rebuild |
| `declared_methods_1000x` | `BenchDeclaredMethods.run()` | repeated `Class.getDeclaredMethods()` metadata rebuild |
| `process_clinit_launch_to_exit` | `ClinitYieldProcessMain.main()` | launcher/process path when the first bytecode schedules a heavy `<clinit>` |
| `process_super_clinit_chain_launch_to_exit` | `ClinitChainYieldProcessMain.main()` | launcher/process path when class initialization walks a heavy superclass chain |

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

The historical tables below predate the later class-init and inherited-static-field scenarios, so
they currently cover only the original four microbenchmarks.

## Baseline (2026-03-12, unoptimized interpreter)

| Benchmark | Time (median) |
|---|---|
| `method_call_1000x` | 4.91 ms |
| `static_field_1000x` | 4.15 ms |
| `string_ldc_1000x` | 3.54 ms |
| `virtual_call_1000x` | 5.06 ms |

## After Issue #13 fixes (2026-03-12)

Changes: `resolve_method_exec_info` helper (eliminates repeated `find_method` calls),
`ConstantPool.entries` wrapped in `Rc` (O(1) clone), `static_fields` restructured to
`HashMap<class, HashMap<field, value>>` (no `format!` key), `intern_string` uses `entry` API.

| Benchmark | Before | After | Change |
|---|---|---|---|
| `method_call_1000x` | 4.91 ms | 4.39 ms | **-10.7%** |
| `static_field_1000x` | 4.15 ms | 3.93 ms | **-5.4%** |
| `string_ldc_1000x` | 3.54 ms | 3.68 ms | ~0% (noise) |
| `virtual_call_1000x` | 5.06 ms | 4.54 ms | **-10.3%** |
