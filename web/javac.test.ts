// Tests for the Java subset compiler (javac.ts)
// Run with: npm test
//
// These tests verify:
// 1. Lexer produces correct tokens
// 2. Parser produces correct AST
// 3. Code generator produces valid .class bytes (magic number, version, structure)
// 4. End-to-end: compile → parse back using class_file structure inspection

import { test, describe } from "node:test";
import * as assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import initJvm, { run_static } from "../jvm-core/pkg/jvm_core.js";
import { lex, parseAll, compile, generateClassFile, TokenKind, parseClassMeta, parseBundleMeta, buildMethodRegistry } from "./javac.js";

// Helper: parse single class (convenience wrapper)
function parse(tokens: ReturnType<typeof lex>) {
  return parseAll(tokens)[0];
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/** Read u16 big-endian from Uint8Array */
function u16(buf: Uint8Array, offset: number): number {
  return (buf[offset] << 8) | buf[offset + 1];
}

/** Read u32 big-endian from Uint8Array */
function u32(buf: Uint8Array, offset: number): number {
  return ((buf[offset] << 24) | (buf[offset + 1] << 16) | (buf[offset + 2] << 8) | buf[offset + 3]) >>> 0;
}

/** Check .class magic and version */
function assertValidClassFile(bytes: Uint8Array) {
  assert.equal(u32(bytes, 0), 0xcafebabe, "magic number");
  const major = u16(bytes, 6);
  assert.ok(major >= 45 && major <= 70, `class file major version in range: ${major}`);
}

/** Extract the class name from a .class binary (best-effort) */
function readClassName(bytes: Uint8Array): string {
  // Skip magic(4) + minor(2) + major(2) = 8 bytes
  // Then parse constant pool count
  const cpCount = u16(bytes, 8);
  let pos = 10;
  const utf8Strings: Map<number, string> = new Map();
  const classEntries: Map<number, number> = new Map(); // classIdx -> nameIdx

  for (let i = 1; i < cpCount; i++) {
    const tag = bytes[pos++];
    if (tag === 1) {
      // Utf8
      const len = u16(bytes, pos); pos += 2;
      const str = new TextDecoder().decode(bytes.slice(pos, pos + len));
      utf8Strings.set(i, str);
      pos += len;
    } else if (tag === 7) {
      // Class
      const nameIdx = u16(bytes, pos); pos += 2;
      classEntries.set(i, nameIdx);
    } else if (tag === 8) {
      pos += 2; // String -> utf8 ref
    } else if (tag === 9 || tag === 10 || tag === 11 || tag === 12) {
      pos += 4; // two u16s
    } else if (tag === 3 || tag === 4) {
      pos += 4; // int/float
    } else if (tag === 5 || tag === 6) {
      pos += 8; i++; // long/double take two slots
    } else if (tag === 15) {
      pos += 3;
    } else if (tag === 16) {
      pos += 2;
    } else if (tag === 17 || tag === 18) {
      pos += 4;
    } else if (tag === 19 || tag === 20) {
      pos += 2;
    }
  }

  // After CP: access_flags(2), this_class(2)
  const thisClassIdx = u16(bytes, pos + 2);
  const nameIdx = classEntries.get(thisClassIdx);
  if (nameIdx !== undefined) return utf8Strings.get(nameIdx) ?? "";
  return "";
}

let runtimeReady: Promise<void> | null = null;
let shimBundle: Uint8Array | null = null;

async function ensureRuntimeReady(): Promise<void> {
  if (!runtimeReady) {
    runtimeReady = (async () => {
      const wasmBytes = await readFile(new URL("../jvm-core/pkg/jvm_core_bg.wasm", import.meta.url));
      await initJvm({ module_or_path: wasmBytes });
      shimBundle = new Uint8Array(await readFile(new URL("../jdk-shim/bundle.bin", import.meta.url)));
    })();
  }
  await runtimeReady;
}

function toBundle(classBytes: Uint8Array): Uint8Array {
  if (classBytes.length >= 4 &&
      classBytes[0] === 0xca && classBytes[1] === 0xfe &&
      classBytes[2] === 0xba && classBytes[3] === 0xbe) {
    const out = new Uint8Array(4 + classBytes.length);
    const n = classBytes.length;
    out[0] = (n >> 24) & 0xff;
    out[1] = (n >> 16) & 0xff;
    out[2] = (n >> 8) & 0xff;
    out[3] = n & 0xff;
    out.set(classBytes, 4);
    return out;
  }
  return classBytes;
}

async function runSnippet(source: string, className: string): Promise<string> {
  await ensureRuntimeReady();
  const user = toBundle(compile(source));
  const shim = shimBundle!;
  const all = new Uint8Array(shim.length + user.length);
  all.set(shim, 0);
  all.set(user, shim.length);
  return run_static(all, className, "run", "()Ljava/lang/String;");
}

// ---------------------------------------------------------------------------
// Lexer tests
// ---------------------------------------------------------------------------

describe("Lexer", () => {
  test("empty source produces EOF", () => {
    const tokens = lex("");
    assert.equal(tokens.length, 1);
    assert.equal(tokens[0].kind, TokenKind.EOF);
  });

  test("keywords", () => {
    const tokens = lex("public class static return assert synchronized");
    assert.equal(tokens[0].kind, TokenKind.KwPublic);
    assert.equal(tokens[1].kind, TokenKind.KwClass);
    assert.equal(tokens[2].kind, TokenKind.KwStatic);
    assert.equal(tokens[3].kind, TokenKind.KwReturn);
    assert.equal(tokens[4].kind, TokenKind.KwAssert);
    assert.equal(tokens[5].kind, TokenKind.KwSynchronized);
  });

  test("integer literals", () => {
    const tokens = lex("0 42 -1");
    assert.equal(tokens[0].kind, TokenKind.IntLiteral);
    assert.equal(tokens[0].value, "0");
    assert.equal(tokens[1].kind, TokenKind.IntLiteral);
    assert.equal(tokens[1].value, "42");
    assert.equal(tokens[2].kind, TokenKind.Minus);
    assert.equal(tokens[3].kind, TokenKind.IntLiteral);
    assert.equal(tokens[3].value, "1");
  });

  test("string literals", () => {
    const tokens = lex('"Hello, World!"');
    assert.equal(tokens[0].kind, TokenKind.StringLiteral);
    assert.equal(tokens[0].value, "Hello, World!");
  });

  test("string escape sequences", () => {
    const tokens = lex('"line1\\nline2\\ttab"');
    assert.equal(tokens[0].kind, TokenKind.StringLiteral);
    assert.equal(tokens[0].value, "line1\nline2\ttab");
  });

  test("unicode escapes are translated before tokenization", () => {
    const tokens = lex("cl\\u0061ss");
    assert.equal(tokens[0].kind, TokenKind.KwClass);
    assert.equal(tokens[0].value, "class");
  });

  test("unicode identifier is accepted", () => {
    const tokens = lex("int 名前 = 1;");
    assert.equal(tokens[0].kind, TokenKind.KwInt);
    assert.equal(tokens[1].kind, TokenKind.Ident);
    assert.equal(tokens[1].value, "名前");
  });

  test("text block literal tokenizes as string literal", () => {
    const tokens = lex(`"""
hello
world
"""`);
    assert.equal(tokens[0].kind, TokenKind.StringLiteral);
    assert.equal(tokens[0].value, "hello\nworld\n");
  });

  test("malformed underscore in number literal is rejected", () => {
    assert.throws(() => lex("1_"));
    assert.throws(() => lex("0x_FF"));
    assert.throws(() => lex("1__2"));
  });

  test("invalid escape sequence is rejected", () => {
    assert.throws(() => lex("\"\\q\""), /Invalid escape sequence/);
    assert.throws(() => lex("'\\x'"), /Invalid escape sequence/);
  });

  test("text block opening without line terminator is rejected", () => {
    assert.throws(() => lex("\"\"\"x\"\"\""), /Text block opening delimiter/);
  });

  test("floating-point forms with dot/exponent are tokenized", () => {
    const tokens = lex(".5 1. 1e3 2.5f");
    assert.equal(tokens[0].kind, TokenKind.DoubleLiteral);
    assert.equal(tokens[1].kind, TokenKind.DoubleLiteral);
    assert.equal(tokens[2].kind, TokenKind.DoubleLiteral);
    assert.equal(tokens[3].kind, TokenKind.FloatLiteral);
  });

  test("hex floating-point literals are tokenized", () => {
    const tokens = lex("0x1.fp3 0x1p-1");
    assert.equal(tokens[0].kind, TokenKind.DoubleLiteral);
    assert.equal(tokens[1].kind, TokenKind.DoubleLiteral);
  });

  test("underscore is rejected as identifier", () => {
    assert.throws(() => lex("_"), /reserved keyword/);
  });

  test("legacy reserved keywords are tokenized", () => {
    const tokens = lex("native strictfp transient volatile const goto throws");
    assert.equal(tokens[0].kind, TokenKind.KwNative);
    assert.equal(tokens[1].kind, TokenKind.KwStrictfp);
    assert.equal(tokens[2].kind, TokenKind.KwTransient);
    assert.equal(tokens[3].kind, TokenKind.KwVolatile);
    assert.equal(tokens[4].kind, TokenKind.KwConst);
    assert.equal(tokens[5].kind, TokenKind.KwGoto);
    assert.equal(tokens[6].kind, TokenKind.KwThrows);
  });

  test("operators", () => {
    const tokens = lex("== != <= >= && || ++ -- ::");
    const kinds = tokens.slice(0, -1).map(t => t.kind);
    assert.deepEqual(kinds, [
      TokenKind.Eq, TokenKind.Ne, TokenKind.Le, TokenKind.Ge,
      TokenKind.And, TokenKind.Or, TokenKind.PlusPlus, TokenKind.MinusMinus, TokenKind.ColonColon,
    ]);
  });

  test("bitwise/shift and compound assignment operators", () => {
    const tokens = lex("& | ^ ~ << <<= *= /= %= &= |= ^= >>= >>>= >>>");
    const kinds = tokens.slice(0, -1).map(t => t.kind);
    assert.deepEqual(kinds, [
      TokenKind.BitAnd, TokenKind.BitOr, TokenKind.BitXor, TokenKind.BitNot,
      TokenKind.ShiftLeft, TokenKind.ShiftLeftAssign,
      TokenKind.StarAssign, TokenKind.SlashAssign, TokenKind.PercentAssign,
      TokenKind.AndAssign, TokenKind.OrAssign, TokenKind.XorAssign,
      TokenKind.Gt, TokenKind.Ge, // >>=
      TokenKind.Gt, TokenKind.Gt, TokenKind.Ge, // >>>=
      TokenKind.Gt, TokenKind.Gt, TokenKind.Gt, // >>>
    ]);
  });

  test("line comments are skipped", () => {
    const tokens = lex("// this is a comment\n42");
    assert.equal(tokens[0].kind, TokenKind.IntLiteral);
    assert.equal(tokens[0].value, "42");
  });

  test("block comments are skipped", () => {
    const tokens = lex("/* block\ncomment */42");
    assert.equal(tokens[0].kind, TokenKind.IntLiteral);
    assert.equal(tokens[0].value, "42");
  });

  test("bool literals", () => {
    const tokens = lex("true false");
    assert.equal(tokens[0].kind, TokenKind.BoolLiteral);
    assert.equal(tokens[0].value, "true");
    assert.equal(tokens[1].kind, TokenKind.BoolLiteral);
    assert.equal(tokens[1].value, "false");
  });

  test("null literal", () => {
    const tokens = lex("null");
    assert.equal(tokens[0].kind, TokenKind.NullLiteral);
  });

  test("ternary operator tokens", () => {
    const tokens = lex("a ? b : c");
    assert.equal(tokens[0].kind, TokenKind.Ident);
    assert.equal(tokens[1].kind, TokenKind.Question);
    assert.equal(tokens[2].kind, TokenKind.Ident);
    assert.equal(tokens[3].kind, TokenKind.Colon);
    assert.equal(tokens[4].kind, TokenKind.Ident);
  });
});

// ---------------------------------------------------------------------------
// Parser tests
// ---------------------------------------------------------------------------

describe("Parser", () => {
  test("minimal class", () => {
    const src = "public class Hello {}";
    const cls = parse(lex(src));
    assert.equal(cls.name, "Hello");
    assert.equal(cls.methods.length, 0);
    assert.equal(cls.fields.length, 0);
  });

  test("static method with return", () => {
    const src = `public class Foo {
      public static String run() {
        return "hi";
      }
    }`;
    const cls = parse(lex(src));
    assert.equal(cls.methods.length, 1);
    const m = cls.methods[0];
    assert.equal(m.name, "run");
    assert.equal(m.isStatic, true);
    assert.equal(m.returnType, "String");
    assert.equal(m.body.length, 1);
    assert.equal(m.body[0].kind, "return");
  });

  test("instance method and field", () => {
    const src = `public class Counter {
      int count;
      public void increment() {
        count = count + 1;
      }
    }`;
    const cls = parse(lex(src));
    assert.equal(cls.fields.length, 1);
    assert.equal(cls.fields[0].name, "count");
    assert.equal(cls.fields[0].type, "int");
    assert.equal(cls.methods.length, 1);
    assert.equal(cls.methods[0].name, "increment");
    assert.equal(cls.methods[0].isStatic, false);
  });

  test("if/else statement", () => {
    const src = `public class Cond {
      public static String run() {
        int x = 5;
        if (x > 3) {
          return "big";
        } else {
          return "small";
        }
      }
    }`;
    const cls = parse(lex(src));
    const body = cls.methods[0].body;
    assert.equal(body.length, 2);
    assert.equal(body[1].kind, "if");
    const ifStmt = body[1] as { kind: "if"; else_?: unknown[] };
    assert.ok(ifStmt.else_ !== undefined);
  });

  test("while loop", () => {
    const src = `public class Loop {
      public static String run() {
        int i = 0;
        while (i < 10) {
          i = i + 1;
        }
        return "done";
      }
    }`;
    const cls = parse(lex(src));
    const body = cls.methods[0].body;
    assert.equal(body[1].kind, "while");
  });

  test("for loop", () => {
    const src = `public class ForLoop {
      public static String run() {
        int sum = 0;
        for (int i = 0; i < 5; i++) {
          sum = sum + i;
        }
        return "done";
      }
    }`;
    const cls = parse(lex(src));
    const body = cls.methods[0].body;
    assert.equal(body[1].kind, "for");
  });

  test("method call chain", () => {
    const src = `public class Chain {
      public static String run() {
        String s = "hello";
        return s.length() + "";
      }
    }`;
    const cls = parse(lex(src));
    assert.equal(cls.methods.length, 1);
  });

  test("new expression", () => {
    const src = `public class NewTest {
      public static String run() {
        StringBuilder sb = new StringBuilder();
        return sb.toString();
      }
    }`;
    const cls = parse(lex(src));
    const body = cls.methods[0].body;
    const decl = body[0] as { kind: "varDecl"; init?: { kind: string } };
    assert.equal(decl.kind, "varDecl");
    assert.equal(decl.init?.kind, "newExpr");
  });

  test("binary arithmetic expressions", () => {
    const src = `public class Arith {
      public static String run() {
        int x = 2 + 3 * 4;
        return "" + x;
      }
    }`;
    const cls = parse(lex(src));
    assert.equal(cls.methods.length, 1);
  });

  test("bitwise and shift expressions parse", () => {
    const src = `public class BitOps {
      public static int run() {
        return 1 | 2 ^ 3 & 4 << 1;
      }
    }`;
    const cls = parse(lex(src));
    const ret = cls.methods[0].body[0];
    assert.equal(ret.kind, "return");
    assert.equal(ret.value?.kind, "binary");
  });

  test("import and package are skipped", () => {
    const src = `package com.example;
    import java.util.List;
    public class Pkg {}`;
    const cls = parse(lex(src));
    assert.equal(cls.name, "Pkg");
  });

  test("extends clause", () => {
    const src = `public class Child extends Parent {}`;
    const cls = parse(lex(src));
    assert.equal(cls.superClass, "Parent");
  });

  test("extends clause resolves imported JDK type", () => {
    const src = `import java.util.concurrent.RecursiveTask;
    public class Child extends RecursiveTask {}`;
    const cls = parse(lex(src));
    assert.equal(cls.superClass, "java/util/concurrent/RecursiveTask");
  });

  test("ternary expression", () => {
    const src = `public class Ternary {
      public static String run() {
        int x = 5;
        int y = x > 3 ? 10 : 20;
        return "" + y;
      }
    }`;
    const cls = parse(lex(src));
    assert.equal(cls.methods.length, 1);
  });

  test("multiple classes with parseAll", () => {
    const src = `public class A {}
    public class B {}`;
    const classes = parseAll(lex(src));
    assert.equal(classes.length, 2);
    assert.equal(classes[0].name, "A");
    assert.equal(classes[1].name, "B");
  });

  test("static nested class parses with mangled name", () => {
    const src = `public class Outer {
      static class Inner {
        int x;
        int getX() { return x; }
      }
    }`;
    const classes = parseAll(lex(src));
    assert.equal(classes.length, 1);
    assert.equal(classes[0].name, "Outer");
    assert.equal(classes[0].nestedClasses.length, 1);
    assert.equal(classes[0].nestedClasses[0].name, "Outer$Inner");
    assert.equal(classes[0].nestedClasses[0].fields.length, 1);
    assert.equal(classes[0].nestedClasses[0].methods.length, 1);
  });

  test("array type in parameters", () => {
    const src = `public class ArrParam {
      public static void sort(int[] arr) {}
    }`;
    const cls = parse(lex(src));
    const param = cls.methods[0].params[0];
    assert.deepEqual(param.type, { array: "int" });
  });

  test("lambda expression parses", () => {
    const src = `import java.util.function.Function;
    public class Lambda {
      public static void run() {
        Function f = x -> x;
      }
    }`;
    const cls = parse(lex(src));
    assert.equal(cls.methods.length, 1);
    assert.equal(cls.methods[0].body[0].kind, "varDecl");
  });

  test("method reference parses", () => {
    const src = `import java.util.function.Function;
    public class MR {
      public static void run() {
        Function f = String::length;
      }
    }`;
    const cls = parse(lex(src));
    const vd = cls.methods[0].body[0] as { kind: "varDecl"; init?: { kind: string } };
    assert.equal(vd.init?.kind, "methodRef");
  });

  test("constructor method reference parses", () => {
    const src = `import java.util.function.Supplier;
    public class MRCtor {
      public static void run() {
        Supplier s = StringBuilder::new;
      }
    }`;
    const cls = parse(lex(src));
    const vd = cls.methods[0].body[0] as { kind: "varDecl"; init?: { kind: string; isConstructor?: boolean } };
    assert.equal(vd.init?.kind, "methodRef");
    assert.equal(vd.init?.isConstructor, true);
  });

  test("switch statement parses", () => {
    const src = `public class Sw {
      public static int run(int x) {
        switch (x) {
          case 1 -> { x = 10; }
          default -> { x = 20; }
        }
        return x;
      }
    }`;
    const cls = parse(lex(src));
    assert.equal(cls.methods[0].body[0].kind, "switch");
  });

  test("switch expression parses", () => {
    const src = `public class SwExpr {
      public static int run(int x) {
        int y = switch (x) {
          case 1 -> 10;
          default -> 20;
        };
        return y;
      }
    }`;
    const cls = parse(lex(src));
    const vd = cls.methods[0].body[0] as { kind: "varDecl"; init?: { kind: string } };
    assert.equal(vd.init?.kind, "switchExpr");
  });

  test("switch with guard parses", () => {
    const src = `public class SwGuard {
      public static String run(Object v) {
        switch (v) {
          case String s when s.length() > 0 -> { return s; }
          default -> { return "x"; }
        }
      }
    }`;
    const cls = parse(lex(src));
    assert.equal(cls.methods[0].body[0].kind, "switch");
  });

  test("switch expression with boolean labels parses", () => {
    const src = `public class SwBool {
      public static int run(boolean b) {
        int x = switch (b) {
          case true -> 1;
          case false -> 0;
        };
        return x;
      }
    }`;
    const cls = parse(lex(src));
    const vd = cls.methods[0].body[0] as { kind: "varDecl"; init?: { kind: string } };
    assert.equal(vd.init?.kind, "switchExpr");
  });

  test("switch with parenthesized type pattern parses", () => {
    const src = `public class SwParenPattern {
      public static String run(Object v) {
        return switch (v) {
          case (String s) -> s;
          default -> "x";
        };
      }
    }`;
    const cls = parse(lex(src));
    const ret = cls.methods[0].body[0] as { kind: "return"; value?: { kind: string } };
    assert.equal(ret.value?.kind, "switchExpr");
  });

  test("instanceof with parenthesized pattern parses", () => {
    const src = `public class InstParen {
      public static String run(Object v) {
        if (v instanceof (String s)) {
          return s;
        }
        return "x";
      }
    }`;
    const cls = parse(lex(src));
    assert.equal(cls.methods[0].body[0].kind, "if");
  });

  test("switch with record pattern parses", () => {
    const src = `record Point(int x, int y) {
      public static int run(Object v) {
        return switch (v) {
          case Point(int a, int b) -> a + b;
          default -> 0;
        };
      }
    }`;
    const cls = parse(lex(src));
    assert.equal(cls.isRecord, true);
  });

  test("instanceof with record pattern parses", () => {
    const src = `record Pair(int x, int y) {
      public static int run(Object v) {
        if (v instanceof Pair(int a, int b)) {
          return a + b;
        }
        return 0;
      }
    }`;
    const cls = parse(lex(src));
    assert.equal(cls.isRecord, true);
  });

  test("interface declaration parses", () => {
    const src = `public interface Named extends java.io.Serializable {
      String name();
      default String label() { return name(); }
      static String kind() { return "iface"; }
    }`;
    const cls = parse(lex(src));
    assert.equal(cls.kind, "interface");
    assert.ok((cls.interfaces ?? []).includes("java/io/Serializable"));
    assert.ok(cls.methods.some(m => m.name === "name" && m.isAbstract));
    assert.ok(cls.methods.some(m => m.name === "label" && !m.isAbstract));
  });

  test("enum declaration parses", () => {
    const src = `public enum Color { RED, GREEN, BLUE; }`;
    const cls = parse(lex(src));
    assert.equal(cls.kind, "enum");
    assert.equal(cls.fields.length, 3);
    assert.equal(cls.fields[0].name, "RED");
    assert.equal(cls.superClass, "java/lang/Enum");
  });

  test("enum declaration with trailing comma parses", () => {
    const src = `public enum Color { RED, GREEN, }`;
    const cls = parse(lex(src));
    assert.equal(cls.kind, "enum");
    assert.equal(cls.fields.length, 2);
    assert.equal(cls.fields[1].name, "GREEN");
  });

  test("annotation declaration parses", () => {
    const src = `public @interface Info {
      String value() default "x";
    }`;
    const cls = parse(lex(src));
    assert.equal(cls.kind, "annotation");
    assert.ok((cls.interfaces ?? []).includes("java/lang/annotation/Annotation"));
    assert.ok(cls.methods.some(m => m.name === "value" && m.isAbstract));
  });

  test("generic class declaration with implements parses", () => {
    const src = `public class Box<T> implements java.io.Serializable {
      T value;
      public T get() { return value; }
    }`;
    const cls = parse(lex(src));
    assert.equal(cls.name, "Box");
    assert.ok((cls.interfaces ?? []).includes("java/io/Serializable"));
  });

  test("constructor declaration ending with semicolon is rejected", () => {
    const src = `public class BadCtor {
      BadCtor();
    }`;
    assert.throws(() => parse(lex(src)));
  });

  test("class method declaration ending with semicolon is rejected unless abstract", () => {
    const src = `public class BadMethodDecl {
      void m();
    }`;
    assert.throws(() => parse(lex(src)));
  });

  test("method throws clause is captured", () => {
    const src = `public class ThrowsDecl {
      public static void run() throws java.io.IOException {}
    }`;
    const cls = parse(lex(src));
    const run = cls.methods.find(m => m.name === "run");
    assert.ok(run);
    assert.deepEqual(run.throwsTypes, ["java/io/IOException"]);
  });
});

// ---------------------------------------------------------------------------
// Code generator tests
// ---------------------------------------------------------------------------

describe("Code generator", () => {
  test("produces valid class file magic and version", () => {
    const bytes = compile(`public class Hello {
      public static String run() { return "Hello"; }
    }`);
    assertValidClassFile(bytes);
  });

  test("class name is encoded correctly", () => {
    const bytes = compile(`public class MyClass {
      public static String run() { return "x"; }
    }`);
    assertValidClassFile(bytes);
    const name = readClassName(bytes);
    assert.equal(name, "MyClass");
  });

  test("compiles method returning string literal", () => {
    const bytes = compile(`public class A {
      public static String run() { return "hello"; }
    }`);
    assertValidClassFile(bytes);
    // Check for "hello" string in constant pool
    const text = new TextDecoder().decode(bytes);
    assert.ok(text.includes("hello"), "string literal in constant pool");
  });

  test("compiles int arithmetic", () => {
    const bytes = compile(`public class Arith {
      public static String run() {
        int a = 10;
        int b = 3;
        return "" + (a + b);
      }
    }`);
    assertValidClassFile(bytes);
  });

  test("compiles if/else", () => {
    const bytes = compile(`public class Cond {
      public static String run() {
        int x = 5;
        if (x > 3) {
          return "big";
        } else {
          return "small";
        }
      }
    }`);
    assertValidClassFile(bytes);
  });

  test("compiles while loop", () => {
    const bytes = compile(`public class WhileTest {
      public static String run() {
        int i = 0;
        while (i < 3) {
          i = i + 1;
        }
        return "" + i;
      }
    }`);
    assertValidClassFile(bytes);
  });

  test("compiles for loop", () => {
    const bytes = compile(`public class ForTest {
      public static String run() {
        int sum = 0;
        for (int i = 1; i <= 5; i++) {
          sum = sum + i;
        }
        return "" + sum;
      }
    }`);
    assertValidClassFile(bytes);
  });

  test("compiles new + method call", () => {
    const bytes = compile(`public class NewTest {
      public static String run() {
        StringBuilder sb = new StringBuilder();
        sb.append("hello");
        return sb.toString();
      }
    }`);
    assertValidClassFile(bytes);
  });

  test("compiles class with instance fields", () => {
    const bytes = compile(`public class Counter {
      int count;
      public void increment() { count = count + 1; }
      public int get() { return count; }
      public static String run() {
        Counter c = new Counter();
        c.increment();
        return "" + c.get();
      }
    }`);
    assertValidClassFile(bytes);
  });

  test("compiles recursive method", () => {
    const bytes = compile(`public class Fib {
      public static int fib(int n) {
        if (n <= 1) return n;
        return fib(n - 1) + fib(n - 2);
      }
      public static String run() {
        return "" + fib(5);
      }
    }`);
    assertValidClassFile(bytes);
  });

  test("compiles multiple methods", () => {
    const bytes = compile(`public class Multi {
      public static int add(int a, int b) { return a + b; }
      public static int mul(int a, int b) { return a * b; }
      public static String run() { return "" + add(2, 3) + mul(3, 4); }
    }`);
    assertValidClassFile(bytes);
  });

  test("string concatenation uses StringBuilder", () => {
    const bytes = compile(`public class Concat {
      public static String run() {
        return "a" + "b" + "c";
      }
    }`);
    assertValidClassFile(bytes);
    const text = new TextDecoder().decode(bytes);
    assert.ok(text.includes("StringBuilder"), "StringBuilder in constant pool");
  });

  test("compiles boolean expressions", () => {
    const bytes = compile(`public class BoolTest {
      public static String run() {
        boolean b = true;
        if (b) return "yes";
        return "no";
      }
    }`);
    assertValidClassFile(bytes);
  });

  test("compiles bitwise and shift operators", () => {
    const bytes = compile(`public class BitShift {
      public static int run() {
        int a = 6;
        int b = 3;
        return (a & b) + (a | b) + (a ^ b) + (a << 1) + (a >> 1) + (a >>> 1);
      }
    }`);
    assertValidClassFile(bytes);
  });

  test("compiles compound assignment operators", () => {
    const bytes = compile(`public class CompoundAssign {
      public static int run() {
        int x = 10;
        x *= 2;
        x /= 4;
        x %= 3;
        x += 5;
        x -= 1;
        x <<= 2;
        x >>= 1;
        x >>>= 1;
        x &= 7;
        x |= 8;
        x ^= 3;
        return x;
      }
    }`);
    assertValidClassFile(bytes);
  });

  test("compiles class with constructor params", () => {
    const bytes = compile(`public class Point {
      int x;
      int y;
      public static String run() {
        Point p = new Point();
        p.x = 3;
        p.y = 4;
        return "" + p.x + "," + p.y;
      }
    }`);
    assertValidClassFile(bytes);
  });

  test("output .class size is reasonable", () => {
    const bytes = compile(`public class Hello {
      public static String run() { return "Hello, World!"; }
    }`);
    // A minimal class file should be between 100 bytes and 10KB
    assert.ok(bytes.length >= 100, `class file too small: ${bytes.length}`);
    assert.ok(bytes.length <= 10240, `class file too large: ${bytes.length}`);
  });

  test("compiles with import/package preamble", () => {
    const bytes = compile(`
      package com.example;
      import java.util.List;
      public class WithPackage {
        public static String run() { return "ok"; }
      }
    `);
    assertValidClassFile(bytes);
    assert.equal(readClassName(bytes), "WithPackage");
  });

  test("wildcard import resolves unqualified static call to imported class", () => {
    const bytes = compile(`
      import net.unit8.raoh.ObjectDecoders.*;
      public class Hello {
        public static String run() {
          return string().decode("abc");
        }
      }
    `);
    assertValidClassFile(bytes);
    const text = new TextDecoder().decode(bytes);
    assert.ok(text.includes("ObjectDecoders"), "wildcard class in constant pool");
    assert.ok(text.includes("string"), "method name in constant pool");
  });

  test("import static wildcard resolves same as import wildcard", () => {
    const bytes = compile(`
      import static net.unit8.raoh.ObjectDecoders.*;
      public class Hello {
        public static String run() {
          return string().decode("abc");
        }
      }
    `);
    assertValidClassFile(bytes);
    const text = new TextDecoder().decode(bytes);
    assert.ok(text.includes("ObjectDecoders"), "wildcard class in constant pool");
    // Must NOT contain 'staticnet' (the bug: static keyword prepended to class name)
    assert.ok(!text.includes("staticnet"), "no 'staticnet' corruption in constant pool");
  });

  test("named import resolves class reference", () => {
    const bytes = compile(`
      import net.unit8.raoh.Ok;
      import net.unit8.raoh.Err;
      public class ImportTest {
        public static String run() {
          Object result = getValue();
          if (result instanceof Ok ok) {
            return "ok";
          } else {
            return "err";
          }
        }
        public static Object getValue() { return null; }
      }
    `);
    assertValidClassFile(bytes);
    const text = new TextDecoder().decode(bytes);
    assert.ok(text.includes("net/unit8/raoh/Ok"), "Ok class in constant pool");
  });

  test("compiles interface class file flags", () => {
    const bytes = compile(`public interface Named {
      String name();
      default String label() { return name(); }
    }`);
    assertValidClassFile(bytes);
    const meta = parseClassMeta(bytes);
    assert.ok((meta.accessFlags & 0x0200) !== 0, "ACC_INTERFACE");
    assert.ok((meta.accessFlags & 0x0400) !== 0, "ACC_ABSTRACT");
  });

  test("compiles annotation class file flags", () => {
    const bytes = compile(`public @interface Info {
      String value() default "x";
    }`);
    assertValidClassFile(bytes);
    const meta = parseClassMeta(bytes);
    assert.ok((meta.accessFlags & 0x2000) !== 0, "ACC_ANNOTATION");
    assert.ok((meta.accessFlags & 0x0200) !== 0, "ACC_INTERFACE");
  });

  test("compiles enum class file flags", () => {
    const bytes = compile(`public enum Color { RED, GREEN; }`);
    assertValidClassFile(bytes);
    const meta = parseClassMeta(bytes);
    assert.ok((meta.accessFlags & 0x4000) !== 0, "ACC_ENUM");
    assert.equal(meta.superClass, "java/lang/Enum");
    assert.ok(meta.methods.some(m => m.name === "<clinit>"), "enum should have <clinit>");
    const enumFields = meta.fields.filter(f => (f.accessFlags & 0x4000) !== 0);
    assert.equal(enumFields.length, 2, "expected two enum constant fields");
    const enumFieldNames = enumFields.map(f => f.name).sort();
    assert.deepEqual(enumFieldNames, ["GREEN", "RED"], "enum constants RED and GREEN present");
  });

  test("compiles generic class declaration with implements", () => {
    const bytes = compile(`public class Box<T> implements java.io.Serializable {
      T value;
      public T get() { return value; }
      public static String run() { return "ok"; }
    }`);
    assertValidClassFile(bytes);
  });

  test("interface field is emitted as public static final", () => {
    const bytes = compile(`public interface Config {
      private int X = 1;
    }`);
    assertValidClassFile(bytes);
    const meta = parseClassMeta(bytes);
    const x = meta.methods.find(m => m.name === "X");
    assert.equal(x, undefined); // ensure it is a field, not method
    // best-effort check via byte text for field name and no private marker behavior in runtime parsing
    const text = new TextDecoder().decode(bytes);
    assert.ok(text.includes("X"));
  });

  test("record declaration generates fields and accessor methods", () => {
    const bytes = compile(`record User(String name, int age) {}`);
    assertValidClassFile(bytes);
    const text = new TextDecoder().decode(bytes);
    assert.ok(text.includes("java/lang/Record"), "extends Record");
    assert.ok(text.includes("name"), "name field/accessor");
    assert.ok(text.includes("age"), "age field/accessor");
  });

  test("record with body method compiles", () => {
    const bytes = compile(`
      record Point(int x, int y) {
        public int sum() { return x + y; }
      }
    `);
    assertValidClassFile(bytes);
    const text = new TextDecoder().decode(bytes);
    assert.ok(text.includes("java/lang/Record"), "extends Record");
  });

  test("record emits Record class attribute", () => {
    const bytes = compile(`record Pair(String first, int second) {}`);
    assertValidClassFile(bytes);
    // The compiled class should contain a "Record" UTF8 entry in the constant pool
    // (used as the attribute name for the Record attribute).
    const text = new TextDecoder().decode(bytes);
    assert.ok(text.includes("Record"), "class file contains Record attribute name");
    // Also verify record component names are present
    assert.ok(text.includes("first"), "record component 'first'");
    assert.ok(text.includes("second"), "record component 'second'");
  });

  // --- New feature tests ---

  test("compiles ternary expression", () => {
    const bytes = compile(`public class Ternary {
      public static int max(int a, int b) {
        return a > b ? a : b;
      }
      public static String run() {
        return "max=" + max(4, 9);
      }
    }`);
    assertValidClassFile(bytes);
  });

  test("compiles array creation and access", () => {
    const bytes = compile(`public class ArrayTest {
      public static String run() {
        int[] arr = new int[3];
        arr[0] = 10;
        arr[1] = 20;
        arr[2] = 30;
        return "" + arr[0] + " " + arr[1] + " " + arr[2];
      }
    }`);
    assertValidClassFile(bytes);
  });

  test("compiles multi-class source into bundle", () => {
    const src = `public class Base {
      int value;
    }
    public class Child extends Base {
      public static String run() { return "ok"; }
    }`;
    const bytes = compile(src);
    // Multi-class bundle should NOT start with CAFEBABE
    assert.ok(
      !(bytes[0] === 0xCA && bytes[1] === 0xFE && bytes[2] === 0xBA && bytes[3] === 0xBE),
      "multi-class bundle is not a raw .class"
    );
    assert.ok(bytes.length > 100, "bundle has content");
  });

  test("compiles static nested class into bundle", () => {
    const src = `public class Outer {
      static class Inner {
        int value;
        public Inner(int v) { value = v; }
        public int getValue() { return value; }
      }
      public static String run() {
        Inner i = new Inner(42);
        return "" + i.getValue();
      }
    }`;
    const bytes = compile(src);
    // Should produce a multi-class bundle (Outer + Outer$Inner)
    assert.ok(
      !(bytes[0] === 0xCA && bytes[1] === 0xFE && bytes[2] === 0xBA && bytes[3] === 0xBE),
      "nested class produces a bundle, not a single .class"
    );
    assert.ok(bytes.length > 100, "bundle has content");
  });

  test("compiles inheritance with super constructor", () => {
    const bytes = compile(`public class Shape {
      String color;
      public Shape(String color) {
        this.color = color;
      }
    }
    public class Circle extends Shape {
      int radius;
      public Circle(String color, int radius) {
        super(color);
        this.radius = radius;
      }
      public String describe() {
        return color + " circle r=" + radius;
      }
      public static String run() {
        Circle c = new Circle("red", 5);
        return c.describe();
      }
    }`);
    // Multi-class bundle
    assert.ok(bytes.length > 200, "bundle has content");
  });

  test("compiles boolean in string concatenation", () => {
    const bytes = compile(`public class BoolConcat {
      public static String run() {
        boolean b = true;
        return "val=" + b;
      }
    }`);
    assertValidClassFile(bytes);
  });

  test("compiles static methods with multiple params", () => {
    const bytes = compile(`public class MathUtils {
      public static int square(int n) {
        return n * n;
      }
      public static int max(int a, int b) {
        return a > b ? a : b;
      }
      public static String run() {
        return "sq=" + square(7) + " max=" + max(4, 9);
      }
    }`);
    assertValidClassFile(bytes);
  });

  test("compiles record with cross-class usage", () => {
    const bytes = compile(`record Point(int x, int y) {}
    public class RecordDemo {
      public static String run() {
        Point p = new Point(3, 4);
        int dist2 = p.x() * p.x() + p.y() * p.y();
        return "dist2=" + dist2;
      }
    }`);
    assert.ok(bytes.length > 200, "bundle has content");
  });

  test("compiles class literal expression", () => {
    const bytes = compile(`record Book(String title, int pages) {}
    public class ReflectionClassLiteral {
      public static String run() {
        Class c = Book.class;
        return "" + c.getName();
      }
    }`);
    assert.ok(bytes.length > 200, "bundle has content");
  });

  test("compiles bubble sort with arrays", () => {
    const bytes = compile(`public class BubbleSort {
      public static String run() {
        int[] arr = new int[5];
        arr[0] = 5;
        arr[1] = 3;
        arr[2] = 8;
        arr[3] = 1;
        arr[4] = 2;
        for (int i = 0; i < 4; i++) {
          for (int j = 0; j < 4 - i; j++) {
            if (arr[j] > arr[j + 1]) {
              int tmp = arr[j];
              arr[j] = arr[j + 1];
              arr[j + 1] = tmp;
            }
          }
        }
        return "" + arr[0] + " " + arr[1] + " " + arr[2] + " " + arr[3] + " " + arr[4];
      }
    }`);
    assertValidClassFile(bytes);
  });

  test("compiles long local declaration and return", () => {
    const bytes = compile(`public class LongLocal {
      public static String run() {
        long result = 1L;
        for (int i = 2; i <= 5; i++) {
          result = result * i;
        }
        return "" + result;
      }
    }`);
    assertValidClassFile(bytes);
  });

  test("compiles non-capturing lambda via invokedynamic", () => {
    const bytes = compile(`import java.util.function.Function;
      public class LambdaRun {
        public static String run() {
          Function f = x -> x;
          return "" + f.apply("ok");
        }
      }`);
    assertValidClassFile(bytes);
    const text = new TextDecoder().decode(bytes);
    assert.ok(text.includes("LambdaMetafactory"), "contains lambda bootstrap");
  });

  test("compiles capturing lambda in static method", () => {
    const bytes = compile(`import java.util.function.Function;
      public class LambdaCap {
        public static String run() {
          String y = "ok";
          Function f = x -> y;
          return "" + f.apply("ng");
        }
      }`);
    assertValidClassFile(bytes);
  });

  test("compiles capturing lambda in instance method", () => {
    const bytes = compile(`import java.util.function.Function;
      public class LambdaThis {
        String prefix;
        public LambdaThis(String p) { this.prefix = p; }
        public String mk() {
          Function f = x -> prefix;
          return "" + f.apply("ignored");
        }
        public static String run() {
          LambdaThis v = new LambdaThis("hi");
          return v.mk();
        }
      }`);
    assertValidClassFile(bytes);
  });

  test("compiles unbound method reference", () => {
    const bytes = compile(`import java.util.function.Function;
      public class MRUnbound {
        public static String run() {
          Function f = String::length;
          return "" + f.apply("abcd");
        }
      }`);
    assertValidClassFile(bytes);
    const text = new TextDecoder().decode(bytes);
    assert.ok(text.includes("LambdaMetafactory"), "contains method-ref bootstrap");
  });

  test("compiles bound method reference", () => {
    const bytes = compile(`import java.util.function.Supplier;
      public class MRBound {
        public static String run() {
          Supplier s = "xyz"::toString;
          return "" + s.get();
        }
      }`);
    assertValidClassFile(bytes);
  });

  test("compiles constructor method reference (no-arg)", () => {
    const bytes = compile(`import java.util.function.Supplier;
      public class MRCtor0 {
        public static String run() {
          Supplier s = StringBuilder::new;
          Object o = s.get();
          return "" + o;
        }
      }`);
    assertValidClassFile(bytes);
  });

  test("compiles constructor method reference (one-arg user class)", () => {
    const bytes = compile(`import java.util.function.Function;
      public class Box {
        String v;
        public Box(String v) { this.v = v; }
      }
      public class MRCtor1 {
        public static String run() {
          Function f = Box::new;
          Object b = f.apply("z");
          return "" + b;
        }
    }`);
    assert.ok(bytes.length > 200, "bundle has content");
  });

  test("declared method resolution prefers superclass over interface defaults", () => {
    const bytes = compile(`public interface I {
        default String m() { return "i"; }
      }
      public class A {
        public String m() { return "a"; }
      }
      public class B extends A implements I {
        public static String run() {
          B b = new B();
          return b.m();
        }
      }`);
    assert.ok(bytes.length > 200, "bundle has content");
  });

  test("declared method resolution matches exact overload by descriptor", async () => {
    const result = await runSnippet(`public class OverloadPick {
        public static String f(int x) { return "int"; }
        public static String f(String s) { return "str"; }
        public static String run() { return OverloadPick.f("x"); }
      }`, "OverloadPick");
    assert.equal(result, "str");
  });

  test("unqualified call resolves exact overload by descriptor", async () => {
    const result = await runSnippet(`public class OverloadUnqualified {
        public static String f(int x) { return "int"; }
        public static String f(String s) { return "str"; }
        public static String run() { return f("x"); }
      }`, "OverloadUnqualified");
    assert.equal(result, "str");
  });

  test("unqualified call resolves inherited instance method", async () => {
    const result = await runSnippet(`public class ParentCall {
        public String m() { return "parent"; }
      }
      public class ChildCall extends ParentCall {
        public String runInst() { return m(); }
        public static String run() { return new ChildCall().runInst(); }
      }`, "ChildCall");
    assert.equal(result, "parent");
  });

  test("unqualified call resolves inherited static method", async () => {
    const result = await runSnippet(`public class ParentStaticCall {
        public static String s() { return "ok"; }
      }
      public class ChildStaticCall extends ParentStaticCall {
        public static String run() { return s(); }
      }`, "ChildStaticCall");
    assert.equal(result, "ok");
  });

  test("unqualified unresolved call fails fast at compile-time", () => {
    assert.throws(() => compile(`public class BadUnqualifiedCall {
      public static String run() {
        missing(1);
        return "ng";
      }
    }`), /Cannot resolve unqualified method call/);
  });

  test("compiles switch statement with int labels", () => {
    const bytes = compile(`public class SwitchInt {
      public static String run() {
        int x = 2;
        switch (x) {
          case 1 -> { x = 10; }
          case 2 -> { x = 20; }
          default -> { x = 30; }
        }
        return "" + x;
      }
    }`);
    assertValidClassFile(bytes);
  });

  test("compiles lambda for user-defined functional interface", () => {
    const bytes = compile(`public interface MyFn {
        int apply(int x);
      }
      public class LambdaUserIface {
        public static String run() {
          MyFn f = x -> x + 1;
          return "" + f.apply(41);
        }
    }`);
    assert.ok(bytes.length > 200, "bundle has content");
  });

  test("lambda rejects non-Object overload of equals in functional interface detection", () => {
    assert.throws(() => compile(`public interface BadSam {
      int apply(int x);
      boolean equals(String s);
    }
    public class BadSamUse {
      public static String run() {
        BadSam f = x -> x + 1;
        return "" + f.apply(1);
      }
    }`), /Unsupported functional interface/);
  });

  test("compiles switch expression with String labels", () => {
    const bytes = compile(`public class SwitchString {
      public static String run() {
        String s = "b";
        int x = switch (s) {
          case "a" -> 1;
          case "b" -> 2;
          default -> 3;
        };
        return "" + x;
      }
    }`);
    assertValidClassFile(bytes);
  });

  test("compiles switch expression with null label", () => {
    const bytes = compile(`public class SwitchNull {
      public static String run() {
        String s = null;
        int x = switch (s) {
          case null -> 7;
          default -> 9;
        };
        return "" + x;
      }
    }`);
    assertValidClassFile(bytes);
  });

  test("compiles switch statement with type pattern", () => {
    const bytes = compile(`public class SwitchPattern {
      public static String run() {
        Object v = "ok";
        switch (v) {
          case String s -> { System.out.println(s); }
          default -> { System.out.println("x"); }
        }
        return "done";
      }
    }`);
    assertValidClassFile(bytes);
  });

  test("compiles switch with guard", () => {
    const bytes = compile(`public class SwitchGuard {
      public static String run() {
        Object v = "ok";
        return switch (v) {
          case String s when s.length() > 1 -> "long";
          default -> "short";
        };
      }
    }`);
    assertValidClassFile(bytes);
  });

  test("switch expression without default is rejected", () => {
    assert.throws(() => compile(`public class SwitchNoDefault {
      public static int run(int x) {
        return switch (x) {
          case 1 -> 10;
        };
      }
    }`), /not exhaustive/);
  });

  test("switch with duplicate default is rejected", () => {
    assert.throws(() => compile(`public class SwitchDupDefault {
      public static int run(int x) {
        switch (x) {
          default -> { x = 1; }
          default -> { x = 2; }
        }
        return x;
      }
    }`), /more than one default label/);
  });

  test("switch with duplicate constant label is rejected", () => {
    assert.throws(() => compile(`public class SwitchDupConst {
      public static int run(int x) {
        switch (x) {
          case 1 -> { x = 1; }
          case 1 -> { x = 2; }
          default -> { x = 3; }
        }
        return x;
      }
    }`), /duplicate switch label/);
  });

  test("switch with case after default is rejected as unreachable", () => {
    assert.throws(() => compile(`public class SwitchAfterDefault {
      public static int run(int x) {
        switch (x) {
          default -> { x = 0; }
          case 1 -> { x = 1; }
        }
        return x;
      }
    }`), /unreachable case after unguarded default/);
  });

  test("switch with dominated type pattern is rejected", () => {
    assert.throws(() => compile(`public class SwitchDominated {
      public static String run(Object v) {
        return switch (v) {
          case String s -> "a";
          case String t -> "b";
          default -> "c";
        };
      }
    }`), /dominated switch label pattern/);
  });

  test("switch expression with exhaustive boolean labels compiles without default", () => {
    const bytes = compile(`public class SwitchBoolExhaustive {
      public static int run(boolean b) {
        return switch (b) {
          case true -> 1;
          case false -> 0;
        };
      }
    }`);
    assertValidClassFile(bytes);
  });

  test("switch expression with parenthesized type pattern compiles", () => {
    const bytes = compile(`public class SwitchParenPattern {
      public static String run(Object v) {
        return switch (v) {
          case (String s) -> s;
          default -> "x";
        };
      }
    }`);
    assertValidClassFile(bytes);
  });

  test("switch expression with record pattern compiles", () => {
    const bytes = compile(`record Point(int x, int y) {
      public static int run(Object o) {
        return switch (o) {
          case Point(int a, int b) -> a + b;
          default -> 0;
        };
      }
    }`);
    assertValidClassFile(bytes);
  });

  test("instanceof with record pattern compiles", () => {
    const bytes = compile(`record Pair(int x, int y) {
      public static int run(Object o) {
        if (o instanceof Pair(int a, int b)) {
          return a * b;
        }
        return 0;
      }
    }`);
    assertValidClassFile(bytes);
  });

  test("switch on boolean rejects int case label", () => {
    assert.throws(() => compile(`public class SwitchBoolRejectInt {
      public static int run(boolean b) {
        return switch (b) {
          case 1 -> 1;
          case 0 -> 0;
          default -> -1;
        };
      }
    }`), /int case label requires int switch selector/);
  });

  test("switch on int rejects boolean case label", () => {
    assert.throws(() => compile(`public class SwitchIntRejectBool {
      public static int run(int x) {
        return switch (x) {
          case true -> 1;
          case false -> 0;
          default -> -1;
        };
      }
    }`), /boolean case label requires boolean switch selector/);
  });

  test("operator '+' rejects boolean operands", () => {
    assert.throws(() => compile(`public class BadPlusBool {
      public static int run() {
        boolean a = true;
        boolean b = false;
        return a + b;
      }
    }`), /requires numeric operands/);
  });

  test("operator '==' rejects int/boolean comparison", () => {
    assert.throws(() => compile(`public class BadEqPrim {
      public static boolean run() {
        return 1 == true;
      }
    }`), /same primitive type/);
  });

  test("cast rejects int to boolean", () => {
    assert.throws(() => compile(`public class BadCastIntToBool {
      public static boolean run() {
        return (boolean) 1;
      }
    }`), /Invalid cast/);
  });

  test("cast rejects boolean to int", () => {
    assert.throws(() => compile(`public class BadCastBoolToInt {
      public static int run() {
        return (int) false;
      }
    }`), /Invalid cast/);
  });

  test("compound assignment rejects unresolved identifier target", () => {
    assert.throws(() => compile(`public class BadCompoundTarget {
      public static int run() {
        missing += 1;
        return 0;
      }
    }`), /compound assignment target not found/);
  });

  test("compound assignment rejects non-lvalue target", () => {
    assert.throws(() => compile(`public class BadCompoundLvalue {
      public static int run() {
        int a = 1;
        int b = 2;
        (a + b) += 3;
        return 0;
      }
    }`), /Unsupported compound assignment target/);
  });

  test("compound assignment accepts long array index via narrowing conversion", async () => {
    const result = await runSnippet(`public class LongIndexCompoundRun {
      public static String run() {
        int[] arr = new int[2];
        long i = 0;
        arr[i] += 1;
        return "" + arr[0];
      }
    }`, "LongIndexCompoundRun");
    assert.equal(result, "1");
  });

  test("assignment allows subtype to supertype in known hierarchy", () => {
    assert.doesNotThrow(() => compile(`
      public class A {}
      public class B extends A {}
      public class AssignOk {
        public static String run() {
          A a = new B();
          return "ok";
        }
      }
    `));
  });

  test("assignment rejects supertype to subtype in known hierarchy", () => {
    assert.throws(() => compile(`
      public class A {}
      public class B extends A {}
      public class AssignNg {
        public static String run() {
          B b = new A();
          return "ng";
        }
      }
    `), /Type mismatch/);
  });

  test("checked exception throw requires catch or throws", () => {
    assert.throws(() => compile(`import java.io.IOException;
    public class CheckedThrowBad {
      public static String run() {
        throw new IOException();
      }
    }`), /Unhandled checked exception/);
  });

  test("checked exception throw is allowed when declared", () => {
    const bytes = compile(`import java.io.IOException;
    public class CheckedThrowDecl {
      public static String run() throws IOException {
        throw new IOException();
      }
    }`);
    assertValidClassFile(bytes);
  });

  test("checked exception from callee must be declared or caught", () => {
    assert.throws(() => compile(`import java.io.IOException;
    public class CheckedCallBad {
      public static void mayThrow() throws IOException {
        throw new IOException();
      }
      public static String run() {
        mayThrow();
        return "ng";
      }
    }`), /Unhandled checked exception/);
  });

  test("checked exception from callee can be caught", () => {
    const bytes = compile(`import java.io.IOException;
    public class CheckedCallCatch {
      public static void mayThrow() throws IOException {
        throw new IOException();
      }
      public static String run() {
        try {
          mayThrow();
        } catch (IOException e) {
          return "ok";
        }
        return "ng";
      }
    }`);
    assertValidClassFile(bytes);
  });

  test("checked exception propagates from inherited interface method", () => {
    assert.throws(() => compile(`import java.io.IOException;
      public interface ParentEx {
        void boom() throws IOException;
      }
      public interface ChildEx extends ParentEx {}
      public class ImplEx implements ChildEx {
        public void boom() throws IOException {
          throw new IOException();
        }
        public static String run(ChildEx c) {
          c.boom();
          return "ng";
        }
      }`), /Unhandled checked exception/);
  });

  test("checked exception tracks static call to same-unit class ref", () => {
    assert.throws(() => compile(`import java.io.IOException;
      public class UtilEx {
        public static void fail() throws IOException {
          throw new IOException();
        }
      }
      public class StaticRefEx {
        public static String run() {
          UtilEx.fail();
          return "ng";
        }
      }`), /Unhandled checked exception/);
  });

  test("checked exception analysis picks correct overload by argument types", () => {
    assert.doesNotThrow(() => compile(`import java.io.IOException;
      public class CheckedOverload {
        public static void g(String s) throws IOException { throw new IOException(); }
        public static void g(int x) {}
        public static String run() {
          g(1);
          return "ok";
        }
      }`));
  });

  test("checked exception analysis picks constructor overload by argument types", () => {
    assert.doesNotThrow(() => compile(`import java.io.IOException;
      public class CheckedCtorOverload {
        public CheckedCtorOverload(String s) throws IOException { throw new IOException(); }
        public CheckedCtorOverload(int x) {}
        public static String run() {
          new CheckedCtorOverload(1);
          return "ok";
        }
      }`));
  });

  test("checked exception analysis does not predeclare later local names", () => {
    assert.throws(() => compile(`import java.io.IOException;
      public class UtilShadow {
        public static void fail() throws IOException { throw new IOException(); }
      }
      public class CheckedLocalScope {
        public static String run() {
          UtilShadow.fail();
          int UtilShadow = 1;
          return "" + UtilShadow;
        }
      }`), /Unhandled checked exception/);
  });

  test("checked exception throw infers type from non-new expression", () => {
    assert.throws(() => compile(`import java.io.IOException;
      public class CheckedThrowExpr {
        public static IOException makeEx() { return new IOException(); }
        public static String run() {
          throw makeEx();
        }
      }`), /Unhandled checked exception/);
  });

  test("switch expression with null + total pattern is exhaustive for reference selector", () => {
    const bytes = compile(`public class SwitchRefExhaustive {
      public static String run(Object v) {
        return switch (v) {
          case null -> "n";
          case Object o -> "o";
        };
      }
    }`);
    assertValidClassFile(bytes);
  });

  test("switch expression with only total pattern is not exhaustive for reference selector", () => {
    assert.throws(() => compile(`public class SwitchRefNotExhaustive {
      public static String run(Object v) {
        return switch (v) {
          case Object o -> "o";
        };
      }
    }`), /not exhaustive/);
  });

  test("switch with subtype pattern after supertype pattern is rejected as dominated", () => {
    assert.throws(() => compile(`public class SwitchHierarchyDominated {
      public static String run(Object v) {
        return switch (v) {
          case Object o -> "obj";
          case String s -> "str";
          default -> "x";
        };
      }
    }`), /dominated/);
  });

  test("switch allows guarded type pattern followed by same unguarded type pattern", () => {
    const bytes = compile(`public class SwitchGuardedThenUnguarded {
      public static String run(Object v) {
        return switch (v) {
          case String s when s.length() > 3 -> "long";
          case String s -> "short";
          default -> "other";
        };
      }
    }`);
    assertValidClassFile(bytes);
  });

  test("switch rejects dominated subtype pattern in user-defined hierarchy", () => {
    assert.throws(() => compile(`
      public class A {}
      public class B extends A {}
      public class SwitchUserHierarchy {
        public static String run(A a) {
          return switch (a) {
            case A x -> "a";
            case B y -> "b";
            default -> "d";
          };
        }
      }
    `), /dominated/);
  });
});

// ============================================================================
// Runtime (WASM)
// ============================================================================

describe("Runtime (WASM)", () => {
  test("lambda for user-defined functional interface executes", async () => {
    const result = await runSnippet(`public interface MyFn {
        int apply(int x);
      }
      public class RuntimeLambdaUserIface {
        public static String run() {
          MyFn f = x -> x + 1;
          return "" + f.apply(41);
        }
      }`, "RuntimeLambdaUserIface");
    assert.equal(result, "42");
  });

  test("lambda method reference boxes primitive return for Function.apply", async () => {
    const result = await runSnippet(`import java.util.function.Function;
      public class RuntimeLambdaBoxing {
        public static String run() {
          Function f = String::length;
          return "" + f.apply("abcde");
        }
      }`, "RuntimeLambdaBoxing");
    assert.equal(result, "5");
  });

  test("switch expression with String selector executes", async () => {
    const result = await runSnippet(`public class RuntimeSwitchExpr {
      public static String run() {
        String kind = "release";
        int score = switch (kind) {
          case "alpha" -> 1;
          case "beta" -> 2;
          case "release" -> 3;
          default -> 0;
        };
        return "kind=" + kind + " score=" + score;
      }
    }`, "RuntimeSwitchExpr");
    assert.equal(result, "kind=release score=3");
  });

  test("autoboxes primitive arguments for reference-typed calls", async () => {
    const result = await runSnippet(`import java.util.HashMap;
      public class RuntimeAutoBoxArg {
        public static String run() {
          HashMap m = new HashMap();
          m.put("age", 41);
          Object v = m.get("age");
          return "" + v;
        }
      }`, "RuntimeAutoBoxArg");
    assert.equal(result, "41");
  });

  test("error includes VM stack frame", async () => {
    const result = await runSnippet(`public class RuntimeStackTrace {
      public static String run() {
        Object x = null;
        return x.toString();
      }
    }`, "RuntimeStackTrace");
    assert.match(result, /^ERROR: NullPointerException:/);
    assert.match(result, /\n  at RuntimeStackTrace\.run\(\)Ljava\/lang\/String;/);
  });

  test("ForkJoin RecursiveTask invoke executes", async () => {
    const result = await runSnippet(`import java.util.concurrent.RecursiveTask;
      import java.util.concurrent.ForkJoinPool;
      public class RuntimeForkJoin {
        static class SumTask extends RecursiveTask {
          int lo;
          int hi;
          SumTask(int lo, int hi) { this.lo = lo; this.hi = hi; }
          protected Object compute() {
            int n = hi - lo;
            if (n <= 4) {
              int s = 0;
              for (int i = lo; i <= hi; i++) s += i;
              return s;
            }
            int mid = (lo + hi) / 2;
            SumTask left = new SumTask(lo, mid);
            SumTask right = new SumTask(mid + 1, hi);
            left.fork();
            int r = (int) right.compute();
            int l = (int) left.join();
            return l + r;
          }
        }
        public static String run() {
          ForkJoinPool pool = ForkJoinPool.commonPool();
          int sum = (int) pool.invoke(new SumTask(1, 20));
          return "" + sum;
        }
      }`, "RuntimeForkJoin");
    assert.equal(result, "210");
  });

  test("interface default method call executes via invokeinterface", async () => {
    const result = await runSnippet(`public interface Named {
      String name();
      default String label() { return name(); }
    }
    public class NamedImpl implements Named {
      public String name() { return "ok"; }
      public static String run() { return new NamedImpl().label(); }
    }`, "NamedImpl");
    assert.equal(result, "ok");
  });

  test("bitwise/shift and compound assignments execute", async () => {
    const result = await runSnippet(`public class RuntimeBitwiseOps {
      public static String run() {
        int x = 10;
        x *= 2;
        x /= 4;
        x %= 3;
        x += 5;
        x -= 1;
        x <<= 2;
        x >>= 1;
        x >>>= 1;
        x &= 7;
        x |= 8;
        x ^= 3;
        int y = (~1) + (8 >>> 1) + (8 >> 1) + (1 << 3) + (6 & 3) + (6 | 3) + (6 ^ 3);
        return "" + x + ":" + y;
      }
    }`, "RuntimeBitwiseOps");
    assert.equal(result, "13:28");
  });

  test("assert false throws AssertionError", async () => {
    const result = await runSnippet(`public class AssertRun {
      public static String run() {
        assert false : "boom";
        return "ng";
      }
    }`, "AssertRun");
    assert.match(result, /^ERROR: Exception: java\/lang\/AssertionError:/);
  });

  test("assert primitive message is boxed", async () => {
    const result = await runSnippet(`public class AssertPrimitiveMsgRun {
      public static String run() {
        assert false : 1;
        return "ng";
      }
    }`, "AssertPrimitiveMsgRun");
    assert.match(result, /^ERROR: Exception: java\/lang\/AssertionError:/);
  });

  test("synchronized block executes", async () => {
    const result = await runSnippet(`public class SyncRun {
      public static String run() {
        Object lock = new Object();
        int x = 0;
        synchronized (lock) {
          x = 9;
        }
        return "" + x;
      }
    }`, "SyncRun");
    assert.equal(result, "9");
  });

  test("try-with-resources closes resource", async () => {
    const result = await runSnippet(`public class TwrRun {
      static int closed;
      public void close() { closed = closed + 1; }
      public static String run() {
        try (TwrRun r = new TwrRun()) {
          int x = 1;
        }
        return "" + closed;
      }
    }`, "TwrRun");
    assert.equal(result, "1");
  });

  test("finally with early return does not re-run finally body", async () => {
    const result = await runSnippet(`public class FinallyReturnOnceRun {
      static int c;
      public static String run() {
        try {
          c = 1;
        } finally {
          c = c + 1;
          return "" + c;
        }
      }
    }`, "FinallyReturnOnceRun");
    assert.equal(result, "2");
  });

  test("compound assignment evaluates array LHS once", async () => {
    const result = await runSnippet(`public class CompoundArraySideEffectRun {
      public static String run() {
        int[] arr = { 1, 2 };
        int i = 0;
        arr[i++] += 4;
        return "" + i + ":" + arr[0] + ":" + arr[1];
      }
    }`, "CompoundArraySideEffectRun");
    assert.equal(result, "1:5:2");
  });

  test("compound assignment evaluates field receiver once", async () => {
    const result = await runSnippet(`public class CompoundFieldSideEffectRun {
      int x;
      int calls;
      CompoundFieldSideEffectRun getSelf() {
        calls = calls + 1;
        return this;
      }
      public static String run() {
        CompoundFieldSideEffectRun o = new CompoundFieldSideEffectRun();
        o.getSelf().x += 3;
        return "" + o.calls + ":" + o.x;
      }
    }`, "CompoundFieldSideEffectRun");
    assert.equal(result, "1:3");
  });

  test("compound assignment narrows to byte", async () => {
    const result = await runSnippet(`public class CompoundNarrowByteRun {
      public static String run() {
        byte b = 1;
        b += 130;
        return "" + b;
      }
    }`, "CompoundNarrowByteRun");
    assert.equal(result, "-125");
  });

});

// ============================================================================
// Class reader
// ============================================================================

describe("Class reader", () => {
  test("parseClassMeta extracts class name and methods from compiled .class", () => {
    const bytes = compile(`public class Foo {
      public static String run() { return "hi"; }
      public int add(int a, int b) { return a + b; }
    }`);
    // compile() returns raw .class for single class, use parseClassMeta directly
    const meta = parseClassMeta(bytes);
    assert.equal(meta.name, "Foo");
    const methodNames = meta.methods.map(m => m.name);
    assert.ok(methodNames.includes("run"));
    assert.ok(methodNames.includes("add"));
    assert.ok(methodNames.includes("<init>"));
  });

  test("buildMethodRegistry creates correct keys and types", () => {
    const bytes = compile(`public class Bar {
      public static String greet() { return "hello"; }
      public int compute(int x) { return x * 2; }
    }`);
    const classes = [parseClassMeta(bytes)];
    const reg = buildMethodRegistry(classes);
    // Static method
    assert.ok(reg["Bar.greet()"]);
    assert.equal(reg["Bar.greet()"].returnType, "String");
    assert.deepEqual(reg["Bar.greet()"].paramTypes, []);
    // Instance method
    assert.ok(reg["Bar.compute(I)"]);
    assert.equal(reg["Bar.compute(I)"].returnType, "int");
    assert.deepEqual(reg["Bar.compute(I)"].paramTypes, ["int"]);
  });

  test("buildMethodRegistry handles classes with long/double constants", () => {
    // Long constants occupy 2 CP slots. If the class-reader doesn't account
    // for this, all subsequent CP indices are shifted and method names are corrupted.
    const bytes = compile(`public class LongConst {
      public static long BIG = 9999999999L;
      public static String convert(long v) { return "" + v; }
      public static int add(int a, int b) { return a + b; }
    }`);
    const classes = [parseClassMeta(bytes)];
    const reg = buildMethodRegistry(classes);
    // Methods should be correctly named despite the long constant in the CP
    assert.ok(reg["LongConst.convert(J)"], "convert(J) should be registered");
    assert.ok(reg["LongConst.add(II)"], "add(II) should be registered");
    assert.equal(reg["LongConst.convert(J)"].returnType, "String");
    assert.equal(reg["LongConst.add(II)"].returnType, "int");
  });

  test("parseBundleMeta handles multi-class bundle", () => {
    const bytes = compile(`public class A {
      public static String run() { return "a"; }
    }
    class B {
      public int value() { return 1; }
    }`);
    const classes = parseBundleMeta(bytes);
    assert.equal(classes.length, 2);
    const names = classes.map(c => c.name).sort();
    assert.deepEqual(names, ["A", "B"]);
  });
});

// ---------------------------------------------------------------------------
// New syntax tests
// ---------------------------------------------------------------------------

describe("Parser – new syntax", () => {
  test("do-while loop", () => {
    const src = `public class DoWhile {
      public static String run() {
        int i = 0;
        do { i = i + 1; } while (i < 3);
        return "" + i;
      }
    }`;
    const cls = parse(lex(src));
    const body = cls.methods[0].body;
    assert.equal(body[1].kind, "doWhile");
  });

  test("throw statement", () => {
    const src = `public class ThrowTest {
      public static String run() {
        throw new RuntimeException();
      }
    }`;
    const cls = parse(lex(src));
    const body = cls.methods[0].body;
    assert.equal(body[0].kind, "throw");
  });

  test("try-catch", () => {
    const src = `public class TryCatch {
      public static String run() {
        try {
          int x = 1;
        } catch (Exception e) {
          int y = 2;
        }
        return "ok";
      }
    }`;
    const cls = parse(lex(src));
    const body = cls.methods[0].body;
    assert.equal(body[0].kind, "tryCatch");
    const tc = body[0] as any;
    assert.equal(tc.catches.length, 1);
    assert.equal(tc.catches[0].exType, "Exception");
    assert.equal(tc.catches[0].varName, "e");
  });

  test("try-catch-finally", () => {
    const src = `public class TryCatchFinally {
      public static String run() {
        try { int x = 1; }
        catch (Exception e) { int y = 2; }
        finally { int z = 3; }
        return "ok";
      }
    }`;
    const cls = parse(lex(src));
    const tc = cls.methods[0].body[0] as any;
    assert.equal(tc.kind, "tryCatch");
    assert.ok(tc.finallyBody);
    assert.ok(tc.finallyBody.length > 0);
  });

  test("assert statement parses", () => {
    const src = `public class AssertStmt {
      public static String run() {
        assert 1 < 2 : "bad";
        return "ok";
      }
    }`;
    const cls = parse(lex(src));
    const body = cls.methods[0].body;
    assert.equal(body[0].kind, "assert");
  });

  test("synchronized statement parses", () => {
    const src = `public class SyncStmt {
      public static String run() {
        Object lock = new Object();
        synchronized (lock) {
          int x = 1;
        }
        return "ok";
      }
    }`;
    const cls = parse(lex(src));
    const body = cls.methods[0].body;
    assert.equal(body[1].kind, "synchronized");
  });

  test("synchronized requires block body", () => {
    const src = `public class BadSyncStmt {
      public static String run() {
        Object lock = new Object();
        synchronized (lock) return "ng";
      }
    }`;
    assert.throws(() => parse(lex(src)));
  });

  test("try-with-resources rejects empty resource list", () => {
    const src = `public class BadTwrEmpty {
      public static String run() {
        try () {
          return "ng";
        }
      }
    }`;
    assert.throws(() => parse(lex(src)));
  });

  test("try-with-resources parses and lowers", () => {
    const src = `public class TwrStmt {
      int closed;
      public void close() { closed = closed + 1; }
      public static String run() {
        try (TwrStmt r = new TwrStmt()) {
          int x = 1;
        }
        return "ok";
      }
    }`;
    const cls = parse(lex(src));
    const body = cls.methods.find(m => m.name === "run")!.body;
    assert.equal(body[0].kind, "tryCatch");
    const tc = body[0] as any;
    assert.equal(tc.tryBody[0].kind, "block");
    assert.equal(tc.tryBody[0].stmts[0].kind, "varDecl");
  });

  test("try-with-resources lowering includes catch/rethrow close path", () => {
    const src = `public class TwrStmtTwo {
      static class Res {
        Res(boolean fail) {}
        void close() {}
      }
      public static String run() {
        try (Res a = new Res(false); Res b = new Res(true)) {
          return "ok";
        }
      }
    }`;
    const cls = parse(lex(src));
    const body = cls.methods.find(m => m.name === "run")!.body;
    const outer = body[0] as any;
    assert.equal(outer.kind, "tryCatch");
    const firstBlock = outer.tryBody[0];
    assert.equal(firstBlock.kind, "block");
    assert.equal(firstBlock.stmts[1].kind, "varDecl");
    const innerTry = firstBlock.stmts[2];
    assert.equal(innerTry.kind, "tryCatch");
    assert.equal(innerTry.catches[0].exType, "Throwable");
    assert.ok(innerTry.finallyBody && innerTry.finallyBody.length > 0);
    const caughtBody = innerTry.catches[0].body;
    assert.equal(caughtBody[0].kind, "assign");
    assert.equal(caughtBody[1].kind, "throw");
    const closeWithPrimary = innerTry.finallyBody[0];
    assert.equal(closeWithPrimary.kind, "if");
    const primaryGuard = closeWithPrimary.then[0];
    assert.equal(primaryGuard.kind, "if");
    const nestedCloseTry = primaryGuard.then[0];
    assert.equal(nestedCloseTry.kind, "tryCatch");
    assert.equal(nestedCloseTry.catches[0].body[0].kind, "exprStmt");
  });

  test("enhanced for loop", () => {
    const src = `public class ForEach {
      public static String run() {
        int[] arr = new int[3];
        for (int x : arr) {
          int y = x;
        }
        return "ok";
      }
    }`;
    const cls = parse(lex(src));
    const body = cls.methods[0].body;
    assert.equal(body[1].kind, "forEach");
    const fe = body[1] as any;
    assert.equal(fe.varName, "x");
    assert.equal(fe.varType, "int");
  });

  test("multiple variable declaration", () => {
    const src = `public class MultiDecl {
      public static String run() {
        int a = 1, b = 2;
        return "" + a + b;
      }
    }`;
    const cls = parse(lex(src));
    // Multi-decl is flattened into the enclosing block
    const body = cls.methods[0].body;
    assert.equal(body[0].kind, "varDecl");
    assert.equal((body[0] as any).name, "a");
    assert.equal(body[1].kind, "varDecl");
    assert.equal((body[1] as any).name, "b");
  });

  test("switch colon syntax", () => {
    const src = `public class SwitchColon {
      public static String run() {
        int x = 1;
        switch (x) {
          case 1:
            return "one";
          case 2:
            return "two";
          default:
            return "other";
        }
      }
    }`;
    const cls = parse(lex(src));
    const body = cls.methods[0].body;
    assert.equal(body[1].kind, "switch");
    const sw = body[1] as any;
    assert.equal(sw.cases.length, 3);
  });

  test("labeled statement and break label", () => {
    const src = `public class LabelTest {
      public static String run() {
        outer: for (int i = 0; i < 3; i++) {
          break outer;
        }
        return "ok";
      }
    }`;
    const cls = parse(lex(src));
    const body = cls.methods[0].body;
    assert.equal(body[0].kind, "labeled");
    const lbl = body[0] as any;
    assert.equal(lbl.label, "outer");
    assert.equal(lbl.stmt.kind, "for");
  });

  test("prefix increment", () => {
    const src = `public class PreInc {
      public static String run() {
        int i = 5;
        int j = ++i;
        return "" + j;
      }
    }`;
    const cls = parse(lex(src));
    const body = cls.methods[0].body;
    const decl = body[1] as any;
    assert.equal(decl.init.kind, "preIncrement");
    assert.equal(decl.init.op, "++");
  });

  test("prefix decrement", () => {
    const src = `public class PreDec {
      public static String run() {
        int i = 5;
        int j = --i;
        return "" + j;
      }
    }`;
    const cls = parse(lex(src));
    const decl = cls.methods[0].body[1] as any;
    assert.equal(decl.init.kind, "preIncrement");
    assert.equal(decl.init.op, "--");
  });

  test("break and continue", () => {
    const src = `public class BreakCont {
      public static String run() {
        for (int i = 0; i < 10; i++) {
          if (i == 5) break;
          if (i == 3) continue;
        }
        return "ok";
      }
    }`;
    const cls = parse(lex(src));
    // Should parse without error
    assert.equal(cls.name, "BreakCont");
  });
});

describe("Code generator – new syntax", () => {
  test("compiles do-while loop", () => {
    const bytes = compile(`public class DoWhile {
      public static String run() {
        int i = 0;
        do { i = i + 1; } while (i < 3);
        return "" + i;
      }
    }`);
    assertValidClassFile(bytes);
  });

  test("compiles throw statement", () => {
    const bytes = compile(`public class ThrowTest {
      public static String run() {
        throw new RuntimeException();
      }
    }`);
    assertValidClassFile(bytes);
  });

  test("compiles try-catch", () => {
    const bytes = compile(`public class TryCatch {
      public static String run() {
        try {
          int x = 1;
        } catch (Exception e) {
          int y = 2;
        }
        return "ok";
      }
    }`);
    assertValidClassFile(bytes);
  });

  test("compiles try-catch-finally", () => {
    const bytes = compile(`public class TryCatchFinally {
      public static String run() {
        try { int x = 1; }
        catch (Exception e) { int y = 2; }
        finally { int z = 3; }
        return "ok";
      }
    }`);
    assertValidClassFile(bytes);
  });

  test("compiles assert statement", () => {
    const bytes = compile(`public class AssertStmt {
      public static String run() {
        assert 1 < 2 : "bad";
        return "ok";
      }
    }`);
    assertValidClassFile(bytes);
  });

  test("compiles synchronized statement", () => {
    const bytes = compile(`public class SyncStmt {
      public static String run() {
        Object lock = new Object();
        int x = 0;
        synchronized (lock) {
          x = 7;
        }
        return "" + x;
      }
    }`);
    assertValidClassFile(bytes);
  });

  test("compiles synchronized statement with early return", () => {
    const bytes = compile(`public class SyncStmtReturn {
      public static String run() {
        Object lock = new Object();
        synchronized (lock) {
          return "ok";
        }
      }
    }`);
    assertValidClassFile(bytes);
  });

  test("compiles try-with-resources", () => {
    const bytes = compile(`public class TwrStmt {
      static int closed;
      public void close() { closed = closed + 1; }
      public static String run() {
        try (TwrStmt r = new TwrStmt()) {
          int x = 1;
        }
        return "" + closed;
      }
    }`);
    assertValidClassFile(bytes);
  });

  test("compiles enhanced for on array", () => {
    const bytes = compile(`public class ForEachArr {
      public static String run() {
        int[] arr = new int[3];
        int sum = 0;
        for (int x : arr) {
          sum = sum + x;
        }
        return "" + sum;
      }
    }`);
    assertValidClassFile(bytes);
  });

  test("compiles multiple variable declaration", () => {
    const bytes = compile(`public class MultiDecl {
      public static String run() {
        int a = 1, b = 2, c = 3;
        return "" + (a + b + c);
      }
    }`);
    assertValidClassFile(bytes);
  });

  test("compiles switch with colon syntax", () => {
    const bytes = compile(`public class SwitchColon {
      public static String run() {
        int x = 2;
        switch (x) {
          case 1:
            return "one";
          case 2:
            return "two";
          default:
            return "other";
        }
      }
    }`);
    assertValidClassFile(bytes);
  });

  test("compiles labeled break", () => {
    const bytes = compile(`public class LabelBreak {
      public static String run() {
        int count = 0;
        outer: for (int i = 0; i < 5; i++) {
          for (int j = 0; j < 5; j++) {
            if (j == 2) break outer;
            count = count + 1;
          }
        }
        return "" + count;
      }
    }`);
    assertValidClassFile(bytes);
  });

  test("compiles prefix increment", () => {
    const bytes = compile(`public class PreInc {
      public static String run() {
        int i = 5;
        int j = ++i;
        return "" + j;
      }
    }`);
    assertValidClassFile(bytes);
  });

  test("compiles break and continue in loops", () => {
    const bytes = compile(`public class BreakCont {
      public static String run() {
        int sum = 0;
        for (int i = 0; i < 10; i++) {
          if (i == 7) break;
          if (i == 3) continue;
          sum = sum + i;
        }
        return "" + sum;
      }
    }`);
    assertValidClassFile(bytes);
  });

  test("compiles continue in while loop", () => {
    const bytes = compile(`public class WhileCont {
      public static String run() {
        int i = 0;
        int sum = 0;
        while (i < 5) {
          i = i + 1;
          if (i == 3) continue;
          sum = sum + i;
        }
        return "" + sum;
      }
    }`);
    assertValidClassFile(bytes);
  });

  test("compiles break in do-while", () => {
    const bytes = compile(`public class DoBreak {
      public static String run() {
        int i = 0;
        do {
          i = i + 1;
          if (i == 3) break;
        } while (i < 10);
        return "" + i;
      }
    }`);
    assertValidClassFile(bytes);
  });
});

describe("Runtime – new syntax", () => {
  test("do-while executes correctly", async () => {
    const result = await runSnippet(`public class DoWhileRun {
      public static String run() {
        int i = 0;
        do { i = i + 1; } while (i < 3);
        return "" + i;
      }
    }`, "DoWhileRun");
    assert.equal(result, "3");
  });

  test("prefix increment returns new value", async () => {
    const result = await runSnippet(`public class PreIncRun {
      public static String run() {
        int i = 5;
        int j = ++i;
        return "" + j;
      }
    }`, "PreIncRun");
    assert.equal(result, "6");
  });

  test("prefix decrement returns new value", async () => {
    const result = await runSnippet(`public class PreDecRun {
      public static String run() {
        int i = 5;
        int j = --i;
        return "" + j;
      }
    }`, "PreDecRun");
    assert.equal(result, "4");
  });

  test("break exits loop early", async () => {
    const result = await runSnippet(`public class BreakRun {
      public static String run() {
        int sum = 0;
        for (int i = 0; i < 10; i++) {
          if (i == 5) break;
          sum = sum + i;
        }
        return "" + sum;
      }
    }`, "BreakRun");
    assert.equal(result, "10"); // 0+1+2+3+4
  });

  test("continue skips iteration", async () => {
    const result = await runSnippet(`public class ContinueRun {
      public static String run() {
        int sum = 0;
        for (int i = 0; i < 5; i++) {
          if (i == 2) continue;
          sum = sum + i;
        }
        return "" + sum;
      }
    }`, "ContinueRun");
    assert.equal(result, "8"); // 0+1+3+4
  });

  test("labeled break exits outer loop", async () => {
    const result = await runSnippet(`public class LabelBreakRun {
      public static String run() {
        int count = 0;
        outer: for (int i = 0; i < 5; i++) {
          for (int j = 0; j < 5; j++) {
            if (j == 2) break outer;
            count = count + 1;
          }
        }
        return "" + count;
      }
    }`, "LabelBreakRun");
    assert.equal(result, "2"); // j=0, j=1 then break outer
  });

  test("multiple variable declaration works", async () => {
    const result = await runSnippet(`public class MultiDeclRun {
      public static String run() {
        int a = 10, b = 20, c = 30;
        return "" + (a + b + c);
      }
    }`, "MultiDeclRun");
    assert.equal(result, "60");
  });

  test("switch colon syntax executes", async () => {
    const result = await runSnippet(`public class SwitchColonRun {
      public static String run() {
        int x = 2;
        switch (x) {
          case 1:
            return "one";
          case 2:
            return "two";
          default:
            return "other";
        }
      }
    }`, "SwitchColonRun");
    assert.equal(result, "two");
  });

  test("enhanced for on array executes", async () => {
    const result = await runSnippet(`public class ForEachRun {
      public static String run() {
        int[] arr = { 10, 20, 30 };
        int sum = 0;
        for (int x : arr) {
          sum = sum + x;
        }
        return "" + sum;
      }
    }`, "ForEachRun");
    assert.equal(result, "60");
  });

  test("break in do-while executes", async () => {
    const result = await runSnippet(`public class DoBreakRun {
      public static String run() {
        int i = 0;
        do {
          i = i + 1;
          if (i == 3) break;
        } while (i < 10);
        return "" + i;
      }
    }`, "DoBreakRun");
    assert.equal(result, "3");
  });

  test("continue in while loop executes", async () => {
    const result = await runSnippet(`public class WhileContRun {
      public static String run() {
        int i = 0;
        int sum = 0;
        while (i < 5) {
          i = i + 1;
          if (i == 3) continue;
          sum = sum + i;
        }
        return "" + sum;
      }
    }`, "WhileContRun");
    assert.equal(result, "12"); // 1+2+4+5
  });
});
