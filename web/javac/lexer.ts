export enum TokenKind {
  // Literals
  IntLiteral = "IntLiteral",
  LongLiteral = "LongLiteral",
  FloatLiteral = "FloatLiteral",
  DoubleLiteral = "DoubleLiteral",
  CharLiteral = "CharLiteral",
  StringLiteral = "StringLiteral",
  BoolLiteral = "BoolLiteral",
  NullLiteral = "NullLiteral",

  // Identifiers & keywords
  Ident = "Ident",
  KwClass = "class",
  KwPublic = "public",
  KwStatic = "static",
  KwVoid = "void",
  KwInt = "int",
  KwLong = "long",
  KwShort = "short",
  KwByte = "byte",
  KwChar = "char",
  KwFloat = "float",
  KwDouble = "double",
  KwBoolean = "boolean",
  KwString = "String",
  KwReturn = "return",
  KwNew = "new",
  KwIf = "if",
  KwElse = "else",
  KwWhile = "while",
  KwFor = "for",
  KwSwitch = "switch",
  KwCase = "case",
  KwDefault = "default",
  KwYield = "yield",
  KwWhen = "when",
  KwThis = "this",
  KwSuper = "super",
  KwExtends = "extends",
  KwImplements = "implements",
  KwImport = "import",
  KwPackage = "package",
  KwPrivate = "private",
  KwProtected = "protected",
  KwFinal = "final",
  KwAbstract = "abstract",
  KwVar = "var",
  KwInstanceof = "instanceof",
  KwRecord = "record",
  KwModule = "module",
  KwOpen = "open",
  KwRequires = "requires",
  KwTransitive = "transitive",
  KwExports = "exports",
  KwOpens = "opens",
  KwTo = "to",
  KwUses = "uses",
  KwProvides = "provides",
  KwWith = "with",
  KwSealed = "sealed",
  KwPermits = "permits",
  KwNonSealed = "non-sealed",
  KwInterface = "interface",
  KwEnum = "enum",
  KwDo = "do",
  KwThrow = "throw",
  KwThrows = "throws",
  KwTry = "try",
  KwCatch = "catch",
  KwFinally = "finally",
  KwAssert = "assert",
  KwSynchronized = "synchronized",
  KwBreak = "break",
  KwContinue = "continue",
  KwNative = "native",
  KwStrictfp = "strictfp",
  KwTransient = "transient",
  KwVolatile = "volatile",
  KwConst = "const",
  KwGoto = "goto",

  // Delimiters
  LParen = "(",
  RParen = ")",
  LBrace = "{",
  RBrace = "}",
  LBracket = "[",
  RBracket = "]",
  Semi = ";",
  Comma = ",",
  Dot = ".",
  Ellipsis = "...",
  At = "@",

  // Operators
  Plus = "+",
  Minus = "-",
  Star = "*",
  Slash = "/",
  Percent = "%",
  Assign = "=",
  Eq = "==",
  Ne = "!=",
  Lt = "<",
  Gt = ">",
  Le = "<=",
  Ge = ">=",
  And = "&&",
  Or = "||",
  BitAnd = "&",
  BitOr = "|",
  BitXor = "^",
  BitNot = "~",
  ShiftLeft = "<<",
  ShiftRight = ">>",
  ShiftUnsigned = ">>>",
  Not = "!",
  PlusAssign = "+=",
  MinusAssign = "-=",
  StarAssign = "*=",
  SlashAssign = "/=",
  PercentAssign = "%=",
  AndAssign = "&=",
  OrAssign = "|=",
  XorAssign = "^=",
  ShiftLeftAssign = "<<=",
  ShiftRightAssign = ">>=",
  ShiftUnsignedAssign = ">>>=",
  PlusPlus = "++",
  MinusMinus = "--",
  Question = "?",
  Colon = ":",
  ColonColon = "::",
  Arrow = "->",

  // Special
  EOF = "EOF",
}

export interface Token {
  kind: TokenKind;
  value: string;
  line: number;
  col: number;
}

const IDENT_START_RE = /[$_\p{ID_Start}]/u;
const IDENT_PART_RE = /[$_\u200C\u200D\p{ID_Continue}]/u;

function isIdentifierStart(ch: string): boolean {
  return ch !== "\0" && IDENT_START_RE.test(ch);
}

function isIdentifierPart(ch: string): boolean {
  return ch !== "\0" && IDENT_PART_RE.test(ch);
}

