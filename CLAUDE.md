# 199xVM

## Git Workflow

- The main integration branch is **`develop`**. All feature branches must be based on `develop` and PRs must target `develop`.
- **`main`** is the release branch only. Never open a PR targeting `main` for feature work.
- Branch naming: `feature/<short-description>` (e.g., `feature/bool-constraints`).
- **ALWAYS** pass `--base develop` when creating a PR — never omit it, as `gh pr create` defaults to `main`:

  ```sh
  gh pr create --base develop --title "..." --body "..."
  ```

- If a PR was accidentally opened against `main`, fix it immediately with `gh pr edit <number> --base develop`.
- **`CLAUDE.md` is excluded from PRs.** Commit changes to `CLAUDE.md` directly on `develop`. Never include it in a feature branch.

## Build Commands

All build tasks are managed via `make`. Key targets:

- `make dev-jars` — download versioned JARs to `web/` via Maven
- `make shim` — compile JDK shim classes → `jdk-shim/bundle.bin`
- `make test-bundle` — compile test classes → `test-classes/bundle.bin`
- `make javac` — compile `web/javac.ts` → `web/javac.js` (esbuild)
- `make wasm` — compile Rust core → `jvm-core/pkg/` (wasm-pack)
- `make all` — dev-jars + shim + javac + wasm (full dev setup)
- `make dist` — assemble `dist/` for deployment
- `make deploy GCS=gs://bucket/path` — upload `dist/` to GCS
- `make test` — run javac compiler tests (`web/javac.test.ts`)
- `make clean` — remove generated artifacts
- `cargo test --package jvm-core` — run Rust integration tests

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
2. **JDK元ファイルのヘッダ（copyright / license）を削除・改変しない。** SHIM追加・更新時は、コピー元JDKファイルのヘッダをそのまま保持する。ヘッダ欠落の新規作成は禁止。
3. **String is native-backed**: `NativePayload::JavaString`（Rust String）で管理。JDKのcompact-string (byte[]) パスは使えない。`Integer.toString`、`AbstractStringBuilder`等は独自実装を維持。
4. **削除してよいもの**: serialization (`readObject`/`writeObject`/`serialVersionUID`/`SharedSecrets`)、`Unsafe`（標準フィールドアクセスに置換）、JDK内部アノテーション (`@IntrinsicCandidate`/`@ForceInline`/`@Stable`/`@ValueBased`)。

## Web Compiler (`web/javac.ts`)

- Import resolution: `resolveClassName(ctx, name)` converts short class names (e.g., `ArrayList`) to internal JVM names (e.g., `java/util/ArrayList`) using the import map. Always use this when emitting class references.
- `lookupKnownMethod(owner, method, argDescs)` does exact match first, then falls back to prefix match (handles subtype args like String→Object).
