# 199xVM

A minimal Java bytecode interpreter compiled to WebAssembly, purpose-built to run [Raoh](https://github.com/kawasima/raoh) in the browser.

## Architecture

```
199xvm/
├── jvm-core/          # Rust — class file parser + bytecode interpreter
│   └── src/
│       ├── class_file.rs   # .class binary parser (JVMS §4)
│       ├── heap.rs         # reference-counted heap + JValue
│       ├── interpreter.rs  # opcode dispatch loop + native stubs
│       └── lib.rs          # wasm-bindgen entry points
├── web/
│   └── index.html          # playground UI (editor + output pane)
├── raoh-classes/           # pre-compiled Raoh .class files (bundle.bin)
└── build-bundle.sh         # packs .class files into bundle.bin
```

## Quick start

### 1. Build Raoh

```sh
mvn -f ../raoh/raoh/pom.xml compile
```

### 2. Pack class files

```sh
./build-bundle.sh
```

### 3. Build the WASM module

```sh
# Install wasm-pack if needed: cargo install wasm-pack
wasm-pack build jvm-core --target web
```

### 4. Serve and open

```sh
# Any static server works, e.g.:
npx serve .
# then open http://localhost:3000/web/
```

## Supported bytecode

The interpreter covers the subset Raoh uses:

- All load/store operations (`aload`, `iload`, etc.)
- Integer and long arithmetic / comparisons
- Control flow (`goto`, `if*`, `tableswitch`, `lookupswitch`)
- Object creation (`new`, `newarray`, `anewarray`)
- Field access (`getfield`, `putfield`, `getstatic`)
- Method invocation: `invokestatic`, `invokevirtual`, `invokespecial`, `invokeinterface`
- `invokedynamic` with three bootstrap handlers:
  - `LambdaMetafactory` — lambda/Decoder composition
  - `StringConcatFactory` — `toString` string building
  - `SwitchBootstraps.typeSwitch` — `Result`/`Ok`/`Err` pattern matching
- Native stubs for `java.lang.*` and `java.util.*` used by Raoh

## Known limitations

- No threads
- No JIT
- GC is reference-counting (no cycle collection)
- `invokedynamic` lambda bodies are stubbed — full `MethodHandle` resolution is a TODO
- Reflection is not supported
