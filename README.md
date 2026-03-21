# 199xVM

A minimal Java bytecode interpreter compiled to WebAssembly, with an in-browser Java compiler.

## Concept

**"Write, compile, and run Java in the browser - no server required."**

199xVM consists of two parts:

1. **JVM interpreter** - Rust compiled to WebAssembly, interprets `.class` bytecode directly
2. **Java compiler** - TypeScript compiler that emits `.class` bytecode in the browser

No transpilation and no server round-trip. Java source is compiled and executed fully client-side.

## Scope and claim

199xVM targets **progressive Java 25 compatibility** for practical in-browser execution.

It is **not** a full implementation of `javac` or HotSpot, and it should not currently be advertised as "fully JLS/JVMS compliant". The conformance matrix below is the source of truth for implementation status.

## Architecture

```text
199xvm/
├── jvm-core/                    # Rust crate, built to jvm_core.wasm
│   └── src/
│       ├── class_file.rs        # classfile parser (JVMS §4)
│       ├── heap.rs              # heap/object model
│       ├── interpreter/
│       │   ├── mod.rs           # interpreter entry + module wiring
│       │   ├── dispatch.rs      # opcode dispatch
│       │   ├── invoke.rs        # method invocation path
│       │   ├── native_static.rs # static native hooks
│       │   └── native_virtual.rs# virtual native hooks
│       └── lib.rs               # wasm-bindgen API
├── web/
│   ├── index.html               # playground UI
│   ├── class-reader.ts          # class/JAR reader for method registry
│   ├── javac.ts                 # compiler entrypoint
│   ├── launcher.js              # process-style JVM launcher API
│   ├── javac/                   # modularized compiler core
│   │   ├── lexer.ts
│   │   ├── parser.ts
│   │   ├── ast.ts
│   │   ├── compiler.ts
│   │   └── method-registry.ts
│   └── javac.test.ts            # compiler tests
├── jdk-shim/                    # Java standard library shims (pure Java, JDK 25 based)
│   └── bundle.bin               # compiled shim class bundle (~815 classes)
├── build-shim.sh
├── build-test-bundle.sh
├── build-clj-smoke.sh           # Clojure validation bundle builder
└── build-dist.sh
```

## JLS/JVMS conformance matrix

Status labels:

- **Implemented**: feature is broadly available in this project scope
- **Partial**: significant subset works, but edge cases or strict checks are incomplete
- **Limited**: intentionally narrow support

### How to maintain this matrix

- Add or update a row whenever language/runtime behavior changes.
- Keep each row tied to concrete evidence (`tests` or implementation path).
- If a row is Partial/Limited, keep `Gap / next step` actionable.

### JLS (Java Language Specification) matrix

| ID | Topic | Status | Evidence | Gap / next step |
| --- | --- | --- | --- | --- |
| JLS-3 | Lexical structure / tokens / literals | Implemented | `lexer.ts` + lexer tests | Keep parity checks for edge lexical forms |
| JLS-6 | Names, scope, and shadowing | Partial | `compiler.ts` scope/shadowing diagnostics + tests | Effectively-final: enclosing-scope reassignment not yet checked |
| JLS-8 | Classes, members, constructors (incl. records/enums) | Partial | parser+codegen tests | Cover remaining declaration constraints and corner cases |
| JLS-9 | Interfaces and inheritance behavior | Partial | method resolution tests | Expand default/static/interface conflict rules |
| JLS-11 | Exceptions and checked-exception analysis | Partial | throw/catch analysis tests | Expand full-path exception typing coverage |
| JLS-14 | Statements (`if/for/while/switch/try/assert/synchronized`) | Partial | parser/codegen/runtime tests | Continue strict semantic checks and flow diagnostics |
| JLS-15 | Expressions, calls, conversions, lambdas | Partial | expression codegen tests | Close gaps in typing/narrowing/inference edge cases |
| JLS-16 | Definite assignment | Limited | basic flow checks | Add full DA/DU data-flow analysis equivalent to `javac` |
| JLS-7 / JLS-13 | Packages/imports and binary compatibility | Partial | import resolution tests | Improve compatibility checks and diagnostics |
| JLS tooling areas | Modules, annotation processing, full toolchain parity | Limited | out of current scope | Track as separate long-term epics |

### JVMS (Java Virtual Machine Specification) matrix

| ID | Topic | Status | Evidence | Gap / next step |
| --- | --- | --- | --- | --- |
| JVMS-4 | ClassFile format and constant pool | Implemented | `class_file.rs` + parser tests | Add stricter validation where parser is permissive |
| JVMS-5 | Linking/loading behavior (project scope) | Implemented | ClassLoader hierarchy (ClassLoader → SecureClassLoader → URLClassLoader), JAR loader, `defineClass`, linkage error tests | Bytecode verifier (§4.10), multi-classloader namespace |
| JVMS-6 | Instruction set execution | Implemented | `dispatch.rs` + integration tests | Keep expanding opcode edge-case tests |
| JVMS-6.5 | Invocation (`invoke*`, `invokedynamic`) | Implemented | `invoke.rs`/`dispatch.rs` + invoke tests | No CallSite caching; bootstrap support is intentionally narrow |
| JVMS exceptions | Exception table dispatch / `athrow` | Implemented | runtime exception tests | Add more mixed `finally`/rethrow regressions |
| JVMS verification | Bytecode verifier strictness | Limited | runtime checks only | Implement stricter verifier-like prechecks |
| JVMS monitors/threads | Monitors + green threads (`Thread.start/join/yield`, `wait/notify/notifyAll`) | Partial | monitor/thread integration tests | Cooperative scheduler only (not OS/preemptive threads); timed wait/join/sleep and interruption semantics are intentionally limited |
| JVMS memory model | GC / object lifecycle behavior | Limited | `heap.rs` (ref-count) | No cycle collector; keep scope explicit |

