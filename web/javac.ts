// Re-export class-reader utilities for use from index.html
export { parseClassMeta, parseBundleMeta, buildMethodRegistry, readJar, classFilesToBundle } from "./class-reader.js";

// 199xVM — Java subset compiler (TypeScript)
//
// Compiles a minimal subset of Java to JVM .class file bytecode.
// Designed to produce class files compatible with the 199xVM interpreter.
//
// Supported:
//   - Class declaration (public class Foo { ... })
//   - Static and instance methods
//   - Fields (instance & static)
//   - Local variables (int, String, boolean)
//   - if / else / while / for
//   - return
//   - Arithmetic: + - * / %
//   - Comparisons: == != < > <= >=
//   - Logical: && || !
//   - String concatenation via +
//   - new ClassName(args)
//   - Method calls: obj.method(args), ClassName.staticMethod(args)
//   - Integer and String literals
//   - System.out.println(...)


import { lex } from "./javac/lexer.js";
import { parseAll } from "./javac/parser.js";
import type {
  ClassDecl,
  Expr,
  FieldDecl,
  MethodDecl,
  ParamDecl,
  Stmt,
  SwitchCase,
  SwitchLabel,
  Type,
} from "./javac/ast.js";

export { lex, TokenKind } from "./javac/lexer.js";
export type { Token } from "./javac/lexer.js";
export type {
  ClassDecl,
  Expr,
  FieldDecl,
  MethodDecl,
  ParamDecl,
  Stmt,
  SwitchCase,
  SwitchLabel,
  Type,
} from "./javac/ast.js";
export { parseAll } from "./javac/parser.js";
// ============================================================================
// Code Generator — produces JVM .class file bytes
// ============================================================================

// Constant pool builder
interface CpEntry {
  tag: number;
  data: number[];
}

class ConstantPoolBuilder {
  private entries: CpEntry[] = [{ tag: 0, data: [] }]; // index 0 placeholder
  private utf8Cache = new Map<string, number>();

  get count(): number { return this.entries.length; }

  addUtf8(s: string): number {
    const cached = this.utf8Cache.get(s);
    if (cached !== undefined) return cached;
    const bytes = new TextEncoder().encode(s);
    const data: number[] = [(bytes.length >> 8) & 0xff, bytes.length & 0xff, ...bytes];
    const idx = this.entries.length;
    this.entries.push({ tag: 1, data });
    this.utf8Cache.set(s, idx);
    return idx;
  }

  addInteger(v: number): number {
    const data = [(v >> 24) & 0xff, (v >> 16) & 0xff, (v >> 8) & 0xff, v & 0xff];
    const idx = this.entries.length;
    this.entries.push({ tag: 3, data });
    return idx;
  }

  addFloat(v: number): number {
    const buf = new ArrayBuffer(4);
    new DataView(buf).setFloat32(0, v);
    const bytes = new Uint8Array(buf);
    const data = [bytes[0], bytes[1], bytes[2], bytes[3]];
    const idx = this.entries.length;
    this.entries.push({ tag: 4, data });
    return idx;
  }

  addDouble(v: number): number {
    const buf = new ArrayBuffer(8);
    new DataView(buf).setFloat64(0, v);
    const bytes = new Uint8Array(buf);
    const data = [...bytes];
    const idx = this.entries.length;
    this.entries.push({ tag: 6, data });
    this.entries.push({ tag: 0, data: [] }); // occupies 2 CP entries
    return idx;
  }

  addLong(v: number): number {
    // CONSTANT_Long uses tag 5 and occupies 2 CP entries
    const hi = Math.floor(v / 0x100000000);
    const lo = v >>> 0;
    const data = [
      (hi >> 24) & 0xff, (hi >> 16) & 0xff, (hi >> 8) & 0xff, hi & 0xff,
      (lo >> 24) & 0xff, (lo >> 16) & 0xff, (lo >> 8) & 0xff, lo & 0xff,
    ];
    const idx = this.entries.length;
    this.entries.push({ tag: 5, data });
    // Long/Double constants occupy two entries; add a placeholder
    this.entries.push({ tag: 0, data: [] });
    return idx;
  }

  addClass(name: string): number {
    const nameIdx = this.addUtf8(name);
    const idx = this.entries.length;
    this.entries.push({ tag: 7, data: [(nameIdx >> 8) & 0xff, nameIdx & 0xff] });
    return idx;
  }

  addString(s: string): number {
    const strIdx = this.addUtf8(s);
    const idx = this.entries.length;
    this.entries.push({ tag: 8, data: [(strIdx >> 8) & 0xff, strIdx & 0xff] });
    return idx;
  }

  addNameAndType(name: string, descriptor: string): number {
    const nameIdx = this.addUtf8(name);
    const descIdx = this.addUtf8(descriptor);
    const idx = this.entries.length;
    this.entries.push({ tag: 12, data: [
      (nameIdx >> 8) & 0xff, nameIdx & 0xff,
      (descIdx >> 8) & 0xff, descIdx & 0xff,
    ]});
    return idx;
  }

  addFieldref(className: string, fieldName: string, descriptor: string): number {
    const classIdx = this.addClass(className);
    const natIdx = this.addNameAndType(fieldName, descriptor);
    const idx = this.entries.length;
    this.entries.push({ tag: 9, data: [
      (classIdx >> 8) & 0xff, classIdx & 0xff,
      (natIdx >> 8) & 0xff, natIdx & 0xff,
    ]});
    return idx;
  }

  addMethodref(className: string, methodName: string, descriptor: string): number {
    const classIdx = this.addClass(className);
    const natIdx = this.addNameAndType(methodName, descriptor);
    const idx = this.entries.length;
    this.entries.push({ tag: 10, data: [
      (classIdx >> 8) & 0xff, classIdx & 0xff,
      (natIdx >> 8) & 0xff, natIdx & 0xff,
    ]});
    return idx;
  }

  addInterfaceMethodref(className: string, methodName: string, descriptor: string): number {
    const classIdx = this.addClass(className);
    const natIdx = this.addNameAndType(methodName, descriptor);
    const idx = this.entries.length;
    this.entries.push({ tag: 11, data: [
      (classIdx >> 8) & 0xff, classIdx & 0xff,
      (natIdx >> 8) & 0xff, natIdx & 0xff,
    ]});
    return idx;
  }

  addMethodHandle(referenceKind: number, referenceIndex: number): number {
    const idx = this.entries.length;
    this.entries.push({ tag: 15, data: [referenceKind & 0xff, (referenceIndex >> 8) & 0xff, referenceIndex & 0xff] });
    return idx;
  }

  addMethodType(descriptor: string): number {
    const descIdx = this.addUtf8(descriptor);
    const idx = this.entries.length;
    this.entries.push({ tag: 16, data: [(descIdx >> 8) & 0xff, descIdx & 0xff] });
    return idx;
  }

  addInvokeDynamic(bootstrapMethodAttrIndex: number, name: string, descriptor: string): number {
    const natIdx = this.addNameAndType(name, descriptor);
    const idx = this.entries.length;
    this.entries.push({ tag: 18, data: [
      (bootstrapMethodAttrIndex >> 8) & 0xff, bootstrapMethodAttrIndex & 0xff,
      (natIdx >> 8) & 0xff, natIdx & 0xff,
    ]});
    return idx;
  }

  serialize(): number[] {
    const out: number[] = [];
    // count (u16)
    const count = this.entries.length;
    out.push((count >> 8) & 0xff, count & 0xff);
    for (let i = 1; i < count; i++) {
      const e = this.entries[i];
      out.push(e.tag, ...e.data);
    }
    return out;
  }
}

// Bytecode emitter
class BytecodeEmitter {
  code: number[] = [];
  maxStack = 0;
  maxLocals = 0;
  exceptionTable: { startPc: number; endPc: number; handlerPc: number; catchType: number }[] = [];
  private currentStack = 0;

  private adjustStack(delta: number) {
    this.currentStack += delta;
    if (this.currentStack > this.maxStack) this.maxStack = this.currentStack;
  }

  emit(byte: number) { this.code.push(byte); }
  emitU16(v: number) { this.code.push((v >> 8) & 0xff, v & 0xff); }

  get pc(): number { return this.code.length; }

  // Stack-tracking emit helpers
  emitPush(opcode: number) { this.emit(opcode); this.adjustStack(1); }
  emitPop(opcode: number) { this.emit(opcode); this.adjustStack(-1); }

  emitIconst(v: number) {
    if (v >= -1 && v <= 5) {
      this.emit(0x03 + v); // iconst_<n> (iconst_m1 = 0x02, iconst_0 = 0x03)
      if (v === -1) this.code[this.code.length - 1] = 0x02;
      else this.code[this.code.length - 1] = 0x03 + v;
    } else if (v >= -128 && v <= 127) {
      this.emit(0x10); // bipush
      this.emit(v & 0xff);
    } else if (v >= -32768 && v <= 32767) {
      this.emit(0x11); // sipush
      this.emitU16(v & 0xffff);
    } else {
      // Use ldc with integer constant
      return false; // caller should handle via ldc
    }
    this.adjustStack(1);
    return true;
  }

  emitFload(idx: number) {
    if (idx <= 3) this.emit(0x22 + idx); // fload_0..3
    else { this.emit(0x17); this.emit(idx); }
    this.adjustStack(1);
  }

  emitFstore(idx: number) {
    if (idx <= 3) this.emit(0x43 + idx); // fstore_0..3
    else { this.emit(0x38); this.emit(idx); }
    this.adjustStack(-1);
  }

  emitDload(idx: number) {
    if (idx <= 3) this.emit(0x26 + idx); // dload_0..3
    else { this.emit(0x18); this.emit(idx); }
    this.adjustStack(1);
  }

  emitDstore(idx: number) {
    if (idx <= 3) this.emit(0x47 + idx); // dstore_0..3
    else { this.emit(0x39); this.emit(idx); }
    this.adjustStack(-1);
  }

  emitFconst(v: number, cp: ConstantPoolBuilder): void {
    if (v === 0.0) { this.emit(0x0b); this.adjustStack(1); } // fconst_0
    else if (v === 1.0) { this.emit(0x0c); this.adjustStack(1); } // fconst_1
    else if (v === 2.0) { this.emit(0x0d); this.adjustStack(1); } // fconst_2
    else { this.emitLdc(cp.addFloat(v)); } // ldc adjusts stack
  }

  emitDconst(v: number, cp: ConstantPoolBuilder): void {
    if (v === 0.0) { this.emit(0x0e); } // dconst_0
    else if (v === 1.0) { this.emit(0x0f); } // dconst_1
    else {
      const cpIdx = cp.addDouble(v);
      this.emit(0x14); this.emitU16(cpIdx); // ldc2_w
    }
    this.adjustStack(1);
  }

  emitLconst(v: number, cp: ConstantPoolBuilder): void {
    if (v === 0) { this.emit(0x09); } // lconst_0
    else if (v === 1) { this.emit(0x0a); } // lconst_1
    else {
      const cpIdx = cp.addLong(v);
      this.emit(0x14); this.emitU16(cpIdx); // ldc2_w
    }
    this.adjustStack(1);
  }

  emitLdc(cpIdx: number) {
    if (cpIdx <= 255) {
      this.emit(0x12); // ldc
      this.emit(cpIdx);
    } else {
      this.emit(0x13); // ldc_w
      this.emitU16(cpIdx);
    }
    this.adjustStack(1);
  }

  emitAload(idx: number) {
    if (idx <= 3) this.emit(0x2a + idx); // aload_0..3
    else { this.emit(0x19); this.emit(idx); }
    this.adjustStack(1);
  }

  emitAstore(idx: number) {
    if (idx <= 3) this.emit(0x4b + idx); // astore_0..3
    else { this.emit(0x3a); this.emit(idx); }
    this.adjustStack(-1);
  }

  emitIload(idx: number) {
    if (idx <= 3) this.emit(0x1a + idx); // iload_0..3
    else { this.emit(0x15); this.emit(idx); }
    this.adjustStack(1);
  }

  emitIstore(idx: number) {
    if (idx <= 3) this.emit(0x3b + idx); // istore_0..3
    else { this.emit(0x36); this.emit(idx); }
    this.adjustStack(-1);
  }

  emitLload(idx: number) {
    if (idx <= 3) this.emit(0x1e + idx); // lload_0..3
    else { this.emit(0x16); this.emit(idx); }
    this.adjustStack(1);
  }

  emitLstore(idx: number) {
    if (idx <= 3) this.emit(0x3f + idx); // lstore_0..3
    else { this.emit(0x37); this.emit(idx); }
    this.adjustStack(-1);
  }

  emitInvokevirtual(cpIdx: number, argCount: number, hasReturn: boolean) {
    this.emit(0xb6);
    this.emitU16(cpIdx);
    // pops objectref + args, pushes result (if non-void)
    this.adjustStack(-(argCount + 1) + (hasReturn ? 1 : 0));
  }

  emitInvokespecial(cpIdx: number, argCount: number, hasReturn: boolean) {
    this.emit(0xb7);
    this.emitU16(cpIdx);
    this.adjustStack(-(argCount + 1) + (hasReturn ? 1 : 0));
  }

  emitInvokestatic(cpIdx: number, argCount: number, hasReturn: boolean) {
    this.emit(0xb8);
    this.emitU16(cpIdx);
    this.adjustStack(-argCount + (hasReturn ? 1 : 0));
  }

  emitInvokeinterface(cpIdx: number, argCount: number, hasReturn: boolean) {
    this.emit(0xb9);
    this.emitU16(cpIdx);
    this.emit(argCount + 1); // count
    this.emit(0); // reserved
    this.adjustStack(-(argCount + 1) + (hasReturn ? 1 : 0));
  }

  emitInvokedynamic(cpIdx: number, argCount: number, hasReturn: boolean) {
    this.emit(0xba);
    this.emitU16(cpIdx);
    this.emit(0);
    this.emit(0);
    this.adjustStack(-argCount + (hasReturn ? 1 : 0));
  }

  // Branch helpers: emit placeholder offset, return patch position
  emitBranch(opcode: number): number {
    this.emit(opcode);
    const patchPos = this.code.length;
    this.emitU16(0); // placeholder
    return patchPos;
  }

  patchBranch(patchPos: number, targetPc: number) {
    const offset = targetPc - (patchPos - 1); // relative to opcode position
    this.code[patchPos] = (offset >> 8) & 0xff;
    this.code[patchPos + 1] = offset & 0xff;
  }

  emitReturn(type: Type) {
    if (type === "void") this.emit(0xb1);
    else if (type === "long") { this.emit(0xad); this.adjustStack(-1); } // lreturn
    else if (type === "float") { this.emit(0xae); this.adjustStack(-1); } // freturn
    else if (type === "double") { this.emit(0xaf); this.adjustStack(-1); } // dreturn
    else if (type === "int" || type === "boolean" || type === "short" || type === "byte" || type === "char") { this.emit(0xac); this.adjustStack(-1); }
    else { this.emit(0xb0); this.adjustStack(-1); } // areturn
  }

  // For if_icmpge etc: pops 2, pushes 0
  adjustStackForCompare() { this.adjustStack(-2); }
  // For iaload/aaload: pops 2 (arrayref + index), pushes 1 (element)
  adjustStackForArrayLoad() { this.adjustStack(-1); }
  // Exception handler entry point: pushes exception object onto empty stack
  adjustStackForCatch() { this.adjustStack(1); }
}

// Type descriptor helpers
function typeToDescriptor(t: Type): string {
  if (t === "int") return "I";
  if (t === "long") return "J";
  if (t === "short") return "S";
  if (t === "byte") return "B";
  if (t === "char") return "C";
  if (t === "float") return "F";
  if (t === "double") return "D";
  if (t === "boolean") return "Z";
  if (t === "void") return "V";
  if (t === "String") return "Ljava/lang/String;";
  if (typeof t === "object" && "className" in t) return `L${t.className.replace(/\./g, "/")};`;
  if (typeof t === "object" && "array" in t) return `[${typeToDescriptor(t.array)}`;
  return "Ljava/lang/Object;";
}

function methodDescriptor(params: ParamDecl[], returnType: Type): string {
  return "(" + params.map(p => typeToDescriptor(p.type)).join("") + ")" + typeToDescriptor(returnType);
}

function isRefType(t: Type): boolean {
  return t !== "int" && t !== "long" && t !== "short" && t !== "byte" && t !== "char"
    && t !== "float" && t !== "double" && t !== "boolean" && t !== "void";
}

function isPrimitiveType(t: Type): boolean {
  return t === "int" || t === "long" || t === "short" || t === "byte" || t === "char"
    || t === "float" || t === "double" || t === "boolean";
}

function sameType(a: Type, b: Type): boolean {
  if (a === b) return true;
  if (typeof a === "object" && typeof b === "object") {
    if ("className" in a && "className" in b) return a.className === b.className;
    if ("array" in a && "array" in b) return sameType(a.array, b.array);
  }
  return false;
}

const WIDENING_RANK: Record<string, number> = {
  byte: 0, short: 1, int: 2, long: 3, float: 4, double: 5, char: 2 /* char → int level */
};

function isAssignable(to: Type, from: Type): boolean {
  if (sameType(to, from)) return true;
  if (isRefType(to) && isRefType(from)) return true;
  // byte/short/char are all ints on JVM — allow narrowing assignments between them and int
  if (isIntLike(to) && isIntLike(from)) return true;
  const toR = typeof to === "string" ? WIDENING_RANK[to] : undefined;
  const fromR = typeof from === "string" ? WIDENING_RANK[from] : undefined;
  if (toR !== undefined && fromR !== undefined) return toR >= fromR;
  return false;
}

function isKnownClass(ctx: CompileContext, cls: string): boolean {
  return cls === "java/lang/Object" || ctx.classSupers.has(cls) || !!BUILTIN_SUPERS[cls];
}

function isAssignableInContext(ctx: CompileContext, to: Type, from: Type): boolean {
  if (sameType(to, from)) return true;

  // Widening primitive conversions
  if (isPrimitiveType(to) && isPrimitiveType(from)) return isAssignable(to, from);
  if (isPrimitiveType(to) || isPrimitiveType(from)) return false;

  // Array assignments: exact array type or to Object.
  if (typeof to === "object" && "array" in to) {
    return typeof from === "object" && "array" in from && isAssignableInContext(ctx, to.array, from.array);
  }
  if (typeof from === "object" && "array" in from) {
    const toCls = toInternalClassName(ctx, to);
    return toCls === "java/lang/Object";
  }

  const toCls = toInternalClassName(ctx, to);
  const fromCls = toInternalClassName(ctx, from);
  if (!toCls || !fromCls) return isAssignable(to, from);
  if (toCls === "java/lang/Object") return true;
  if (fromCls === "java/lang/Object") return true;
  if (isClassSupertype(ctx, toCls, fromCls)) return true;

  // If both classes are known in hierarchy and not related, reject.
  if (isKnownClass(ctx, toCls) && isKnownClass(ctx, fromCls)) return false;
  // Unknown external hierarchy: keep permissive compatibility.
  return true;
}

function isCastConvertible(to: Type, from: Type): boolean {
  if (sameType(to, from)) return true;
  const toPrim = isPrimitiveType(to);
  const fromPrim = isPrimitiveType(from);
  if (toPrim && fromPrim) {
    // All numeric primitives can cast between each other
    const numerics: string[] = ["byte", "short", "char", "int", "long", "float", "double"];
    return numerics.includes(to as string) && numerics.includes(from as string);
  }
  // Unboxing cast: reference → primitive (e.g., (int) someObject)
  if (toPrim && !fromPrim) return true;
  // Boxing cast: primitive → reference (e.g., (Object) someInt)
  if (!toPrim && fromPrim) return true;
  return true;
}

/** Map from primitive type to its wrapper class and unbox method. */
const UNBOX_INFO: Record<string, { wrapper: string; method: string; desc: string }> = {
  int:     { wrapper: "java/lang/Integer",   method: "intValue",     desc: "()I" },
  long:    { wrapper: "java/lang/Long",      method: "longValue",    desc: "()J" },
  float:   { wrapper: "java/lang/Float",     method: "floatValue",   desc: "()F" },
  double:  { wrapper: "java/lang/Double",    method: "doubleValue",  desc: "()D" },
  boolean: { wrapper: "java/lang/Boolean",   method: "booleanValue", desc: "()Z" },
  byte:    { wrapper: "java/lang/Byte",      method: "byteValue",    desc: "()B" },
  short:   { wrapper: "java/lang/Short",     method: "shortValue",   desc: "()S" },
  char:    { wrapper: "java/lang/Character", method: "charValue",    desc: "()C" },
};

const BOX_INFO: Record<string, { wrapper: string; desc: string }> = {
  int:     { wrapper: "java/lang/Integer",   desc: "(I)Ljava/lang/Integer;" },
  long:    { wrapper: "java/lang/Long",      desc: "(J)Ljava/lang/Long;" },
  float:   { wrapper: "java/lang/Float",     desc: "(F)Ljava/lang/Float;" },
  double:  { wrapper: "java/lang/Double",    desc: "(D)Ljava/lang/Double;" },
  boolean: { wrapper: "java/lang/Boolean",   desc: "(Z)Ljava/lang/Boolean;" },
  byte:    { wrapper: "java/lang/Byte",      desc: "(B)Ljava/lang/Byte;" },
  short:   { wrapper: "java/lang/Short",     desc: "(S)Ljava/lang/Short;" },
  char:    { wrapper: "java/lang/Character", desc: "(C)Ljava/lang/Character;" },
};

