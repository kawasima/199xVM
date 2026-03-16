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
// All standard JDK methods (String, Integer, StringBuilder, etc.) must be registered
// dynamically via setMethodRegistry() before calling compile().
// In the browser this happens in loadClassBundle() (index.html).
// In tests this happens at module init (javac.test.ts top-level block).
// Without a loaded shim registry, compile() will fail to resolve JDK method calls.
const BASE_KNOWN_METHODS: Record<string, MethodSig> = {
  // IO (java.lang.IO — JEP 463/512 compact source helper, 199xVM native stub)
  "java/lang/IO.println(Ljava/lang/Object;)": { owner: "java/lang/IO", returnType: "void", paramTypes: [{ className: "java/lang/Object" }], isStatic: true },
  "java/lang/IO.println()": { owner: "java/lang/IO", returnType: "void", paramTypes: [], isStatic: true },
  "java/lang/IO.print(Ljava/lang/Object;)": { owner: "java/lang/IO", returnType: "void", paramTypes: [{ className: "java/lang/Object" }], isStatic: true },
};

let knownMethods: Record<string, MethodSig> = { ...BASE_KNOWN_METHODS };

// ---- Performance indexes (rebuilt on registry mutation) ----
/** Maps "owner.method" → array of {key, sig} for O(1) method group lookup. */
let methodIndex = new Map<string, { key: string; sig: MethodSig }[]>();
/** Set of all known owner class names (internal form) for O(1) hasKnownMethodOwnerPrefix(). */
let ownerSet = new Set<string>();

function rebuildIndexes(): void {
  methodIndex = new Map();
  ownerSet = new Set();
  addToIndexes(knownMethods);
}

/** Add entries to the indexes incrementally (avoids full rebuild). */
function addToIndexes(entries: Record<string, MethodSig>): void {
  for (const key of Object.keys(entries)) {
    const dotIdx = key.indexOf(".");
    if (dotIdx < 0) continue;
    const owner = key.slice(0, dotIdx);
    ownerSet.add(owner);
    const parenIdx = key.indexOf("(", dotIdx);
    const groupKey = parenIdx > 0 ? key.slice(0, parenIdx) : key;
    let group = methodIndex.get(groupKey);
    if (!group) {
      group = [];
      methodIndex.set(groupKey, group);
    }
    // Avoid duplicate entries when the same key is registered again
    if (!group.some(e => e.key === key)) {
      group.push({ key, sig: entries[key] });
    }
  }
}

// Build initial indexes
rebuildIndexes();

/** Maps a class name to the interfaces it implements (populated from loaded JARs). */
let knownClassInterfaces: Record<string, string[]> = {};

/** Merge an externally-built method registry into the known methods table. */
export function setMethodRegistry(reg: Record<string, MethodSig>): void {
  knownMethods = { ...knownMethods, ...reg };
  addToIndexes(reg);
}

/** Merge class→interfaces mappings built from loaded JARs. */
export function setClassInterfaces(ifaces: Record<string, string[]>): void {
  for (const [cls, list] of Object.entries(ifaces)) {
    const existing = new Set(knownClassInterfaces[cls] ?? []);
    for (const i of list) existing.add(i);
    knownClassInterfaces[cls] = [...existing];
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
  rebuildIndexes();
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
  const group = methodIndex.get(`${owner}.${method}`);
  if (!group) return undefined;
  const wantedArgs = splitDescriptorArgs(argDescs);
  for (const entry of group) {
    const start = entry.key.indexOf("(");
    const end = entry.key.indexOf(")");
    if (start < 0 || end < 0) continue;
    const keyArgs = splitDescriptorArgs(entry.key.slice(start + 1, end));
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
    if (compatible) return entry.sig;
  }
  return undefined;
}

export function findKnownMethodByArity(owner: string, method: string, arity: number, wantStatic: boolean): MethodSig | undefined {
  const group = methodIndex.get(`${owner}.${method}`);
  if (!group) return undefined;
  for (const entry of group) {
    const isStatic = entry.sig.isStatic ?? false;
    if (isStatic !== wantStatic) continue;
    if (entry.sig.paramTypes.length === arity) return entry.sig;
  }
  return undefined;
}

export function findKnownFunctionalInterface(owner: string): FunctionalSig | undefined {
  const prefix = `${owner}.`;
  const candidates: { name: string; sig: MethodSig }[] = [];
  // Filter method groups by owner prefix (still iterates all groups)
  for (const [groupKey, group] of methodIndex) {
    if (!groupKey.startsWith(prefix)) continue;
    const methodName = groupKey.slice(prefix.length);
    for (const entry of group) {
      const open = entry.key.indexOf("(");
      const end = entry.key.indexOf(")");
      if (open < 0 || end < 0) continue;
      const signatureKey = `${methodName}(${entry.key.slice(open + 1, end)})`;
      if (methodName === "<init>" || OBJECT_PUBLIC_INSTANCE_METHODS.has(signatureKey)) continue;
      const sig = entry.sig;
      if (!sig.isInterface || sig.isStatic) continue;
      if (sig.isAbstract === false) continue;
      candidates.push({ name: methodName, sig });
    }
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
  return ownerSet.has(owner);
}

/** Return all known class names (internal form, e.g. "java/util/ArrayList"). */
export function getKnownClassNames(): string[] {
  return [...ownerSet];
}

/** Return all known classes under a given package prefix. */
export function getKnownClassesByPackage(pkg: string): string[] {
  const prefix = pkg + "/";
  return [...ownerSet].filter(c => c.startsWith(prefix) && !c.includes("/", prefix.length));
}

/** Return all methods for a given owner class. */
export function getMethodsForClass(owner: string): { name: string; sig: MethodSig }[] {
  const prefix = `${owner}.`;
  const results: { name: string; sig: MethodSig }[] = [];
  for (const [groupKey, group] of methodIndex) {
    if (!groupKey.startsWith(prefix)) continue;
    const methodName = groupKey.slice(prefix.length);
    for (const entry of group) {
      results.push({ name: methodName, sig: entry.sig });
    }
  }
  return results;
}
