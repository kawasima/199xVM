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

// ============================================================================
// Lexer
// ============================================================================

export enum TokenKind {
  // Literals
  IntLiteral = "IntLiteral",
  LongLiteral = "LongLiteral",
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
      const isLong = peek() === "L" || peek() === "l";
      if (isLong) raw += advance();
      tokens.push({ kind: isLong ? TokenKind.LongLiteral : TokenKind.IntLiteral, value: raw, line: startLine, col: startCol });
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

// ============================================================================
// AST
// ============================================================================

export type Type = "int" | "long" | "boolean" | "void" | "String" | { className: string } | { array: Type };

export interface ClassDecl {
  name: string;
  superClass: string;
  isRecord?: boolean;
  recordComponents?: ParamDecl[];
  fields: FieldDecl[];
  methods: MethodDecl[];
  importMap: Map<string, string>; // simpleName -> internal JVM name
  packageImports: string[]; // package names for import-on-demand (e.g. "java/util")
  staticWildcardImports: string[]; // owner class internal names for import static T.*
}

export interface FieldDecl {
  name: string;
  type: Type;
  isStatic: boolean;
  isPrivate?: boolean;
  isFinal?: boolean;
  initializer?: Expr;
}

export interface MethodDecl {
  name: string;
  returnType: Type;
  params: ParamDecl[];
  body: Stmt[];
  isStatic: boolean;
}

export interface ParamDecl {
  name: string;
  type: Type;
}

export type SwitchLabel =
  | { kind: "default" }
  | { kind: "null" }
  | { kind: "bool"; value: boolean }
  | { kind: "int"; value: number }
  | { kind: "string"; value: string }
  | { kind: "typePattern"; typeName: string; bindVar: string }
  | { kind: "recordPattern"; typeName: string; bindVars: string[] };

export interface SwitchCase {
  labels: SwitchLabel[];
  guard?: Expr;
  expr?: Expr;
  stmts?: Stmt[];
}

export type Stmt =
  | { kind: "varDecl"; name: string; type: Type; init?: Expr }
  | { kind: "assign"; target: Expr; value: Expr }
  | { kind: "exprStmt"; expr: Expr }
  | { kind: "return"; value?: Expr }
  | { kind: "yield"; value: Expr }
  | { kind: "if"; cond: Expr; then: Stmt[]; else_?: Stmt[] }
  | { kind: "while"; cond: Expr; body: Stmt[] }
  | { kind: "for"; init?: Stmt; cond?: Expr; update?: Stmt; body: Stmt[] }
  | { kind: "switch"; selector: Expr; cases: SwitchCase[] }
  | { kind: "block"; stmts: Stmt[] };

export type Expr =
  | { kind: "intLit"; value: number }
  | { kind: "longLit"; value: number }
  | { kind: "stringLit"; value: string }
  | { kind: "boolLit"; value: boolean }
  | { kind: "nullLit" }
  | { kind: "ident"; name: string }
  | { kind: "this" }
  | { kind: "binary"; op: string; left: Expr; right: Expr }
  | { kind: "unary"; op: string; operand: Expr }
  | { kind: "call"; object?: Expr; method: string; args: Expr[] }
  | { kind: "staticCall"; className: string; method: string; args: Expr[] }
  | { kind: "fieldAccess"; object: Expr; field: string }
  | { kind: "newExpr"; className: string; args: Expr[] }
  | { kind: "cast"; type: Type; expr: Expr }
  | { kind: "postIncrement"; operand: Expr; op: "++" | "--" }
  | { kind: "instanceof"; expr: Expr; checkType: string; bindVar?: string; recordBindVars?: string[] }
  | { kind: "staticField"; className: string; field: string }
  | { kind: "arrayAccess"; array: Expr; index: Expr }
  | { kind: "arrayLit"; elemType: Type; elements: Expr[] }
  | { kind: "newArray"; elemType: Type; size: Expr }
  | { kind: "superCall"; args: Expr[] }
  | { kind: "ternary"; cond: Expr; thenExpr: Expr; elseExpr: Expr }
  | { kind: "switchExpr"; selector: Expr; cases: SwitchCase[] }
  | { kind: "lambda"; params: string[]; bodyExpr?: Expr; bodyStmts?: Stmt[] }
  | { kind: "methodRef"; target: Expr; method: string; isConstructor: boolean };

// ============================================================================
// Parser
// ============================================================================

export function parseAll(tokens: Token[]): ClassDecl[] {
  let pos = 0;

  function peek(): Token { return tokens[pos] ?? tokens[tokens.length - 1]; }
  function advance(): Token { return tokens[pos++]; }
  function expect(kind: TokenKind): Token {
    const t = peek();
    if (t.kind !== kind) throw new Error(`Expected ${kind} but got ${t.kind} ("${t.value}") at line ${t.line}:${t.col}`);
    return advance();
  }
  function match(kind: TokenKind): boolean {
    if (peek().kind === kind) { advance(); return true; }
    return false;
  }
  function at(kind: TokenKind): boolean { return peek().kind === kind; }
  function parseIntLiteral(raw: string): number {
    let s = raw.replace(/_/g, "");
    if (s.endsWith("L") || s.endsWith("l")) s = s.slice(0, -1);
    if (/^0[xX][0-9a-fA-F]+$/.test(s)) return Number.parseInt(s.slice(2), 16);
    if (/^0[bB][01]+$/.test(s)) return Number.parseInt(s.slice(2), 2);
    if (/^0[0-7]+$/.test(s) && s.length > 1) return Number.parseInt(s.slice(1), 8);
    if (/^[0-9]+$/.test(s)) return Number.parseInt(s, 10);
    throw new Error(`Invalid integer literal: ${raw}`);
  }
  function parseQualifiedName(): string {
    let name = expect(TokenKind.Ident).value;
    while (at(TokenKind.Dot) && tokens[pos + 1]?.kind === TokenKind.Ident) {
      advance(); // dot
      name += "." + expect(TokenKind.Ident).value;
    }
    return name;
  }

  // Collect import/package statements
  // Build a map: simple name -> internal JVM name (e.g. "Ok" -> "net/unit8/raoh/Ok")
  const importMap = new Map<string, string>();
  const packageImports: string[] = ["java/lang"];
  const staticWildcardImports: string[] = [];
  while (at(TokenKind.KwImport) || at(TokenKind.KwPackage)) {
    const isImport = at(TokenKind.KwImport);
    advance(); // consume 'import' or 'package'
    if (isImport) {
      const isStaticImport = match(TokenKind.KwStatic);
      const base = parseQualifiedName();
      if (match(TokenKind.Dot)) {
        if (match(TokenKind.Star)) {
          if (isStaticImport) {
            staticWildcardImports.push(base.replace(/\./g, "/"));
          } else {
            const internalBase = base.replace(/\./g, "/");
            packageImports.push(internalBase);
            // Backward compatibility: if wildcard target looks like a type, allow unqualified static calls.
            if (/^[A-Z]/.test(base.split(".").pop() ?? "")) {
              staticWildcardImports.push(internalBase);
            }
          }
        } else {
          const member = expect(TokenKind.Ident).value;
          if (!isStaticImport) {
            const fqn = `${base}.${member}`;
            importMap.set(member, fqn.replace(/\./g, "/"));
          }
          // single static import is parsed but not resolved yet
        }
      } else if (!isStaticImport) {
        const simpleName = base.split(".").pop()!;
        importMap.set(simpleName, base.replace(/\./g, "/"));
      } else {
        const lastDot = base.lastIndexOf(".");
        if (lastDot < 0) throw new Error(`Invalid static import near "${base}"`);
        // single static import is parsed but not resolved yet
      }
    } else {
      // package — skip
      while (!at(TokenKind.Semi) && !at(TokenKind.EOF)) advance();
    }
    expect(TokenKind.Semi);
  }

  // Parse one or more class/record declarations
  const results: ClassDecl[] = [];
  while (!at(TokenKind.EOF)) {
    results.push(parseOneClass());
  }
  return results;

  function parseOneClass(): ClassDecl {
    // Skip modifiers before class/record
    while (at(TokenKind.KwPublic) || at(TokenKind.KwAbstract) || at(TokenKind.KwFinal)) advance();

    // record Foo(TypeA a, TypeB b) { ... }
    if (at(TokenKind.KwRecord)) {
      advance(); // consume 'record'
      const recordName = expect(TokenKind.Ident).value;
      // Parse record components: (TypeA a, TypeB b, ...)
      expect(TokenKind.LParen);
      const components: ParamDecl[] = [];
      if (!at(TokenKind.RParen)) {
        do {
          const cType = parseType();
          const cName = expect(TokenKind.Ident).value;
          components.push({ name: cName, type: cType });
        } while (match(TokenKind.Comma));
      }
      expect(TokenKind.RParen);
      // Skip implements clause if present
      if (match(TokenKind.KwImplements)) {
        while (!at(TokenKind.LBrace) && !at(TokenKind.EOF)) advance();
      }
      expect(TokenKind.LBrace);
      const recordFields: FieldDecl[] = [];
      const recordMethods: MethodDecl[] = [];
      // Parse any explicitly declared methods inside the record body
      while (!at(TokenKind.RBrace) && !at(TokenKind.EOF)) {
        parseMember(recordFields, recordMethods, recordName, true);
      }
      expect(TokenKind.RBrace);

      // Generate fields from components
      for (const c of components) {
        recordFields.push({ name: c.name, type: c.type, isStatic: false, isPrivate: true, isFinal: true });
      }

      // Generate canonical constructor if not already declared
      const hasInit = recordMethods.some(m => m.name === "<init>");
      if (!hasInit) {
        const initBody: Stmt[] = components.map(c => ({
          kind: "assign" as const,
          target: { kind: "fieldAccess" as const, object: { kind: "this" as const }, field: c.name },
          value: { kind: "ident" as const, name: c.name },
        }));
        recordMethods.push({
          name: "<init>",
          returnType: "void",
          params: components,
          body: initBody,
          isStatic: false,
        });
      }

      // Generate accessor methods for each component if not already declared
      for (const c of components) {
        const alreadyDeclared = recordMethods.some(m => m.name === c.name && m.params.length === 0);
        if (!alreadyDeclared) {
          recordMethods.push({
            name: c.name,
            returnType: c.type,
            params: [],
            body: [{ kind: "return" as const, value: { kind: "fieldAccess" as const, object: { kind: "this" as const }, field: c.name } }],
            isStatic: false,
          });
        }
      }
      // Basic Object method synthesis for records when not declared explicitly.
      // Full JLS semantics require component-wise implementations.
      if (!recordMethods.some(m => m.name === "equals" && m.params.length === 1)) {
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
              right: { kind: "ident", name: "other" },
            },
          }],
          isStatic: false,
        });
      }
      if (!recordMethods.some(m => m.name === "hashCode" && m.params.length === 0)) {
        recordMethods.push({
          name: "hashCode",
          returnType: "int",
          params: [],
          body: [{ kind: "return", value: { kind: "intLit", value: 0 } }],
          isStatic: false,
        });
      }
      if (!recordMethods.some(m => m.name === "toString" && m.params.length === 0)) {
        recordMethods.push({
          name: "toString",
          returnType: "String",
          params: [],
          body: [{ kind: "return", value: { kind: "stringLit", value: `${recordName}[]` } }],
          isStatic: false,
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
        staticWildcardImports,
      };
    }

    expect(TokenKind.KwClass);
    const className = expect(TokenKind.Ident).value;

    let superClass = "java/lang/Object";
    if (match(TokenKind.KwExtends)) {
      superClass = parseQualifiedName().replace(/\./g, "/");
    }
    // Skip implements
    if (match(TokenKind.KwImplements)) {
      parseQualifiedName();
      while (match(TokenKind.Comma)) parseQualifiedName();
    }

    expect(TokenKind.LBrace);

    const fields: FieldDecl[] = [];
    const methods: MethodDecl[] = [];

    while (!at(TokenKind.RBrace) && !at(TokenKind.EOF)) {
      parseMember(fields, methods, className, false);
    }
    expect(TokenKind.RBrace);

    return {
      name: className,
      superClass,
      isRecord: false,
      recordComponents: [],
      fields,
      methods,
      importMap,
      packageImports,
      staticWildcardImports,
    };
  }

  function parseMember(fields: FieldDecl[], methods: MethodDecl[], ownerName: string, inRecord: boolean) {
    let isStatic = false;

    // Consume modifiers
    while (true) {
      if (at(TokenKind.KwPublic) || at(TokenKind.KwPrivate) || at(TokenKind.KwProtected)) { advance(); continue; }
      if (at(TokenKind.KwStatic)) { advance(); isStatic = true; continue; }
      if (at(TokenKind.KwFinal) || at(TokenKind.KwAbstract)) { advance(); continue; }
      break;
    }

    // Constructor: modifiers followed by ClassName(...)
    // Detected by lookahead: current token is Ident and next is '('
    if (at(TokenKind.Ident) && tokens[pos + 1]?.kind === TokenKind.LParen && peek().value === ownerName) {
      advance(); // constructor name
      expect(TokenKind.LParen);
      const params: ParamDecl[] = [];
      if (!at(TokenKind.RParen)) {
        do {
          const pType = parseType();
          const pName = expect(TokenKind.Ident).value;
          params.push({ name: pName, type: pType });
        } while (match(TokenKind.Comma));
      }
      expect(TokenKind.RParen);
      expect(TokenKind.LBrace);
      const body = parseBlock();
      expect(TokenKind.RBrace);
      methods.push({ name: "<init>", returnType: "void", params, body, isStatic: false });
      return;
    }

    const retType = parseType();
    const name = expect(TokenKind.Ident).value;

    if (at(TokenKind.LParen)) {
      // Method
      expect(TokenKind.LParen);
      const params: ParamDecl[] = [];
      if (!at(TokenKind.RParen)) {
        do {
          const pType = parseType();
          const pName = expect(TokenKind.Ident).value;
          params.push({ name: pName, type: pType });
        } while (match(TokenKind.Comma));
      }
      expect(TokenKind.RParen);
      expect(TokenKind.LBrace);
      const body = parseBlock();
      expect(TokenKind.RBrace);
      methods.push({ name, returnType: retType, params, body, isStatic });
    } else {
      // Field
      let init: Expr | undefined;
      if (match(TokenKind.Assign)) {
        init = parseExpr();
      }
      expect(TokenKind.Semi);
      fields.push({ name, type: retType, isStatic, initializer: init, isPrivate: inRecord && !isStatic, isFinal: inRecord && !isStatic });
    }
  }

  function parseType(): Type {
    let base: Type;
    if (match(TokenKind.KwInt)) base = "int";
    else if (match(TokenKind.KwLong)) base = "long";
    else if (match(TokenKind.KwBoolean)) base = "boolean";
    else if (match(TokenKind.KwVoid)) base = "void";
    else if (match(TokenKind.KwString)) base = "String";
    else if (match(TokenKind.KwVar)) throw new Error(`'var' is only allowed for local variables with initializer`);
    else {
      const name = expect(TokenKind.Ident).value;
      // Skip generic type parameters like <String>
      if (at(TokenKind.Lt)) {
        advance();
        let depth = 1;
        while (depth > 0 && !at(TokenKind.EOF)) {
          if (at(TokenKind.Lt)) depth++;
          if (at(TokenKind.Gt)) depth--;
          advance();
        }
      }
      base = { className: name };
    }
    // Check for array suffix: Type[]
    if (at(TokenKind.LBracket) && tokens[pos + 1]?.kind === TokenKind.RBracket) {
      advance(); advance();
      return { array: base };
    }
    return base;
  }

  function parseBlock(): Stmt[] {
    const stmts: Stmt[] = [];
    while (!at(TokenKind.RBrace) && !at(TokenKind.EOF)) {
      stmts.push(parseStmt());
    }
    return stmts;
  }

  function parseSwitchLabel(): SwitchLabel {
    function parsePatternBindVar(): string {
      if (match(TokenKind.KwVar)) return expect(TokenKind.Ident).value;
      if ((at(TokenKind.KwInt) || at(TokenKind.KwBoolean) || at(TokenKind.KwString) || at(TokenKind.Ident))
          && tokens[pos + 1]?.kind === TokenKind.Ident) {
        advance(); // explicit component type
      }
      return expect(TokenKind.Ident).value;
    }
    function parseRecordPatternBindVars(): string[] {
      const bindVars: string[] = [];
      expect(TokenKind.LParen);
      if (!at(TokenKind.RParen)) {
        do { bindVars.push(parsePatternBindVar()); } while (match(TokenKind.Comma));
      }
      expect(TokenKind.RParen);
      return bindVars;
    }
    if (match(TokenKind.LParen)) {
      const nested = parseSwitchLabel();
      if (nested.kind !== "typePattern") {
        if (nested.kind !== "recordPattern") {
          throw new Error("parenthesized switch label currently supports only type/record patterns");
        }
      }
      expect(TokenKind.RParen);
      return nested;
    }
    if (at(TokenKind.NullLiteral)) {
      advance();
      return { kind: "null" };
    }
    if (at(TokenKind.BoolLiteral)) {
      return { kind: "bool", value: advance().value === "true" };
    }
    if (at(TokenKind.IntLiteral)) {
      return { kind: "int", value: parseIntLiteral(advance().value) };
    }
    if (at(TokenKind.StringLiteral)) {
      return { kind: "string", value: advance().value };
    }
    if (at(TokenKind.Ident)) {
      const typeName = parseQualifiedName();
      if (at(TokenKind.LParen)) {
        return { kind: "recordPattern", typeName, bindVars: parseRecordPatternBindVars() };
      }
      const bindVar = expect(TokenKind.Ident).value;
      return { kind: "typePattern", typeName, bindVar };
    }
    if (at(TokenKind.KwString)) {
      advance();
      const bindVar = expect(TokenKind.Ident).value;
      return { kind: "typePattern", typeName: "java/lang/String", bindVar };
    }
    throw new Error(`Unsupported switch label at line ${peek().line}:${peek().col}`);
  }

  function parseSwitchCases(isExpr: boolean): SwitchCase[] {
    const cases: SwitchCase[] = [];
    expect(TokenKind.LBrace);
    while (!at(TokenKind.RBrace) && !at(TokenKind.EOF)) {
      const labels: SwitchLabel[] = [];
      if (match(TokenKind.KwDefault)) {
        labels.push({ kind: "default" });
      } else {
        expect(TokenKind.KwCase);
        labels.push(parseSwitchLabel());
        while (match(TokenKind.Comma)) labels.push(parseSwitchLabel());
      }
      let guard: Expr | undefined;
      if (match(TokenKind.KwWhen)) {
        guard = parseExpr();
      }
      expect(TokenKind.Arrow);
      if (isExpr) {
        if (at(TokenKind.LBrace)) {
          expect(TokenKind.LBrace);
          const stmts: Stmt[] = [];
          while (!at(TokenKind.RBrace) && !at(TokenKind.EOF)) stmts.push(parseStmt());
          expect(TokenKind.RBrace);
          cases.push({ labels, guard, stmts });
        } else {
          const expr = parseExpr();
          expect(TokenKind.Semi);
          cases.push({ labels, guard, expr });
        }
      } else {
        if (at(TokenKind.LBrace)) {
          expect(TokenKind.LBrace);
          const stmts: Stmt[] = [];
          while (!at(TokenKind.RBrace) && !at(TokenKind.EOF)) stmts.push(parseStmt());
          expect(TokenKind.RBrace);
          cases.push({ labels, guard, stmts });
        } else {
          const stmt = parseStmt();
          cases.push({ labels, guard, stmts: [stmt] });
        }
      }
    }
    expect(TokenKind.RBrace);
    validateSwitchCases(cases, isExpr);
    return cases;
  }

  function validateSwitchCases(cases: SwitchCase[], isExpr: boolean): void {
    let defaultCount = 0;
    let nullCount = 0;
    let seenDefaultNoGuard = false;
    const seenConstLabels = new Set<string>();
    const seenUnguardedTypePatterns = new Set<string>();
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
        if (c.labels.length !== 1 || (c.labels[0].kind !== "typePattern" && c.labels[0].kind !== "recordPattern")) {
          throw new Error("switch guard 'when' is only supported with a single type pattern label");
        }
      }
      const unguardedPattern = c.labels.find(l => l.kind === "typePattern" || l.kind === "recordPattern");
      if (unguardedPattern && !c.guard) {
        seenUnguardedTypePatterns.add(unguardedPattern.typeName);
      }
      if (isExpr && !c.expr && !(c.stmts && c.stmts.some(s => s.kind === "yield"))) {
        throw new Error("switch expression case must provide value expression or yield");
      }
      if (seenDefaultNoGuard && !caseHasDefaultNoGuard) {
        throw new Error("switch has unreachable case after unguarded default");
      }
      if (caseHasDefaultNoGuard) seenDefaultNoGuard = true;
    }
  }

  function parseStmt(): Stmt {
    // Block
    if (at(TokenKind.LBrace)) {
      expect(TokenKind.LBrace);
      const stmts = parseBlock();
      expect(TokenKind.RBrace);
      return { kind: "block", stmts };
    }

    // Return
    if (at(TokenKind.KwReturn)) {
      advance();
      if (at(TokenKind.Semi)) { advance(); return { kind: "return" }; }
      const value = parseExpr();
      expect(TokenKind.Semi);
      return { kind: "return", value };
    }

    if (at(TokenKind.KwYield)) {
      advance();
      const value = parseExpr();
      expect(TokenKind.Semi);
      return { kind: "yield", value };
    }

    // If
    if (at(TokenKind.KwIf)) {
      advance();
      expect(TokenKind.LParen);
      const cond = parseExpr();
      expect(TokenKind.RParen);
      let then: Stmt[];
      if (at(TokenKind.LBrace)) {
        expect(TokenKind.LBrace);
        then = parseBlock();
        expect(TokenKind.RBrace);
      } else {
        then = [parseStmt()];
      }
      let else_: Stmt[] | undefined;
      if (match(TokenKind.KwElse)) {
        if (at(TokenKind.LBrace)) {
          expect(TokenKind.LBrace);
          else_ = parseBlock();
          expect(TokenKind.RBrace);
        } else {
          else_ = [parseStmt()];
        }
      }
      return { kind: "if", cond, then, else_ };
    }

    // While
    if (at(TokenKind.KwWhile)) {
      advance();
      expect(TokenKind.LParen);
      const cond = parseExpr();
      expect(TokenKind.RParen);
      expect(TokenKind.LBrace);
      const body = parseBlock();
      expect(TokenKind.RBrace);
      return { kind: "while", cond, body };
    }

    // For
    if (at(TokenKind.KwFor)) {
      advance();
      expect(TokenKind.LParen);
      let init: Stmt | undefined;
      if (!at(TokenKind.Semi)) init = parseStmtNoSemi();
      expect(TokenKind.Semi);
      let cond: Expr | undefined;
      if (!at(TokenKind.Semi)) cond = parseExpr();
      expect(TokenKind.Semi);
      let update: Stmt | undefined;
      if (!at(TokenKind.RParen)) update = parseStmtNoSemi();
      expect(TokenKind.RParen);
      expect(TokenKind.LBrace);
      const body = parseBlock();
      expect(TokenKind.RBrace);
      return { kind: "for", init, cond, update, body };
    }

    if (at(TokenKind.KwSwitch)) {
      advance();
      expect(TokenKind.LParen);
      const selector = parseExpr();
      expect(TokenKind.RParen);
      const cases = parseSwitchCases(false);
      return { kind: "switch", selector, cases };
    }

    // Variable declaration or expression statement
    // Check if it looks like a type followed by an identifier (var decl)
    if (at(TokenKind.KwVar)) {
      advance();
      const name = expect(TokenKind.Ident).value;
      expect(TokenKind.Assign);
      const init = parseExpr();
      expect(TokenKind.Semi);
      return { kind: "varDecl", name, type: inferLocalVarType(init), init };
    }
    if (isTypeStart() && isVarDecl()) {
      const type = parseType();
      const name = expect(TokenKind.Ident).value;
      let init: Expr | undefined;
      if (match(TokenKind.Assign)) init = parseExpr();
      expect(TokenKind.Semi);
      return { kind: "varDecl", name, type, init };
    }

    // Expression statement (may be assignment)
    const expr = parseExpr();
    if (match(TokenKind.Assign)) {
      const value = parseExpr();
      expect(TokenKind.Semi);
      return { kind: "assign", target: expr, value };
    }
    if (match(TokenKind.PlusAssign)) {
      const value = parseExpr();
      expect(TokenKind.Semi);
      return { kind: "assign", target: expr, value: { kind: "binary", op: "+", left: expr, right: value } };
    }
    if (match(TokenKind.MinusAssign)) {
      const value = parseExpr();
      expect(TokenKind.Semi);
      return { kind: "assign", target: expr, value: { kind: "binary", op: "-", left: expr, right: value } };
    }
    expect(TokenKind.Semi);
    return { kind: "exprStmt", expr };
  }

  function parseStmtNoSemi(): Stmt {
    // For init/update — similar to parseStmt but no semicolon
    if (at(TokenKind.KwVar)) {
      advance();
      const name = expect(TokenKind.Ident).value;
      expect(TokenKind.Assign);
      const init = parseExpr();
      return { kind: "varDecl", name, type: inferLocalVarType(init), init };
    }
    if (isTypeStart() && isVarDecl()) {
      const type = parseType();
      const name = expect(TokenKind.Ident).value;
      let init: Expr | undefined;
      if (match(TokenKind.Assign)) init = parseExpr();
      return { kind: "varDecl", name, type, init };
    }
    const expr = parseExpr();
    if (match(TokenKind.Assign)) {
      const value = parseExpr();
      return { kind: "assign", target: expr, value };
    }
    if (match(TokenKind.PlusAssign)) {
      const value = parseExpr();
      return { kind: "assign", target: expr, value: { kind: "binary", op: "+", left: expr, right: value } };
    }
    if (match(TokenKind.PlusPlus)) {
      return { kind: "assign", target: expr, value: { kind: "binary", op: "+", left: expr, right: { kind: "intLit", value: 1 } } };
    }
    if (match(TokenKind.MinusMinus)) {
      return { kind: "assign", target: expr, value: { kind: "binary", op: "-", left: expr, right: { kind: "intLit", value: 1 } } };
    }
    return { kind: "exprStmt", expr };
  }

  function isTypeStart(): boolean {
    const k = peek().kind;
    return k === TokenKind.KwInt || k === TokenKind.KwBoolean || k === TokenKind.KwVoid
      || k === TokenKind.KwString || k === TokenKind.Ident;
  }

  function isVarDecl(): boolean {
    // Lookahead: type name (= | ;)
    const saved = pos;
    try {
      // Skip type (including generic params)
      if (at(TokenKind.KwInt) || at(TokenKind.KwBoolean) || at(TokenKind.KwVoid) || at(TokenKind.KwString)) {
        advance();
        // Skip array suffix []
        if (at(TokenKind.LBracket) && tokens[pos + 1]?.kind === TokenKind.RBracket) { advance(); advance(); }
      } else if (at(TokenKind.Ident)) {
        advance();
        // Skip generics
        if (at(TokenKind.Lt)) {
          let depth = 1; advance();
          while (depth > 0 && !at(TokenKind.EOF)) {
            if (at(TokenKind.Lt)) depth++;
            if (at(TokenKind.Gt)) depth--;
            advance();
          }
        }
      } else {
        return false;
      }
      // If the "type" name was followed by '(' it's a method call, not a type
      if (at(TokenKind.LParen)) return false;
      // Skip array brackets
      if (at(TokenKind.LBracket) && tokens[pos + 1]?.kind === TokenKind.RBracket) {
        advance(); advance();
      }
      // Must be followed by an identifier (the variable name)
      if (!at(TokenKind.Ident)) return false;
      advance(); // skip variable name
      // After variable name must be '=', ';', or end of statement — not '('
      if (at(TokenKind.LParen)) return false;
      return true;
    } finally {
      pos = saved;
    }
  }

  // Expression parsing with precedence climbing
  function parseExpr(): Expr {
    if (isLambdaStart()) {
      return parseLambdaExpr();
    }
    const expr = parseOr();
    if (at(TokenKind.Question)) {
      advance(); // consume '?'
      const thenExpr = parseExpr();
      expect(TokenKind.Colon);
      const elseExpr = parseExpr();
      return { kind: "ternary", cond: expr, thenExpr, elseExpr };
    }
    return expr;
  }

  function isLambdaStart(): boolean {
    if (at(TokenKind.Ident) && tokens[pos + 1]?.kind === TokenKind.Arrow) return true;
    if (!at(TokenKind.LParen)) return false;
    let i = pos + 1;
    let expectIdent = true;
    while (i < tokens.length && tokens[i].kind !== TokenKind.RParen) {
      const k = tokens[i].kind;
      if (expectIdent) {
        if (k !== TokenKind.Ident) return false;
        expectIdent = false;
      } else {
        if (k !== TokenKind.Comma) return false;
        expectIdent = true;
      }
      i++;
    }
    if (i >= tokens.length || tokens[i].kind !== TokenKind.RParen) return false;
    return tokens[i + 1]?.kind === TokenKind.Arrow;
  }

  function parseLambdaExpr(): Expr {
    const params: string[] = [];
    if (at(TokenKind.Ident) && tokens[pos + 1]?.kind === TokenKind.Arrow) {
      params.push(advance().value);
      expect(TokenKind.Arrow);
    } else {
      expect(TokenKind.LParen);
      if (!at(TokenKind.RParen)) {
        do { params.push(expect(TokenKind.Ident).value); } while (match(TokenKind.Comma));
      }
      expect(TokenKind.RParen);
      expect(TokenKind.Arrow);
    }
    if (at(TokenKind.LBrace)) {
      expect(TokenKind.LBrace);
      const bodyStmts = parseBlock();
      expect(TokenKind.RBrace);
      return { kind: "lambda", params, bodyStmts };
    }
    const bodyExpr = parseExpr();
    return { kind: "lambda", params, bodyExpr };
  }

  function inferLocalVarType(init: Expr): Type {
    switch (init.kind) {
      case "intLit": return "int";
      case "longLit": return "long";
      case "boolLit": return "boolean";
      case "stringLit": return "String";
      case "newArray": return { array: init.elemType };
      case "arrayLit": return { array: init.elemType };
      case "newExpr": return { className: init.className };
      case "cast": return init.type;
      default: return { className: "java/lang/Object" };
    }
  }

  function parseOr(): Expr {
    let left = parseAnd();
    while (at(TokenKind.Or)) {
      advance();
      const right = parseAnd();
      left = { kind: "binary", op: "||", left, right };
    }
    return left;
  }

  function parseAnd(): Expr {
    let left = parseEquality();
    while (at(TokenKind.And)) {
      advance();
      const right = parseEquality();
      left = { kind: "binary", op: "&&", left, right };
    }
    return left;
  }

  function parseEquality(): Expr {
    let left = parseComparison();
    while (at(TokenKind.Eq) || at(TokenKind.Ne) || at(TokenKind.KwInstanceof)) {
      if (at(TokenKind.KwInstanceof)) {
        advance();
        function parsePatternBindVar(): string {
          if (match(TokenKind.KwVar)) return expect(TokenKind.Ident).value;
          if ((at(TokenKind.KwInt) || at(TokenKind.KwBoolean) || at(TokenKind.KwString) || at(TokenKind.Ident))
              && tokens[pos + 1]?.kind === TokenKind.Ident) {
            advance(); // explicit component type
          }
          return expect(TokenKind.Ident).value;
        }
        function parseInstanceofPattern(): { typeName: string; bindVar?: string; recordBindVars?: string[] } {
          if (match(TokenKind.LParen)) {
            const inner = parseInstanceofPattern();
            expect(TokenKind.RParen);
            return inner;
          }
          let typeName: string;
          if (at(TokenKind.KwString)) {
            advance();
            typeName = "java/lang/String";
          } else {
            typeName = parseQualifiedName();
          }
          // Skip generic params like <?>
          if (at(TokenKind.Lt)) {
            let depth = 1; advance();
            while (depth > 0 && !at(TokenKind.EOF)) {
              if (at(TokenKind.Lt)) depth++;
              if (at(TokenKind.Gt)) depth--;
              advance();
            }
          }
          if (at(TokenKind.LParen)) {
            const bindVars: string[] = [];
            advance();
            if (!at(TokenKind.RParen)) {
              do { bindVars.push(parsePatternBindVar()); } while (match(TokenKind.Comma));
            }
            expect(TokenKind.RParen);
            return { typeName, recordBindVars: bindVars };
          }
          let bindVar: string | undefined;
          if (at(TokenKind.Ident)) bindVar = advance().value;
          return { typeName, bindVar };
        }
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

  function parseComparison(): Expr {
    let left = parseAdditive();
    while (at(TokenKind.Lt) || at(TokenKind.Gt) || at(TokenKind.Le) || at(TokenKind.Ge)) {
      const op = advance().value;
      const right = parseAdditive();
      left = { kind: "binary", op, left, right };
    }
    return left;
  }

  function parseAdditive(): Expr {
    let left = parseMultiplicative();
    while (at(TokenKind.Plus) || at(TokenKind.Minus)) {
      const op = advance().value;
      const right = parseMultiplicative();
      left = { kind: "binary", op, left, right };
    }
    return left;
  }

  function parseMultiplicative(): Expr {
    let left = parseUnary();
    while (at(TokenKind.Star) || at(TokenKind.Slash) || at(TokenKind.Percent)) {
      const op = advance().value;
      const right = parseUnary();
      left = { kind: "binary", op, left, right };
    }
    return left;
  }

  function parseUnary(): Expr {
    if (at(TokenKind.Minus)) {
      advance();
      const operand = parseUnary();
      return { kind: "unary", op: "-", operand };
    }
    if (at(TokenKind.Not)) {
      advance();
      const operand = parseUnary();
      return { kind: "unary", op: "!", operand };
    }
    return parsePostfix();
  }

  function parsePostfix(): Expr {
    let expr = parsePrimary();

    while (true) {
      if (at(TokenKind.Dot)) {
        advance();
        const name = expect(TokenKind.Ident).value;
        if (at(TokenKind.LParen)) {
          // Method call
          expect(TokenKind.LParen);
          const args: Expr[] = [];
          if (!at(TokenKind.RParen)) {
            do { args.push(parseExpr()); } while (match(TokenKind.Comma));
          }
          expect(TokenKind.RParen);
          expr = { kind: "call", object: expr, method: name, args };
        } else {
          // Field access
          expr = { kind: "fieldAccess", object: expr, field: name };
        }
      } else if (at(TokenKind.LBracket)) {
        advance();
        const index = parseExpr();
        expect(TokenKind.RBracket);
        expr = { kind: "arrayAccess", array: expr, index };
      } else if (at(TokenKind.PlusPlus)) {
        advance();
        expr = { kind: "postIncrement", operand: expr, op: "++" };
      } else if (at(TokenKind.MinusMinus)) {
        advance();
        expr = { kind: "postIncrement", operand: expr, op: "--" };
      } else if (at(TokenKind.ColonColon)) {
        advance();
        if (match(TokenKind.KwNew)) {
          expr = { kind: "methodRef", target: expr, method: "<init>", isConstructor: true };
        } else {
          const method = expect(TokenKind.Ident).value;
          expr = { kind: "methodRef", target: expr, method, isConstructor: false };
        }
        break;
      } else {
        break;
      }
    }
    return expr;
  }

  function parsePrimary(): Expr {
    // Int literal
    if (at(TokenKind.IntLiteral)) {
      return { kind: "intLit", value: parseIntLiteral(advance().value) };
    }
    // Long literal
    if (at(TokenKind.LongLiteral)) {
      return { kind: "longLit", value: parseIntLiteral(advance().value) };
    }
    // String literal
    if (at(TokenKind.StringLiteral)) {
      return { kind: "stringLit", value: advance().value };
    }
    // Bool literal
    if (at(TokenKind.BoolLiteral)) {
      return { kind: "boolLit", value: advance().value === "true" };
    }
    // null
    if (at(TokenKind.NullLiteral)) {
      advance();
      return { kind: "nullLit" };
    }
    // this
    if (at(TokenKind.KwThis)) {
      advance();
      return { kind: "this" };
    }
    // String class literal-like reference in expressions (e.g., String::length)
    if (at(TokenKind.KwString)) {
      advance();
      return { kind: "ident", name: "String" };
    }
    // switch expression
    if (at(TokenKind.KwSwitch)) {
      advance();
      expect(TokenKind.LParen);
      const selector = parseExpr();
      expect(TokenKind.RParen);
      const cases = parseSwitchCases(true);
      return { kind: "switchExpr", selector, cases };
    }
    // super(args) — explicit superclass constructor call
    if (at(TokenKind.KwSuper)) {
      advance();
      expect(TokenKind.LParen);
      const args: Expr[] = [];
      if (!at(TokenKind.RParen)) {
        do { args.push(parseExpr()); } while (match(TokenKind.Comma));
      }
      expect(TokenKind.RParen);
      return { kind: "superCall", args };
    }
    // Array initializer: { expr, expr, ... }
    if (at(TokenKind.LBrace)) {
      advance();
      const elements: Expr[] = [];
      if (!at(TokenKind.RBrace)) {
        do { elements.push(parseExpr()); } while (match(TokenKind.Comma));
      }
      expect(TokenKind.RBrace);
      return { kind: "arrayLit", elemType: "int", elements };
    }
    // new
    if (at(TokenKind.KwNew)) {
      advance();
      // new int[n] or new SomeType[n]
      if (at(TokenKind.KwInt) || at(TokenKind.KwBoolean)) {
        const elemType: Type = at(TokenKind.KwInt) ? "int" : "boolean";
        advance();
        expect(TokenKind.LBracket);
        const size = parseExpr();
        expect(TokenKind.RBracket);
        return { kind: "newArray", elemType, size };
      }
      const cls = expect(TokenKind.Ident).value;
      // new ClassName[n]
      if (at(TokenKind.LBracket)) {
        advance();
        const size = parseExpr();
        expect(TokenKind.RBracket);
        return { kind: "newArray", elemType: { className: cls }, size };
      }
      // Skip generic params
      if (at(TokenKind.Lt)) {
        let depth = 1; advance();
        while (depth > 0 && !at(TokenKind.EOF)) {
          if (at(TokenKind.Lt)) depth++;
          if (at(TokenKind.Gt)) depth--;
          advance();
        }
      }
      expect(TokenKind.LParen);
      const args: Expr[] = [];
      if (!at(TokenKind.RParen)) {
        do { args.push(parseExpr()); } while (match(TokenKind.Comma));
      }
      expect(TokenKind.RParen);
      return { kind: "newExpr", className: cls, args };
    }
    // Parenthesized expression or cast: (Type) expr
    if (at(TokenKind.LParen)) {
      // Lookahead: is it (TypeName) followed by an expression? -> cast
      // Heuristic: (Ident) ident | (Ident) this | (Ident) new
      const savedPos = pos;
      advance(); // consume '('
      if (at(TokenKind.Ident) || at(TokenKind.KwString) || at(TokenKind.KwInt) || at(TokenKind.KwBoolean)) {
        // Try to read type name (with optional generics)
        let typeName = advance().value;
        // Skip generic params
        if (at(TokenKind.Lt)) {
          let depth = 1; advance();
          while (depth > 0 && !at(TokenKind.EOF)) {
            if (at(TokenKind.Lt)) depth++;
            if (at(TokenKind.Gt)) depth--;
            advance();
          }
        }
        if (at(TokenKind.RParen)) {
          advance(); // consume ')'
          // Check if this looks like a cast (next token starts an expression but not an operator)
          if (at(TokenKind.Ident) || at(TokenKind.KwThis) || at(TokenKind.KwNew) ||
              at(TokenKind.LParen) || at(TokenKind.IntLiteral) || at(TokenKind.StringLiteral) ||
              at(TokenKind.BoolLiteral) || at(TokenKind.NullLiteral)) {
            const castExpr = parseUnary();
            const castType: Type = typeName === "String" ? "String"
              : typeName === "int" ? "int"
              : typeName === "boolean" ? "boolean"
              : { className: typeName };
            return { kind: "cast", type: castType, expr: castExpr };
          }
        }
        // Not a cast — restore and fall through to parenthesized expr
        pos = savedPos;
        advance();
      }
      const expr = parseExpr();
      expect(TokenKind.RParen);
      return expr;
    }
    // Identifier (variable, class reference, or unqualified method call)
    if (at(TokenKind.Ident)) {
      const name = advance().value;
      // Unqualified method call: name(args...)
      if (at(TokenKind.LParen)) {
        expect(TokenKind.LParen);
        const args: Expr[] = [];
        if (!at(TokenKind.RParen)) {
          do { args.push(parseExpr()); } while (match(TokenKind.Comma));
        }
        expect(TokenKind.RParen);
        return { kind: "call", method: name, args };
      }
      return { kind: "ident", name };
    }

    throw new Error(`Unexpected token: ${peek().kind} ("${peek().value}") at line ${peek().line}:${peek().col}`);
  }
}

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
    else if (type === "int" || type === "boolean") { this.emit(0xac); this.adjustStack(-1); }
    else { this.emit(0xb0); this.adjustStack(-1); } // areturn
  }
}

