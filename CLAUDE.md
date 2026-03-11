# 199xVM

## Environment Setup
The Bash tool runs in bash (not fish). npm and cargo are NOT in the default bash PATH.
Always use absolute paths or source the profile first:
```bash
# For every Bash tool call, prefix with:
. "$HOME/.bash_profile" 2>/dev/null; export PATH="/opt/homebrew/bin:$HOME/.cargo/bin:$PATH"

# Or use absolute paths:
/opt/homebrew/bin/npm test
$HOME/.cargo/bin/cargo test --package jvm-core
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
- **Target: Java 25 API compatibility.** JDK shim classes should provide the same public API as JDK 25.
- JDK standard library classes are implemented as pure Java shims in `jdk-shim/` (compiled to bytecode).
- **Do NOT add native stubs in Rust** for classes that can be implemented in Java bytecode. If the class doesn't require native (OS-level) functionality, write a Java shim instead.
- To add a new JDK class: create the `.java` file under `jdk-shim/java/...`, add it to `build-shim.sh` entry points if needed, and run `./build-shim.sh`.
- The VM only uses native stubs (`native_virtual` in `interpreter.rs`) for truly native operations (e.g., `String` backed by Rust `String`, `PrintStream`).

### Shim Implementation Policy
- **JDK source copy first**: When implementing a shim, start by copying the JDK 25 source. Replace only the parts that depend on JDK-internal APIs (`jdk.internal.*`, `sun.*`, `@IntrinsicCandidate`, etc.) with simple standalone implementations.
- **String is native-backed**: This VM's `String` is backed by Rust `NativePayload::JavaString`, not a byte array. Classes that construct strings at the byte level (Integer.toString, AbstractStringBuilder) must use their own implementation rather than JDK's internal compact-string path.
- **Remove serialization**: Serialization-related code (`readObject`, `writeObject`, `serialVersionUID`, `SharedSecrets` for deserialization validation) can be removed — this VM does not support Java serialization.
- **Remove `Unsafe` usage**: Replace `jdk.internal.misc.Unsafe` usages with standard field access.
- **Annotations**: Strip `@IntrinsicCandidate`, `@ForceInline`, `@Stable`, `@jdk.internal.ValueBased` — they have no effect in this VM.

## Web Compiler (`web/javac.ts`)
- The `KNOWN_METHODS` table maps JDK method signatures to their descriptors for correct bytecode emission.
- Import resolution: `resolveClassName(ctx, name)` converts short class names (e.g., `ArrayList`) to internal JVM names (e.g., `java/util/ArrayList`) using the import map. Always use this when emitting class references.
- `lookupKnownMethod(owner, method, argDescs)` does exact match first, then falls back to prefix match (handles subtype args like String→Object).
