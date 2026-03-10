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
import { lex, parseAll, compile, generateClassFile, TokenKind } from "./javac.js";

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
    const tokens = lex("public class static return");
    assert.equal(tokens[0].kind, TokenKind.KwPublic);
    assert.equal(tokens[1].kind, TokenKind.KwClass);
    assert.equal(tokens[2].kind, TokenKind.KwStatic);
    assert.equal(tokens[3].kind, TokenKind.KwReturn);
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

  test("operators", () => {
    const tokens = lex("== != <= >= && || ++ --");
    const kinds = tokens.slice(0, -1).map(t => t.kind);
    assert.deepEqual(kinds, [
      TokenKind.Eq, TokenKind.Ne, TokenKind.Le, TokenKind.Ge,
      TokenKind.And, TokenKind.Or, TokenKind.PlusPlus, TokenKind.MinusMinus,
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

  test("array type in parameters", () => {
    const src = `public class ArrParam {
      public static void sort(int[] arr) {}
    }`;
    const cls = parse(lex(src));
    const param = cls.methods[0].params[0];
    assert.deepEqual(param.type, { array: "int" });
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
});