## JAR loader

199xVM can load classes directly from JAR files at runtime via `Vm::load_jar()`. The Rust `zip` crate (WASM-compatible) parses the ZIP central directory up front, but individual class/resource entries are decompressed only on first access. Non-class resources remain addressable through `ClassLoader.getResourceAsStream()`.

The frontend also exposes a `jar_to_bundle()` WASM export that converts JAR bytes to the flat bundle format, falling back to the JS-side `readJar()` when WASM is not available.

## Launcher API

199xVM now exposes a process-style launcher layer on top of the low-level VM primitives.

- Public JS API: `launchClasspathMain({ classpath, mainClass, args, stdio })`
- Return value: `ProcessHandle`
- `ProcessHandle.stdin`: writable byte stream
- `ProcessHandle.stdout`: readable byte stream
- `ProcessHandle.stderr`: readable byte stream
- `ProcessHandle.wait()`: resolves to `{ exitCode, uncaughtException? }`
- `ProcessHandle.kill()`: terminates the in-VM process

Design notes:

- The VM is responsible only for Unix-like process I/O (`stdin`/`stdout`/`stderr`)
- REPL/readline/autocomplete remain userland concerns
- Low-level `run_static()` / `run_with_jars()` remain available
- Browser `inherit` for stdout/stderr is implemented by forwarding process chunks to `console.log` / `console.error`
- Stream granularity is chunk/byte-based, not line-based

Current scope:

- `launchClasspathMain()` is implemented
- `classpath` currently accepts an array of JAR byte arrays
- `stdio.stdin: "inherit"` is not implemented yet; use `"pipe"` or `"ignore"`
- `launchJar()` is intentionally deferred
- Manifest `Class-Path` and TTY-specific behavior are out of scope for now

## JVM language support

199xVM also has a dedicated **Clojure 1.12.0** validation lane:

<details>
<summary>Clojure validation details</summary>

```sh
# Preferred: Docker / OrbStack
make clj-smoke-test-docker
make clj-upstream-test-docker
make clj-upstream-coverage-docker

# Local fallback: build artifacts with Clojure CLI + git
./build-clj-smoke.sh
make clj-smoke-test
make clj-upstream-test
make clj-upstream-coverage
```

Artifacts produced by `build-clj-smoke.sh`:

- `clj-smoke/smoke.jar`: AOT-compiled minimal smoke entry point (`ClojureSmokeEntry.run()`)
- `clj-smoke/upstream-tests.jar`: selected upstream `clojure/clojure` test namespaces, support helpers, and runner (`ClojureUpstreamTestEntry`)
- `clj-smoke/clojure-jars.txt`: local copied runtime JARs for Clojure 1.12.0

The upstream runner currently stages a selected subset of `clojure.test-clojure` namespaces.
Execution is split into ignored Rust diagnostics (`clojure_upstream_*`, `clojure_diag_*`) so failures
localize to one namespace, or one `deftest` var for the heavier paths, instead of one long monolithic run. See
`test-sources/clojure/src/upstream/runner.clj` and `make clj-upstream-coverage` for the current
exact selection and milestone numbers.

The default `clojure_upstream_*` gate is intentionally a small representative slice:
`atoms`, `logic`, and `try-catch`. The slower or currently unsupported upstream namespaces are kept
as opt-in diagnostics (`clojure_diag_*`). Today that includes the `fn` compiler/spec path plus
`control`, `evaluation`, `keywords`, `macros`, `metadata`, `other-functions`, `predicates`,
`special`, `string`, and `vectors`. That keeps the routinely used Clojure lane under the
one-minute-per-test budget while preserving a dedicated place to investigate compiler/runtime
performance and semantics gaps. For longer local diagnostics, `clojure_integration_test` also
accepts `UPSTREAM_MAX_ELAPSED_SECS`, `UPSTREAM_LOG_OUTPUT`, and `UPSTREAM_TIMING`.

`make clj-upstream-coverage` reports a simple milestone metric for that subset:

- selected namespaces / total `clojure.test-clojure` namespaces
- selected `deftest` vars / total `deftest` vars in `clojure.test-clojure`

This is a suite-selection metric, not JVM line or branch coverage.

The upstream harness currently applies a local `java.specification.version=1.8` / `java.vm.specification.version=1.8` compatibility override before requiring those namespaces so Clojure 1.12.0 takes its older reflection path. This override is scoped to the harness and does not change the VM's advertised Java 25 identity.

