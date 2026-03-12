import { lex } from "./lexer.js";
import { parseAll } from "./parser.js";
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
} from "./ast.js";
import {
  findKnownFunctionalInterface,
  findKnownMethodByArity,
  hasFunctionalArg,
  hasKnownMethodOwnerPrefix,
  lookupKnownMethod,
  setMethodRegistry,
} from "./method-registry.js";
import type { FunctionalSig } from "./method-registry.js";
export { setMethodRegistry };

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
      if (e.tag === 0) continue; // placeholder for long/double second slot — no bytes emitted
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

function enumConstructorDescriptor(paramTypes: Type[]): string {
  return "(Ljava/lang/String;I" + paramTypes.map(typeToDescriptor).join("") + ")V";
}

function isRefType(t: Type): boolean {
  return t !== "int" && t !== "long" && t !== "short" && t !== "byte" && t !== "char"
    && t !== "float" && t !== "double" && t !== "boolean" && t !== "void";
}

function isPrimitiveType(t: Type): boolean {
  return t === "int" || t === "long" || t === "short" || t === "byte" || t === "char"
    || t === "float" || t === "double" || t === "boolean";
}

function isIntegralType(t: Type): boolean {
  return t === "int" || t === "long" || t === "short" || t === "byte" || t === "char";
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
      if (hasKnownMethodOwnerPrefix(candidate)) return candidate;
    }
    return `${ctx.packageImports[0]}/${name}`;
  }
  return name;
}

interface ResolvedMethodCandidate {
  owner: string;
  paramTypes: Type[];
  returnType: Type;
  isStatic: boolean;
  isInterface: boolean;
}

function ownerSearchOrder(ctx: CompileContext, startOwner: string): string[] {
  const classChain: string[] = [];
  const seenClass = new Set<string>();
  let cur: string | undefined = startOwner;
  while (cur && !seenClass.has(cur)) {
    seenClass.add(cur);
    classChain.push(cur);
    const decl = ctx.classDecls.get(cur);
    if (decl) cur = decl.superClass ? resolveClassName(ctx, decl.superClass) : undefined;
    else cur = ctx.classSupers.get(cur) ?? BUILTIN_SUPERS[cur];
  }

  const interfaces: string[] = [];
  const seenIface = new Set<string>();
  const queue: string[] = [];
  for (const owner of classChain) {
    const decl = ctx.classDecls.get(owner);
    if (!decl) continue;
    for (const itf of decl.interfaces ?? []) queue.push(resolveClassName(ctx, itf));
  }
  while (queue.length > 0) {
    const itf = queue.shift()!;
    if (seenIface.has(itf)) continue;
    seenIface.add(itf);
    interfaces.push(itf);
    const decl = ctx.classDecls.get(itf);
    if (!decl) continue;
    for (const parent of decl.interfaces ?? []) queue.push(resolveClassName(ctx, parent));
  }

  return [...classChain, ...interfaces];
}

function resolveMethodCandidate(
  ctx: CompileContext,
  ownerClass: string,
  method: string,
  args: Expr[],
  wantStatic: boolean,
): ResolvedMethodCandidate | undefined {
  const argTypes = args.map(a => inferType(ctx, a));
  return resolveMethodCandidateByTypes(ctx, ownerClass, method, argTypes, wantStatic, args);
}

function resolveMethodCandidateByTypes(
  ctx: CompileContext,
  ownerClass: string,
  method: string,
  argTypes: Type[],
  wantStatic: boolean,
  originalArgs: Expr[] = [],
): ResolvedMethodCandidate | undefined {
  const argDescs = argTypes.map(typeToDescriptor).join("");
  for (const owner of ownerSearchOrder(ctx, ownerClass)) {
    const decl = ctx.classDecls.get(owner);
    if (decl) {
      const candidates = decl.methods.filter(mm => mm.name === method && mm.isStatic === wantStatic && mm.params.length === argTypes.length);
      if (candidates.length === 0) continue;
      const exactMatches = candidates.filter(mm => mm.params.map(p => typeToDescriptor(p.type)).join("") === argDescs);
      let m: MethodDecl | undefined;
      if (exactMatches.length === 1) m = exactMatches[0];
      else if (exactMatches.length > 1) {
        throw new Error(`Ambiguous method overload: ${owner}.${method}(${argDescs})`);
      } else if (candidates.length === 1) {
        m = candidates[0];
      } else {
        throw new Error(`Ambiguous method overload: ${owner}.${method}(${argDescs})`);
      }
      return {
        owner,
        paramTypes: m.params.map(p => p.type),
        returnType: m.returnType,
        isStatic: m.isStatic,
        isInterface: decl.kind === "interface" || decl.kind === "annotation",
      };
    }
    const exactSig = lookupKnownMethod(owner, method, argDescs);
    const sig = exactSig
      ?? (hasFunctionalArg(originalArgs) ? findKnownMethodByArity(owner, method, argTypes.length, wantStatic) : undefined);
    if (!sig) continue;
    return {
      owner,
      paramTypes: sig.paramTypes,
      returnType: sig.returnType,
      isStatic: !!sig.isStatic,
      isInterface: !!sig.isInterface,
    };
  }
  return undefined;
}

function resolveUnqualifiedMethodCandidate(
  ctx: CompileContext,
  method: string,
  args: Expr[],
): ResolvedMethodCandidate | undefined {
  const staticResolved = resolveMethodCandidate(ctx, ctx.className, method, args, true);
  const instResolved = resolveMethodCandidate(ctx, ctx.className, method, args, false);
  if (ctx.ownerIsStatic) {
    if (instResolved && !staticResolved) {
      throw new Error(`Cannot call instance method '${method}' from static context`);
    }
    return staticResolved;
  }
  return instResolved ?? staticResolved;
}

