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
- **Target: Java 25 API compatibility.** Shim classes provide the same public API as JDK 25.
- JDK standard library classes are implemented as pure Java shims in `jdk-shim/` (compiled to bytecode).
- To add a new JDK class: create `.java` under `jdk-shim/java/...`, add to `build-shim.sh` entry points if needed, run `./build-shim.sh`.
- Native stubs (`native_virtual` in `interpreter.rs`) are only for truly native operations (e.g., `String` backed by Rust, `PrintStream`). **Do NOT add native stubs** for classes implementable in Java.

### Shim Implementation Policy
1. **JDK 25ソースをコピーして始める。** `jdk.internal.*`、`sun.*`、nativeメソッドの部分だけ代替実装に置き換える。ゼロから書かない。
2. **String is native-backed**: `NativePayload::JavaString`（Rust String）で管理。JDKのcompact-string (byte[]) パスは使えない。`Integer.toString`、`AbstractStringBuilder`等は独自実装を維持。
3. **削除してよいもの**: serialization (`readObject`/`writeObject`/`serialVersionUID`/`SharedSecrets`)、`Unsafe`（標準フィールドアクセスに置換）、JDK内部アノテーション (`@IntrinsicCandidate`/`@ForceInline`/`@Stable`/`@ValueBased`)。

## Web Compiler (`web/javac.ts`)
- The `KNOWN_METHODS` table maps JDK method signatures to their descriptors for correct bytecode emission.
- Import resolution: `resolveClassName(ctx, name)` converts short class names (e.g., `ArrayList`) to internal JVM names (e.g., `java/util/ArrayList`) using the import map. Always use this when emitting class references.
- `lookupKnownMethod(owner, method, argDescs)` does exact match first, then falls back to prefix match (handles subtype args like String→Object).
