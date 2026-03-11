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
- **JDK shims in pure Java** — standard library classes are implemented as Java source compiled to bytecode, not as native Rust stubs
- **Hackable** — the interpreter is ~4,500 lines of Rust; the compiler is ~5,600 lines of TypeScript

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
│   ├── class-reader.ts     # .class / JAR parser for method registry
│   └── javac.test.ts       # compiler test suite (146 tests)
├── jdk-shim/               # JDK standard library shims (pure Java, 249 classes)
│   ├── java/lang/          # String, StringBuilder, Integer, Record, ...
│   ├── java/util/          # ArrayList, HashMap, Optional, stream, ...
│   ├── java/math/          # BigInteger, BigDecimal, MathContext, ...
│   ├── java/time/          # Month, ZoneId, temporal, format, ...
│   ├── java/io/            # InputStream, OutputStream, Serializable, ...
│   ├── java/text/          # DateFormat, SimpleDateFormat, Formatter, ...
│   └── bundle.bin          # compiled shim classes (length-prefixed bundle)
├── raoh-classes/            # Pre-compiled Raoh decoder library (66 classes)
├── build-shim.sh           # compile shim sources → bundle.bin
├── build-test-bundle.sh    # compile test classes → test-classes/bundle.bin
└── build-dist.sh           # build all artifacts and deploy to GCS
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
# → jdk-shim/bundle.bin (249 shim classes)
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

The compiler supports a substantial subset of Java:

- Class declarations with fields, constructors, instance/static methods
- Inheritance (`extends`) with `super()` calls
- Record types (with `Record` class attribute emission)
- Control flow: `if`/`else`, `while`, `do-while`, `for`, enhanced `for`, `break`, `continue`, labeled `break`
- Switch statements and switch expressions (including pattern matching, guards, record patterns)
- Expressions: arithmetic, comparisons, logical `&&`/`||`/`!`, string concatenation, ternary `? :`
- `new`, method calls (static, virtual, interface), field access
- Arrays: `new int[n]`, `arr[i]`, `arr.length`
- Lambda expressions and method references (`invokedynamic` + `LambdaMetafactory`)
- `instanceof` with type patterns and record patterns
- Unboxing / boxing casts (e.g., `(int) someObject`)
- `try`/`catch`/`finally`, `throw`
- `import` resolution for JDK classes (named, wildcard, static)
- Multi-class source files (compiled to length-prefixed bundle)

### Example snippets (playground)

| Category | Snippets |
| --- | --- |
| Basics | Hello World, Arithmetic, String ops, String.formatted(), Loops, Conditionals |
| OOP | Class with fields, Inheritance, Record type, Static methods |
| Algorithms | Fibonacci (recursive/iterative), Factorial, GCD, Bubble sort, Binary search |
| Collections | ArrayList, List operations |
| Modern Java | Lambda & method ref, Switch expression, Pattern matching, Record |
| Raoh | ObjectDecoders, MapDecoders, JsonDecoders (string, int/decimal, field, combine) |
| JVM Showcase | Reflection + Record, BigDecimal, CompletableFuture, ForkJoin, Pattern + Switch + Record |

---

## JDK shim classes

Standard library classes are implemented as **pure Java** in `jdk-shim/`, compiled to bytecode with `javac --patch-module`. Shims target **Java 25 API compatibility** — implementations start from JDK 25 source, replacing only internal API dependencies (`jdk.internal.*`, `sun.*`).

Currently shimmed (249 classes across 15 packages):

- `java.lang`: Object, String, StringBuilder, Integer, Long, Float, Double, Boolean, Character, Math, System, Class, Record, Enum, 30+ exception types
- `java.lang.reflect`: Field, Method, Constructor, Array, RecordComponent, Modifier, ...
- `java.lang.annotation`: Annotation, Target, Retention, ...
- `java.util`: ArrayList, HashMap, HashSet, LinkedHashMap, ArrayDeque, BitSet, Optional, Arrays, Collections, Formatter, ...
- `java.util.stream`: Stream, StreamImpl, Collector, Collectors
- `java.util.function`: Function, BiFunction, Predicate, Consumer, BiConsumer, Supplier
- `java.util.regex`: Pattern, Matcher
- `java.util.concurrent`: ForkJoinPool, CompletableFuture, ExecutorService, CountDownLatch, ConcurrentHashMap, RecursiveTask, ...
- `java.util.concurrent.atomic`: AtomicReference, AtomicLong, ...
- `java.util.concurrent.locks`: Lock, ReentrantLock, Condition
- `java.math`: BigInteger, BigDecimal, MathContext, RoundingMode
- `java.time`: Month, ZoneId, temporal (ChronoField, ChronoUnit), format (DateTimeFormatter)
- `java.text`: DateFormat, SimpleDateFormat
- `java.io`: InputStream, OutputStream, PrintStream, Serializable, ...
- `java.beans`: ConstructorProperties, Transient

