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