function mergeTernaryType(a: Type, b: Type): Type {
  if (sameType(a, b)) return a;
  const numOrder = ["byte", "short", "char", "int", "long", "float", "double"] as const;
  const ai = numOrder.indexOf(a as typeof numOrder[number]);
  const bi = numOrder.indexOf(b as typeof numOrder[number]);
  if (ai >= 0 && bi >= 0) return numOrder[Math.max(ai, bi)];
  if ((a === "int" && b === "boolean") || (a === "boolean" && b === "int")) return "int";
  if (isRefType(a) && isRefType(b)) return { className: "java/lang/Object" };
  return a;
}

// Known class type mappings for method return types
interface MethodSig {
  owner: string;
  returnType: Type;
  paramTypes: Type[];
  isInterface?: boolean;
  isStatic?: boolean;
}

let knownMethods: Record<string, MethodSig> = {
  // Integer
  "java/lang/Integer.valueOf(I)": { owner: "java/lang/Integer", returnType: { className: "java/lang/Integer" }, paramTypes: ["int"], isStatic: true },
  "java/lang/Integer.toString()": { owner: "java/lang/Integer", returnType: "String", paramTypes: [] },
  "java/lang/Integer.intValue()": { owner: "java/lang/Integer", returnType: "int", paramTypes: [] },
  // String
  "java/lang/String.length()": { owner: "java/lang/String", returnType: "int", paramTypes: [] },
  "java/lang/String.charAt(I)": { owner: "java/lang/String", returnType: "int", paramTypes: ["int"] },
  "java/lang/String.substring(I)": { owner: "java/lang/String", returnType: "String", paramTypes: ["int"] },
  "java/lang/String.substring(II)": { owner: "java/lang/String", returnType: "String", paramTypes: ["int", "int"] },
  "java/lang/String.equals(Ljava/lang/Object;)": { owner: "java/lang/String", returnType: "boolean", paramTypes: [{ className: "java/lang/Object" }] },
  "java/lang/String.isEmpty()": { owner: "java/lang/String", returnType: "boolean", paramTypes: [] },
  "java/lang/String.contains(Ljava/lang/CharSequence;)": { owner: "java/lang/String", returnType: "boolean", paramTypes: [{ className: "java/lang/CharSequence" }] },
  "java/lang/String.concat(Ljava/lang/String;)": { owner: "java/lang/String", returnType: "String", paramTypes: ["String"] },
  "java/lang/String.toString()": { owner: "java/lang/String", returnType: "String", paramTypes: [] },
  // Object
  "java/lang/Object.toString()": { owner: "java/lang/Object", returnType: "String", paramTypes: [] },
  "java/lang/Object.getClass()": {
    owner: "java/lang/Object",
    returnType: { className: "java/lang/Class" },
    paramTypes: [],
  },
  // StringBuilder
  "java/lang/StringBuilder.<init>()": { owner: "java/lang/StringBuilder", returnType: "void", paramTypes: [] },
  "java/lang/StringBuilder.append(Ljava/lang/String;)": { owner: "java/lang/StringBuilder", returnType: { className: "java/lang/StringBuilder" }, paramTypes: ["String"] },
  "java/lang/StringBuilder.append(I)": { owner: "java/lang/StringBuilder", returnType: { className: "java/lang/StringBuilder" }, paramTypes: ["int"] },
  "java/lang/StringBuilder.append(J)": { owner: "java/lang/StringBuilder", returnType: { className: "java/lang/StringBuilder" }, paramTypes: ["long"] },
  "java/lang/StringBuilder.append(F)": { owner: "java/lang/StringBuilder", returnType: { className: "java/lang/StringBuilder" }, paramTypes: ["float"] },
  "java/lang/StringBuilder.append(D)": { owner: "java/lang/StringBuilder", returnType: { className: "java/lang/StringBuilder" }, paramTypes: ["double"] },
  "java/lang/StringBuilder.append(C)": { owner: "java/lang/StringBuilder", returnType: { className: "java/lang/StringBuilder" }, paramTypes: ["char"] },
  "java/lang/StringBuilder.append(Z)": { owner: "java/lang/StringBuilder", returnType: { className: "java/lang/StringBuilder" }, paramTypes: ["boolean"] },
  "java/lang/StringBuilder.append(Ljava/lang/Object;)": { owner: "java/lang/StringBuilder", returnType: { className: "java/lang/StringBuilder" }, paramTypes: [{ className: "java/lang/Object" }] },
  "java/lang/StringBuilder.toString()": { owner: "java/lang/StringBuilder", returnType: "String", paramTypes: [] },
  // ArrayList
  "java/util/ArrayList.<init>()": { owner: "java/util/ArrayList", returnType: "void", paramTypes: [] },
  "java/util/ArrayList.add(Ljava/lang/Object;)": { owner: "java/util/ArrayList", returnType: "boolean", paramTypes: [{ className: "java/lang/Object" }], isInterface: false },
  "java/util/ArrayList.get(I)": { owner: "java/util/ArrayList", returnType: { className: "java/lang/Object" }, paramTypes: ["int"], isInterface: false },
  "java/util/ArrayList.size()": { owner: "java/util/ArrayList", returnType: "int", paramTypes: [], isInterface: false },
  "java/util/ArrayList.remove(I)": { owner: "java/util/ArrayList", returnType: { className: "java/lang/Object" }, paramTypes: ["int"], isInterface: false },
  "java/util/ArrayList.set(ILjava/lang/Object;)": { owner: "java/util/ArrayList", returnType: { className: "java/lang/Object" }, paramTypes: ["int", { className: "java/lang/Object" }], isInterface: false },
  "java/util/ArrayList.isEmpty()": { owner: "java/util/ArrayList", returnType: "boolean", paramTypes: [], isInterface: false },
  // List interface
  "java/util/List.add(Ljava/lang/Object;)": { owner: "java/util/List", returnType: "boolean", paramTypes: [{ className: "java/lang/Object" }], isInterface: true },
  "java/util/List.get(I)": { owner: "java/util/List", returnType: { className: "java/lang/Object" }, paramTypes: ["int"], isInterface: true },
  "java/util/List.size()": { owner: "java/util/List", returnType: "int", paramTypes: [], isInterface: true },
  // Functional interfaces
  "java/util/function/Function.apply(Ljava/lang/Object;)": {
    owner: "java/util/function/Function",
    returnType: { className: "java/lang/Object" },
    paramTypes: [{ className: "java/lang/Object" }],
    isInterface: true,
  },
  "java/util/function/BiFunction.apply(Ljava/lang/Object;Ljava/lang/Object;)": {
    owner: "java/util/function/BiFunction",
    returnType: { className: "java/lang/Object" },
    paramTypes: [{ className: "java/lang/Object" }, { className: "java/lang/Object" }],
    isInterface: true,
  },
  "java/util/function/Predicate.test(Ljava/lang/Object;)": {
    owner: "java/util/function/Predicate",
    returnType: "boolean",
    paramTypes: [{ className: "java/lang/Object" }],
    isInterface: true,
  },
  "java/util/function/Consumer.accept(Ljava/lang/Object;)": {
    owner: "java/util/function/Consumer",
    returnType: "void",
    paramTypes: [{ className: "java/lang/Object" }],
    isInterface: true,
  },
  "java/util/function/Supplier.get()": {
    owner: "java/util/function/Supplier",
    returnType: { className: "java/lang/Object" },
    paramTypes: [],
    isInterface: true,
  },
  "java/lang/Runnable.run()": {
    owner: "java/lang/Runnable",
    returnType: "void",
    paramTypes: [],
    isInterface: true,
  },
  // CompletableFuture
  "java/util/concurrent/CompletableFuture.supplyAsync(Ljava/util/function/Supplier;)": {
    owner: "java/util/concurrent/CompletableFuture",
    returnType: { className: "java/util/concurrent/CompletableFuture" },
    paramTypes: [{ className: "java/util/function/Supplier" }],
    isStatic: true,
  },
  "java/util/concurrent/CompletableFuture.thenApply(Ljava/util/function/Function;)": {
    owner: "java/util/concurrent/CompletableFuture",
    returnType: { className: "java/util/concurrent/CompletableFuture" },
    paramTypes: [{ className: "java/util/function/Function" }],
  },
  "java/util/concurrent/CompletableFuture.join()": {
    owner: "java/util/concurrent/CompletableFuture",
    returnType: { className: "java/lang/Object" },
    paramTypes: [],
  },
  // Raoh core (baseline signatures to avoid load-order sensitivity)
  "net/unit8/raoh/ObjectDecoders.string()": {
    owner: "net/unit8/raoh/ObjectDecoders",
    returnType: { className: "net/unit8/raoh/builtin/StringDecoder" },
    paramTypes: [],
    isStatic: true,
  },
  "net/unit8/raoh/ObjectDecoders.int_()": {
    owner: "net/unit8/raoh/ObjectDecoders",
    returnType: { className: "net/unit8/raoh/builtin/IntDecoder" },
    paramTypes: [],
    isStatic: true,
  },
  "net/unit8/raoh/map/MapDecoders.field(Ljava/lang/String;Lnet/unit8/raoh/Decoder;)": {
    owner: "net/unit8/raoh/map/MapDecoders",
    returnType: { className: "net/unit8/raoh/FieldDecoder" },
    paramTypes: ["String", { className: "net/unit8/raoh/Decoder" }],
    isStatic: true,
  },
  "net/unit8/raoh/map/MapDecoders.combine(Lnet/unit8/raoh/Decoder;Lnet/unit8/raoh/Decoder;)": {
    owner: "net/unit8/raoh/map/MapDecoders",
    returnType: { className: "net/unit8/raoh/combinator/Combiner2" },
    paramTypes: [{ className: "net/unit8/raoh/Decoder" }, { className: "net/unit8/raoh/Decoder" }],
    isStatic: true,
  },
  "net/unit8/raoh/combinator/Combiner2.map(Ljava/util/function/BiFunction;)": {
    owner: "net/unit8/raoh/combinator/Combiner2",
    returnType: { className: "net/unit8/raoh/Decoder" },
    paramTypes: [{ className: "java/util/function/BiFunction" }],
  },
  "net/unit8/raoh/Decoder.decode(Ljava/lang/Object;)": {
    owner: "net/unit8/raoh/Decoder",
    returnType: { className: "net/unit8/raoh/Result" },
    paramTypes: [{ className: "java/lang/Object" }],
    isInterface: true,
  },
  "net/unit8/raoh/Result.getOrThrow()": {
    owner: "net/unit8/raoh/Result",
    returnType: { className: "java/lang/Object" },
    paramTypes: [],
    isInterface: true,
  },
  // PrintStream
  "java/io/PrintStream.println(Ljava/lang/String;)": { owner: "java/io/PrintStream", returnType: "void", paramTypes: ["String"] },
  "java/io/PrintStream.println(I)": { owner: "java/io/PrintStream", returnType: "void", paramTypes: ["int"] },
  "java/io/PrintStream.println(Ljava/lang/Object;)": { owner: "java/io/PrintStream", returnType: "void", paramTypes: [{ className: "java/lang/Object" }] },
  "java/io/PrintStream.print(Ljava/lang/String;)": { owner: "java/io/PrintStream", returnType: "void", paramTypes: ["String"] },
  "java/io/PrintStream.print(I)": { owner: "java/io/PrintStream", returnType: "void", paramTypes: ["int"] },
  // Class / reflection
  "java/lang/Class.getName()": {
    owner: "java/lang/Class",
    returnType: "String",
    paramTypes: [],
  },
  "java/lang/Class.isInstance(Ljava/lang/Object;)": {
    owner: "java/lang/Class",
    returnType: "boolean",
    paramTypes: [{ className: "java/lang/Object" }],
  },
  "java/lang/Class.getDeclaredFields()": {
    owner: "java/lang/Class",
    returnType: { array: { className: "java/lang/reflect/Field" } },
    paramTypes: [],
  },
  "java/lang/Class.getDeclaredMethods()": {
    owner: "java/lang/Class",
    returnType: { array: { className: "java/lang/reflect/Method" } },
    paramTypes: [],
  },
  "java/lang/Class.getDeclaredConstructors()": {
    owner: "java/lang/Class",
    returnType: { array: { className: "java/lang/reflect/Constructor" } },
    paramTypes: [],
  },
  "java/lang/Class.getInterfaces()": {
    owner: "java/lang/Class",
    returnType: { array: { className: "java/lang/Class" } },
    paramTypes: [],
  },
  "java/lang/Class.getSuperclass()": {
    owner: "java/lang/Class",
    returnType: { className: "java/lang/Class" },
    paramTypes: [],
  },
  "java/lang/Class.isInterface()": {
    owner: "java/lang/Class",
    returnType: "boolean",
    paramTypes: [],
  },
  "java/lang/Class.isAssignableFrom(Ljava/lang/Class;)": {
    owner: "java/lang/Class",
    returnType: "boolean",
    paramTypes: [{ className: "java/lang/Class" }],
  },
  "java/lang/Class.getModifiers()": {
    owner: "java/lang/Class",
    returnType: "int",
    paramTypes: [],
  },
  "java/lang/Class.isRecord()": {
    owner: "java/lang/Class",
    returnType: "boolean",
    paramTypes: [],
  },
  "java/lang/Class.getRecordComponents()": {
    owner: "java/lang/Class",
    returnType: { array: { className: "java/lang/reflect/RecordComponent" } },
    paramTypes: [],
  },
};

/** Merge an externally-built method registry into the known methods table. */
export function setMethodRegistry(reg: Record<string, MethodSig>): void {
  knownMethods = { ...knownMethods, ...reg };
}

// Environment for tracking local variables
interface LocalVar {
  name: string;
  type: Type;
  slot: number;
}

interface CompileContext {
  className: string;
  superClass: string;
  cp: ConstantPoolBuilder;
  method: MethodDecl;
  locals: LocalVar[];
  nextSlot: number;
  fields: FieldDecl[];         // own fields
  inheritedFields: FieldDecl[]; // fields from superclass(es)
  allMethods: MethodDecl[];
  importMap: Map<string, string>;
  packageImports: string[];
  staticWildcardImports: string[];
  classSupers: Map<string, string>;
  classDecls: Map<string, ClassDecl>;
  lambdaCounter: { value: number };
  generatedMethods: MethodDecl[];
  lambdaBootstraps: LambdaBootstrap[];
  ownerIsStatic: boolean;
  // Loop break/continue support
  breakPatches: { label?: string; patches: number[] }[];
  continuePatches: { label?: string; targets: number[] }[];
}

interface FunctionalSig {
  samMethod: string;
  params: Type[];
  returnType: Type;
}

interface LambdaBootstrap {
  samDescriptor: string;
  implOwner: string;
  implMethodName: string;
  implDescriptor: string;
  implIsInterface?: boolean;
  invokedName: string;
  invokedDescriptor: string;
  implRefKind: number;
}

const FUNCTIONAL_IFACES: Record<string, FunctionalSig> = {
  "java/lang/Runnable": { samMethod: "run", params: [], returnType: "void" },
  "java/util/function/Supplier": { samMethod: "get", params: [], returnType: { className: "java/lang/Object" } },
  "java/util/function/Consumer": { samMethod: "accept", params: [{ className: "java/lang/Object" }], returnType: "void" },
  "java/util/function/Predicate": { samMethod: "test", params: [{ className: "java/lang/Object" }], returnType: "boolean" },
  "java/util/function/Function": { samMethod: "apply", params: [{ className: "java/lang/Object" }], returnType: { className: "java/lang/Object" } },
  "java/util/function/BiFunction": {
    samMethod: "apply",
    params: [{ className: "java/lang/Object" }, { className: "java/lang/Object" }],
    returnType: { className: "java/lang/Object" },
  },
};

/** Look up a method in knownMethods, falling back to name-only match if exact arg types don't match. */
function lookupKnownMethod(owner: string, method: string, argDescs: string): MethodSig | undefined {
  const exact = knownMethods[`${owner}.${method}(${argDescs})`];
  if (exact) return exact;
  // Fallback: choose compatible overload by arity and primitive/ref compatibility.
  const prefix = `${owner}.${method}(`;
  const wantedArgs = splitDescriptorArgs(argDescs);
  let firstCompatible: MethodSig | undefined;
  for (const key of Object.keys(knownMethods)) {
    if (!key.startsWith(prefix)) continue;
    const start = key.indexOf("(");
    const end = key.indexOf(")");
    if (start < 0 || end < 0) continue;
    const keyArgs = splitDescriptorArgs(key.slice(start + 1, end));
    if (keyArgs.length !== wantedArgs.length) continue;
    const compatible = keyArgs.every((a, i) => {
      const b = wantedArgs[i];
      if (a === b) return true;
      const aRef = a.startsWith("L") || a.startsWith("[");
      const bRef = b.startsWith("L") || b.startsWith("[");
      return aRef && bRef;
    });
    if (compatible) {
      firstCompatible = knownMethods[key];
      break;
    }
  }
  return firstCompatible;
}

function findKnownMethodByArity(owner: string, method: string, arity: number, wantStatic: boolean): MethodSig | undefined {
  const prefix = `${owner}.${method}(`;
  for (const key of Object.keys(knownMethods)) {
    if (!key.startsWith(prefix)) continue;
    const sig = knownMethods[key];
    const isStatic = sig.isStatic ?? false;
    if (isStatic !== wantStatic) continue;
    if (sig.paramTypes.length === arity) return sig;
  }
  return undefined;
}

function splitDescriptorArgs(descs: string): string[] {
  const args: string[] = [];
  for (let i = 0; i < descs.length;) {
    if (descs[i] === "[") {
      let j = i;
      while (descs[j] === "[") j++;
      if (descs[j] === "L") {
        const semi = descs.indexOf(";", j);
        args.push(descs.slice(i, semi + 1));
        i = semi + 1;
      } else {
        args.push(descs.slice(i, j + 1));
        i = j + 1;
      }
      continue;
    }
    if (descs[i] === "L") {
      const semi = descs.indexOf(";", i);
      args.push(descs.slice(i, semi + 1));
      i = semi + 1;
      continue;
    }
    args.push(descs[i]);
    i++;
  }
  return args;
}

function hasFunctionalArg(args: Expr[]): boolean {
  return args.some(a => a.kind === "lambda" || a.kind === "methodRef");
}

/** Resolve a simple class name to its internal JVM name using the import map. */
function resolveClassName(ctx: CompileContext, name: string): string {
  // Already internal (contains '/') or fully qualified (contains '.')
  if (name.includes("/")) return name;
  if (name.includes(".")) return name.replace(/\./g, "/");
  const explicit = ctx.importMap.get(name);
  if (explicit) return explicit;
  if (ctx.classDecls.has(name)) return name;
  if (/^[A-Z]/.test(name) && ctx.packageImports.length > 0) {
    for (const pkg of ctx.packageImports) {
      const candidate = `${pkg}/${name}`;
      if (Object.keys(knownMethods).some(k => k.startsWith(`${candidate}.`))) return candidate;
    }
    return `${ctx.packageImports[0]}/${name}`;
  }
  return name;
}

function findLocal(ctx: CompileContext, name: string): LocalVar | undefined {
  return ctx.locals.find(l => l.name === name);
}

function addLocal(ctx: CompileContext, name: string, type: Type): number {
  const slot = ctx.nextSlot++;
  ctx.locals.push({ name, type, slot });
  return slot;
}

