export type Type = "int" | "long" | "short" | "byte" | "char" | "float" | "double" | "boolean" | "void" | "String" | { className: string } | { array: Type };

export interface ClassDecl {
  name: string;
  kind?: "class" | "interface" | "enum" | "annotation";
  superClass: string;
  interfaces?: string[];
  isRecord?: boolean;
  recordComponents?: ParamDecl[];
  fields: FieldDecl[];
  methods: MethodDecl[];
  nestedClasses: ClassDecl[];
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
  isAbstract?: boolean;
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
  | { kind: "doWhile"; cond: Expr; body: Stmt[] }
  | { kind: "forEach"; varName: string; varType: Type; iterable: Expr; body: Stmt[] }
  | { kind: "throw"; expr: Expr }
  | { kind: "tryCatch"; tryBody: Stmt[]; catches: { exType: string; varName: string; body: Stmt[] }[]; finallyBody?: Stmt[] }
  | { kind: "break"; label?: string }
  | { kind: "continue"; label?: string }
  | { kind: "labeled"; label: string; stmt: Stmt }
  | { kind: "block"; stmts: Stmt[] };

export type Expr =
  | { kind: "intLit"; value: number }
  | { kind: "longLit"; value: number }
  | { kind: "floatLit"; value: number }
  | { kind: "doubleLit"; value: number }
  | { kind: "charLit"; value: number }
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
  | { kind: "preIncrement"; operand: Expr; op: "++" | "--" }
  | { kind: "instanceof"; expr: Expr; checkType: string; bindVar?: string; recordBindVars?: string[] }
  | { kind: "staticField"; className: string; field: string }
  | { kind: "arrayAccess"; array: Expr; index: Expr }
  | { kind: "arrayLit"; elemType: Type; elements: Expr[] }
  | { kind: "newArray"; elemType: Type; size: Expr }
  | { kind: "superCall"; args: Expr[] }
  | { kind: "ternary"; cond: Expr; thenExpr: Expr; elseExpr: Expr }
  | { kind: "switchExpr"; selector: Expr; cases: SwitchCase[] }
  | { kind: "lambda"; params: string[]; bodyExpr?: Expr; bodyStmts?: Stmt[] }
  | { kind: "methodRef"; target: Expr; method: string; isConstructor: boolean }
  | { kind: "classLit"; className: string };
