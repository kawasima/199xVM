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
  KwInterface = "interface",
  KwEnum = "enum",
  KwDo = "do",
  KwThrow = "throw",
  KwTry = "try",
  KwCatch = "catch",
  KwFinally = "finally",
  KwAssert = "assert",
  KwSynchronized = "synchronized",
  KwBreak = "break",
  KwContinue = "continue",

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
  interface: TokenKind.KwInterface,
  enum: TokenKind.KwEnum,
  do: TokenKind.KwDo,
  throw: TokenKind.KwThrow,
  try: TokenKind.KwTry,
  catch: TokenKind.KwCatch,
  finally: TokenKind.KwFinally,
  assert: TokenKind.KwAssert,
  synchronized: TokenKind.KwSynchronized,
  break: TokenKind.KwBreak,
  continue: TokenKind.KwContinue,
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
  function scanDigits(startLine: number, startCol: number, digitRe: RegExp): string {
    let out = "";
    let sawDigit = false;
    let prevUnderscore = false;
    while (true) {
      const c = peek();
      if (digitRe.test(c)) {
        sawDigit = true;
        prevUnderscore = false;
        out += advance();
        continue;
      }
      if (c === "_") {
        const next = peekN(1);
        if (!sawDigit || prevUnderscore || !digitRe.test(next)) {
          throw new Error(`Invalid underscore placement in number literal at line ${startLine}:${startCol}`);
        }
        prevUnderscore = true;
        out += advance();
        continue;
      }
      break;
    }
    if (!sawDigit || prevUnderscore) {
      throw new Error(`Malformed number literal at line ${startLine}:${startCol}`);
    }
    return out;
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
      while (pos < source.length) {
        if (peek() === '"' && peekN(1) === '"' && peekN(2) === '"') {
          advance(); advance(); advance();
          tokens.push({ kind: TokenKind.StringLiteral, value: s, line: startLine, col: startCol });
          s = "";
          break;
        }
        if (peek() === "\\") {
          advance();
          s += parseEscape(startLine, startCol, true);
        } else {
          s += advance();
        }
      }
      if (s.length > 0 || !(tokens[tokens.length - 1]?.kind === TokenKind.StringLiteral && tokens[tokens.length - 1].line === startLine && tokens[tokens.length - 1].col === startCol)) {
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

    // Floating-point starting with decimal point: .123, .5e2, .0f
    if (ch === "." && /[0-9]/.test(peekN(1))) {
      let raw = ".";
      advance();
      raw += scanDigits(startLine, startCol, /[0-9]/);
      if (peek() === "e" || peek() === "E") {
        raw += advance();
        if (peek() === "+" || peek() === "-") raw += advance();
        raw += scanDigits(startLine, startCol, /[0-9]/);
      }
      const isFloat = peek() === "f" || peek() === "F";
      const isDouble = peek() === "d" || peek() === "D";
      if (isFloat || isDouble) raw += advance();
      tokens.push({ kind: isFloat ? TokenKind.FloatLiteral : TokenKind.DoubleLiteral, value: raw, line: startLine, col: startCol });
      continue;
    }

    // Number literal
    if (/[0-9]/.test(ch)) {
      let raw = "";
      let hasDecimal = false;
      if (peek() === "0" && (source[pos + 1] === "x" || source[pos + 1] === "X")) {
        raw += advance(); // 0
        raw += advance(); // x/X
        raw += scanDigits(startLine, startCol, /[0-9a-fA-F]/);
      } else if (peek() === "0" && (source[pos + 1] === "b" || source[pos + 1] === "B")) {
        raw += advance(); // 0
        raw += advance(); // b/B
        raw += scanDigits(startLine, startCol, /[01]/);
      } else if (peek() === "0" && /[0-9_]/.test(source[pos + 1] ?? "")) {
        raw += advance(); // leading 0
        raw += scanDigits(startLine, startCol, /[0-7]/);
      } else {
        raw += scanDigits(startLine, startCol, /[0-9]/);
      }
      // Decimal point (float/double)
      if (peek() === ".") {
        hasDecimal = true;
        raw += advance(); // '.'
        if (/[0-9_]/.test(peek())) raw += scanDigits(startLine, startCol, /[0-9]/);
      }
      // Exponent
      if (peek() === "e" || peek() === "E") {
        hasDecimal = true; // exponent implies floating point
        raw += advance();
        if (peek() === "+" || peek() === "-") raw += advance();
        raw += scanDigits(startLine, startCol, /[0-9]/);
      }
      // Suffix
      const isFloat = peek() === "f" || peek() === "F";
      const isDouble = peek() === "d" || peek() === "D";
      const isLong = !isFloat && !isDouble && (peek() === "L" || peek() === "l");
      if (isFloat || isDouble || isLong) raw += advance();
      if (isLong && hasDecimal) {
        throw new Error(`Invalid long literal with floating-point form at line ${startLine}:${startCol}`);
      }
      if (isFloat) {
        tokens.push({ kind: TokenKind.FloatLiteral, value: raw, line: startLine, col: startCol });
      } else if (isDouble || hasDecimal) {
        tokens.push({ kind: TokenKind.DoubleLiteral, value: raw, line: startLine, col: startCol });
      } else if (isLong) {
        tokens.push({ kind: TokenKind.LongLiteral, value: raw, line: startLine, col: startCol });
      } else {
        tokens.push({ kind: TokenKind.IntLiteral, value: raw, line: startLine, col: startCol });
      }
      continue;
    }

    // Identifier / keyword
    if (isIdentifierStart(ch)) {
      let ident = "";
      while (isIdentifierPart(peek())) ident += advance();
      const kw = Object.prototype.hasOwnProperty.call(KEYWORDS, ident) ? KEYWORDS[ident] : undefined;
      tokens.push({ kind: kw ?? TokenKind.Ident, value: ident, line: startLine, col: startCol });
      continue;
    }

    // Multi-char operators
    const two = pos + 1 < source.length ? ch + source[pos + 1] : "";
    const three = pos + 2 < source.length ? ch + source[pos + 1] + source[pos + 2] : "";
    if (three === "<<=") { advance(); advance(); advance(); tokens.push({ kind: TokenKind.ShiftLeftAssign, value: "<<=", line: startLine, col: startCol }); continue; }
    if (two === "==") { advance(); advance(); tokens.push({ kind: TokenKind.Eq, value: "==", line: startLine, col: startCol }); continue; }
    if (two === "!=") { advance(); advance(); tokens.push({ kind: TokenKind.Ne, value: "!=", line: startLine, col: startCol }); continue; }
    if (two === "<=") { advance(); advance(); tokens.push({ kind: TokenKind.Le, value: "<=", line: startLine, col: startCol }); continue; }
    if (two === ">=") { advance(); advance(); tokens.push({ kind: TokenKind.Ge, value: ">=", line: startLine, col: startCol }); continue; }
    if (two === "&&") { advance(); advance(); tokens.push({ kind: TokenKind.And, value: "&&", line: startLine, col: startCol }); continue; }
    if (two === "||") { advance(); advance(); tokens.push({ kind: TokenKind.Or, value: "||", line: startLine, col: startCol }); continue; }
    if (two === "<<") { advance(); advance(); tokens.push({ kind: TokenKind.ShiftLeft, value: "<<", line: startLine, col: startCol }); continue; }
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
