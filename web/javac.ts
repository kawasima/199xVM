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
  KwBoolean = "boolean",
  KwString = "String",
  KwReturn = "return",
  KwNew = "new",
  KwIf = "if",
  KwElse = "else",
  KwWhile = "while",
  KwFor = "for",
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

  // Special
  EOF = "EOF",
}

export interface Token {
  kind: TokenKind;
  value: string;
  line: number;
  col: number;
}

const KEYWORDS: Record<string, TokenKind> = {
  class: TokenKind.KwClass,
  public: TokenKind.KwPublic,
  static: TokenKind.KwStatic,
  void: TokenKind.KwVoid,
  int: TokenKind.KwInt,
  boolean: TokenKind.KwBoolean,
  String: TokenKind.KwString,
  return: TokenKind.KwReturn,
  new: TokenKind.KwNew,
  if: TokenKind.KwIf,
  else: TokenKind.KwElse,
  while: TokenKind.KwWhile,
  for: TokenKind.KwFor,
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
      advance(); advance();
      while (pos + 1 < source.length && !(peek() === "*" && source[pos + 1] === "/")) advance();
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
      advance(); // closing "
      tokens.push({ kind: TokenKind.StringLiteral, value: s, line: startLine, col: startCol });
      continue;
    }

    // Number literal
    if (/[0-9]/.test(ch)) {
      let num = "";
      while (/[0-9]/.test(peek())) num += advance();
      // Skip long suffix
      if (peek() === "L" || peek() === "l") advance();
      tokens.push({ kind: TokenKind.IntLiteral, value: num, line: startLine, col: startCol });
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

    // Skip unknown
    advance();
  }

  tokens.push({ kind: TokenKind.EOF, value: "", line, col });
  return tokens;
}

// ============================================================================
// AST
// ============================================================================

export type Type = "int" | "boolean" | "void" | "String" | { className: string } | { array: Type };

export interface ClassDecl {
  name: string;
  superClass: string;
  fields: FieldDecl[];
  methods: MethodDecl[];
  importMap: Map<string, string>; // simpleName -> internal JVM name
  wildcardImports: string[]; // internal JVM names of wildcard-imported classes (e.g. "net/unit8/raoh/ObjectDecoders")
}

export interface FieldDecl {
  name: string;
  type: Type;
  isStatic: boolean;
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

export type Stmt =
  | { kind: "varDecl"; name: string; type: Type; init?: Expr }
  | { kind: "assign"; target: Expr; value: Expr }
  | { kind: "exprStmt"; expr: Expr }
  | { kind: "return"; value?: Expr }
  | { kind: "if"; cond: Expr; then: Stmt[]; else_?: Stmt[] }
  | { kind: "while"; cond: Expr; body: Stmt[] }
  | { kind: "for"; init?: Stmt; cond?: Expr; update?: Stmt; body: Stmt[] }
  | { kind: "block"; stmts: Stmt[] };

export type Expr =
  | { kind: "intLit"; value: number }
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
  | { kind: "instanceof"; expr: Expr; checkType: string; bindVar?: string }
  | { kind: "staticField"; className: string; field: string }
  | { kind: "arrayAccess"; array: Expr; index: Expr }
  | { kind: "arrayLit"; elemType: Type; elements: Expr[] }
  | { kind: "newArray"; elemType: Type; size: Expr }
  | { kind: "superCall"; args: Expr[] }
  | { kind: "ternary"; cond: Expr; thenExpr: Expr; elseExpr: Expr };

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

