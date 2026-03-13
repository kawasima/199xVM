// class-reader.ts — Extract metadata from .class binaries and bundle.bin files.
//
// Used to auto-generate the method registry (KNOWN_METHODS) from the JDK shim
// bundle, eliminating the need for a hand-maintained lookup table.

import type { Type } from "./javac/ast.js";

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

export interface ClassMeta {
  name: string;           // "java/util/ArrayList"
  accessFlags: number;
  superClass: string;
  interfaces: string[];
  fields: FieldMeta[];
  methods: MethodMeta[];
}

export interface FieldMeta {
  name: string;
  descriptor: string;
  accessFlags: number;
}

export interface MethodMeta {
  name: string;
  descriptor: string;     // "(ILjava/lang/Object;)Ljava/lang/Object;"
  accessFlags: number;
}

export interface MethodSig {
  owner: string;
  returnType: Type;
  paramTypes: Type[];
  isInterface?: boolean;
}

// ---------------------------------------------------------------------------
// .class file parser — extract metadata only (no bytecode)
// ---------------------------------------------------------------------------

export function parseClassMeta(data: Uint8Array): ClassMeta {
  const dv = new DataView(data.buffer, data.byteOffset, data.byteLength);
  let pos = 0;

  function u8()  { return dv.getUint8(pos++); }
  function u16() { const v = dv.getUint16(pos); pos += 2; return v; }
  function u32() { const v = dv.getUint32(pos); pos += 4; return v; }
  function skip(n: number) { pos += n; }

  // Magic + version
  const magic = u32();
  if (magic !== 0xCAFEBABE) throw new Error("Not a valid .class file");
  skip(4); // minor + major version

  // Constant pool (1-based)
  const cpCount = u16();
  const cp: (string | null)[] = [null];
  for (let i = 1; i < cpCount; i++) {
    const tag = u8();
    switch (tag) {
      case 1: { // Utf8
        const len = u16();
        let s = "";
        for (let j = 0; j < len; j++) s += String.fromCharCode(u8());
        cp.push(s);
        break;
      }
      case 7:  { cp.push(`#class:${u16()}`); break; }
      case 8:  { cp.push(`#str:${u16()}`); break; }
      case 9:  { cp.push(`#field:${u16()}:${u16()}`); break; }
      case 10: { cp.push(`#meth:${u16()}:${u16()}`); break; }
      case 11: { cp.push(`#imeth:${u16()}:${u16()}`); break; }
      case 12: { cp.push(`#nat:${u16()}:${u16()}`); break; }
      case 18: { cp.push(`#indy:${u16()}:${u16()}`); break; }
      case 3:  { skip(4); cp.push(null); break; } // Integer
      case 4:  { skip(4); cp.push(null); break; } // Float
      case 5:  { skip(8); cp.push(null); cp.push(null); i++; break; } // Long (2 slots)
      case 6:  { skip(8); cp.push(null); cp.push(null); i++; break; } // Double (2 slots)
      case 15: { skip(3); cp.push(null); break; } // MethodHandle
      case 16: { skip(2); cp.push(null); break; } // MethodType
      case 17: { skip(4); cp.push(null); break; } // Dynamic
      case 19: { skip(2); cp.push(null); break; } // Module
      case 20: { skip(2); cp.push(null); break; } // Package
      default: { cp.push(null); break; }
    }
  }

  function resolveClass(idx: number): string {
    const entry = cp[idx];
    if (!entry) return "";
    const m = entry.match(/^#class:(\d+)$/);
    return m ? (cp[+m[1]] ?? "") : "";
  }

  // Access flags, this class, super class
  const accessFlags = u16();
  const thisClassName = resolveClass(u16());
  const superClassName = resolveClass(u16());

  // Interfaces
  const ifCount = u16();
  const interfaces: string[] = [];
  for (let i = 0; i < ifCount; i++) {
    interfaces.push(resolveClass(u16()));
  }

  // Fields
  const fieldCount = u16();
  const fields: FieldMeta[] = [];
  for (let i = 0; i < fieldCount; i++) {
    const fFlags = u16();
    const fNameIdx = u16();
    const fDescIdx = u16();
    const fName = cp[fNameIdx] ?? "";
    const fDesc = cp[fDescIdx] ?? "";
    fields.push({ name: fName, descriptor: fDesc, accessFlags: fFlags });
    const attrCount = u16();
    for (let a = 0; a < attrCount; a++) { skip(2); skip(u32()); }
  }

  // Methods
  const methodCount = u16();
  const methods: MethodMeta[] = [];
  for (let i = 0; i < methodCount; i++) {
    const mFlags = u16();
    const mNameIdx = u16();
    const mDescIdx = u16();
    const mName = cp[mNameIdx] ?? "";
    const mDesc = cp[mDescIdx] ?? "";
    methods.push({ name: mName, descriptor: mDesc, accessFlags: mFlags });
    const attrCount = u16();
    for (let a = 0; a < attrCount; a++) { skip(2); skip(u32()); }
  }

  return {
    name: thisClassName,
    accessFlags,
    superClass: superClassName,
    interfaces,
    fields,
    methods,
  };
}

// ---------------------------------------------------------------------------
// Bundle parser — [u32 length][bytes] × N
// ---------------------------------------------------------------------------

export function parseBundleMeta(bundle: Uint8Array): ClassMeta[] {
  const dv = new DataView(bundle.buffer, bundle.byteOffset, bundle.byteLength);
  const classes: ClassMeta[] = [];
  let pos = 0;
  while (pos + 4 <= bundle.length) {
    const size = dv.getUint32(pos);
    pos += 4;
    if (pos + size > bundle.length) break;
    try {
      classes.push(parseClassMeta(bundle.subarray(pos, pos + size)));
    } catch {
      // Skip invalid class files
    }
    pos += size;
  }
  return classes;
}

// ---------------------------------------------------------------------------
// JVM descriptor → Type conversion
// ---------------------------------------------------------------------------

/** Convert a single JVM field descriptor to a compiler Type. */
export function descriptorToType(desc: string): Type {
  switch (desc[0]) {
    case "B": return "byte";
    case "C": return "char";
    case "S": return "short";
    case "I": return "int";
    case "J": return "long";
    case "F": return "float";
    case "D": return "double";
    case "Z": return "boolean";
    case "V": return "void";
    case "L": {
      // Ljava/lang/String; → "String"
      const className = desc.slice(1, desc.length - 1);
      if (className === "java/lang/String") return "String";
      return { className };
    }
    case "[": {
      return { array: descriptorToType(desc.slice(1)) };
    }
    default: return { className: desc };
  }
}

/** Parse a method descriptor "(params)ret" into param types and return type. */
export function parseMethodDescriptor(desc: string): { params: Type[]; ret: Type } {
  const params: Type[] = [];
  let i = 1; // skip '('
  while (i < desc.length && desc[i] !== ")") {
    const [type, consumed] = parseOneDescriptor(desc, i);
    params.push(type);
    i += consumed;
  }
  i++; // skip ')'
  const [ret] = parseOneDescriptor(desc, i);
  return { params, ret };
}

function parseOneDescriptor(desc: string, start: number): [Type, number] {
  switch (desc[start]) {
    case "B": return ["byte", 1];
    case "C": return ["char", 1];
    case "S": return ["short", 1];
    case "I": return ["int", 1];
    case "J": return ["long", 1];
    case "F": return ["float", 1];
    case "D": return ["double", 1];
    case "Z": return ["boolean", 1];
    case "V": return ["void", 1];
    case "L": {
      const semi = desc.indexOf(";", start);
      const className = desc.slice(start + 1, semi);
      if (className === "java/lang/String") return ["String", semi - start + 1];
      return [{ className }, semi - start + 1];
    }
    case "[": {
      const [inner, consumed] = parseOneDescriptor(desc, start + 1);
      return [{ array: inner }, 1 + consumed];
    }
    default: return [{ className: desc.slice(start) }, desc.length - start];
  }
}

// ---------------------------------------------------------------------------
// Method registry builder
// ---------------------------------------------------------------------------

export function buildMethodRegistry(classes: ClassMeta[]): Record<string, MethodSig> {
  const registry: Record<string, MethodSig> = {};
  for (const cls of classes) {
    const isInterface = (cls.accessFlags & 0x0200) !== 0;
    for (const m of cls.methods) {
      // Extract the raw argument descriptors from "(args)ret"
      const lparen = m.descriptor.indexOf("(");
      const rparen = m.descriptor.indexOf(")");
      if (lparen < 0 || rparen < 0) continue;
      const argDescs = m.descriptor.slice(lparen + 1, rparen);
      const retDesc = m.descriptor.slice(rparen + 1);

      const key = `${cls.name}.${m.name}(${argDescs})`;
      const { params } = parseMethodDescriptor(m.descriptor);
      const ret = descriptorToType(retDesc);

      const isStatic = (m.accessFlags & 0x0008) !== 0;
      const isAbstract = (m.accessFlags & 0x0400) !== 0;
      registry[key] = {
        owner: cls.name,
        returnType: ret,
        paramTypes: params,
        ...(isInterface ? { isInterface: true } : {}),
        ...(isStatic ? { isStatic: true } : {}),
        isAbstract,
      };
    }
  }
  return registry;
}

// ---------------------------------------------------------------------------
// ZIP/JAR reader
// ---------------------------------------------------------------------------

/** Read a JAR (ZIP) file and return a map of filename → bytes for .class files. */
export async function readJar(jarBytes: Uint8Array): Promise<Map<string, Uint8Array>> {
  const dv = new DataView(jarBytes.buffer, jarBytes.byteOffset, jarBytes.byteLength);
  const result = new Map<string, Uint8Array>();

  // Find End of Central Directory record (last 22+ bytes).
  // Signature: 0x06054b50
  let eocdPos = -1;
  for (let i = jarBytes.length - 22; i >= Math.max(0, jarBytes.length - 65557); i--) {
    if (dv.getUint32(i, true) === 0x06054b50) {
      eocdPos = i;
      break;
    }
  }
  if (eocdPos < 0) throw new Error("Not a valid ZIP/JAR file (EOCD not found)");

  const cdOffset = dv.getUint32(eocdPos + 16, true);
  const cdEntries = dv.getUint16(eocdPos + 10, true);

  // Walk Central Directory entries
  let cdPos = cdOffset;
  for (let i = 0; i < cdEntries; i++) {
    if (dv.getUint32(cdPos, true) !== 0x02014b50) break;

    const compressionMethod = dv.getUint16(cdPos + 10, true);
    const compressedSize = dv.getUint32(cdPos + 20, true);
    const uncompressedSize = dv.getUint32(cdPos + 24, true);
    const nameLen = dv.getUint16(cdPos + 28, true);
    const extraLen = dv.getUint16(cdPos + 30, true);
    const commentLen = dv.getUint16(cdPos + 32, true);
    const localHeaderOffset = dv.getUint32(cdPos + 42, true);

    // Read filename
    const nameBytes = jarBytes.subarray(cdPos + 46, cdPos + 46 + nameLen);
    const fileName = new TextDecoder().decode(nameBytes);

    cdPos += 46 + nameLen + extraLen + commentLen;

    // Only process .class files
    if (!fileName.endsWith(".class")) continue;

    // Read from local file header to get actual data offset
    const localExtraLen = dv.getUint16(localHeaderOffset + 28, true);
    const localNameLen = dv.getUint16(localHeaderOffset + 26, true);
    const dataOffset = localHeaderOffset + 30 + localNameLen + localExtraLen;
    const rawData = jarBytes.subarray(dataOffset, dataOffset + compressedSize);

    if (compressionMethod === 0) {
      // STORED — no compression
      result.set(fileName, rawData);
    } else if (compressionMethod === 8) {
      // DEFLATED — use DecompressionStream
      const ds = new DecompressionStream("deflate-raw");
      const writer = ds.writable.getWriter();
      const reader = ds.readable.getReader();

      const writePromise = writer.write(rawData).then(() => writer.close());
      const chunks: Uint8Array[] = [];
      let totalLen = 0;
      // eslint-disable-next-line no-constant-condition
      while (true) {
        const { done, value } = await reader.read();
        if (done) break;
        chunks.push(value);
        totalLen += value.length;
      }
      await writePromise;

      const decompressed = new Uint8Array(uncompressedSize || totalLen);
      let offset = 0;
      for (const chunk of chunks) {
        decompressed.set(chunk, offset);
        offset += chunk.length;
      }
      result.set(fileName, decompressed);
    }
    // Other compression methods are ignored
  }

  return result;
}

/** Convert a map of class files to bundle.bin format: [u32 length][bytes] × N */
export function classFilesToBundle(classFiles: Map<string, Uint8Array>): Uint8Array {
  let totalSize = 0;
  for (const data of classFiles.values()) {
    totalSize += 4 + data.length;
  }

  const bundle = new Uint8Array(totalSize);
  const dv = new DataView(bundle.buffer);
  let pos = 0;

  for (const data of classFiles.values()) {
    dv.setUint32(pos, data.length);
    pos += 4;
    bundle.set(data, pos);
    pos += data.length;
  }

  return bundle;
}
