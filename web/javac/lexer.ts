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
  KwDo = "do",
  KwThrow = "throw",
  KwTry = "try",
  KwCatch = "catch",
  KwFinally = "finally",
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
  Not = "!",
  PlusAssign = "+=",
  MinusAssign = "-=",
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
  do: TokenKind.KwDo,
  throw: TokenKind.KwThrow,
  try: TokenKind.KwTry,
  catch: TokenKind.KwCatch,
  finally: TokenKind.KwFinally,
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
          const esc = advance();
          switch (esc) {
            case "n": s += "\n"; break;
            case "t": s += "\t"; break;
            case "\\": s += "\\"; break;
            case '"': s += '"'; break;
            default: s += esc;
          }
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
      let charVal: number;
      if (peek() === "\\") {
        advance(); // backslash
        const esc = advance();
        switch (esc) {
          case "n": charVal = 10; break;
          case "t": charVal = 9; break;
          case "r": charVal = 13; break;
          case "\\": charVal = 92; break;
          case "'": charVal = 39; break;
          case "\"": charVal = 34; break;
          case "0": charVal = 0; break;
          default: charVal = esc.charCodeAt(0);
        }
      } else {
        charVal = advance().charCodeAt(0);
      }
      if (peek() !== "'") throw new Error(`Unterminated char literal at line ${startLine}:${startCol}`);
      advance(); // closing '
      tokens.push({ kind: TokenKind.CharLiteral, value: String(charVal), line: startLine, col: startCol });
      continue;
    }

    // Number literal
    if (/[0-9]/.test(ch)) {
      let raw = "";
      if (peek() === "0" && (source[pos + 1] === "x" || source[pos + 1] === "X")) {
        raw += advance(); // 0
        raw += advance(); // x/X
        while (/[0-9a-fA-F_]/.test(peek())) raw += advance();
      } else if (peek() === "0" && (source[pos + 1] === "b" || source[pos + 1] === "B")) {
        raw += advance(); // 0
        raw += advance(); // b/B
        while (/[01_]/.test(peek())) raw += advance();
      } else if (peek() === "0" && /[0-7_]/.test(source[pos + 1] ?? "")) {
        raw += advance(); // leading 0
        while (/[0-7_]/.test(peek())) raw += advance();
      } else {
        while (/[0-9_]/.test(peek())) raw += advance();
      }
      // Decimal point (float/double)
      let hasDecimal = false;
      if (peek() === "." && /[0-9]/.test(source[pos + 1] ?? "")) {
        hasDecimal = true;
        raw += advance(); // '.'
        while (/[0-9_]/.test(peek())) raw += advance();
      }
      // Exponent
      if (peek() === "e" || peek() === "E") {
        hasDecimal = true; // exponent implies floating point
        raw += advance();
        if (peek() === "+" || peek() === "-") raw += advance();
        while (/[0-9_]/.test(peek())) raw += advance();
      }
      // Suffix
      const isFloat = peek() === "f" || peek() === "F";
      const isDouble = peek() === "d" || peek() === "D";
      const isLong = !isFloat && !isDouble && (peek() === "L" || peek() === "l");
      if (isFloat || isDouble || isLong) raw += advance();
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
    if (/[a-zA-Z_$]/.test(ch)) {
      let ident = "";
      while (/[a-zA-Z0-9_$]/.test(peek())) ident += advance();
      const kw = Object.prototype.hasOwnProperty.call(KEYWORDS, ident) ? KEYWORDS[ident] : undefined;
      tokens.push({ kind: kw ?? TokenKind.Ident, value: ident, line: startLine, col: startCol });
      continue;
    }

    // Multi-char operators
    const two = pos + 1 < source.length ? ch + source[pos + 1] : "";
    if (two === "==") { advance(); advance(); tokens.push({ kind: TokenKind.Eq, value: "==", line: startLine, col: startCol }); continue; }
    if (two === "!=") { advance(); advance(); tokens.push({ kind: TokenKind.Ne, value: "!=", line: startLine, col: startCol }); continue; }
    if (two === "<=") { advance(); advance(); tokens.push({ kind: TokenKind.Le, value: "<=", line: startLine, col: startCol }); continue; }
    if (two === ">=") { advance(); advance(); tokens.push({ kind: TokenKind.Ge, value: ">=", line: startLine, col: startCol }); continue; }
    if (two === "&&") { advance(); advance(); tokens.push({ kind: TokenKind.And, value: "&&", line: startLine, col: startCol }); continue; }
    if (two === "||") { advance(); advance(); tokens.push({ kind: TokenKind.Or, value: "||", line: startLine, col: startCol }); continue; }
    if (two === "+=") { advance(); advance(); tokens.push({ kind: TokenKind.PlusAssign, value: "+=", line: startLine, col: startCol }); continue; }
    if (two === "-=") { advance(); advance(); tokens.push({ kind: TokenKind.MinusAssign, value: "-=", line: startLine, col: startCol }); continue; }
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
      "+": TokenKind.Plus, "-": TokenKind.Minus,
      "*": TokenKind.Star, "/": TokenKind.Slash, "%": TokenKind.Percent,
      "=": TokenKind.Assign, "<": TokenKind.Lt, ">": TokenKind.Gt,
      "!": TokenKind.Not,
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
