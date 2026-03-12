import { TokenKind, type Token } from "./lexer.js";
import type { ClassDecl, Expr, Stmt, Type } from "./ast.js";

const JAVA_LANG_SIMPLE_NAMES = new Set([
  "Object",
  "Class",
  "System",
  "Throwable",
  "Exception",
  "RuntimeException",
  "Integer",
  "Long",
  "Short",
  "Byte",
  "Character",
  "Boolean",
  "Float",
  "Double",
  "StringBuilder",
  "Math",
]);

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
  function isNameSegmentToken(kind: TokenKind): boolean {
    return kind === TokenKind.Ident || kind === TokenKind.KwString;
  }
  function parseNameSegment(): string {
    if (!isNameSegmentToken(peek().kind)) {
      const t = peek();
      throw new Error(`Expected Ident but got ${t.kind} ("${t.value}") at line ${t.line}:${t.col}`);
    }
    return advance().value;
  }
  function parseQualifiedName(): string {
    let name = parseNameSegment();
    while (at(TokenKind.Dot) && isNameSegmentToken(tokens[pos + 1]?.kind ?? TokenKind.EOF)) {
      advance(); // dot
      name += "." + parseNameSegment();
    }
    return name;
  }
  function consumeGenericAngleToken(depth: number): number | undefined {
    let delta: number | undefined;
    if (at(TokenKind.Lt)) delta = 1;
    else if (at(TokenKind.Gt)) delta = -1;
    else if (at(TokenKind.ShiftRight)) delta = -2;
    else if (at(TokenKind.ShiftUnsigned)) delta = -3;
    else return undefined;
    const t = advance();
    const nextDepth = depth + delta;
    if (nextDepth < 0) {
      throw new Error(`Unmatched '>' in generic type at line ${t.line}:${t.col}`);
    }
    return nextDepth;
  }

  function resolveDeclaredClassName(name: string): string {
    if (name.includes("/")) return name;
    if (name.includes(".")) return name.replace(/\./g, "/");
    const explicit = importMap.get(name);
    if (explicit) return explicit;
    if (packageImports.includes("java/lang") && JAVA_LANG_SIMPLE_NAMES.has(name)) {
      return `java/lang/${name}`;
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

    if (at(TokenKind.At) && tokens[pos + 1]?.kind === TokenKind.KwInterface) {
      return parseAnnotationDecl();
    }

    if (at(TokenKind.KwInterface)) {
      return parseInterfaceDecl();
    }

    if (at(TokenKind.KwEnum)) {
      return parseEnumDecl();
    }

    // record Foo(TypeA a, TypeB b) { ... }
    if (at(TokenKind.KwRecord)) {
      advance(); // consume 'record'
      const recordName = expect(TokenKind.Ident).value;
      skipTypeParametersIfPresent();
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
      const interfaces: string[] = [];
      if (match(TokenKind.KwImplements)) {
        interfaces.push(...parseTypeNameList());
      }
      expect(TokenKind.LBrace);
      const recordFields: FieldDecl[] = [];
      const recordMethods: MethodDecl[] = [];
      const recordNestedClasses: ClassDecl[] = [];
      // Parse any explicitly declared methods inside the record body
      while (!at(TokenKind.RBrace) && !at(TokenKind.EOF)) {
        parseMember(recordFields, recordMethods, recordNestedClasses, recordName, "record");
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
        kind: "class",
        superClass: "java/lang/Record",
        interfaces,
        isRecord: true,
        recordComponents: components,
        fields: recordFields,
        methods: recordMethods,
        nestedClasses: recordNestedClasses,
        importMap,
        packageImports,
        staticWildcardImports,
      };
    }

    expect(TokenKind.KwClass);
    const className = expect(TokenKind.Ident).value;
    skipTypeParametersIfPresent();

    let superClass = "java/lang/Object";
    if (match(TokenKind.KwExtends)) {
      superClass = parseResolvedTypeName();
    }
    const interfaces: string[] = [];
    if (match(TokenKind.KwImplements)) {
      interfaces.push(...parseTypeNameList());
    }

    expect(TokenKind.LBrace);

    const fields: FieldDecl[] = [];
    const methods: MethodDecl[] = [];
    const nestedClasses: ClassDecl[] = [];

    while (!at(TokenKind.RBrace) && !at(TokenKind.EOF)) {
      parseMember(fields, methods, nestedClasses, className, "class");
    }
    expect(TokenKind.RBrace);

    return {
      name: className,
      kind: "class",
      superClass,
      interfaces,
      isRecord: false,
      recordComponents: [],
      fields,
      methods,
      nestedClasses,
      importMap,
      packageImports,
      staticWildcardImports,
    };
  }

  function parseInterfaceDecl(): ClassDecl {
    expect(TokenKind.KwInterface);
    const name = expect(TokenKind.Ident).value;
    skipTypeParametersIfPresent();
    const interfaces: string[] = [];
    if (match(TokenKind.KwExtends)) {
      interfaces.push(...parseTypeNameList());
    }
    expect(TokenKind.LBrace);
    const fields: FieldDecl[] = [];
    const methods: MethodDecl[] = [];
    const nestedClasses: ClassDecl[] = [];
    while (!at(TokenKind.RBrace) && !at(TokenKind.EOF)) {
      parseMember(fields, methods, nestedClasses, name, "interface");
    }
    expect(TokenKind.RBrace);
    return {
      name,
      kind: "interface",
      superClass: "java/lang/Object",
      interfaces,
      fields,
      methods,
      nestedClasses,
      importMap,
      packageImports,
      staticWildcardImports,
    };
  }

  function parseAnnotationDecl(): ClassDecl {
    expect(TokenKind.At);
    expect(TokenKind.KwInterface);
    const name = expect(TokenKind.Ident).value;
    expect(TokenKind.LBrace);
    const fields: FieldDecl[] = [];
    const methods: MethodDecl[] = [];
    const nestedClasses: ClassDecl[] = [];
    while (!at(TokenKind.RBrace) && !at(TokenKind.EOF)) {
      parseMember(fields, methods, nestedClasses, name, "annotation");
    }
    expect(TokenKind.RBrace);
    return {
      name,
      kind: "annotation",
      superClass: "java/lang/Object",
      interfaces: ["java/lang/annotation/Annotation"],
      fields,
      methods,
      nestedClasses,
      importMap,
      packageImports,
      staticWildcardImports,
    };
  }

  function parseEnumDecl(): ClassDecl {
    expect(TokenKind.KwEnum);
    const name = expect(TokenKind.Ident).value;
    const interfaces: string[] = [];
    if (match(TokenKind.KwImplements)) {
      interfaces.push(...parseTypeNameList());
    }
    expect(TokenKind.LBrace);
    const fields: FieldDecl[] = [];
    const methods: MethodDecl[] = [];
    const nestedClasses: ClassDecl[] = [];

    while (!at(TokenKind.RBrace) && !at(TokenKind.Semi) && !at(TokenKind.EOF)) {
      const constName = expect(TokenKind.Ident).value;
      const args: Expr[] = [];
      if (match(TokenKind.LParen)) {
        if (!at(TokenKind.RParen)) {
          do { args.push(parseExpr()); } while (match(TokenKind.Comma));
        }
        expect(TokenKind.RParen);
      }
      if (match(TokenKind.LBrace)) {
        let depth = 1;
        while (depth > 0 && !at(TokenKind.EOF)) {
          if (match(TokenKind.LBrace)) depth++;
          else if (match(TokenKind.RBrace)) depth--;
          else advance();
        }
      }
      fields.push({
        name: constName,
        type: { className: name },
        isStatic: true,
        isFinal: true,
        isEnumConstant: true,
        initializer: { kind: "newExpr", className: name, args },
      });
      if (!match(TokenKind.Comma)) break;
      if (at(TokenKind.Semi) || at(TokenKind.RBrace) || at(TokenKind.EOF)) break;
    }

    if (match(TokenKind.Semi)) {
      while (!at(TokenKind.RBrace) && !at(TokenKind.EOF)) {
        parseMember(fields, methods, nestedClasses, name, "enum");
      }
    }

    expect(TokenKind.RBrace);
    return {
      name,
      kind: "enum",
      superClass: "java/lang/Enum",
      interfaces,
      isRecord: false,
      recordComponents: [],
      fields,
      methods,
      nestedClasses,
      importMap,
      packageImports,
      staticWildcardImports,
    };
  }

  function parseResolvedTypeName(): string {
    const name = parseQualifiedName();
    skipTypeArgumentsIfPresent();
    return resolveDeclaredClassName(name);
  }

  function parseTypeNameList(): string[] {
    const out = [parseResolvedTypeName()];
    while (match(TokenKind.Comma)) out.push(parseResolvedTypeName());
    return out;
  }

  function skipTypeParametersIfPresent(): void {
    if (!at(TokenKind.Lt)) return;
    let depth = 1;
    advance();
    while (depth > 0 && !at(TokenKind.EOF)) {
      const nextDepth = consumeGenericAngleToken(depth);
      if (nextDepth !== undefined) depth = nextDepth;
      else advance();
    }
  }

  function skipTypeArgumentsIfPresent(): void {
    skipTypeParametersIfPresent();
  }

  function parseMember(
    fields: FieldDecl[],
    methods: MethodDecl[],
    nestedClasses: ClassDecl[],
    ownerName: string,
    ownerKind: "class" | "record" | "interface" | "annotation" | "enum",
  ) {
    let isStatic = false;
    let isAbstract = ownerKind === "interface" || ownerKind === "annotation";
    let isPrivate = false;
    let isSynchronized = false;

    // Consume modifiers
    while (true) {
      if (at(TokenKind.KwPublic) || at(TokenKind.KwProtected)) { advance(); continue; }
      if (at(TokenKind.KwPrivate)) { advance(); isPrivate = true; continue; }
      if (at(TokenKind.KwStatic)) { advance(); isStatic = true; continue; }
      if (at(TokenKind.KwAbstract)) { advance(); isAbstract = true; continue; }
      if (at(TokenKind.KwFinal)) { advance(); continue; }
      if (at(TokenKind.KwDefault)) { advance(); continue; }
      if (at(TokenKind.KwSynchronized)) {
        if (ownerKind === "interface" || ownerKind === "annotation") {
          throw new Error("'synchronized' is not allowed on interface or annotation members");
        }
        advance(); isSynchronized = true; continue;
      }
      break;
    }

    // Static nested class: static class Inner { ... }
    if (at(TokenKind.KwClass) && isStatic) {
      advance(); // consume 'class'
      const nestedName = expect(TokenKind.Ident).value;
      const mangledName = ownerName + "$" + nestedName;
      let nestedSuper = "java/lang/Object";
      const nestedInterfaces: string[] = [];
      if (match(TokenKind.KwExtends)) {
        nestedSuper = parseResolvedTypeName();
      }
      if (match(TokenKind.KwImplements)) {
        nestedInterfaces.push(...parseTypeNameList());
      }
      expect(TokenKind.LBrace);
      const nf: FieldDecl[] = [];
      const nm: MethodDecl[] = [];
      const nnc: ClassDecl[] = [];
      while (!at(TokenKind.RBrace) && !at(TokenKind.EOF)) {
        parseMember(nf, nm, nnc, mangledName, "class");
      }
      expect(TokenKind.RBrace);
      // Register simple name so outer class can refer to "Inner" as "Outer$Inner"
      importMap.set(nestedName, mangledName);
      nestedClasses.push({
        name: mangledName,
        kind: "class",
        superClass: nestedSuper,
        interfaces: nestedInterfaces,
        isRecord: false,
        recordComponents: [],
        fields: nf,
        methods: nm,
        nestedClasses: nnc,
        importMap,
        packageImports,
        staticWildcardImports,
      });
      return;
    }

    // Constructor: modifiers followed by ClassName(...)
    // Detected by lookahead: current token is Ident and next is '('
    // For nested classes, match either the mangled name (Outer$Inner) or the simple name (Inner).
    const simpleOwnerName = ownerName.includes("$") ? ownerName.slice(ownerName.lastIndexOf("$") + 1) : ownerName;
    skipTypeParametersIfPresent();
    if ((ownerKind === "class" || ownerKind === "record" || ownerKind === "enum")
        && at(TokenKind.Ident)
        && tokens[pos + 1]?.kind === TokenKind.LParen
        && (peek().value === ownerName || peek().value === simpleOwnerName)) {
      if (isSynchronized) throw new Error("'synchronized' is not allowed on constructors");
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
      const throwsTypes = parseOptionalThrowsClause();
      if (match(TokenKind.Semi)) throw new Error("constructor declaration cannot end with ';'");
      expect(TokenKind.LBrace);
      const body = parseBlock();
      expect(TokenKind.RBrace);
      methods.push({ name: "<init>", returnType: "void", params, body, isStatic: false, throwsTypes });
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
      const throwsTypes = parseOptionalThrowsClause();
      if (ownerKind === "annotation" && at(TokenKind.KwDefault)) {
        advance(); // default
        parseExpr(); // ignore default value for now
        expect(TokenKind.Semi);
        methods.push({ name, returnType: retType, params, body: [], isStatic, isAbstract: true, isSynchronized, throwsTypes });
        return;
      }
      if (match(TokenKind.Semi)) {
        const inInterfaceLike = ownerKind === "interface" || ownerKind === "annotation";
        if (!inInterfaceLike && !isAbstract) {
          throw new Error("Method declarations in classes, enums, and records must have a body unless declared abstract.");
        }
        methods.push({ name, returnType: retType, params, body: [], isStatic, isAbstract: inInterfaceLike || isAbstract, isSynchronized, throwsTypes });
      } else {
        expect(TokenKind.LBrace);
        const body = parseBlock();
        expect(TokenKind.RBrace);
        methods.push({ name, returnType: retType, params, body, isStatic, isAbstract: false, isSynchronized, throwsTypes });
      }
    } else {
      // Field
      let init: Expr | undefined;
      if (match(TokenKind.Assign)) {
        init = parseExpr();
      }
      expect(TokenKind.Semi);
      const inRecord = ownerKind === "record";
      const inInterfaceLike = ownerKind === "interface" || ownerKind === "annotation";
      fields.push({
        name,
        type: retType,
        isStatic: inInterfaceLike || isStatic,
        initializer: init,
        isPrivate: inInterfaceLike ? false : inRecord && !isStatic ? true : isPrivate,
        isFinal: inRecord && !isStatic ? true : inInterfaceLike ? true : undefined,
      });
    }
  }

  // Parse method/constructor throws clause and return resolved type names.
  function parseOptionalThrowsClause(): string[] {
    if (!match(TokenKind.KwThrows)) return [];
    const out = [parseResolvedTypeName()];
    while (match(TokenKind.Comma)) {
      out.push(parseResolvedTypeName());
    }
    return out;
  }

  function parseType(): Type {
    let base: Type;
    if (match(TokenKind.KwInt)) base = "int";
    else if (match(TokenKind.KwLong)) base = "long";
    else if (match(TokenKind.KwShort)) base = "short";
    else if (match(TokenKind.KwByte)) base = "byte";
    else if (match(TokenKind.KwChar)) base = "char";
    else if (match(TokenKind.KwFloat)) base = "float";
    else if (match(TokenKind.KwDouble)) base = "double";
    else if (match(TokenKind.KwBoolean)) base = "boolean";
    else if (match(TokenKind.KwVoid)) base = "void";
    else if (match(TokenKind.KwString)) base = "String";
    else if (match(TokenKind.KwVar)) throw new Error(`'var' is only allowed for local variables with initializer`);
    else {
      const name = parseQualifiedName();
      // Skip generic type parameters like <String>
      if (at(TokenKind.Lt)) {
        advance();
        let depth = 1;
        while (depth > 0 && !at(TokenKind.EOF)) {
          const nextDepth = consumeGenericAngleToken(depth);
          if (nextDepth !== undefined) depth = nextDepth;
          else advance();
        }
      }
      // Resolve imports/java.lang-known types eagerly, but keep unresolved simple
      // names as-is so same-compilation-unit user types still work.
      const resolvedName = resolveDeclaredClassName(name);
      base = { className: resolvedName };
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
      const s = parseStmt();
      // Flatten multi-decl blocks into the enclosing block
      if (s.kind === "block" && s.stmts.every(ss => ss.kind === "varDecl")) {
        stmts.push(...s.stmts);
      } else {
        stmts.push(s);
      }
    }
    return stmts;
  }

  function parseSwitchLabel(): SwitchLabel {
    function parsePatternBindVar(): string {
      if (match(TokenKind.KwVar)) return expect(TokenKind.Ident).value;
      if ((at(TokenKind.KwInt) || at(TokenKind.KwLong) || at(TokenKind.KwBoolean) || at(TokenKind.KwString) || at(TokenKind.Ident))
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
      // Colon syntax: "case X:" — collect statements until next case/default/}
      if (at(TokenKind.Colon)) {
        advance(); // consume ':'
        const stmts: Stmt[] = [];
        while (!at(TokenKind.RBrace) && !at(TokenKind.KwCase) && !at(TokenKind.KwDefault) && !at(TokenKind.EOF)) {
          stmts.push(parseStmt());
        }
        cases.push({ labels, guard, stmts });
        continue;
      }
      // Arrow syntax: "case X ->"
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

  function parseAssignOrCompoundTail(target: Expr): Stmt | null {
    if (match(TokenKind.Assign)) {
      const value = parseExpr();
      return { kind: "assign", target, value };
    }
    function makeCompound(op: string): Stmt {
      const value = parseExpr();
      return { kind: "compoundAssign", target, op, value };
    }
    if (match(TokenKind.PlusAssign)) return makeCompound("+");
    if (match(TokenKind.MinusAssign)) return makeCompound("-");
    if (match(TokenKind.StarAssign)) return makeCompound("*");
    if (match(TokenKind.SlashAssign)) return makeCompound("/");
    if (match(TokenKind.PercentAssign)) return makeCompound("%");
    if (match(TokenKind.AndAssign)) return makeCompound("&");
    if (match(TokenKind.OrAssign)) return makeCompound("|");
    if (match(TokenKind.XorAssign)) return makeCompound("^");
    if (match(TokenKind.ShiftLeftAssign)) return makeCompound("<<");
    if (match(TokenKind.ShiftRightAssign)) return makeCompound(">>");
    if (match(TokenKind.ShiftUnsignedAssign)) return makeCompound(">>>");
    // Backward-compat fallback for older token streams where >>= is split as '>' '>='.
    if (at(TokenKind.Gt) && tokens[pos + 1]?.kind === TokenKind.Ge) {
      advance();
      advance();
      return makeCompound(">>");
    }
    // Backward-compat fallback for older token streams where >>>= is split.
    if (at(TokenKind.Gt) && tokens[pos + 1]?.kind === TokenKind.Gt && tokens[pos + 2]?.kind === TokenKind.Ge) {
      advance();
      advance();
      advance();
      return makeCompound(">>>");
    }
    return null;
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
      let body: Stmt[];
      if (at(TokenKind.LBrace)) {
        expect(TokenKind.LBrace);
        body = parseBlock();
        expect(TokenKind.RBrace);
      } else {
        body = [parseStmt()];
      }
      return { kind: "while", cond, body };
    }

    // Do-While
    if (at(TokenKind.KwDo)) {
      advance();
      let body: Stmt[];
      if (at(TokenKind.LBrace)) {
        expect(TokenKind.LBrace);
        body = parseBlock();
        expect(TokenKind.RBrace);
      } else {
        body = [parseStmt()];
      }
      expect(TokenKind.KwWhile);
      expect(TokenKind.LParen);
      const cond = parseExpr();
      expect(TokenKind.RParen);
      expect(TokenKind.Semi);
      return { kind: "doWhile", cond, body };
    }

    // For (enhanced and classic)
    if (at(TokenKind.KwFor)) {
      advance();
      expect(TokenKind.LParen);
      // Try to detect enhanced for: "Type name : expr"
      if (isEnhancedFor()) {
        const varType = parseType();
        const varName = expect(TokenKind.Ident).value;
        expect(TokenKind.Colon);
        const iterable = parseExpr();
        expect(TokenKind.RParen);
        let body: Stmt[];
        if (at(TokenKind.LBrace)) {
          expect(TokenKind.LBrace);
          body = parseBlock();
          expect(TokenKind.RBrace);
        } else {
          body = [parseStmt()];
        }
        return { kind: "forEach", varName, varType, iterable, body };
      }
      let init: Stmt | undefined;
      if (!at(TokenKind.Semi)) init = parseStmtNoSemi();
      expect(TokenKind.Semi);
      let cond: Expr | undefined;
      if (!at(TokenKind.Semi)) cond = parseExpr();
      expect(TokenKind.Semi);
      let update: Stmt | undefined;
      if (!at(TokenKind.RParen)) update = parseStmtNoSemi();
      expect(TokenKind.RParen);
      let body: Stmt[];
      if (at(TokenKind.LBrace)) {
        expect(TokenKind.LBrace);
        body = parseBlock();
        expect(TokenKind.RBrace);
      } else {
        body = [parseStmt()];
      }
      return { kind: "for", init, cond, update, body };
    }

    // Throw
    if (at(TokenKind.KwThrow)) {
      advance();
      const expr = parseExpr();
      expect(TokenKind.Semi);
      return { kind: "throw", expr };
    }

    // Assert
    if (at(TokenKind.KwAssert)) {
      advance();
      const cond = parseExpr();
      let message: Expr | undefined;
      if (match(TokenKind.Colon)) {
        message = parseExpr();
      }
      expect(TokenKind.Semi);
      return { kind: "assert", cond, message };
    }

    // Synchronized
    if (at(TokenKind.KwSynchronized)) {
      advance();
      expect(TokenKind.LParen);
      const monitor = parseExpr();
      expect(TokenKind.RParen);
      expect(TokenKind.LBrace);
      const body = parseBlock();
      expect(TokenKind.RBrace);
      return { kind: "synchronized", monitor, body };
    }

    // Try/Catch/Finally
    if (at(TokenKind.KwTry)) {
      advance();
      const resources: { name: string; type: Type; init: Expr }[] = [];
      if (match(TokenKind.LParen)) {
        if (at(TokenKind.RParen)) {
          throw new Error("try-with-resources requires at least one resource");
        }
        while (!at(TokenKind.RParen) && !at(TokenKind.EOF)) {
          while (at(TokenKind.KwFinal)) advance();
          let resType: Type;
          let resName: string;
          let resInit: Expr;
          if (match(TokenKind.KwVar)) {
            resName = expect(TokenKind.Ident).value;
            expect(TokenKind.Assign);
            resInit = parseExpr();
            resType = inferLocalVarType(resInit);
          } else {
            resType = parseType();
            resName = expect(TokenKind.Ident).value;
            expect(TokenKind.Assign);
            resInit = parseExpr();
          }
          resources.push({ name: resName, type: resType, init: resInit });
          if (match(TokenKind.Semi)) {
            if (at(TokenKind.RParen)) break;
          } else {
            break;
          }
        }
        expect(TokenKind.RParen);
      }
      expect(TokenKind.LBrace);
      const tryBody = parseBlock();
      expect(TokenKind.RBrace);
      const catches: { exType: string; varName: string; body: Stmt[] }[] = [];
      while (at(TokenKind.KwCatch)) {
        advance();
        expect(TokenKind.LParen);
        const exType = expect(TokenKind.Ident).value;
        const varName = expect(TokenKind.Ident).value;
        expect(TokenKind.RParen);
        expect(TokenKind.LBrace);
        const body = parseBlock();
        expect(TokenKind.RBrace);
        catches.push({ exType, varName, body });
      }
      let finallyBody: Stmt[] | undefined;
      if (at(TokenKind.KwFinally)) {
        advance();
        expect(TokenKind.LBrace);
        finallyBody = parseBlock();
        expect(TokenKind.RBrace);
      }
      if (resources.length === 0) return { kind: "tryCatch", tryBody, catches, finallyBody };
      // Desugar try-with-resources into nested try/catch wrappers.
      // Each resource is declared as null first, then assigned inside try so that
      // both normal and exceptional paths close already-acquired resources.
      let loweredTryBody = tryBody;
      for (let i = resources.length - 1; i >= 0; i--) {
        const r = resources[i];
        const exName = `\u0001twr_ex_${i}`;
        const closeExName = `\u0001twr_close_ex_${i}`;
        const primaryName = `\u0001twr_primary_${i}`;
        const closeWithPrimary: Stmt = {
          kind: "if",
          cond: { kind: "binary", op: "!=", left: { kind: "ident", name: r.name }, right: { kind: "nullLit" } },
          then: [
            {
              kind: "if",
              cond: { kind: "binary", op: "!=", left: { kind: "ident", name: primaryName }, right: { kind: "nullLit" } },
              then: [{
                kind: "tryCatch",
                tryBody: [{
                  kind: "exprStmt",
                  expr: { kind: "call", object: { kind: "ident", name: r.name }, method: "close", args: [] },
                }],
                catches: [{
                  exType: "Throwable",
                  varName: closeExName,
                  body: [{
                    kind: "exprStmt",
                    expr: {
                      kind: "call",
                      object: { kind: "ident", name: primaryName },
                      method: "addSuppressed",
                      args: [{ kind: "ident", name: closeExName }],
                    },
                  }],
                }],
              }],
              else_: [{
                kind: "exprStmt",
                expr: { kind: "call", object: { kind: "ident", name: r.name }, method: "close", args: [] },
              }],
            },
          ],
        };
        loweredTryBody = [{
          kind: "block",
          stmts: [
            { kind: "varDecl", name: r.name, type: r.type, init: { kind: "nullLit" } },
            { kind: "varDecl", name: primaryName, type: { className: "java/lang/Throwable" }, init: { kind: "nullLit" } },
            {
              kind: "tryCatch",
              tryBody: [
                { kind: "assign", target: { kind: "ident", name: r.name }, value: r.init },
                ...loweredTryBody,
              ],
              catches: [{
                exType: "Throwable",
                varName: exName,
                body: [
                  { kind: "assign", target: { kind: "ident", name: primaryName }, value: { kind: "ident", name: exName } },
                  { kind: "throw", expr: { kind: "ident", name: exName } },
                ],
              }],
              finallyBody: [closeWithPrimary],
            },
          ],
        }];
      }
      return { kind: "tryCatch", tryBody: loweredTryBody, catches, finallyBody };
    }

    // Break
    if (at(TokenKind.KwBreak)) {
      advance();
      let label: string | undefined;
      if (at(TokenKind.Ident)) label = advance().value;
      expect(TokenKind.Semi);
      return { kind: "break", label };
    }

    // Continue
    if (at(TokenKind.KwContinue)) {
      advance();
      let label: string | undefined;
      if (at(TokenKind.Ident)) label = advance().value;
      expect(TokenKind.Semi);
      return { kind: "continue", label };
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
      // Multi-decl: var a = 1, b = 2;
      if (at(TokenKind.Comma)) {
        const stmts: Stmt[] = [{ kind: "varDecl", name, type: inferLocalVarType(init), init }];
        while (match(TokenKind.Comma)) {
          const n2 = expect(TokenKind.Ident).value;
          expect(TokenKind.Assign);
          const i2 = parseExpr();
          stmts.push({ kind: "varDecl", name: n2, type: inferLocalVarType(i2), init: i2 });
        }
        expect(TokenKind.Semi);
        return { kind: "block", stmts };
      }
      expect(TokenKind.Semi);
      return { kind: "varDecl", name, type: inferLocalVarType(init), init };
    }
    if (isTypeStart() && isVarDecl()) {
      const type = parseType();
      const name = expect(TokenKind.Ident).value;
      let init: Expr | undefined;
      if (match(TokenKind.Assign)) init = parseExpr();
      // Multi-decl: int a = 1, b = 2;
      if (at(TokenKind.Comma)) {
        const stmts: Stmt[] = [{ kind: "varDecl", name, type, init }];
        while (match(TokenKind.Comma)) {
          const n2 = expect(TokenKind.Ident).value;
          let i2: Expr | undefined;
          if (match(TokenKind.Assign)) i2 = parseExpr();
          stmts.push({ kind: "varDecl", name: n2, type, init: i2 });
        }
        expect(TokenKind.Semi);
        return { kind: "block", stmts };
      }
      expect(TokenKind.Semi);
      return { kind: "varDecl", name, type, init };
    }

    // Label statement: "ident : stmt" (but not ident :: which is method ref)
    if (at(TokenKind.Ident) && tokens[pos + 1]?.kind === TokenKind.Colon
        && tokens[pos + 2]?.kind !== TokenKind.Colon) {
      const label = advance().value;
      advance(); // consume ':'
      const stmt = parseStmt();
      return { kind: "labeled", label, stmt };
    }

    // Expression statement (may be assignment)
    const expr = parseExpr();
    const assignStmt = parseAssignOrCompoundTail(expr);
    if (assignStmt) {
      expect(TokenKind.Semi);
      return assignStmt;
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
    const assignStmt = parseAssignOrCompoundTail(expr);
    if (assignStmt) return assignStmt;
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
    return k === TokenKind.KwInt || k === TokenKind.KwLong || k === TokenKind.KwShort || k === TokenKind.KwByte
      || k === TokenKind.KwChar || k === TokenKind.KwFloat || k === TokenKind.KwDouble
      || k === TokenKind.KwBoolean || k === TokenKind.KwVoid
      || k === TokenKind.KwString || k === TokenKind.Ident;
  }

  function isVarDecl(): boolean {
    // Lookahead: type name (= | ;)
    const saved = pos;
    try {
      // Skip type (including generic params)
      if (at(TokenKind.KwInt) || at(TokenKind.KwLong) || at(TokenKind.KwShort) || at(TokenKind.KwByte)
          || at(TokenKind.KwChar) || at(TokenKind.KwFloat) || at(TokenKind.KwDouble)
          || at(TokenKind.KwBoolean) || at(TokenKind.KwVoid) || at(TokenKind.KwString)) {
        advance();
        // Skip array suffix []
        if (at(TokenKind.LBracket) && tokens[pos + 1]?.kind === TokenKind.RBracket) { advance(); advance(); }
      } else if (at(TokenKind.Ident) || at(TokenKind.KwString)) {
        // Qualified type name: a.b.C
        advance();
        while (at(TokenKind.Dot)) {
          advance();
          if (!(at(TokenKind.Ident) || at(TokenKind.KwString))) return false;
          advance();
        }
        // Skip generics
        if (at(TokenKind.Lt)) {
          let depth = 1; advance();
          while (depth > 0 && !at(TokenKind.EOF)) {
            const nextDepth = consumeGenericAngleToken(depth);
            if (nextDepth !== undefined) depth = nextDepth;
            else advance();
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

  // Lookahead: "Type name :" inside for-loop parens → enhanced for
  function isEnhancedFor(): boolean {
    const saved = pos;
    try {
      // Skip type (including generic params, array brackets, qualified names)
      if (at(TokenKind.KwVar)) {
        advance();
      } else if (at(TokenKind.KwInt) || at(TokenKind.KwLong) || at(TokenKind.KwShort) || at(TokenKind.KwByte)
          || at(TokenKind.KwChar) || at(TokenKind.KwFloat) || at(TokenKind.KwDouble)
          || at(TokenKind.KwBoolean) || at(TokenKind.KwString)) {
        advance();
        if (at(TokenKind.LBracket) && tokens[pos + 1]?.kind === TokenKind.RBracket) { advance(); advance(); }
      } else if (at(TokenKind.Ident)) {
        advance();
        while (at(TokenKind.Dot)) { advance(); if (!at(TokenKind.Ident)) return false; advance(); }
        if (at(TokenKind.Lt)) {
          let depth = 1; advance();
          while (depth > 0 && !at(TokenKind.EOF)) {
            const nextDepth = consumeGenericAngleToken(depth);
            if (nextDepth !== undefined) depth = nextDepth;
            else advance();
          }
        }
        if (at(TokenKind.LBracket) && tokens[pos + 1]?.kind === TokenKind.RBracket) { advance(); advance(); }
      } else {
        return false;
      }
      // Must be: Ident ':'
      if (!at(TokenKind.Ident)) return false;
      advance();
      return at(TokenKind.Colon);
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
      case "floatLit": return "float";
      case "doubleLit": return "double";
      case "charLit": return "char";
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
    let left = parseBitwiseOr();
    while (at(TokenKind.And)) {
      advance();
      const right = parseBitwiseOr();
      left = { kind: "binary", op: "&&", left, right };
    }
    return left;
  }

  function parseBitwiseOr(): Expr {
    let left = parseBitwiseXor();
    while (at(TokenKind.BitOr)) {
      advance();
      const right = parseBitwiseXor();
      left = { kind: "binary", op: "|", left, right };
    }
    return left;
  }

  function parseBitwiseXor(): Expr {
    let left = parseBitwiseAnd();
    while (at(TokenKind.BitXor)) {
      advance();
      const right = parseBitwiseAnd();
      left = { kind: "binary", op: "^", left, right };
    }
    return left;
  }

  function parseBitwiseAnd(): Expr {
    let left = parseEquality();
    while (at(TokenKind.BitAnd)) {
      advance();
      const right = parseEquality();
      left = { kind: "binary", op: "&", left, right };
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
          if ((at(TokenKind.KwInt) || at(TokenKind.KwLong) || at(TokenKind.KwBoolean) || at(TokenKind.KwString) || at(TokenKind.Ident))
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
              const nextDepth = consumeGenericAngleToken(depth);
              if (nextDepth !== undefined) depth = nextDepth;
              else advance();
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
    let left = parseShift();
    while (at(TokenKind.Lt) || at(TokenKind.Gt) || at(TokenKind.Le) || at(TokenKind.Ge)) {
      const op = advance().value;
      const right = parseShift();
      left = { kind: "binary", op, left, right };
    }
    return left;
  }

  function parseShift(): Expr {
    let left = parseAdditive();
    while (true) {
      if (match(TokenKind.ShiftLeft)) {
        const right = parseAdditive();
        left = { kind: "binary", op: "<<", left, right };
        continue;
      }
      if (match(TokenKind.ShiftRight)) {
        const right = parseAdditive();
        left = { kind: "binary", op: ">>", left, right };
        continue;
      }
      if (match(TokenKind.ShiftUnsigned)) {
        const right = parseAdditive();
        left = { kind: "binary", op: ">>>", left, right };
        continue;
      }
      // Backward compatibility for older tokenization.
      if (at(TokenKind.Gt) && tokens[pos + 1]?.kind === TokenKind.Gt) {
        if (tokens[pos + 2]?.kind === TokenKind.Ge) break;
        if (tokens[pos + 2]?.kind === TokenKind.Gt) {
          advance(); advance(); advance();
          const right = parseAdditive();
          left = { kind: "binary", op: ">>>", left, right };
          continue;
        }
        advance(); advance();
        const right = parseAdditive();
        left = { kind: "binary", op: ">>", left, right };
        continue;
      }
      break;
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
    if (at(TokenKind.BitNot)) {
      advance();
      const operand = parseUnary();
      return { kind: "unary", op: "~", operand };
    }
    if (at(TokenKind.PlusPlus)) {
      advance();
      const operand = parseUnary();
      return { kind: "preIncrement", operand, op: "++" };
    }
    if (at(TokenKind.MinusMinus)) {
      advance();
      const operand = parseUnary();
      return { kind: "preIncrement", operand, op: "--" };
    }
    return parsePostfix();
  }

  function parsePostfix(): Expr {
    function exprToQualifiedName(e: Expr): string | null {
      if (e.kind === "ident") return e.name;
      if (e.kind === "fieldAccess") {
        const left = exprToQualifiedName(e.object);
        if (!left) return null;
        return `${left}.${e.field}`;
      }
      return null;
    }

    let expr = parsePrimary();

    while (true) {
      if (at(TokenKind.Dot)) {
        advance();
        if (at(TokenKind.KwClass)) {
          advance();
          const qn = exprToQualifiedName(expr);
          if (!qn) throw new Error("Class literal target must be a type name");
          expr = { kind: "classLit", className: qn };
          continue;
        }
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
    // Float literal
    if (at(TokenKind.FloatLiteral)) {
      let raw = advance().value.replace(/_/g, "");
      if (raw.endsWith("f") || raw.endsWith("F")) raw = raw.slice(0, -1);
      return { kind: "floatLit", value: parseFloat(raw) };
    }
    // Double literal
    if (at(TokenKind.DoubleLiteral)) {
      let raw = advance().value.replace(/_/g, "");
      if (raw.endsWith("d") || raw.endsWith("D")) raw = raw.slice(0, -1);
      return { kind: "doubleLit", value: parseFloat(raw) };
    }
    // Char literal
    if (at(TokenKind.CharLiteral)) {
      return { kind: "charLit", value: parseInt(advance().value, 10) };
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
          const nextDepth = consumeGenericAngleToken(depth);
          if (nextDepth !== undefined) depth = nextDepth;
          else advance();
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
      if (at(TokenKind.Ident) || at(TokenKind.KwString) || at(TokenKind.KwInt) || at(TokenKind.KwLong)
          || at(TokenKind.KwShort) || at(TokenKind.KwByte) || at(TokenKind.KwChar)
          || at(TokenKind.KwFloat) || at(TokenKind.KwDouble) || at(TokenKind.KwBoolean)) {
        // Try to read type name (with optional generics)
        let typeName = advance().value;
        // Skip generic params
        if (at(TokenKind.Lt)) {
          let depth = 1; advance();
          while (depth > 0 && !at(TokenKind.EOF)) {
            const nextDepth = consumeGenericAngleToken(depth);
            if (nextDepth !== undefined) depth = nextDepth;
            else advance();
          }
        }
        if (at(TokenKind.RParen)) {
          advance(); // consume ')'
          // Check if this looks like a cast (next token starts an expression but not an operator)
          if (at(TokenKind.Ident) || at(TokenKind.KwThis) || at(TokenKind.KwNew) ||
              at(TokenKind.LParen) || at(TokenKind.IntLiteral) || at(TokenKind.StringLiteral) ||
              at(TokenKind.LongLiteral) || at(TokenKind.FloatLiteral) || at(TokenKind.DoubleLiteral) ||
              at(TokenKind.CharLiteral) || at(TokenKind.BoolLiteral) || at(TokenKind.NullLiteral)) {
            const castExpr = parseUnary();
            const castType: Type = typeName === "String" ? "String"
              : typeName === "int" ? "int"
              : typeName === "long" ? "long"
              : typeName === "short" ? "short"
              : typeName === "byte" ? "byte"
              : typeName === "char" ? "char"
              : typeName === "float" ? "float"
              : typeName === "double" ? "double"
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