// Type descriptor helpers
function typeToDescriptor(t: Type): string {
  if (t === "int") return "I";
  if (t === "long") return "J";
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
  return t !== "int" && t !== "long" && t !== "boolean" && t !== "void";
}

function isPrimitiveType(t: Type): boolean {
  return t === "int" || t === "long" || t === "boolean";
}

function sameType(a: Type, b: Type): boolean {
  if (a === b) return true;
  if (typeof a === "object" && typeof b === "object") {
    if ("className" in a && "className" in b) return a.className === b.className;
    if ("array" in a && "array" in b) return sameType(a.array, b.array);
  }
  return false;
}

function isAssignable(to: Type, from: Type): boolean {
  if (sameType(to, from)) return true;
  if (isRefType(to) && isRefType(from)) return true;
  if (to === "long" && from === "int") return true; // widening int→long
  if (to === "int" && from === "boolean") return false;
  if (to === "boolean" && from === "int") return false;
  return false;
}

function isKnownClass(ctx: CompileContext, cls: string): boolean {
  return cls === "java/lang/Object" || ctx.classSupers.has(cls) || !!BUILTIN_SUPERS[cls];
}

function isAssignableInContext(ctx: CompileContext, to: Type, from: Type): boolean {
  if (sameType(to, from)) return true;

  // Widening: int → long
  if (to === "long" && from === "int") return true;
  // Primitive assignments: exact type only in this subset.
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
  if (toPrim || fromPrim) {
    // In this subset, only identity primitive casts are supported.
    return false;
  }
  return true;
}

