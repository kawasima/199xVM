# 199xVM

A minimal Java bytecode interpreter compiled to WebAssembly.

## Concept

**"Run Java in the browser, without a server."**

199xVM interprets Java `.class` files directly inside WebAssembly.
No transpilation, no server round-trip — the bytecode runs in the browser as-is.

The immediate motivation is running [Raoh](https://github.com/kawasima/raoh)
(a zero-dependency Java validation library) as an interactive playground.
However, the VM is designed to be general-purpose: any Java code that fits
within the [supported bytecode subset](#supported-bytecode) can be loaded and executed.

### Design goals

- **Browser-first** — the only runtime is a `.wasm` module served as a static file.
- **Zero Java dependencies** — no JDK, no Gradle, no Node.js required at runtime.
- **Hackable** — the entire interpreter is ~1,200 lines of Rust; easy to extend.
- **General-purpose entry point** — callers supply the class name, method name,
  and descriptor at runtime. No hard-coded Raoh assumptions.

---

## Architecture

```text
199xvm/
├── jvm-core/               # Rust crate — compiled to jvm_core.wasm
│   └── src/
│       ├── class_file.rs   # .class binary parser (JVMS §4, up to version 69)
│       ├── heap.rs         # reference-counted heap  (JValue / JObject)
│       ├── interpreter.rs  # opcode dispatch loop + java.* native stubs
│       └── lib.rs          # wasm-bindgen public API
├── web/
│   └── index.html          # playground UI (editor pane + output pane)
├── raoh-classes/           # pre-compiled Raoh .class files → bundle.bin
└── build-bundle.sh         # pack a directory of .class files into bundle.bin
```

### Class bundle format

Classes are shipped as a single binary blob:

```
[ u32 length (big-endian) ][ raw .class bytes ]  ×  N classes
```

`build-bundle.sh` produces this from any `target/classes` directory.
The browser fetches the blob once and passes it to `run_static`.

---

## Public API (wasm-bindgen)

### `run_static(class_bundle, main_class, method_name, descriptor) → String`

Load a class bundle and invoke an arbitrary **static method**.

| Parameter | Example |
| --- | --- |
| `class_bundle` | `Uint8Array` — output of `build-bundle.sh` |
| `main_class` | `"com/example/Hello"` (internal name) |
| `method_name` | `"greet"` |
| `descriptor` | `"(Ljava/lang/String;)Ljava/lang/String;"` |

Returns the `toString()` of the result, or `"ERROR: …"` on failure.

The caller is responsible for supplying the correct class name and descriptor.
No Raoh-specific assumptions are baked in.

### `parse_class(class_bytes) → String`

Parse a single `.class` file and return `"OK: <ClassName> (vMAJOR.MINOR)"`
or `"ERROR: …"`. Useful for debugging bundle contents.

---

## Quick start

### 1. Compile the target Java project

```sh
# Example: Raoh
mvn -f ../raoh/raoh/pom.xml compile

# Any Maven project:
mvn -f /path/to/project/pom.xml compile
```

### 2. Pack class files into a bundle

```sh
./build-bundle.sh path/to/target/classes
# → raoh-classes/bundle.bin
```

### 3. Build the WASM module

```sh
# Install wasm-pack if needed:
cargo install wasm-pack

wasm-pack build jvm-core --target web
# → jvm-core/pkg/jvm_core.js + jvm_core_bg.wasm
```

### 4. Serve and open

```sh
npx serve .
# open http://localhost:3000/web/
```

---

## Supported bytecode

The interpreter covers:

- All load / store operations (`aload`, `iload`, `dload`, …)
- Integer and long arithmetic, bitwise ops, comparisons
- Control flow: `goto`, `goto_w`, `if*`, `tableswitch`, `lookupswitch`
- Object creation: `new`, `newarray`, `anewarray`
- Array access: `aaload`, `aastore`, `iaload`, `baload`, `arraylength`
- Field access: `getfield`, `putfield`, `getstatic` (putstatic is a no-op)
- Method invocation: `invokestatic`, `invokevirtual`, `invokespecial`, `invokeinterface`
- `invokedynamic` with three bootstrap handlers:
  - `LambdaMetafactory` — lambda capture (Decoder composition, `Function`, etc.)
  - `StringConcatFactory` — `toString` / `+` string concatenation
  - `SwitchBootstraps.typeSwitch` — sealed-interface / pattern-matching switch
- Type checks: `instanceof`, `checkcast` (cast is a no-op)
- Exception throwing: `athrow`
- `wide` prefix for large local variable indices
- Native stubs for commonly used `java.lang.*` and `java.util.*` methods

---

## Known limitations

| Area | Status |
| --- | --- |
| Threads / `synchronized` | Not supported (`monitorenter`/`monitorexit` are no-ops) |
| JIT compilation | Not planned — interpreter only |
| GC | Reference-counting; no cycle collection |
| `invokedynamic` lambda bodies | Stubbed — captured args are stored but the impl `MethodHandle` is not yet resolved and called |
| Reflection (`Class.forName`, `Method.invoke`, …) | Not supported |
| `java.io` / `java.nio` | Not supported |
| `java.net` | Not supported |
| Exception handling (`try`/`catch`) | Exception table is parsed but handler dispatch is not yet implemented |
| `float` / `double` arithmetic | Basic ops work; transcendentals (`Math.sin`, etc.) are not stubbed |

---

## TODO

### High priority (needed to run Raoh end-to-end)

- [ ] **`invokedynamic` lambda invocation** — when a lambda object's functional
  interface method is called (`invokeinterface` on `$$Lambda`), resolve and
  execute the captured implementation `MethodHandle`.
- [ ] **Exception table dispatch** — implement `try`/`catch` handler lookup so
  that Raoh's error-accumulation paths work correctly.
- [ ] **`java.util.stream.Stream` stubs** — Raoh uses `Stream.map`, `Stream.collect`,
  `Collectors.toList`, etc. for building `Issues` lists.
- [ ] **`java.util.List.copyOf` / `List.of` with varargs** — needed for `Issues`
  construction.
- [ ] **`String` comparison stubs** — `String.equals`, `String.isEmpty`,
  `String.length`, `String.contains` are called in decoder constraints.

### Medium priority (general-purpose JVM quality)

- [ ] **`putstatic` with static field storage** — currently discards the value;
  add a per-class static field table in `Vm`.
- [ ] **`invokedynamic` — `ObjectMethods.bootstrap`** — needed for `Record`
  `equals` / `hashCode` / `toString` (used by `Ok` and `Err`).
- [ ] **`java.util.HashMap` / `ArrayList` native operations** — `put`, `get`,
  `containsKey`, `size`, `isEmpty`, `iterator`.
- [ ] **`java.util.Optional` native operations** — `isPresent`, `get`, `map`,
  `orElse`.
- [ ] **Proper `null` check in field access** — `getfield`/`putfield` on null
  should throw `NullPointerException` with path information.

### Low priority / future

- [ ] **Source map / stack trace** — use `LineNumberTable` to produce readable
  error messages.
- [ ] **Class loading from URL** — let the playground fetch individual `.class`
  files on demand instead of requiring a pre-built bundle.
- [ ] **Incremental class compilation service** — a tiny server endpoint that
  accepts a Java snippet, compiles it with `javac`, and returns the `.class`
  bytes so users can write arbitrary code in the editor.
- [ ] **Cycle-collecting GC** — replace `Rc` with a tracing collector for
  long-running sessions.
- [ ] **`float` / `double` complete coverage** — `fcmpl`, `fcmpg`, `dcmpl`,
  `dcmpg`, `Math.*` stubs.
- [ ] **`multianewarray`** — multi-dimensional array creation.

---

## Contributing

The core interpreter loop is in [jvm-core/src/interpreter.rs](jvm-core/src/interpreter.rs).
Each opcode is a single `match` arm — straightforward to add new ones.
Native method stubs live in `native_static` and `native_virtual` at the bottom
of the same file.
