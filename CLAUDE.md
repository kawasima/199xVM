# 199xVM

## Environment Setup
When running shell commands, always ensure cargo is in PATH:
```
export PATH="$HOME/.cargo/bin:$PATH"
```

## Build Commands
- `./build-shim.sh` — compile JDK shim classes → `jdk-shim/bundle.bin`
- `./build-test-bundle.sh` — compile test classes → `test-classes/bundle.bin`
- `npm run build:javac` — compile `web/javac.ts` → `web/javac.js` (esbuild)
- `cargo test --package jvm-core` — run integration tests
- `npm test` — run javac compiler tests (`web/javac.test.ts`)

## Testing
- Integration tests: `jvm-core/tests/integration_test.rs` — uses `include_bytes!` to embed pre-built `.class` bundles. Rebuild bundles before running tests if Java sources change.
- Compiler tests: `web/javac.test.ts` — tests for lexer, parser, code generator. Run with `npm test`.
- Always run `npm test` after modifying `web/javac.ts`.

## JDK Shim Design
- JDK standard library classes are implemented as pure Java shims in `jdk-shim/` (compiled to bytecode).
- **Do NOT add native stubs in Rust** for classes that can be implemented in Java bytecode. If the class doesn't require native (OS-level) functionality, write a Java shim instead.
- To add a new JDK class: create the `.java` file under `jdk-shim/java/...`, add it to `build-shim.sh` entry points if needed, and run `./build-shim.sh`.
- The VM only uses native stubs (`native_virtual` in `interpreter.rs`) for truly native operations (e.g., `String` backed by Rust `String`, `PrintStream`).

## Web Compiler (`web/javac.ts`)
- The `KNOWN_METHODS` table maps JDK method signatures to their descriptors for correct bytecode emission.
- Import resolution: `resolveClassName(ctx, name)` converts short class names (e.g., `ArrayList`) to internal JVM names (e.g., `java/util/ArrayList`) using the import map. Always use this when emitting class references.
- `lookupKnownMethod(owner, method, argDescs)` does exact match first, then falls back to prefix match (handles subtype args like String→Object).