function mergeTernaryType(a: Type, b: Type): Type {
  if (sameType(a, b)) return a;
  if ((a === "int" && b === "boolean") || (a === "boolean" && b === "int")) return "int";
  if ((a === "long" && b === "int") || (a === "int" && b === "long")) return "long";
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
  // StringBuilder
  "java/lang/StringBuilder.<init>()": { owner: "java/lang/StringBuilder", returnType: "void", paramTypes: [] },
  "java/lang/StringBuilder.append(Ljava/lang/String;)": { owner: "java/lang/StringBuilder", returnType: { className: "java/lang/StringBuilder" }, paramTypes: ["String"] },
  "java/lang/StringBuilder.append(I)": { owner: "java/lang/StringBuilder", returnType: { className: "java/lang/StringBuilder" }, paramTypes: ["int"] },
  "java/lang/StringBuilder.append(J)": { owner: "java/lang/StringBuilder", returnType: { className: "java/lang/StringBuilder" }, paramTypes: ["long"] },
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
  // PrintStream
  "java/io/PrintStream.println(Ljava/lang/String;)": { owner: "java/io/PrintStream", returnType: "void", paramTypes: ["String"] },
  "java/io/PrintStream.println(I)": { owner: "java/io/PrintStream", returnType: "void", paramTypes: ["int"] },
  "java/io/PrintStream.println(Ljava/lang/Object;)": { owner: "java/io/PrintStream", returnType: "void", paramTypes: [{ className: "java/lang/Object" }] },
  "java/io/PrintStream.print(Ljava/lang/String;)": { owner: "java/io/PrintStream", returnType: "void", paramTypes: ["String"] },
  "java/io/PrintStream.print(I)": { owner: "java/io/PrintStream", returnType: "void", paramTypes: ["int"] },
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
}