function findLocal(ctx: CompileContext, name: string): LocalVar | undefined {
  for (let i = ctx.locals.length - 1; i >= 0; i--) {
    if (ctx.locals[i].name === name) return ctx.locals[i];
  }
  return undefined;
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
      if (["<<", ">>", ">>>"].includes(expr.op)) {
        const lt = inferType(ctx, expr.left);
        return lt === "long" ? "long" : "int";
      }
      if (["&", "|", "^"].includes(expr.op)) {
        const lt = inferType(ctx, expr.left);
        const rt = inferType(ctx, expr.right);
        if (lt === "boolean" && rt === "boolean") return "boolean";
        if (lt === "long" || rt === "long") return "long";
        return "int";
      }
      return "boolean"; // comparison operators
    }
    case "unary": {
      if (expr.op === "~") {
        const t = inferType(ctx, expr.operand);
        return t === "long" ? "long" : "int";
      }
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
        const resolved = resolveMethodCandidate(ctx, ownerClass, expr.method, expr.args, false);
        if (resolved) return resolved.returnType;
      } else {
        const resolved = resolveUnqualifiedMethodCandidate(ctx, expr.method, expr.args);
        if (resolved) return resolved.returnType;
        // Static import-on-demand method
        if (ctx.staticWildcardImports.length > 0) {
          for (const owner of ctx.staticWildcardImports) {
            const resolved = resolveMethodCandidate(ctx, owner, expr.method, expr.args, true);
            if (resolved) return resolved.returnType;
          }
        }
      }
      return { className: "java/lang/Object" };
    }
    case "staticCall": {
      const internalName = expr.className.replace(/\./g, "/");
      const resolved = resolveMethodCandidate(ctx, internalName, expr.method, expr.args, true);
      if (resolved) return resolved.returnType;
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

      // Boolean bitwise operators (non short-circuit)
      if (["&", "|", "^"].includes(expr.op) && leftType === "boolean" && rightType === "boolean") {
        compileExpr(ctx, emitter, expr.left);
        compileExpr(ctx, emitter, expr.right);
        if (expr.op === "&") emitter.emit(0x7e); // iand
        else if (expr.op === "|") emitter.emit(0x80); // ior
        else emitter.emit(0x82); // ixor
        break;
      }

      // Numeric promotion: determine the promoted type
      function promoteNumeric(a: Type, b: Type): Type {
        if (a === "double" || b === "double") return "double";
        if (a === "float" || b === "float") return "float";
        if (a === "long" || b === "long") return "long";
        return "int";
      }
      function promoteIntegral(a: Type, b: Type): Type {
        return (a === "long" || b === "long") ? "long" : "int";
      }

      // Type check for arithmetic operators
      if (["+", "-", "*", "/", "%"].includes(expr.op)) {
        if (!isPrimitiveType(leftType) || !isPrimitiveType(rightType) || leftType === "boolean" || rightType === "boolean") {
          throw new Error(`Operator '${expr.op}' requires numeric operands`);
        }
      }
      if (["&", "|", "^"].includes(expr.op)) {
        if (!isIntegralType(leftType) || !isIntegralType(rightType)) {
          throw new Error(`Operator '${expr.op}' requires integral operands`);
        }
      }
      if (["<<", ">>", ">>>"].includes(expr.op)) {
        if (!isIntegralType(leftType) || !isIntegralType(rightType)) {
          throw new Error(`Operator '${expr.op}' requires integral operands`);
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
        : ["&", "|", "^"].includes(expr.op)
          ? promoteIntegral(leftType, rightType)
        : promoteNumeric(leftType, rightType);

      if (["<<", ">>", ">>>"].includes(expr.op)) {
        const promotedLeft = leftType === "long" ? "long" : "int";
        compileExpr(ctx, emitter, expr.left);
        emitWideningConversion(emitter, leftType, promotedLeft);
        compileExpr(ctx, emitter, expr.right);
        emitWideningConversion(emitter, rightType, "int");
        emitNarrowingConversion(emitter, rightType, "int");
        if (promotedLeft === "long") {
          if (expr.op === "<<") emitter.emit(0x79); // lshl
          else if (expr.op === ">>") emitter.emit(0x7b); // lshr
          else emitter.emit(0x7d); // lushr
        } else {
          if (expr.op === "<<") emitter.emit(0x78); // ishl
          else if (expr.op === ">>") emitter.emit(0x7a); // ishr
          else emitter.emit(0x7c); // iushr
        }
        break;
      }

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
          case "&": emitter.emit(0x7f); break; // land
          case "|": emitter.emit(0x81); break; // lor
          case "^": emitter.emit(0x83); break; // lxor
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
          case "&": emitter.emit(0x7e); break; // iand
          case "|": emitter.emit(0x80); break; // ior
          case "^": emitter.emit(0x82); break; // ixor
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
      if (expr.op === "~") {
        if (!isIntegralType(operandType)) throw new Error("Unary '~' requires integral operand");
        if (operandType === "long") {
          emitter.emitLconst(-1, ctx.cp);
          emitter.emit(0x83); // lxor
        } else {
          emitter.emitIconst(-1) || (() => {
            const cpIdx = ctx.cp.addInteger(-1);
            emitter.emitLdc(cpIdx);
          })();
          emitter.emit(0x82); // ixor
        }
      }
      break;
    }
    case "newExpr": {
      const internalName = resolveClassName(ctx, expr.className);
      const classIdx = ctx.cp.addClass(internalName);
      emitter.emit(0xbb); // new
      emitter.emitU16(classIdx);
      emitter.emit(0x59); // dup

      // Resolve constructor first so lambda/method-ref args receive target type context.
      const resolvedCtor = resolveMethodCandidate(ctx, internalName, "<init>", expr.args, false);
      let desc: string;
      if (resolvedCtor) {
        expr.args.forEach((arg, i) => compileExpr(ctx, emitter, arg, resolvedCtor.paramTypes[i] ?? { className: "java/lang/Object" }));
        const sigArgDescs = resolvedCtor.paramTypes.map(typeToDescriptor).join("");
        desc = "(" + sigArgDescs + ")V";
      } else {
        const argTypes = expr.args.map(a => typeToDescriptor(inferType(ctx, a)));
        for (const arg of expr.args) compileExpr(ctx, emitter, arg);
        desc = "(" + argTypes.join("") + ")V";
      }

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
        const ctorResolved = resolveMethodCandidateByTypes(
          ctx,
          targetClass,
          "<init>",
          ctorParams.map(p => p.type),
          false,
        );
        const ctorTypes = ctorResolved?.paramTypes ?? ctorParams.map(p => p.type);
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
        const staticResolved = resolveMethodCandidateByTypes(ctx, implOwner, expr.method, sig.params, true);
        if (staticResolved) {
          implDescriptor = "(" + staticResolved.paramTypes.map(typeToDescriptor).join("") + ")" + typeToDescriptor(staticResolved.returnType);
          implRefKind = 6;
          implIsInterface = staticResolved.isInterface;
        } else {
          // Unbound instance: first SAM arg is receiver
          const instResolved = resolveMethodCandidateByTypes(
            ctx,
            implOwner,
            expr.method,
            sig.params.slice(1),
            false,
          );
          if (instResolved) {
            implDescriptor = "(" + instResolved.paramTypes.map(typeToDescriptor).join("") + ")" + typeToDescriptor(instResolved.returnType);
            implRefKind = instResolved.isInterface ? 9 : 5;
            implIsInterface = instResolved.isInterface;
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

        const boundResolved = resolveMethodCandidateByTypes(ctx, implOwner, expr.method, sig.params, false);
        if (boundResolved) {
          implDescriptor = "(" + boundResolved.paramTypes.map(typeToDescriptor).join("") + ")" + typeToDescriptor(boundResolved.returnType);
          implRefKind = boundResolved.isInterface ? 9 : 5;
          implIsInterface = boundResolved.isInterface;
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
        const resolved = resolveMethodCandidate(ctx, internalName, expr.method, expr.args, true);
        if (resolved) {
          expr.args.forEach((arg, i) => compileExpr(ctx, emitter, arg, resolved.paramTypes[i] ?? { className: "java/lang/Object" }));
          const sigArgDescs = resolved.paramTypes.map(t => typeToDescriptor(t)).join("");
          const desc = "(" + sigArgDescs + ")" + typeToDescriptor(resolved.returnType);
          const mRef = ctx.cp.addMethodref(internalName, expr.method, desc);
          emitter.emitInvokestatic(mRef, expr.args.length, resolved.returnType !== "void");
        } else {
          const argTypes = expr.args.map(a => typeToDescriptor(inferType(ctx, a)));
          for (const arg of expr.args) compileExpr(ctx, emitter, arg);
          // Fallback descriptor: declared static method in owner class by arity, else Object-returning.
          const userMethod = ctx.classDecls.get(internalName)?.methods
            .find(m => m.name === expr.method && m.isStatic && m.params.length === expr.args.length);
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

    const resolved = resolveMethodCandidate(ctx, ownerClass, expr.method, expr.args, false);

    let desc: string;
    let retType: Type;
    let isInterface = false;

    if (resolved) {
      expr.args.forEach((arg, i) => compileExpr(ctx, emitter, arg, resolved.paramTypes[i] ?? { className: "java/lang/Object" }));
      retType = resolved.returnType;
      const sigArgDescs = resolved.paramTypes.map(t => typeToDescriptor(t)).join("");
      desc = "(" + sigArgDescs + ")" + typeToDescriptor(retType);
      isInterface = resolved.isInterface;
    } else {
      // Check user-defined methods
      const userMethod = ctx.classDecls.get(ownerClass)?.methods
        .find(m => m.name === expr.method && !m.isStatic && m.params.length === expr.args.length);
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
    const resolved = resolveUnqualifiedMethodCandidate(ctx, expr.method, expr.args);
    if (resolved) {
      const desc = "(" + resolved.paramTypes.map(typeToDescriptor).join("") + ")" + typeToDescriptor(resolved.returnType);
      if (resolved.isStatic) {
        const mRef = ctx.cp.addMethodref(resolved.owner, expr.method, desc);
        expr.args.forEach((arg, i) => compileExpr(ctx, emitter, arg, resolved.paramTypes[i] ?? { className: "java/lang/Object" }));
        emitter.emitInvokestatic(mRef, expr.args.length, resolved.returnType !== "void");
      } else {
        if (ctx.ownerIsStatic) {
          throw new Error(`Cannot call instance method '${expr.method}' from static context`);
        }
        emitter.emitAload(0); // this
        expr.args.forEach((arg, i) => compileExpr(ctx, emitter, arg, resolved.paramTypes[i] ?? { className: "java/lang/Object" }));
        if (resolved.isInterface) {
          const mRef = ctx.cp.addInterfaceMethodref(resolved.owner, expr.method, desc);
          emitter.emitInvokeinterface(mRef, expr.args.length, resolved.returnType !== "void");
        } else {
          const mRef = ctx.cp.addMethodref(resolved.owner, expr.method, desc);
          emitter.emitInvokevirtual(mRef, expr.args.length, resolved.returnType !== "void");
        }
      }
    } else if (ctx.staticWildcardImports.length > 0) {
      // Try static-import-on-demand owners in order
      let ownerClass = ctx.staticWildcardImports[0];
      let resolved: ResolvedMethodCandidate | undefined;
      for (const owner of ctx.staticWildcardImports) {
        const candidate = resolveMethodCandidate(ctx, owner, expr.method, expr.args, true);
        if (candidate) {
          ownerClass = owner;
          resolved = candidate;
          break;
        }
      }
      const argTypes = expr.args.map(a => typeToDescriptor(inferType(ctx, a)));
      if (resolved) {
        expr.args.forEach((arg, i) => compileExpr(ctx, emitter, arg, resolved!.paramTypes[i] ?? { className: "java/lang/Object" }));
      } else {
        for (const arg of expr.args) compileExpr(ctx, emitter, arg);
      }
      const retType: Type = resolved?.returnType ?? { className: "java/lang/Object" };
      const sigArgDescs = resolved ? resolved.paramTypes.map(t => typeToDescriptor(t)).join("") : argTypes.join("");
      const desc = "(" + sigArgDescs + ")" + typeToDescriptor(retType);
      const mRef = ctx.cp.addMethodref(ownerClass, expr.method, desc);
      emitter.emitInvokestatic(mRef, expr.args.length, retType !== "void");
    } else {
      throw new Error(`Cannot resolve unqualified method call: ${expr.method}/${expr.args.length}`);
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

type ExitAction = { kind: "monitor"; slot: number } | { kind: "finally"; body: Stmt[] };

function getExitActions(ctx: CompileContext): ExitAction[] {
  const anyCtx = ctx as unknown as { __exitActions?: ExitAction[] };
  if (!anyCtx.__exitActions) anyCtx.__exitActions = [];
  return anyCtx.__exitActions;
}

function emitPendingExitActions(ctx: CompileContext, emitter: BytecodeEmitter, minDepth = 0): void {
  const actions = getExitActions(ctx);
  for (let i = actions.length - 1; i >= minDepth; i--) {
    const action = actions[i];
    if (action.kind === "monitor") {
      emitter.emitAload(action.slot);
      emitter.emitPop(0xc3); // monitorexit
    } else {
      withScopedLocals(ctx, () => {
        for (const s of action.body) compileStmt(ctx, emitter, s);
      });
    }
  }
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
    case "compoundAssign": collectExprIdentifiers(stmt.target, out); collectExprIdentifiers(stmt.value, out); break;
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
    case "assert":
      collectExprIdentifiers(stmt.cond, out);
      if (stmt.message) collectExprIdentifiers(stmt.message, out);
      break;
    case "synchronized":
      collectExprIdentifiers(stmt.monitor, out);
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

const OBJECT_PUBLIC_INSTANCE_METHODS = new Set([
  "toString()",
  "hashCode()",
  "equals(Ljava/lang/Object;)",
  "getClass()",
  "notify()",
  "notifyAll()",
  "wait()",
  "wait(J)",
  "wait(JI)",
]);

function functionalSigForType(ctx: CompileContext, t: Type): { ifaceName: string; sig: FunctionalSig } {
  if (!(typeof t === "object" && "className" in t)) {
    throw new Error("Lambda target type must be a functional interface");
  }
  const ifaceName = resolveClassName(ctx, t.className);
  const abstractMethods = new Map<string, FunctionalSig>();
  const visited = new Set<string>();

  const collectAbstractMethods = (current: string): void => {
    if (visited.has(current)) return;
    visited.add(current);
    const decl = ctx.classDecls.get(current);
    if (!decl) return;
    for (const m of decl.methods) {
      if (m.name === "<init>") continue;
      if (m.isStatic) continue;
      if (!m.isAbstract) continue;
      const params = m.params.map(p => p.type);
      const key = `${m.name}(${params.map(typeToDescriptor).join("")})`;
      if (OBJECT_PUBLIC_INSTANCE_METHODS.has(key)) continue;
      if (!abstractMethods.has(key)) {
        abstractMethods.set(key, { samMethod: m.name, params, returnType: m.returnType });
      }
    }
    for (const parent of decl.interfaces ?? []) {
      collectAbstractMethods(resolveClassName(ctx, parent));
    }
  };

  collectAbstractMethods(ifaceName);
  if (abstractMethods.size === 1) {
    return { ifaceName, sig: Array.from(abstractMethods.values())[0] };
  }

  const known = findKnownFunctionalInterface(ifaceName);
  if (known) return { ifaceName, sig: known };
  throw new Error(`Unsupported functional interface for lambda/method reference: ${ifaceName}`);
}

const BUILTIN_SUPERS: Record<string, string> = {
  "java/lang/String": "java/lang/Object",
  "java/lang/Integer": "java/lang/Object",
  "java/lang/StringBuilder": "java/lang/Object",
  "java/util/ArrayList": "java/lang/Object",
  "java/io/PrintStream": "java/lang/Object",
  "java/lang/Throwable": "java/lang/Object",
  "java/lang/Exception": "java/lang/Throwable",
  "java/lang/RuntimeException": "java/lang/Exception",
  "java/lang/Error": "java/lang/Throwable",
  "java/io/IOException": "java/lang/Exception",
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
    case "compoundAssign": {
      const tempName = (p: string) => `${p}${ctx.nextSlot}`;
      const emitBinaryIntoTarget = (leftExpr: Expr, targetType: Type, targetLabel: string): Type => {
        const binaryExpr: Expr = { kind: "binary", op: stmt.op, left: leftExpr, right: stmt.value };
        const resultType = inferType(ctx, binaryExpr);
        ensureAssignable(ctx, targetType, resultType, targetLabel);
        compileExpr(ctx, emitter, binaryExpr, targetType);
        emitWideningConversion(emitter, resultType, targetType);
        emitNarrowingConversion(emitter, resultType, targetType);
        return resultType;
      };

      if (stmt.target.kind === "ident") {
        const loc = findLocal(ctx, stmt.target.name);
        if (loc) {
          emitBinaryIntoTarget({ kind: "ident", name: stmt.target.name }, loc.type, `local '${stmt.target.name}'`);
          emitStoreLocalByType(emitter, loc.slot, loc.type);
        } else {
          const field = ctx.fields.find(f => f.name === stmt.target.name);
          if (field) {
            emitBinaryIntoTarget({ kind: "ident", name: stmt.target.name }, field.type, `field '${stmt.target.name}'`);
            if (field.isStatic) {
              const fRef = ctx.cp.addFieldref(ctx.className, field.name, typeToDescriptor(field.type));
              emitter.emit(0xb3); // putstatic
              emitter.emitU16(fRef);
            } else {
              const resultSlot = addLocal(ctx, tempName("$ca_res_"), field.type);
              if (emitter.maxLocals <= resultSlot) emitter.maxLocals = resultSlot + 1;
              emitStoreLocalByType(emitter, resultSlot, field.type);
              emitter.emitAload(0);
              emitLoadLocalByType(emitter, resultSlot, field.type);
              const fRef = ctx.cp.addFieldref(ctx.className, field.name, typeToDescriptor(field.type));
              emitter.emit(0xb5); // putfield
              emitter.emitU16(fRef);
            }
          } else {
            throw new Error(`compound assignment target not found: '${stmt.target.name}'`);
          }
        }
      } else if (stmt.target.kind === "fieldAccess") {
        const targetType = inferType(ctx, stmt.target);
        const objType = inferType(ctx, stmt.target.object);
        const ownerClass = typeof objType === "object" && "className" in objType ? objType.className : ctx.className;
        const fieldRef = ctx.cp.addFieldref(ownerClass, stmt.target.field, typeToDescriptor(targetType));
        // Evaluate receiver once
        compileExpr(ctx, emitter, stmt.target.object);
        const objSlot = addLocal(ctx, tempName("$ca_obj_"), objType);
        if (emitter.maxLocals <= objSlot) emitter.maxLocals = objSlot + 1;
        emitter.emitAstore(objSlot);
        // Load current field value once
        emitter.emitAload(objSlot);
        emitter.emit(0xb4); // getfield
        emitter.emitU16(fieldRef);
        const leftName = tempName("$ca_left_");
        const leftSlot = addLocal(ctx, leftName, targetType);
        if (emitter.maxLocals <= leftSlot) emitter.maxLocals = leftSlot + 1;
        emitStoreLocalByType(emitter, leftSlot, targetType);
        emitBinaryIntoTarget({ kind: "ident", name: leftName }, targetType, `field '${stmt.target.field}'`);
        const resSlot = addLocal(ctx, tempName("$ca_res_"), targetType);
        if (emitter.maxLocals <= resSlot) emitter.maxLocals = resSlot + 1;
        emitStoreLocalByType(emitter, resSlot, targetType);
        emitter.emitAload(objSlot);
        emitLoadLocalByType(emitter, resSlot, targetType);
        emitter.emit(0xb5); // putfield
        emitter.emitU16(fieldRef);
      } else if (stmt.target.kind === "arrayAccess") {
        const elemType = inferType(ctx, stmt.target);
        const arrType = inferType(ctx, stmt.target.array);
        const indexType = inferType(ctx, stmt.target.index);
        const emitArrayLoad = (t: Type) => {
          if (t === "int") emitter.emit(0x2e); // iaload
          else if (t === "long") emitter.emit(0x2f); // laload
          else if (t === "float") emitter.emit(0x30); // faload
          else if (t === "double") emitter.emit(0x31); // daload
          else if (t === "byte" || t === "boolean") emitter.emit(0x33); // baload
          else if (t === "char") emitter.emit(0x34); // caload
          else if (t === "short") emitter.emit(0x35); // saload
          else emitter.emit(0x32); // aaload
        };
        const emitArrayStore = (t: Type) => {
          if (t === "int") emitter.emit(0x4f); // iastore
          else if (t === "long") emitter.emit(0x50); // lastore
          else if (t === "float") emitter.emit(0x51); // fastore
          else if (t === "double") emitter.emit(0x52); // dastore
          else if (t === "byte" || t === "boolean") emitter.emit(0x54); // bastore
          else if (t === "char") emitter.emit(0x55); // castore
          else if (t === "short") emitter.emit(0x56); // sastore
          else emitter.emit(0x53); // aastore
        };
        // Evaluate array and index once
        compileExpr(ctx, emitter, stmt.target.array);
        const arrSlot = addLocal(ctx, tempName("$ca_arr_"), arrType);
        if (emitter.maxLocals <= arrSlot) emitter.maxLocals = arrSlot + 1;
        emitter.emitAstore(arrSlot);
        compileExpr(ctx, emitter, stmt.target.index, "int");
        if (indexType === "long") {
          emitter.emit(0x88); // l2i
        } else if (!(indexType === "int" || indexType === "byte" || indexType === "short" || indexType === "char")) {
          throw new Error(`Invalid array index type in compound assignment: ${String(indexType)}`);
        }
        const idxSlot = addLocal(ctx, tempName("$ca_idx_"), "int");
        if (emitter.maxLocals <= idxSlot) emitter.maxLocals = idxSlot + 1;
        emitter.emitIstore(idxSlot);
        emitter.emitAload(arrSlot);
        emitter.emitIload(idxSlot);
        emitArrayLoad(elemType);
        emitter.adjustStackForArrayLoad();
        const leftName = tempName("$ca_left_");
        const leftSlot = addLocal(ctx, leftName, elemType);
        if (emitter.maxLocals <= leftSlot) emitter.maxLocals = leftSlot + 1;
        emitStoreLocalByType(emitter, leftSlot, elemType);
        emitBinaryIntoTarget({ kind: "ident", name: leftName }, elemType, "array element");
        const resSlot = addLocal(ctx, tempName("$ca_res_"), elemType);
        if (emitter.maxLocals <= resSlot) emitter.maxLocals = resSlot + 1;
        emitStoreLocalByType(emitter, resSlot, elemType);
        emitter.emitAload(arrSlot);
        emitter.emitIload(idxSlot);
        emitLoadLocalByType(emitter, resSlot, elemType);
        emitArrayStore(elemType);
      } else {
        throw new Error(`Unsupported compound assignment target of kind '${(stmt.target as Expr).kind}'`);
      }
      break;
    }
    case "exprStmt": {
      // Preserve side effects for ++/-- used as a statement (e.g. field/array increments).
      // Compile as compound assignment so LHS read/modify/write is emitted correctly.
      if (stmt.expr.kind === "postIncrement" || stmt.expr.kind === "preIncrement") {
        const op = stmt.expr.op === "++" ? "+" : "-";
        compileStmt(ctx, emitter, {
          kind: "compoundAssign",
          target: stmt.expr.operand,
          op,
          value: { kind: "intLit", value: 1 },
        });
        break;
      }
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
      emitPendingExitActions(ctx, emitter);
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
      const breakInfo = { label: undefined as string | undefined, patches: [] as number[], exitDepth: getExitActions(ctx).length };
      const continueInfo = { label: undefined as string | undefined, targets: [] as number[], exitDepth: getExitActions(ctx).length };
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
        const breakInfo = { label: undefined as string | undefined, patches: [] as number[], exitDepth: getExitActions(ctx).length };
        const continueInfo = { label: undefined as string | undefined, targets: [] as number[], exitDepth: getExitActions(ctx).length };
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
      const breakInfo = { label: undefined as string | undefined, patches: [] as number[], exitDepth: getExitActions(ctx).length };
      const continueInfo = { label: undefined as string | undefined, targets: [] as number[], exitDepth: getExitActions(ctx).length };
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
        const breakInfo = { label: undefined as string | undefined, patches: [] as number[], exitDepth: getExitActions(ctx).length };
        const continueInfo = { label: undefined as string | undefined, targets: [] as number[], exitDepth: getExitActions(ctx).length };
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
    case "assert": {
      if (inferType(ctx, stmt.cond) !== "boolean") throw new Error("assert condition must be boolean");
      compileExpr(ctx, emitter, stmt.cond);
      const patchOk = emitter.emitBranch(0x9a); // ifne
      const assertionClass = ctx.cp.addClass("java/lang/AssertionError");
      emitter.emit(0xbb); // new
      emitter.emitU16(assertionClass);
      emitter.emit(0x59); // dup
      if (stmt.message) {
        const msgType = inferType(ctx, stmt.message);
        compileExpr(ctx, emitter, stmt.message);
        if (isPrimitiveType(msgType)) {
          const info = BOX_INFO[msgType as string];
          if (!info) throw new Error("assert message boxing failed");
          const boxRef = ctx.cp.addMethodref(info.wrapper, "valueOf", info.desc);
          emitter.emit(0xb8); // invokestatic
          emitter.emitU16(boxRef);
        }
        const initRef = ctx.cp.addMethodref("java/lang/AssertionError", "<init>", "(Ljava/lang/Object;)V");
        emitter.emitInvokespecial(initRef, 1, false);
      } else {
        const initRef = ctx.cp.addMethodref("java/lang/AssertionError", "<init>", "()V");
        emitter.emitInvokespecial(initRef, 0, false);
      }
      emitter.emit(0xbf); // athrow
      emitter.patchBranch(patchOk, emitter.pc);
      break;
    }
    case "synchronized": {
      const monitorType = inferType(ctx, stmt.monitor);
      if (!isRefType(monitorType)) throw new Error("synchronized monitor must be a reference type");
      withScopedLocals(ctx, () => {
        const syncMonName = `$sync_mon_${ctx.nextSlot}`;
        const syncExName = `$sync_ex_${ctx.nextSlot}`;
        compileExpr(ctx, emitter, stmt.monitor);
        const monSlot = addLocal(ctx, syncMonName, monitorType);
        if (emitter.maxLocals <= monSlot) emitter.maxLocals = monSlot + 1;
        emitter.emitAstore(monSlot);

        emitter.emitAload(monSlot);
        emitter.emitPop(0xc2); // monitorenter

        const syncStart = emitter.pc;
        const exitActions = getExitActions(ctx);
        exitActions.push({ kind: "monitor", slot: monSlot });
        withScopedLocals(ctx, () => {
          for (const s of stmt.body) compileStmt(ctx, emitter, s);
        });
        exitActions.pop();
        emitter.emitAload(monSlot);
        emitter.emitPop(0xc3); // monitorexit
        const patchEnd = emitter.emitBranch(0xa7); // goto end

        const handlerPc = emitter.pc;
        emitter.adjustStackForCatch();
        const exSlot = addLocal(ctx, syncExName, { className: "java/lang/Throwable" });
        if (emitter.maxLocals <= exSlot) emitter.maxLocals = exSlot + 1;
        emitter.emitAstore(exSlot);
        emitter.emitAload(monSlot);
        emitter.emitPop(0xc3); // monitorexit
        emitter.emitAload(exSlot);
        emitter.emit(0xbf); // athrow
        emitter.exceptionTable.push({ startPc: syncStart, endPc: handlerPc, handlerPc, catchType: 0 });
        emitter.patchBranch(patchEnd, emitter.pc);
      });
      break;
    }
    case "tryCatch": {
      // Emit try/catch/finally with exception-table based finally handling.
      const tryStart = emitter.pc;
      const exitActions = getExitActions(ctx);
      if (stmt.finallyBody) exitActions.push({ kind: "finally", body: stmt.finallyBody });
      withScopedLocals(ctx, () => {
        for (const s of stmt.tryBody) compileStmt(ctx, emitter, s);
      });
      const tryEnd = emitter.pc;
      const patchEnd = emitter.emitBranch(0xa7); // goto after all catches
      const catchEndPatches: number[] = [patchEnd];
      const exceptionTable: { startPc: number; endPc: number; handlerPc: number; catchType: number }[] = [];
      const catchRanges: { startPc: number; endPc: number }[] = [];
      for (const c of stmt.catches) {
        const handlerPc = emitter.pc;
        const catchClass = resolveClassName(ctx, c.exType);
        const classIdx = ctx.cp.addClass(catchClass);
        exceptionTable.push({ startPc: tryStart, endPc: tryEnd, handlerPc, catchType: classIdx });
        const catchStart = emitter.pc;
        withScopedLocals(ctx, () => {
          // The exception object is on the stack
          emitter.adjustStackForCatch();
          const slot = addLocal(ctx, c.varName, { className: catchClass });
          if (emitter.maxLocals <= slot) emitter.maxLocals = slot + 1;
          emitter.emitAstore(slot);
          for (const s of c.body) compileStmt(ctx, emitter, s);
        });
        const catchEnd = emitter.pc;
        catchRanges.push({ startPc: catchStart, endPc: catchEnd });
        catchEndPatches.push(emitter.emitBranch(0xa7)); // goto end
      }
      if (stmt.finallyBody) {
        // Disable this try's exitAction while emitting finally itself to avoid re-entry.
        exitActions.pop();
        const finallyStart = emitter.pc;
        for (const p of catchEndPatches) emitter.patchBranch(p, finallyStart);
        withScopedLocals(ctx, () => {
          for (const s of stmt.finallyBody!) compileStmt(ctx, emitter, s);
        });
        const patchAfterFinally = emitter.emitBranch(0xa7); // skip exceptional finally handler on normal flow
        const finallyHandlerPc = emitter.pc;
        emitter.adjustStackForCatch();
        const exSlot = addLocal(ctx, `\u0001finally_ex_${ctx.nextSlot}`, { className: "java/lang/Throwable" });
        if (emitter.maxLocals <= exSlot) emitter.maxLocals = exSlot + 1;
        emitter.emitAstore(exSlot);
        withScopedLocals(ctx, () => {
          for (const s of stmt.finallyBody!) compileStmt(ctx, emitter, s);
        });
        emitter.emitAload(exSlot);
        emitter.emit(0xbf); // athrow
        exceptionTable.push({ startPc: tryStart, endPc: tryEnd, handlerPc: finallyHandlerPc, catchType: 0 });
        for (const r of catchRanges) {
          if (r.endPc > r.startPc) {
            exceptionTable.push({ startPc: r.startPc, endPc: r.endPc, handlerPc: finallyHandlerPc, catchType: 0 });
          }
        }
        emitter.patchBranch(patchAfterFinally, emitter.pc);
      } else {
        for (const p of catchEndPatches) emitter.patchBranch(p, emitter.pc);
      }
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
        emitPendingExitActions(ctx, emitter, info.exitDepth ?? 0);
        info.patches.push(emitter.emitBranch(0xa7));
      } else {
        const info = ctx.breakPatches[ctx.breakPatches.length - 1];
        if (!info) throw new Error("break outside of loop/switch");
        emitPendingExitActions(ctx, emitter, info.exitDepth ?? 0);
        info.patches.push(emitter.emitBranch(0xa7));
      }
      break;
    }
    case "continue": {
      if (stmt.label) {
        const info = [...ctx.continuePatches].reverse().find(c => c.label === stmt.label);
        if (!info) throw new Error(`continue label '${stmt.label}' not found`);
        emitPendingExitActions(ctx, emitter, info.exitDepth ?? 0);
        info.targets.push(emitter.emitBranch(0xa7));
      } else {
        const info = ctx.continuePatches[ctx.continuePatches.length - 1];
        if (!info) throw new Error("continue outside of loop");
        emitPendingExitActions(ctx, emitter, info.exitDepth ?? 0);
        info.targets.push(emitter.emitBranch(0xa7));
      }
      break;
    }
    case "labeled": {
      const breakInfo = { label: stmt.label, patches: [] as number[], exitDepth: getExitActions(ctx).length };
      const continueInfo = { label: stmt.label, targets: [] as number[], exitDepth: getExitActions(ctx).length };
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
    case "compoundAssign": return exprHasSuperCall(stmt.target) || exprHasSuperCall(stmt.value);
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
    case "assert": return exprHasSuperCall(stmt.cond) || !!stmt.message && exprHasSuperCall(stmt.message);
    case "synchronized": return exprHasSuperCall(stmt.monitor) || stmt.body.some(stmtHasSuperCall);
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

function resolveClassNameInDecl(classDecl: ClassDecl, classDecls: Map<string, ClassDecl>, name: string): string {
  if (name.includes("/")) return name;
  if (name.includes(".")) return name.replace(/\./g, "/");
  const explicit = classDecl.importMap.get(name);
  if (explicit) return explicit;
  if (classDecls.has(name)) return name;
  if (/^[A-Z]/.test(name) && classDecl.packageImports.length > 0) {
    for (const pkg of classDecl.packageImports) {
      const candidate = `${pkg}/${name}`;
      if (hasKnownMethodOwnerPrefix(candidate)) return candidate;
    }
    return `${classDecl.packageImports[0]}/${name}`;
  }
  return name;
}

function isClassSupertypeInMaps(
  classSupers: Map<string, string>,
  maybeSuper: string,
  maybeSub: string,
): boolean {
  if (maybeSuper === maybeSub) return true;
  let cur = maybeSub;
  const seen = new Set<string>();
  while (!seen.has(cur)) {
    seen.add(cur);
    const next = classSupers.get(cur) ?? BUILTIN_SUPERS[cur];
    if (!next) return false;
    if (next === maybeSuper) return true;
    cur = next;
  }
  return false;
}

function isCheckedExceptionType(classSupers: Map<string, string>, exClass: string): boolean {
  const isThrowable = isClassSupertypeInMaps(classSupers, "java/lang/Throwable", exClass);
  const isRuntime = isClassSupertypeInMaps(classSupers, "java/lang/RuntimeException", exClass);
  const isError = isClassSupertypeInMaps(classSupers, "java/lang/Error", exClass);
  // Fail closed for unknown refs: unless proven RuntimeException/Error, treat as checked.
  if (!isThrowable) return true;
  if (isRuntime || isError) return false;
  return true;
}

function findDeclaredMethodByArity(
  classDecls: Map<string, ClassDecl>,
  classSupers: Map<string, string>,
  ownerClass: string,
  methodName: string,
  argTypes: Type[],
  wantStatic: boolean | undefined,
): MethodDecl | undefined {
  const arity = argTypes.length;
  const argDescs = argTypes.map(typeToDescriptor).join("");
  const pick = (decl: ClassDecl): MethodDecl | undefined => {
    const candidates = decl.methods.filter(m => m.name === methodName
      && m.params.length === arity
      && (wantStatic === undefined || m.isStatic === wantStatic));
    if (candidates.length === 0) return undefined;
    const exactMatches = candidates.filter(m => m.params.map(p => typeToDescriptor(p.type)).join("") === argDescs);
    if (exactMatches.length === 1) return exactMatches[0];
    if (exactMatches.length > 1) throw new Error(`Ambiguous method overload in checked-exception analysis: ${ownerClass}.${methodName}(${argDescs})`);
    if (candidates.length === 1) return candidates[0];
    throw new Error(`Ambiguous method overload in checked-exception analysis: ${ownerClass}.${methodName}(${argDescs})`);
  };

  const classChain: string[] = [];
  const seenClass = new Set<string>();
  let cur: string | undefined = ownerClass;
  while (cur && !seenClass.has(cur)) {
    seenClass.add(cur);
    classChain.push(cur);
    cur = classSupers.get(cur) ?? BUILTIN_SUPERS[cur];
  }
  for (const cls of classChain) {
    const decl = classDecls.get(cls);
    if (decl) {
      const found = pick(decl);
      if (found) return found;
    }
  }

  const queue: string[] = [];
  const seenIface = new Set<string>();
  for (const cls of classChain) {
    const decl = classDecls.get(cls);
    if (!decl) continue;
    for (const itf of decl.interfaces ?? []) queue.push(itf);
  }
  while (queue.length > 0) {
    const itf = queue.shift()!;
    if (seenIface.has(itf)) continue;
    seenIface.add(itf);
    const decl = classDecls.get(itf);
    if (!decl) continue;
    const found = pick(decl);
    if (found) return found;
    for (const parent of decl.interfaces ?? []) queue.push(parent);
  }
  return undefined;
}

function collectExprCheckedExceptions(
  classDecl: ClassDecl,
  classDecls: Map<string, ClassDecl>,
  classSupers: Map<string, string>,
  expr: Expr,
  localTypes: Map<string, Type>,
  ownerIsStatic: boolean,
): Set<string> {
  const inferArgTypesForChecks = (args: Expr[]): Type[] => {
    const inferCtx = {
      className: classDecl.name,
      superClass: classDecl.superClass,
      fields: classDecl.fields,
      inheritedFields: [],
      locals: Array.from(localTypes, ([name, type], idx) => ({ name, type, slot: idx })),
      importMap: classDecl.importMap,
      packageImports: classDecl.packageImports,
      staticWildcardImports: classDecl.staticWildcardImports,
      classSupers,
      classDecls,
      allMethods: classDecl.methods,
      ownerIsStatic,
    } as unknown as CompileContext;
    return args.map(a => inferType(inferCtx, a));
  };

  const out = new Set<string>();
  const merge = (s: Set<string>) => { for (const e of s) out.add(e); };
  const addThrown = (name: string | undefined) => {
    if (!name) return;
    if (!isCheckedExceptionType(classSupers, name)) return;
    out.add(name);
  };
  const fromMethod = (owner: string, name: string, argTypes: Type[], wantStatic: boolean | undefined) => {
    const m = findDeclaredMethodByArity(classDecls, classSupers, owner, name, argTypes, wantStatic);
    for (const t of m?.throwsTypes ?? []) {
      const resolved = resolveClassNameInDecl(classDecl, classDecls, t);
      addThrown(resolved);
    }
  };

  switch (expr.kind) {
    case "binary":
      merge(collectExprCheckedExceptions(classDecl, classDecls, classSupers, expr.left, localTypes, ownerIsStatic));
      merge(collectExprCheckedExceptions(classDecl, classDecls, classSupers, expr.right, localTypes, ownerIsStatic));
      break;
    case "unary":
      merge(collectExprCheckedExceptions(classDecl, classDecls, classSupers, expr.operand, localTypes, ownerIsStatic));
      break;
    case "call":
      if (expr.object) merge(collectExprCheckedExceptions(classDecl, classDecls, classSupers, expr.object, localTypes, ownerIsStatic));
      for (const a of expr.args) merge(collectExprCheckedExceptions(classDecl, classDecls, classSupers, a, localTypes, ownerIsStatic));
      const callArgTypes = inferArgTypesForChecks(expr.args);
      if (!expr.object) {
        fromMethod(classDecl.name, expr.method, callArgTypes, undefined);
      } else if (expr.object.kind === "this") {
        fromMethod(classDecl.name, expr.method, callArgTypes, false);
      } else if (expr.object.kind === "newExpr") {
        const owner = resolveClassNameInDecl(classDecl, classDecls, expr.object.className);
        fromMethod(owner, expr.method, callArgTypes, false);
      } else if (expr.object.kind === "ident") {
        const t = localTypes.get(expr.object.name);
        if (t && typeof t === "object" && "className" in t) {
          fromMethod(resolveClassNameInDecl(classDecl, classDecls, t.className), expr.method, callArgTypes, false);
        } else {
          const owner = resolveClassNameInDecl(classDecl, classDecls, expr.object.name);
          const looksLikeClassRef = !localTypes.has(expr.object.name)
            && (/^[A-Z]/.test(expr.object.name) || classDecl.importMap.has(expr.object.name) || owner !== expr.object.name);
          if (looksLikeClassRef) fromMethod(owner, expr.method, callArgTypes, true);
        }
      }
      break;
    case "staticCall":
      for (const a of expr.args) merge(collectExprCheckedExceptions(classDecl, classDecls, classSupers, a, localTypes, ownerIsStatic));
      fromMethod(
        resolveClassNameInDecl(classDecl, classDecls, expr.className),
        expr.method,
        inferArgTypesForChecks(expr.args),
        true,
      );
      break;
    case "fieldAccess":
      merge(collectExprCheckedExceptions(classDecl, classDecls, classSupers, expr.object, localTypes, ownerIsStatic));
      break;
    case "newExpr": {
      for (const a of expr.args) merge(collectExprCheckedExceptions(classDecl, classDecls, classSupers, a, localTypes, ownerIsStatic));
      const owner = resolveClassNameInDecl(classDecl, classDecls, expr.className);
      const ctorArgTypes = inferArgTypesForChecks(expr.args);
      const ctor = findDeclaredMethodByArity(classDecls, classSupers, owner, "<init>", ctorArgTypes, false);
      for (const t of ctor?.throwsTypes ?? []) addThrown(resolveClassNameInDecl(classDecl, classDecls, t));
      break;
    }
    case "cast":
      merge(collectExprCheckedExceptions(classDecl, classDecls, classSupers, expr.expr, localTypes, ownerIsStatic));
      break;
    case "postIncrement":
    case "preIncrement":
      merge(collectExprCheckedExceptions(classDecl, classDecls, classSupers, expr.operand, localTypes, ownerIsStatic));
      break;
    case "instanceof":
      merge(collectExprCheckedExceptions(classDecl, classDecls, classSupers, expr.expr, localTypes, ownerIsStatic));
      break;
    case "arrayAccess":
      merge(collectExprCheckedExceptions(classDecl, classDecls, classSupers, expr.array, localTypes, ownerIsStatic));
      merge(collectExprCheckedExceptions(classDecl, classDecls, classSupers, expr.index, localTypes, ownerIsStatic));
      break;
    case "arrayLit":
      for (const e of expr.elements) merge(collectExprCheckedExceptions(classDecl, classDecls, classSupers, e, localTypes, ownerIsStatic));
      break;
    case "newArray":
      merge(collectExprCheckedExceptions(classDecl, classDecls, classSupers, expr.size, localTypes, ownerIsStatic));
      break;
    case "ternary":
      merge(collectExprCheckedExceptions(classDecl, classDecls, classSupers, expr.cond, localTypes, ownerIsStatic));
      merge(collectExprCheckedExceptions(classDecl, classDecls, classSupers, expr.thenExpr, localTypes, ownerIsStatic));
      merge(collectExprCheckedExceptions(classDecl, classDecls, classSupers, expr.elseExpr, localTypes, ownerIsStatic));
      break;
    case "switchExpr":
      merge(collectExprCheckedExceptions(classDecl, classDecls, classSupers, expr.selector, localTypes, ownerIsStatic));
      for (const c of expr.cases) {
        if (c.guard) merge(collectExprCheckedExceptions(classDecl, classDecls, classSupers, c.guard, localTypes, ownerIsStatic));
        if (c.expr) merge(collectExprCheckedExceptions(classDecl, classDecls, classSupers, c.expr, localTypes, ownerIsStatic));
        if (c.stmts) merge(collectStmtListCheckedExceptions(classDecl, classDecls, classSupers, c.stmts, new Map(localTypes), ownerIsStatic));
      }
      break;
    case "methodRef":
      merge(collectExprCheckedExceptions(classDecl, classDecls, classSupers, expr.target, localTypes, ownerIsStatic));
      break;
    default:
      break;
  }
  return out;
}

function collectStmtListCheckedExceptions(
  classDecl: ClassDecl,
  classDecls: Map<string, ClassDecl>,
  classSupers: Map<string, string>,
  stmts: Stmt[],
  localTypes: Map<string, Type>,
  ownerIsStatic: boolean,
): Set<string> {
  const out = new Set<string>();
  const merge = (s: Set<string>) => { for (const e of s) out.add(e); };
  for (const s of stmts) {
    merge(collectStmtCheckedExceptions(classDecl, classDecls, classSupers, s, localTypes, ownerIsStatic));
  }
  return out;
}

function collectStmtCheckedExceptions(
  classDecl: ClassDecl,
  classDecls: Map<string, ClassDecl>,
  classSupers: Map<string, string>,
  stmt: Stmt,
  localTypes: Map<string, Type>,
  ownerIsStatic: boolean,
): Set<string> {
  const out = new Set<string>();
  const merge = (s: Set<string>) => { for (const e of s) out.add(e); };
  const addThrown = (name: string | undefined) => {
    if (!name) return;
    if (!isCheckedExceptionType(classSupers, name)) return;
    out.add(name);
  };
  const inferExprTypeForChecks = (expr: Expr): Type => {
    const inferCtx = {
      className: classDecl.name,
      superClass: classDecl.superClass,
      fields: classDecl.fields,
      inheritedFields: [],
      locals: Array.from(localTypes, ([name, type], idx) => ({ name, type, slot: idx })),
      importMap: classDecl.importMap,
      packageImports: classDecl.packageImports,
      staticWildcardImports: classDecl.staticWildcardImports,
      classSupers,
      classDecls,
      allMethods: classDecl.methods,
      ownerIsStatic,
    } as unknown as CompileContext;
    return inferType(inferCtx, expr);
  };
  switch (stmt.kind) {
    case "varDecl":
      if (stmt.init) merge(collectExprCheckedExceptions(classDecl, classDecls, classSupers, stmt.init, localTypes, ownerIsStatic));
      localTypes.set(stmt.name, stmt.type);
      break;
    case "assign":
      merge(collectExprCheckedExceptions(classDecl, classDecls, classSupers, stmt.target, localTypes, ownerIsStatic));
      merge(collectExprCheckedExceptions(classDecl, classDecls, classSupers, stmt.value, localTypes, ownerIsStatic));
      break;
    case "compoundAssign":
      merge(collectExprCheckedExceptions(classDecl, classDecls, classSupers, stmt.target, localTypes, ownerIsStatic));
      merge(collectExprCheckedExceptions(classDecl, classDecls, classSupers, stmt.value, localTypes, ownerIsStatic));
      break;
    case "exprStmt":
      merge(collectExprCheckedExceptions(classDecl, classDecls, classSupers, stmt.expr, localTypes, ownerIsStatic));
      break;
    case "return":
      if (stmt.value) merge(collectExprCheckedExceptions(classDecl, classDecls, classSupers, stmt.value, localTypes, ownerIsStatic));
      break;
    case "yield":
      merge(collectExprCheckedExceptions(classDecl, classDecls, classSupers, stmt.value, localTypes, ownerIsStatic));
      break;
    case "if":
      merge(collectExprCheckedExceptions(classDecl, classDecls, classSupers, stmt.cond, localTypes, ownerIsStatic));
      merge(collectStmtListCheckedExceptions(classDecl, classDecls, classSupers, stmt.then, new Map(localTypes), ownerIsStatic));
      if (stmt.else_) merge(collectStmtListCheckedExceptions(classDecl, classDecls, classSupers, stmt.else_, new Map(localTypes), ownerIsStatic));
      break;
    case "while":
      merge(collectExprCheckedExceptions(classDecl, classDecls, classSupers, stmt.cond, localTypes, ownerIsStatic));
      merge(collectStmtListCheckedExceptions(classDecl, classDecls, classSupers, stmt.body, new Map(localTypes), ownerIsStatic));
      break;
    case "for": {
      const scoped = new Map(localTypes);
      if (stmt.init) merge(collectStmtCheckedExceptions(classDecl, classDecls, classSupers, stmt.init, scoped, ownerIsStatic));
      if (stmt.cond) merge(collectExprCheckedExceptions(classDecl, classDecls, classSupers, stmt.cond, scoped, ownerIsStatic));
      if (stmt.update) merge(collectStmtCheckedExceptions(classDecl, classDecls, classSupers, stmt.update, scoped, ownerIsStatic));
      merge(collectStmtListCheckedExceptions(classDecl, classDecls, classSupers, stmt.body, scoped, ownerIsStatic));
      break;
    }
    case "switch":
      merge(collectExprCheckedExceptions(classDecl, classDecls, classSupers, stmt.selector, localTypes, ownerIsStatic));
      for (const c of stmt.cases) {
        if (c.guard) merge(collectExprCheckedExceptions(classDecl, classDecls, classSupers, c.guard, localTypes, ownerIsStatic));
        if (c.expr) merge(collectExprCheckedExceptions(classDecl, classDecls, classSupers, c.expr, localTypes, ownerIsStatic));
        if (c.stmts) merge(collectStmtListCheckedExceptions(classDecl, classDecls, classSupers, c.stmts, new Map(localTypes), ownerIsStatic));
      }
      break;
    case "doWhile":
      merge(collectStmtListCheckedExceptions(classDecl, classDecls, classSupers, stmt.body, new Map(localTypes), ownerIsStatic));
      merge(collectExprCheckedExceptions(classDecl, classDecls, classSupers, stmt.cond, localTypes, ownerIsStatic));
      break;
    case "forEach":
      merge(collectExprCheckedExceptions(classDecl, classDecls, classSupers, stmt.iterable, localTypes, ownerIsStatic));
      merge(collectStmtListCheckedExceptions(classDecl, classDecls, classSupers, stmt.body, new Map(localTypes), ownerIsStatic));
      break;
    case "assert":
      merge(collectExprCheckedExceptions(classDecl, classDecls, classSupers, stmt.cond, localTypes, ownerIsStatic));
      if (stmt.message) merge(collectExprCheckedExceptions(classDecl, classDecls, classSupers, stmt.message, localTypes, ownerIsStatic));
      break;
    case "synchronized":
      merge(collectExprCheckedExceptions(classDecl, classDecls, classSupers, stmt.monitor, localTypes, ownerIsStatic));
      merge(collectStmtListCheckedExceptions(classDecl, classDecls, classSupers, stmt.body, new Map(localTypes), ownerIsStatic));
      break;
    case "throw": {
      merge(collectExprCheckedExceptions(classDecl, classDecls, classSupers, stmt.expr, localTypes, ownerIsStatic));
      if (stmt.expr.kind === "newExpr") {
        addThrown(resolveClassNameInDecl(classDecl, classDecls, stmt.expr.className));
      } else if (stmt.expr.kind === "ident") {
        if (stmt.expr.name.startsWith("\u0001twr_")) break;
        const t = localTypes.get(stmt.expr.name);
        if (t && typeof t === "object" && "className" in t) {
          addThrown(resolveClassNameInDecl(classDecl, classDecls, t.className));
        }
      } else {
        const exprType = (stmt.expr as any).staticType ?? (stmt.expr as any).type ?? inferExprTypeForChecks(stmt.expr);
        if (exprType && typeof exprType === "object" && "className" in exprType) {
          addThrown(resolveClassNameInDecl(classDecl, classDecls, exprType.className));
        }
      }
      break;
    }
    case "tryCatch": {
      const thrownTry = collectStmtListCheckedExceptions(classDecl, classDecls, classSupers, stmt.tryBody, new Map(localTypes), ownerIsStatic);
      for (const c of stmt.catches) {
        const catchType = resolveClassNameInDecl(classDecl, classDecls, c.exType);
        for (const e of Array.from(thrownTry)) {
          if (isClassSupertypeInMaps(classSupers, catchType, e)) thrownTry.delete(e);
        }
      }
      merge(thrownTry);
      for (const c of stmt.catches) {
        const catchScope = new Map(localTypes);
        catchScope.set(c.varName, { className: resolveClassNameInDecl(classDecl, classDecls, c.exType) });
        merge(collectStmtListCheckedExceptions(classDecl, classDecls, classSupers, c.body, catchScope, ownerIsStatic));
      }
      if (stmt.finallyBody) {
        merge(collectStmtListCheckedExceptions(classDecl, classDecls, classSupers, stmt.finallyBody, new Map(localTypes), ownerIsStatic));
      }
      break;
    }
    case "labeled":
      merge(collectStmtCheckedExceptions(classDecl, classDecls, classSupers, stmt.stmt, new Map(localTypes), ownerIsStatic));
      break;
    case "block":
      merge(collectStmtListCheckedExceptions(classDecl, classDecls, classSupers, stmt.stmts, new Map(localTypes), ownerIsStatic));
      break;
    default:
      break;
  }
  return out;
}

function validateCheckedExceptions(
  classDecl: ClassDecl,
  method: MethodDecl,
  classDecls: Map<string, ClassDecl>,
  classSupers: Map<string, string>,
): void {
  if (method.name.startsWith("lambda$")) return;
  const localTypes = new Map<string, Type>();
  if (!method.isStatic) localTypes.set("this", { className: classDecl.name });
  for (const p of method.params) localTypes.set(p.name, p.type);
  const uncaught = collectStmtListCheckedExceptions(classDecl, classDecls, classSupers, method.body, localTypes, method.isStatic);
  const declared = (method.throwsTypes ?? []).map(t => resolveClassNameInDecl(classDecl, classDecls, t));
  for (const ex of uncaught) {
    const covered = declared.some(d => isClassSupertypeInMaps(classSupers, d, ex));
    if (!covered) {
      throw new Error(`Unhandled checked exception in ${classDecl.name}.${method.name}: ${ex}`);
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

export function compile(source: string, implicitClassName?: string): Uint8Array {
  const tokens = lex(source);
  const classDecls = flattenClasses(parseAll(tokens, implicitClassName));
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
  const ifaceIndexes = (classDecl.interfaces ?? []).map(i => cp.addClass(i));

  const isInterfaceLike = classDecl.kind === "interface" || classDecl.kind === "annotation";
  const isEnumClass = classDecl.kind === "enum";
  // Add default constructor if none exists (not for interfaces/annotations)
  const hasInit = classDecl.methods.some(m => m.name === "<init>");
  if (!isInterfaceLike && !hasInit) {
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
    code?: number[];
    maxStack?: number;
    maxLocals?: number;
    hasCode: boolean;
    exceptionTable?: { startPc: number; endPc: number; handlerPc: number; catchType: number }[];
  }[] = [];

  const methodQueue: MethodDecl[] = [...classDecl.methods];
  let generatedDrain = 0;
  for (let mi = 0; mi < methodQueue.length; mi++) {
    const method = methodQueue[mi];
    validateConstructorBody(method);
    validateCheckedExceptions(classDecl, method, classDecls, classSupers);
    const nameIdx = cp.addUtf8(method.name);
    const desc = isEnumClass && method.name === "<init>"
      ? enumConstructorDescriptor(method.params.map(p => p.type))
      : methodDescriptor(method.params, method.returnType);
    const descIdx = cp.addUtf8(desc);

    let accessFlags = 0x0001; // ACC_PUBLIC
    if (method.name === "<init>" && classDecl.kind === "enum") accessFlags = 0x0002; // ACC_PRIVATE
    if (method.isStatic) accessFlags |= 0x0008; // ACC_STATIC
    const methodIsAbstract = method.name !== "<init>" && !!method.isAbstract;
    if (method.isSynchronized && !methodIsAbstract) accessFlags |= 0x0020; // ACC_SYNCHRONIZED
    if (methodIsAbstract) accessFlags |= 0x0400; // ACC_ABSTRACT

    if (methodIsAbstract) {
      compiledMethods.push({ nameIdx, descIdx, accessFlags, hasCode: false });
    } else if (method.name === "<init>") {
      const emitter = new BytecodeEmitter();
      const hasSuperCall = method.body.length > 0
        && method.body[0].kind === "exprStmt"
        && method.body[0].expr.kind === "superCall";
      if (isEnumClass) {
        if (hasSuperCall) {
          throw new Error("explicit super(...) call in enum constructor is not supported");
        }
        const superInitRef = cp.addMethodref("java/lang/Enum", "<init>", "(Ljava/lang/String;I)V");
        emitter.emitAload(0); // this
        emitter.emitAload(1); // enum name (synthetic)
        emitter.emitIload(2); // ordinal (synthetic)
        emitter.emitInvokespecial(superInitRef, 2, false);
      } else if (!hasSuperCall) {
        const superInitRef = cp.addMethodref(classDecl.superClass, "<init>", "()V");
        emitter.emitAload(0); // this
        emitter.emitInvokespecial(superInitRef, 0, false);
      }

      const initParamSlotBase = isEnumClass ? 3 : 1;
      // Set up locals for constructor params (slot 0 = this, enum adds synthetic name/ordinal)
      const initCtx: CompileContext = {
        className: classDecl.name, superClass: classDecl.superClass, cp, method,
        locals: method.params.map((p, i) => ({ name: p.name, type: p.type, slot: i + initParamSlotBase })),
        nextSlot: method.params.length + initParamSlotBase,
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
      const minLocals = method.params.length + initParamSlotBase;
      if (emitter.maxLocals < minLocals) emitter.maxLocals = minLocals;

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
        hasCode: true,
        code: emitter.code,
        maxStack: Math.max(emitter.maxStack, 4),
        maxLocals: Math.max(emitter.maxLocals, minLocals),
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
        hasCode: true,
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

  // Synthesize static initialization for enum constants and static field initializers.
  const hasClinit = classDecl.methods.some(m => m.name === "<clinit>");
  const hasStaticFieldInitializers = classDecl.fields.some(f => f.isStatic && !!f.initializer);
  const enumConstantCount = isEnumClass ? classDecl.fields.filter(f => !!f.isEnumConstant).length : 0;
  if (!hasClinit && (hasStaticFieldInitializers || enumConstantCount > 0)) {
    const clinitMethod: MethodDecl = { name: "<clinit>", returnType: "void", params: [], body: [], isStatic: true };
    const clinitCtx: CompileContext = {
      className: classDecl.name,
      superClass: classDecl.superClass,
      cp,
      method: clinitMethod,
      locals: [],
      nextSlot: 0,
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
      ownerIsStatic: true,
      breakPatches: [],
      continuePatches: [],
    };
    const emitter = new BytecodeEmitter();
    const classIdx = cp.addClass(classDecl.name);
    const enumCtors = classDecl.methods.filter(m => m.name === "<init>");
    let enumOrdinal = 0;
    for (const field of classDecl.fields) {
      if (!field.isStatic) continue;
      if (field.isEnumConstant) {
        const init = field.initializer;
        if (!init || init.kind !== "newExpr") {
          throw new Error(`Enum constant ${field.name} must have initializer`);
        }
        const ctor = enumCtors.find(m => m.params.length === init.args.length);
        if (!ctor) {
          throw new Error(`No enum constructor matches ${field.name}(${init.args.length} args)`);
        }
        emitter.emit(0xbb); // new
        emitter.emitU16(classIdx);
        emitter.emit(0x59); // dup
        emitter.emitLdc(cp.addString(field.name));
        emitter.emitIconst(enumOrdinal++);
        for (let ai = 0; ai < init.args.length; ai++) {
          compileExpr(clinitCtx, emitter, init.args[ai], ctor.params[ai]?.type);
        }
        const ctorDesc = enumConstructorDescriptor(ctor.params.map(p => p.type));
        const ctorRef = cp.addMethodref(classDecl.name, "<init>", ctorDesc);
        emitter.emitInvokespecial(ctorRef, 2 + init.args.length, false);
        const fieldRef = cp.addFieldref(classDecl.name, field.name, typeToDescriptor(field.type));
        emitter.emit(0xb3); // putstatic
        emitter.emitU16(fieldRef);
        continue;
      }
      if (!field.initializer) continue;
      compileExpr(clinitCtx, emitter, field.initializer, field.type);
      const fieldRef = cp.addFieldref(classDecl.name, field.name, typeToDescriptor(field.type));
      emitter.emit(0xb3); // putstatic
      emitter.emitU16(fieldRef);
    }
    emitter.emit(0xb1); // return
    compiledMethods.push({
      nameIdx: cp.addUtf8("<clinit>"),
      descIdx: cp.addUtf8("()V"),
      accessFlags: 0x0008, // ACC_STATIC
      hasCode: true,
      code: emitter.code,
      maxStack: Math.max(emitter.maxStack, 6),
      maxLocals: 0,
    });
  }

  // Build fields
  const compiledFields: { nameIdx: number; descIdx: number; accessFlags: number }[] = [];
  for (const field of classDecl.fields) {
    const nameIdx = cp.addUtf8(field.name);
    const descIdx = cp.addUtf8(typeToDescriptor(field.type));
    let accessFlags = field.isPrivate ? 0x0002 : 0x0001; // ACC_PRIVATE/ACC_PUBLIC
    if (field.isStatic) accessFlags |= 0x0008;
    if (field.isFinal) accessFlags |= 0x0010;
    if (isEnumClass && field.isEnumConstant) accessFlags |= 0x4000; // ACC_ENUM
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

  // Access flags
  const hasAbstractMethods = classDecl.methods.some(m => !!m.isAbstract);
  let classFlags: number;
  if (classDecl.kind === "annotation") classFlags = 0x2601; // PUBLIC | INTERFACE | ABSTRACT | ANNOTATION
  else if (classDecl.kind === "interface") classFlags = 0x0601; // PUBLIC | INTERFACE | ABSTRACT
  else if (classDecl.kind === "enum") classFlags = 0x4031; // PUBLIC | SUPER | FINAL | ENUM
  else if (classDecl.isImplicit) classFlags = 0x0031; // implicit (compact source) classes are final
  else classFlags = classDecl.isRecord ? 0x0031 : 0x0021; // record classes are final
  if (hasAbstractMethods && classDecl.kind !== "interface" && classDecl.kind !== "annotation") {
    classFlags &= ~0x0010; // clear FINAL
    classFlags |= 0x0400;  // set ABSTRACT
  }
  out.push((classFlags >> 8) & 0xff, classFlags & 0xff);
  // this_class
  out.push((thisClassIdx >> 8) & 0xff, thisClassIdx & 0xff);
  // super_class
  out.push((superClassIdx >> 8) & 0xff, superClassIdx & 0xff);
  // interfaces_count
  out.push((ifaceIndexes.length >> 8) & 0xff, ifaceIndexes.length & 0xff);
  for (const ii of ifaceIndexes) {
    out.push((ii >> 8) & 0xff, ii & 0xff);
  }

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
    if (!m.hasCode) {
      out.push(0x00, 0x00); // attributes_count = 0
    } else {
      out.push(0x00, 0x01); // attributes_count = 1 (Code)
      out.push((codeAttrName >> 8) & 0xff, codeAttrName & 0xff);
      const codeLen = m.code!.length;
      const exTblLen = m.exceptionTable ? m.exceptionTable.length : 0;
      const attrLen = 2 + 2 + 4 + codeLen + 2 + exTblLen * 8 + 2; // max_stack + max_locals + code_length + code + exception_table + attributes_count
      out.push((attrLen >> 24) & 0xff, (attrLen >> 16) & 0xff, (attrLen >> 8) & 0xff, attrLen & 0xff);
      out.push(((m.maxStack ?? 0) >> 8) & 0xff, (m.maxStack ?? 0) & 0xff);
      out.push(((m.maxLocals ?? 0) >> 8) & 0xff, (m.maxLocals ?? 0) & 0xff);
      out.push((codeLen >> 24) & 0xff, (codeLen >> 16) & 0xff, (codeLen >> 8) & 0xff, codeLen & 0xff);
      out.push(...m.code!);
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