// Infer the type of an expression (best-effort)
function inferType(ctx: CompileContext, expr: Expr): Type {
  switch (expr.kind) {
    case "intLit": return "int";
    case "longLit": return "long";
    case "floatLit": return "float";
    case "doubleLit": return "double";
    case "charLit": return "char";
    case "stringLit": return "String";
    case "boolLit": return "boolean";
    case "nullLit": return { className: "java/lang/Object" };
    case "this": return { className: ctx.className };
    case "ident": {
      const loc = findLocal(ctx, expr.name);
      if (loc) return loc.type;
      const field = ctx.fields.find(f => f.name === expr.name);
      if (field) return field.type;
      const inherited = ctx.inheritedFields.find(f => f.name === expr.name);
      if (inherited) return inherited.type;
      return { className: expr.name };
    }
    case "binary": {
      if (["+", "-", "*", "/", "%"].includes(expr.op)) {
        const lt = inferType(ctx, expr.left);
        const rt = inferType(ctx, expr.right);
        // String concatenation
        if (expr.op === "+" && (lt === "String" || rt === "String")) return "String";
        // Numeric promotion
        if (lt === "double" || rt === "double") return "double";
        if (lt === "float" || rt === "float") return "float";
        if (lt === "long" || rt === "long") return "long";
        return "int"; // byte/short/char promote to int
      }
      return "boolean"; // comparison operators
    }
    case "unary": {
      if (expr.op === "!") return "boolean";
      const t = inferType(ctx, expr.operand);
      if (t === "double") return "double";
      if (t === "float") return "float";
      if (t === "long") return "long";
      return "int";
    }
    case "newExpr": return { className: resolveClassName(ctx, expr.className) };
    case "call": {
      if (expr.object) {
        const objType = inferType(ctx, expr.object);
        const rawOwner = objType === "String" ? "java/lang/String"
          : typeof objType === "object" && "className" in objType ? objType.className
          : "java/lang/Object";
        const ownerClass = resolveClassName(ctx, rawOwner);
        // Look in knownMethods
        const argDescs = expr.args.map(a => typeToDescriptor(inferType(ctx, a))).join("");
        const exactSig = lookupKnownMethod(ownerClass, expr.method, argDescs);
        const sig = exactSig
          ?? (hasFunctionalArg(expr.args) ? findKnownMethodByArity(ownerClass, expr.method, expr.args.length, false) : undefined);
        if (sig) return sig.returnType;
        // Check user-defined methods
        const userMethod = ctx.allMethods.find(m => m.name === expr.method);
        if (userMethod) return userMethod.returnType;
      } else {
        // Unqualified call — look in user-defined methods
        const userMethod = ctx.allMethods.find(m => m.name === expr.method);
        if (userMethod) return userMethod.returnType;
        // Static import-on-demand method
        if (ctx.staticWildcardImports.length > 0) {
          const argDescs = expr.args.map(a => typeToDescriptor(inferType(ctx, a))).join("");
          for (const owner of ctx.staticWildcardImports) {
            const exact = lookupKnownMethod(owner, expr.method, argDescs);
            const sig = exact
              ?? (hasFunctionalArg(expr.args) ? findKnownMethodByArity(owner, expr.method, expr.args.length, true) : undefined);
            if (sig) return sig.returnType;
          }
        }
      }
      return { className: "java/lang/Object" };
    }
    case "staticCall": {
      const argDescs = expr.args.map(a => typeToDescriptor(inferType(ctx, a))).join("");
      const internalName = expr.className.replace(/\./g, "/");
      const exactSig = lookupKnownMethod(internalName, expr.method, argDescs);
      const sig = exactSig
        ?? (hasFunctionalArg(expr.args) ? findKnownMethodByArity(internalName, expr.method, expr.args.length, true) : undefined);
      if (sig) return sig.returnType;
      // Check user-defined static methods
      const userMethod = ctx.allMethods.find(m => m.name === expr.method && m.isStatic);
      if (userMethod) return userMethod.returnType;
      return { className: "java/lang/Object" };
    }
    case "fieldAccess": {
      if (expr.field === "out") return { className: "java/io/PrintStream" };
      if (expr.field === "length") return "int";
      const fld = ctx.fields.find(f => f.name === expr.field);
      if (fld) return fld.type;
      return { className: "java/lang/Object" };
    }
    case "cast": return expr.type;
    case "postIncrement": return inferType(ctx, expr.operand);
    case "preIncrement": return inferType(ctx, expr.operand);
    case "instanceof": return "boolean";
    case "staticField": return { className: "java/lang/Object" };
    case "arrayAccess": {
      const arrType = inferType(ctx, expr.array);
      if (typeof arrType === "object" && "array" in arrType) return arrType.array;
      return "int"; // fallback
    }
    case "arrayLit": return { array: expr.elemType };
    case "newArray": return { array: expr.elemType };
    case "superCall": return "void";
    case "ternary": return mergeTernaryType(inferType(ctx, expr.thenExpr), inferType(ctx, expr.elseExpr));
    case "switchExpr": {
      let current: Type | undefined;
      for (const c of expr.cases) {
        if (c.expr) {
          const t = inferType(ctx, c.expr);
          current = current ? mergeTernaryType(current, t) : t;
        } else if (c.stmts) {
          for (const s of c.stmts) {
            if (s.kind === "yield") {
              const t = inferType(ctx, s.value);
              current = current ? mergeTernaryType(current, t) : t;
            }
          }
        }
      }
      return current ?? { className: "java/lang/Object" };
    }
    case "lambda": return { className: "java/lang/Object" };
    case "methodRef": return { className: "java/lang/Object" };
    case "classLit": return { className: "java/lang/Class" };
  }
}

function compileExpr(ctx: CompileContext, emitter: BytecodeEmitter, expr: Expr, expectedType?: Type): void {
  switch (expr.kind) {
    case "intLit": {
      if (!emitter.emitIconst(expr.value)) {
        const cpIdx = ctx.cp.addInteger(expr.value);
        emitter.emitLdc(cpIdx);
      }
      break;
    }
    case "longLit": {
      emitter.emitLconst(expr.value, ctx.cp);
      break;
    }
    case "floatLit": {
      emitter.emitFconst(expr.value, ctx.cp);
      break;
    }
    case "doubleLit": {
      emitter.emitDconst(expr.value, ctx.cp);
      break;
    }
    case "charLit": {
      // char is stored as int on JVM
      if (!emitter.emitIconst(expr.value)) {
        const cpIdx = ctx.cp.addInteger(expr.value);
        emitter.emitLdc(cpIdx);
      }
      break;
    }
    case "stringLit": {
      const cpIdx = ctx.cp.addString(expr.value);
      emitter.emitLdc(cpIdx);
      break;
    }
    case "boolLit": {
      emitter.emitIconst(expr.value ? 1 : 0);
      break;
    }
    case "nullLit": {
      emitter.emit(0x01); // aconst_null
      break;
    }
    case "this": {
      emitter.emitAload(0);
      break;
    }
    case "ident": {
      const loc = findLocal(ctx, expr.name);
      if (loc) {
        emitLoadLocalByType(emitter, loc.slot, loc.type);
        break;
      }
      // Check own fields
      const field = ctx.fields.find(f => f.name === expr.name);
      if (field) {
        if (field.isStatic) {
          const fRef = ctx.cp.addFieldref(ctx.className, expr.name, typeToDescriptor(field.type));
          emitter.emit(0xb2); // getstatic
          emitter.emitU16(fRef);
        } else {
          emitter.emitAload(0); // this
          const fRef = ctx.cp.addFieldref(ctx.className, expr.name, typeToDescriptor(field.type));
          emitter.emit(0xb4); // getfield
          emitter.emitU16(fRef);
        }
        break;
      }
      // Check inherited fields (superclass fields accessed without this.)
      const inherited = ctx.inheritedFields.find(f => f.name === expr.name);
      if (inherited) {
        emitter.emitAload(0); // this
        const fRef = ctx.cp.addFieldref(ctx.superClass, expr.name, typeToDescriptor(inherited.type));
        emitter.emit(0xb4); // getfield
        emitter.emitU16(fRef);
        break;
      }
      // Must be a class name reference — push as-is (will be consumed by field/method access)
      // We handle this in the call/fieldAccess cases
      break;
    }
    case "binary": {
      const leftType = inferType(ctx, expr.left);
      const rightType = inferType(ctx, expr.right);

      // String concatenation: use StringBuilder
      if (expr.op === "+" && (leftType === "String" || rightType === "String")) {
        compileStringConcat(ctx, emitter, expr);
        break;
      }

      // Logical operators with short-circuit
      if (expr.op === "&&") {
        if (!(leftType === "boolean" && rightType === "boolean")) {
          throw new Error("Operator '&&' requires boolean operands");
        }
        compileExpr(ctx, emitter, expr.left);
        const patchFalse = emitter.emitBranch(0x99); // ifeq
        compileExpr(ctx, emitter, expr.right);
        const patchEnd = emitter.emitBranch(0xa7); // goto
        emitter.patchBranch(patchFalse, emitter.pc);
        emitter.emitIconst(0);
        emitter.patchBranch(patchEnd, emitter.pc);
        break;
      }
      if (expr.op === "||") {
        if (!(leftType === "boolean" && rightType === "boolean")) {
          throw new Error("Operator '||' requires boolean operands");
        }
        compileExpr(ctx, emitter, expr.left);
        const patchEvalRight = emitter.emitBranch(0x99); // ifeq
        emitter.emitIconst(1);
        const patchEnd = emitter.emitBranch(0xa7); // goto
        emitter.patchBranch(patchEvalRight, emitter.pc);
        compileExpr(ctx, emitter, expr.right);
        emitter.patchBranch(patchEnd, emitter.pc);
        break;
      }

      // Numeric promotion: determine the promoted type
      function promoteNumeric(a: Type, b: Type): Type {
        if (a === "double" || b === "double") return "double";
        if (a === "float" || b === "float") return "float";
        if (a === "long" || b === "long") return "long";
        return "int";
      }

      // Type check for arithmetic operators
      if (["+", "-", "*", "/", "%"].includes(expr.op)) {
        if (!isPrimitiveType(leftType) || !isPrimitiveType(rightType) || leftType === "boolean" || rightType === "boolean") {
          throw new Error(`Operator '${expr.op}' requires numeric operands`);
        }
      }
      if (["<", ">", "<=", ">="].includes(expr.op)) {
        if (!isPrimitiveType(leftType) || !isPrimitiveType(rightType) || leftType === "boolean" || rightType === "boolean") {
          throw new Error(`Operator '${expr.op}' requires numeric operands`);
        }
      }
      if (expr.op === "==" || expr.op === "!=") {
        const leftRef = isRefType(leftType);
        const rightRef = isRefType(rightType);
        if (leftRef !== rightRef) {
          throw new Error(`Operator '${expr.op}' requires operands of compatible categories`);
        }
        if (!leftRef && !rightRef && !sameType(leftType, rightType)) {
          // Allow numeric comparison with promotion
          if (leftType === "boolean" || rightType === "boolean") {
            throw new Error(`Operator '${expr.op}' requires operands of the same primitive type`);
          }
        }
      }

      const promoted = (expr.op === "==" || expr.op === "!=")
        && (isRefType(leftType) || isRefType(rightType))
        ? leftType // ref compare, no promotion
        : promoteNumeric(leftType, rightType);

      // Emit operands with widening
      compileExpr(ctx, emitter, expr.left);
      if (!isRefType(leftType)) emitWideningConversion(emitter, leftType, promoted);
      compileExpr(ctx, emitter, expr.right);
      if (!isRefType(rightType)) emitWideningConversion(emitter, rightType, promoted);

      // Emit operation based on promoted type
      if (promoted === "double") {
        switch (expr.op) {
          case "+": emitter.emit(0x63); break; // dadd
          case "-": emitter.emit(0x67); break; // dsub
          case "*": emitter.emit(0x6b); break; // dmul
          case "/": emitter.emit(0x6f); break; // ddiv
          case "%": emitter.emit(0x73); break; // drem
          case "==": case "!=": case "<": case ">": case "<=": case ">=": {
            emitter.emitPush(0x97); // dcmpl → int
            const jumpOp = { "==": 0x9a, "!=": 0x99, "<": 0x9c, ">": 0x9e, "<=": 0x9d, ">=": 0x9b }[expr.op]!;
            const patchFalse = emitter.emitBranch(jumpOp);
            emitter.emitIconst(1);
            const patchEnd = emitter.emitBranch(0xa7);
            emitter.patchBranch(patchFalse, emitter.pc);
            emitter.emitIconst(0);
            emitter.patchBranch(patchEnd, emitter.pc);
            break;
          }
          default: throw new Error(`Unsupported binary operator: ${expr.op}`);
        }
      } else if (promoted === "float") {
        switch (expr.op) {
          case "+": emitter.emit(0x62); break; // fadd
          case "-": emitter.emit(0x66); break; // fsub
          case "*": emitter.emit(0x6a); break; // fmul
          case "/": emitter.emit(0x6e); break; // fdiv
          case "%": emitter.emit(0x72); break; // frem
          case "==": case "!=": case "<": case ">": case "<=": case ">=": {
            emitter.emitPush(0x95); // fcmpl → int
            const jumpOp = { "==": 0x9a, "!=": 0x99, "<": 0x9c, ">": 0x9e, "<=": 0x9d, ">=": 0x9b }[expr.op]!;
            const patchFalse = emitter.emitBranch(jumpOp);
            emitter.emitIconst(1);
            const patchEnd = emitter.emitBranch(0xa7);
            emitter.patchBranch(patchFalse, emitter.pc);
            emitter.emitIconst(0);
            emitter.patchBranch(patchEnd, emitter.pc);
            break;
          }
          default: throw new Error(`Unsupported binary operator: ${expr.op}`);
        }
      } else if (promoted === "long") {
        switch (expr.op) {
          case "+": emitter.emit(0x61); break; // ladd
          case "-": emitter.emit(0x65); break; // lsub
          case "*": emitter.emit(0x69); break; // lmul
          case "/": emitter.emit(0x6d); break; // ldiv
          case "%": emitter.emit(0x71); break; // lrem
          case "==": case "!=": case "<": case ">": case "<=": case ">=": {
            emitter.emitPush(0x94); // lcmp → int
            const jumpOp = { "==": 0x9a, "!=": 0x99, "<": 0x9c, ">": 0x9e, "<=": 0x9d, ">=": 0x9b }[expr.op]!;
            const patchFalse = emitter.emitBranch(jumpOp);
            emitter.emitIconst(1);
            const patchEnd = emitter.emitBranch(0xa7);
            emitter.patchBranch(patchFalse, emitter.pc);
            emitter.emitIconst(0);
            emitter.patchBranch(patchEnd, emitter.pc);
            break;
          }
          default: throw new Error(`Unsupported binary operator: ${expr.op}`);
        }
      } else {
        // int (includes byte/short/char promoted to int)
        switch (expr.op) {
          case "+": emitter.emit(0x60); break; // iadd
          case "-": emitter.emit(0x64); break; // isub
          case "*": emitter.emit(0x68); break; // imul
          case "/": emitter.emit(0x6c); break; // idiv
          case "%": emitter.emit(0x70); break; // irem
          case "==": case "!=": case "<": case ">": case "<=": case ">=": {
            const refCompare = (expr.op === "==" || expr.op === "!=")
              && (isRefType(leftType) || isRefType(rightType));
            const jumpOp = refCompare
              ? (expr.op === "==" ? 0xa6 : 0xa5) // if_acmpne / if_acmpeq
              : { "==": 0xa0, "!=": 0x9f, "<": 0xa2, ">": 0xa4, "<=": 0xa3, ">=": 0xa1 }[expr.op]!;
            const patchFalse = emitter.emitBranch(jumpOp);
            emitter.emitIconst(1);
            const patchEnd = emitter.emitBranch(0xa7);
            emitter.patchBranch(patchFalse, emitter.pc);
            emitter.emitIconst(0);
            emitter.patchBranch(patchEnd, emitter.pc);
            break;
          }
          default: throw new Error(`Unsupported binary operator: ${expr.op}`);
        }
      }
      break;
    }
    case "unary": {
      const operandType = inferType(ctx, expr.operand);
      compileExpr(ctx, emitter, expr.operand);
      if (expr.op === "-") {
        if (operandType === "double") emitter.emit(0x77); // dneg
        else if (operandType === "float") emitter.emit(0x76); // fneg
        else if (operandType === "long") emitter.emit(0x75); // lneg
        else if (isPrimitiveType(operandType) && operandType !== "boolean") emitter.emit(0x74); // ineg
        else throw new Error("Unary '-' requires numeric operand");
      }
      if (expr.op === "!") {
        if (operandType !== "boolean") throw new Error("Unary '!' requires boolean operand");
        // XOR with 1
        emitter.emitIconst(1);
        emitter.emit(0x82); // ixor
      }
      break;
    }
    case "newExpr": {
      const internalName = resolveClassName(ctx, expr.className);
      const classIdx = ctx.cp.addClass(internalName);
      emitter.emit(0xbb); // new
      emitter.emitU16(classIdx);
      emitter.emit(0x59); // dup
      // Compile constructor args
      const argTypes = expr.args.map(a => typeToDescriptor(inferType(ctx, a)));
      for (const arg of expr.args) compileExpr(ctx, emitter, arg);
      const desc = "(" + argTypes.join("") + ")V";
      const mRef = ctx.cp.addMethodref(internalName, "<init>", desc);
      emitter.emitInvokespecial(mRef, expr.args.length, false);
      break;
    }
    case "call": {
      compileCall(ctx, emitter, expr);
      break;
    }
    case "staticCall": {
      const internalName = expr.className.replace(/\./g, "/");
      const argTypes = expr.args.map(a => typeToDescriptor(inferType(ctx, a)));
      for (const arg of expr.args) compileExpr(ctx, emitter, arg);
      const retType = inferType(ctx, expr);
      const desc = "(" + argTypes.join("") + ")" + typeToDescriptor(retType);
      const mRef = ctx.cp.addMethodref(internalName, expr.method, desc);
      emitter.emitInvokestatic(mRef, expr.args.length, retType !== "void");
      break;
    }
    case "fieldAccess": {
      compileFieldAccess(ctx, emitter, expr);
      break;
    }
    case "postIncrement": {
      // For simple ident post-increment in expression context
      if (expr.operand.kind === "ident") {
        const loc = findLocal(ctx, expr.operand.name);
        if (loc && (loc.type === "int" || loc.type === "boolean")) {
          emitter.emitIload(loc.slot); // push old value
          // Increment in place
          emitter.emit(0x84); // iinc
          emitter.emit(loc.slot);
          emitter.emit(expr.op === "++" ? 1 : 0xff); // +1 or -1
          break;
        }
      }
      compileExpr(ctx, emitter, expr.operand);
      break;
    }
    case "preIncrement": {
      // For simple ident pre-increment: increment first, then push new value
      if (expr.operand.kind === "ident") {
        const loc = findLocal(ctx, expr.operand.name);
        if (loc && (loc.type === "int" || loc.type === "boolean")) {
          emitter.emit(0x84); // iinc
          emitter.emit(loc.slot);
          emitter.emit(expr.op === "++" ? 1 : 0xff); // +1 or -1
          emitter.emitIload(loc.slot); // push new value
          break;
        }
      }
      compileExpr(ctx, emitter, expr.operand);
      break;
    }
    case "cast": {
      const srcType = inferType(ctx, expr.expr);
      if (!isCastConvertible(expr.type, srcType)) {
        throw new Error(`Invalid cast from ${typeToDescriptor(srcType)} to ${typeToDescriptor(expr.type)}`);
      }
      compileExpr(ctx, emitter, expr.expr);
      if (isPrimitiveType(expr.type) && isRefType(srcType)) {
        // Unboxing cast: (int) someObject → checkcast Integer; invokevirtual intValue
        const info = UNBOX_INFO[expr.type as string];
        if (info) {
          const classIdx = ctx.cp.addClass(info.wrapper);
          emitter.emit(0xc0); // checkcast
          emitter.emitU16(classIdx);
          const methodRef = ctx.cp.addMethodref(info.wrapper, info.method, info.desc);
          emitter.emit(0xb6); // invokevirtual
          emitter.emitU16(methodRef);
        }
      } else if (isRefType(expr.type) && isPrimitiveType(srcType)) {
        // Boxing cast: (Object) someInt → invokestatic Integer.valueOf(int)
        const info = BOX_INFO[srcType as string];
        if (info) {
          const methodRef = ctx.cp.addMethodref(info.wrapper, "valueOf", info.desc);
          emitter.emit(0xb8); // invokestatic
          emitter.emitU16(methodRef);
        }
      } else if (isRefType(expr.type)) {
        const castClass = typeof expr.type === "object" && "className" in expr.type
          ? resolveClassName(ctx, expr.type.className)
          : "java/lang/Object";
        const classIdx = ctx.cp.addClass(castClass);
        emitter.emit(0xc0); // checkcast
        emitter.emitU16(classIdx);
      } else if (isPrimitiveType(expr.type) && isPrimitiveType(srcType)) {
        // Numeric cast: try widening first, then narrowing
        emitWideningConversion(emitter, srcType, expr.type);
        emitNarrowingConversion(emitter, srcType, expr.type);
      }
      break;
    }
    case "instanceof": {
      compileExpr(ctx, emitter, expr.expr);
      const checkClass = resolveClassName(ctx, expr.checkType);
      const classIdx = ctx.cp.addClass(checkClass);
      emitter.emit(0xc1); // instanceof
      emitter.emitU16(classIdx);
      // If there's a pattern variable, store the object into a new local after the instanceof check
      // (The actual binding is handled in compileStmt for if-instanceof patterns)
      break;
    }
    case "staticField": {
      const ownerClass = resolveClassName(ctx, expr.className);
      // Try known static fields first
      if (ownerClass === "java/lang/System" && expr.field === "out") {
        const fieldRef = ctx.cp.addFieldref("java/lang/System", "out", "Ljava/io/PrintStream;");
        emitter.emit(0xb2); emitter.emitU16(fieldRef);
      } else {
        // Generic static field access — type assumed Object
        const fieldRef = ctx.cp.addFieldref(ownerClass, expr.field, "Ljava/lang/Object;");
        emitter.emit(0xb2); emitter.emitU16(fieldRef);
      }
      break;
    }
    case "newArray": {
      compileExpr(ctx, emitter, expr.size);
      if (expr.elemType === "int" || expr.elemType === "boolean") {
        emitter.emit(0xbc); // newarray
        emitter.emit(expr.elemType === "int" ? 10 : 4); // T_INT=10, T_BOOLEAN=4
      } else {
        const internalName = typeof expr.elemType === "object" && "className" in expr.elemType
          ? expr.elemType.className : "java/lang/Object";
        const classIdx = ctx.cp.addClass(internalName);
        emitter.emit(0xbd); // anewarray
        emitter.emitU16(classIdx);
      }
      break;
    }
    case "arrayLit": {
      // Emit size, newarray, then fill each element
      emitter.emitIconst(expr.elements.length) || (() => {
        const cpIdx = ctx.cp.addInteger(expr.elements.length);
        emitter.emitLdc(cpIdx);
      })();
      if (expr.elemType === "int" || expr.elemType === "boolean") {
        emitter.emit(0xbc); emitter.emit(10); // newarray T_INT
      } else {
        const internalName = typeof expr.elemType === "object" && "className" in expr.elemType
          ? expr.elemType.className : "java/lang/Object";
        const classIdx = ctx.cp.addClass(internalName);
        emitter.emit(0xbd); emitter.emitU16(classIdx); // anewarray
      }
      for (let i = 0; i < expr.elements.length; i++) {
        emitter.emit(0x59); // dup
        emitter.emitIconst(i) || (() => { const ci = ctx.cp.addInteger(i); emitter.emitLdc(ci); })();
        compileExpr(ctx, emitter, expr.elements[i]);
        if (expr.elemType === "int" || expr.elemType === "boolean") {
          emitter.emit(0x4f); // iastore
        } else {
          emitter.emit(0x53); // aastore
        }
      }
      break;
    }
    case "arrayAccess": {
      compileExpr(ctx, emitter, expr.array);
      compileExpr(ctx, emitter, expr.index);
      const elemType = inferType(ctx, expr);
      if (elemType === "int" || elemType === "boolean") {
        emitter.emit(0x2e); // iaload
      } else {
        emitter.emit(0x32); // aaload
      }
      break;
    }
    case "superCall": {
      // invokespecial SuperClass.<init>(args)V
      emitter.emitAload(0); // this
      const argTypes = expr.args.map(a => typeToDescriptor(inferType(ctx, a)));
      for (const arg of expr.args) compileExpr(ctx, emitter, arg);
      const desc = "(" + argTypes.join("") + ")V";
      const mRef = ctx.cp.addMethodref(ctx.superClass, "<init>", desc);
      emitter.emitInvokespecial(mRef, expr.args.length, false);
      break;
    }
    case "ternary": {
      // cond ? thenExpr : elseExpr
      if (inferType(ctx, expr.cond) !== "boolean") {
        throw new Error("Ternary condition must be boolean");
      }
      const thenType = inferType(ctx, expr.thenExpr);
      const elseType = inferType(ctx, expr.elseExpr);
      const refCompatible = isRefType(thenType) && isRefType(elseType);
      if (!refCompatible && !isAssignableInContext(ctx, thenType, elseType) && !isAssignableInContext(ctx, elseType, thenType)) {
        throw new Error("Ternary branches must have compatible types");
      }
      compileExpr(ctx, emitter, expr.cond);
      const patchElse = emitter.emitBranch(0x99); // ifeq — jump to else if cond == 0
      compileExpr(ctx, emitter, expr.thenExpr);
      const patchEnd = emitter.emitBranch(0xa7); // goto — skip else
      emitter.patchBranch(patchElse, emitter.pc);
      compileExpr(ctx, emitter, expr.elseExpr);
      emitter.patchBranch(patchEnd, emitter.pc);
      break;
    }
    case "switchExpr": {
      compileSwitchExpr(ctx, emitter, expr, expectedType);
      break;
    }
    case "lambda": {
      if (!expectedType) {
        throw new Error("Lambda expression requires target type context");
      }
      const { ifaceName, sig } = functionalSigForType(ctx, expectedType);
      if (expr.params.length !== sig.params.length) {
        throw new Error(`Lambda parameter count mismatch: expected ${sig.params.length}, got ${expr.params.length}`);
      }

      // Non-capturing lambdas only for now.
      const used = new Set<string>();
      if (expr.bodyExpr) collectExprIdentifiers(expr.bodyExpr, used);
      if (expr.bodyStmts) for (const s of expr.bodyStmts) collectStmtIdentifiers(s, used);
      const paramSet = new Set(expr.params);
      const captures = ctx.locals.filter(l => used.has(l.name) && !paramSet.has(l.name));
      const needsThisCapture = !ctx.ownerIsStatic;

      const lambdaId = ctx.lambdaCounter.value++;
      const implName = `lambda$${ctx.method.name}$${lambdaId}`;
      const captureParams: ParamDecl[] = captures.map(c => ({ name: c.name, type: c.type }));
      const lambdaParams: ParamDecl[] = expr.params.map((p, i) => ({ name: p, type: sig.params[i] }));
      const implParams: ParamDecl[] = [...captureParams, ...lambdaParams];
      const implBody: Stmt[] = expr.bodyExpr
        ? [{ kind: "return", value: expr.bodyExpr }]
        : (expr.bodyStmts ?? []);
      const implMethod: MethodDecl = {
        name: implName,
        returnType: sig.returnType,
        params: implParams,
        body: implBody,
        isStatic: !needsThisCapture,
      };
      ctx.generatedMethods.push(implMethod);

      const implDesc = methodDescriptor(implParams, sig.returnType);
      const capturedTypes: Type[] = [
        ...(needsThisCapture ? [{ className: ctx.className } as Type] : []),
        ...captures.map(c => c.type),
      ];
      for (const cap of captures) {
        if (cap.type === "void") throw new Error("Unsupported capture type: void");
      }
      for (let i = 0; i < capturedTypes.length; i++) {
        compileExpr(ctx, emitter, needsThisCapture && i === 0 ? ({ kind: "this" } as Expr) : ({ kind: "ident", name: captures[needsThisCapture ? i - 1 : i].name } as Expr));
      }
      const invokedDesc = "(" + capturedTypes.map(typeToDescriptor).join("") + ")" + typeToDescriptor(expectedType);
      const samDescriptor = "(" + sig.params.map(typeToDescriptor).join("") + ")" + typeToDescriptor(sig.returnType);
      ctx.lambdaBootstraps.push({
        samDescriptor,
        implOwner: ctx.className,
        implMethodName: implName,
        implDescriptor: implDesc,
        invokedName: sig.samMethod,
        invokedDescriptor: invokedDesc,
        implRefKind: needsThisCapture ? 5 : 6,
      });
      const bootstrapIdx = ctx.lambdaBootstraps.length - 1;
      const indyIdx = ctx.cp.addInvokeDynamic(bootstrapIdx, sig.samMethod, invokedDesc);
      emitter.emitInvokedynamic(indyIdx, capturedTypes.length, true);
      break;
    }
    case "methodRef": {
      if (!expectedType) throw new Error("Method reference requires target type context");
      const { sig } = functionalSigForType(ctx, expectedType);

      let implOwner = "";
      let implName = expr.method;
      let implDescriptor = "";
      let implRefKind = 6;
      let implIsInterface = false;
      let captureTypes: Type[] = [];

      const isClassRef = expr.target.kind === "ident"
        && !findLocal(ctx, expr.target.name)
        && (/^[A-Z]/.test(expr.target.name) || ctx.importMap.has(expr.target.name) || resolveClassName(ctx, expr.target.name) !== expr.target.name);

      if (expr.isConstructor) {
        if (!(expr.target.kind === "ident" && isClassRef)) {
          throw new Error("Constructor method reference target must be a class name");
        }
        const targetClass = resolveClassName(ctx, expr.target.name);
        const ctorId = ctx.lambdaCounter.value++;
        const ctorImplName = `lambda$ctor$${ctorId}`;
        const ctorParams: ParamDecl[] = sig.params.map((p, i) => ({ name: `p${i}`, type: p }));
        const argDescs = ctorParams.map(p => typeToDescriptor(p.type)).join("");
        const ctorKnown = lookupKnownMethod(targetClass, "<init>", argDescs)
          ?? findKnownMethodByArity(targetClass, "<init>", ctorParams.length, false);
        const ctorTypes = ctorKnown?.paramTypes ?? ctorParams.map(p => p.type);
        const ctorArgs: Expr[] = ctorParams.map((p, i) => {
          const need = ctorTypes[i];
          if (need && !sameType(need, p.type)) {
            return { kind: "cast", type: need, expr: { kind: "ident", name: p.name } } as Expr;
          }
          return { kind: "ident", name: p.name } as Expr;
        });
        const ctorMethod: MethodDecl = {
          name: ctorImplName,
          returnType: sig.returnType,
          params: ctorParams,
          body: [{ kind: "return", value: { kind: "newExpr", className: targetClass, args: ctorArgs } }],
          isStatic: true,
        };
        ctx.generatedMethods.push(ctorMethod);
        const implDescCtor = methodDescriptor(ctorMethod.params, ctorMethod.returnType);
        const invokedDescriptorCtor = "()" + typeToDescriptor(expectedType);
        const samDescriptorCtor = "(" + sig.params.map(typeToDescriptor).join("") + ")" + typeToDescriptor(sig.returnType);
        ctx.lambdaBootstraps.push({
          samDescriptor: samDescriptorCtor,
          implOwner: ctx.className,
          implMethodName: ctorImplName,
          implDescriptor: implDescCtor,
          invokedName: sig.samMethod,
          invokedDescriptor: invokedDescriptorCtor,
          implRefKind: 6,
        });
        const bootstrapIdx = ctx.lambdaBootstraps.length - 1;
        const indyIdx = ctx.cp.addInvokeDynamic(bootstrapIdx, sig.samMethod, invokedDescriptorCtor);
        emitter.emitInvokedynamic(indyIdx, 0, true);
        break;
      }

      if (isClassRef && expr.target.kind === "ident") {
        implOwner = resolveClassName(ctx, expr.target.name);
        // Prefer static method reference
        const staticSig = findKnownMethodByArity(implOwner, expr.method, sig.params.length, true);
        if (staticSig) {
          implDescriptor = "(" + staticSig.paramTypes.map(typeToDescriptor).join("") + ")" + typeToDescriptor(staticSig.returnType);
          implRefKind = 6;
          implIsInterface = staticSig.isInterface ?? false;
        } else {
          // Unbound instance: first SAM arg is receiver
          const instSig = findKnownMethodByArity(implOwner, expr.method, Math.max(0, sig.params.length - 1), false);
          if (instSig) {
            implDescriptor = "(" + instSig.paramTypes.map(typeToDescriptor).join("") + ")" + typeToDescriptor(instSig.returnType);
            implRefKind = instSig.isInterface ? 9 : 5;
            implIsInterface = instSig.isInterface ?? false;
          } else if (implOwner === ctx.className) {
            const staticUser = ctx.allMethods.find(m => m.name === expr.method && m.isStatic && m.params.length === sig.params.length);
            if (staticUser) {
              implDescriptor = methodDescriptor(staticUser.params, staticUser.returnType);
              implRefKind = 6;
            } else {
              const instUser = ctx.allMethods.find(m => m.name === expr.method && !m.isStatic && m.params.length === Math.max(0, sig.params.length - 1));
              if (!instUser) throw new Error(`Cannot resolve method reference ${implOwner}::${expr.method}`);
              implDescriptor = methodDescriptor(instUser.params, instUser.returnType);
              implRefKind = 5;
            }
          } else {
            throw new Error(`Cannot resolve method reference ${implOwner}::${expr.method}`);
          }
        }
      } else {
        const t = inferType(ctx, expr.target);
        implOwner = t === "String" ? "java/lang/String"
          : (typeof t === "object" && "className" in t ? resolveClassName(ctx, t.className) : "java/lang/Object");
        captureTypes = [t === "String" ? "String" : t];
        compileExpr(ctx, emitter, expr.target);

        const boundSig = findKnownMethodByArity(implOwner, expr.method, sig.params.length, false);
        if (boundSig) {
          implDescriptor = "(" + boundSig.paramTypes.map(typeToDescriptor).join("") + ")" + typeToDescriptor(boundSig.returnType);
          implRefKind = boundSig.isInterface ? 9 : 5;
          implIsInterface = boundSig.isInterface ?? false;
        } else if (implOwner === ctx.className) {
          const m = ctx.allMethods.find(mm => mm.name === expr.method && !mm.isStatic && mm.params.length === sig.params.length);
          if (!m) throw new Error(`Cannot resolve method reference target::${expr.method}`);
          implDescriptor = methodDescriptor(m.params, m.returnType);
          implRefKind = 5;
        } else {
          throw new Error(`Cannot resolve method reference target::${expr.method}`);
        }
      }

      const invokedDescriptor = "(" + captureTypes.map(typeToDescriptor).join("") + ")" + typeToDescriptor(expectedType);
      const samDescriptor = "(" + sig.params.map(typeToDescriptor).join("") + ")" + typeToDescriptor(sig.returnType);
      ctx.lambdaBootstraps.push({
        samDescriptor,
        implOwner,
        implMethodName: implName,
        implDescriptor,
        implIsInterface,
        invokedName: sig.samMethod,
        invokedDescriptor,
        implRefKind,
      });
      const bootstrapIdx = ctx.lambdaBootstraps.length - 1;
      const indyIdx = ctx.cp.addInvokeDynamic(bootstrapIdx, sig.samMethod, invokedDescriptor);
      emitter.emitInvokedynamic(indyIdx, captureTypes.length, true);
      break;
    }
    case "classLit": {
      const className = resolveClassName(ctx, expr.className);
      const classIdx = ctx.cp.addClass(className);
      emitter.emitLdc(classIdx);
      break;
    }
    default:
      throw new Error(`Unsupported expression: ${(expr as Expr).kind}`);
  }
}