function preprocessUnicodeEscapes(input: string): string {
  // JLS 3.3: translate Unicode escapes before lexical analysis.
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

const KEYWORDS: Record<string, TokenKind> = {
  class: TokenKind.KwClass,
  public: TokenKind.KwPublic,
  static: TokenKind.KwStatic,
  void: TokenKind.KwVoid,
  int: TokenKind.KwInt,
  long: TokenKind.KwLong,
  short: TokenKind.KwShort,
  byte: TokenKind.KwByte,
  char: TokenKind.KwChar,
  float: TokenKind.KwFloat,
  double: TokenKind.KwDouble,
  boolean: TokenKind.KwBoolean,
  String: TokenKind.KwString,
  return: TokenKind.KwReturn,
  new: TokenKind.KwNew,
  if: TokenKind.KwIf,
  else: TokenKind.KwElse,
  while: TokenKind.KwWhile,
  for: TokenKind.KwFor,
  switch: TokenKind.KwSwitch,
  case: TokenKind.KwCase,
  default: TokenKind.KwDefault,
  yield: TokenKind.KwYield,
  when: TokenKind.KwWhen,
  this: TokenKind.KwThis,
  super: TokenKind.KwSuper,
  true: TokenKind.BoolLiteral,
  false: TokenKind.BoolLiteral,
  null: TokenKind.NullLiteral,
  extends: TokenKind.KwExtends,
  implements: TokenKind.KwImplements,
  import: TokenKind.KwImport,
  package: TokenKind.KwPackage,
  private: TokenKind.KwPrivate,
  protected: TokenKind.KwProtected,
  final: TokenKind.KwFinal,
  abstract: TokenKind.KwAbstract,
  var: TokenKind.KwVar,
  instanceof: TokenKind.KwInstanceof,
  record: TokenKind.KwRecord,
  module: TokenKind.KwModule,
  open: TokenKind.KwOpen,
  requires: TokenKind.KwRequires,
  transitive: TokenKind.KwTransitive,
  exports: TokenKind.KwExports,
  opens: TokenKind.KwOpens,
  to: TokenKind.KwTo,
  uses: TokenKind.KwUses,
  provides: TokenKind.KwProvides,
  with: TokenKind.KwWith,
  sealed: TokenKind.KwSealed,
  permits: TokenKind.KwPermits,
  "non-sealed": TokenKind.KwNonSealed,
  interface: TokenKind.KwInterface,
  enum: TokenKind.KwEnum,
  do: TokenKind.KwDo,
  throw: TokenKind.KwThrow,
  throws: TokenKind.KwThrows,
  try: TokenKind.KwTry,
  catch: TokenKind.KwCatch,
  finally: TokenKind.KwFinally,
  assert: TokenKind.KwAssert,
  synchronized: TokenKind.KwSynchronized,
  break: TokenKind.KwBreak,
  continue: TokenKind.KwContinue,
  native: TokenKind.KwNative,
  strictfp: TokenKind.KwStrictfp,
  transient: TokenKind.KwTransient,
  volatile: TokenKind.KwVolatile,
  const: TokenKind.KwConst,
  goto: TokenKind.KwGoto,
};

export function lex(source: string): Token[] {
  source = preprocessUnicodeEscapes(source);
  const tokens: Token[] = [];
  let pos = 0;
  let line = 1;
  let col = 1;

  function peek(): string {
    return pos < source.length ? source[pos] : "\0";
  }
  function advance(): string {
    const ch = source[pos++];
    if (ch === "\n") { line++; col = 1; } else { col++; }
    return ch;
  }
  function peekN(n: number): string {
    return pos + n < source.length ? source[pos + n] : "\0";
  }
  function parseEscape(startLine: number, startCol: number, inTextBlock: boolean): string {
    const esc = advance();
    switch (esc) {
      case "b": return "\b";
      case "t": return "\t";
      case "n": return "\n";
      case "f": return "\f";
      case "r": return "\r";
      case "\"": return "\"";
      case "'": return "'";
      case "\\": return "\\";
      case "s": return " ";
      case "\r":
        if (!inTextBlock) throw new Error(`Invalid escape sequence at line ${startLine}:${startCol}`);
        if (peek() === "\n") advance();
        return "";
      case "\n":
        if (!inTextBlock) throw new Error(`Invalid escape sequence at line ${startLine}:${startCol}`);
        return "";
      default:
        if (/[0-7]/.test(esc)) {
          let oct = esc;
          const maxExtra = esc <= "3" ? 2 : 1;
          for (let i = 0; i < maxExtra; i++) {
            if (!/[0-7]/.test(peek())) break;
            oct += advance();
          }
          return String.fromCharCode(parseInt(oct, 8));
        }
        throw new Error(`Invalid escape sequence at line ${startLine}:${startCol}`);
    }
  }
  function parseNumberLiteral(startLine: number, startCol: number): { kind: TokenKind; value: string; len: number } | undefined {
    const rem = source.slice(pos);
    const DEC = "[0-9](?:_?[0-9])*";
    const NZDEC = "[1-9](?:_?[0-9])*";
    const OCT = "[0-7](?:_?[0-7])*";
    const HEX = "[0-9a-fA-F](?:_?[0-9a-fA-F])*";
    const BIN = "[01](?:_?[01])*";
    const EXP10 = `[eE][+-]?${DEC}`;
    const EXP2 = `[pP][+-]?${DEC}`;
    const patterns: Array<{ re: RegExp; kind: TokenKind }> = [
      { re: new RegExp(`^0[xX](?:${HEX}\\.(?:${HEX})?|(?:${HEX})?\\.${HEX})${EXP2}[fFdD]?`), kind: TokenKind.DoubleLiteral },
      { re: new RegExp(`^0[xX]${HEX}${EXP2}[fFdD]?`), kind: TokenKind.DoubleLiteral },
      { re: new RegExp(`^(?:${DEC}\\.(?:${DEC})?|\\.${DEC})(?:${EXP10})?[fFdD]?`), kind: TokenKind.DoubleLiteral },
      { re: new RegExp(`^${DEC}${EXP10}[fFdD]?`), kind: TokenKind.DoubleLiteral },
      { re: new RegExp(`^${DEC}[fFdD]`), kind: TokenKind.FloatLiteral },
      { re: new RegExp(`^0[xX]${HEX}[lL]?`), kind: TokenKind.IntLiteral },
      { re: new RegExp(`^0[bB]${BIN}[lL]?`), kind: TokenKind.IntLiteral },
      { re: new RegExp(`^0(?:_?[0-7])+[lL]?`), kind: TokenKind.IntLiteral },
      { re: new RegExp(`^(?:0|${NZDEC})[lL]?`), kind: TokenKind.IntLiteral },
    ];

    let best: { text: string; kind: TokenKind } | undefined;
    for (const p of patterns) {
      const m = rem.match(p.re);
      if (!m) continue;
      const text = m[0];
      if (!best || text.length > best.text.length) best = { text, kind: p.kind };
    }
    if (!best) return undefined;

    const matched = best.text;
    const last = matched[matched.length - 1];
    if (last === "_") {
      throw new Error(`Invalid underscore placement in number literal at line ${startLine}:${startCol}`);
    }

    const next = rem[matched.length] ?? "\0";
    if (isIdentifierPart(next)) {
      throw new Error(`Malformed number literal at line ${startLine}:${startCol}`);
    }
    if (rem.startsWith("0") && matched === "0" && /[0-9_]/.test(next)) {
      throw new Error(`Malformed octal literal at line ${startLine}:${startCol}`);
    }

    if (/[fF]/.test(last)) best.kind = TokenKind.FloatLiteral;
    else if (/[dD]/.test(last)) best.kind = TokenKind.DoubleLiteral;
    else if (/[lL]/.test(last)) best.kind = TokenKind.LongLiteral;
    else if (best.kind === TokenKind.IntLiteral) best.kind = TokenKind.IntLiteral;
    else best.kind = TokenKind.DoubleLiteral;
    return { kind: best.kind, value: matched, len: matched.length };
  }

  while (pos < source.length) {
    const ch = peek();

    // Whitespace
    if (/\s/.test(ch)) { advance(); continue; }

    // Line comment
    if (ch === "/" && pos + 1 < source.length && source[pos + 1] === "/") {
      while (pos < source.length && peek() !== "\n") advance();
      continue;
    }
    // Block comment
    if (ch === "/" && pos + 1 < source.length && source[pos + 1] === "*") {
      const cLine = line;
      const cCol = col;
      advance(); advance();
      while (pos + 1 < source.length && !(peek() === "*" && source[pos + 1] === "/")) advance();
      if (pos + 1 >= source.length) {
        throw new Error(`Unterminated block comment at line ${cLine}:${cCol}`);
      }
      advance(); advance();
      continue;
    }

    const startLine = line;
    const startCol = col;

    // Text block literal
    if (ch === '"' && peekN(1) === '"' && peekN(2) === '"') {
      advance(); advance(); advance();
      if (!(peek() === "\n" || peek() === "\r")) {
        throw new Error(`Text block opening delimiter must be followed by line terminator at line ${startLine}:${startCol}`);
      }
      if (peek() === "\r") {
        advance();
        if (peek() === "\n") advance();
      } else {
        advance();
      }
      let s = "";
      let closed = false;
      while (pos < source.length) {
        if (peek() === '"' && peekN(1) === '"' && peekN(2) === '"') {
          advance(); advance(); advance();
          tokens.push({ kind: TokenKind.StringLiteral, value: s, line: startLine, col: startCol });
          closed = true;
          break;
        }
        if (peek() === "\\") {
          advance();
          s += parseEscape(startLine, startCol, true);
        } else {
          s += advance();
        }
      }
      if (!closed) {
        throw new Error(`Unterminated text block at line ${startLine}:${startCol}`);
      }
      continue;
    }

    // String literal
    if (ch === '"') {
      advance();
      let s = "";
      while (peek() !== '"' && peek() !== "\0") {
        if (peek() === "\n" || peek() === "\r") {
          throw new Error(`Unterminated string literal at line ${startLine}:${startCol}`);
        }
        if (peek() === "\\") {
          advance();
          s += parseEscape(startLine, startCol, false);
        } else {
          s += advance();
        }
      }
      if (peek() === "\0") {
        throw new Error(`Unterminated string literal at line ${startLine}:${startCol}`);
      }
      advance(); // closing "
      tokens.push({ kind: TokenKind.StringLiteral, value: s, line: startLine, col: startCol });
      continue;
    }

    // Char literal
    if (ch === "'") {
      advance(); // opening '
      if (peek() === "'" || peek() === "\n" || peek() === "\r" || peek() === "\0") {
        throw new Error(`Malformed char literal at line ${startLine}:${startCol}`);
      }
      let chValue = "";
      if (peek() === "\\") {
        advance();
        chValue = parseEscape(startLine, startCol, false);
      } else {
        chValue = advance();
      }
      if (peek() !== "'") throw new Error(`Unterminated char literal at line ${startLine}:${startCol}`);
      advance(); // closing '
      if (chValue.length !== 1) throw new Error(`Malformed char literal at line ${startLine}:${startCol}`);
      tokens.push({ kind: TokenKind.CharLiteral, value: String(chValue.charCodeAt(0)), line: startLine, col: startCol });
      continue;
    }

    // Number literal
    if (/[0-9]/.test(ch) || (ch === "." && /[0-9]/.test(peekN(1)))) {
      const parsed = parseNumberLiteral(startLine, startCol);
      if (!parsed) throw new Error(`Malformed number literal at line ${startLine}:${startCol}`);
      for (let i = 0; i < parsed.len; i++) advance();
      tokens.push({ kind: parsed.kind, value: parsed.value, line: startLine, col: startCol });
      continue;
    }

    // Hyphenated restricted keyword: non-sealed
    if (source.startsWith("non-sealed", pos)) {
      const after = source[pos + "non-sealed".length] ?? "\0";
      if (!isIdentifierPart(after)) {
        for (let i = 0; i < "non-sealed".length; i++) advance();
        tokens.push({ kind: TokenKind.KwNonSealed, value: "non-sealed", line: startLine, col: startCol });
        continue;
      }
    }

    // Identifier / keyword
    if (isIdentifierStart(ch)) {
      let ident = "";
      while (isIdentifierPart(peek())) ident += advance();
      if (ident === "_") {
        throw new Error(`'_' is a reserved keyword and cannot be used as an identifier at line ${startLine}:${startCol}`);
      }
      const kw = Object.prototype.hasOwnProperty.call(KEYWORDS, ident) ? KEYWORDS[ident] : undefined;
      tokens.push({ kind: kw ?? TokenKind.Ident, value: ident, line: startLine, col: startCol });
      continue;
    }

    // Multi-char operators
    const two = pos + 1 < source.length ? ch + source[pos + 1] : "";
    const three = pos + 2 < source.length ? ch + source[pos + 1] + source[pos + 2] : "";
    const four = pos + 3 < source.length ? ch + source[pos + 1] + source[pos + 2] + source[pos + 3] : "";
    if (four === ">>>=") { advance(); advance(); advance(); advance(); tokens.push({ kind: TokenKind.ShiftUnsignedAssign, value: ">>>=", line: startLine, col: startCol }); continue; }
    if (three === "<<=") { advance(); advance(); advance(); tokens.push({ kind: TokenKind.ShiftLeftAssign, value: "<<=", line: startLine, col: startCol }); continue; }
    if (three === ">>>") { advance(); advance(); advance(); tokens.push({ kind: TokenKind.ShiftUnsigned, value: ">>>", line: startLine, col: startCol }); continue; }
    if (three === "...") { advance(); advance(); advance(); tokens.push({ kind: TokenKind.Ellipsis, value: "...", line: startLine, col: startCol }); continue; }
    if (three === ">>=") { advance(); advance(); advance(); tokens.push({ kind: TokenKind.ShiftRightAssign, value: ">>=", line: startLine, col: startCol }); continue; }
    if (two === "==") { advance(); advance(); tokens.push({ kind: TokenKind.Eq, value: "==", line: startLine, col: startCol }); continue; }
    if (two === "!=") { advance(); advance(); tokens.push({ kind: TokenKind.Ne, value: "!=", line: startLine, col: startCol }); continue; }
    if (two === "<=") { advance(); advance(); tokens.push({ kind: TokenKind.Le, value: "<=", line: startLine, col: startCol }); continue; }
    if (two === ">=") { advance(); advance(); tokens.push({ kind: TokenKind.Ge, value: ">=", line: startLine, col: startCol }); continue; }
    if (two === "&&") { advance(); advance(); tokens.push({ kind: TokenKind.And, value: "&&", line: startLine, col: startCol }); continue; }
    if (two === "||") { advance(); advance(); tokens.push({ kind: TokenKind.Or, value: "||", line: startLine, col: startCol }); continue; }
    if (two === "<<") { advance(); advance(); tokens.push({ kind: TokenKind.ShiftLeft, value: "<<", line: startLine, col: startCol }); continue; }
    if (two === ">>") { advance(); advance(); tokens.push({ kind: TokenKind.ShiftRight, value: ">>", line: startLine, col: startCol }); continue; }
    if (two === "+=") { advance(); advance(); tokens.push({ kind: TokenKind.PlusAssign, value: "+=", line: startLine, col: startCol }); continue; }
    if (two === "-=") { advance(); advance(); tokens.push({ kind: TokenKind.MinusAssign, value: "-=", line: startLine, col: startCol }); continue; }
    if (two === "*=") { advance(); advance(); tokens.push({ kind: TokenKind.StarAssign, value: "*=", line: startLine, col: startCol }); continue; }
    if (two === "/=") { advance(); advance(); tokens.push({ kind: TokenKind.SlashAssign, value: "/=", line: startLine, col: startCol }); continue; }
    if (two === "%=") { advance(); advance(); tokens.push({ kind: TokenKind.PercentAssign, value: "%=", line: startLine, col: startCol }); continue; }
    if (two === "&=") { advance(); advance(); tokens.push({ kind: TokenKind.AndAssign, value: "&=", line: startLine, col: startCol }); continue; }
    if (two === "|=") { advance(); advance(); tokens.push({ kind: TokenKind.OrAssign, value: "|=", line: startLine, col: startCol }); continue; }
    if (two === "^=") { advance(); advance(); tokens.push({ kind: TokenKind.XorAssign, value: "^=", line: startLine, col: startCol }); continue; }
    if (two === "::") { advance(); advance(); tokens.push({ kind: TokenKind.ColonColon, value: "::", line: startLine, col: startCol }); continue; }
    if (two === "->") { advance(); advance(); tokens.push({ kind: TokenKind.Arrow, value: "->", line: startLine, col: startCol }); continue; }
    if (two === "++") { advance(); advance(); tokens.push({ kind: TokenKind.PlusPlus, value: "++", line: startLine, col: startCol }); continue; }
    if (two === "--") { advance(); advance(); tokens.push({ kind: TokenKind.MinusMinus, value: "--", line: startLine, col: startCol }); continue; }

    // Single-char tokens
    const singles: Record<string, TokenKind> = {
      "(": TokenKind.LParen, ")": TokenKind.RParen,
      "{": TokenKind.LBrace, "}": TokenKind.RBrace,
      "[": TokenKind.LBracket, "]": TokenKind.RBracket,
      ";": TokenKind.Semi, ",": TokenKind.Comma, ".": TokenKind.Dot,
      "@": TokenKind.At,
      "+": TokenKind.Plus, "-": TokenKind.Minus,
      "*": TokenKind.Star, "/": TokenKind.Slash, "%": TokenKind.Percent,
      "=": TokenKind.Assign, "<": TokenKind.Lt, ">": TokenKind.Gt,
      "&": TokenKind.BitAnd, "|": TokenKind.BitOr, "^": TokenKind.BitXor, "~": TokenKind.BitNot, "!": TokenKind.Not,
      "?": TokenKind.Question, ":": TokenKind.Colon,
    };

    if (singles[ch]) {
      advance();
      tokens.push({ kind: singles[ch], value: ch, line: startLine, col: startCol });
      continue;
    }

    throw new Error(`Unknown character "${ch}" at line ${startLine}:${startCol}`);
  }

  tokens.push({ kind: TokenKind.EOF, value: "", line, col });
  return tokens;
}