  // Collect import/package statements
  // Build a map: simple name -> internal JVM name (e.g. "Ok" -> "net/unit8/raoh/Ok")
  const importMap = new Map<string, string>();
  const wildcardImports: string[] = [];
  while (at(TokenKind.KwImport) || at(TokenKind.KwPackage)) {
    const isImport = at(TokenKind.KwImport);
    advance(); // consume 'import' or 'package'
    if (isImport) {
      // Skip optional 'static' keyword (import static ...)
      if (at(TokenKind.KwStatic)) advance();
      // Collect the dotted name until semicolon
      let fqn = "";
      while (!at(TokenKind.Semi) && !at(TokenKind.EOF)) {
        fqn += peek().value;
        advance();
      }
      // fqn e.g. "net.unit8.raoh.Ok"  or "net.unit8.raoh.*"
      if (fqn.endsWith(".*")) {
        // Wildcard import: record the class before ".*"
        const classPath = fqn.slice(0, -2).replace(/\./g, "/");
        wildcardImports.push(classPath);
      } else {
        const parts = fqn.split(".");
        const simpleName = parts[parts.length - 1];
        const internalName = fqn.replace(/\./g, "/");
        importMap.set(simpleName, internalName);
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
        parseMember(recordFields, recordMethods);
      }
      expect(TokenKind.RBrace);

      // Generate fields from components
      for (const c of components) {
        recordFields.push({ name: c.name, type: c.type, isStatic: false });
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

      return { name: recordName, superClass: "java/lang/Record", fields: recordFields, methods: recordMethods, importMap, wildcardImports };
    }

    expect(TokenKind.KwClass);
    const className = expect(TokenKind.Ident).value;

    let superClass = "java/lang/Object";
    if (match(TokenKind.KwExtends)) {
      superClass = expect(TokenKind.Ident).value;
      superClass = superClass.replace(/\./g, "/");
    }
    // Skip implements
    if (match(TokenKind.KwImplements)) {
      expect(TokenKind.Ident);
      while (match(TokenKind.Comma)) expect(TokenKind.Ident);
    }

    expect(TokenKind.LBrace);

    const fields: FieldDecl[] = [];
    const methods: MethodDecl[] = [];

    while (!at(TokenKind.RBrace) && !at(TokenKind.EOF)) {
      parseMember(fields, methods);
    }
    expect(TokenKind.RBrace);

    return { name: className, superClass, fields, methods, importMap, wildcardImports };
  }

  function parseMember(fields: FieldDecl[], methods: MethodDecl[]) {
    let isStatic = false;
    let isPublic = false;

    // Consume modifiers
    while (true) {
      if (at(TokenKind.KwPublic) || at(TokenKind.KwPrivate) || at(TokenKind.KwProtected)) { advance(); isPublic = true; continue; }
      if (at(TokenKind.KwStatic)) { advance(); isStatic = true; continue; }
      if (at(TokenKind.KwFinal) || at(TokenKind.KwAbstract)) { advance(); continue; }
      break;
    }

    // Constructor: modifiers followed by ClassName(...)
    // Detected by lookahead: current token is Ident and next is '('
    if (at(TokenKind.Ident) && tokens[pos + 1]?.kind === TokenKind.LParen) {
      advance(); // constructor name (same as class name, not needed)
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
      fields.push({ name, type: retType, isStatic, initializer: init });
    }
  }

  function parseType(): Type {
    let base: Type;
    if (match(TokenKind.KwInt)) base = "int";
    else if (match(TokenKind.KwBoolean)) base = "boolean";
    else if (match(TokenKind.KwVoid)) base = "void";
    else if (match(TokenKind.KwString)) base = "String";
    else if (match(TokenKind.KwVar)) base = { className: "java/lang/Object" };
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

    // Variable declaration or expression statement
    // Check if it looks like a type followed by an identifier (var decl)
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
      || k === TokenKind.KwString || k === TokenKind.KwVar || k === TokenKind.Ident;
  }

  function isVarDecl(): boolean {
    // Lookahead: type name (= | ;)
    const saved = pos;
    try {
      // Skip type (including generic params)
      if (at(TokenKind.KwInt) || at(TokenKind.KwBoolean) || at(TokenKind.KwVoid) || at(TokenKind.KwString) || at(TokenKind.KwVar)) {
        const wasVar = at(TokenKind.KwVar);
        advance();
        // Skip array suffix []
        if (at(TokenKind.LBracket) && tokens[pos + 1]?.kind === TokenKind.RBracket) { advance(); advance(); }
        // var is always followed by a variable name
        if (wasVar) return at(TokenKind.Ident);
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
        // Parse type (possibly with generic wildcard)
        const typeName = expect(TokenKind.Ident).value;
        // Skip generic params like <?>
        if (at(TokenKind.Lt)) {
          let depth = 1; advance();
          while (depth > 0 && !at(TokenKind.EOF)) {
            if (at(TokenKind.Lt)) depth++;
            if (at(TokenKind.Gt)) depth--;
            advance();
          }
        }
        // Optional pattern variable: instanceof Ok<?> ok
        let bindVar: string | undefined;
        if (at(TokenKind.Ident)) {
          bindVar = advance().value;
        }
        left = { kind: "instanceof", expr: left, checkType: typeName, bindVar };
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
      } else {
        break;
      }
    }
    return expr;
  }

  function parsePrimary(): Expr {
    // Int literal
    if (at(TokenKind.IntLiteral)) {
      return { kind: "intLit", value: parseInt(advance().value, 10) };
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
              at(TokenKind.LParen) || at(TokenKind.IntLiteral) || at(TokenKind.StringLiteral)) {
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
    else if (type === "int" || type === "boolean") { this.emit(0xac); this.adjustStack(-1); }
    else { this.emit(0xb0); this.adjustStack(-1); } // areturn
  }
}

// Type descriptor helpers
function typeToDescriptor(t: Type): string {
  if (t === "int") return "I";
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
  return t !== "int" && t !== "boolean" && t !== "void";
}

// Known class type mappings for method return types
interface MethodSig {
  owner: string;
  returnType: Type;
  paramTypes: Type[];
  isInterface?: boolean;
}

const KNOWN_METHODS: Record<string, MethodSig> = {
  // Integer
  "java/lang/Integer.valueOf(I)": { owner: "java/lang/Integer", returnType: { className: "java/lang/Integer" }, paramTypes: ["int"] },
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
  // StringBuilder
  "java/lang/StringBuilder.<init>()": { owner: "java/lang/StringBuilder", returnType: "void", paramTypes: [] },
  "java/lang/StringBuilder.append(Ljava/lang/String;)": { owner: "java/lang/StringBuilder", returnType: { className: "java/lang/StringBuilder" }, paramTypes: ["String"] },
  "java/lang/StringBuilder.append(I)": { owner: "java/lang/StringBuilder", returnType: { className: "java/lang/StringBuilder" }, paramTypes: ["int"] },
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
  // PrintStream
  "java/io/PrintStream.println(Ljava/lang/String;)": { owner: "java/io/PrintStream", returnType: "void", paramTypes: ["String"] },
  "java/io/PrintStream.println(I)": { owner: "java/io/PrintStream", returnType: "void", paramTypes: ["int"] },
  "java/io/PrintStream.println(Ljava/lang/Object;)": { owner: "java/io/PrintStream", returnType: "void", paramTypes: [{ className: "java/lang/Object" }] },
  "java/io/PrintStream.print(Ljava/lang/String;)": { owner: "java/io/PrintStream", returnType: "void", paramTypes: ["String"] },
  "java/io/PrintStream.print(I)": { owner: "java/io/PrintStream", returnType: "void", paramTypes: ["int"] },
};

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
  wildcardImports: string[];
}

/** Look up a method in KNOWN_METHODS, falling back to name-only match if exact arg types don't match. */
function lookupKnownMethod(owner: string, method: string, argDescs: string): MethodSig | undefined {
  const exact = KNOWN_METHODS[`${owner}.${method}(${argDescs})`];
  if (exact) return exact;
  // Fallback: find by owner.method prefix (handles e.g. String arg passed to Object param)
  const prefix = `${owner}.${method}(`;
  for (const key of Object.keys(KNOWN_METHODS)) {
    if (key.startsWith(prefix)) return KNOWN_METHODS[key];
  }
  return undefined;
}

/** Resolve a simple class name to its internal JVM name using the import map. */
function resolveClassName(ctx: CompileContext, name: string): string {
  // Already internal (contains '/') or fully qualified (contains '.')
  if (name.includes("/")) return name;
  if (name.includes(".")) return name.replace(/\./g, "/");
  return ctx.importMap.get(name) ?? name;
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
        return "int";
      }
      return "boolean"; // comparison operators
    }
    case "unary": return expr.op === "!" ? "boolean" : "int";
    case "newExpr": return { className: resolveClassName(ctx, expr.className) };
    case "call": {
      if (expr.object) {
        const objType = inferType(ctx, expr.object);
        const rawOwner = objType === "String" ? "java/lang/String"
          : typeof objType === "object" && "className" in objType ? objType.className
          : "java/lang/Object";
        const ownerClass = resolveClassName(ctx, rawOwner);
        // Look in KNOWN_METHODS
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
        // Wildcard-imported static method — return type unknown, assume Object
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
    case "postIncrement": return "int";
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
    case "ternary": return inferType(ctx, expr.thenExpr);
  }
}