Native stubs (Rust) are only used for operations requiring host access:

- `String` methods (backed by Rust `NativePayload::JavaString`)
- `PrintStream.println` / `System.out` (output capture)
- `System.currentTimeMillis` (via `js_sys::Date::now()` on WASM)
- `System.identityHashCode`

---

## Supported bytecode

The interpreter covers:

- Load/store: `aload`, `iload`, `lload`, `fload`, `dload`, `astore`, `istore`, `lstore`, ...
- Constants: `iconst`, `lconst`, `fconst`, `dconst`, `bipush`, `sipush`, `ldc`, `ldc_w`, `ldc2_w`
- Arithmetic: `iadd`, `isub`, `imul`, `idiv`, `irem`, `ineg`, `ladd`, `lsub`, `lmul`, `ldiv`, `fadd`, `fsub`, `fmul`, `fdiv`, `dadd`, `dsub`, `dmul`, `ddiv`, ...
- Type conversion: `i2l`, `i2f`, `i2d`, `l2i`, `l2f`, `l2d`, `f2i`, `f2d`, `d2i`, `d2f`, `i2b`, `i2c`, `i2s`
- Comparisons: `if_icmp*`, `if_acmp*`, `ifle`, `ifeq`, `ifne`, `ifnull`, `ifnonnull`, `lcmp`, `fcmpl`, `fcmpg`, `dcmpl`, `dcmpg`
- Control flow: `goto`, `tableswitch`, `lookupswitch`
- Objects: `new`, `newarray`, `anewarray`, `multianewarray`, `arraylength`
- Arrays: `iaload`, `iastore`, `aaload`, `aastore`, `baload`, `bastore`, `caload`, `castore`, `laload`, `lastore`, `faload`, `fastore`, `daload`, `dastore`
- Fields: `getfield`, `putfield`, `getstatic`, `putstatic`
- Methods: `invokestatic`, `invokevirtual`, `invokespecial`, `invokeinterface`
- `invokedynamic`: LambdaMetafactory, StringConcatFactory, SwitchBootstraps
- Type checks: `instanceof`, `checkcast`
- Exceptions: `athrow`, try/catch dispatch via exception table
- Stack: `dup`, `dup_x1`, `dup_x2`, `dup2`, `swap`, `pop`, `pop2`
- `wide` prefix, `iinc`, `monitorenter`/`monitorexit` (no-op)

---

## Known limitations

| Area | Status |
| --- | --- |
| Lambda / Stream | `invokedynamic` lambda capture works; stream operations cover `map`, `filter`, `reduce`, `collect`, `forEach`, `findAny`, `min`, `toList` |
| Threads / `synchronized` | Not supported (`monitorenter`/`monitorexit` are no-ops) |
| GC | Reference-counting; no cycle collection |
| Reflection | Basic support: `getRecordComponents`, `getClass`, `getSimpleName`, `forName` |
| `java.net` | Not supported |
| `float` / `double` | Arithmetic works; `Math.*` transcendentals are partially stubbed |

---

## Development

```sh
# Run compiler tests (146 tests)
npm test

# Run VM integration tests (8 tests)
cargo test --package jvm-core

# Rebuild everything
./build-shim.sh && npm run build:javac && wasm-pack build jvm-core --target web

# Deploy (incremental upload to GCS)
./build-dist.sh
```

---

## Contributing

- **Interpreter**: [jvm-core/src/interpreter.rs](jvm-core/src/interpreter.rs) — each opcode is a `match` arm
- **Compiler**: [web/javac.ts](web/javac.ts) — lexer, parser, code generator
- **Class reader**: [web/class-reader.ts](web/class-reader.ts) — `.class` / JAR parser for method registry
- **JDK shims**: [jdk-shim/](jdk-shim/) — pure Java implementations of standard library classes