function compileStringConcat(ctx: CompileContext, emitter: BytecodeEmitter, expr: Expr & { kind: "binary" }): void {
  // Flatten the concatenation tree
  const parts: Expr[] = [];
  function flatten(e: Expr) {
    if (e.kind === "binary" && e.op === "+") {
      const lt = inferType(ctx, e.left);
      const rt = inferType(ctx, e.right);
      if (lt === "String" || rt === "String") {
        flatten(e.left);
        flatten(e.right);
        return;
      }
    }
    parts.push(e);
  }
  flatten(expr);

  // new StringBuilder()
  const sbClass = ctx.cp.addClass("java/lang/StringBuilder");
  emitter.emit(0xbb); emitter.emitU16(sbClass); // new
  emitter.emit(0x59); // dup
  const initRef = ctx.cp.addMethodref("java/lang/StringBuilder", "<init>", "()V");
  emitter.emitInvokespecial(initRef, 0, false);

  // .append() for each part
  for (const part of parts) {
    const partType = inferType(ctx, part);
    compileExpr(ctx, emitter, part);
    let appendDesc: string;
    if (partType === "int" || partType === "short" || partType === "byte") {
      appendDesc = "(I)Ljava/lang/StringBuilder;";
    } else if (partType === "long") {
      appendDesc = "(J)Ljava/lang/StringBuilder;";
    } else if (partType === "float") {
      appendDesc = "(F)Ljava/lang/StringBuilder;";
    } else if (partType === "double") {
      appendDesc = "(D)Ljava/lang/StringBuilder;";
    } else if (partType === "char") {
      appendDesc = "(C)Ljava/lang/StringBuilder;";
    } else if (partType === "boolean") {
      appendDesc = "(Z)Ljava/lang/StringBuilder;";
    } else if (partType === "String") {
      appendDesc = "(Ljava/lang/String;)Ljava/lang/StringBuilder;";
    } else {
      appendDesc = "(Ljava/lang/Object;)Ljava/lang/StringBuilder;";
    }
    const appendRef = ctx.cp.addMethodref("java/lang/StringBuilder", "append", appendDesc);
    emitter.emitInvokevirtual(appendRef, 1, true);
  }

  // .toString()
  const toStringRef = ctx.cp.addMethodref("java/lang/StringBuilder", "toString", "()Ljava/lang/String;");
  emitter.emitInvokevirtual(toStringRef, 0, true);
}

function compileCall(ctx: CompileContext, emitter: BytecodeEmitter, expr: Expr & { kind: "call" }): void {
  // Handle System.out.println specially
  if (expr.object?.kind === "fieldAccess" &&
      expr.object.object.kind === "ident" && expr.object.object.name === "System" &&
      expr.object.field === "out") {
    // getstatic System.out
    const fieldRef = ctx.cp.addFieldref("java/lang/System", "out", "Ljava/io/PrintStream;");
    emitter.emit(0xb2); // getstatic
    emitter.emitU16(fieldRef);

    const argType = expr.args.length > 0 ? inferType(ctx, expr.args[0]) : "void";
    for (const arg of expr.args) compileExpr(ctx, emitter, arg);

    let desc: string;
    if (argType === "int" || argType === "short" || argType === "byte") desc = "(I)V";
    else if (argType === "long") desc = "(J)V";
    else if (argType === "float") desc = "(F)V";
    else if (argType === "double") desc = "(D)V";
    else if (argType === "char") desc = "(C)V";
    else if (argType === "boolean") desc = "(Z)V";
    else if (argType === "String") desc = "(Ljava/lang/String;)V";
    else desc = "(Ljava/lang/Object;)V";
    const mRef = ctx.cp.addMethodref("java/io/PrintStream", expr.method, desc);
    emitter.emitInvokevirtual(mRef, expr.args.length, false);
    return;
  }

  if (expr.object) {
    const objType = inferType(ctx, expr.object);

    // Resolve to an ident-based static call: ClassName.method(...)
    if (expr.object.kind === "ident") {
      const name = expr.object.name;
      // Check if it's a class name (starts with uppercase or in importMap, and not a local var)
      if ((/^[A-Z]/.test(name) || ctx.importMap.has(name)) && !findLocal(ctx, name)) {
        const internalName = resolveClassName(ctx, name);
        const argTypes = expr.args.map(a => typeToDescriptor(inferType(ctx, a)));

        // Try known methods
        const exactSig = lookupKnownMethod(internalName, expr.method, argTypes.join(""));
        const sig = exactSig
          ?? (hasFunctionalArg(expr.args) ? findKnownMethodByArity(internalName, expr.method, expr.args.length, true) : undefined);
        if (sig) {
          expr.args.forEach((arg, i) => compileExpr(ctx, emitter, arg, sig.paramTypes[i] ?? { className: "java/lang/Object" }));
          const sigArgDescs = sig.paramTypes.map(t => typeToDescriptor(t)).join("");
          const desc = "(" + sigArgDescs + ")" + typeToDescriptor(sig.returnType);
          const mRef = ctx.cp.addMethodref(internalName, expr.method, desc);
          emitter.emitInvokestatic(mRef, expr.args.length, sig.returnType !== "void");
        } else {
          for (const arg of expr.args) compileExpr(ctx, emitter, arg);
          // User-defined static method in same or another class
          const userMethod = ctx.allMethods.find(m => m.name === expr.method && m.isStatic);
          const retType = userMethod ? userMethod.returnType : { className: "java/lang/Object" } as Type;
          const desc = "(" + argTypes.join("") + ")" + typeToDescriptor(retType);
          const mRef = ctx.cp.addMethodref(internalName, expr.method, desc);
          emitter.emitInvokestatic(mRef, expr.args.length, retType !== "void");
        }
        return;
      }
    }

    // Instance method call
    compileExpr(ctx, emitter, expr.object);
    const argTypes = expr.args.map(a => typeToDescriptor(inferType(ctx, a)));

    const rawOwner = objType === "String" ? "java/lang/String"
      : typeof objType === "object" && "className" in objType ? objType.className
      : "java/lang/Object";
    const ownerClass = resolveClassName(ctx, rawOwner);

    // Look up return type
    const exactSig = lookupKnownMethod(ownerClass, expr.method, argTypes.join(""));
    const sig = exactSig
      ?? (hasFunctionalArg(expr.args) ? findKnownMethodByArity(ownerClass, expr.method, expr.args.length, false) : undefined);

    let desc: string;
    let retType: Type;
    let isInterface = false;

    if (sig) {
      expr.args.forEach((arg, i) => compileExpr(ctx, emitter, arg, sig.paramTypes[i] ?? { className: "java/lang/Object" }));
      retType = sig.returnType;
      const sigArgDescs = sig.paramTypes.map(t => typeToDescriptor(t)).join("");
      desc = "(" + sigArgDescs + ")" + typeToDescriptor(retType);
      isInterface = sig.isInterface ?? false;
    } else {
      // Check user-defined methods
      const userMethod = ctx.allMethods.find(m => m.name === expr.method && !m.isStatic);
      if (userMethod) {
        expr.args.forEach((arg, i) => compileExpr(ctx, emitter, arg, userMethod.params[i]?.type));
      } else {
        for (const arg of expr.args) compileExpr(ctx, emitter, arg);
      }
      retType = userMethod ? userMethod.returnType : { className: "java/lang/Object" } as Type;
      desc = "(" + argTypes.join("") + ")" + typeToDescriptor(retType);
    }

    if (isInterface) {
      const mRef = ctx.cp.addInterfaceMethodref(ownerClass, expr.method, desc);
      emitter.emitInvokeinterface(mRef, expr.args.length, retType !== "void");
    } else {
      const mRef = ctx.cp.addMethodref(ownerClass, expr.method, desc);
      emitter.emitInvokevirtual(mRef, expr.args.length, retType !== "void");
    }
  } else {
    // Unqualified method call — call on this or static
    const userMethod = ctx.allMethods.find(m => m.name === expr.method);
    if (userMethod) {
      const desc = methodDescriptor(userMethod.params, userMethod.returnType);
      const mRef = ctx.cp.addMethodref(ctx.className, expr.method, desc);
      if (userMethod.isStatic) {
        expr.args.forEach((arg, i) => compileExpr(ctx, emitter, arg, userMethod.params[i]?.type));
        emitter.emitInvokestatic(mRef, expr.args.length, userMethod.returnType !== "void");
      } else {
        emitter.emitAload(0); // this
        expr.args.forEach((arg, i) => compileExpr(ctx, emitter, arg, userMethod.params[i]?.type));
        emitter.emitInvokevirtual(mRef, expr.args.length, userMethod.returnType !== "void");
      }
    } else if (ctx.staticWildcardImports.length > 0) {
      // Try static-import-on-demand owners in order
      const argTypes = expr.args.map(a => typeToDescriptor(inferType(ctx, a)));
      let ownerClass = ctx.staticWildcardImports[0];
      let sig: MethodSig | undefined;
      for (const owner of ctx.staticWildcardImports) {
        const exact = lookupKnownMethod(owner, expr.method, argTypes.join(""));
        const candidate = exact
          ?? (hasFunctionalArg(expr.args) ? findKnownMethodByArity(owner, expr.method, expr.args.length, true) : undefined);
        if (candidate) {
          ownerClass = owner;
          sig = candidate;
          break;
        }
      }
      if (sig) {
        expr.args.forEach((arg, i) => compileExpr(ctx, emitter, arg, sig!.paramTypes[i] ?? { className: "java/lang/Object" }));
      } else {
        for (const arg of expr.args) compileExpr(ctx, emitter, arg);
      }
      const retType: Type = sig?.returnType ?? { className: "java/lang/Object" };
      const sigArgDescs = sig ? sig.paramTypes.map(t => typeToDescriptor(t)).join("") : argTypes.join("");
      const desc = "(" + sigArgDescs + ")" + typeToDescriptor(retType);
      const mRef = ctx.cp.addMethodref(ownerClass, expr.method, desc);
      emitter.emitInvokestatic(mRef, expr.args.length, retType !== "void");
    }
  }
}