function compileExpr(ctx: CompileContext, emitter: BytecodeEmitter, expr: Expr): void {
  switch (expr.kind) {
    case "intLit": {
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

      // Integer arithmetic
      compileExpr(ctx, emitter, expr.left);
      compileExpr(ctx, emitter, expr.right);

      switch (expr.op) {
        case "+": emitter.emit(0x60); break; // iadd
        case "-": emitter.emit(0x64); break; // isub
        case "*": emitter.emit(0x68); break; // imul
        case "/": emitter.emit(0x6c); break; // idiv
        case "%": emitter.emit(0x70); break; // irem

        // Comparisons — produce 0 or 1
        case "==": case "!=": case "<": case ">": case "<=": case ">=": {
          const jumpOp = {
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
        case "&&": case "||": break; // handled separately below
      }

      // Logical operators (short-circuit)
      if (expr.op === "&&") {
        // Redo: compile with short-circuit
        // Pop the two values we already pushed
        emitter.code.length -= emitter.code.length - emitter.pc; // nope, too late
        // Actually, let's handle && and || at the top before compiling both sides
      }
      break;
    }
    case "unary": {
      compileExpr(ctx, emitter, expr.operand);
      if (expr.op === "-") emitter.emit(0x74); // ineg
      if (expr.op === "!") {
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
      compileExpr(ctx, emitter, expr.cond);
      const patchElse = emitter.emitBranch(0x99); // ifeq — jump to else if cond == 0
      compileExpr(ctx, emitter, expr.thenExpr);
      const patchEnd = emitter.emitBranch(0xa7); // goto — skip else
      emitter.patchBranch(patchElse, emitter.pc);
      compileExpr(ctx, emitter, expr.elseExpr);
      emitter.patchBranch(patchEnd, emitter.pc);
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
        for (const arg of expr.args) compileExpr(ctx, emitter, arg);

        // Try known methods
        const sig = lookupKnownMethod(internalName, expr.method, argTypes.join(""));
        if (sig) {
          const sigArgDescs = sig.paramTypes.map(t => typeToDescriptor(t)).join("");
          const desc = "(" + sigArgDescs + ")" + typeToDescriptor(sig.returnType);
          const mRef = ctx.cp.addMethodref(internalName, expr.method, desc);
          emitter.emitInvokestatic(mRef, expr.args.length, sig.returnType !== "void");
        } else {
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
    for (const arg of expr.args) compileExpr(ctx, emitter, arg);

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
      retType = sig.returnType;
      const sigArgDescs = sig.paramTypes.map(t => typeToDescriptor(t)).join("");
      desc = "(" + sigArgDescs + ")" + typeToDescriptor(retType);
      isInterface = sig.isInterface ?? false;
    } else {
      // Check user-defined methods
      const userMethod = ctx.allMethods.find(m => m.name === expr.method && !m.isStatic);
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
      const argTypes = expr.args.map(a => typeToDescriptor(inferType(ctx, a)));
      for (const arg of expr.args) compileExpr(ctx, emitter, arg);
      const desc = methodDescriptor(userMethod.params, userMethod.returnType);
      const mRef = ctx.cp.addMethodref(ctx.className, expr.method, desc);
      if (userMethod.isStatic) {
        emitter.emitInvokestatic(mRef, expr.args.length, userMethod.returnType !== "void");
      } else {
        emitter.emitAload(0); // this
        emitter.emitInvokevirtual(mRef, expr.args.length, userMethod.returnType !== "void");
      }
    } else if (ctx.wildcardImports.length > 0) {
      // Try each wildcard-imported class as a static call target
      const argTypes = expr.args.map(a => typeToDescriptor(inferType(ctx, a)));
      for (const arg of expr.args) compileExpr(ctx, emitter, arg);
      // Use the first wildcard import (most specific import wins)
      const ownerClass = ctx.wildcardImports[0];
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
      const fieldRef = ctx.cp.addFieldref(resolved, expr.field, "Ljava/lang/Object;");
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
      const fieldRef = ctx.cp.addFieldref(ownerClass, expr.field, "Ljava/lang/Object;");
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
        compileExpr(ctx, emitter, init);
        if (stmt.type === "int" || stmt.type === "boolean") emitter.emitIstore(slot);
        else emitter.emitAstore(slot);
      }
      break;
    }
    case "assign": {
      if (stmt.target.kind === "ident") {
        const loc = findLocal(ctx, stmt.target.name);
        if (loc) {
          compileExpr(ctx, emitter, stmt.value);
          if (loc.type === "int" || loc.type === "boolean") emitter.emitIstore(loc.slot);
          else emitter.emitAstore(loc.slot);
        } else {
          // Field assignment
          const field = ctx.fields.find(f => f.name === stmt.target.name);
          if (field) {
            if (field.isStatic) {
              compileExpr(ctx, emitter, stmt.value);
              const fRef = ctx.cp.addFieldref(ctx.className, field.name, typeToDescriptor(field.type));
              emitter.emit(0xb3); // putstatic
              emitter.emitU16(fRef);
            } else {
              emitter.emitAload(0); // this
              compileExpr(ctx, emitter, stmt.value);
              const fRef = ctx.cp.addFieldref(ctx.className, field.name, typeToDescriptor(field.type));
              emitter.emit(0xb5); // putfield
              emitter.emitU16(fRef);
            }
          }
        }
      } else if (stmt.target.kind === "fieldAccess") {
        compileExpr(ctx, emitter, stmt.target.object);
        compileExpr(ctx, emitter, stmt.value);
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
        compileExpr(ctx, emitter, stmt.value);
        const elemType = inferType(ctx, stmt.target);
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
        compileExpr(ctx, emitter, stmt.value);
      }
      emitter.emitReturn(ctx.method.returnType);
      break;
    }
    case "if": {
      compileExpr(ctx, emitter, stmt.cond);
      const patchElse = emitter.emitBranch(0x99); // ifeq (jump if false)
      // If condition is instanceof with a pattern variable, bind it at the start of then-branch
      if (stmt.cond.kind === "instanceof" && stmt.cond.bindVar) {
        const bindVar = stmt.cond.bindVar;
        const checkClass = resolveClassName(ctx, stmt.cond.checkType);
        // Re-load the source expression and cast it to the pattern type
        compileExpr(ctx, emitter, stmt.cond.expr);
        const classIdx = ctx.cp.addClass(checkClass);
        emitter.emit(0xc0); emitter.emitU16(classIdx); // checkcast
        const slot = addLocal(ctx, bindVar, { className: checkClass });
        if (emitter.maxLocals <= slot) emitter.maxLocals = slot + 1;
        emitter.emitAstore(slot);
      }
      for (const s of stmt.then) compileStmt(ctx, emitter, s);
      if (stmt.else_) {
        const patchEnd = emitter.emitBranch(0xa7); // goto
        emitter.patchBranch(patchElse, emitter.pc);
        for (const s of stmt.else_) compileStmt(ctx, emitter, s);
        emitter.patchBranch(patchEnd, emitter.pc);
      } else {
        emitter.patchBranch(patchElse, emitter.pc);
      }
      break;
    }
    case "while": {
      const loopStart = emitter.pc;
      compileExpr(ctx, emitter, stmt.cond);
      const patchExit = emitter.emitBranch(0x99); // ifeq
      for (const s of stmt.body) compileStmt(ctx, emitter, s);
      // goto loopStart
      const gotoOp = emitter.emitBranch(0xa7);
      emitter.patchBranch(gotoOp, loopStart);
      emitter.patchBranch(patchExit, emitter.pc);
      break;
    }
    case "for": {
      if (stmt.init) compileStmt(ctx, emitter, stmt.init);
      const loopStart = emitter.pc;
      let patchExit = -1;
      if (stmt.cond) {
        compileExpr(ctx, emitter, stmt.cond);
        patchExit = emitter.emitBranch(0x99); // ifeq
      }
      for (const s of stmt.body) compileStmt(ctx, emitter, s);
      if (stmt.update) compileStmt(ctx, emitter, stmt.update);
      const gotoOp = emitter.emitBranch(0xa7);
      emitter.patchBranch(gotoOp, loopStart);
      if (patchExit >= 0) emitter.patchBranch(patchExit, emitter.pc);
      break;
    }
    case "block": {
      for (const s of stmt.stmts) compileStmt(ctx, emitter, s);
      break;
    }
  }
}

function compileMethod(classDecl: ClassDecl, method: MethodDecl, cp: ConstantPoolBuilder, allMethods: MethodDecl[], inheritedFields: FieldDecl[]): { code: number[]; maxStack: number; maxLocals: number } {
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
    wildcardImports: classDecl.wildcardImports,
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

  for (const method of classDecl.methods) {
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
        wildcardImports: classDecl.wildcardImports,
      };
      if (emitter.maxLocals < method.params.length + 1) emitter.maxLocals = method.params.length + 1;

      // Initialize instance fields with initializers
      for (const field of classDecl.fields) {
        if (!field.isStatic && field.initializer) {
          emitter.emitAload(0); // this
          compileExpr(initCtx, emitter, field.initializer);
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
      const result = compileMethod(classDecl, method, cp, allMethods, inheritedFields);
      compiledMethods.push({
        nameIdx, descIdx, accessFlags,
        code: result.code,
        maxStack: result.maxStack,
        maxLocals: result.maxLocals,
      });
    }
  }

  // Build fields
  const compiledFields: { nameIdx: number; descIdx: number; accessFlags: number }[] = [];
  for (const field of classDecl.fields) {
    const nameIdx = cp.addUtf8(field.name);
    const descIdx = cp.addUtf8(typeToDescriptor(field.type));
    let accessFlags = 0x0001; // ACC_PUBLIC (simplified)
    if (field.isStatic) accessFlags |= 0x0008;
    compiledFields.push({ nameIdx, descIdx, accessFlags });
  }

  // Code attribute name
  const codeAttrName = cp.addUtf8("Code");

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
  const classFlags = 0x0021;
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

  // class attributes_count = 0
  out.push(0x00, 0x00);

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
