// Re-export class-reader utilities for use from index.html
export { parseClassMeta, parseBundleMeta, buildMethodRegistry, buildClassInterfaces, readJar, classFilesToBundle } from "./class-reader.js";

export { lex, TokenKind } from "./javac/lexer.js";
export type { Token } from "./javac/lexer.js";

export type {
  ClassDecl,
  Expr,
  FieldDecl,
  MethodDecl,
  ParamDecl,
  Stmt,
  SwitchCase,
  SwitchLabel,
  Type,
} from "./javac/ast.js";

export { parseAll } from "./javac/parser.js";
export { compile, generateClassFile, setMethodRegistry } from "./javac/compiler.js";
export { resetMethodRegistry, setClassInterfaces } from "./javac/method-registry.js";
export { disassemble } from "./javac/disasm.js";