function compileFieldAccess(ctx: CompileContext, emitter: BytecodeEmitter, expr: Expr & { kind: "fieldAccess" }): void {
  // Check if the object is a class name (static field access)
  if (expr.object.kind === "ident") {
    const name = expr.object.name;
    const resolved = resolveClassName(ctx, name);
    // It's a static access if: not a local variable AND (starts uppercase OR is in importMap OR resolved differs)
    const isLocal = !!findLocal(ctx, name);
    const isClassRef = !isLocal && (/^[A-Z]/.test(name) || ctx.importMap.has(name) || resolved !== name);
    if (isClassRef) {
      let desc = "Ljava/lang/Object;";
      if (resolved === "java/lang/System" && expr.field === "out") desc = "Ljava/io/PrintStream;";
      const fieldRef = ctx.cp.addFieldref(resolved, expr.field, desc);
      emitter.emit(0xb2); // getstatic
      emitter.emitU16(fieldRef);
      return;
    }
  }

  // Chained field access for fully-qualified static: net.unit8.raoh.Path.ROOT
  // Represented as nested fieldAccess nodes; collapse to a single getstatic
  if (expr.object.kind === "fieldAccess") {
    // Try to collapse the chain into a class name + field
    function collapseChain(e: Expr): { className: string; field: string } | null {
      if (e.kind === "fieldAccess") {
        const inner = collapseChain(e.object);
        if (inner) return { className: inner.className + "/" + inner.field, field: e.field };
      }
      if (e.kind === "ident") return { className: e.name, field: "" };
      return null;
    }
    const chain = collapseChain(expr.object);
    if (chain) {
      const ownerClass = (chain.field ? chain.className + "/" + chain.field : chain.className).replace(/\./g, "/");
      let desc = "Ljava/lang/Object;";
      if (ownerClass === "java/lang/System" && expr.field === "out") desc = "Ljava/io/PrintStream;";
      const fieldRef = ctx.cp.addFieldref(ownerClass, expr.field, desc);
      emitter.emit(0xb2); // getstatic
      emitter.emitU16(fieldRef);
      return;
    }
  }

  // Array .length
  if (expr.field === "length") {
    const objType = inferType(ctx, expr.object);
    if (typeof objType === "object" && "array" in objType) {
      compileExpr(ctx, emitter, expr.object);
      emitter.emit(0xbe); // arraylength
      return;
    }
  }

  // Instance field
  compileExpr(ctx, emitter, expr.object);
  const objType = inferType(ctx, expr.object);
  const ownerClass = typeof objType === "object" && "className" in objType ? objType.className : ctx.className;
  const fld = ctx.fields.find(f => f.name === expr.field);
  const fieldType = fld ? typeToDescriptor(fld.type) : "Ljava/lang/Object;";
  const fieldRef = ctx.cp.addFieldref(ownerClass, expr.field, fieldType);
  emitter.emit(0xb4); // getfield
  emitter.emitU16(fieldRef);
}

function withScopedLocals(ctx: CompileContext, fn: () => void): void {
  const savedLen = ctx.locals.length;
  const savedNext = ctx.nextSlot;
  fn();
  ctx.locals.length = savedLen;
  ctx.nextSlot = savedNext;
}

function ensureAssignable(ctx: CompileContext, target: Type, value: Type, reason: string): void {
  if (!isAssignableInContext(ctx, target, value)) {
    throw new Error(`Type mismatch for ${reason}: cannot assign ${typeToDescriptor(value)} to ${typeToDescriptor(target)}`);
  }
}

function collectExprIdentifiers(expr: Expr, out: Set<string>): void {
  switch (expr.kind) {
    case "ident": out.add(expr.name); break;
    case "binary": collectExprIdentifiers(expr.left, out); collectExprIdentifiers(expr.right, out); break;
    case "unary": collectExprIdentifiers(expr.operand, out); break;
    case "call":
      if (expr.object) collectExprIdentifiers(expr.object, out);
      for (const a of expr.args) collectExprIdentifiers(a, out);
      break;
    case "staticCall": for (const a of expr.args) collectExprIdentifiers(a, out); break;
    case "fieldAccess": collectExprIdentifiers(expr.object, out); break;
    case "newExpr": for (const a of expr.args) collectExprIdentifiers(a, out); break;
    case "cast": collectExprIdentifiers(expr.expr, out); break;
    case "postIncrement": collectExprIdentifiers(expr.operand, out); break;
    case "preIncrement": collectExprIdentifiers(expr.operand, out); break;
    case "instanceof": collectExprIdentifiers(expr.expr, out); break;
    case "arrayAccess": collectExprIdentifiers(expr.array, out); collectExprIdentifiers(expr.index, out); break;
    case "arrayLit": for (const e of expr.elements) collectExprIdentifiers(e, out); break;
    case "newArray": collectExprIdentifiers(expr.size, out); break;
    case "superCall": for (const a of expr.args) collectExprIdentifiers(a, out); break;
    case "ternary":
      collectExprIdentifiers(expr.cond, out);
      collectExprIdentifiers(expr.thenExpr, out);
      collectExprIdentifiers(expr.elseExpr, out);
      break;
    case "switchExpr":
      collectExprIdentifiers(expr.selector, out);
      for (const c of expr.cases) {
        if (c.guard) collectExprIdentifiers(c.guard, out);
        if (c.expr) collectExprIdentifiers(c.expr, out);
        if (c.stmts) for (const s of c.stmts) collectStmtIdentifiers(s, out);
      }
      break;
    case "lambda":
      // Nested lambdas are treated independently.
      break;
    case "methodRef":
      collectExprIdentifiers(expr.target, out);
      break;
    case "classLit":
      break;
    default:
      break;
  }
}

function collectStmtIdentifiers(stmt: Stmt, out: Set<string>): void {
  switch (stmt.kind) {
    case "varDecl": if (stmt.init) collectExprIdentifiers(stmt.init, out); break;
    case "assign": collectExprIdentifiers(stmt.target, out); collectExprIdentifiers(stmt.value, out); break;
    case "exprStmt": collectExprIdentifiers(stmt.expr, out); break;
    case "return": if (stmt.value) collectExprIdentifiers(stmt.value, out); break;
    case "yield": collectExprIdentifiers(stmt.value, out); break;
    case "if":
      collectExprIdentifiers(stmt.cond, out);
      for (const s of stmt.then) collectStmtIdentifiers(s, out);
      if (stmt.else_) for (const s of stmt.else_) collectStmtIdentifiers(s, out);
      break;
    case "while":
      collectExprIdentifiers(stmt.cond, out);
      for (const s of stmt.body) collectStmtIdentifiers(s, out);
      break;
    case "for":
      if (stmt.init) collectStmtIdentifiers(stmt.init, out);
      if (stmt.cond) collectExprIdentifiers(stmt.cond, out);
      if (stmt.update) collectStmtIdentifiers(stmt.update, out);
      for (const s of stmt.body) collectStmtIdentifiers(s, out);
      break;
    case "switch":
      collectExprIdentifiers(stmt.selector, out);
      for (const c of stmt.cases) {
        if (c.guard) collectExprIdentifiers(c.guard, out);
        if (c.expr) collectExprIdentifiers(c.expr, out);
        if (c.stmts) for (const s of c.stmts) collectStmtIdentifiers(s, out);
      }
      break;
    case "doWhile":
      collectExprIdentifiers(stmt.cond, out);
      for (const s of stmt.body) collectStmtIdentifiers(s, out);
      break;
    case "forEach":
      collectExprIdentifiers(stmt.iterable, out);
      for (const s of stmt.body) collectStmtIdentifiers(s, out);
      break;
    case "throw":
      collectExprIdentifiers(stmt.expr, out);
      break;
    case "tryCatch":
      for (const s of stmt.tryBody) collectStmtIdentifiers(s, out);
      for (const c of stmt.catches) for (const s of c.body) collectStmtIdentifiers(s, out);
      if (stmt.finallyBody) for (const s of stmt.finallyBody) collectStmtIdentifiers(s, out);
      break;
    case "break":
    case "continue":
      break;
    case "labeled":
      collectStmtIdentifiers(stmt.stmt, out);
      break;
    case "block":
      for (const s of stmt.stmts) collectStmtIdentifiers(s, out);
      break;
  }
}

function descriptorToType(desc: string): Type {
  if (desc === "I") return "int";
  if (desc === "J") return "long";
  if (desc === "S") return "short";
  if (desc === "B") return "byte";
  if (desc === "C") return "char";
  if (desc === "F") return "float";
  if (desc === "D") return "double";
  if (desc === "Z") return "boolean";
  if (desc === "V") return "void";
  if (desc.startsWith("L") && desc.endsWith(";")) {
    const cls = desc.slice(1, -1);
    if (cls === "java/lang/String") return "String";
    return { className: cls };
  }
  if (desc.startsWith("[")) return { array: descriptorToType(desc.slice(1)) };
  return { className: "java/lang/Object" };
}

function functionalSigForType(ctx: CompileContext, t: Type): { ifaceName: string; sig: FunctionalSig } {
  if (!(typeof t === "object" && "className" in t)) {
    throw new Error("Lambda target type must be a functional interface");
  }
  const ifaceName = resolveClassName(ctx, t.className);
  const sig = FUNCTIONAL_IFACES[ifaceName];
  if (!sig) throw new Error(`Unsupported functional interface for lambda: ${ifaceName}`);
  return { ifaceName, sig };
}

const BUILTIN_SUPERS: Record<string, string> = {
  "java/lang/String": "java/lang/Object",
  "java/lang/Integer": "java/lang/Object",
  "java/lang/StringBuilder": "java/lang/Object",
  "java/util/ArrayList": "java/lang/Object",
  "java/io/PrintStream": "java/lang/Object",
};

function toInternalClassName(ctx: CompileContext, t: Type): string | undefined {
  if (t === "String") return "java/lang/String";
  if (typeof t === "object" && "className" in t) return resolveClassName(ctx, t.className);
  return undefined;
}

function isClassSupertype(ctx: CompileContext, maybeSuper: string, maybeSub: string): boolean {
  if (maybeSuper === maybeSub) return true;
  let cur = maybeSub;
  const seen = new Set<string>();
  while (!seen.has(cur)) {
    seen.add(cur);
    const next = ctx.classSupers.get(cur) ?? BUILTIN_SUPERS[cur];
    if (!next) return false;
    if (next === maybeSuper) return true;
    cur = next;
  }
  return false;
}

function isPatternTotalForSelector(ctx: CompileContext, selectorType: Type, patternTypeName: string): boolean {
  const selectorClass = toInternalClassName(ctx, selectorType);
  if (!selectorClass) return false;
  const patternClass = resolveClassName(ctx, patternTypeName);
  return isClassSupertype(ctx, patternClass, selectorClass);
}

function validateSwitchSemanticsCompile(ctx: CompileContext, selectorType: Type, cases: SwitchCase[], isExpr: boolean): void {
  let seenTotalNonNullPattern = false;
  let seenNullCase = false;
  const unguardedPatterns: string[] = [];

  for (const c of cases) {
    const hasGuard = !!c.guard;
    for (const l of c.labels) {
      if (l.kind === "bool" && selectorType !== "boolean") {
        throw new Error("boolean case label requires boolean switch selector");
      }
      if (l.kind === "int" && selectorType !== "int") {
        throw new Error("int case label requires int switch selector");
      }
      if (l.kind === "null" && !isRefType(selectorType)) {
        throw new Error("null case label requires reference switch selector");
      }
      if (l.kind === "string" && !isRefType(selectorType)) {
        throw new Error("String case label requires reference switch selector");
      }
      if ((l.kind === "typePattern" || l.kind === "recordPattern") && !isRefType(selectorType)) {
        throw new Error("type pattern case requires reference switch selector");
      }
      if (l.kind === "null") {
        seenNullCase = true;
        if (seenTotalNonNullPattern) {
          // Non-null total patterns do not dominate null.
        }
      } else {
        if (seenTotalNonNullPattern) {
          throw new Error("switch label is dominated by previous total type pattern");
        }
      }
      if (l.kind === "typePattern" || l.kind === "recordPattern") {
        const pat = resolveClassName(ctx, l.typeName);
        if (!hasGuard) {
          for (const prev of unguardedPatterns) {
            if (isClassSupertype(ctx, prev, pat)) {
              throw new Error(`dominated switch label pattern: ${"typeName" in l ? l.typeName : pat}`);
            }
          }
          unguardedPatterns.push(pat);
          if (isPatternTotalForSelector(ctx, selectorType, "typeName" in l ? l.typeName : pat)) {
            seenTotalNonNullPattern = true;
          }
        }
      }
    }
    if (c.guard && inferType(ctx, c.guard) !== "boolean") {
      throw new Error("switch guard must be boolean");
    }
  }

  if (isExpr) {
    const hasUnguardedDefault = cases.some(c => !c.guard && c.labels.some(l => l.kind === "default"));
    if (hasUnguardedDefault) return;
    const hasTrue = cases.some(c => !c.guard && c.labels.some(l => l.kind === "bool" && l.value));
    const hasFalse = cases.some(c => !c.guard && c.labels.some(l => l.kind === "bool" && !l.value));
    const exhaustiveBoolean = selectorType === "boolean" && hasTrue && hasFalse;
    const exhaustiveRef = isRefType(selectorType) && seenNullCase && seenTotalNonNullPattern;
    if (!exhaustiveBoolean && !exhaustiveRef) {
      throw new Error("switch expression is not exhaustive: provide default or exhaustive labels");
    }
  }
}

function resolveClassDecl(ctx: CompileContext, typeName: string): ClassDecl | undefined {
  const internal = resolveClassName(ctx, typeName);
  return ctx.classDecls.get(internal) ?? ctx.classDecls.get(typeName);
}

function isIntLike(t: Type): boolean {
  return t === "int" || t === "short" || t === "byte" || t === "char" || t === "boolean";
}

function emitWideningConversion(emitter: BytecodeEmitter, from: Type, to: Type): void {
  if (sameType(from, to)) return;
  if (isIntLike(from) && isIntLike(to)) return; // all stored as int on JVM
  if (isIntLike(from) && to === "long") { emitter.emit(0x85); return; } // i2l
  if (isIntLike(from) && to === "float") { emitter.emit(0x86); return; } // i2f
  if (isIntLike(from) && to === "double") { emitter.emit(0x87); return; } // i2d
  if (from === "long" && to === "float") { emitter.emit(0x89); return; } // l2f
  if (from === "long" && to === "double") { emitter.emit(0x8a); return; } // l2d
  if (from === "float" && to === "double") { emitter.emit(0x8d); return; } // f2d
}

function emitNarrowingConversion(emitter: BytecodeEmitter, from: Type, to: Type): void {
  if (sameType(from, to)) return;
  // Narrowing from int-like to sub-int types
  if (isIntLike(from) && to === "byte") { emitter.emit(0x91); return; } // i2b
  if (isIntLike(from) && to === "char") { emitter.emit(0x92); return; } // i2c
  if (isIntLike(from) && to === "short") { emitter.emit(0x93); return; } // i2s
  // Long narrowing
  if (from === "long" && isIntLike(to)) { emitter.emit(0x88); return; } // l2i
  if (from === "long" && to === "float") { emitter.emit(0x89); return; } // l2f (widening, but used in cast)
  if (from === "long" && to === "double") { emitter.emit(0x8a); return; } // l2d
  // Float narrowing
  if (from === "float" && isIntLike(to)) { emitter.emit(0x8b); return; } // f2i
  if (from === "float" && to === "long") { emitter.emit(0x8c); return; } // f2l
  // Double narrowing
  if (from === "double" && isIntLike(to)) { emitter.emit(0x8e); return; } // d2i
  if (from === "double" && to === "long") { emitter.emit(0x8f); return; } // d2l
  if (from === "double" && to === "float") { emitter.emit(0x90); return; } // d2f
}

function emitStoreLocalByType(emitter: BytecodeEmitter, slot: number, t: Type): void {
  if (t === "long") emitter.emitLstore(slot);
  else if (t === "float") emitter.emitFstore(slot);
  else if (t === "double") emitter.emitDstore(slot);
  else if (t === "int" || t === "boolean" || t === "short" || t === "byte" || t === "char") emitter.emitIstore(slot);
  else emitter.emitAstore(slot);
}

function emitLoadLocalByType(emitter: BytecodeEmitter, slot: number, t: Type): void {
  if (t === "long") emitter.emitLload(slot);
  else if (t === "float") emitter.emitFload(slot);
  else if (t === "double") emitter.emitDload(slot);
  else if (t === "int" || t === "boolean" || t === "short" || t === "byte" || t === "char") emitter.emitIload(slot);
  else emitter.emitAload(slot);
}

function bindPatternLabelLocals(
  ctx: CompileContext,
  emitter: BytecodeEmitter,
  selectorSlot: number,
  selectorType: Type,
  label: SwitchLabel,
): void {
  if (label.kind !== "typePattern" && label.kind !== "recordPattern") {
    throw new Error("internal: expected pattern label");
  }
  emitLoadLocalByType(emitter, selectorSlot, selectorType);
  const checkClass = resolveClassName(ctx, label.typeName);
  const classIdx = ctx.cp.addClass(checkClass);
  emitter.emit(0xc0); emitter.emitU16(classIdx); // checkcast

  if (label.kind === "typePattern") {
    const slot = addLocal(ctx, label.bindVar, { className: checkClass });
    if (emitter.maxLocals <= slot) emitter.maxLocals = slot + 1;
    emitter.emitAstore(slot);
    return;
  }

  const recordDecl = resolveClassDecl(ctx, label.typeName);
  if (!recordDecl?.isRecord || !recordDecl.recordComponents) {
    throw new Error(`record pattern requires known record declaration: ${label.typeName}`);
  }
  if (recordDecl.recordComponents.length !== label.bindVars.length) {
    throw new Error(`record pattern arity mismatch for ${label.typeName}`);
  }
  const recSlot = addLocal(ctx, "$rec_pat", { className: checkClass });
  if (emitter.maxLocals <= recSlot) emitter.maxLocals = recSlot + 1;
  emitter.emitAstore(recSlot);

  for (let i = 0; i < label.bindVars.length; i++) {
    const c = recordDecl.recordComponents[i];
    emitter.emitAload(recSlot);
    const mRef = ctx.cp.addMethodref(checkClass, c.name, "()" + typeToDescriptor(c.type));
    emitter.emitInvokevirtual(mRef, 0, true);
    const slot = addLocal(ctx, label.bindVars[i], c.type);
    if (emitter.maxLocals <= slot) emitter.maxLocals = slot + 1;
    emitStoreLocalByType(emitter, slot, c.type);
  }
}