interface FunctionalSig {
  samMethod: string;
  params: Type[];
  returnType: Type;
}

interface LambdaBootstrap {
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
        // Long promotion
        if (lt === "long" || rt === "long") return "long";
        return "int";
      }
      return "boolean"; // comparison operators
    }
    case "unary": return expr.op === "!" ? "boolean" : inferType(ctx, expr.operand) === "long" ? "long" : "int";
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
        const sig = lookupKnownMethod(ownerClass, expr.method, argDescs);
        if (sig) return sig.returnType;
        // Check user-defined methods
        const userMethod = ctx.allMethods.find(m => m.name === expr.method);
        if (userMethod) return userMethod.returnType;
      } else {
        // Unqualified call — look in user-defined methods
        const userMethod = ctx.allMethods.find(m => m.name === expr.method);
        if (userMethod) return userMethod.returnType;
        // Static import-on-demand method — return type unknown, assume Object
      }
      return { className: "java/lang/Object" };
    }
    case "staticCall": {
      const argDescs = expr.args.map(a => typeToDescriptor(inferType(ctx, a))).join("");
      const internalName = expr.className.replace(/\./g, "/");
      const sig = lookupKnownMethod(internalName, expr.method, argDescs);
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
  }
}

function compileExpr(ctx: CompileContext, emitter: BytecodeEmitter, expr: Expr, expectedType?: Type): void {
  switch (expr.kind) {
    case "intLit": {
      // If expected type is long, emit as long constant
      if (expectedType === "long") {
        emitter.emitLconst(expr.value, ctx.cp);
        break;
      }
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
        if (loc.type === "int" || loc.type === "boolean") emitter.emitIload(loc.slot);
        else emitter.emitAload(loc.slot);
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

      // Determine if long arithmetic is needed
      const isLongOp = leftType === "long" || rightType === "long";

      // Arithmetic/comparison type checks
      if (["+", "-", "*", "/", "%"].includes(expr.op)) {
        const validInt = leftType === "int" && rightType === "int";
        const validLong = isLongOp && (leftType === "int" || leftType === "long") && (rightType === "int" || rightType === "long");
        if (!validInt && !validLong) {
          throw new Error(`Operator '${expr.op}' requires int or long operands`);
        }
      }
      if (["<", ">", "<=", ">="].includes(expr.op)) {
        const validInt = leftType === "int" && rightType === "int";
        const validLong = isLongOp && (leftType === "int" || leftType === "long") && (rightType === "int" || rightType === "long");
        if (!validInt && !validLong) {
          throw new Error(`Operator '${expr.op}' requires int or long operands`);
        }
      }
      if (expr.op === "==" || expr.op === "!=") {
        const leftRef = isRefType(leftType);
        const rightRef = isRefType(rightType);
        if (leftRef !== rightRef) {
          throw new Error(`Operator '${expr.op}' requires operands of compatible categories`);
        }
        if (!leftRef && !rightRef && !sameType(leftType, rightType) && !isLongOp) {
          throw new Error(`Operator '${expr.op}' requires operands of the same primitive type`);
        }
      }

      // Emit operands with widening i2l if needed
      compileExpr(ctx, emitter, expr.left);
      if (isLongOp && leftType === "int") emitter.emit(0x85); // i2l
      compileExpr(ctx, emitter, expr.right);
      if (isLongOp && rightType === "int") emitter.emit(0x85); // i2l

      if (isLongOp) {
        switch (expr.op) {
          case "+": emitter.emit(0x61); break; // ladd
          case "-": emitter.emit(0x65); break; // lsub
          case "*": emitter.emit(0x69); break; // lmul
          case "/": emitter.emit(0x6d); break; // ldiv
          case "%": emitter.emit(0x71); break; // lrem
          case "==": case "!=": case "<": case ">": case "<=": case ">=": {
            emitter.emitPush(0x94); // lcmp: pops 2 longs, pushes 1 int; net with adjustStack(+1) from emit+push gives correct -1 after the two operands
            const jumpOp = {
              "==": 0x9a, // ifne
              "!=": 0x99, // ifeq
              "<": 0x9c,  // ifge
              ">": 0x9e,  // ifle
              "<=": 0x9d, // ifgt
              ">=": 0x9b, // iflt
            }[expr.op]!;
            const patchFalse = emitter.emitBranch(jumpOp);
            emitter.emitIconst(1);
            const patchEnd = emitter.emitBranch(0xa7); // goto
            emitter.patchBranch(patchFalse, emitter.pc);
            emitter.emitIconst(0);
            emitter.patchBranch(patchEnd, emitter.pc);
            break;
          }
          default:
            throw new Error(`Unsupported binary operator for long: ${expr.op}`);
        }
      } else {
        switch (expr.op) {
          case "+": emitter.emit(0x60); break; // iadd
          case "-": emitter.emit(0x64); break; // isub
          case "*": emitter.emit(0x68); break; // imul
          case "/": emitter.emit(0x6c); break; // idiv
          case "%": emitter.emit(0x70); break; // irem

          // Comparisons — produce 0 or 1
          case "==": case "!=": case "<": case ">": case "<=": case ">=": {
            const refCompare = expr.op === "==" || expr.op === "!="
              ? (isRefType(leftType) || isRefType(rightType))
              : false;
            const jumpOp = refCompare
              ? (expr.op === "==" ? 0xa6 : 0xa5) // if_acmpne / if_acmpeq
              : {
                  "==": 0xa0, // if_icmpne
                  "!=": 0x9f, // if_icmpeq
                  "<": 0xa2,  // if_icmpge
                  ">": 0xa4,  // if_icmple
                  "<=": 0xa3, // if_icmpgt
                  ">=": 0xa1, // if_icmplt
                }[expr.op]!;
            const patchFalse = emitter.emitBranch(jumpOp);
            emitter.emitIconst(1);
            const patchEnd = emitter.emitBranch(0xa7); // goto
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
        if (operandType === "long") {
          emitter.emit(0x75); // lneg
        } else if (operandType === "int") {
          emitter.emit(0x74); // ineg
        } else {
          throw new Error("Unary '-' requires int or long operand");
        }
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
    case "cast": {
      const srcType = inferType(ctx, expr.expr);
      if (!isCastConvertible(expr.type, srcType)) {
        throw new Error(`Invalid cast from ${typeToDescriptor(srcType)} to ${typeToDescriptor(expr.type)}`);
      }
      compileExpr(ctx, emitter, expr.expr);
      if (isRefType(expr.type)) {
        const castClass = typeof expr.type === "object" && "className" in expr.type
          ? resolveClassName(ctx, expr.type.className)
          : "java/lang/Object";
        const classIdx = ctx.cp.addClass(castClass);
        emitter.emit(0xc0); // checkcast
        emitter.emitU16(classIdx);
      }
      // For primitive casts (int, boolean) — no bytecode needed for same-size types
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
      ctx.lambdaBootstraps.push({
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
        ctx.lambdaBootstraps.push({
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
      ctx.lambdaBootstraps.push({
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
    if (partType === "int") {
      appendDesc = "(I)Ljava/lang/StringBuilder;";
    } else if (partType === "long") {
      appendDesc = "(J)Ljava/lang/StringBuilder;";
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
    if (argType === "int") desc = "(I)V";
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
        const sig = lookupKnownMethod(internalName, expr.method, argTypes.join(""));
        if (sig) {
          expr.args.forEach((arg, i) => compileExpr(ctx, emitter, arg, sig.paramTypes[i]));
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
    const sig = lookupKnownMethod(ownerClass, expr.method, argTypes.join(""));

    let desc: string;
    let retType: Type;
    let isInterface = false;

    if (sig) {
      expr.args.forEach((arg, i) => compileExpr(ctx, emitter, arg, sig.paramTypes[i]));
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
      for (const arg of expr.args) compileExpr(ctx, emitter, arg);
      const ownerClass = ctx.staticWildcardImports.find(owner => !!lookupKnownMethod(owner, expr.method, argTypes.join("")))
        ?? ctx.staticWildcardImports[0];
      const retType: Type = { className: "java/lang/Object" };
      const desc = "(" + argTypes.join("") + ")" + typeToDescriptor(retType);
      const mRef = ctx.cp.addMethodref(ownerClass, expr.method, desc);
      emitter.emitInvokestatic(mRef, expr.args.length, true);
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
    case "block":
      for (const s of stmt.stmts) collectStmtIdentifiers(s, out);
      break;
  }
}

function descriptorToType(desc: string): Type {
  if (desc === "I") return "int";
  if (desc === "J") return "long";
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

function emitStoreLocalByType(emitter: BytecodeEmitter, slot: number, t: Type): void {
  if (t === "long") emitter.emitLstore(slot);
  else if (t === "int" || t === "boolean") emitter.emitIstore(slot);
  else emitter.emitAstore(slot);
}

function emitLoadLocalByType(emitter: BytecodeEmitter, slot: number, t: Type): void {
  if (t === "long") emitter.emitLload(slot);
  else if (t === "int" || t === "boolean") emitter.emitIload(slot);
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
        if (stmt.type === "long" && initType === "int") emitter.emit(0x85); // i2l
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
          if (loc.type === "long" && valType === "int") emitter.emit(0x85); // i2l
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
        ensureAssignable(ctx, ctx.method.returnType, inferType(ctx, stmt.value), `return in ${ctx.method.name}`);
        compileExpr(ctx, emitter, stmt.value, ctx.method.returnType);
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
      const loopStart = emitter.pc;
      compileExpr(ctx, emitter, stmt.cond);
      const patchExit = emitter.emitBranch(0x99); // ifeq
      withScopedLocals(ctx, () => {
        for (const s of stmt.body) compileStmt(ctx, emitter, s);
      });
      // goto loopStart
      const gotoOp = emitter.emitBranch(0xa7);
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
          patchExit = emitter.emitBranch(0x99); // ifeq
        }
        withScopedLocals(ctx, () => {
          for (const s of stmt.body) compileStmt(ctx, emitter, s);
        });
        if (stmt.update) compileStmt(ctx, emitter, stmt.update);
        const gotoOp = emitter.emitBranch(0xa7);
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
): { code: number[]; maxStack: number; maxLocals: number } {
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

  return { code: emitter.code, maxStack: Math.max(emitter.maxStack, 4), maxLocals: emitter.maxLocals };
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
export function compile(source: string): Uint8Array {
  const tokens = lex(source);
  const classDecls = parseAll(tokens);
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
  const serializedBootstrapMethods: { methodRef: number; args: number[] }[] = [];
  for (const lb of lambdaBootstraps) {
    const metafactoryRef = cp.addMethodref("java/lang/invoke/LambdaMetafactory", "metafactory", "()V");
    const bootstrapMethodRef = cp.addMethodHandle(6, metafactoryRef);
    const implMethodRef = lb.implIsInterface
      ? cp.addInterfaceMethodref(lb.implOwner, lb.implMethodName, lb.implDescriptor)
      : cp.addMethodref(lb.implOwner, lb.implMethodName, lb.implDescriptor);
    const implHandle = cp.addMethodHandle(lb.implRefKind, implMethodRef);
    const samType = cp.addMethodType(lb.implDescriptor);
    serializedBootstrapMethods.push({ methodRef: bootstrapMethodRef, args: [samType, implHandle] });
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
    const attrLen = 2 + 2 + 4 + codeLen + 2 + 2; // max_stack + max_locals + code_length + code + exception_table_length + attributes_count
    out.push((attrLen >> 24) & 0xff, (attrLen >> 16) & 0xff, (attrLen >> 8) & 0xff, attrLen & 0xff);
    out.push((m.maxStack >> 8) & 0xff, m.maxStack & 0xff);
    out.push((m.maxLocals >> 8) & 0xff, m.maxLocals & 0xff);
    out.push((codeLen >> 24) & 0xff, (codeLen >> 16) & 0xff, (codeLen >> 8) & 0xff, codeLen & 0xff);
    out.push(...m.code);
    out.push(0x00, 0x00); // exception_table_length = 0
    out.push(0x00, 0x00); // attributes_count = 0
  }

  // class attributes
  const classAttrCount = serializedBootstrapMethods.length > 0 ? 1 : 0;
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
