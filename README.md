# 199xVM

A minimal Java bytecode interpreter compiled to WebAssembly, with an in-browser Java compiler.

## Concept

**"Write, compile, and run Java in the browser — no server required."**

199xVM consists of two parts:

1. **JVM interpreter** — Rust compiled to WebAssembly, interprets `.class` bytecode directly
2. **Java compiler** — TypeScript (`web/javac.ts`), compiles a subset of Java to `.class` bytecode in the browser

No transpilation, no server round-trip — write Java in the editor, compile to bytecode, and execute it, all client-side.

### Design goals

- **Browser-first** — the only runtime is a `.wasm` module + JS, served as static files
- **Zero server dependency** — compile and run Java entirely in the browser
- **JDK shims in pure Java** — standard library classes (`ArrayList`, `HashMap`, `StringBuilder`, etc.) are implemented as Java source compiled to bytecode, not as native Rust stubs
- **Hackable** — the interpreter is ~1,600 lines of Rust; the compiler is ~2,400 lines of TypeScript

---

## Architecture

```text
199xvm/
├── jvm-core/               # Rust crate — compiled to jvm_core.wasm
│   └── src/
│       ├── class_file.rs   # .class binary parser (JVMS §4)
│       ├── heap.rs         # reference-counted heap (JValue / JObject)
│       ├── interpreter.rs  # opcode dispatch loop + native stubs
│       └── lib.rs          # wasm-bindgen public API
├── web/
│   ├── index.html          # playground UI (CodeMirror editor + output)
│   ├── javac.ts            # in-browser Java subset compiler
│   └── javac.test.ts       # compiler test suite
├── jdk-shim/               # JDK standard library shims (pure Java)
│   ├── java/lang/          # String, StringBuilder, Integer, Record, ...
│   ├── java/util/          # ArrayList, HashMap, Optional, ...
│   └── bundle.bin          # compiled shim classes (length-prefixed bundle)
├── build-shim.sh           # compile shim sources → bundle.bin
└── build-test-bundle.sh    # compile test classes → test-classes/bundle.bin
```

### Class bundle format

Classes are shipped as a single binary blob:

```
[ u32 length (big-endian) ][ raw .class bytes ]  ×  N classes
```

The browser fetches shim `bundle.bin`, the compiler produces user class bytes, and both are concatenated before passing to the VM.

---

## Quick start

### 1. Build the WASM module

```sh
cargo install wasm-pack
wasm-pack build jvm-core --target web
```

### 2. Build JDK shims

```sh
./build-shim.sh
# → jdk-shim/bundle.bin (63 shim classes)
```

### 3. Build the compiler

```sh
npm install
npm run build:javac
# → web/javac.js
```

### 4. Serve and open

```sh
npx serve .
# open http://localhost:3000/web/
```

---

## In-browser compiler (`web/javac.ts`)

The compiler supports a subset of Java:

- Class declarations with fields, constructors, instance/static methods
- Inheritance (`extends`) with `super()` calls
- Record types
- Control flow: `if`/`else`, `while`, `for`, ternary `? :`
- Expressions: arithmetic, comparisons, logical `&&`/`||`/`!`, string concatenation
- `new`, method calls (static, virtual), field access
- Arrays: `new int[n]`, `arr[i]`, `arr.length`
- `import` resolution for JDK classes
- Multi-class source files (compiled to length-prefixed bundle)

### Example snippets included

| Category | Snippets |
| --- | --- |
| Basics | Hello World, Arithmetic, String ops, Loops, Conditionals |
| OOP | Class with fields, Inheritance, Record type, Static methods |
| Algorithms | Fibonacci, Factorial, GCD, Bubble sort, Binary search |
| Collections | ArrayList, List operations |

---

## JDK shim classes

Standard library classes are implemented as **pure Java** in `jdk-shim/`, compiled to bytecode with `javac --patch-module`. This approach avoids native Rust stubs for anything that can be expressed in Java.

Currently shimmed:
- `java.lang`: Object, String, StringBuilder, Integer, Long, Boolean, Record, Enum, ...
- `java.util`: ArrayList, HashMap, Optional, Collections, Arrays, Iterator, ...
- `java.util.stream`: Stream, Collectors (basic)
- `java.util.function`: Function, Predicate, Consumer, Supplier, ...

Native stubs (Rust) are only used for operations requiring host access:
- `String` methods (backed by Rust `String`)
- `PrintStream.println` (output capture)

---

## Supported bytecode

The interpreter covers:

- Load/store: `aload`, `iload`, `lload`, `dload`, `astore`, `istore`, ...
- Constants: `iconst`, `lconst`, `bipush`, `sipush`, `ldc`
- Arithmetic: `iadd`, `isub`, `imul`, `idiv`, `irem`, `ineg`, `ladd`, `lsub`, ...
- Comparisons: `if_icmp*`, `ifle`, `ifeq`, `lcmp`
- Control flow: `goto`, `tableswitch`, `lookupswitch`
- Objects: `new`, `newarray`, `anewarray`, `arraylength`
- Arrays: `iaload`, `iastore`, `aaload`, `aastore`, `baload`, `bastore`
- Fields: `getfield`, `putfield`, `getstatic`, `putstatic`
- Methods: `invokestatic`, `invokevirtual`, `invokespecial`, `invokeinterface`
- `invokedynamic`: LambdaMetafactory, StringConcatFactory, SwitchBootstraps
- Type checks: `instanceof`, `checkcast`
- Exceptions: `athrow`
- `wide` prefix, `dup`, `dup_x1`, `swap`, `pop`, `pop2`

---

## Known limitations

| Area | Status |
| --- | --- |
| Lambda / Stream | `invokedynamic` lambda capture works; stream operations are basic |
| Threads / `synchronized` | Not supported (`monitorenter`/`monitorexit` are no-ops) |
| GC | Reference-counting; no cycle collection |
| Reflection | Not supported |
| `java.io` / `java.net` | Not supported |
| Exception handling (`try`/`catch`) | `athrow` works; catch dispatch is not implemented |
| `float` / `double` | Basic ops work; `Math.*` transcendentals are not stubbed |

---

## Development

```sh
# Run compiler tests
npm test

# Run VM integration tests
export PATH="$HOME/.cargo/bin:$PATH"
cargo test --package jvm-core

# Rebuild everything
./build-shim.sh && npm run build:javac && wasm-pack build jvm-core --target web
```

---

## Contributing

- **Interpreter**: [jvm-core/src/interpreter.rs](jvm-core/src/interpreter.rs) — each opcode is a `match` arm
- **Compiler**: [web/javac.ts](web/javac.ts) — lexer, parser, code generator
- **JDK shims**: [jdk-shim/](jdk-shim/) — pure Java implementations of standard library classes