function emitSwitchLabelMatch(
  ctx: CompileContext,
  emitter: BytecodeEmitter,
  selectorSlot: number,
  selectorType: Type,
  label: SwitchLabel,
): number {
  if (label.kind === "default") return emitter.emitBranch(0xa7); // goto
  if (label.kind === "bool") {
    if (selectorType !== "boolean") {
      throw new Error("boolean case label requires boolean switch selector");
    }
    emitter.emitIload(selectorSlot);
    emitter.emitIconst(label.value ? 1 : 0);
    return emitter.emitBranch(0x9f); // if_icmpeq
  }
  if (label.kind === "int") {
    if (selectorType !== "int") {
      throw new Error("int case label requires int switch selector");
    }
    emitter.emitIload(selectorSlot);
    if (!emitter.emitIconst(label.value)) {
      emitter.emitLdc(ctx.cp.addInteger(label.value));
    }
    return emitter.emitBranch(0x9f); // if_icmpeq
  }
  if (label.kind === "null") {
    if (!isRefType(selectorType)) throw new Error("null case label requires reference switch selector");
    emitter.emitAload(selectorSlot);
    return emitter.emitBranch(0xc6); // ifnull
  }
  if (label.kind === "string") {
    if (selectorType !== "String" && !(typeof selectorType === "object" && "className" in selectorType)) {
      throw new Error("String case label requires reference switch selector");
    }
    emitter.emitAload(selectorSlot);
    const patchNull = emitter.emitBranch(0xc6); // ifnull -> skip this label
    emitter.emitAload(selectorSlot);
    emitter.emitLdc(ctx.cp.addString(label.value));
    const equalsRef = ctx.cp.addMethodref("java/lang/String", "equals", "(Ljava/lang/Object;)Z");
    emitter.emitInvokevirtual(equalsRef, 1, true);
    const patchMatch = emitter.emitBranch(0x9a); // ifne
    emitter.patchBranch(patchNull, emitter.pc);
    return patchMatch;
  }
  // type/record pattern
  if (!isRefType(selectorType)) throw new Error("type pattern case requires reference switch selector");
  emitter.emitAload(selectorSlot);
  const checkClass = resolveClassName(ctx, label.typeName);
  const classIdx = ctx.cp.addClass(checkClass);
  emitter.emit(0xc1); // instanceof
  emitter.emitU16(classIdx);
  return emitter.emitBranch(0x9a); // ifne
}

function compileSwitchCaseStmts(ctx: CompileContext, emitter: BytecodeEmitter, c: SwitchCase): void {
  if (c.expr) {
    compileExpr(ctx, emitter, c.expr);
    emitter.emit(0x57); // pop
    return;
  }
  for (const s of c.stmts ?? []) compileStmt(ctx, emitter, s);
}

function compileSwitchStmt(ctx: CompileContext, emitter: BytecodeEmitter, stmt: Extract<Stmt, { kind: "switch" }>): void {
  withScopedLocals(ctx, () => {
    const selectorType = inferType(ctx, stmt.selector);
    validateSwitchSemanticsCompile(ctx, selectorType, stmt.cases, false);
    const selectorSlot = addLocal(ctx, "$switch_sel", selectorType);
    if (emitter.maxLocals <= selectorSlot) emitter.maxLocals = selectorSlot + 1;
    if (selectorType === "int" || selectorType === "boolean") {
      compileExpr(ctx, emitter, stmt.selector, selectorType);
      emitter.emitIstore(selectorSlot);
    } else {
      compileExpr(ctx, emitter, stmt.selector, selectorType);
      emitter.emitAstore(selectorSlot);
    }

    const endPatches: number[] = [];
    for (const c of stmt.cases) {
      const matches = c.labels.map(l => ({ label: l, patch: emitSwitchLabelMatch(ctx, emitter, selectorSlot, selectorType, l) }));
      const patchNext = emitter.emitBranch(0xa7); // no match -> next case checks
      const bodyStart = emitter.pc;
      for (const m of matches) emitter.patchBranch(m.patch, bodyStart);
      withScopedLocals(ctx, () => {
        const patternLabel = c.labels.find(l => l.kind === "typePattern" || l.kind === "recordPattern");
        if (patternLabel) {
          bindPatternLabelLocals(ctx, emitter, selectorSlot, selectorType, patternLabel);
        }
        if (c.guard) {
          if (inferType(ctx, c.guard) !== "boolean") {
            throw new Error("switch guard must be boolean");
          }
          compileExpr(ctx, emitter, c.guard, "boolean");
          const guardFail = emitter.emitBranch(0x99); // ifeq
          compileSwitchCaseStmts(ctx, emitter, c);
          endPatches.push(emitter.emitBranch(0xa7));
          emitter.patchBranch(guardFail, emitter.pc);
        } else {
          compileSwitchCaseStmts(ctx, emitter, c);
          endPatches.push(emitter.emitBranch(0xa7));
        }
      });
      emitter.patchBranch(patchNext, emitter.pc);
    }
    for (const p of endPatches) emitter.patchBranch(p, emitter.pc);
  });
}

function compileSwitchExpr(ctx: CompileContext, emitter: BytecodeEmitter, expr: Extract<Expr, { kind: "switchExpr" }>, expectedType?: Type): void {
  const resultType = expectedType ?? inferType(ctx, expr);
  withScopedLocals(ctx, () => {
    const selectorType = inferType(ctx, expr.selector);
    validateSwitchSemanticsCompile(ctx, selectorType, expr.cases, true);
    const selectorSlot = addLocal(ctx, "$switch_expr_sel", selectorType);
    if (emitter.maxLocals <= selectorSlot) emitter.maxLocals = selectorSlot + 1;
    if (selectorType === "int" || selectorType === "boolean") {
      compileExpr(ctx, emitter, expr.selector, selectorType);
      emitter.emitIstore(selectorSlot);
    } else {
      compileExpr(ctx, emitter, expr.selector, selectorType);
      emitter.emitAstore(selectorSlot);
    }

    const endPatches: number[] = [];
    for (const c of expr.cases) {
      const matches = c.labels.map(l => ({ label: l, patch: emitSwitchLabelMatch(ctx, emitter, selectorSlot, selectorType, l) }));
      const patchNext = emitter.emitBranch(0xa7); // no match -> next checks
      const bodyStart = emitter.pc;
      for (const m of matches) emitter.patchBranch(m.patch, bodyStart);
      withScopedLocals(ctx, () => {
        const patternLabel = c.labels.find(l => l.kind === "typePattern" || l.kind === "recordPattern");
        if (patternLabel) {
          bindPatternLabelLocals(ctx, emitter, selectorSlot, selectorType, patternLabel);
        }
        if (c.guard) {
          if (inferType(ctx, c.guard) !== "boolean") {
            throw new Error("switch guard must be boolean");
          }
          compileExpr(ctx, emitter, c.guard, "boolean");
          const guardFail = emitter.emitBranch(0x99); // ifeq
          if (c.expr) {
            compileExpr(ctx, emitter, c.expr, resultType);
            endPatches.push(emitter.emitBranch(0xa7));
          } else {
            let yielded = false;
            for (const s of c.stmts ?? []) {
              if (s.kind === "yield") {
                compileExpr(ctx, emitter, s.value, resultType);
                endPatches.push(emitter.emitBranch(0xa7));
                yielded = true;
                break;
              }
              compileStmt(ctx, emitter, s);
            }
            if (!yielded) throw new Error("switch expression block must yield a value");
          }
          emitter.patchBranch(guardFail, emitter.pc);
        } else if (c.expr) {
          compileExpr(ctx, emitter, c.expr, resultType);
          endPatches.push(emitter.emitBranch(0xa7));
        } else {
          let yielded = false;
          for (const s of c.stmts ?? []) {
            if (s.kind === "yield") {
              compileExpr(ctx, emitter, s.value, resultType);
              endPatches.push(emitter.emitBranch(0xa7));
              yielded = true;
              break;
            }
            compileStmt(ctx, emitter, s);
          }
          if (!yielded) throw new Error("switch expression block must yield a value");
        }
      });
      emitter.patchBranch(patchNext, emitter.pc);
    }
    if (endPatches.length === 0) throw new Error("switch expression has no producible branch");
    for (const p of endPatches) emitter.patchBranch(p, emitter.pc);
  });
}

function compileStmt(ctx: CompileContext, emitter: BytecodeEmitter, stmt: Stmt): void {
  switch (stmt.kind) {
    case "varDecl": {
      const slot = addLocal(ctx, stmt.name, stmt.type);
      if (emitter.maxLocals <= slot) emitter.maxLocals = slot + 1;
      if (stmt.init) {
        // If we have a { ... } array literal, patch the elemType from the declared type
        let init = stmt.init;
        if (init.kind === "arrayLit" && typeof stmt.type === "object" && "array" in stmt.type) {
          init = { ...init, elemType: stmt.type.array };
        }
        const initType = inferType(ctx, init);
        ensureAssignable(ctx, stmt.type, initType, `local '${stmt.name}'`);
        compileExpr(ctx, emitter, init, stmt.type);
        emitWideningConversion(emitter, initType, stmt.type);
        emitStoreLocalByType(emitter, slot, stmt.type);
      }
      break;
    }
    case "assign": {
      if (stmt.target.kind === "ident") {
        const loc = findLocal(ctx, stmt.target.name);
        if (loc) {
          const valType = inferType(ctx, stmt.value);
          ensureAssignable(ctx, loc.type, valType, `local '${stmt.target.name}'`);
          compileExpr(ctx, emitter, stmt.value, loc.type);
          emitWideningConversion(emitter, valType, loc.type);
          emitStoreLocalByType(emitter, loc.slot, loc.type);
        } else {
          // Field assignment
          const field = ctx.fields.find(f => f.name === stmt.target.name);
          if (field) {
            ensureAssignable(ctx, field.type, inferType(ctx, stmt.value), `field '${stmt.target.name}'`);
            if (field.isStatic) {
              compileExpr(ctx, emitter, stmt.value, field.type);
              const fRef = ctx.cp.addFieldref(ctx.className, field.name, typeToDescriptor(field.type));
              emitter.emit(0xb3); // putstatic
              emitter.emitU16(fRef);
            } else {
              emitter.emitAload(0); // this
              compileExpr(ctx, emitter, stmt.value, field.type);
              const fRef = ctx.cp.addFieldref(ctx.className, field.name, typeToDescriptor(field.type));
              emitter.emit(0xb5); // putfield
              emitter.emitU16(fRef);
            }
          }
        }
      } else if (stmt.target.kind === "fieldAccess") {
        compileExpr(ctx, emitter, stmt.target.object);
        const targetType = inferType(ctx, stmt.target);
        ensureAssignable(ctx, targetType, inferType(ctx, stmt.value), `field '${stmt.target.field}'`);
        compileExpr(ctx, emitter, stmt.value, targetType);
        const objType = inferType(ctx, stmt.target.object);
        const ownerClass = typeof objType === "object" && "className" in objType ? objType.className : ctx.className;
        const fld = ctx.fields.find(f => f.name === stmt.target.field);
        const fieldType = fld ? typeToDescriptor(fld.type) : typeToDescriptor(inferType(ctx, stmt.value));
        const fieldRef = ctx.cp.addFieldref(ownerClass, stmt.target.field, fieldType);
        emitter.emit(0xb5); // putfield
        emitter.emitU16(fieldRef);
      } else if (stmt.target.kind === "arrayAccess") {
        compileExpr(ctx, emitter, stmt.target.array);
        compileExpr(ctx, emitter, stmt.target.index);
        const elemType = inferType(ctx, stmt.target);
        compileExpr(ctx, emitter, stmt.value, elemType);
        if (elemType === "int" || elemType === "boolean") {
          emitter.emit(0x4f); // iastore
        } else {
          emitter.emit(0x53); // aastore
        }
      }
      break;
    }
    case "exprStmt": {
      compileExpr(ctx, emitter, stmt.expr);
      // Pop result if non-void
      const exprType = inferType(ctx, stmt.expr);
      if (exprType !== "void") {
        emitter.emit(0x57); // pop
      }
      break;
    }
    case "return": {
      if (stmt.value) {
        const retValType = inferType(ctx, stmt.value);
        const returnNeedsBoxing = isRefType(ctx.method.returnType) && isPrimitiveType(retValType);
        if (!returnNeedsBoxing) {
          ensureAssignable(ctx, ctx.method.returnType, retValType, `return in ${ctx.method.name}`);
        }
        compileExpr(ctx, emitter, stmt.value, ctx.method.returnType);
        if (returnNeedsBoxing) {
          const info = BOX_INFO[retValType as string];
          if (!info) {
            throw new Error(`Type mismatch for return in ${ctx.method.name}: cannot assign ${typeToDescriptor(retValType)} to ${typeToDescriptor(ctx.method.returnType)}`);
          }
          const methodRef = ctx.cp.addMethodref(info.wrapper, "valueOf", info.desc);
          emitter.emit(0xb8); // invokestatic
          emitter.emitU16(methodRef);
        }
        emitWideningConversion(emitter, retValType, ctx.method.returnType);
      }
      emitter.emitReturn(ctx.method.returnType);
      break;
    }
    case "yield": {
      throw new Error("yield statement is only allowed in switch expressions");
    }
    case "if": {
      if (inferType(ctx, stmt.cond) !== "boolean") throw new Error("if condition must be boolean");
      compileExpr(ctx, emitter, stmt.cond);
      const patchElse = emitter.emitBranch(0x99); // ifeq (jump if false)
      // If condition is instanceof with a pattern variable, bind it at the start of then-branch
      withScopedLocals(ctx, () => {
        if (stmt.cond.kind === "instanceof" && (stmt.cond.bindVar || stmt.cond.recordBindVars)) {
          const checkClass = resolveClassName(ctx, stmt.cond.checkType);
          // Re-load the source expression and cast it to the pattern type
          compileExpr(ctx, emitter, stmt.cond.expr);
          const classIdx = ctx.cp.addClass(checkClass);
          emitter.emit(0xc0); emitter.emitU16(classIdx); // checkcast
          if (stmt.cond.bindVar) {
            const slot = addLocal(ctx, stmt.cond.bindVar, { className: checkClass });
            if (emitter.maxLocals <= slot) emitter.maxLocals = slot + 1;
            emitter.emitAstore(slot);
          } else {
            const recordDecl = resolveClassDecl(ctx, stmt.cond.checkType);
            if (!recordDecl?.isRecord || !recordDecl.recordComponents) {
              throw new Error(`record pattern requires known record declaration: ${stmt.cond.checkType}`);
            }
            const bindVars = stmt.cond.recordBindVars ?? [];
            if (bindVars.length !== recordDecl.recordComponents.length) {
              throw new Error(`record pattern arity mismatch for ${stmt.cond.checkType}`);
            }
            const recSlot = addLocal(ctx, "$if_rec_pat", { className: checkClass });
            if (emitter.maxLocals <= recSlot) emitter.maxLocals = recSlot + 1;
            emitter.emitAstore(recSlot);
            for (let i = 0; i < bindVars.length; i++) {
              const c = recordDecl.recordComponents[i];
              emitter.emitAload(recSlot);
              const mRef = ctx.cp.addMethodref(checkClass, c.name, "()" + typeToDescriptor(c.type));
              emitter.emitInvokevirtual(mRef, 0, true);
              const slot = addLocal(ctx, bindVars[i], c.type);
              if (emitter.maxLocals <= slot) emitter.maxLocals = slot + 1;
              emitStoreLocalByType(emitter, slot, c.type);
            }
          }
        }
        for (const s of stmt.then) compileStmt(ctx, emitter, s);
      });
      if (stmt.else_) {
        const patchEnd = emitter.emitBranch(0xa7); // goto
        emitter.patchBranch(patchElse, emitter.pc);
        withScopedLocals(ctx, () => {
          for (const s of stmt.else_!) compileStmt(ctx, emitter, s);
        });
        emitter.patchBranch(patchEnd, emitter.pc);
      } else {
        emitter.patchBranch(patchElse, emitter.pc);
      }
      break;
    }
    case "while": {
      if (inferType(ctx, stmt.cond) !== "boolean") throw new Error("while condition must be boolean");
      const breakInfo = { label: undefined as string | undefined, patches: [] as number[] };
      const continueInfo = { label: undefined as string | undefined, targets: [] as number[] };
      ctx.breakPatches.push(breakInfo);
      ctx.continuePatches.push(continueInfo);
      const loopStart = emitter.pc;
      compileExpr(ctx, emitter, stmt.cond);
      const patchExit = emitter.emitBranch(0x99); // ifeq
      withScopedLocals(ctx, () => {
        for (const s of stmt.body) compileStmt(ctx, emitter, s);
      });
      const continueTarget = emitter.pc;
      // goto loopStart
      const gotoOp = emitter.emitBranch(0xa7);
      emitter.patchBranch(gotoOp, loopStart);
      emitter.patchBranch(patchExit, emitter.pc);
      ctx.breakPatches.pop();
      ctx.continuePatches.pop();
      for (const p of breakInfo.patches) emitter.patchBranch(p, emitter.pc);
      for (const p of continueInfo.targets) emitter.patchBranch(p, continueTarget);
      break;
    }
    case "for": {
      withScopedLocals(ctx, () => {
        if (stmt.init) compileStmt(ctx, emitter, stmt.init);
        const breakInfo = { label: undefined as string | undefined, patches: [] as number[] };
        const continueInfo = { label: undefined as string | undefined, targets: [] as number[] };
        ctx.breakPatches.push(breakInfo);
        ctx.continuePatches.push(continueInfo);
        const loopStart = emitter.pc;
        let patchExit = -1;
        if (stmt.cond) {
          if (inferType(ctx, stmt.cond) !== "boolean") throw new Error("for condition must be boolean");
          compileExpr(ctx, emitter, stmt.cond);
          patchExit = emitter.emitBranch(0x99); // ifeq
        }
        withScopedLocals(ctx, () => {
          for (const s of stmt.body) compileStmt(ctx, emitter, s);
        });
        const continueTarget = emitter.pc;
        if (stmt.update) compileStmt(ctx, emitter, stmt.update);
        const gotoOp = emitter.emitBranch(0xa7);
        emitter.patchBranch(gotoOp, loopStart);
        if (patchExit >= 0) emitter.patchBranch(patchExit, emitter.pc);
        ctx.breakPatches.pop();
        ctx.continuePatches.pop();
        for (const p of breakInfo.patches) emitter.patchBranch(p, emitter.pc);
        for (const p of continueInfo.targets) emitter.patchBranch(p, continueTarget);
      });
      break;
    }
    case "switch": {
      compileSwitchStmt(ctx, emitter, stmt);
      break;
    }
    case "doWhile": {
      if (inferType(ctx, stmt.cond) !== "boolean") throw new Error("do-while condition must be boolean");
      const breakInfo = { label: undefined as string | undefined, patches: [] as number[] };
      const continueInfo = { label: undefined as string | undefined, targets: [] as number[] };
      ctx.breakPatches.push(breakInfo);
      ctx.continuePatches.push(continueInfo);
      const loopStart = emitter.pc;
      withScopedLocals(ctx, () => {
        for (const s of stmt.body) compileStmt(ctx, emitter, s);
      });
      const continueTarget = emitter.pc;
      compileExpr(ctx, emitter, stmt.cond);
      // ifne → loopStart (jump if true)
      const gotoOp = emitter.emitBranch(0x9a); // ifne
      emitter.patchBranch(gotoOp, loopStart);
      ctx.breakPatches.pop();
      ctx.continuePatches.pop();
      for (const p of breakInfo.patches) emitter.patchBranch(p, emitter.pc);
      for (const p of continueInfo.targets) emitter.patchBranch(p, continueTarget);
      break;
    }
    case "forEach": {
      withScopedLocals(ctx, () => {
        const iterableType = inferType(ctx, stmt.iterable);
        const breakInfo = { label: undefined as string | undefined, patches: [] as number[] };
        const continueInfo = { label: undefined as string | undefined, targets: [] as number[] };
        ctx.breakPatches.push(breakInfo);
        ctx.continuePatches.push(continueInfo);
        if (typeof iterableType === "object" && "array" in iterableType) {
          // Array iteration: for (T x : arr) → int $i=0; while($i < arr.length) { T x = arr[$i]; ... $i++; }
          compileExpr(ctx, emitter, stmt.iterable);
          const arrSlot = addLocal(ctx, "$forEach_arr", iterableType);
          if (emitter.maxLocals <= arrSlot) emitter.maxLocals = arrSlot + 1;
          emitter.emitAstore(arrSlot);
          const idxSlot = addLocal(ctx, "$forEach_idx", "int");
          if (emitter.maxLocals <= idxSlot) emitter.maxLocals = idxSlot + 1;
          emitter.emitIconst(0);
          emitter.emitIstore(idxSlot);
          const loopStart = emitter.pc;
          // $i < arr.length
          emitter.emitIload(idxSlot);
          emitter.emitAload(arrSlot);
          emitter.emit(0xbe); // arraylength
          emitter.adjustStackForCompare();
          const patchExit = emitter.emitBranch(0xa2); // if_icmpge → exit
          // T x = arr[$i]
          const elemSlot = addLocal(ctx, stmt.varName, stmt.varType);
          if (emitter.maxLocals <= elemSlot) emitter.maxLocals = elemSlot + 1;
          emitter.emitAload(arrSlot);
          emitter.emitIload(idxSlot);
          if (stmt.varType === "int" || stmt.varType === "boolean" || stmt.varType === "byte" || stmt.varType === "short" || stmt.varType === "char") {
            emitter.emit(0x2e); // iaload
          } else {
            emitter.emit(0x32); // aaload
          }
          emitter.adjustStackForArrayLoad();
          emitStoreLocalByType(emitter, elemSlot, stmt.varType);
          withScopedLocals(ctx, () => {
            for (const s of stmt.body) compileStmt(ctx, emitter, s);
          });
          const continueTarget = emitter.pc;
          // $i++
          emitter.emit(0x84); emitter.emit(idxSlot); emitter.emit(1); // iinc
          const gotoOp = emitter.emitBranch(0xa7);
          emitter.patchBranch(gotoOp, loopStart);
          emitter.patchBranch(patchExit, emitter.pc);
          for (const p of breakInfo.patches) emitter.patchBranch(p, emitter.pc);
          for (const p of continueInfo.targets) emitter.patchBranch(p, continueTarget);
        } else {
          // Iterable iteration: for (T x : iterable) → Iterator $it = iterable.iterator(); while($it.hasNext()) { T x = (T) $it.next(); ... }
          compileExpr(ctx, emitter, stmt.iterable);
          const iteratorRef = ctx.cp.addInterfaceMethodref("java/lang/Iterable", "iterator", "()Ljava/util/Iterator;");
          emitter.emitInvokeinterface(iteratorRef, 0, true);
          const itSlot = addLocal(ctx, "$forEach_it", { className: "java/util/Iterator" });
          if (emitter.maxLocals <= itSlot) emitter.maxLocals = itSlot + 1;
          emitter.emitAstore(itSlot);
          const loopStart = emitter.pc;
          emitter.emitAload(itSlot);
          const hasNextRef = ctx.cp.addInterfaceMethodref("java/util/Iterator", "hasNext", "()Z");
          emitter.emitInvokeinterface(hasNextRef, 0, true);
          const patchExit = emitter.emitBranch(0x99); // ifeq → exit
          const elemSlot = addLocal(ctx, stmt.varName, stmt.varType);
          if (emitter.maxLocals <= elemSlot) emitter.maxLocals = elemSlot + 1;
          emitter.emitAload(itSlot);
          const nextRef = ctx.cp.addInterfaceMethodref("java/util/Iterator", "next", "()Ljava/lang/Object;");
          emitter.emitInvokeinterface(nextRef, 0, true);
          // Cast to target type if needed
          if (typeof stmt.varType === "object" && "className" in stmt.varType) {
            const classIdx = ctx.cp.addClass(stmt.varType.className);
            emitter.emit(0xc0); emitter.emitU16(classIdx); // checkcast
          } else if (stmt.varType === "String") {
            const classIdx = ctx.cp.addClass("java/lang/String");
            emitter.emit(0xc0); emitter.emitU16(classIdx); // checkcast
          }
          emitStoreLocalByType(emitter, elemSlot, stmt.varType);
          withScopedLocals(ctx, () => {
            for (const s of stmt.body) compileStmt(ctx, emitter, s);
          });
          const continueTarget = emitter.pc;
          const gotoOp = emitter.emitBranch(0xa7);
          emitter.patchBranch(gotoOp, loopStart);
          emitter.patchBranch(patchExit, emitter.pc);
          for (const p of breakInfo.patches) emitter.patchBranch(p, emitter.pc);
          for (const p of continueInfo.targets) emitter.patchBranch(p, continueTarget);
        }
        ctx.breakPatches.pop();
        ctx.continuePatches.pop();
      });
      break;
    }
    case "throw": {
      compileExpr(ctx, emitter, stmt.expr);
      emitter.emit(0xbf); // athrow
      break;
    }
    case "tryCatch": {
      // Simplified try/catch: emit try body, then goto end; emit each catch handler
      // Exception table entries are not yet supported in the bytecode emitter,
      // so we emit the structure and register exception table entries.
      const tryStart = emitter.pc;
      withScopedLocals(ctx, () => {
        for (const s of stmt.tryBody) compileStmt(ctx, emitter, s);
      });
      const tryEnd = emitter.pc;
      const patchEnd = emitter.emitBranch(0xa7); // goto after all catches
      const catchEndPatches: number[] = [patchEnd];
      const exceptionTable: { startPc: number; endPc: number; handlerPc: number; catchType: number }[] = [];
      for (const c of stmt.catches) {
        const handlerPc = emitter.pc;
        const catchClass = resolveClassName(ctx, c.exType);
        const classIdx = ctx.cp.addClass(catchClass);
        exceptionTable.push({ startPc: tryStart, endPc: tryEnd, handlerPc, catchType: classIdx });
        withScopedLocals(ctx, () => {
          // The exception object is on the stack
          emitter.adjustStackForCatch();
          const slot = addLocal(ctx, c.varName, { className: catchClass });
          if (emitter.maxLocals <= slot) emitter.maxLocals = slot + 1;
          emitter.emitAstore(slot);
          for (const s of c.body) compileStmt(ctx, emitter, s);
        });
        catchEndPatches.push(emitter.emitBranch(0xa7)); // goto end
      }
      // Finally block (if present) — simplified: inline after try and each catch
      if (stmt.finallyBody) {
        // Patch all catch-end gotos to here, then emit finally
        for (const p of catchEndPatches) emitter.patchBranch(p, emitter.pc);
        withScopedLocals(ctx, () => {
          for (const s of stmt.finallyBody!) compileStmt(ctx, emitter, s);
        });
      } else {
        for (const p of catchEndPatches) emitter.patchBranch(p, emitter.pc);
      }
      // Store exception table entries on the emitter
      for (const entry of exceptionTable) {
        emitter.exceptionTable.push(entry);
      }
      break;
    }
    case "break": {
      if (stmt.label) {
        // Find the labeled break target
        const info = [...ctx.breakPatches].reverse().find(b => b.label === stmt.label);
        if (!info) throw new Error(`break label '${stmt.label}' not found`);
        info.patches.push(emitter.emitBranch(0xa7));
      } else {
        const info = ctx.breakPatches[ctx.breakPatches.length - 1];
        if (!info) throw new Error("break outside of loop/switch");
        info.patches.push(emitter.emitBranch(0xa7));
      }
      break;
    }
    case "continue": {
      if (stmt.label) {
        const info = [...ctx.continuePatches].reverse().find(c => c.label === stmt.label);
        if (!info) throw new Error(`continue label '${stmt.label}' not found`);
        info.targets.push(emitter.emitBranch(0xa7));
      } else {
        const info = ctx.continuePatches[ctx.continuePatches.length - 1];
        if (!info) throw new Error("continue outside of loop");
        info.targets.push(emitter.emitBranch(0xa7));
      }
      break;
    }
    case "labeled": {
      const breakInfo = { label: stmt.label, patches: [] as number[] };
      const continueInfo = { label: stmt.label, targets: [] as number[] };
      ctx.breakPatches.push(breakInfo);
      ctx.continuePatches.push(continueInfo);
      compileStmt(ctx, emitter, stmt.stmt);
      ctx.breakPatches.pop();
      ctx.continuePatches.pop();
      for (const p of breakInfo.patches) emitter.patchBranch(p, emitter.pc);
      // continue targets for labeled loops are patched by the loop itself
      break;
    }
    case "block": {
      withScopedLocals(ctx, () => {
        for (const s of stmt.stmts) compileStmt(ctx, emitter, s);
      });
      break;
    }
    default:
      throw new Error(`Unsupported statement: ${(stmt as Stmt).kind}`);
  }
}

