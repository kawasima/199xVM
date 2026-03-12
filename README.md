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
│   ├── javac/                   # modularized compiler core
│   │   ├── lexer.ts
│   │   ├── parser.ts
│   │   ├── ast.ts
│   │   ├── compiler.ts
│   │   └── method-registry.ts
│   └── javac.test.ts            # compiler tests
├── jdk-shim/                    # Java standard library shims (pure Java)
│   └── bundle.bin               # compiled shim class bundle
├── build-shim.sh
├── build-test-bundle.sh
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
| JLS-6 | Names, scope, and shadowing | Partial | `parser.ts` / `compiler.ts` | Tighten complex shadowing and ambiguity diagnostics |
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
| JVMS-5 | Linking/loading behavior (project scope) | Implemented | ClassLoader paths + linkage error tests | Bytecode verifier (§4.10), classloader hierarchy, multi-classloader namespace |
| JVMS-6 | Instruction set execution | Implemented | `dispatch.rs` + integration tests | Keep expanding opcode edge-case tests |
| JVMS-6.5 | Invocation (`invoke*`, `invokedynamic`) | Implemented | `invoke.rs`/`dispatch.rs` + invoke tests | No CallSite caching; bootstrap support is intentionally narrow |
| JVMS exceptions | Exception table dispatch / `athrow` | Implemented | runtime exception tests | Add more mixed `finally`/rethrow regressions |
| JVMS verification | Bytecode verifier strictness | Limited | runtime checks only | Implement stricter verifier-like prechecks |
| JVMS monitors/threads | Monitors + green threads (`Thread.start/join/yield`, `wait/notify/notifyAll`) | Partial | monitor/thread integration tests | Cooperative scheduler only (not OS/preemptive threads); timed wait/join/sleep and interruption semantics are intentionally limited |
| JVMS memory model | GC / object lifecycle behavior | Limited | `heap.rs` (ref-count) | No cycle collector; keep scope explicit |

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
npm test

# VM integration tests
cargo test --package jvm-core

# Rebuild core artifacts
./build-shim.sh
npm run build:javac
wasm-pack build jvm-core --target web
```

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
