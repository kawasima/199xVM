import type { Expr, Type } from "./ast.js";

export interface MethodSig {
  owner: string;
  returnType: Type;
  paramTypes: Type[];
  isInterface?: boolean;
  isStatic?: boolean;
  isAbstract?: boolean;
}

// VM-specific methods that are not in jdk-shim/bundle.bin.
// All standard JDK methods (String, Integer, StringBuilder, etc.) are registered
// dynamically from jdk-shim/bundle.bin via setMethodRegistry() at startup.
const BASE_KNOWN_METHODS: Record<string, MethodSig> = {
  // IO (java.lang.IO — JEP 463/512 compact source helper, 199xVM native stub)
  "java/lang/IO.println(Ljava/lang/Object;)": { owner: "java/lang/IO", returnType: "void", paramTypes: [{ className: "java/lang/Object" }], isStatic: true },
  "java/lang/IO.println()": { owner: "java/lang/IO", returnType: "void", paramTypes: [], isStatic: true },
  "java/lang/IO.print(Ljava/lang/Object;)": { owner: "java/lang/IO", returnType: "void", paramTypes: [{ className: "java/lang/Object" }], isStatic: true },
};

let knownMethods: Record<string, MethodSig> = { ...BASE_KNOWN_METHODS };

/** Maps a class name to the interfaces it implements (populated from loaded JARs). */
let knownClassInterfaces: Record<string, string[]> = {};

/** Merge an externally-built method registry into the known methods table. */
export function setMethodRegistry(reg: Record<string, MethodSig>): void {
  knownMethods = { ...knownMethods, ...reg };
}

/** Merge class→interfaces mappings built from loaded JARs. */
export function setClassInterfaces(ifaces: Record<string, string[]>): void {
  for (const [cls, list] of Object.entries(ifaces)) {
    knownClassInterfaces[cls] = [...(knownClassInterfaces[cls] ?? []), ...list.filter(i => !(knownClassInterfaces[cls] ?? []).includes(i))];
  }
}

/** Return known interfaces for a class, or undefined if none recorded. */
export function getKnownClassInterfaces(cls: string): string[] | undefined {
  return knownClassInterfaces[cls];
}

/** Reset the method registry to the built-in defaults (useful for test isolation). */
export function resetMethodRegistry(): void {
  knownMethods = { ...BASE_KNOWN_METHODS };
  knownClassInterfaces = {};
}

export interface FunctionalSig {
  samMethod: string;
  params: Type[];
  returnType: Type;
}

const OBJECT_PUBLIC_INSTANCE_METHODS = new Set([
  "toString()",
  "hashCode()",
  "equals(Ljava/lang/Object;)",
  "getClass()",
  "notify()",
  "notifyAll()",
  "wait()",
  "wait(J)",
  "wait(JI)",
]);

/** Look up a method in knownMethods, falling back to name-only match if exact arg types don't match. */
export function lookupKnownMethod(owner: string, method: string, argDescs: string): MethodSig | undefined {
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
      if (aRef && bRef) return true;
      // Autoboxing: primitive actual → reference param (e.g. int → Object)
      if (aRef && !bRef) return true;
      return false;
    });
    if (compatible) {
      firstCompatible = knownMethods[key];
      break;
    }
  }
  return firstCompatible;
}

export function findKnownMethodByArity(owner: string, method: string, arity: number, wantStatic: boolean): MethodSig | undefined {
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

export function findKnownFunctionalInterface(owner: string): FunctionalSig | undefined {
  const prefix = `${owner}.`;
  const candidates: { name: string; sig: MethodSig }[] = [];
  for (const key of Object.keys(knownMethods)) {
    if (!key.startsWith(prefix)) continue;
    const open = key.indexOf("(", prefix.length);
    const end = key.indexOf(")", open + 1);
    if (open < 0) continue;
    if (end < 0) continue;
    const methodName = key.slice(prefix.length, open);
    const signatureKey = `${methodName}(${key.slice(open + 1, end)})`;
    if (methodName === "<init>" || OBJECT_PUBLIC_INSTANCE_METHODS.has(signatureKey)) continue;
    const sig = knownMethods[key];
    if (!sig.isInterface || sig.isStatic) continue;
    // Skip default (non-abstract) methods — only the SAM (abstract) method counts.
    // isAbstract===false means the registry explicitly marked it as non-abstract.
    if (sig.isAbstract === false) continue;
    candidates.push({ name: methodName, sig });
  }
  if (candidates.length !== 1) return undefined;
  return {
    samMethod: candidates[0].name,
    params: candidates[0].sig.paramTypes,
    returnType: candidates[0].sig.returnType,
  };
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

export function hasFunctionalArg(args: Expr[]): boolean {
  return args.some(a => a.kind === "lambda" || a.kind === "methodRef");
}

export function hasKnownMethodOwnerPrefix(owner: string): boolean {
  return Object.keys(knownMethods).some(k => k.startsWith(`${owner}.`));
}