function compileMethod(
  classDecl: ClassDecl,
  method: MethodDecl,
  cp: ConstantPoolBuilder,
  allMethods: MethodDecl[],
  inheritedFields: FieldDecl[],
  classSupers: Map<string, string>,
  classDecls: Map<string, ClassDecl>,
  lambdaCounter: { value: number },
  generatedMethods: MethodDecl[],
  lambdaBootstraps: LambdaBootstrap[],
): { code: number[]; maxStack: number; maxLocals: number; exceptionTable: { startPc: number; endPc: number; handlerPc: number; catchType: number }[] } {
  const emitter = new BytecodeEmitter();
  const locals: LocalVar[] = [];
  let nextSlot = 0;

  // For instance methods, slot 0 = this
  if (!method.isStatic) {
    locals.push({ name: "this", type: { className: classDecl.name }, slot: 0 });
    nextSlot = 1;
  }
  // Parameters
  for (const p of method.params) {
    locals.push({ name: p.name, type: p.type, slot: nextSlot });
    nextSlot++;
  }

  const ctx: CompileContext = {
    className: classDecl.name,
    superClass: classDecl.superClass,
    cp,
    method,
    locals,
    nextSlot,
    fields: classDecl.fields,
    inheritedFields,
    allMethods,
    importMap: classDecl.importMap,
    packageImports: classDecl.packageImports,
    staticWildcardImports: classDecl.staticWildcardImports,
    classSupers,
    classDecls,
    lambdaCounter,
    generatedMethods,
    lambdaBootstraps,
    ownerIsStatic: method.isStatic,
    breakPatches: [],
    continuePatches: [],
  };

  emitter.maxLocals = nextSlot;

  for (const stmt of method.body) {
    compileStmt(ctx, emitter, stmt);
  }

  // If method doesn't explicitly return, add return
  const lastByte = emitter.code.length > 0 ? emitter.code[emitter.code.length - 1] : -1;
  const isReturn = lastByte === 0xb1 || lastByte === 0xac || lastByte === 0xb0 || lastByte === 0xad || lastByte === 0xae || lastByte === 0xaf;
  if (!isReturn) {
    emitter.emitReturn(method.returnType);
  }

  return { code: emitter.code, maxStack: Math.max(emitter.maxStack, 4), maxLocals: emitter.maxLocals, exceptionTable: emitter.exceptionTable };
}

function exprHasSuperCall(expr: Expr): boolean {
  switch (expr.kind) {
    case "superCall": return true;
    case "binary": return exprHasSuperCall(expr.left) || exprHasSuperCall(expr.right);
    case "unary": return exprHasSuperCall(expr.operand);
    case "call": return (expr.object ? exprHasSuperCall(expr.object) : false) || expr.args.some(exprHasSuperCall);
    case "staticCall": return expr.args.some(exprHasSuperCall);
    case "fieldAccess": return exprHasSuperCall(expr.object);
    case "newExpr": return expr.args.some(exprHasSuperCall);
    case "cast": return exprHasSuperCall(expr.expr);
    case "postIncrement": return exprHasSuperCall(expr.operand);
    case "preIncrement": return exprHasSuperCall(expr.operand);
    case "instanceof": return exprHasSuperCall(expr.expr);
    case "arrayAccess": return exprHasSuperCall(expr.array) || exprHasSuperCall(expr.index);
    case "arrayLit": return expr.elements.some(exprHasSuperCall);
    case "newArray": return exprHasSuperCall(expr.size);
    case "ternary": return exprHasSuperCall(expr.cond) || exprHasSuperCall(expr.thenExpr) || exprHasSuperCall(expr.elseExpr);
    case "switchExpr":
      return exprHasSuperCall(expr.selector)
        || expr.cases.some(c => (c.expr && exprHasSuperCall(c.expr)) || (c.stmts && c.stmts.some(stmtHasSuperCall)));
    case "lambda":
      return !!expr.bodyExpr && exprHasSuperCall(expr.bodyExpr)
        || !!expr.bodyStmts && expr.bodyStmts.some(stmtHasSuperCall);
    case "methodRef":
      return exprHasSuperCall(expr.target);
    default: return false;
  }
}

function stmtHasSuperCall(stmt: Stmt): boolean {
  switch (stmt.kind) {
    case "varDecl": return !!stmt.init && exprHasSuperCall(stmt.init);
    case "assign": return exprHasSuperCall(stmt.target) || exprHasSuperCall(stmt.value);
    case "exprStmt": return exprHasSuperCall(stmt.expr);
    case "return": return !!stmt.value && exprHasSuperCall(stmt.value);
    case "yield": return exprHasSuperCall(stmt.value);
    case "if": return exprHasSuperCall(stmt.cond) || stmt.then.some(stmtHasSuperCall) || !!stmt.else_?.some(stmtHasSuperCall);
    case "while": return exprHasSuperCall(stmt.cond) || stmt.body.some(stmtHasSuperCall);
    case "for": return !!stmt.init && stmtHasSuperCall(stmt.init) || !!stmt.cond && exprHasSuperCall(stmt.cond) || !!stmt.update && stmtHasSuperCall(stmt.update) || stmt.body.some(stmtHasSuperCall);
    case "switch":
      return exprHasSuperCall(stmt.selector)
        || stmt.cases.some(c => (c.expr && exprHasSuperCall(c.expr)) || (c.stmts && c.stmts.some(stmtHasSuperCall)));
    case "doWhile": return exprHasSuperCall(stmt.cond) || stmt.body.some(stmtHasSuperCall);
    case "forEach": return exprHasSuperCall(stmt.iterable) || stmt.body.some(stmtHasSuperCall);
    case "throw": return exprHasSuperCall(stmt.expr);
    case "tryCatch": return stmt.tryBody.some(stmtHasSuperCall)
      || stmt.catches.some(c => c.body.some(stmtHasSuperCall))
      || !!stmt.finallyBody?.some(stmtHasSuperCall);
    case "break": return false;
    case "continue": return false;
    case "labeled": return stmtHasSuperCall(stmt.stmt);
    case "block": return stmt.stmts.some(stmtHasSuperCall);
  }
}

function validateConstructorBody(method: MethodDecl): void {
  if (method.name !== "<init>") {
    if (method.body.some(stmtHasSuperCall)) {
      throw new Error("super(...) call is only allowed in constructors");
    }
    return;
  }
  const topLevelSuperCalls = method.body.filter(s => s.kind === "exprStmt" && s.expr.kind === "superCall");
  if (topLevelSuperCalls.length === 0) {
    if (method.body.some(stmtHasSuperCall)) {
      throw new Error("super(...) call must be the first statement in constructor");
    }
    return;
  }
  const first = method.body[0];
  if (!(first.kind === "exprStmt" && first.expr.kind === "superCall")) {
    throw new Error("super(...) call must be the first statement in constructor");
  }
  if (topLevelSuperCalls.length > 1) {
    throw new Error("super(...) call may appear at most once in constructor body");
  }
  for (let i = 1; i < method.body.length; i++) {
    if (stmtHasSuperCall(method.body[i])) {
      throw new Error("super(...) call must be the first statement in constructor");
    }
  }
}

// Produce bundle bytes for all classes in source.
// Bundle format: for each class, 4-byte big-endian length followed by .class bytes.
// For a single class, returns just the raw .class bytes (backward compat with index.html).
// Recursively collect all nested classes into a flat list.
function flattenClasses(decls: ClassDecl[]): ClassDecl[] {
  const result: ClassDecl[] = [];
  for (const cd of decls) {
    result.push(cd);
    if (cd.nestedClasses.length > 0) {
      result.push(...flattenClasses(cd.nestedClasses));
    }
  }
  return result;
}

export function compile(source: string): Uint8Array {
  const tokens = lex(source);
  const classDecls = flattenClasses(parseAll(tokens));
  if (classDecls.length === 1) {
    return generateClassFile(classDecls[0], classDecls);
  }
  // Multiple classes: build a length-prefixed bundle
  const classFiles = classDecls.map(cd => generateClassFile(cd, classDecls));
  let total = 0;
  for (const cf of classFiles) total += 4 + cf.length;
  const bundle = new Uint8Array(total);
  let off = 0;
  for (const cf of classFiles) {
    bundle[off++] = (cf.length >> 24) & 0xff;
    bundle[off++] = (cf.length >> 16) & 0xff;
    bundle[off++] = (cf.length >>  8) & 0xff;
    bundle[off++] =  cf.length        & 0xff;
    bundle.set(cf, off);
    off += cf.length;
  }
  return bundle;
}

