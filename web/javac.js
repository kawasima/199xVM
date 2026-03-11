var __defProp = Object.defineProperty;
var __defNormalProp = (obj, key, value) => key in obj ? __defProp(obj, key, { enumerable: true, configurable: true, writable: true, value }) : obj[key] = value;
var __publicField = (obj, key, value) => __defNormalProp(obj, typeof key !== "symbol" ? key + "" : key, value);

// web/class-reader.ts
function parseClassMeta(data) {
  const dv = new DataView(data.buffer, data.byteOffset, data.byteLength);
  let pos = 0;
  function u8() {
    return dv.getUint8(pos++);
  }
  function u16() {
    const v = dv.getUint16(pos);
    pos += 2;
    return v;
  }
  function u32() {
    const v = dv.getUint32(pos);
    pos += 4;
    return v;
  }
  function skip(n) {
    pos += n;
  }
  const magic = u32();
  if (magic !== 3405691582) throw new Error("Not a valid .class file");
  skip(4);
  const cpCount = u16();
  const cp = [null];
  for (let i = 1; i < cpCount; i++) {
    const tag = u8();
    switch (tag) {
      case 1: {
        const len = u16();
        let s = "";
        for (let j = 0; j < len; j++) s += String.fromCharCode(u8());
        cp.push(s);
        break;
      }
      case 7: {
        cp.push(`#class:${u16()}`);
        break;
      }
      case 8: {
        cp.push(`#str:${u16()}`);
        break;
      }
      case 9: {
        cp.push(`#field:${u16()}:${u16()}`);
        break;
      }
      case 10: {
        cp.push(`#meth:${u16()}:${u16()}`);
        break;
      }
      case 11: {
        cp.push(`#imeth:${u16()}:${u16()}`);
        break;
      }
      case 12: {
        cp.push(`#nat:${u16()}:${u16()}`);
        break;
      }
      case 18: {
        cp.push(`#indy:${u16()}:${u16()}`);
        break;
      }
      case 3: {
        skip(4);
        cp.push(null);
        break;
      }
      // Integer
      case 4: {
        skip(4);
        cp.push(null);
        break;
      }
      // Float
      case 5: {
        skip(8);
        cp.push(null);
        i++;
        break;
      }
      // Long (2 slots)
      case 6: {
        skip(8);
        cp.push(null);
        i++;
        break;
      }
      // Double (2 slots)
      case 15: {
        skip(3);
        cp.push(null);
        break;
      }
      // MethodHandle
      case 16: {
        skip(2);
        cp.push(null);
        break;
      }
      // MethodType
      case 17: {
        skip(4);
        cp.push(null);
        break;
      }
      // Dynamic
      case 19: {
        skip(2);
        cp.push(null);
        break;
      }
      // Module
      case 20: {
        skip(2);
        cp.push(null);
        break;
      }
      // Package
      default: {
        cp.push(null);
        break;
      }
    }
  }
  function resolveClass(idx) {
    const entry = cp[idx];
    if (!entry) return "";
    const m = entry.match(/^#class:(\d+)$/);
    return m ? cp[+m[1]] ?? "" : "";
  }
  const accessFlags = u16();
  const thisClassName = resolveClass(u16());
  const superClassName = resolveClass(u16());
  const ifCount = u16();
  const interfaces = [];
  for (let i = 0; i < ifCount; i++) {
    interfaces.push(resolveClass(u16()));
  }
  const fieldCount = u16();
  for (let i = 0; i < fieldCount; i++) {
    skip(6);
    const attrCount = u16();
    for (let a = 0; a < attrCount; a++) {
      skip(2);
      skip(u32());
    }
  }
  const methodCount = u16();
  const methods = [];
  for (let i = 0; i < methodCount; i++) {
    const mFlags = u16();
    const mNameIdx = u16();
    const mDescIdx = u16();
    const mName = cp[mNameIdx] ?? "";
    const mDesc = cp[mDescIdx] ?? "";
    methods.push({ name: mName, descriptor: mDesc, accessFlags: mFlags });
    const attrCount = u16();
    for (let a = 0; a < attrCount; a++) {
      skip(2);
      skip(u32());
    }
  }
  return {
    name: thisClassName,
    accessFlags,
    superClass: superClassName,
    interfaces,
    methods
  };
}
function parseBundleMeta(bundle) {
  const dv = new DataView(bundle.buffer, bundle.byteOffset, bundle.byteLength);
  const classes = [];
  let pos = 0;
  while (pos + 4 <= bundle.length) {
    const size = dv.getUint32(pos);
    pos += 4;
    if (pos + size > bundle.length) break;
    try {
      classes.push(parseClassMeta(bundle.subarray(pos, pos + size)));
    } catch {
    }
    pos += size;
  }
  return classes;
}
function descriptorToType(desc) {
  switch (desc[0]) {
    case "B":
      return "byte";
    case "C":
      return "char";
    case "S":
      return "short";
    case "I":
      return "int";
    case "J":
      return "long";
    case "F":
      return "float";
    case "D":
      return "double";
    case "Z":
      return "boolean";
    case "V":
      return "void";
    case "L": {
      const className = desc.slice(1, desc.length - 1);
      if (className === "java/lang/String") return "String";
      return { className };
    }
    case "[": {
      return { array: descriptorToType(desc.slice(1)) };
    }
    default:
      return { className: desc };
  }
}
function parseMethodDescriptor(desc) {
  const params = [];
  let i = 1;
  while (i < desc.length && desc[i] !== ")") {
    const [type, consumed] = parseOneDescriptor(desc, i);
    params.push(type);
    i += consumed;
  }
  i++;
  const [ret] = parseOneDescriptor(desc, i);
  return { params, ret };
}
function parseOneDescriptor(desc, start) {
  switch (desc[start]) {
    case "B":
      return ["byte", 1];
    case "C":
      return ["char", 1];
    case "S":
      return ["short", 1];
    case "I":
      return ["int", 1];
    case "J":
      return ["long", 1];
    case "F":
      return ["float", 1];
    case "D":
      return ["double", 1];
    case "Z":
      return ["boolean", 1];
    case "V":
      return ["void", 1];
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
    default:
      return [{ className: desc.slice(start) }, desc.length - start];
  }
}
function buildMethodRegistry(classes) {
  const registry = {};
  for (const cls of classes) {
    const isInterface = (cls.accessFlags & 512) !== 0;
    for (const m of cls.methods) {
      const lparen = m.descriptor.indexOf("(");
      const rparen = m.descriptor.indexOf(")");
      if (lparen < 0 || rparen < 0) continue;
      const argDescs = m.descriptor.slice(lparen + 1, rparen);
      const retDesc = m.descriptor.slice(rparen + 1);
      const key = `${cls.name}.${m.name}(${argDescs})`;
      const { params } = parseMethodDescriptor(m.descriptor);
      const ret = descriptorToType(retDesc);
      registry[key] = {
        owner: cls.name,
        returnType: ret,
        paramTypes: params,
        ...isInterface ? { isInterface: true } : {}
      };
    }
  }
  return registry;
}
async function readJar(jarBytes) {
  const dv = new DataView(jarBytes.buffer, jarBytes.byteOffset, jarBytes.byteLength);
  const result = /* @__PURE__ */ new Map();
  let eocdPos = -1;
  for (let i = jarBytes.length - 22; i >= Math.max(0, jarBytes.length - 65557); i--) {
    if (dv.getUint32(i, true) === 101010256) {
      eocdPos = i;
      break;
    }
  }
  if (eocdPos < 0) throw new Error("Not a valid ZIP/JAR file (EOCD not found)");
  const cdOffset = dv.getUint32(eocdPos + 16, true);
  const cdEntries = dv.getUint16(eocdPos + 10, true);
  let cdPos = cdOffset;
  for (let i = 0; i < cdEntries; i++) {
    if (dv.getUint32(cdPos, true) !== 33639248) break;
    const compressionMethod = dv.getUint16(cdPos + 10, true);
    const compressedSize = dv.getUint32(cdPos + 20, true);
    const uncompressedSize = dv.getUint32(cdPos + 24, true);
    const nameLen = dv.getUint16(cdPos + 28, true);
    const extraLen = dv.getUint16(cdPos + 30, true);
    const commentLen = dv.getUint16(cdPos + 32, true);
    const localHeaderOffset = dv.getUint32(cdPos + 42, true);
    const nameBytes = jarBytes.subarray(cdPos + 46, cdPos + 46 + nameLen);
    const fileName = new TextDecoder().decode(nameBytes);
    cdPos += 46 + nameLen + extraLen + commentLen;
    if (!fileName.endsWith(".class")) continue;
    const localExtraLen = dv.getUint16(localHeaderOffset + 28, true);
    const localNameLen = dv.getUint16(localHeaderOffset + 26, true);
    const dataOffset = localHeaderOffset + 30 + localNameLen + localExtraLen;
    const rawData = jarBytes.subarray(dataOffset, dataOffset + compressedSize);
    if (compressionMethod === 0) {
      result.set(fileName, rawData);
    } else if (compressionMethod === 8) {
      const ds = new DecompressionStream("deflate-raw");
      const writer = ds.writable.getWriter();
      const reader = ds.readable.getReader();
      const writePromise = writer.write(rawData).then(() => writer.close());
      const chunks = [];
      let totalLen = 0;
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
  }
  return result;
}
function classFilesToBundle(classFiles) {
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

// web/javac.ts
var TokenKind = /* @__PURE__ */ ((TokenKind2) => {
  TokenKind2["IntLiteral"] = "IntLiteral";
  TokenKind2["LongLiteral"] = "LongLiteral";
  TokenKind2["FloatLiteral"] = "FloatLiteral";
  TokenKind2["DoubleLiteral"] = "DoubleLiteral";
  TokenKind2["CharLiteral"] = "CharLiteral";
  TokenKind2["StringLiteral"] = "StringLiteral";
  TokenKind2["BoolLiteral"] = "BoolLiteral";
  TokenKind2["NullLiteral"] = "NullLiteral";
  TokenKind2["Ident"] = "Ident";
  TokenKind2["KwClass"] = "class";
  TokenKind2["KwPublic"] = "public";
  TokenKind2["KwStatic"] = "static";
  TokenKind2["KwVoid"] = "void";
  TokenKind2["KwInt"] = "int";
  TokenKind2["KwLong"] = "long";
  TokenKind2["KwShort"] = "short";
  TokenKind2["KwByte"] = "byte";
  TokenKind2["KwChar"] = "char";
  TokenKind2["KwFloat"] = "float";
  TokenKind2["KwDouble"] = "double";
  TokenKind2["KwBoolean"] = "boolean";
  TokenKind2["KwString"] = "String";
  TokenKind2["KwReturn"] = "return";
  TokenKind2["KwNew"] = "new";
  TokenKind2["KwIf"] = "if";
  TokenKind2["KwElse"] = "else";
  TokenKind2["KwWhile"] = "while";
  TokenKind2["KwFor"] = "for";
  TokenKind2["KwSwitch"] = "switch";
  TokenKind2["KwCase"] = "case";
  TokenKind2["KwDefault"] = "default";
  TokenKind2["KwYield"] = "yield";
  TokenKind2["KwWhen"] = "when";
  TokenKind2["KwThis"] = "this";
  TokenKind2["KwSuper"] = "super";
  TokenKind2["KwExtends"] = "extends";
  TokenKind2["KwImplements"] = "implements";
  TokenKind2["KwImport"] = "import";
  TokenKind2["KwPackage"] = "package";
  TokenKind2["KwPrivate"] = "private";
  TokenKind2["KwProtected"] = "protected";
  TokenKind2["KwFinal"] = "final";
  TokenKind2["KwAbstract"] = "abstract";
  TokenKind2["KwVar"] = "var";
  TokenKind2["KwInstanceof"] = "instanceof";
  TokenKind2["KwRecord"] = "record";
  TokenKind2["LParen"] = "(";
  TokenKind2["RParen"] = ")";
  TokenKind2["LBrace"] = "{";
  TokenKind2["RBrace"] = "}";
  TokenKind2["LBracket"] = "[";
  TokenKind2["RBracket"] = "]";
  TokenKind2["Semi"] = ";";
  TokenKind2["Comma"] = ",";
  TokenKind2["Dot"] = ".";
  TokenKind2["Plus"] = "+";
  TokenKind2["Minus"] = "-";
  TokenKind2["Star"] = "*";
  TokenKind2["Slash"] = "/";
  TokenKind2["Percent"] = "%";
  TokenKind2["Assign"] = "=";
  TokenKind2["Eq"] = "==";
  TokenKind2["Ne"] = "!=";
  TokenKind2["Lt"] = "<";
  TokenKind2["Gt"] = ">";
  TokenKind2["Le"] = "<=";
  TokenKind2["Ge"] = ">=";
  TokenKind2["And"] = "&&";
  TokenKind2["Or"] = "||";
  TokenKind2["Not"] = "!";
  TokenKind2["PlusAssign"] = "+=";
  TokenKind2["MinusAssign"] = "-=";
  TokenKind2["PlusPlus"] = "++";
  TokenKind2["MinusMinus"] = "--";
  TokenKind2["Question"] = "?";
  TokenKind2["Colon"] = ":";
  TokenKind2["ColonColon"] = "::";
  TokenKind2["Arrow"] = "->";
  TokenKind2["EOF"] = "EOF";
  return TokenKind2;
})(TokenKind || {});
function preprocessUnicodeEscapes(input) {
  let out = "";
  for (let i = 0; i < input.length; i++) {
    if (input[i] !== "\\") {
      out += input[i];
      continue;
    }
    let j = i + 1;
    if (j >= input.length || input[j] !== "u") {
      out += input[i];
      continue;
    }
    while (j < input.length && input[j] === "u") j++;
    if (j + 4 > input.length) throw new Error("Invalid Unicode escape sequence");
    const hex = input.slice(j, j + 4);
    if (!/^[0-9a-fA-F]{4}$/.test(hex)) throw new Error(`Invalid Unicode escape: \\u${hex}`);
    out += String.fromCharCode(parseInt(hex, 16));
    i = j + 3;
  }
  return out;
}
var KEYWORDS = {
  class: "class" /* KwClass */,
  public: "public" /* KwPublic */,
  static: "static" /* KwStatic */,
  void: "void" /* KwVoid */,
  int: "int" /* KwInt */,
  long: "long" /* KwLong */,
  short: "short" /* KwShort */,
  byte: "byte" /* KwByte */,
  char: "char" /* KwChar */,
  float: "float" /* KwFloat */,
  double: "double" /* KwDouble */,
  boolean: "boolean" /* KwBoolean */,
  String: "String" /* KwString */,
  return: "return" /* KwReturn */,
  new: "new" /* KwNew */,
  if: "if" /* KwIf */,
  else: "else" /* KwElse */,
  while: "while" /* KwWhile */,
  for: "for" /* KwFor */,
  switch: "switch" /* KwSwitch */,
  case: "case" /* KwCase */,
  default: "default" /* KwDefault */,
  yield: "yield" /* KwYield */,
  when: "when" /* KwWhen */,
  this: "this" /* KwThis */,
  super: "super" /* KwSuper */,
  true: "BoolLiteral" /* BoolLiteral */,
  false: "BoolLiteral" /* BoolLiteral */,
  null: "NullLiteral" /* NullLiteral */,
  extends: "extends" /* KwExtends */,
  implements: "implements" /* KwImplements */,
  import: "import" /* KwImport */,
  package: "package" /* KwPackage */,
  private: "private" /* KwPrivate */,
  protected: "protected" /* KwProtected */,
  final: "final" /* KwFinal */,
  abstract: "abstract" /* KwAbstract */,
  var: "var" /* KwVar */,
  instanceof: "instanceof" /* KwInstanceof */,
  record: "record" /* KwRecord */
};
function lex(source) {
  source = preprocessUnicodeEscapes(source);
  const tokens = [];
  let pos = 0;
  let line = 1;
  let col = 1;
  function peek() {
    return pos < source.length ? source[pos] : "\0";
  }
  function advance() {
    const ch = source[pos++];
    if (ch === "\n") {
      line++;
      col = 1;
    } else {
      col++;
    }
    return ch;
  }
  while (pos < source.length) {
    const ch = peek();
    if (/\s/.test(ch)) {
      advance();
      continue;
    }
    if (ch === "/" && pos + 1 < source.length && source[pos + 1] === "/") {
      while (pos < source.length && peek() !== "\n") advance();
      continue;
    }
    if (ch === "/" && pos + 1 < source.length && source[pos + 1] === "*") {
      const cLine = line;
      const cCol = col;
      advance();
      advance();
      while (pos + 1 < source.length && !(peek() === "*" && source[pos + 1] === "/")) advance();
      if (pos + 1 >= source.length) {
        throw new Error(`Unterminated block comment at line ${cLine}:${cCol}`);
      }
      advance();
      advance();
      continue;
    }
    const startLine = line;
    const startCol = col;
    if (ch === '"') {
      advance();
      let s = "";
      while (peek() !== '"' && peek() !== "\0") {
        if (peek() === "\n" || peek() === "\r") {
          throw new Error(`Unterminated string literal at line ${startLine}:${startCol}`);
        }
        if (peek() === "\\") {
          advance();
          const esc = advance();
          switch (esc) {
            case "n":
              s += "\n";
              break;
            case "t":
              s += "	";
              break;
            case "\\":
              s += "\\";
              break;
            case '"':
              s += '"';
              break;
            default:
              s += esc;
          }
        } else {
          s += advance();
        }
      }
      if (peek() === "\0") {
        throw new Error(`Unterminated string literal at line ${startLine}:${startCol}`);
      }
      advance();
      tokens.push({ kind: "StringLiteral" /* StringLiteral */, value: s, line: startLine, col: startCol });
      continue;
    }
    if (ch === "'") {
      advance();
      let charVal;
      if (peek() === "\\") {
        advance();
        const esc = advance();
        switch (esc) {
          case "n":
            charVal = 10;
            break;
          case "t":
            charVal = 9;
            break;
          case "r":
            charVal = 13;
            break;
          case "\\":
            charVal = 92;
            break;
          case "'":
            charVal = 39;
            break;
          case '"':
            charVal = 34;
            break;
          case "0":
            charVal = 0;
            break;
          default:
            charVal = esc.charCodeAt(0);
        }
      } else {
        charVal = advance().charCodeAt(0);
      }
      if (peek() !== "'") throw new Error(`Unterminated char literal at line ${startLine}:${startCol}`);
      advance();
      tokens.push({ kind: "CharLiteral" /* CharLiteral */, value: String(charVal), line: startLine, col: startCol });
      continue;
    }
    if (/[0-9]/.test(ch)) {
      let raw = "";
      if (peek() === "0" && (source[pos + 1] === "x" || source[pos + 1] === "X")) {
        raw += advance();
        raw += advance();
        while (/[0-9a-fA-F_]/.test(peek())) raw += advance();
      } else if (peek() === "0" && (source[pos + 1] === "b" || source[pos + 1] === "B")) {
        raw += advance();
        raw += advance();
        while (/[01_]/.test(peek())) raw += advance();
      } else if (peek() === "0" && /[0-7_]/.test(source[pos + 1] ?? "")) {
        raw += advance();
        while (/[0-7_]/.test(peek())) raw += advance();
      } else {
        while (/[0-9_]/.test(peek())) raw += advance();
      }
      let hasDecimal = false;
      if (peek() === "." && /[0-9]/.test(source[pos + 1] ?? "")) {
        hasDecimal = true;
        raw += advance();
        while (/[0-9_]/.test(peek())) raw += advance();
      }
      if (peek() === "e" || peek() === "E") {
        hasDecimal = true;
        raw += advance();
        if (peek() === "+" || peek() === "-") raw += advance();
        while (/[0-9_]/.test(peek())) raw += advance();
      }
      const isFloat = peek() === "f" || peek() === "F";
      const isDouble = peek() === "d" || peek() === "D";
      const isLong = !isFloat && !isDouble && (peek() === "L" || peek() === "l");
      if (isFloat || isDouble || isLong) raw += advance();
      if (isFloat) {
        tokens.push({ kind: "FloatLiteral" /* FloatLiteral */, value: raw, line: startLine, col: startCol });
      } else if (isDouble || hasDecimal) {
        tokens.push({ kind: "DoubleLiteral" /* DoubleLiteral */, value: raw, line: startLine, col: startCol });
      } else if (isLong) {
        tokens.push({ kind: "LongLiteral" /* LongLiteral */, value: raw, line: startLine, col: startCol });
      } else {
        tokens.push({ kind: "IntLiteral" /* IntLiteral */, value: raw, line: startLine, col: startCol });
      }
      continue;
    }
    if (/[a-zA-Z_$]/.test(ch)) {
      let ident = "";
      while (/[a-zA-Z0-9_$]/.test(peek())) ident += advance();
      const kw = Object.prototype.hasOwnProperty.call(KEYWORDS, ident) ? KEYWORDS[ident] : void 0;
      tokens.push({ kind: kw ?? "Ident" /* Ident */, value: ident, line: startLine, col: startCol });
      continue;
    }
    const two = pos + 1 < source.length ? ch + source[pos + 1] : "";
    if (two === "==") {
      advance();
      advance();
      tokens.push({ kind: "==" /* Eq */, value: "==", line: startLine, col: startCol });
      continue;
    }
    if (two === "!=") {
      advance();
      advance();
      tokens.push({ kind: "!=" /* Ne */, value: "!=", line: startLine, col: startCol });
      continue;
    }
    if (two === "<=") {
      advance();
      advance();
      tokens.push({ kind: "<=" /* Le */, value: "<=", line: startLine, col: startCol });
      continue;
    }
    if (two === ">=") {
      advance();
      advance();
      tokens.push({ kind: ">=" /* Ge */, value: ">=", line: startLine, col: startCol });
      continue;
    }
    if (two === "&&") {
      advance();
      advance();
      tokens.push({ kind: "&&" /* And */, value: "&&", line: startLine, col: startCol });
      continue;
    }
    if (two === "||") {
      advance();
      advance();
      tokens.push({ kind: "||" /* Or */, value: "||", line: startLine, col: startCol });
      continue;
    }
    if (two === "+=") {
      advance();
      advance();
      tokens.push({ kind: "+=" /* PlusAssign */, value: "+=", line: startLine, col: startCol });
      continue;
    }
    if (two === "-=") {
      advance();
      advance();
      tokens.push({ kind: "-=" /* MinusAssign */, value: "-=", line: startLine, col: startCol });
      continue;
    }
    if (two === "::") {
      advance();
      advance();
      tokens.push({ kind: "::" /* ColonColon */, value: "::", line: startLine, col: startCol });
      continue;
    }
    if (two === "->") {
      advance();
      advance();
      tokens.push({ kind: "->" /* Arrow */, value: "->", line: startLine, col: startCol });
      continue;
    }
    if (two === "++") {
      advance();
      advance();
      tokens.push({ kind: "++" /* PlusPlus */, value: "++", line: startLine, col: startCol });
      continue;
    }
    if (two === "--") {
      advance();
      advance();
      tokens.push({ kind: "--" /* MinusMinus */, value: "--", line: startLine, col: startCol });
      continue;
    }
    const singles = {
      "(": "(" /* LParen */,
      ")": ")" /* RParen */,
      "{": "{" /* LBrace */,
      "}": "}" /* RBrace */,
      "[": "[" /* LBracket */,
      "]": "]" /* RBracket */,
      ";": ";" /* Semi */,
      ",": "," /* Comma */,
      ".": "." /* Dot */,
      "+": "+" /* Plus */,
      "-": "-" /* Minus */,
      "*": "*" /* Star */,
      "/": "/" /* Slash */,
      "%": "%" /* Percent */,
      "=": "=" /* Assign */,
      "<": "<" /* Lt */,
      ">": ">" /* Gt */,
      "!": "!" /* Not */,
      "?": "?" /* Question */,
      ":": ":" /* Colon */
    };
    if (singles[ch]) {
      advance();
      tokens.push({ kind: singles[ch], value: ch, line: startLine, col: startCol });
      continue;
    }
    throw new Error(`Unknown character "${ch}" at line ${startLine}:${startCol}`);
  }
  tokens.push({ kind: "EOF" /* EOF */, value: "", line, col });
  return tokens;
}
function parseAll(tokens) {
  let pos = 0;
  function peek() {
    return tokens[pos] ?? tokens[tokens.length - 1];
  }
  function advance() {
    return tokens[pos++];
  }
  function expect(kind) {
    const t = peek();
    if (t.kind !== kind) throw new Error(`Expected ${kind} but got ${t.kind} ("${t.value}") at line ${t.line}:${t.col}`);
    return advance();
  }
  function match(kind) {
    if (peek().kind === kind) {
      advance();
      return true;
    }
    return false;
  }
  function at(kind) {
    return peek().kind === kind;
  }
  function parseIntLiteral(raw) {
    let s = raw.replace(/_/g, "");
    if (s.endsWith("L") || s.endsWith("l")) s = s.slice(0, -1);
    if (/^0[xX][0-9a-fA-F]+$/.test(s)) return Number.parseInt(s.slice(2), 16);
    if (/^0[bB][01]+$/.test(s)) return Number.parseInt(s.slice(2), 2);
    if (/^0[0-7]+$/.test(s) && s.length > 1) return Number.parseInt(s.slice(1), 8);
    if (/^[0-9]+$/.test(s)) return Number.parseInt(s, 10);
    throw new Error(`Invalid integer literal: ${raw}`);
  }
  function parseQualifiedName() {
    let name = expect("Ident" /* Ident */).value;
    while (at("." /* Dot */) && tokens[pos + 1]?.kind === "Ident" /* Ident */) {
      advance();
      name += "." + expect("Ident" /* Ident */).value;
    }
    return name;
  }
  const importMap = /* @__PURE__ */ new Map();
  const packageImports = ["java/lang"];
  const staticWildcardImports = [];
  while (at("import" /* KwImport */) || at("package" /* KwPackage */)) {
    const isImport = at("import" /* KwImport */);
    advance();
    if (isImport) {
      const isStaticImport = match("static" /* KwStatic */);
      const base = parseQualifiedName();
      if (match("." /* Dot */)) {
        if (match("*" /* Star */)) {
          if (isStaticImport) {
            staticWildcardImports.push(base.replace(/\./g, "/"));
          } else {
            const internalBase = base.replace(/\./g, "/");
            packageImports.push(internalBase);
            if (/^[A-Z]/.test(base.split(".").pop() ?? "")) {
              staticWildcardImports.push(internalBase);
            }
          }
        } else {
          const member = expect("Ident" /* Ident */).value;
          if (!isStaticImport) {
            const fqn = `${base}.${member}`;
            importMap.set(member, fqn.replace(/\./g, "/"));
          }
        }
      } else if (!isStaticImport) {
        const simpleName = base.split(".").pop();
        importMap.set(simpleName, base.replace(/\./g, "/"));
      } else {
        const lastDot = base.lastIndexOf(".");
        if (lastDot < 0) throw new Error(`Invalid static import near "${base}"`);
      }
    } else {
      while (!at(";" /* Semi */) && !at("EOF" /* EOF */)) advance();
    }
    expect(";" /* Semi */);
  }
  const results = [];
  while (!at("EOF" /* EOF */)) {
    results.push(parseOneClass());
  }
  return results;
  function parseOneClass() {
    while (at("public" /* KwPublic */) || at("abstract" /* KwAbstract */) || at("final" /* KwFinal */)) advance();
    if (at("record" /* KwRecord */)) {
      advance();
      const recordName = expect("Ident" /* Ident */).value;
      expect("(" /* LParen */);
      const components = [];
      if (!at(")" /* RParen */)) {
        do {
          const cType = parseType();
          const cName = expect("Ident" /* Ident */).value;
          components.push({ name: cName, type: cType });
        } while (match("," /* Comma */));
      }
      expect(")" /* RParen */);
      if (match("implements" /* KwImplements */)) {
        while (!at("{" /* LBrace */) && !at("EOF" /* EOF */)) advance();
      }
      expect("{" /* LBrace */);
      const recordFields = [];
      const recordMethods = [];
      while (!at("}" /* RBrace */) && !at("EOF" /* EOF */)) {
        parseMember(recordFields, recordMethods, recordName, true);
      }
      expect("}" /* RBrace */);
      for (const c of components) {
        recordFields.push({ name: c.name, type: c.type, isStatic: false, isPrivate: true, isFinal: true });
      }
      const hasInit = recordMethods.some((m) => m.name === "<init>");
      if (!hasInit) {
        const initBody = components.map((c) => ({
          kind: "assign",
          target: { kind: "fieldAccess", object: { kind: "this" }, field: c.name },
          value: { kind: "ident", name: c.name }
        }));
        recordMethods.push({
          name: "<init>",
          returnType: "void",
          params: components,
          body: initBody,
          isStatic: false
        });
      }
      for (const c of components) {
        const alreadyDeclared = recordMethods.some((m) => m.name === c.name && m.params.length === 0);
        if (!alreadyDeclared) {
          recordMethods.push({
            name: c.name,
            returnType: c.type,
            params: [],
            body: [{ kind: "return", value: { kind: "fieldAccess", object: { kind: "this" }, field: c.name } }],
            isStatic: false
          });
        }
      }
      if (!recordMethods.some((m) => m.name === "equals" && m.params.length === 1)) {
        recordMethods.push({
          name: "equals",
          returnType: "boolean",
          params: [{ name: "other", type: { className: "java/lang/Object" } }],
          body: [{
            kind: "return",
            value: {
              kind: "binary",
              op: "==",
              left: { kind: "this" },
              right: { kind: "ident", name: "other" }
            }
          }],
          isStatic: false
        });
      }
      if (!recordMethods.some((m) => m.name === "hashCode" && m.params.length === 0)) {
        recordMethods.push({
          name: "hashCode",
          returnType: "int",
          params: [],
          body: [{ kind: "return", value: { kind: "intLit", value: 0 } }],
          isStatic: false
        });
      }
      if (!recordMethods.some((m) => m.name === "toString" && m.params.length === 0)) {
        recordMethods.push({
          name: "toString",
          returnType: "String",
          params: [],
          body: [{ kind: "return", value: { kind: "stringLit", value: `${recordName}[]` } }],
          isStatic: false
        });
      }
      return {
        name: recordName,
        superClass: "java/lang/Record",
        isRecord: true,
        recordComponents: components,
        fields: recordFields,
        methods: recordMethods,
        importMap,
        packageImports,
        staticWildcardImports
      };
    }
    expect("class" /* KwClass */);
    const className = expect("Ident" /* Ident */).value;
    let superClass = "java/lang/Object";
    if (match("extends" /* KwExtends */)) {
      superClass = parseQualifiedName().replace(/\./g, "/");
    }
    if (match("implements" /* KwImplements */)) {
      parseQualifiedName();
      while (match("," /* Comma */)) parseQualifiedName();
    }
    expect("{" /* LBrace */);
    const fields = [];
    const methods = [];
    while (!at("}" /* RBrace */) && !at("EOF" /* EOF */)) {
      parseMember(fields, methods, className, false);
    }
    expect("}" /* RBrace */);
    return {
      name: className,
      superClass,
      isRecord: false,
      recordComponents: [],
      fields,
      methods,
      importMap,
      packageImports,
      staticWildcardImports
    };
  }
  function parseMember(fields, methods, ownerName, inRecord) {
    let isStatic = false;
    while (true) {
      if (at("public" /* KwPublic */) || at("private" /* KwPrivate */) || at("protected" /* KwProtected */)) {
        advance();
        continue;
      }
      if (at("static" /* KwStatic */)) {
        advance();
        isStatic = true;
        continue;
      }
      if (at("final" /* KwFinal */) || at("abstract" /* KwAbstract */)) {
        advance();
        continue;
      }
      break;
    }
    if (at("Ident" /* Ident */) && tokens[pos + 1]?.kind === "(" /* LParen */ && peek().value === ownerName) {
      advance();
      expect("(" /* LParen */);
      const params = [];
      if (!at(")" /* RParen */)) {
        do {
          const pType = parseType();
          const pName = expect("Ident" /* Ident */).value;
          params.push({ name: pName, type: pType });
        } while (match("," /* Comma */));
      }
      expect(")" /* RParen */);
      expect("{" /* LBrace */);
      const body = parseBlock();
      expect("}" /* RBrace */);
      methods.push({ name: "<init>", returnType: "void", params, body, isStatic: false });
      return;
    }
    const retType = parseType();
    const name = expect("Ident" /* Ident */).value;
    if (at("(" /* LParen */)) {
      expect("(" /* LParen */);
      const params = [];
      if (!at(")" /* RParen */)) {
        do {
          const pType = parseType();
          const pName = expect("Ident" /* Ident */).value;
          params.push({ name: pName, type: pType });
        } while (match("," /* Comma */));
      }
      expect(")" /* RParen */);
      expect("{" /* LBrace */);
      const body = parseBlock();
      expect("}" /* RBrace */);
      methods.push({ name, returnType: retType, params, body, isStatic });
    } else {
      let init;
      if (match("=" /* Assign */)) {
        init = parseExpr();
      }
      expect(";" /* Semi */);
      fields.push({ name, type: retType, isStatic, initializer: init, isPrivate: inRecord && !isStatic, isFinal: inRecord && !isStatic });
    }
  }
  function parseType() {
    let base;
    if (match("int" /* KwInt */)) base = "int";
    else if (match("long" /* KwLong */)) base = "long";
    else if (match("short" /* KwShort */)) base = "short";
    else if (match("byte" /* KwByte */)) base = "byte";
    else if (match("char" /* KwChar */)) base = "char";
    else if (match("float" /* KwFloat */)) base = "float";
    else if (match("double" /* KwDouble */)) base = "double";
    else if (match("boolean" /* KwBoolean */)) base = "boolean";
    else if (match("void" /* KwVoid */)) base = "void";
    else if (match("String" /* KwString */)) base = "String";
    else if (match("var" /* KwVar */)) throw new Error(`'var' is only allowed for local variables with initializer`);
    else {
      const name = expect("Ident" /* Ident */).value;
      if (at("<" /* Lt */)) {
        advance();
        let depth = 1;
        while (depth > 0 && !at("EOF" /* EOF */)) {
          if (at("<" /* Lt */)) depth++;
          if (at(">" /* Gt */)) depth--;
          advance();
        }
      }
      base = { className: name };
    }
    if (at("[" /* LBracket */) && tokens[pos + 1]?.kind === "]" /* RBracket */) {
      advance();
      advance();
      return { array: base };
    }
    return base;
  }
  function parseBlock() {
    const stmts = [];
    while (!at("}" /* RBrace */) && !at("EOF" /* EOF */)) {
      stmts.push(parseStmt());
    }
    return stmts;
  }
  function parseSwitchLabel() {
    function parsePatternBindVar() {
      if (match("var" /* KwVar */)) return expect("Ident" /* Ident */).value;
      if ((at("int" /* KwInt */) || at("long" /* KwLong */) || at("boolean" /* KwBoolean */) || at("String" /* KwString */) || at("Ident" /* Ident */)) && tokens[pos + 1]?.kind === "Ident" /* Ident */) {
        advance();
      }
      return expect("Ident" /* Ident */).value;
    }
    function parseRecordPatternBindVars() {
      const bindVars = [];
      expect("(" /* LParen */);
      if (!at(")" /* RParen */)) {
        do {
          bindVars.push(parsePatternBindVar());
        } while (match("," /* Comma */));
      }
      expect(")" /* RParen */);
      return bindVars;
    }
    if (match("(" /* LParen */)) {
      const nested = parseSwitchLabel();
      if (nested.kind !== "typePattern") {
        if (nested.kind !== "recordPattern") {
          throw new Error("parenthesized switch label currently supports only type/record patterns");
        }
      }
      expect(")" /* RParen */);
      return nested;
    }
    if (at("NullLiteral" /* NullLiteral */)) {
      advance();
      return { kind: "null" };
    }
    if (at("BoolLiteral" /* BoolLiteral */)) {
      return { kind: "bool", value: advance().value === "true" };
    }
    if (at("IntLiteral" /* IntLiteral */)) {
      return { kind: "int", value: parseIntLiteral(advance().value) };
    }
    if (at("StringLiteral" /* StringLiteral */)) {
      return { kind: "string", value: advance().value };
    }
    if (at("Ident" /* Ident */)) {
      const typeName = parseQualifiedName();
      if (at("(" /* LParen */)) {
        return { kind: "recordPattern", typeName, bindVars: parseRecordPatternBindVars() };
      }
      const bindVar = expect("Ident" /* Ident */).value;
      return { kind: "typePattern", typeName, bindVar };
    }
    if (at("String" /* KwString */)) {
      advance();
      const bindVar = expect("Ident" /* Ident */).value;
      return { kind: "typePattern", typeName: "java/lang/String", bindVar };
    }
    throw new Error(`Unsupported switch label at line ${peek().line}:${peek().col}`);
  }
  function parseSwitchCases(isExpr) {
    const cases = [];
    expect("{" /* LBrace */);
    while (!at("}" /* RBrace */) && !at("EOF" /* EOF */)) {
      const labels = [];
      if (match("default" /* KwDefault */)) {
        labels.push({ kind: "default" });
      } else {
        expect("case" /* KwCase */);
        labels.push(parseSwitchLabel());
        while (match("," /* Comma */)) labels.push(parseSwitchLabel());
      }
      let guard;
      if (match("when" /* KwWhen */)) {
        guard = parseExpr();
      }
      expect("->" /* Arrow */);
      if (isExpr) {
        if (at("{" /* LBrace */)) {
          expect("{" /* LBrace */);
          const stmts = [];
          while (!at("}" /* RBrace */) && !at("EOF" /* EOF */)) stmts.push(parseStmt());
          expect("}" /* RBrace */);
          cases.push({ labels, guard, stmts });
        } else {
          const expr = parseExpr();
          expect(";" /* Semi */);
          cases.push({ labels, guard, expr });
        }
      } else {
        if (at("{" /* LBrace */)) {
          expect("{" /* LBrace */);
          const stmts = [];
          while (!at("}" /* RBrace */) && !at("EOF" /* EOF */)) stmts.push(parseStmt());
          expect("}" /* RBrace */);
          cases.push({ labels, guard, stmts });
        } else {
          const stmt = parseStmt();
          cases.push({ labels, guard, stmts: [stmt] });
        }
      }
    }
    expect("}" /* RBrace */);
    validateSwitchCases(cases, isExpr);
    return cases;
  }
  function validateSwitchCases(cases, isExpr) {
    let defaultCount = 0;
    let nullCount = 0;
    let seenDefaultNoGuard = false;
    const seenConstLabels = /* @__PURE__ */ new Set();
    const seenUnguardedTypePatterns = /* @__PURE__ */ new Set();
    for (const c of cases) {
      let caseHasDefaultNoGuard = false;
      for (const l of c.labels) {
        if (l.kind === "default") {
          defaultCount++;
          if (defaultCount > 1) throw new Error("switch cannot have more than one default label");
        }
        if (l.kind === "null") {
          nullCount++;
          if (nullCount > 1) throw new Error("switch cannot have more than one null label");
        }
        if (l.kind === "default" && !c.guard) caseHasDefaultNoGuard = true;
        if (l.kind === "int") {
          const key = `int:${l.value}`;
          if (seenConstLabels.has(key)) throw new Error(`duplicate switch label: ${l.value}`);
          seenConstLabels.add(key);
        }
        if (l.kind === "bool") {
          const key = `bool:${l.value ? 1 : 0}`;
          if (seenConstLabels.has(key)) throw new Error(`duplicate switch label: ${l.value}`);
          seenConstLabels.add(key);
        }
        if (l.kind === "string") {
          const key = `str:${l.value}`;
          if (seenConstLabels.has(key)) throw new Error(`duplicate switch label: "${l.value}"`);
          seenConstLabels.add(key);
        }
        if (l.kind === "null") {
          const key = "null";
          if (seenConstLabels.has(key)) throw new Error("duplicate switch label: null");
          seenConstLabels.add(key);
        }
        if ((l.kind === "typePattern" || l.kind === "recordPattern") && seenUnguardedTypePatterns.has(l.typeName) && !c.guard) {
          throw new Error(`dominated switch label pattern: ${l.typeName}`);
        }
      }
      if (c.guard) {
        if (c.labels.length !== 1 || c.labels[0].kind !== "typePattern" && c.labels[0].kind !== "recordPattern") {
          throw new Error("switch guard 'when' is only supported with a single type pattern label");
        }
      }
      const unguardedPattern = c.labels.find((l) => l.kind === "typePattern" || l.kind === "recordPattern");
      if (unguardedPattern && !c.guard) {
        seenUnguardedTypePatterns.add(unguardedPattern.typeName);
      }
      if (isExpr && !c.expr && !(c.stmts && c.stmts.some((s) => s.kind === "yield"))) {
        throw new Error("switch expression case must provide value expression or yield");
      }
      if (seenDefaultNoGuard && !caseHasDefaultNoGuard) {
        throw new Error("switch has unreachable case after unguarded default");
      }
      if (caseHasDefaultNoGuard) seenDefaultNoGuard = true;
    }
  }
  function parseStmt() {
    if (at("{" /* LBrace */)) {
      expect("{" /* LBrace */);
      const stmts = parseBlock();
      expect("}" /* RBrace */);
      return { kind: "block", stmts };
    }
    if (at("return" /* KwReturn */)) {
      advance();
      if (at(";" /* Semi */)) {
        advance();
        return { kind: "return" };
      }
      const value = parseExpr();
      expect(";" /* Semi */);
      return { kind: "return", value };
    }
    if (at("yield" /* KwYield */)) {
      advance();
      const value = parseExpr();
      expect(";" /* Semi */);
      return { kind: "yield", value };
    }
    if (at("if" /* KwIf */)) {
      advance();
      expect("(" /* LParen */);
      const cond = parseExpr();
      expect(")" /* RParen */);
      let then;
      if (at("{" /* LBrace */)) {
        expect("{" /* LBrace */);
        then = parseBlock();
        expect("}" /* RBrace */);
      } else {
        then = [parseStmt()];
      }
      let else_;
      if (match("else" /* KwElse */)) {
        if (at("{" /* LBrace */)) {
          expect("{" /* LBrace */);
          else_ = parseBlock();
          expect("}" /* RBrace */);
        } else {
          else_ = [parseStmt()];
        }
      }
      return { kind: "if", cond, then, else_ };
    }
    if (at("while" /* KwWhile */)) {
      advance();
      expect("(" /* LParen */);
      const cond = parseExpr();
      expect(")" /* RParen */);
      expect("{" /* LBrace */);
      const body = parseBlock();
      expect("}" /* RBrace */);
      return { kind: "while", cond, body };
    }
    if (at("for" /* KwFor */)) {
      advance();
      expect("(" /* LParen */);
      let init;
      if (!at(";" /* Semi */)) init = parseStmtNoSemi();
      expect(";" /* Semi */);
      let cond;
      if (!at(";" /* Semi */)) cond = parseExpr();
      expect(";" /* Semi */);
      let update;
      if (!at(")" /* RParen */)) update = parseStmtNoSemi();
      expect(")" /* RParen */);
      expect("{" /* LBrace */);
      const body = parseBlock();
      expect("}" /* RBrace */);
      return { kind: "for", init, cond, update, body };
    }
    if (at("switch" /* KwSwitch */)) {
      advance();
      expect("(" /* LParen */);
      const selector = parseExpr();
      expect(")" /* RParen */);
      const cases = parseSwitchCases(false);
      return { kind: "switch", selector, cases };
    }
    if (at("var" /* KwVar */)) {
      advance();
      const name = expect("Ident" /* Ident */).value;
      expect("=" /* Assign */);
      const init = parseExpr();
      expect(";" /* Semi */);
      return { kind: "varDecl", name, type: inferLocalVarType(init), init };
    }
    if (isTypeStart() && isVarDecl()) {
      const type = parseType();
      const name = expect("Ident" /* Ident */).value;
      let init;
      if (match("=" /* Assign */)) init = parseExpr();
      expect(";" /* Semi */);
      return { kind: "varDecl", name, type, init };
    }
    const expr = parseExpr();
    if (match("=" /* Assign */)) {
      const value = parseExpr();
      expect(";" /* Semi */);
      return { kind: "assign", target: expr, value };
    }
    if (match("+=" /* PlusAssign */)) {
      const value = parseExpr();
      expect(";" /* Semi */);
      return { kind: "assign", target: expr, value: { kind: "binary", op: "+", left: expr, right: value } };
    }
    if (match("-=" /* MinusAssign */)) {
      const value = parseExpr();
      expect(";" /* Semi */);
      return { kind: "assign", target: expr, value: { kind: "binary", op: "-", left: expr, right: value } };
    }
    expect(";" /* Semi */);
    return { kind: "exprStmt", expr };
  }
  function parseStmtNoSemi() {
    if (at("var" /* KwVar */)) {
      advance();
      const name = expect("Ident" /* Ident */).value;
      expect("=" /* Assign */);
      const init = parseExpr();
      return { kind: "varDecl", name, type: inferLocalVarType(init), init };
    }
    if (isTypeStart() && isVarDecl()) {
      const type = parseType();
      const name = expect("Ident" /* Ident */).value;
      let init;
      if (match("=" /* Assign */)) init = parseExpr();
      return { kind: "varDecl", name, type, init };
    }
    const expr = parseExpr();
    if (match("=" /* Assign */)) {
      const value = parseExpr();
      return { kind: "assign", target: expr, value };
    }
    if (match("+=" /* PlusAssign */)) {
      const value = parseExpr();
      return { kind: "assign", target: expr, value: { kind: "binary", op: "+", left: expr, right: value } };
    }
    if (match("++" /* PlusPlus */)) {
      return { kind: "assign", target: expr, value: { kind: "binary", op: "+", left: expr, right: { kind: "intLit", value: 1 } } };
    }
    if (match("--" /* MinusMinus */)) {
      return { kind: "assign", target: expr, value: { kind: "binary", op: "-", left: expr, right: { kind: "intLit", value: 1 } } };
    }
    return { kind: "exprStmt", expr };
  }
  function isTypeStart() {
    const k = peek().kind;
    return k === "int" /* KwInt */ || k === "long" /* KwLong */ || k === "short" /* KwShort */ || k === "byte" /* KwByte */ || k === "char" /* KwChar */ || k === "float" /* KwFloat */ || k === "double" /* KwDouble */ || k === "boolean" /* KwBoolean */ || k === "void" /* KwVoid */ || k === "String" /* KwString */ || k === "Ident" /* Ident */;
  }
  function isVarDecl() {
    const saved = pos;
    try {
      if (at("int" /* KwInt */) || at("long" /* KwLong */) || at("short" /* KwShort */) || at("byte" /* KwByte */) || at("char" /* KwChar */) || at("float" /* KwFloat */) || at("double" /* KwDouble */) || at("boolean" /* KwBoolean */) || at("void" /* KwVoid */) || at("String" /* KwString */)) {
        advance();
        if (at("[" /* LBracket */) && tokens[pos + 1]?.kind === "]" /* RBracket */) {
          advance();
          advance();
        }
      } else if (at("Ident" /* Ident */)) {
        advance();
        if (at("<" /* Lt */)) {
          let depth = 1;
          advance();
          while (depth > 0 && !at("EOF" /* EOF */)) {
            if (at("<" /* Lt */)) depth++;
            if (at(">" /* Gt */)) depth--;
            advance();
          }
        }
      } else {
        return false;
      }
      if (at("(" /* LParen */)) return false;
      if (at("[" /* LBracket */) && tokens[pos + 1]?.kind === "]" /* RBracket */) {
        advance();
        advance();
      }
      if (!at("Ident" /* Ident */)) return false;
      advance();
      if (at("(" /* LParen */)) return false;
      return true;
    } finally {
      pos = saved;
    }
  }
  function parseExpr() {
    if (isLambdaStart()) {
      return parseLambdaExpr();
    }
    const expr = parseOr();
    if (at("?" /* Question */)) {
      advance();
      const thenExpr = parseExpr();
      expect(":" /* Colon */);
      const elseExpr = parseExpr();
      return { kind: "ternary", cond: expr, thenExpr, elseExpr };
    }
    return expr;
  }
  function isLambdaStart() {
    if (at("Ident" /* Ident */) && tokens[pos + 1]?.kind === "->" /* Arrow */) return true;
    if (!at("(" /* LParen */)) return false;
    let i = pos + 1;
    let expectIdent = true;
    while (i < tokens.length && tokens[i].kind !== ")" /* RParen */) {
      const k = tokens[i].kind;
      if (expectIdent) {
        if (k !== "Ident" /* Ident */) return false;
        expectIdent = false;
      } else {
        if (k !== "," /* Comma */) return false;
        expectIdent = true;
      }
      i++;
    }
    if (i >= tokens.length || tokens[i].kind !== ")" /* RParen */) return false;
    return tokens[i + 1]?.kind === "->" /* Arrow */;
  }
  function parseLambdaExpr() {
    const params = [];
    if (at("Ident" /* Ident */) && tokens[pos + 1]?.kind === "->" /* Arrow */) {
      params.push(advance().value);
      expect("->" /* Arrow */);
    } else {
      expect("(" /* LParen */);
      if (!at(")" /* RParen */)) {
        do {
          params.push(expect("Ident" /* Ident */).value);
        } while (match("," /* Comma */));
      }
      expect(")" /* RParen */);
      expect("->" /* Arrow */);
    }
    if (at("{" /* LBrace */)) {
      expect("{" /* LBrace */);
      const bodyStmts = parseBlock();
      expect("}" /* RBrace */);
      return { kind: "lambda", params, bodyStmts };
    }
    const bodyExpr = parseExpr();
    return { kind: "lambda", params, bodyExpr };
  }
  function inferLocalVarType(init) {
    switch (init.kind) {
      case "intLit":
        return "int";
      case "longLit":
        return "long";
      case "floatLit":
        return "float";
      case "doubleLit":
        return "double";
      case "charLit":
        return "char";
      case "boolLit":
        return "boolean";
      case "stringLit":
        return "String";
      case "newArray":
        return { array: init.elemType };
      case "arrayLit":
        return { array: init.elemType };
      case "newExpr":
        return { className: init.className };
      case "cast":
        return init.type;
      default:
        return { className: "java/lang/Object" };
    }
  }
  function parseOr() {
    let left = parseAnd();
    while (at("||" /* Or */)) {
      advance();
      const right = parseAnd();
      left = { kind: "binary", op: "||", left, right };
    }
    return left;
  }
  function parseAnd() {
    let left = parseEquality();
    while (at("&&" /* And */)) {
      advance();
      const right = parseEquality();
      left = { kind: "binary", op: "&&", left, right };
    }
    return left;
  }
  function parseEquality() {
    let left = parseComparison();
    while (at("==" /* Eq */) || at("!=" /* Ne */) || at("instanceof" /* KwInstanceof */)) {
      if (at("instanceof" /* KwInstanceof */)) {
        let parsePatternBindVar = function() {
          if (match("var" /* KwVar */)) return expect("Ident" /* Ident */).value;
          if ((at("int" /* KwInt */) || at("long" /* KwLong */) || at("boolean" /* KwBoolean */) || at("String" /* KwString */) || at("Ident" /* Ident */)) && tokens[pos + 1]?.kind === "Ident" /* Ident */) {
            advance();
          }
          return expect("Ident" /* Ident */).value;
        }, parseInstanceofPattern = function() {
          if (match("(" /* LParen */)) {
            const inner = parseInstanceofPattern();
            expect(")" /* RParen */);
            return inner;
          }
          let typeName;
          if (at("String" /* KwString */)) {
            advance();
            typeName = "java/lang/String";
          } else {
            typeName = parseQualifiedName();
          }
          if (at("<" /* Lt */)) {
            let depth = 1;
            advance();
            while (depth > 0 && !at("EOF" /* EOF */)) {
              if (at("<" /* Lt */)) depth++;
              if (at(">" /* Gt */)) depth--;
              advance();
            }
          }
          if (at("(" /* LParen */)) {
            const bindVars = [];
            advance();
            if (!at(")" /* RParen */)) {
              do {
                bindVars.push(parsePatternBindVar());
              } while (match("," /* Comma */));
            }
            expect(")" /* RParen */);
            return { typeName, recordBindVars: bindVars };
          }
          let bindVar;
          if (at("Ident" /* Ident */)) bindVar = advance().value;
          return { typeName, bindVar };
        };
        advance();
        const p = parseInstanceofPattern();
        left = { kind: "instanceof", expr: left, checkType: p.typeName, bindVar: p.bindVar, recordBindVars: p.recordBindVars };
      } else {
        const op = advance().value;
        const right = parseComparison();
        left = { kind: "binary", op, left, right };
      }
    }
    return left;
  }
  function parseComparison() {
    let left = parseAdditive();
    while (at("<" /* Lt */) || at(">" /* Gt */) || at("<=" /* Le */) || at(">=" /* Ge */)) {
      const op = advance().value;
      const right = parseAdditive();
      left = { kind: "binary", op, left, right };
    }
    return left;
  }
  function parseAdditive() {
    let left = parseMultiplicative();
    while (at("+" /* Plus */) || at("-" /* Minus */)) {
      const op = advance().value;
      const right = parseMultiplicative();
      left = { kind: "binary", op, left, right };
    }
    return left;
  }
  function parseMultiplicative() {
    let left = parseUnary();
    while (at("*" /* Star */) || at("/" /* Slash */) || at("%" /* Percent */)) {
      const op = advance().value;
      const right = parseUnary();
      left = { kind: "binary", op, left, right };
    }
    return left;
  }
  function parseUnary() {
    if (at("-" /* Minus */)) {
      advance();
      const operand = parseUnary();
      return { kind: "unary", op: "-", operand };
    }
    if (at("!" /* Not */)) {
      advance();
      const operand = parseUnary();
      return { kind: "unary", op: "!", operand };
    }
    return parsePostfix();
  }
  function parsePostfix() {
    let expr = parsePrimary();
    while (true) {
      if (at("." /* Dot */)) {
        advance();
        const name = expect("Ident" /* Ident */).value;
        if (at("(" /* LParen */)) {
          expect("(" /* LParen */);
          const args = [];
          if (!at(")" /* RParen */)) {
            do {
              args.push(parseExpr());
            } while (match("," /* Comma */));
          }
          expect(")" /* RParen */);
          expr = { kind: "call", object: expr, method: name, args };
        } else {
          expr = { kind: "fieldAccess", object: expr, field: name };
        }
      } else if (at("[" /* LBracket */)) {
        advance();
        const index = parseExpr();
        expect("]" /* RBracket */);
        expr = { kind: "arrayAccess", array: expr, index };
      } else if (at("++" /* PlusPlus */)) {
        advance();
        expr = { kind: "postIncrement", operand: expr, op: "++" };
      } else if (at("--" /* MinusMinus */)) {
        advance();
        expr = { kind: "postIncrement", operand: expr, op: "--" };
      } else if (at("::" /* ColonColon */)) {
        advance();
        if (match("new" /* KwNew */)) {
          expr = { kind: "methodRef", target: expr, method: "<init>", isConstructor: true };
        } else {
          const method = expect("Ident" /* Ident */).value;
          expr = { kind: "methodRef", target: expr, method, isConstructor: false };
        }
        break;
      } else {
        break;
      }
    }
    return expr;
  }
  function parsePrimary() {
    if (at("IntLiteral" /* IntLiteral */)) {
      return { kind: "intLit", value: parseIntLiteral(advance().value) };
    }
    if (at("LongLiteral" /* LongLiteral */)) {
      return { kind: "longLit", value: parseIntLiteral(advance().value) };
    }
    if (at("FloatLiteral" /* FloatLiteral */)) {
      let raw = advance().value.replace(/_/g, "");
      if (raw.endsWith("f") || raw.endsWith("F")) raw = raw.slice(0, -1);
      return { kind: "floatLit", value: parseFloat(raw) };
    }
    if (at("DoubleLiteral" /* DoubleLiteral */)) {
      let raw = advance().value.replace(/_/g, "");
      if (raw.endsWith("d") || raw.endsWith("D")) raw = raw.slice(0, -1);
      return { kind: "doubleLit", value: parseFloat(raw) };
    }
    if (at("CharLiteral" /* CharLiteral */)) {
      return { kind: "charLit", value: parseInt(advance().value, 10) };
    }
    if (at("StringLiteral" /* StringLiteral */)) {
      return { kind: "stringLit", value: advance().value };
    }
    if (at("BoolLiteral" /* BoolLiteral */)) {
      return { kind: "boolLit", value: advance().value === "true" };
    }
    if (at("NullLiteral" /* NullLiteral */)) {
      advance();
      return { kind: "nullLit" };
    }
    if (at("this" /* KwThis */)) {
      advance();
      return { kind: "this" };
    }
    if (at("String" /* KwString */)) {
      advance();
      return { kind: "ident", name: "String" };
    }
    if (at("switch" /* KwSwitch */)) {
      advance();
      expect("(" /* LParen */);
      const selector = parseExpr();
      expect(")" /* RParen */);
      const cases = parseSwitchCases(true);
      return { kind: "switchExpr", selector, cases };
    }
    if (at("super" /* KwSuper */)) {
      advance();
      expect("(" /* LParen */);
      const args = [];
      if (!at(")" /* RParen */)) {
        do {
          args.push(parseExpr());
        } while (match("," /* Comma */));
      }
      expect(")" /* RParen */);
      return { kind: "superCall", args };
    }
    if (at("{" /* LBrace */)) {
      advance();
      const elements = [];
      if (!at("}" /* RBrace */)) {
        do {
          elements.push(parseExpr());
        } while (match("," /* Comma */));
      }
      expect("}" /* RBrace */);
      return { kind: "arrayLit", elemType: "int", elements };
    }
    if (at("new" /* KwNew */)) {
      advance();
      if (at("int" /* KwInt */) || at("boolean" /* KwBoolean */)) {
        const elemType = at("int" /* KwInt */) ? "int" : "boolean";
        advance();
        expect("[" /* LBracket */);
        const size = parseExpr();
        expect("]" /* RBracket */);
        return { kind: "newArray", elemType, size };
      }
      const cls = expect("Ident" /* Ident */).value;
      if (at("[" /* LBracket */)) {
        advance();
        const size = parseExpr();
        expect("]" /* RBracket */);
        return { kind: "newArray", elemType: { className: cls }, size };
      }
      if (at("<" /* Lt */)) {
        let depth = 1;
        advance();
        while (depth > 0 && !at("EOF" /* EOF */)) {
          if (at("<" /* Lt */)) depth++;
          if (at(">" /* Gt */)) depth--;
          advance();
        }
      }
      expect("(" /* LParen */);
      const args = [];
      if (!at(")" /* RParen */)) {
        do {
          args.push(parseExpr());
        } while (match("," /* Comma */));
      }
      expect(")" /* RParen */);
      return { kind: "newExpr", className: cls, args };
    }
    if (at("(" /* LParen */)) {
      const savedPos = pos;
      advance();
      if (at("Ident" /* Ident */) || at("String" /* KwString */) || at("int" /* KwInt */) || at("long" /* KwLong */) || at("short" /* KwShort */) || at("byte" /* KwByte */) || at("char" /* KwChar */) || at("float" /* KwFloat */) || at("double" /* KwDouble */) || at("boolean" /* KwBoolean */)) {
        let typeName = advance().value;
        if (at("<" /* Lt */)) {
          let depth = 1;
          advance();
          while (depth > 0 && !at("EOF" /* EOF */)) {
            if (at("<" /* Lt */)) depth++;
            if (at(">" /* Gt */)) depth--;
            advance();
          }
        }
        if (at(")" /* RParen */)) {
          advance();
          if (at("Ident" /* Ident */) || at("this" /* KwThis */) || at("new" /* KwNew */) || at("(" /* LParen */) || at("IntLiteral" /* IntLiteral */) || at("StringLiteral" /* StringLiteral */) || at("LongLiteral" /* LongLiteral */) || at("FloatLiteral" /* FloatLiteral */) || at("DoubleLiteral" /* DoubleLiteral */) || at("CharLiteral" /* CharLiteral */) || at("BoolLiteral" /* BoolLiteral */) || at("NullLiteral" /* NullLiteral */)) {
            const castExpr = parseUnary();
            const castType = typeName === "String" ? "String" : typeName === "int" ? "int" : typeName === "long" ? "long" : typeName === "short" ? "short" : typeName === "byte" ? "byte" : typeName === "char" ? "char" : typeName === "float" ? "float" : typeName === "double" ? "double" : typeName === "boolean" ? "boolean" : { className: typeName };
            return { kind: "cast", type: castType, expr: castExpr };
          }
        }
        pos = savedPos;
        advance();
      }
      const expr = parseExpr();
      expect(")" /* RParen */);
      return expr;
    }
    if (at("Ident" /* Ident */)) {
      const name = advance().value;
      if (at("(" /* LParen */)) {
        expect("(" /* LParen */);
        const args = [];
        if (!at(")" /* RParen */)) {
          do {
            args.push(parseExpr());
          } while (match("," /* Comma */));
        }
        expect(")" /* RParen */);
        return { kind: "call", method: name, args };
      }
      return { kind: "ident", name };
    }
    throw new Error(`Unexpected token: ${peek().kind} ("${peek().value}") at line ${peek().line}:${peek().col}`);
  }
}
var ConstantPoolBuilder = class {
  constructor() {
    __publicField(this, "entries", [{ tag: 0, data: [] }]);
    // index 0 placeholder
    __publicField(this, "utf8Cache", /* @__PURE__ */ new Map());
  }
  get count() {
    return this.entries.length;
  }
  addUtf8(s) {
    const cached = this.utf8Cache.get(s);
    if (cached !== void 0) return cached;
    const bytes = new TextEncoder().encode(s);
    const data = [bytes.length >> 8 & 255, bytes.length & 255, ...bytes];
    const idx = this.entries.length;
    this.entries.push({ tag: 1, data });
    this.utf8Cache.set(s, idx);
    return idx;
  }
  addInteger(v) {
    const data = [v >> 24 & 255, v >> 16 & 255, v >> 8 & 255, v & 255];
    const idx = this.entries.length;
    this.entries.push({ tag: 3, data });
    return idx;
  }
  addFloat(v) {
    const buf = new ArrayBuffer(4);
    new DataView(buf).setFloat32(0, v);
    const bytes = new Uint8Array(buf);
    const data = [bytes[0], bytes[1], bytes[2], bytes[3]];
    const idx = this.entries.length;
    this.entries.push({ tag: 4, data });
    return idx;
  }
  addDouble(v) {
    const buf = new ArrayBuffer(8);
    new DataView(buf).setFloat64(0, v);
    const bytes = new Uint8Array(buf);
    const data = [...bytes];
    const idx = this.entries.length;
    this.entries.push({ tag: 6, data });
    this.entries.push({ tag: 0, data: [] });
    return idx;
  }
  addLong(v) {
    const hi = Math.floor(v / 4294967296);
    const lo = v >>> 0;
    const data = [
      hi >> 24 & 255,
      hi >> 16 & 255,
      hi >> 8 & 255,
      hi & 255,
      lo >> 24 & 255,
      lo >> 16 & 255,
      lo >> 8 & 255,
      lo & 255
    ];
    const idx = this.entries.length;
    this.entries.push({ tag: 5, data });
    this.entries.push({ tag: 0, data: [] });
    return idx;
  }
  addClass(name) {
    const nameIdx = this.addUtf8(name);
    const idx = this.entries.length;
    this.entries.push({ tag: 7, data: [nameIdx >> 8 & 255, nameIdx & 255] });
    return idx;
  }
  addString(s) {
    const strIdx = this.addUtf8(s);
    const idx = this.entries.length;
    this.entries.push({ tag: 8, data: [strIdx >> 8 & 255, strIdx & 255] });
    return idx;
  }
  addNameAndType(name, descriptor) {
    const nameIdx = this.addUtf8(name);
    const descIdx = this.addUtf8(descriptor);
    const idx = this.entries.length;
    this.entries.push({ tag: 12, data: [
      nameIdx >> 8 & 255,
      nameIdx & 255,
      descIdx >> 8 & 255,
      descIdx & 255
    ] });
    return idx;
  }
  addFieldref(className, fieldName, descriptor) {
    const classIdx = this.addClass(className);
    const natIdx = this.addNameAndType(fieldName, descriptor);
    const idx = this.entries.length;
    this.entries.push({ tag: 9, data: [
      classIdx >> 8 & 255,
      classIdx & 255,
      natIdx >> 8 & 255,
      natIdx & 255
    ] });
    return idx;
  }
  addMethodref(className, methodName, descriptor) {
    const classIdx = this.addClass(className);
    const natIdx = this.addNameAndType(methodName, descriptor);
    const idx = this.entries.length;
    this.entries.push({ tag: 10, data: [
      classIdx >> 8 & 255,
      classIdx & 255,
      natIdx >> 8 & 255,
      natIdx & 255
    ] });
    return idx;
  }
  addInterfaceMethodref(className, methodName, descriptor) {
    const classIdx = this.addClass(className);
    const natIdx = this.addNameAndType(methodName, descriptor);
    const idx = this.entries.length;
    this.entries.push({ tag: 11, data: [
      classIdx >> 8 & 255,
      classIdx & 255,
      natIdx >> 8 & 255,
      natIdx & 255
    ] });
    return idx;
  }
  addMethodHandle(referenceKind, referenceIndex) {
    const idx = this.entries.length;
    this.entries.push({ tag: 15, data: [referenceKind & 255, referenceIndex >> 8 & 255, referenceIndex & 255] });
    return idx;
  }
  addMethodType(descriptor) {
    const descIdx = this.addUtf8(descriptor);
    const idx = this.entries.length;
    this.entries.push({ tag: 16, data: [descIdx >> 8 & 255, descIdx & 255] });
    return idx;
  }
  addInvokeDynamic(bootstrapMethodAttrIndex, name, descriptor) {
    const natIdx = this.addNameAndType(name, descriptor);
    const idx = this.entries.length;
    this.entries.push({ tag: 18, data: [
      bootstrapMethodAttrIndex >> 8 & 255,
      bootstrapMethodAttrIndex & 255,
      natIdx >> 8 & 255,
      natIdx & 255
    ] });
    return idx;
  }
  serialize() {
    const out = [];
    const count = this.entries.length;
    out.push(count >> 8 & 255, count & 255);
    for (let i = 1; i < count; i++) {
      const e = this.entries[i];
      out.push(e.tag, ...e.data);
    }
    return out;
  }
};
var BytecodeEmitter = class {
  constructor() {
    __publicField(this, "code", []);
    __publicField(this, "maxStack", 0);
    __publicField(this, "maxLocals", 0);
    __publicField(this, "currentStack", 0);
  }
  adjustStack(delta) {
    this.currentStack += delta;
    if (this.currentStack > this.maxStack) this.maxStack = this.currentStack;
  }
  emit(byte) {
    this.code.push(byte);
  }
  emitU16(v) {
    this.code.push(v >> 8 & 255, v & 255);
  }
  get pc() {
    return this.code.length;
  }
  // Stack-tracking emit helpers
  emitPush(opcode) {
    this.emit(opcode);
    this.adjustStack(1);
  }
  emitPop(opcode) {
    this.emit(opcode);
    this.adjustStack(-1);
  }
  emitIconst(v) {
    if (v >= -1 && v <= 5) {
      this.emit(3 + v);
      if (v === -1) this.code[this.code.length - 1] = 2;
      else this.code[this.code.length - 1] = 3 + v;
    } else if (v >= -128 && v <= 127) {
      this.emit(16);
      this.emit(v & 255);
    } else if (v >= -32768 && v <= 32767) {
      this.emit(17);
      this.emitU16(v & 65535);
    } else {
      return false;
    }
    this.adjustStack(1);
    return true;
  }
  emitFload(idx) {
    if (idx <= 3) this.emit(34 + idx);
    else {
      this.emit(23);
      this.emit(idx);
    }
    this.adjustStack(1);
  }
  emitFstore(idx) {
    if (idx <= 3) this.emit(67 + idx);
    else {
      this.emit(56);
      this.emit(idx);
    }
    this.adjustStack(-1);
  }
  emitDload(idx) {
    if (idx <= 3) this.emit(38 + idx);
    else {
      this.emit(24);
      this.emit(idx);
    }
    this.adjustStack(1);
  }
  emitDstore(idx) {
    if (idx <= 3) this.emit(71 + idx);
    else {
      this.emit(57);
      this.emit(idx);
    }
    this.adjustStack(-1);
  }
  emitFconst(v, cp) {
    if (v === 0) {
      this.emit(11);
      this.adjustStack(1);
    } else if (v === 1) {
      this.emit(12);
      this.adjustStack(1);
    } else if (v === 2) {
      this.emit(13);
      this.adjustStack(1);
    } else {
      this.emitLdc(cp.addFloat(v));
    }
  }
  emitDconst(v, cp) {
    if (v === 0) {
      this.emit(14);
    } else if (v === 1) {
      this.emit(15);
    } else {
      const cpIdx = cp.addDouble(v);
      this.emit(20);
      this.emitU16(cpIdx);
    }
    this.adjustStack(1);
  }
  emitLconst(v, cp) {
    if (v === 0) {
      this.emit(9);
    } else if (v === 1) {
      this.emit(10);
    } else {
      const cpIdx = cp.addLong(v);
      this.emit(20);
      this.emitU16(cpIdx);
    }
    this.adjustStack(1);
  }
  emitLdc(cpIdx) {
    if (cpIdx <= 255) {
      this.emit(18);
      this.emit(cpIdx);
    } else {
      this.emit(19);
      this.emitU16(cpIdx);
    }
    this.adjustStack(1);
  }
  emitAload(idx) {
    if (idx <= 3) this.emit(42 + idx);
    else {
      this.emit(25);
      this.emit(idx);
    }
    this.adjustStack(1);
  }
  emitAstore(idx) {
    if (idx <= 3) this.emit(75 + idx);
    else {
      this.emit(58);
      this.emit(idx);
    }
    this.adjustStack(-1);
  }
  emitIload(idx) {
    if (idx <= 3) this.emit(26 + idx);
    else {
      this.emit(21);
      this.emit(idx);
    }
    this.adjustStack(1);
  }
  emitIstore(idx) {
    if (idx <= 3) this.emit(59 + idx);
    else {
      this.emit(54);
      this.emit(idx);
    }
    this.adjustStack(-1);
  }
  emitLload(idx) {
    if (idx <= 3) this.emit(30 + idx);
    else {
      this.emit(22);
      this.emit(idx);
    }
    this.adjustStack(1);
  }
  emitLstore(idx) {
    if (idx <= 3) this.emit(63 + idx);
    else {
      this.emit(55);
      this.emit(idx);
    }
    this.adjustStack(-1);
  }
  emitInvokevirtual(cpIdx, argCount, hasReturn) {
    this.emit(182);
    this.emitU16(cpIdx);
    this.adjustStack(-(argCount + 1) + (hasReturn ? 1 : 0));
  }
  emitInvokespecial(cpIdx, argCount, hasReturn) {
    this.emit(183);
    this.emitU16(cpIdx);
    this.adjustStack(-(argCount + 1) + (hasReturn ? 1 : 0));
  }
  emitInvokestatic(cpIdx, argCount, hasReturn) {
    this.emit(184);
    this.emitU16(cpIdx);
    this.adjustStack(-argCount + (hasReturn ? 1 : 0));
  }
  emitInvokeinterface(cpIdx, argCount, hasReturn) {
    this.emit(185);
    this.emitU16(cpIdx);
    this.emit(argCount + 1);
    this.emit(0);
    this.adjustStack(-(argCount + 1) + (hasReturn ? 1 : 0));
  }
  emitInvokedynamic(cpIdx, argCount, hasReturn) {
    this.emit(186);
    this.emitU16(cpIdx);
    this.emit(0);
    this.emit(0);
    this.adjustStack(-argCount + (hasReturn ? 1 : 0));
  }
  // Branch helpers: emit placeholder offset, return patch position
  emitBranch(opcode) {
    this.emit(opcode);
    const patchPos = this.code.length;
    this.emitU16(0);
    return patchPos;
  }
  patchBranch(patchPos, targetPc) {
    const offset = targetPc - (patchPos - 1);
    this.code[patchPos] = offset >> 8 & 255;
    this.code[patchPos + 1] = offset & 255;
  }
  emitReturn(type) {
    if (type === "void") this.emit(177);
    else if (type === "long") {
      this.emit(173);
      this.adjustStack(-1);
    } else if (type === "float") {
      this.emit(174);
      this.adjustStack(-1);
    } else if (type === "double") {
      this.emit(175);
      this.adjustStack(-1);
    } else if (type === "int" || type === "boolean" || type === "short" || type === "byte" || type === "char") {
      this.emit(172);
      this.adjustStack(-1);
    } else {
      this.emit(176);
      this.adjustStack(-1);
    }
  }
};
function typeToDescriptor(t) {
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
function methodDescriptor(params, returnType) {
  return "(" + params.map((p) => typeToDescriptor(p.type)).join("") + ")" + typeToDescriptor(returnType);
}
function isRefType(t) {
  return t !== "int" && t !== "long" && t !== "short" && t !== "byte" && t !== "char" && t !== "float" && t !== "double" && t !== "boolean" && t !== "void";
}
function isPrimitiveType(t) {
  return t === "int" || t === "long" || t === "short" || t === "byte" || t === "char" || t === "float" || t === "double" || t === "boolean";
}
function sameType(a, b) {
  if (a === b) return true;
  if (typeof a === "object" && typeof b === "object") {
    if ("className" in a && "className" in b) return a.className === b.className;
    if ("array" in a && "array" in b) return sameType(a.array, b.array);
  }
  return false;
}
var WIDENING_RANK = {
  byte: 0,
  short: 1,
  int: 2,
  long: 3,
  float: 4,
  double: 5,
  char: 2
  /* char → int level */
};
function isAssignable(to, from) {
  if (sameType(to, from)) return true;
  if (isRefType(to) && isRefType(from)) return true;
  if (isIntLike(to) && isIntLike(from)) return true;
  const toR = typeof to === "string" ? WIDENING_RANK[to] : void 0;
  const fromR = typeof from === "string" ? WIDENING_RANK[from] : void 0;
  if (toR !== void 0 && fromR !== void 0) return toR >= fromR;
  return false;
}
function isKnownClass(ctx, cls) {
  return cls === "java/lang/Object" || ctx.classSupers.has(cls) || !!BUILTIN_SUPERS[cls];
}
function isAssignableInContext(ctx, to, from) {
  if (sameType(to, from)) return true;
  if (isPrimitiveType(to) && isPrimitiveType(from)) return isAssignable(to, from);
  if (isPrimitiveType(to) || isPrimitiveType(from)) return false;
  if (typeof to === "object" && "array" in to) {
    return typeof from === "object" && "array" in from && isAssignableInContext(ctx, to.array, from.array);
  }
  if (typeof from === "object" && "array" in from) {
    const toCls2 = toInternalClassName(ctx, to);
    return toCls2 === "java/lang/Object";
  }
  const toCls = toInternalClassName(ctx, to);
  const fromCls = toInternalClassName(ctx, from);
  if (!toCls || !fromCls) return isAssignable(to, from);
  if (toCls === "java/lang/Object") return true;
  if (fromCls === "java/lang/Object") return true;
  if (isClassSupertype(ctx, toCls, fromCls)) return true;
  if (isKnownClass(ctx, toCls) && isKnownClass(ctx, fromCls)) return false;
  return true;
}
function isCastConvertible(to, from) {
  if (sameType(to, from)) return true;
  const toPrim = isPrimitiveType(to);
  const fromPrim = isPrimitiveType(from);
  if (toPrim && fromPrim) {
    const numerics = ["byte", "short", "char", "int", "long", "float", "double"];
    return numerics.includes(to) && numerics.includes(from);
  }
  if (toPrim || fromPrim) return false;
  return true;
}
function mergeTernaryType(a, b) {
  if (sameType(a, b)) return a;
  const numOrder = ["byte", "short", "char", "int", "long", "float", "double"];
  const ai = numOrder.indexOf(a);
  const bi = numOrder.indexOf(b);
  if (ai >= 0 && bi >= 0) return numOrder[Math.max(ai, bi)];
  if (a === "int" && b === "boolean" || a === "boolean" && b === "int") return "int";
  if (isRefType(a) && isRefType(b)) return { className: "java/lang/Object" };
  return a;
}
var knownMethods = {
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
    isInterface: true
  },
  "java/util/function/BiFunction.apply(Ljava/lang/Object;Ljava/lang/Object;)": {
    owner: "java/util/function/BiFunction",
    returnType: { className: "java/lang/Object" },
    paramTypes: [{ className: "java/lang/Object" }, { className: "java/lang/Object" }],
    isInterface: true
  },
  "java/util/function/Predicate.test(Ljava/lang/Object;)": {
    owner: "java/util/function/Predicate",
    returnType: "boolean",
    paramTypes: [{ className: "java/lang/Object" }],
    isInterface: true
  },
  "java/util/function/Consumer.accept(Ljava/lang/Object;)": {
    owner: "java/util/function/Consumer",
    returnType: "void",
    paramTypes: [{ className: "java/lang/Object" }],
    isInterface: true
  },
  "java/util/function/Supplier.get()": {
    owner: "java/util/function/Supplier",
    returnType: { className: "java/lang/Object" },
    paramTypes: [],
    isInterface: true
  },
  "java/lang/Runnable.run()": {
    owner: "java/lang/Runnable",
    returnType: "void",
    paramTypes: [],
    isInterface: true
  },
  // PrintStream
  "java/io/PrintStream.println(Ljava/lang/String;)": { owner: "java/io/PrintStream", returnType: "void", paramTypes: ["String"] },
  "java/io/PrintStream.println(I)": { owner: "java/io/PrintStream", returnType: "void", paramTypes: ["int"] },
  "java/io/PrintStream.println(Ljava/lang/Object;)": { owner: "java/io/PrintStream", returnType: "void", paramTypes: [{ className: "java/lang/Object" }] },
  "java/io/PrintStream.print(Ljava/lang/String;)": { owner: "java/io/PrintStream", returnType: "void", paramTypes: ["String"] },
  "java/io/PrintStream.print(I)": { owner: "java/io/PrintStream", returnType: "void", paramTypes: ["int"] }
};
function setMethodRegistry(reg) {
  knownMethods = { ...knownMethods, ...reg };
}
var FUNCTIONAL_IFACES = {
  "java/lang/Runnable": { samMethod: "run", params: [], returnType: "void" },
  "java/util/function/Supplier": { samMethod: "get", params: [], returnType: { className: "java/lang/Object" } },
  "java/util/function/Consumer": { samMethod: "accept", params: [{ className: "java/lang/Object" }], returnType: "void" },
  "java/util/function/Predicate": { samMethod: "test", params: [{ className: "java/lang/Object" }], returnType: "boolean" },
  "java/util/function/Function": { samMethod: "apply", params: [{ className: "java/lang/Object" }], returnType: { className: "java/lang/Object" } },
  "java/util/function/BiFunction": {
    samMethod: "apply",
    params: [{ className: "java/lang/Object" }, { className: "java/lang/Object" }],
    returnType: { className: "java/lang/Object" }
  }
};
function lookupKnownMethod(owner, method, argDescs) {
  const exact = knownMethods[`${owner}.${method}(${argDescs})`];
  if (exact) return exact;
  const prefix = `${owner}.${method}(`;
  const wantedArgs = splitDescriptorArgs(argDescs);
  let firstCompatible;
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
function findKnownMethodByArity(owner, method, arity, wantStatic) {
  const prefix = `${owner}.${method}(`;
  for (const key of Object.keys(knownMethods)) {
    if (!key.startsWith(prefix)) continue;
    const sig = knownMethods[key];
    const isStatic = sig.isStatic ?? false;
    if (isStatic !== wantStatic) continue;
    if (sig.paramTypes.length === arity) return sig;
  }
  return void 0;
}
function splitDescriptorArgs(descs) {
  const args = [];
  for (let i = 0; i < descs.length; ) {
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
function resolveClassName(ctx, name) {
  if (name.includes("/")) return name;
  if (name.includes(".")) return name.replace(/\./g, "/");
  const explicit = ctx.importMap.get(name);
  if (explicit) return explicit;
  if (ctx.classDecls.has(name)) return name;
  if (/^[A-Z]/.test(name) && ctx.packageImports.length > 0) {
    for (const pkg of ctx.packageImports) {
      const candidate = `${pkg}/${name}`;
      if (Object.keys(knownMethods).some((k) => k.startsWith(`${candidate}.`))) return candidate;
    }
    return `${ctx.packageImports[0]}/${name}`;
  }
  return name;
}
function findLocal(ctx, name) {
  return ctx.locals.find((l) => l.name === name);
}
function addLocal(ctx, name, type) {
  const slot = ctx.nextSlot++;
  ctx.locals.push({ name, type, slot });
  return slot;
}
function inferType(ctx, expr) {
  switch (expr.kind) {
    case "intLit":
      return "int";
    case "longLit":
      return "long";
    case "floatLit":
      return "float";
    case "doubleLit":
      return "double";
    case "charLit":
      return "char";
    case "stringLit":
      return "String";
    case "boolLit":
      return "boolean";
    case "nullLit":
      return { className: "java/lang/Object" };
    case "this":
      return { className: ctx.className };
    case "ident": {
      const loc = findLocal(ctx, expr.name);
      if (loc) return loc.type;
      const field = ctx.fields.find((f) => f.name === expr.name);
      if (field) return field.type;
      const inherited = ctx.inheritedFields.find((f) => f.name === expr.name);
      if (inherited) return inherited.type;
      return { className: expr.name };
    }
    case "binary": {
      if (["+", "-", "*", "/", "%"].includes(expr.op)) {
        const lt = inferType(ctx, expr.left);
        const rt = inferType(ctx, expr.right);
        if (expr.op === "+" && (lt === "String" || rt === "String")) return "String";
        if (lt === "double" || rt === "double") return "double";
        if (lt === "float" || rt === "float") return "float";
        if (lt === "long" || rt === "long") return "long";
        return "int";
      }
      return "boolean";
    }
    case "unary": {
      if (expr.op === "!") return "boolean";
      const t = inferType(ctx, expr.operand);
      if (t === "double") return "double";
      if (t === "float") return "float";
      if (t === "long") return "long";
      return "int";
    }
    case "newExpr":
      return { className: resolveClassName(ctx, expr.className) };
    case "call": {
      if (expr.object) {
        const objType = inferType(ctx, expr.object);
        const rawOwner = objType === "String" ? "java/lang/String" : typeof objType === "object" && "className" in objType ? objType.className : "java/lang/Object";
        const ownerClass = resolveClassName(ctx, rawOwner);
        const argDescs = expr.args.map((a) => typeToDescriptor(inferType(ctx, a))).join("");
        const sig = lookupKnownMethod(ownerClass, expr.method, argDescs);
        if (sig) return sig.returnType;
        const userMethod = ctx.allMethods.find((m) => m.name === expr.method);
        if (userMethod) return userMethod.returnType;
      } else {
        const userMethod = ctx.allMethods.find((m) => m.name === expr.method);
        if (userMethod) return userMethod.returnType;
      }
      return { className: "java/lang/Object" };
    }
    case "staticCall": {
      const argDescs = expr.args.map((a) => typeToDescriptor(inferType(ctx, a))).join("");
      const internalName = expr.className.replace(/\./g, "/");
      const sig = lookupKnownMethod(internalName, expr.method, argDescs);
      if (sig) return sig.returnType;
      const userMethod = ctx.allMethods.find((m) => m.name === expr.method && m.isStatic);
      if (userMethod) return userMethod.returnType;
      return { className: "java/lang/Object" };
    }
    case "fieldAccess": {
      if (expr.field === "out") return { className: "java/io/PrintStream" };
      if (expr.field === "length") return "int";
      const fld = ctx.fields.find((f) => f.name === expr.field);
      if (fld) return fld.type;
      return { className: "java/lang/Object" };
    }
    case "cast":
      return expr.type;
    case "postIncrement":
      return inferType(ctx, expr.operand);
    case "instanceof":
      return "boolean";
    case "staticField":
      return { className: "java/lang/Object" };
    case "arrayAccess": {
      const arrType = inferType(ctx, expr.array);
      if (typeof arrType === "object" && "array" in arrType) return arrType.array;
      return "int";
    }
    case "arrayLit":
      return { array: expr.elemType };
    case "newArray":
      return { array: expr.elemType };
    case "superCall":
      return "void";
    case "ternary":
      return mergeTernaryType(inferType(ctx, expr.thenExpr), inferType(ctx, expr.elseExpr));
    case "switchExpr": {
      let current;
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
    case "lambda":
      return { className: "java/lang/Object" };
    case "methodRef":
      return { className: "java/lang/Object" };
  }
}
function compileExpr(ctx, emitter, expr, expectedType) {
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
      emitter.emit(1);
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
      const field = ctx.fields.find((f) => f.name === expr.name);
      if (field) {
        if (field.isStatic) {
          const fRef = ctx.cp.addFieldref(ctx.className, expr.name, typeToDescriptor(field.type));
          emitter.emit(178);
          emitter.emitU16(fRef);
        } else {
          emitter.emitAload(0);
          const fRef = ctx.cp.addFieldref(ctx.className, expr.name, typeToDescriptor(field.type));
          emitter.emit(180);
          emitter.emitU16(fRef);
        }
        break;
      }
      const inherited = ctx.inheritedFields.find((f) => f.name === expr.name);
      if (inherited) {
        emitter.emitAload(0);
        const fRef = ctx.cp.addFieldref(ctx.superClass, expr.name, typeToDescriptor(inherited.type));
        emitter.emit(180);
        emitter.emitU16(fRef);
        break;
      }
      break;
    }
    case "binary": {
      let promoteNumeric = function(a, b) {
        if (a === "double" || b === "double") return "double";
        if (a === "float" || b === "float") return "float";
        if (a === "long" || b === "long") return "long";
        return "int";
      };
      const leftType = inferType(ctx, expr.left);
      const rightType = inferType(ctx, expr.right);
      if (expr.op === "+" && (leftType === "String" || rightType === "String")) {
        compileStringConcat(ctx, emitter, expr);
        break;
      }
      if (expr.op === "&&") {
        if (!(leftType === "boolean" && rightType === "boolean")) {
          throw new Error("Operator '&&' requires boolean operands");
        }
        compileExpr(ctx, emitter, expr.left);
        const patchFalse = emitter.emitBranch(153);
        compileExpr(ctx, emitter, expr.right);
        const patchEnd = emitter.emitBranch(167);
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
        const patchEvalRight = emitter.emitBranch(153);
        emitter.emitIconst(1);
        const patchEnd = emitter.emitBranch(167);
        emitter.patchBranch(patchEvalRight, emitter.pc);
        compileExpr(ctx, emitter, expr.right);
        emitter.patchBranch(patchEnd, emitter.pc);
        break;
      }
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
          if (leftType === "boolean" || rightType === "boolean") {
            throw new Error(`Operator '${expr.op}' requires operands of the same primitive type`);
          }
        }
      }
      const promoted = (expr.op === "==" || expr.op === "!=") && (isRefType(leftType) || isRefType(rightType)) ? leftType : promoteNumeric(leftType, rightType);
      compileExpr(ctx, emitter, expr.left);
      if (!isRefType(leftType)) emitWideningConversion(emitter, leftType, promoted);
      compileExpr(ctx, emitter, expr.right);
      if (!isRefType(rightType)) emitWideningConversion(emitter, rightType, promoted);
      if (promoted === "double") {
        switch (expr.op) {
          case "+":
            emitter.emit(99);
            break;
          // dadd
          case "-":
            emitter.emit(103);
            break;
          // dsub
          case "*":
            emitter.emit(107);
            break;
          // dmul
          case "/":
            emitter.emit(111);
            break;
          // ddiv
          case "%":
            emitter.emit(115);
            break;
          // drem
          case "==":
          case "!=":
          case "<":
          case ">":
          case "<=":
          case ">=": {
            emitter.emitPush(151);
            const jumpOp = { "==": 154, "!=": 153, "<": 156, ">": 158, "<=": 157, ">=": 155 }[expr.op];
            const patchFalse = emitter.emitBranch(jumpOp);
            emitter.emitIconst(1);
            const patchEnd = emitter.emitBranch(167);
            emitter.patchBranch(patchFalse, emitter.pc);
            emitter.emitIconst(0);
            emitter.patchBranch(patchEnd, emitter.pc);
            break;
          }
          default:
            throw new Error(`Unsupported binary operator: ${expr.op}`);
        }
      } else if (promoted === "float") {
        switch (expr.op) {
          case "+":
            emitter.emit(98);
            break;
          // fadd
          case "-":
            emitter.emit(102);
            break;
          // fsub
          case "*":
            emitter.emit(106);
            break;
          // fmul
          case "/":
            emitter.emit(110);
            break;
          // fdiv
          case "%":
            emitter.emit(114);
            break;
          // frem
          case "==":
          case "!=":
          case "<":
          case ">":
          case "<=":
          case ">=": {
            emitter.emitPush(149);
            const jumpOp = { "==": 154, "!=": 153, "<": 156, ">": 158, "<=": 157, ">=": 155 }[expr.op];
            const patchFalse = emitter.emitBranch(jumpOp);
            emitter.emitIconst(1);
            const patchEnd = emitter.emitBranch(167);
            emitter.patchBranch(patchFalse, emitter.pc);
            emitter.emitIconst(0);
            emitter.patchBranch(patchEnd, emitter.pc);
            break;
          }
          default:
            throw new Error(`Unsupported binary operator: ${expr.op}`);
        }
      } else if (promoted === "long") {
        switch (expr.op) {
          case "+":
            emitter.emit(97);
            break;
          // ladd
          case "-":
            emitter.emit(101);
            break;
          // lsub
          case "*":
            emitter.emit(105);
            break;
          // lmul
          case "/":
            emitter.emit(109);
            break;
          // ldiv
          case "%":
            emitter.emit(113);
            break;
          // lrem
          case "==":
          case "!=":
          case "<":
          case ">":
          case "<=":
          case ">=": {
            emitter.emitPush(148);
            const jumpOp = { "==": 154, "!=": 153, "<": 156, ">": 158, "<=": 157, ">=": 155 }[expr.op];
            const patchFalse = emitter.emitBranch(jumpOp);
            emitter.emitIconst(1);
            const patchEnd = emitter.emitBranch(167);
            emitter.patchBranch(patchFalse, emitter.pc);
            emitter.emitIconst(0);
            emitter.patchBranch(patchEnd, emitter.pc);
            break;
          }
          default:
            throw new Error(`Unsupported binary operator: ${expr.op}`);
        }
      } else {
        switch (expr.op) {
          case "+":
            emitter.emit(96);
            break;
          // iadd
          case "-":
            emitter.emit(100);
            break;
          // isub
          case "*":
            emitter.emit(104);
            break;
          // imul
          case "/":
            emitter.emit(108);
            break;
          // idiv
          case "%":
            emitter.emit(112);
            break;
          // irem
          case "==":
          case "!=":
          case "<":
          case ">":
          case "<=":
          case ">=": {
            const refCompare = (expr.op === "==" || expr.op === "!=") && (isRefType(leftType) || isRefType(rightType));
            const jumpOp = refCompare ? expr.op === "==" ? 166 : 165 : { "==": 160, "!=": 159, "<": 162, ">": 164, "<=": 163, ">=": 161 }[expr.op];
            const patchFalse = emitter.emitBranch(jumpOp);
            emitter.emitIconst(1);
            const patchEnd = emitter.emitBranch(167);
            emitter.patchBranch(patchFalse, emitter.pc);
            emitter.emitIconst(0);
            emitter.patchBranch(patchEnd, emitter.pc);
            break;
          }
          default:
            throw new Error(`Unsupported binary operator: ${expr.op}`);
        }
      }
      break;
    }
    case "unary": {
      const operandType = inferType(ctx, expr.operand);
      compileExpr(ctx, emitter, expr.operand);
      if (expr.op === "-") {
        if (operandType === "double") emitter.emit(119);
        else if (operandType === "float") emitter.emit(118);
        else if (operandType === "long") emitter.emit(117);
        else if (isPrimitiveType(operandType) && operandType !== "boolean") emitter.emit(116);
        else throw new Error("Unary '-' requires numeric operand");
      }
      if (expr.op === "!") {
        if (operandType !== "boolean") throw new Error("Unary '!' requires boolean operand");
        emitter.emitIconst(1);
        emitter.emit(130);
      }
      break;
    }
    case "newExpr": {
      const internalName = resolveClassName(ctx, expr.className);
      const classIdx = ctx.cp.addClass(internalName);
      emitter.emit(187);
      emitter.emitU16(classIdx);
      emitter.emit(89);
      const argTypes = expr.args.map((a) => typeToDescriptor(inferType(ctx, a)));
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
      const argTypes = expr.args.map((a) => typeToDescriptor(inferType(ctx, a)));
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
      if (expr.operand.kind === "ident") {
        const loc = findLocal(ctx, expr.operand.name);
        if (loc && (loc.type === "int" || loc.type === "boolean")) {
          emitter.emitIload(loc.slot);
          emitter.emit(132);
          emitter.emit(loc.slot);
          emitter.emit(expr.op === "++" ? 1 : 255);
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
      if (isRefType(expr.type)) {
        const castClass = typeof expr.type === "object" && "className" in expr.type ? resolveClassName(ctx, expr.type.className) : "java/lang/Object";
        const classIdx = ctx.cp.addClass(castClass);
        emitter.emit(192);
        emitter.emitU16(classIdx);
      } else if (isPrimitiveType(expr.type) && isPrimitiveType(srcType)) {
        emitWideningConversion(emitter, srcType, expr.type);
        emitNarrowingConversion(emitter, srcType, expr.type);
      }
      break;
    }
    case "instanceof": {
      compileExpr(ctx, emitter, expr.expr);
      const checkClass = resolveClassName(ctx, expr.checkType);
      const classIdx = ctx.cp.addClass(checkClass);
      emitter.emit(193);
      emitter.emitU16(classIdx);
      break;
    }
    case "staticField": {
      const ownerClass = resolveClassName(ctx, expr.className);
      if (ownerClass === "java/lang/System" && expr.field === "out") {
        const fieldRef = ctx.cp.addFieldref("java/lang/System", "out", "Ljava/io/PrintStream;");
        emitter.emit(178);
        emitter.emitU16(fieldRef);
      } else {
        const fieldRef = ctx.cp.addFieldref(ownerClass, expr.field, "Ljava/lang/Object;");
        emitter.emit(178);
        emitter.emitU16(fieldRef);
      }
      break;
    }
    case "newArray": {
      compileExpr(ctx, emitter, expr.size);
      if (expr.elemType === "int" || expr.elemType === "boolean") {
        emitter.emit(188);
        emitter.emit(expr.elemType === "int" ? 10 : 4);
      } else {
        const internalName = typeof expr.elemType === "object" && "className" in expr.elemType ? expr.elemType.className : "java/lang/Object";
        const classIdx = ctx.cp.addClass(internalName);
        emitter.emit(189);
        emitter.emitU16(classIdx);
      }
      break;
    }
    case "arrayLit": {
      emitter.emitIconst(expr.elements.length) || (() => {
        const cpIdx = ctx.cp.addInteger(expr.elements.length);
        emitter.emitLdc(cpIdx);
      })();
      if (expr.elemType === "int" || expr.elemType === "boolean") {
        emitter.emit(188);
        emitter.emit(10);
      } else {
        const internalName = typeof expr.elemType === "object" && "className" in expr.elemType ? expr.elemType.className : "java/lang/Object";
        const classIdx = ctx.cp.addClass(internalName);
        emitter.emit(189);
        emitter.emitU16(classIdx);
      }
      for (let i = 0; i < expr.elements.length; i++) {
        emitter.emit(89);
        emitter.emitIconst(i) || (() => {
          const ci = ctx.cp.addInteger(i);
          emitter.emitLdc(ci);
        })();
        compileExpr(ctx, emitter, expr.elements[i]);
        if (expr.elemType === "int" || expr.elemType === "boolean") {
          emitter.emit(79);
        } else {
          emitter.emit(83);
        }
      }
      break;
    }
    case "arrayAccess": {
      compileExpr(ctx, emitter, expr.array);
      compileExpr(ctx, emitter, expr.index);
      const elemType = inferType(ctx, expr);
      if (elemType === "int" || elemType === "boolean") {
        emitter.emit(46);
      } else {
        emitter.emit(50);
      }
      break;
    }
    case "superCall": {
      emitter.emitAload(0);
      const argTypes = expr.args.map((a) => typeToDescriptor(inferType(ctx, a)));
      for (const arg of expr.args) compileExpr(ctx, emitter, arg);
      const desc = "(" + argTypes.join("") + ")V";
      const mRef = ctx.cp.addMethodref(ctx.superClass, "<init>", desc);
      emitter.emitInvokespecial(mRef, expr.args.length, false);
      break;
    }
    case "ternary": {
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
      const patchElse = emitter.emitBranch(153);
      compileExpr(ctx, emitter, expr.thenExpr);
      const patchEnd = emitter.emitBranch(167);
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
      const used = /* @__PURE__ */ new Set();
      if (expr.bodyExpr) collectExprIdentifiers(expr.bodyExpr, used);
      if (expr.bodyStmts) for (const s of expr.bodyStmts) collectStmtIdentifiers(s, used);
      const paramSet = new Set(expr.params);
      const captures = ctx.locals.filter((l) => used.has(l.name) && !paramSet.has(l.name));
      const needsThisCapture = !ctx.ownerIsStatic;
      const lambdaId = ctx.lambdaCounter.value++;
      const implName = `lambda$${ctx.method.name}$${lambdaId}`;
      const captureParams = captures.map((c) => ({ name: c.name, type: c.type }));
      const lambdaParams = expr.params.map((p, i) => ({ name: p, type: sig.params[i] }));
      const implParams = [...captureParams, ...lambdaParams];
      const implBody = expr.bodyExpr ? [{ kind: "return", value: expr.bodyExpr }] : expr.bodyStmts ?? [];
      const implMethod = {
        name: implName,
        returnType: sig.returnType,
        params: implParams,
        body: implBody,
        isStatic: !needsThisCapture
      };
      ctx.generatedMethods.push(implMethod);
      const implDesc = methodDescriptor(implParams, sig.returnType);
      const capturedTypes = [
        ...needsThisCapture ? [{ className: ctx.className }] : [],
        ...captures.map((c) => c.type)
      ];
      for (const cap of captures) {
        if (cap.type === "void") throw new Error("Unsupported capture type: void");
      }
      for (let i = 0; i < capturedTypes.length; i++) {
        compileExpr(ctx, emitter, needsThisCapture && i === 0 ? { kind: "this" } : { kind: "ident", name: captures[needsThisCapture ? i - 1 : i].name });
      }
      const invokedDesc = "(" + capturedTypes.map(typeToDescriptor).join("") + ")" + typeToDescriptor(expectedType);
      ctx.lambdaBootstraps.push({
        implOwner: ctx.className,
        implMethodName: implName,
        implDescriptor: implDesc,
        invokedName: sig.samMethod,
        invokedDescriptor: invokedDesc,
        implRefKind: needsThisCapture ? 5 : 6
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
      let captureTypes = [];
      const isClassRef = expr.target.kind === "ident" && !findLocal(ctx, expr.target.name) && (/^[A-Z]/.test(expr.target.name) || ctx.importMap.has(expr.target.name) || resolveClassName(ctx, expr.target.name) !== expr.target.name);
      if (expr.isConstructor) {
        if (!(expr.target.kind === "ident" && isClassRef)) {
          throw new Error("Constructor method reference target must be a class name");
        }
        const targetClass = resolveClassName(ctx, expr.target.name);
        const ctorId = ctx.lambdaCounter.value++;
        const ctorImplName = `lambda$ctor$${ctorId}`;
        const ctorParams = sig.params.map((p, i) => ({ name: `p${i}`, type: p }));
        const argDescs = ctorParams.map((p) => typeToDescriptor(p.type)).join("");
        const ctorKnown = lookupKnownMethod(targetClass, "<init>", argDescs) ?? findKnownMethodByArity(targetClass, "<init>", ctorParams.length, false);
        const ctorTypes = ctorKnown?.paramTypes ?? ctorParams.map((p) => p.type);
        const ctorArgs = ctorParams.map((p, i) => {
          const need = ctorTypes[i];
          if (need && !sameType(need, p.type)) {
            return { kind: "cast", type: need, expr: { kind: "ident", name: p.name } };
          }
          return { kind: "ident", name: p.name };
        });
        const ctorMethod = {
          name: ctorImplName,
          returnType: sig.returnType,
          params: ctorParams,
          body: [{ kind: "return", value: { kind: "newExpr", className: targetClass, args: ctorArgs } }],
          isStatic: true
        };
        ctx.generatedMethods.push(ctorMethod);
        const implDescCtor = methodDescriptor(ctorMethod.params, ctorMethod.returnType);
        const invokedDescriptorCtor = "()" + typeToDescriptor(expectedType);
        ctx.lambdaBootstraps.push({
          implOwner: ctx.className,
          implMethodName: ctorImplName,
          implDescriptor: implDescCtor,
          invokedName: sig.samMethod,
          invokedDescriptor: invokedDescriptorCtor,
          implRefKind: 6
        });
        const bootstrapIdx2 = ctx.lambdaBootstraps.length - 1;
        const indyIdx2 = ctx.cp.addInvokeDynamic(bootstrapIdx2, sig.samMethod, invokedDescriptorCtor);
        emitter.emitInvokedynamic(indyIdx2, 0, true);
        break;
      }
      if (isClassRef && expr.target.kind === "ident") {
        implOwner = resolveClassName(ctx, expr.target.name);
        const staticSig = findKnownMethodByArity(implOwner, expr.method, sig.params.length, true);
        if (staticSig) {
          implDescriptor = "(" + staticSig.paramTypes.map(typeToDescriptor).join("") + ")" + typeToDescriptor(staticSig.returnType);
          implRefKind = 6;
          implIsInterface = staticSig.isInterface ?? false;
        } else {
          const instSig = findKnownMethodByArity(implOwner, expr.method, Math.max(0, sig.params.length - 1), false);
          if (instSig) {
            implDescriptor = "(" + instSig.paramTypes.map(typeToDescriptor).join("") + ")" + typeToDescriptor(instSig.returnType);
            implRefKind = instSig.isInterface ? 9 : 5;
            implIsInterface = instSig.isInterface ?? false;
          } else if (implOwner === ctx.className) {
            const staticUser = ctx.allMethods.find((m) => m.name === expr.method && m.isStatic && m.params.length === sig.params.length);
            if (staticUser) {
              implDescriptor = methodDescriptor(staticUser.params, staticUser.returnType);
              implRefKind = 6;
            } else {
              const instUser = ctx.allMethods.find((m) => m.name === expr.method && !m.isStatic && m.params.length === Math.max(0, sig.params.length - 1));
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
        implOwner = t === "String" ? "java/lang/String" : typeof t === "object" && "className" in t ? resolveClassName(ctx, t.className) : "java/lang/Object";
        captureTypes = [t === "String" ? "String" : t];
        compileExpr(ctx, emitter, expr.target);
        const boundSig = findKnownMethodByArity(implOwner, expr.method, sig.params.length, false);
        if (boundSig) {
          implDescriptor = "(" + boundSig.paramTypes.map(typeToDescriptor).join("") + ")" + typeToDescriptor(boundSig.returnType);
          implRefKind = boundSig.isInterface ? 9 : 5;
          implIsInterface = boundSig.isInterface ?? false;
        } else if (implOwner === ctx.className) {
          const m = ctx.allMethods.find((mm) => mm.name === expr.method && !mm.isStatic && mm.params.length === sig.params.length);
          if (!m) throw new Error(`Cannot resolve method reference target::${expr.method}`);
          implDescriptor = methodDescriptor(m.params, m.returnType);
          implRefKind = 5;
        } else {
          throw new Error(`Cannot resolve method reference target::${expr.method}`);
        }
      }
      const invokedDescriptor = "(" + captureTypes.map(typeToDescriptor).join("") + ")" + typeToDescriptor(expectedType);
      ctx.lambdaBootstraps.push({
        implOwner,
        implMethodName: implName,
        implDescriptor,
        implIsInterface,
        invokedName: sig.samMethod,
        invokedDescriptor,
        implRefKind
      });
      const bootstrapIdx = ctx.lambdaBootstraps.length - 1;
      const indyIdx = ctx.cp.addInvokeDynamic(bootstrapIdx, sig.samMethod, invokedDescriptor);
      emitter.emitInvokedynamic(indyIdx, captureTypes.length, true);
      break;
    }
    default:
      throw new Error(`Unsupported expression: ${expr.kind}`);
  }
}
function compileStringConcat(ctx, emitter, expr) {
  const parts = [];
  function flatten(e) {
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
  const sbClass = ctx.cp.addClass("java/lang/StringBuilder");
  emitter.emit(187);
  emitter.emitU16(sbClass);
  emitter.emit(89);
  const initRef = ctx.cp.addMethodref("java/lang/StringBuilder", "<init>", "()V");
  emitter.emitInvokespecial(initRef, 0, false);
  for (const part of parts) {
    const partType = inferType(ctx, part);
    compileExpr(ctx, emitter, part);
    let appendDesc;
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
  const toStringRef = ctx.cp.addMethodref("java/lang/StringBuilder", "toString", "()Ljava/lang/String;");
  emitter.emitInvokevirtual(toStringRef, 0, true);
}
function compileCall(ctx, emitter, expr) {
  if (expr.object?.kind === "fieldAccess" && expr.object.object.kind === "ident" && expr.object.object.name === "System" && expr.object.field === "out") {
    const fieldRef = ctx.cp.addFieldref("java/lang/System", "out", "Ljava/io/PrintStream;");
    emitter.emit(178);
    emitter.emitU16(fieldRef);
    const argType = expr.args.length > 0 ? inferType(ctx, expr.args[0]) : "void";
    for (const arg of expr.args) compileExpr(ctx, emitter, arg);
    let desc;
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
    if (expr.object.kind === "ident") {
      const name = expr.object.name;
      if ((/^[A-Z]/.test(name) || ctx.importMap.has(name)) && !findLocal(ctx, name)) {
        const internalName = resolveClassName(ctx, name);
        const argTypes2 = expr.args.map((a) => typeToDescriptor(inferType(ctx, a)));
        const sig2 = lookupKnownMethod(internalName, expr.method, argTypes2.join(""));
        if (sig2) {
          expr.args.forEach((arg, i) => compileExpr(ctx, emitter, arg, sig2.paramTypes[i]));
          const sigArgDescs = sig2.paramTypes.map((t) => typeToDescriptor(t)).join("");
          const desc2 = "(" + sigArgDescs + ")" + typeToDescriptor(sig2.returnType);
          const mRef = ctx.cp.addMethodref(internalName, expr.method, desc2);
          emitter.emitInvokestatic(mRef, expr.args.length, sig2.returnType !== "void");
        } else {
          for (const arg of expr.args) compileExpr(ctx, emitter, arg);
          const userMethod = ctx.allMethods.find((m) => m.name === expr.method && m.isStatic);
          const retType2 = userMethod ? userMethod.returnType : { className: "java/lang/Object" };
          const desc2 = "(" + argTypes2.join("") + ")" + typeToDescriptor(retType2);
          const mRef = ctx.cp.addMethodref(internalName, expr.method, desc2);
          emitter.emitInvokestatic(mRef, expr.args.length, retType2 !== "void");
        }
        return;
      }
    }
    compileExpr(ctx, emitter, expr.object);
    const argTypes = expr.args.map((a) => typeToDescriptor(inferType(ctx, a)));
    const rawOwner = objType === "String" ? "java/lang/String" : typeof objType === "object" && "className" in objType ? objType.className : "java/lang/Object";
    const ownerClass = resolveClassName(ctx, rawOwner);
    const sig = lookupKnownMethod(ownerClass, expr.method, argTypes.join(""));
    let desc;
    let retType;
    let isInterface = false;
    if (sig) {
      expr.args.forEach((arg, i) => compileExpr(ctx, emitter, arg, sig.paramTypes[i]));
      retType = sig.returnType;
      const sigArgDescs = sig.paramTypes.map((t) => typeToDescriptor(t)).join("");
      desc = "(" + sigArgDescs + ")" + typeToDescriptor(retType);
      isInterface = sig.isInterface ?? false;
    } else {
      const userMethod = ctx.allMethods.find((m) => m.name === expr.method && !m.isStatic);
      if (userMethod) {
        expr.args.forEach((arg, i) => compileExpr(ctx, emitter, arg, userMethod.params[i]?.type));
      } else {
        for (const arg of expr.args) compileExpr(ctx, emitter, arg);
      }
      retType = userMethod ? userMethod.returnType : { className: "java/lang/Object" };
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
    const userMethod = ctx.allMethods.find((m) => m.name === expr.method);
    if (userMethod) {
      const desc = methodDescriptor(userMethod.params, userMethod.returnType);
      const mRef = ctx.cp.addMethodref(ctx.className, expr.method, desc);
      if (userMethod.isStatic) {
        expr.args.forEach((arg, i) => compileExpr(ctx, emitter, arg, userMethod.params[i]?.type));
        emitter.emitInvokestatic(mRef, expr.args.length, userMethod.returnType !== "void");
      } else {
        emitter.emitAload(0);
        expr.args.forEach((arg, i) => compileExpr(ctx, emitter, arg, userMethod.params[i]?.type));
        emitter.emitInvokevirtual(mRef, expr.args.length, userMethod.returnType !== "void");
      }
    } else if (ctx.staticWildcardImports.length > 0) {
      const argTypes = expr.args.map((a) => typeToDescriptor(inferType(ctx, a)));
      for (const arg of expr.args) compileExpr(ctx, emitter, arg);
      const ownerClass = ctx.staticWildcardImports.find((owner) => !!lookupKnownMethod(owner, expr.method, argTypes.join(""))) ?? ctx.staticWildcardImports[0];
      const retType = { className: "java/lang/Object" };
      const desc = "(" + argTypes.join("") + ")" + typeToDescriptor(retType);
      const mRef = ctx.cp.addMethodref(ownerClass, expr.method, desc);
      emitter.emitInvokestatic(mRef, expr.args.length, true);
    }
  }
}
function compileFieldAccess(ctx, emitter, expr) {
  if (expr.object.kind === "ident") {
    const name = expr.object.name;
    const resolved = resolveClassName(ctx, name);
    const isLocal = !!findLocal(ctx, name);
    const isClassRef = !isLocal && (/^[A-Z]/.test(name) || ctx.importMap.has(name) || resolved !== name);
    if (isClassRef) {
      let desc = "Ljava/lang/Object;";
      if (resolved === "java/lang/System" && expr.field === "out") desc = "Ljava/io/PrintStream;";
      const fieldRef2 = ctx.cp.addFieldref(resolved, expr.field, desc);
      emitter.emit(178);
      emitter.emitU16(fieldRef2);
      return;
    }
  }
  if (expr.object.kind === "fieldAccess") {
    let collapseChain = function(e) {
      if (e.kind === "fieldAccess") {
        const inner = collapseChain(e.object);
        if (inner) return { className: inner.className + "/" + inner.field, field: e.field };
      }
      if (e.kind === "ident") return { className: e.name, field: "" };
      return null;
    };
    const chain = collapseChain(expr.object);
    if (chain) {
      const ownerClass2 = (chain.field ? chain.className + "/" + chain.field : chain.className).replace(/\./g, "/");
      let desc = "Ljava/lang/Object;";
      if (ownerClass2 === "java/lang/System" && expr.field === "out") desc = "Ljava/io/PrintStream;";
      const fieldRef2 = ctx.cp.addFieldref(ownerClass2, expr.field, desc);
      emitter.emit(178);
      emitter.emitU16(fieldRef2);
      return;
    }
  }
  if (expr.field === "length") {
    const objType2 = inferType(ctx, expr.object);
    if (typeof objType2 === "object" && "array" in objType2) {
      compileExpr(ctx, emitter, expr.object);
      emitter.emit(190);
      return;
    }
  }
  compileExpr(ctx, emitter, expr.object);
  const objType = inferType(ctx, expr.object);
  const ownerClass = typeof objType === "object" && "className" in objType ? objType.className : ctx.className;
  const fld = ctx.fields.find((f) => f.name === expr.field);
  const fieldType = fld ? typeToDescriptor(fld.type) : "Ljava/lang/Object;";
  const fieldRef = ctx.cp.addFieldref(ownerClass, expr.field, fieldType);
  emitter.emit(180);
  emitter.emitU16(fieldRef);
}
function withScopedLocals(ctx, fn) {
  const savedLen = ctx.locals.length;
  const savedNext = ctx.nextSlot;
  fn();
  ctx.locals.length = savedLen;
  ctx.nextSlot = savedNext;
}
function ensureAssignable(ctx, target, value, reason) {
  if (!isAssignableInContext(ctx, target, value)) {
    throw new Error(`Type mismatch for ${reason}: cannot assign ${typeToDescriptor(value)} to ${typeToDescriptor(target)}`);
  }
}
function collectExprIdentifiers(expr, out) {
  switch (expr.kind) {
    case "ident":
      out.add(expr.name);
      break;
    case "binary":
      collectExprIdentifiers(expr.left, out);
      collectExprIdentifiers(expr.right, out);
      break;
    case "unary":
      collectExprIdentifiers(expr.operand, out);
      break;
    case "call":
      if (expr.object) collectExprIdentifiers(expr.object, out);
      for (const a of expr.args) collectExprIdentifiers(a, out);
      break;
    case "staticCall":
      for (const a of expr.args) collectExprIdentifiers(a, out);
      break;
    case "fieldAccess":
      collectExprIdentifiers(expr.object, out);
      break;
    case "newExpr":
      for (const a of expr.args) collectExprIdentifiers(a, out);
      break;
    case "cast":
      collectExprIdentifiers(expr.expr, out);
      break;
    case "postIncrement":
      collectExprIdentifiers(expr.operand, out);
      break;
    case "instanceof":
      collectExprIdentifiers(expr.expr, out);
      break;
    case "arrayAccess":
      collectExprIdentifiers(expr.array, out);
      collectExprIdentifiers(expr.index, out);
      break;
    case "arrayLit":
      for (const e of expr.elements) collectExprIdentifiers(e, out);
      break;
    case "newArray":
      collectExprIdentifiers(expr.size, out);
      break;
    case "superCall":
      for (const a of expr.args) collectExprIdentifiers(a, out);
      break;
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
      break;
    case "methodRef":
      collectExprIdentifiers(expr.target, out);
      break;
    default:
      break;
  }
}
function collectStmtIdentifiers(stmt, out) {
  switch (stmt.kind) {
    case "varDecl":
      if (stmt.init) collectExprIdentifiers(stmt.init, out);
      break;
    case "assign":
      collectExprIdentifiers(stmt.target, out);
      collectExprIdentifiers(stmt.value, out);
      break;
    case "exprStmt":
      collectExprIdentifiers(stmt.expr, out);
      break;
    case "return":
      if (stmt.value) collectExprIdentifiers(stmt.value, out);
      break;
    case "yield":
      collectExprIdentifiers(stmt.value, out);
      break;
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
    case "block":
      for (const s of stmt.stmts) collectStmtIdentifiers(s, out);
      break;
  }
}
function functionalSigForType(ctx, t) {
  if (!(typeof t === "object" && "className" in t)) {
    throw new Error("Lambda target type must be a functional interface");
  }
  const ifaceName = resolveClassName(ctx, t.className);
  const sig = FUNCTIONAL_IFACES[ifaceName];
  if (!sig) throw new Error(`Unsupported functional interface for lambda: ${ifaceName}`);
  return { ifaceName, sig };
}
var BUILTIN_SUPERS = {
  "java/lang/String": "java/lang/Object",
  "java/lang/Integer": "java/lang/Object",
  "java/lang/StringBuilder": "java/lang/Object",
  "java/util/ArrayList": "java/lang/Object",
  "java/io/PrintStream": "java/lang/Object"
};
function toInternalClassName(ctx, t) {
  if (t === "String") return "java/lang/String";
  if (typeof t === "object" && "className" in t) return resolveClassName(ctx, t.className);
  return void 0;
}
function isClassSupertype(ctx, maybeSuper, maybeSub) {
  if (maybeSuper === maybeSub) return true;
  let cur = maybeSub;
  const seen = /* @__PURE__ */ new Set();
  while (!seen.has(cur)) {
    seen.add(cur);
    const next = ctx.classSupers.get(cur) ?? BUILTIN_SUPERS[cur];
    if (!next) return false;
    if (next === maybeSuper) return true;
    cur = next;
  }
  return false;
}
function isPatternTotalForSelector(ctx, selectorType, patternTypeName) {
  const selectorClass = toInternalClassName(ctx, selectorType);
  if (!selectorClass) return false;
  const patternClass = resolveClassName(ctx, patternTypeName);
  return isClassSupertype(ctx, patternClass, selectorClass);
}
function validateSwitchSemanticsCompile(ctx, selectorType, cases, isExpr) {
  let seenTotalNonNullPattern = false;
  let seenNullCase = false;
  const unguardedPatterns = [];
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
    const hasUnguardedDefault = cases.some((c) => !c.guard && c.labels.some((l) => l.kind === "default"));
    if (hasUnguardedDefault) return;
    const hasTrue = cases.some((c) => !c.guard && c.labels.some((l) => l.kind === "bool" && l.value));
    const hasFalse = cases.some((c) => !c.guard && c.labels.some((l) => l.kind === "bool" && !l.value));
    const exhaustiveBoolean = selectorType === "boolean" && hasTrue && hasFalse;
    const exhaustiveRef = isRefType(selectorType) && seenNullCase && seenTotalNonNullPattern;
    if (!exhaustiveBoolean && !exhaustiveRef) {
      throw new Error("switch expression is not exhaustive: provide default or exhaustive labels");
    }
  }
}
function resolveClassDecl(ctx, typeName) {
  const internal = resolveClassName(ctx, typeName);
  return ctx.classDecls.get(internal) ?? ctx.classDecls.get(typeName);
}
function isIntLike(t) {
  return t === "int" || t === "short" || t === "byte" || t === "char" || t === "boolean";
}
function emitWideningConversion(emitter, from, to) {
  if (sameType(from, to)) return;
  if (isIntLike(from) && isIntLike(to)) return;
  if (isIntLike(from) && to === "long") {
    emitter.emit(133);
    return;
  }
  if (isIntLike(from) && to === "float") {
    emitter.emit(134);
    return;
  }
  if (isIntLike(from) && to === "double") {
    emitter.emit(135);
    return;
  }
  if (from === "long" && to === "float") {
    emitter.emit(137);
    return;
  }
  if (from === "long" && to === "double") {
    emitter.emit(138);
    return;
  }
  if (from === "float" && to === "double") {
    emitter.emit(141);
    return;
  }
}
function emitNarrowingConversion(emitter, from, to) {
  if (sameType(from, to)) return;
  if (isIntLike(from) && to === "byte") {
    emitter.emit(145);
    return;
  }
  if (isIntLike(from) && to === "char") {
    emitter.emit(146);
    return;
  }
  if (isIntLike(from) && to === "short") {
    emitter.emit(147);
    return;
  }
  if (from === "long" && isIntLike(to)) {
    emitter.emit(136);
    return;
  }
  if (from === "long" && to === "float") {
    emitter.emit(137);
    return;
  }
  if (from === "long" && to === "double") {
    emitter.emit(138);
    return;
  }
  if (from === "float" && isIntLike(to)) {
    emitter.emit(139);
    return;
  }
  if (from === "float" && to === "long") {
    emitter.emit(140);
    return;
  }
  if (from === "double" && isIntLike(to)) {
    emitter.emit(142);
    return;
  }
  if (from === "double" && to === "long") {
    emitter.emit(143);
    return;
  }
  if (from === "double" && to === "float") {
    emitter.emit(144);
    return;
  }
}
function emitStoreLocalByType(emitter, slot, t) {
  if (t === "long") emitter.emitLstore(slot);
  else if (t === "float") emitter.emitFstore(slot);
  else if (t === "double") emitter.emitDstore(slot);
  else if (t === "int" || t === "boolean" || t === "short" || t === "byte" || t === "char") emitter.emitIstore(slot);
  else emitter.emitAstore(slot);
}
function emitLoadLocalByType(emitter, slot, t) {
  if (t === "long") emitter.emitLload(slot);
  else if (t === "float") emitter.emitFload(slot);
  else if (t === "double") emitter.emitDload(slot);
  else if (t === "int" || t === "boolean" || t === "short" || t === "byte" || t === "char") emitter.emitIload(slot);
  else emitter.emitAload(slot);
}
function bindPatternLabelLocals(ctx, emitter, selectorSlot, selectorType, label) {
  if (label.kind !== "typePattern" && label.kind !== "recordPattern") {
    throw new Error("internal: expected pattern label");
  }
  emitLoadLocalByType(emitter, selectorSlot, selectorType);
  const checkClass = resolveClassName(ctx, label.typeName);
  const classIdx = ctx.cp.addClass(checkClass);
  emitter.emit(192);
  emitter.emitU16(classIdx);
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
function emitSwitchLabelMatch(ctx, emitter, selectorSlot, selectorType, label) {
  if (label.kind === "default") return emitter.emitBranch(167);
  if (label.kind === "bool") {
    if (selectorType !== "boolean") {
      throw new Error("boolean case label requires boolean switch selector");
    }
    emitter.emitIload(selectorSlot);
    emitter.emitIconst(label.value ? 1 : 0);
    return emitter.emitBranch(159);
  }
  if (label.kind === "int") {
    if (selectorType !== "int") {
      throw new Error("int case label requires int switch selector");
    }
    emitter.emitIload(selectorSlot);
    if (!emitter.emitIconst(label.value)) {
      emitter.emitLdc(ctx.cp.addInteger(label.value));
    }
    return emitter.emitBranch(159);
  }
  if (label.kind === "null") {
    if (!isRefType(selectorType)) throw new Error("null case label requires reference switch selector");
    emitter.emitAload(selectorSlot);
    return emitter.emitBranch(198);
  }
  if (label.kind === "string") {
    if (selectorType !== "String" && !(typeof selectorType === "object" && "className" in selectorType)) {
      throw new Error("String case label requires reference switch selector");
    }
    emitter.emitAload(selectorSlot);
    const patchNull = emitter.emitBranch(198);
    emitter.emitAload(selectorSlot);
    emitter.emitLdc(ctx.cp.addString(label.value));
    const equalsRef = ctx.cp.addMethodref("java/lang/String", "equals", "(Ljava/lang/Object;)Z");
    emitter.emitInvokevirtual(equalsRef, 1, true);
    const patchMatch = emitter.emitBranch(154);
    emitter.patchBranch(patchNull, emitter.pc);
    return patchMatch;
  }
  if (!isRefType(selectorType)) throw new Error("type pattern case requires reference switch selector");
  emitter.emitAload(selectorSlot);
  const checkClass = resolveClassName(ctx, label.typeName);
  const classIdx = ctx.cp.addClass(checkClass);
  emitter.emit(193);
  emitter.emitU16(classIdx);
  return emitter.emitBranch(154);
}
function compileSwitchCaseStmts(ctx, emitter, c) {
  if (c.expr) {
    compileExpr(ctx, emitter, c.expr);
    emitter.emit(87);
    return;
  }
  for (const s of c.stmts ?? []) compileStmt(ctx, emitter, s);
}
function compileSwitchStmt(ctx, emitter, stmt) {
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
    const endPatches = [];
    for (const c of stmt.cases) {
      const matches = c.labels.map((l) => ({ label: l, patch: emitSwitchLabelMatch(ctx, emitter, selectorSlot, selectorType, l) }));
      const patchNext = emitter.emitBranch(167);
      const bodyStart = emitter.pc;
      for (const m of matches) emitter.patchBranch(m.patch, bodyStart);
      withScopedLocals(ctx, () => {
        const patternLabel = c.labels.find((l) => l.kind === "typePattern" || l.kind === "recordPattern");
        if (patternLabel) {
          bindPatternLabelLocals(ctx, emitter, selectorSlot, selectorType, patternLabel);
        }
        if (c.guard) {
          if (inferType(ctx, c.guard) !== "boolean") {
            throw new Error("switch guard must be boolean");
          }
          compileExpr(ctx, emitter, c.guard, "boolean");
          const guardFail = emitter.emitBranch(153);
          compileSwitchCaseStmts(ctx, emitter, c);
          endPatches.push(emitter.emitBranch(167));
          emitter.patchBranch(guardFail, emitter.pc);
        } else {
          compileSwitchCaseStmts(ctx, emitter, c);
          endPatches.push(emitter.emitBranch(167));
        }
      });
      emitter.patchBranch(patchNext, emitter.pc);
    }
    for (const p of endPatches) emitter.patchBranch(p, emitter.pc);
  });
}
function compileSwitchExpr(ctx, emitter, expr, expectedType) {
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
    const endPatches = [];
    for (const c of expr.cases) {
      const matches = c.labels.map((l) => ({ label: l, patch: emitSwitchLabelMatch(ctx, emitter, selectorSlot, selectorType, l) }));
      const patchNext = emitter.emitBranch(167);
      const bodyStart = emitter.pc;
      for (const m of matches) emitter.patchBranch(m.patch, bodyStart);
      withScopedLocals(ctx, () => {
        const patternLabel = c.labels.find((l) => l.kind === "typePattern" || l.kind === "recordPattern");
        if (patternLabel) {
          bindPatternLabelLocals(ctx, emitter, selectorSlot, selectorType, patternLabel);
        }
        if (c.guard) {
          if (inferType(ctx, c.guard) !== "boolean") {
            throw new Error("switch guard must be boolean");
          }
          compileExpr(ctx, emitter, c.guard, "boolean");
          const guardFail = emitter.emitBranch(153);
          if (c.expr) {
            compileExpr(ctx, emitter, c.expr, resultType);
            endPatches.push(emitter.emitBranch(167));
          } else {
            let yielded = false;
            for (const s of c.stmts ?? []) {
              if (s.kind === "yield") {
                compileExpr(ctx, emitter, s.value, resultType);
                endPatches.push(emitter.emitBranch(167));
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
          endPatches.push(emitter.emitBranch(167));
        } else {
          let yielded = false;
          for (const s of c.stmts ?? []) {
            if (s.kind === "yield") {
              compileExpr(ctx, emitter, s.value, resultType);
              endPatches.push(emitter.emitBranch(167));
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
function compileStmt(ctx, emitter, stmt) {
  switch (stmt.kind) {
    case "varDecl": {
      const slot = addLocal(ctx, stmt.name, stmt.type);
      if (emitter.maxLocals <= slot) emitter.maxLocals = slot + 1;
      if (stmt.init) {
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
          const field = ctx.fields.find((f) => f.name === stmt.target.name);
          if (field) {
            ensureAssignable(ctx, field.type, inferType(ctx, stmt.value), `field '${stmt.target.name}'`);
            if (field.isStatic) {
              compileExpr(ctx, emitter, stmt.value, field.type);
              const fRef = ctx.cp.addFieldref(ctx.className, field.name, typeToDescriptor(field.type));
              emitter.emit(179);
              emitter.emitU16(fRef);
            } else {
              emitter.emitAload(0);
              compileExpr(ctx, emitter, stmt.value, field.type);
              const fRef = ctx.cp.addFieldref(ctx.className, field.name, typeToDescriptor(field.type));
              emitter.emit(181);
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
        const fld = ctx.fields.find((f) => f.name === stmt.target.field);
        const fieldType = fld ? typeToDescriptor(fld.type) : typeToDescriptor(inferType(ctx, stmt.value));
        const fieldRef = ctx.cp.addFieldref(ownerClass, stmt.target.field, fieldType);
        emitter.emit(181);
        emitter.emitU16(fieldRef);
      } else if (stmt.target.kind === "arrayAccess") {
        compileExpr(ctx, emitter, stmt.target.array);
        compileExpr(ctx, emitter, stmt.target.index);
        const elemType = inferType(ctx, stmt.target);
        compileExpr(ctx, emitter, stmt.value, elemType);
        if (elemType === "int" || elemType === "boolean") {
          emitter.emit(79);
        } else {
          emitter.emit(83);
        }
      }
      break;
    }
    case "exprStmt": {
      compileExpr(ctx, emitter, stmt.expr);
      const exprType = inferType(ctx, stmt.expr);
      if (exprType !== "void") {
        emitter.emit(87);
      }
      break;
    }
    case "return": {
      if (stmt.value) {
        const retValType = inferType(ctx, stmt.value);
        ensureAssignable(ctx, ctx.method.returnType, retValType, `return in ${ctx.method.name}`);
        compileExpr(ctx, emitter, stmt.value, ctx.method.returnType);
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
      const patchElse = emitter.emitBranch(153);
      withScopedLocals(ctx, () => {
        if (stmt.cond.kind === "instanceof" && (stmt.cond.bindVar || stmt.cond.recordBindVars)) {
          const checkClass = resolveClassName(ctx, stmt.cond.checkType);
          compileExpr(ctx, emitter, stmt.cond.expr);
          const classIdx = ctx.cp.addClass(checkClass);
          emitter.emit(192);
          emitter.emitU16(classIdx);
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
        const patchEnd = emitter.emitBranch(167);
        emitter.patchBranch(patchElse, emitter.pc);
        withScopedLocals(ctx, () => {
          for (const s of stmt.else_) compileStmt(ctx, emitter, s);
        });
        emitter.patchBranch(patchEnd, emitter.pc);
      } else {
        emitter.patchBranch(patchElse, emitter.pc);
      }
      break;
    }
    case "while": {
      if (inferType(ctx, stmt.cond) !== "boolean") throw new Error("while condition must be boolean");
      const loopStart = emitter.pc;
      compileExpr(ctx, emitter, stmt.cond);
      const patchExit = emitter.emitBranch(153);
      withScopedLocals(ctx, () => {
        for (const s of stmt.body) compileStmt(ctx, emitter, s);
      });
      const gotoOp = emitter.emitBranch(167);
      emitter.patchBranch(gotoOp, loopStart);
      emitter.patchBranch(patchExit, emitter.pc);
      break;
    }
    case "for": {
      withScopedLocals(ctx, () => {
        if (stmt.init) compileStmt(ctx, emitter, stmt.init);
        const loopStart = emitter.pc;
        let patchExit = -1;
        if (stmt.cond) {
          if (inferType(ctx, stmt.cond) !== "boolean") throw new Error("for condition must be boolean");
          compileExpr(ctx, emitter, stmt.cond);
          patchExit = emitter.emitBranch(153);
        }
        withScopedLocals(ctx, () => {
          for (const s of stmt.body) compileStmt(ctx, emitter, s);
        });
        if (stmt.update) compileStmt(ctx, emitter, stmt.update);
        const gotoOp = emitter.emitBranch(167);
        emitter.patchBranch(gotoOp, loopStart);
        if (patchExit >= 0) emitter.patchBranch(patchExit, emitter.pc);
      });
      break;
    }
    case "switch": {
      compileSwitchStmt(ctx, emitter, stmt);
      break;
    }
    case "block": {
      withScopedLocals(ctx, () => {
        for (const s of stmt.stmts) compileStmt(ctx, emitter, s);
      });
      break;
    }
    default:
      throw new Error(`Unsupported statement: ${stmt.kind}`);
  }
}
function compileMethod(classDecl, method, cp, allMethods, inheritedFields, classSupers, classDecls, lambdaCounter, generatedMethods, lambdaBootstraps) {
  const emitter = new BytecodeEmitter();
  const locals = [];
  let nextSlot = 0;
  if (!method.isStatic) {
    locals.push({ name: "this", type: { className: classDecl.name }, slot: 0 });
    nextSlot = 1;
  }
  for (const p of method.params) {
    locals.push({ name: p.name, type: p.type, slot: nextSlot });
    nextSlot++;
  }
  const ctx = {
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
    ownerIsStatic: method.isStatic
  };
  emitter.maxLocals = nextSlot;
  for (const stmt of method.body) {
    compileStmt(ctx, emitter, stmt);
  }
  const lastByte = emitter.code.length > 0 ? emitter.code[emitter.code.length - 1] : -1;
  const isReturn = lastByte === 177 || lastByte === 172 || lastByte === 176 || lastByte === 173 || lastByte === 174 || lastByte === 175;
  if (!isReturn) {
    emitter.emitReturn(method.returnType);
  }
  return { code: emitter.code, maxStack: Math.max(emitter.maxStack, 4), maxLocals: emitter.maxLocals };
}
function exprHasSuperCall(expr) {
  switch (expr.kind) {
    case "superCall":
      return true;
    case "binary":
      return exprHasSuperCall(expr.left) || exprHasSuperCall(expr.right);
    case "unary":
      return exprHasSuperCall(expr.operand);
    case "call":
      return (expr.object ? exprHasSuperCall(expr.object) : false) || expr.args.some(exprHasSuperCall);
    case "staticCall":
      return expr.args.some(exprHasSuperCall);
    case "fieldAccess":
      return exprHasSuperCall(expr.object);
    case "newExpr":
      return expr.args.some(exprHasSuperCall);
    case "cast":
      return exprHasSuperCall(expr.expr);
    case "postIncrement":
      return exprHasSuperCall(expr.operand);
    case "instanceof":
      return exprHasSuperCall(expr.expr);
    case "arrayAccess":
      return exprHasSuperCall(expr.array) || exprHasSuperCall(expr.index);
    case "arrayLit":
      return expr.elements.some(exprHasSuperCall);
    case "newArray":
      return exprHasSuperCall(expr.size);
    case "ternary":
      return exprHasSuperCall(expr.cond) || exprHasSuperCall(expr.thenExpr) || exprHasSuperCall(expr.elseExpr);
    case "switchExpr":
      return exprHasSuperCall(expr.selector) || expr.cases.some((c) => c.expr && exprHasSuperCall(c.expr) || c.stmts && c.stmts.some(stmtHasSuperCall));
    case "lambda":
      return !!expr.bodyExpr && exprHasSuperCall(expr.bodyExpr) || !!expr.bodyStmts && expr.bodyStmts.some(stmtHasSuperCall);
    case "methodRef":
      return exprHasSuperCall(expr.target);
    default:
      return false;
  }
}
function stmtHasSuperCall(stmt) {
  switch (stmt.kind) {
    case "varDecl":
      return !!stmt.init && exprHasSuperCall(stmt.init);
    case "assign":
      return exprHasSuperCall(stmt.target) || exprHasSuperCall(stmt.value);
    case "exprStmt":
      return exprHasSuperCall(stmt.expr);
    case "return":
      return !!stmt.value && exprHasSuperCall(stmt.value);
    case "yield":
      return exprHasSuperCall(stmt.value);
    case "if":
      return exprHasSuperCall(stmt.cond) || stmt.then.some(stmtHasSuperCall) || !!stmt.else_?.some(stmtHasSuperCall);
    case "while":
      return exprHasSuperCall(stmt.cond) || stmt.body.some(stmtHasSuperCall);
    case "for":
      return !!stmt.init && stmtHasSuperCall(stmt.init) || !!stmt.cond && exprHasSuperCall(stmt.cond) || !!stmt.update && stmtHasSuperCall(stmt.update) || stmt.body.some(stmtHasSuperCall);
    case "switch":
      return exprHasSuperCall(stmt.selector) || stmt.cases.some((c) => c.expr && exprHasSuperCall(c.expr) || c.stmts && c.stmts.some(stmtHasSuperCall));
    case "block":
      return stmt.stmts.some(stmtHasSuperCall);
  }
}
function validateConstructorBody(method) {
  if (method.name !== "<init>") {
    if (method.body.some(stmtHasSuperCall)) {
      throw new Error("super(...) call is only allowed in constructors");
    }
    return;
  }
  const topLevelSuperCalls = method.body.filter((s) => s.kind === "exprStmt" && s.expr.kind === "superCall");
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
function compile(source) {
  const tokens = lex(source);
  const classDecls = parseAll(tokens);
  if (classDecls.length === 1) {
    return generateClassFile(classDecls[0], classDecls);
  }
  const classFiles = classDecls.map((cd) => generateClassFile(cd, classDecls));
  let total = 0;
  for (const cf of classFiles) total += 4 + cf.length;
  const bundle = new Uint8Array(total);
  let off = 0;
  for (const cf of classFiles) {
    bundle[off++] = cf.length >> 24 & 255;
    bundle[off++] = cf.length >> 16 & 255;
    bundle[off++] = cf.length >> 8 & 255;
    bundle[off++] = cf.length & 255;
    bundle.set(cf, off);
    off += cf.length;
  }
  return bundle;
}
function generateClassFile(classDecl, allClassDecls = [classDecl]) {
  const allMethods = allClassDecls.flatMap((cd) => cd.methods);
  const classSupers = /* @__PURE__ */ new Map();
  const classDecls = /* @__PURE__ */ new Map();
  for (const cd of allClassDecls) {
    classSupers.set(cd.name, cd.superClass);
    classDecls.set(cd.name, cd);
  }
  const lambdaCounter = { value: 0 };
  const generatedMethods = [];
  const lambdaBootstraps = [];
  const inheritedFields = [];
  let superName = classDecl.superClass;
  while (superName && superName !== "java/lang/Object") {
    const superDecl = allClassDecls.find((cd) => cd.name === superName);
    if (!superDecl) break;
    inheritedFields.push(...superDecl.fields.filter((f) => !f.isStatic));
    superName = superDecl.superClass;
  }
  const cp = new ConstantPoolBuilder();
  const thisClassIdx = cp.addClass(classDecl.name);
  const superClassIdx = cp.addClass(classDecl.superClass);
  const hasInit = classDecl.methods.some((m) => m.name === "<init>");
  if (!hasInit) {
    classDecl.methods.unshift({
      name: "<init>",
      returnType: "void",
      params: [],
      body: [],
      isStatic: false
    });
  }
  const compiledMethods = [];
  const methodQueue = [...classDecl.methods];
  let generatedDrain = 0;
  for (let mi = 0; mi < methodQueue.length; mi++) {
    const method = methodQueue[mi];
    validateConstructorBody(method);
    const nameIdx = cp.addUtf8(method.name);
    const desc = methodDescriptor(method.params, method.returnType);
    const descIdx = cp.addUtf8(desc);
    let accessFlags = 1;
    if (method.isStatic) accessFlags |= 8;
    if (method.name === "<init>") {
      const emitter = new BytecodeEmitter();
      const hasSuperCall = method.body.length > 0 && method.body[0].kind === "exprStmt" && method.body[0].expr.kind === "superCall";
      if (!hasSuperCall) {
        const superInitRef = cp.addMethodref(classDecl.superClass, "<init>", "()V");
        emitter.emitAload(0);
        emitter.emitInvokespecial(superInitRef, 0, false);
      }
      const initCtx = {
        className: classDecl.name,
        superClass: classDecl.superClass,
        cp,
        method,
        locals: method.params.map((p, i) => ({ name: p.name, type: p.type, slot: i + 1 })),
        nextSlot: method.params.length + 1,
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
        ownerIsStatic: false
      };
      if (emitter.maxLocals < method.params.length + 1) emitter.maxLocals = method.params.length + 1;
      for (const field of classDecl.fields) {
        if (!field.isStatic && field.initializer) {
          emitter.emitAload(0);
          compileExpr(initCtx, emitter, field.initializer, field.type);
          const fRef = cp.addFieldref(classDecl.name, field.name, typeToDescriptor(field.type));
          emitter.emit(181);
          emitter.emitU16(fRef);
        }
      }
      for (const stmt of method.body) {
        compileStmt(initCtx, emitter, stmt);
      }
      emitter.emit(177);
      compiledMethods.push({
        nameIdx,
        descIdx,
        accessFlags,
        code: emitter.code,
        maxStack: Math.max(emitter.maxStack, 4),
        maxLocals: Math.max(emitter.maxLocals, method.params.length + 1)
      });
    } else {
      const result = compileMethod(
        classDecl,
        method,
        cp,
        allMethods,
        inheritedFields,
        classSupers,
        classDecls,
        lambdaCounter,
        generatedMethods,
        lambdaBootstraps
      );
      compiledMethods.push({
        nameIdx,
        descIdx,
        accessFlags,
        code: result.code,
        maxStack: result.maxStack,
        maxLocals: result.maxLocals
      });
    }
    while (generatedDrain < generatedMethods.length) {
      const gm = generatedMethods[generatedDrain++];
      methodQueue.push(gm);
      allMethods.push(gm);
    }
  }
  const compiledFields = [];
  for (const field of classDecl.fields) {
    const nameIdx = cp.addUtf8(field.name);
    const descIdx = cp.addUtf8(typeToDescriptor(field.type));
    let accessFlags = field.isPrivate ? 2 : 1;
    if (field.isStatic) accessFlags |= 8;
    if (field.isFinal) accessFlags |= 16;
    compiledFields.push({ nameIdx, descIdx, accessFlags });
  }
  const codeAttrName = cp.addUtf8("Code");
  const bootstrapAttrName = cp.addUtf8("BootstrapMethods");
  const serializedBootstrapMethods = [];
  for (const lb of lambdaBootstraps) {
    const metafactoryRef = cp.addMethodref("java/lang/invoke/LambdaMetafactory", "metafactory", "()V");
    const bootstrapMethodRef = cp.addMethodHandle(6, metafactoryRef);
    const implMethodRef = lb.implIsInterface ? cp.addInterfaceMethodref(lb.implOwner, lb.implMethodName, lb.implDescriptor) : cp.addMethodref(lb.implOwner, lb.implMethodName, lb.implDescriptor);
    const implHandle = cp.addMethodHandle(lb.implRefKind, implMethodRef);
    const samType = cp.addMethodType(lb.implDescriptor);
    serializedBootstrapMethods.push({ methodRef: bootstrapMethodRef, args: [samType, implHandle] });
  }
  const out = [];
  out.push(202, 254, 186, 190);
  out.push(0, 0);
  out.push(0, 52);
  out.push(...cp.serialize());
  const classFlags = classDecl.isRecord ? 49 : 33;
  out.push(classFlags >> 8 & 255, classFlags & 255);
  out.push(thisClassIdx >> 8 & 255, thisClassIdx & 255);
  out.push(superClassIdx >> 8 & 255, superClassIdx & 255);
  out.push(0, 0);
  out.push(compiledFields.length >> 8 & 255, compiledFields.length & 255);
  for (const f of compiledFields) {
    out.push(f.accessFlags >> 8 & 255, f.accessFlags & 255);
    out.push(f.nameIdx >> 8 & 255, f.nameIdx & 255);
    out.push(f.descIdx >> 8 & 255, f.descIdx & 255);
    out.push(0, 0);
  }
  out.push(compiledMethods.length >> 8 & 255, compiledMethods.length & 255);
  for (const m of compiledMethods) {
    out.push(m.accessFlags >> 8 & 255, m.accessFlags & 255);
    out.push(m.nameIdx >> 8 & 255, m.nameIdx & 255);
    out.push(m.descIdx >> 8 & 255, m.descIdx & 255);
    out.push(0, 1);
    out.push(codeAttrName >> 8 & 255, codeAttrName & 255);
    const codeLen = m.code.length;
    const attrLen = 2 + 2 + 4 + codeLen + 2 + 2;
    out.push(attrLen >> 24 & 255, attrLen >> 16 & 255, attrLen >> 8 & 255, attrLen & 255);
    out.push(m.maxStack >> 8 & 255, m.maxStack & 255);
    out.push(m.maxLocals >> 8 & 255, m.maxLocals & 255);
    out.push(codeLen >> 24 & 255, codeLen >> 16 & 255, codeLen >> 8 & 255, codeLen & 255);
    out.push(...m.code);
    out.push(0, 0);
    out.push(0, 0);
  }
  const classAttrCount = serializedBootstrapMethods.length > 0 ? 1 : 0;
  out.push(classAttrCount >> 8 & 255, classAttrCount & 255);
  if (serializedBootstrapMethods.length > 0) {
    out.push(bootstrapAttrName >> 8 & 255, bootstrapAttrName & 255);
    const bmCount = serializedBootstrapMethods.length;
    const bodyLen = 2 + serializedBootstrapMethods.reduce((s, bm) => s + 4 + bm.args.length * 2, 0);
    out.push(bodyLen >> 24 & 255, bodyLen >> 16 & 255, bodyLen >> 8 & 255, bodyLen & 255);
    out.push(bmCount >> 8 & 255, bmCount & 255);
    for (const bm of serializedBootstrapMethods) {
      out.push(bm.methodRef >> 8 & 255, bm.methodRef & 255);
      out.push(bm.args.length >> 8 & 255, bm.args.length & 255);
      for (const a of bm.args) out.push(a >> 8 & 255, a & 255);
    }
  }
  return new Uint8Array(out);
}
var OPCODES = {
  0: "nop",
  1: "aconst_null",
  2: "iconst_m1",
  3: "iconst_0",
  4: "iconst_1",
  5: "iconst_2",
  6: "iconst_3",
  7: "iconst_4",
  8: "iconst_5",
  9: "lconst_0",
  10: "lconst_1",
  16: "bipush",
  17: "sipush",
  18: "ldc",
  19: "ldc_w",
  21: "iload",
  25: "aload",
  26: "iload_0",
  27: "iload_1",
  28: "iload_2",
  29: "iload_3",
  42: "aload_0",
  43: "aload_1",
  44: "aload_2",
  45: "aload_3",
  54: "istore",
  58: "astore",
  59: "istore_0",
  60: "istore_1",
  61: "istore_2",
  62: "istore_3",
  75: "astore_0",
  76: "astore_1",
  77: "astore_2",
  78: "astore_3",
  87: "pop",
  88: "pop2",
  89: "dup",
  96: "iadd",
  100: "isub",
  104: "imul",
  108: "idiv",
  112: "irem",
  116: "ineg",
  132: "iinc",
  153: "ifeq",
  154: "ifne",
  155: "iflt",
  156: "ifge",
  157: "ifgt",
  158: "ifle",
  159: "if_icmpeq",
  160: "if_icmpne",
  161: "if_icmplt",
  162: "if_icmpge",
  163: "if_icmpgt",
  164: "if_icmple",
  165: "if_acmpeq",
  166: "if_acmpne",
  167: "goto",
  172: "ireturn",
  176: "areturn",
  177: "return",
  178: "getstatic",
  179: "putstatic",
  180: "getfield",
  181: "putfield",
  182: "invokevirtual",
  183: "invokespecial",
  184: "invokestatic",
  185: "invokeinterface",
  186: "invokedynamic",
  187: "new",
  188: "newarray",
  190: "arraylength",
  191: "athrow",
  192: "checkcast",
  193: "instanceof",
  198: "ifnull",
  199: "ifnonnull"
};
var OPCODE_WIDTHS = {
  16: 1,
  17: 2,
  18: 1,
  19: 2,
  21: 1,
  25: 1,
  54: 1,
  58: 1,
  132: 2,
  153: 2,
  154: 2,
  155: 2,
  156: 2,
  157: 2,
  158: 2,
  159: 2,
  160: 2,
  161: 2,
  162: 2,
  163: 2,
  164: 2,
  165: 2,
  166: 2,
  167: 2,
  178: 2,
  179: 2,
  180: 2,
  181: 2,
  182: 2,
  183: 2,
  184: 2,
  185: 4,
  186: 4,
  187: 2,
  188: 1,
  192: 2,
  193: 2,
  198: 2,
  199: 2
};
function disassemble(classBytes) {
  const dv = new DataView(classBytes.buffer, classBytes.byteOffset, classBytes.byteLength);
  const lines = [];
  let pos = 0;
  function u8() {
    return dv.getUint8(pos++);
  }
  function u16() {
    const v = dv.getUint16(pos);
    pos += 2;
    return v;
  }
  function u32() {
    const v = dv.getUint32(pos);
    pos += 4;
    return v;
  }
  function skip(n) {
    pos += n;
  }
  const magic = u32();
  if (magic !== 3405691582) return "Not a valid .class file";
  const minor = u16(), major = u16();
  const cpCount = u16();
  const cp = [null];
  for (let i = 1; i < cpCount; i++) {
    const tag = u8();
    switch (tag) {
      case 1: {
        const len = u16();
        let s = "";
        for (let j = 0; j < len; j++) s += String.fromCharCode(u8());
        cp.push(s);
        break;
      }
      case 7: {
        cp.push(`#class:${u16()}`);
        break;
      }
      case 8: {
        cp.push(`#str:${u16()}`);
        break;
      }
      case 9: {
        cp.push(`#field:${u16()}:${u16()}`);
        break;
      }
      case 10: {
        cp.push(`#meth:${u16()}:${u16()}`);
        break;
      }
      case 11: {
        cp.push(`#imeth:${u16()}:${u16()}`);
        break;
      }
      case 12: {
        cp.push(`#nat:${u16()}:${u16()}`);
        break;
      }
      case 18: {
        cp.push(`#indy:${u16()}:${u16()}`);
        break;
      }
      case 3: {
        cp.push(`int:${dv.getInt32(pos)}`);
        pos += 4;
        break;
      }
      case 4: {
        cp.push(`float:${dv.getFloat32(pos)}`);
        pos += 4;
        break;
      }
      case 5: {
        cp.push(`long:${dv.getBigInt64 ? dv.getBigInt64(pos) : pos}`);
        pos += 8;
        cp.push(null);
        i++;
        break;
      }
      case 15: {
        cp.push(`#mhnd:${u8()}:${u16()}`);
        break;
      }
      case 16: {
        cp.push(`#mtype:${u16()}`);
        break;
      }
      default: {
        cp.push(`?tag${tag}`);
        break;
      }
    }
  }
  function cpClass(idx) {
    const entry = cp[idx];
    if (!entry) return `#${idx}`;
    const m = entry.match(/^#class:(\d+)$/);
    return m ? (cp[+m[1]] ?? `#${m[1]}`).replace(/\//g, ".") : entry;
  }
  function cpNat(idx) {
    const entry = cp[idx] ?? "";
    const m = entry.match(/^#nat:(\d+):(\d+)$/);
    if (!m) return ["?", "?"];
    return [cp[+m[1]] ?? "?", cp[+m[2]] ?? "?"];
  }
  function cpRef(idx) {
    const entry = cp[idx] ?? "";
    const m = entry.match(/^#(?:meth|field|imeth):(\d+):(\d+)$/);
    if (!m) return `#${idx}`;
    const cls = cpClass(+m[1]);
    const [name, desc] = cpNat(+m[2]);
    return `${cls}.${name}:${desc}`;
  }
  function cpString(idx) {
    const entry = cp[idx] ?? "";
    const m = entry.match(/^#str:(\d+)$/);
    return m ? `"${cp[+m[1]] ?? ""}"` : entry;
  }
  function cpIndy(idx) {
    const entry = cp[idx] ?? "";
    const m = entry.match(/^#indy:(\d+):(\d+)$/);
    if (!m) return `#${idx}`;
    const [name, desc] = cpNat(+m[2]);
    return `#${m[1]}:${name}${desc}`;
  }
  const accessFlags = u16();
  const thisClass = cpClass(u16());
  const superClass = cpClass(u16());
  const flagStr = [
    accessFlags & 1 ? "public" : "",
    accessFlags & 32 ? "/* super */" : ""
  ].filter(Boolean).join(" ");
  lines.push(`${flagStr} class ${thisClass}`);
  if (superClass && superClass !== "java.lang.Object") {
    lines.push(`  extends ${superClass}`);
  }
  const ifCount = u16();
  for (let i = 0; i < ifCount; i++) u16();
  const fieldCount = u16();
  if (fieldCount > 0) lines.push("");
  for (let i = 0; i < fieldCount; i++) {
    const fFlags = u16();
    const fName = cp[u16()] ?? "?";
    const fDesc = cp[u16()] ?? "?";
    const fAccess = [
      fFlags & 1 ? "public" : fFlags & 2 ? "private" : "",
      fFlags & 8 ? "static" : "",
      fFlags & 16 ? "final" : ""
    ].filter(Boolean).join(" ");
    lines.push(`  ${fAccess} ${descToType(fDesc)} ${fName};`);
    const attrCount = u16();
    for (let a = 0; a < attrCount; a++) {
      u16();
      skip(u32());
    }
  }
  const methodCount = u16();
  for (let i = 0; i < methodCount; i++) {
    const mFlags = u16();
    const mName = cp[u16()] ?? "?";
    const mDesc = cp[u16()] ?? "?";
    const mAccess = [
      mFlags & 1 ? "public" : mFlags & 2 ? "private" : "",
      mFlags & 8 ? "static" : ""
    ].filter(Boolean).join(" ");
    const [paramTypes, retType] = parseDescriptor(mDesc);
    const paramStr = paramTypes.map((t, j) => `${t} arg${j}`).join(", ");
    const displayName = mName === "<init>" ? thisClass.split(".").pop() : mName;
    lines.push("");
    lines.push(`  ${mAccess} ${mName === "<init>" ? "" : retType + " "}${displayName}(${paramStr});`);
    const attrCount = u16();
    for (let a = 0; a < attrCount; a++) {
      const attrName = cp[u16()] ?? "?";
      const attrLen = u32();
      if (attrName === "Code") {
        lines.push("    Code:");
        u16();
        u16();
        const codeLen = u32();
        const codeStart = pos;
        const codeEnd = codeStart + codeLen;
        while (pos < codeEnd) {
          const offset = pos - codeStart;
          const op = u8();
          const opName = OPCODES[op] ?? `unknown(0x${op.toString(16).padStart(2, "0")})`;
          const width = OPCODE_WIDTHS[op] ?? 0;
          let operandStr = "";
          if (op === 182 || op === 183 || op === 184) {
            const ref = u16();
            operandStr = `#${ref.toString().padStart(2)} // ${cpRef(ref)}`;
          } else if (op === 185 || op === 186) {
            const ref = u16();
            skip(2);
            const label = op === 186 ? cpIndy(ref) : cpRef(ref);
            operandStr = `#${ref.toString().padStart(2)} // ${op === 186 ? "InvokeDynamic" : "InterfaceMethod"} ${label}`;
          } else if (op === 178 || op === 179 || op === 180 || op === 181) {
            const ref = u16();
            operandStr = `#${ref.toString().padStart(2)} // ${cpRef(ref)}`;
          } else if (op === 187 || op === 192 || op === 193) {
            const ref = u16();
            operandStr = `#${ref.toString().padStart(2)} // class ${cpClass(ref)}`;
          } else if (op === 18) {
            const ref = u8();
            const v = cp[ref] ?? `#${ref}`;
            operandStr = `#${ref.toString().padStart(2)} // ${v.startsWith("#str:") ? cpString(ref) : v}`;
          } else if (op === 19) {
            const ref = u16();
            const v = cp[ref] ?? `#${ref}`;
            operandStr = `#${ref.toString().padStart(2)} // ${v.startsWith("#str:") ? cpString(ref) : v}`;
          } else if (op === 132) {
            const idx = u8(), c = dv.getInt8(pos++);
            operandStr = `${idx}, ${c}`;
          } else if (op === 16) {
            operandStr = `${dv.getInt8(pos++)}`;
          } else if (op === 17) {
            operandStr = `${dv.getInt16(pos)}`;
            pos += 2;
          } else if (width === 1) {
            operandStr = `${u8()}`;
          } else if (width === 2) {
            const raw = dv.getInt16(pos);
            pos += 2;
            if (op >= 153 && op <= 167) operandStr = `${offset + raw}`;
            else operandStr = `${raw}`;
          } else if (width === 4) {
            operandStr = `${dv.getInt32(pos)}`;
            pos += 4;
          }
          lines.push(`       ${offset.toString().padStart(3)}: ${opName.padEnd(18)} ${operandStr}`);
        }
        const excCount = u16();
        skip(excCount * 8);
        const codeAttrCount = u16();
        for (let ca = 0; ca < codeAttrCount; ca++) {
          u16();
          skip(u32());
        }
      } else {
        skip(attrLen);
      }
    }
  }
  lines.unshift(`// class file v${major}.${minor}`);
  return lines.join("\n");
}
function descToType(desc) {
  if (desc === "I") return "int";
  if (desc === "Z") return "boolean";
  if (desc === "V") return "void";
  if (desc === "J") return "long";
  if (desc === "D") return "double";
  if (desc === "F") return "float";
  if (desc.startsWith("L") && desc.endsWith(";")) {
    return desc.slice(1, -1).split("/").pop();
  }
  if (desc.startsWith("[")) return descToType(desc.slice(1)) + "[]";
  return desc;
}
function parseDescriptor(desc) {
  const m = desc.match(/^\(([^)]*)\)(.+)$/);
  if (!m) return [[], desc];
  const params = [];
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
export {
  TokenKind,
  buildMethodRegistry,
  classFilesToBundle,
  compile,
  disassemble,
  generateClassFile,
  lex,
  parseAll,
  parseBundleMeta,
  parseClassMeta,
  readJar,
  setMethodRegistry
};