This remains an isolated diagnostic path — it is **not** part of `make test` or the default
`cargo test`. The slow-path tests live in a dedicated integration target,
`clojure_integration_test.rs`, with one smoke test plus namespace-scoped or var-scoped upstream
diagnostics. For routine development, use `make clj-smoke-test` as the main Clojure gate. The
`clojure_upstream_*` tests are sequential diagnostic probes with a hard per-test timeout under
60 seconds, intended to localize slow or broken areas rather than serve as a fast green suite.

The VM also has a generic interpreter profiler that is not tied to the Clojure harness. Set
`JVM_PROFILE=1` to emit an aggregated report to stderr at process end, and optionally set
`JVM_PROFILE_TOP=<n>` to change how many methods / `Class.forName` targets / opcodes are shown.
This works through `JvmProcess`, the direct VM invoke helpers, and the `run_bundle` CLI, so it can
be used for any Java workload that runs on 199xVM.

Heavier diagnostics can be targeted at a single upstream `deftest` var by passing a selector like
`clojure.test-clojure.evaluation/Collections` through the runner. When one upstream `deftest`
remains too large, the harness can also route to a local wrapper namespace that splits that var
into smaller probes. The Rust harness uses those forms for `clojure_diag_evaluation_*` so
`evaluation` remains debuggable without one 90-second monolith.

This is additional validation signal for JVM capability work; the JLS/JVMS conformance matrix above remains the source of truth for project claims.

</details>

## JDK shim policy

JDK APIs are provided primarily as **pure Java shims** under `jdk-shim/` and compiled to bytecode.

- Target is Java 25 API compatibility where practical
- Prefer Java shim implementations over Rust native stubs
- Keep Rust natives only for runtime-boundary functionality (for example output/time/string bridging)

## Quick start

```sh
# 1) Build wasm VM
cargo install wasm-pack
wasm-pack build jvm-core --target web

# 2) Build JDK shims
./build-shim.sh

# 3) Build compiler
npm install
npm run build:javac

# 4) Serve
npx serve .
# open http://localhost:3000/web/
```

## Development

```sh
# Compiler tests
npm test                    # or: make test

# VM integration tests
cargo test --package jvm-core

# Launcher API tests
make launcher-test

# Clojure validation lane
# See the folded "Clojure validation details" section above.

# Rebuild core artifacts
./build-shim.sh
npm run build:javac
wasm-pack build jvm-core --target web
```

### Docker (three ways: VM only / Compiler only / Web Playground)

Use Docker or OrbStack with the project’s `docker-compose.yml`. Only the **web** service is started by `docker-compose up`; the **rust**, **java**, **clojure**, and **node** services are for one-off builds/tests via `docker-compose run <service> ...`.

**1. VM (wasm) only**

```sh
docker-compose run rust make wasm
docker-compose run rust cargo test --package jvm-core --lib
docker-compose run rust cargo fmt --all
```

The `--lib` flag runs only unit tests. For full integration tests (which need `test-classes/bundle.bin`), build the test bundle first:  
`docker-compose run node npm ci && docker-compose run node make javac`, then `docker-compose run java make test-bundle`. After that, `docker-compose run rust cargo test --package jvm-core` runs all tests.

The separate Clojure validation lane is documented in the folded section above.
It uses the dedicated `clojure` service, which is based on the official Clojure tools.deps image, while the main `java` service stays focused on JVM build lanes.

**2. Compiler only**

```sh
docker-compose run node npm ci   # first time (or when package.json changes)
docker-compose run node make javac
docker-compose run node make test
```

**3. Web Playground (full dist + serve)**

Build all artifacts, then start the web server:

```sh
docker-compose run java make dev-jars
docker-compose run rust make wasm
docker-compose run java make shim
docker-compose run node make javac
docker-compose run node make dist
docker-compose up   # serves dist/ at <http://localhost:3000/>
```

Then open <http://localhost:3000/>. Alternatively, use `make docker-playground` to run the build steps above and then `docker-compose up`.

> **Note:** `docker-compose up` (web service) serves `dist/`. Run `dist-docker` or the manual steps above before starting it, otherwise `dist/` will be empty.

## Known limitations (high level)

- Full Java 25 language/toolchain parity is out of scope today
- Full JVM verification and GC semantics are not implemented
- Threading is cooperative green-thread based (not full HotSpot/OS-thread parity)
- Some advanced language semantics are intentionally staged and tightened incrementally

## Contributing

- Interpreter entry: `jvm-core/src/interpreter/mod.rs`
- Interpreter dispatch/runtime pieces: `jvm-core/src/interpreter/dispatch.rs`, `invoke.rs`, `native_static.rs`, `native_virtual.rs`
- Compiler entry: `web/javac.ts`
- Compiler modules: `web/javac/lexer.ts`, `parser.ts`, `ast.ts`, `compiler.ts`, `method-registry.ts`
- Compiler tests: `web/javac.test.ts`
- JDK shims: `jdk-shim/`

PRs for feature work should target `develop`.