export function generateClassFile(classDecl: ClassDecl, allClassDecls: ClassDecl[] = [classDecl]): Uint8Array {
  const allMethods = allClassDecls.flatMap(cd => cd.methods);
  const classSupers = new Map<string, string>();
  const classDecls = new Map<string, ClassDecl>();
  for (const cd of allClassDecls) {
    classSupers.set(cd.name, cd.superClass);
    classDecls.set(cd.name, cd);
  }
  const lambdaCounter = { value: 0 };
  const generatedMethods: MethodDecl[] = [];
  const lambdaBootstraps: LambdaBootstrap[] = [];
  // Collect fields from superclass chain
  const inheritedFields: FieldDecl[] = [];
  let superName = classDecl.superClass;
  while (superName && superName !== "java/lang/Object") {
    const superDecl = allClassDecls.find(cd => cd.name === superName);
    if (!superDecl) break;
    inheritedFields.push(...superDecl.fields.filter(f => !f.isStatic));
    superName = superDecl.superClass;
  }
  const cp = new ConstantPoolBuilder();

  // Reserve this_class and super_class
  const thisClassIdx = cp.addClass(classDecl.name);
  const superClassIdx = cp.addClass(classDecl.superClass);

  // Add default constructor if none exists
  const hasInit = classDecl.methods.some(m => m.name === "<init>");
  if (!hasInit) {
    classDecl.methods.unshift({
      name: "<init>",
      returnType: "void",
      params: [],
      body: [],
      isStatic: false,
    });
  }

  // Compile all methods
  const compiledMethods: {
    nameIdx: number;
    descIdx: number;
    accessFlags: number;
    code: number[];
    maxStack: number;
    maxLocals: number;
    exceptionTable?: { startPc: number; endPc: number; handlerPc: number; catchType: number }[];
  }[] = [];

  const methodQueue: MethodDecl[] = [...classDecl.methods];
  let generatedDrain = 0;
  for (let mi = 0; mi < methodQueue.length; mi++) {
    const method = methodQueue[mi];
    validateConstructorBody(method);
    const nameIdx = cp.addUtf8(method.name);
    const desc = methodDescriptor(method.params, method.returnType);
    const descIdx = cp.addUtf8(desc);

    let accessFlags = 0x0001; // ACC_PUBLIC
    if (method.isStatic) accessFlags |= 0x0008; // ACC_STATIC

    if (method.name === "<init>") {
      const emitter = new BytecodeEmitter();
      // If the constructor body starts with super(args), it will emit its own invokespecial.
      // Otherwise emit a default super() call.
      const hasSuperCall = method.body.length > 0 &&
        method.body[0].kind === "exprStmt" &&
        method.body[0].expr.kind === "superCall";
      if (!hasSuperCall) {
        const superInitRef = cp.addMethodref(classDecl.superClass, "<init>", "()V");
        emitter.emitAload(0); // this
        emitter.emitInvokespecial(superInitRef, 0, false);
      }

      // Set up locals for constructor params (slot 0 = this, 1..n = params)
      const initCtx: CompileContext = {
        className: classDecl.name, superClass: classDecl.superClass, cp, method,
        locals: method.params.map((p, i) => ({ name: p.name, type: p.type, slot: i + 1 })),
        nextSlot: method.params.length + 1,
        fields: classDecl.fields, inheritedFields, allMethods,
        importMap: classDecl.importMap,
        packageImports: classDecl.packageImports,
        staticWildcardImports: classDecl.staticWildcardImports,
        classSupers,
        classDecls,
        lambdaCounter,
        generatedMethods,
        lambdaBootstraps,
        ownerIsStatic: false,
        breakPatches: [],
        continuePatches: [],
      };
      if (emitter.maxLocals < method.params.length + 1) emitter.maxLocals = method.params.length + 1;

      // Initialize instance fields with initializers
      for (const field of classDecl.fields) {
        if (!field.isStatic && field.initializer) {
          emitter.emitAload(0); // this
          compileExpr(initCtx, emitter, field.initializer, field.type);
          const fRef = cp.addFieldref(classDecl.name, field.name, typeToDescriptor(field.type));
          emitter.emit(0xb5); // putfield
          emitter.emitU16(fRef);
        }
      }

      // Compile explicit constructor body statements
      for (const stmt of method.body) {
        compileStmt(initCtx, emitter, stmt);
      }

      emitter.emit(0xb1); // return
      compiledMethods.push({
        nameIdx, descIdx, accessFlags,
        code: emitter.code,
        maxStack: Math.max(emitter.maxStack, 4),
        maxLocals: Math.max(emitter.maxLocals, method.params.length + 1),
        exceptionTable: emitter.exceptionTable.length > 0 ? emitter.exceptionTable : undefined,
      });
    } else {
      const result = compileMethod(
        classDecl, method, cp, allMethods, inheritedFields,
        classSupers, classDecls,
        lambdaCounter, generatedMethods, lambdaBootstraps,
      );
      compiledMethods.push({
        nameIdx, descIdx, accessFlags,
        code: result.code,
        maxStack: result.maxStack,
        maxLocals: result.maxLocals,
        exceptionTable: result.exceptionTable.length > 0 ? result.exceptionTable : undefined,
      });
    }
    while (generatedDrain < generatedMethods.length) {
      const gm = generatedMethods[generatedDrain++];
      methodQueue.push(gm);
      allMethods.push(gm);
    }
  }

  // Build fields
  const compiledFields: { nameIdx: number; descIdx: number; accessFlags: number }[] = [];
  for (const field of classDecl.fields) {
    const nameIdx = cp.addUtf8(field.name);
    const descIdx = cp.addUtf8(typeToDescriptor(field.type));
    let accessFlags = field.isPrivate ? 0x0002 : 0x0001; // ACC_PRIVATE/ACC_PUBLIC
    if (field.isStatic) accessFlags |= 0x0008;
    if (field.isFinal) accessFlags |= 0x0010;
    compiledFields.push({ nameIdx, descIdx, accessFlags });
  }

  // Code attribute name
  const codeAttrName = cp.addUtf8("Code");
  const bootstrapAttrName = cp.addUtf8("BootstrapMethods");
  const recordAttrName = classDecl.isRecord ? cp.addUtf8("Record") : 0;

  // Pre-register record component names/descriptors in the constant pool
  const recordComponentCpEntries: { nameIdx: number; descIdx: number }[] = [];
  if (classDecl.isRecord && classDecl.recordComponents) {
    for (const c of classDecl.recordComponents) {
      recordComponentCpEntries.push({
        nameIdx: cp.addUtf8(c.name),
        descIdx: cp.addUtf8(typeToDescriptor(c.type)),
      });
    }
  }
  const serializedBootstrapMethods: { methodRef: number; args: number[] }[] = [];
  for (const lb of lambdaBootstraps) {
    const metafactoryRef = cp.addMethodref("java/lang/invoke/LambdaMetafactory", "metafactory", "()V");
    const bootstrapMethodRef = cp.addMethodHandle(6, metafactoryRef);
    const implMethodRef = lb.implIsInterface
      ? cp.addInterfaceMethodref(lb.implOwner, lb.implMethodName, lb.implDescriptor)
      : cp.addMethodref(lb.implOwner, lb.implMethodName, lb.implDescriptor);
    const implHandle = cp.addMethodHandle(lb.implRefKind, implMethodRef);
    const samType = cp.addMethodType(lb.samDescriptor);
    const instantiatedType = cp.addMethodType(lb.implDescriptor);
    serializedBootstrapMethods.push({ methodRef: bootstrapMethodRef, args: [samType, implHandle, instantiatedType] });
  }

  // Now serialize the class file
  const out: number[] = [];

  // Magic
  out.push(0xCA, 0xFE, 0xBA, 0xBE);
  // Version: 0.52 (Java 8 — 199xVM supports up to 69 but 52 is safe)
  out.push(0x00, 0x00); // minor
  out.push(0x00, 0x34); // major = 52

  // Constant pool
  out.push(...cp.serialize());

  // Access flags: ACC_PUBLIC | ACC_SUPER
  const classFlags = classDecl.isRecord ? 0x0031 : 0x0021; // record classes are final
  out.push((classFlags >> 8) & 0xff, classFlags & 0xff);
  // this_class
  out.push((thisClassIdx >> 8) & 0xff, thisClassIdx & 0xff);
  // super_class
  out.push((superClassIdx >> 8) & 0xff, superClassIdx & 0xff);
  // interfaces_count
  out.push(0x00, 0x00);

  // fields_count
  out.push((compiledFields.length >> 8) & 0xff, compiledFields.length & 0xff);
  for (const f of compiledFields) {
    out.push((f.accessFlags >> 8) & 0xff, f.accessFlags & 0xff);
    out.push((f.nameIdx >> 8) & 0xff, f.nameIdx & 0xff);
    out.push((f.descIdx >> 8) & 0xff, f.descIdx & 0xff);
    out.push(0x00, 0x00); // attributes_count = 0
  }

  // methods_count
  out.push((compiledMethods.length >> 8) & 0xff, compiledMethods.length & 0xff);
  for (const m of compiledMethods) {
    out.push((m.accessFlags >> 8) & 0xff, m.accessFlags & 0xff);
    out.push((m.nameIdx >> 8) & 0xff, m.nameIdx & 0xff);
    out.push((m.descIdx >> 8) & 0xff, m.descIdx & 0xff);
    // 1 attribute: Code
    out.push(0x00, 0x01);

    // Code attribute
    out.push((codeAttrName >> 8) & 0xff, codeAttrName & 0xff);
    const codeLen = m.code.length;
    const exTblLen = m.exceptionTable ? m.exceptionTable.length : 0;
    const attrLen = 2 + 2 + 4 + codeLen + 2 + exTblLen * 8 + 2; // max_stack + max_locals + code_length + code + exception_table + attributes_count
    out.push((attrLen >> 24) & 0xff, (attrLen >> 16) & 0xff, (attrLen >> 8) & 0xff, attrLen & 0xff);
    out.push((m.maxStack >> 8) & 0xff, m.maxStack & 0xff);
    out.push((m.maxLocals >> 8) & 0xff, m.maxLocals & 0xff);
    out.push((codeLen >> 24) & 0xff, (codeLen >> 16) & 0xff, (codeLen >> 8) & 0xff, codeLen & 0xff);
    out.push(...m.code);
    out.push((exTblLen >> 8) & 0xff, exTblLen & 0xff);
    if (m.exceptionTable) {
      for (const e of m.exceptionTable) {
        out.push((e.startPc >> 8) & 0xff, e.startPc & 0xff);
        out.push((e.endPc >> 8) & 0xff, e.endPc & 0xff);
        out.push((e.handlerPc >> 8) & 0xff, e.handlerPc & 0xff);
        out.push((e.catchType >> 8) & 0xff, e.catchType & 0xff);
      }
    }
    out.push(0x00, 0x00); // attributes_count = 0
  }

  // class attributes
  let classAttrCount = 0;
  if (serializedBootstrapMethods.length > 0) classAttrCount++;
  if (classDecl.isRecord && recordComponentCpEntries.length > 0) classAttrCount++;
  out.push((classAttrCount >> 8) & 0xff, classAttrCount & 0xff);
  if (serializedBootstrapMethods.length > 0) {
    out.push((bootstrapAttrName >> 8) & 0xff, bootstrapAttrName & 0xff);
    const bmCount = serializedBootstrapMethods.length;
    const bodyLen = 2 + serializedBootstrapMethods.reduce((s, bm) => s + 4 + bm.args.length * 2, 0);
    out.push((bodyLen >> 24) & 0xff, (bodyLen >> 16) & 0xff, (bodyLen >> 8) & 0xff, bodyLen & 0xff);
    out.push((bmCount >> 8) & 0xff, bmCount & 0xff);
    for (const bm of serializedBootstrapMethods) {
      out.push((bm.methodRef >> 8) & 0xff, bm.methodRef & 0xff);
      out.push((bm.args.length >> 8) & 0xff, bm.args.length & 0xff);
      for (const a of bm.args) out.push((a >> 8) & 0xff, a & 0xff);
    }
  }
  // Record attribute
  if (classDecl.isRecord && recordComponentCpEntries.length > 0) {
    out.push((recordAttrName >> 8) & 0xff, recordAttrName & 0xff);
    // Each component: name(2) + descriptor(2) + attributes_count(2) = 6 bytes
    const recBodyLen = 2 + recordComponentCpEntries.length * 6;
    out.push((recBodyLen >> 24) & 0xff, (recBodyLen >> 16) & 0xff, (recBodyLen >> 8) & 0xff, recBodyLen & 0xff);
    out.push((recordComponentCpEntries.length >> 8) & 0xff, recordComponentCpEntries.length & 0xff);
    for (const rc of recordComponentCpEntries) {
      out.push((rc.nameIdx >> 8) & 0xff, rc.nameIdx & 0xff);
      out.push((rc.descIdx >> 8) & 0xff, rc.descIdx & 0xff);
      out.push(0x00, 0x00); // attributes_count = 0
    }
  }

  return new Uint8Array(out);
}

// ============================================================================
// Disassembler — javap-style output from raw .class bytes
// ============================================================================

const OPCODES: Record<number, string> = {
  0x00: "nop",         0x01: "aconst_null",  0x02: "iconst_m1",  0x03: "iconst_0",
  0x04: "iconst_1",    0x05: "iconst_2",     0x06: "iconst_3",   0x07: "iconst_4",
  0x08: "iconst_5",    0x09: "lconst_0",     0x0a: "lconst_1",   0x10: "bipush",
  0x11: "sipush",      0x12: "ldc",          0x13: "ldc_w",      0x15: "iload",
  0x19: "aload",       0x1a: "iload_0",      0x1b: "iload_1",    0x1c: "iload_2",
  0x1d: "iload_3",     0x2a: "aload_0",      0x2b: "aload_1",    0x2c: "aload_2",
  0x2d: "aload_3",     0x36: "istore",       0x3a: "astore",     0x3b: "istore_0",
  0x3c: "istore_1",    0x3d: "istore_2",     0x3e: "istore_3",   0x4b: "astore_0",
  0x4c: "astore_1",    0x4d: "astore_2",     0x4e: "astore_3",   0x57: "pop",
  0x58: "pop2",        0x59: "dup",          0x60: "iadd",        0x64: "isub",
  0x68: "imul",        0x6c: "idiv",         0x70: "irem",        0x74: "ineg",
  0x84: "iinc",        0x99: "ifeq",         0x9a: "ifne",        0x9b: "iflt",
  0x9c: "ifge",        0x9d: "ifgt",         0x9e: "ifle",        0x9f: "if_icmpeq",
  0xa0: "if_icmpne",   0xa1: "if_icmplt",    0xa2: "if_icmpge",   0xa3: "if_icmpgt",
  0xa4: "if_icmple",   0xa5: "if_acmpeq",    0xa6: "if_acmpne",   0xa7: "goto",
  0xac: "ireturn",     0xb0: "areturn",      0xb1: "return",      0xb2: "getstatic",
  0xb3: "putstatic",   0xb4: "getfield",     0xb5: "putfield",    0xb6: "invokevirtual",
  0xb7: "invokespecial", 0xb8: "invokestatic", 0xb9: "invokeinterface", 0xba: "invokedynamic",
  0xbb: "new",         0xbc: "newarray",     0xbe: "arraylength", 0xbf: "athrow",
  0xc0: "checkcast",   0xc1: "instanceof",   0xc6: "ifnull",      0xc7: "ifnonnull",
};

// Instruction operand widths (bytes after opcode), -1 = variable
const OPCODE_WIDTHS: Record<number, number> = {
  0x10: 1, 0x11: 2, 0x12: 1, 0x13: 2,
  0x15: 1, 0x19: 1, 0x36: 1, 0x3a: 1,
  0x84: 2,
  0x99: 2, 0x9a: 2, 0x9b: 2, 0x9c: 2, 0x9d: 2, 0x9e: 2,
  0x9f: 2, 0xa0: 2, 0xa1: 2, 0xa2: 2, 0xa3: 2, 0xa4: 2,
  0xa5: 2, 0xa6: 2, 0xa7: 2,
  0xb2: 2, 0xb3: 2, 0xb4: 2, 0xb5: 2,
  0xb6: 2, 0xb7: 2, 0xb8: 2, 0xb9: 4, 0xba: 4,
  0xbb: 2, 0xbc: 1, 0xc0: 2, 0xc1: 2, 0xc6: 2, 0xc7: 2,
};

export function disassemble(classBytes: Uint8Array): string {
  const dv = new DataView(classBytes.buffer, classBytes.byteOffset, classBytes.byteLength);
  const lines: string[] = [];
  let pos = 0;

  function u8()  { return dv.getUint8(pos++); }
  function u16() { const v = dv.getUint16(pos); pos += 2; return v; }
  function u32() { const v = dv.getUint32(pos); pos += 4; return v; }
  function skip(n: number) { pos += n; }

  // Magic + version
  const magic = u32();
  if (magic !== 0xCAFEBABE) return "Not a valid .class file";
  const minor = u16(), major = u16();

  // Constant pool
  const cpCount = u16();
  const cp: (string | null)[] = [null]; // 1-based
  for (let i = 1; i < cpCount; i++) {
    const tag = u8();
    switch (tag) {
      case 1: { // Utf8
        const len = u16();
        let s = "";
        for (let j = 0; j < len; j++) s += String.fromCharCode(u8());
        cp.push(s); break;
      }
      case 7: { cp.push(`#class:${u16()}`); break; }
      case 8: { cp.push(`#str:${u16()}`); break; }
      case 9: { cp.push(`#field:${u16()}:${u16()}`); break; }
      case 10: { cp.push(`#meth:${u16()}:${u16()}`); break; }
      case 11: { cp.push(`#imeth:${u16()}:${u16()}`); break; }
      case 12: { cp.push(`#nat:${u16()}:${u16()}`); break; }
      case 18: { cp.push(`#indy:${u16()}:${u16()}`); break; }
      case 3: { cp.push(`int:${dv.getInt32(pos)}`); pos += 4; break; }
      case 4: { cp.push(`float:${dv.getFloat32(pos)}`); pos += 4; break; }
      case 5: { cp.push(`long:${dv.getBigInt64 ? dv.getBigInt64(pos) : pos}`); pos += 8; cp.push(null); i++; break; }
      case 15: { cp.push(`#mhnd:${u8()}:${u16()}`); break; }
      case 16: { cp.push(`#mtype:${u16()}`); break; }
      default: { cp.push(`?tag${tag}`); break; }
    }
  }

  // Helpers to resolve cp refs
  function cpClass(idx: number): string {
    const entry = cp[idx];
    if (!entry) return `#${idx}`;
    const m = entry.match(/^#class:(\d+)$/);
    return m ? (cp[+m[1]] ?? `#${m[1]}`).replace(/\//g, ".") : entry;
  }
  function cpNat(idx: number): [string, string] {
    const entry = cp[idx] ?? "";
    const m = entry.match(/^#nat:(\d+):(\d+)$/);
    if (!m) return ["?", "?"];
    return [cp[+m[1]] ?? "?", cp[+m[2]] ?? "?"];
  }
  function cpRef(idx: number): string {
    const entry = cp[idx] ?? "";
    const m = entry.match(/^#(?:meth|field|imeth):(\d+):(\d+)$/);
    if (!m) return `#${idx}`;
    const cls = cpClass(+m[1]);
    const [name, desc] = cpNat(+m[2]);
    return `${cls}.${name}:${desc}`;
  }
  function cpString(idx: number): string {
    const entry = cp[idx] ?? "";
    const m = entry.match(/^#str:(\d+)$/);
    return m ? `"${cp[+m[1]] ?? ""}"` : entry;
  }
  function cpIndy(idx: number): string {
    const entry = cp[idx] ?? "";
    const m = entry.match(/^#indy:(\d+):(\d+)$/);
    if (!m) return `#${idx}`;
    const [name, desc] = cpNat(+m[2]);
    return `#${m[1]}:${name}${desc}`;
  }

  // Access flags
  const accessFlags = u16();
  const thisClass = cpClass(u16());
  const superClass = cpClass(u16());

  const flagStr = [
    accessFlags & 0x0001 ? "public" : "",
    accessFlags & 0x0020 ? "/* super */" : "",
  ].filter(Boolean).join(" ");

  lines.push(`${flagStr} class ${thisClass}`);
  if (superClass && superClass !== "java.lang.Object") {
    lines.push(`  extends ${superClass}`);
  }

  // Interfaces
  const ifCount = u16();
  for (let i = 0; i < ifCount; i++) u16();

  // Fields
  const fieldCount = u16();
  if (fieldCount > 0) lines.push("");
  for (let i = 0; i < fieldCount; i++) {
    const fFlags = u16();
    const fName = cp[u16()] ?? "?";
    const fDesc = cp[u16()] ?? "?";
    const fAccess = [
      fFlags & 0x0001 ? "public" : fFlags & 0x0002 ? "private" : "",
      fFlags & 0x0008 ? "static" : "",
      fFlags & 0x0010 ? "final" : "",
    ].filter(Boolean).join(" ");
    lines.push(`  ${fAccess} ${descToType(fDesc)} ${fName};`);
    const attrCount = u16();
    for (let a = 0; a < attrCount; a++) { u16(); skip(u32()); }
  }

  // Methods
  const methodCount = u16();
  for (let i = 0; i < methodCount; i++) {
    const mFlags = u16();
    const mName = cp[u16()] ?? "?";
    const mDesc = cp[u16()] ?? "?";
    const mAccess = [
      mFlags & 0x0001 ? "public" : mFlags & 0x0002 ? "private" : "",
      mFlags & 0x0008 ? "static" : "",
    ].filter(Boolean).join(" ");

    const [paramTypes, retType] = parseDescriptor(mDesc);
    const paramStr = paramTypes.map((t, j) => `${t} arg${j}`).join(", ");
    const displayName = mName === "<init>" ? thisClass.split(".").pop()! : mName;

    lines.push("");
    lines.push(`  ${mAccess} ${mName === "<init>" ? "" : retType + " "}${displayName}(${paramStr});`);

    const attrCount = u16();
    for (let a = 0; a < attrCount; a++) {
      const attrName = cp[u16()] ?? "?";
      const attrLen = u32();
      if (attrName === "Code") {
        lines.push("    Code:");
        u16(); u16(); // maxStack, maxLocals
        const codeLen = u32();
        const codeStart = pos;
        const codeEnd = codeStart + codeLen;

        while (pos < codeEnd) {
          const offset = pos - codeStart;
          const op = u8();
          const opName = OPCODES[op] ?? `unknown(0x${op.toString(16).padStart(2,"0")})`;
          const width = OPCODE_WIDTHS[op] ?? 0;
          let operandStr = "";

          if (op === 0xb6 || op === 0xb7 || op === 0xb8) { // invoke{virtual,special,static}
            const ref = u16();
            operandStr = `#${ref.toString().padStart(2)} // ${cpRef(ref)}`;
          } else if (op === 0xb9 || op === 0xba) { // invokeinterface, invokedynamic
            const ref = u16(); skip(2);
            const label = op === 0xba ? cpIndy(ref) : cpRef(ref);
            operandStr = `#${ref.toString().padStart(2)} // ${op === 0xba ? "InvokeDynamic" : "InterfaceMethod"} ${label}`;
          } else if (op === 0xb2 || op === 0xb3 || op === 0xb4 || op === 0xb5) { // field ops
            const ref = u16();
            operandStr = `#${ref.toString().padStart(2)} // ${cpRef(ref)}`;
          } else if (op === 0xbb || op === 0xc0 || op === 0xc1) { // new, checkcast, instanceof
            const ref = u16();
            operandStr = `#${ref.toString().padStart(2)} // class ${cpClass(ref)}`;
          } else if (op === 0x12) { // ldc
            const ref = u8();
            const v = cp[ref] ?? `#${ref}`;
            operandStr = `#${ref.toString().padStart(2)} // ${v.startsWith("#str:") ? cpString(ref) : v}`;
          } else if (op === 0x13) { // ldc_w
            const ref = u16();
            const v = cp[ref] ?? `#${ref}`;
            operandStr = `#${ref.toString().padStart(2)} // ${v.startsWith("#str:") ? cpString(ref) : v}`;
          } else if (op === 0x84) { // iinc
            const idx = u8(), c = dv.getInt8(pos++);
            operandStr = `${idx}, ${c}`;
          } else if (op === 0x10) { // bipush
            operandStr = `${dv.getInt8(pos++)}`;
          } else if (op === 0x11) { // sipush
            operandStr = `${dv.getInt16(pos)}`; pos += 2;
          } else if (width === 1) {
            operandStr = `${u8()}`;
          } else if (width === 2) {
            const raw = dv.getInt16(pos); pos += 2;
            // branch instructions show target offset
            if (op >= 0x99 && op <= 0xa7) operandStr = `${offset + raw}`;
            else operandStr = `${raw}`;
          } else if (width === 4) {
            operandStr = `${dv.getInt32(pos)}`; pos += 4;
          }

          lines.push(`       ${offset.toString().padStart(3)}: ${opName.padEnd(18)} ${operandStr}`);
        }

        // Skip exception table + remaining Code attrs
        const excCount = u16();
        skip(excCount * 8);
        const codeAttrCount = u16();
        for (let ca = 0; ca < codeAttrCount; ca++) { u16(); skip(u32()); }
      } else {
        skip(attrLen);
      }
    }
  }

  lines.unshift(`// class file v${major}.${minor}`);
  return lines.join("\n");
}

function descToType(desc: string): string {
  if (desc === "I") return "int";
  if (desc === "Z") return "boolean";
  if (desc === "V") return "void";
  if (desc === "J") return "long";
  if (desc === "D") return "double";
  if (desc === "F") return "float";
  if (desc.startsWith("L") && desc.endsWith(";")) {
    return desc.slice(1, -1).split("/").pop()!;
  }
  if (desc.startsWith("[")) return descToType(desc.slice(1)) + "[]";
  return desc;
}

function parseDescriptor(desc: string): [string[], string] {
  const m = desc.match(/^\(([^)]*)\)(.+)$/);
  if (!m) return [[], desc];
  const params: string[] = [];
  let i = 0;
  const p = m[1];
  while (i < p.length) {
    if (p[i] === "L") {
      const end = p.indexOf(";", i);
      params.push(descToType(p.slice(i, end + 1)));
      i = end + 1;
    } else if (p[i] === "[") {
      let j = i + 1;
      while (j < p.length && p[j] === "[") j++;
      if (p[j] === "L") {
        const end = p.indexOf(";", j);
        params.push(descToType(p.slice(i, end + 1)));
        i = end + 1;
      } else {
        params.push(descToType(p.slice(i, j + 1)));
        i = j + 1;
      }
    } else {
      params.push(descToType(p[i]));
      i++;
    }
  }
  return [params, descToType(m[2])];
}
