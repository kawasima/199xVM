import type { Expr, Type } from "./ast.js";

export interface MethodSig {
  owner: string;
  returnType: Type;
  paramTypes: Type[];
  isInterface?: boolean;
  isStatic?: boolean;
}

const BASE_KNOWN_METHODS: Record<string, MethodSig> = {
  // Integer
  "java/lang/Integer.valueOf(I)": { owner: "java/lang/Integer", returnType: { className: "java/lang/Integer" }, paramTypes: ["int"], isStatic: true },
  "java/lang/Integer.toString()": { owner: "java/lang/Integer", returnType: "String", paramTypes: [] },
  "java/lang/Integer.intValue()": { owner: "java/lang/Integer", returnType: "int", paramTypes: [] },
  // String
  "java/lang/String.length()": { owner: "java/lang/String", returnType: "int", paramTypes: [] },
  "java/lang/String.charAt(I)": { owner: "java/lang/String", returnType: "char", paramTypes: ["int"] },
  "java/lang/String.substring(I)": { owner: "java/lang/String", returnType: "String", paramTypes: ["int"] },
  "java/lang/String.substring(II)": { owner: "java/lang/String", returnType: "String", paramTypes: ["int", "int"] },
  "java/lang/String.equals(Ljava/lang/Object;)": { owner: "java/lang/String", returnType: "boolean", paramTypes: [{ className: "java/lang/Object" }] },
  "java/lang/String.isEmpty()": { owner: "java/lang/String", returnType: "boolean", paramTypes: [] },
  "java/lang/String.contains(Ljava/lang/CharSequence;)": { owner: "java/lang/String", returnType: "boolean", paramTypes: [{ className: "java/lang/CharSequence" }] },
  "java/lang/String.concat(Ljava/lang/String;)": { owner: "java/lang/String", returnType: "String", paramTypes: ["String"] },
  "java/lang/String.toString()": { owner: "java/lang/String", returnType: "String", paramTypes: [] },
  // Object
  "java/lang/Object.toString()": { owner: "java/lang/Object", returnType: "String", paramTypes: [] },
  "java/lang/Object.wait()": { owner: "java/lang/Object", returnType: "void", paramTypes: [] },
  "java/lang/Object.notify()": { owner: "java/lang/Object", returnType: "void", paramTypes: [] },
  "java/lang/Object.notifyAll()": { owner: "java/lang/Object", returnType: "void", paramTypes: [] },
  "java/lang/Object.getClass()": {
    owner: "java/lang/Object",
    returnType: { className: "java/lang/Class" },
    paramTypes: [],
  },
  // Thread
  "java/lang/Thread.<init>(Ljava/lang/Runnable;)": {
    owner: "java/lang/Thread",
    returnType: "void",
    paramTypes: [{ className: "java/lang/Runnable" }],
  },
  "java/lang/Thread.start()": {
    owner: "java/lang/Thread",
    returnType: "void",
    paramTypes: [],
  },
  "java/lang/Thread.join()": {
    owner: "java/lang/Thread",
    returnType: "void",
    paramTypes: [],
  },
  "java/lang/Thread.yield()": {
    owner: "java/lang/Thread",
    returnType: "void",
    paramTypes: [],
    isStatic: true,
  },
  "java/lang/Throwable.addSuppressed(Ljava/lang/Throwable;)": {
    owner: "java/lang/Throwable",
    returnType: "void",
    paramTypes: [{ className: "java/lang/Throwable" }],
  },
  // StringBuilder
  "java/lang/StringBuilder.<init>()": { owner: "java/lang/StringBuilder", returnType: "void", paramTypes: [] },
  "java/lang/StringBuilder.append(Ljava/lang/String;)": { owner: "java/lang/StringBuilder", returnType: { className: "java/lang/StringBuilder" }, paramTypes: ["String"] },
  "java/lang/StringBuilder.append(I)": { owner: "java/lang/StringBuilder", returnType: { className: "java/lang/StringBuilder" }, paramTypes: ["int"] },
  "java/lang/StringBuilder.append(J)": { owner: "java/lang/StringBuilder", returnType: { className: "java/lang/StringBuilder" }, paramTypes: ["long"] },
  "java/lang/StringBuilder.append(F)": { owner: "java/lang/StringBuilder", returnType: { className: "java/lang/StringBuilder" }, paramTypes: ["float"] },
  "java/lang/StringBuilder.append(D)": { owner: "java/lang/StringBuilder", returnType: { className: "java/lang/StringBuilder" }, paramTypes: ["double"] },
  "java/lang/StringBuilder.append(C)": { owner: "java/lang/StringBuilder", returnType: { className: "java/lang/StringBuilder" }, paramTypes: ["char"] },
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
  // CompletableFuture
  "java/util/concurrent/CompletableFuture.supplyAsync(Ljava/util/function/Supplier;)": {
    owner: "java/util/concurrent/CompletableFuture",
    returnType: { className: "java/util/concurrent/CompletableFuture" },
    paramTypes: [{ className: "java/util/function/Supplier" }],
    isStatic: true,
  },
  "java/util/concurrent/CompletableFuture.thenApply(Ljava/util/function/Function;)": {
    owner: "java/util/concurrent/CompletableFuture",
    returnType: { className: "java/util/concurrent/CompletableFuture" },
    paramTypes: [{ className: "java/util/function/Function" }],
  },
  "java/util/concurrent/CompletableFuture.join()": {
    owner: "java/util/concurrent/CompletableFuture",
    returnType: { className: "java/lang/Object" },
    paramTypes: [],
  },
  // Raoh core (baseline signatures to avoid load-order sensitivity)
  "net/unit8/raoh/ObjectDecoders.string()": {
    owner: "net/unit8/raoh/ObjectDecoders",
    returnType: { className: "net/unit8/raoh/builtin/StringDecoder" },
    paramTypes: [],
    isStatic: true,
  },
  "net/unit8/raoh/ObjectDecoders.int_()": {
    owner: "net/unit8/raoh/ObjectDecoders",
    returnType: { className: "net/unit8/raoh/builtin/IntDecoder" },
    paramTypes: [],
    isStatic: true,
  },
  "net/unit8/raoh/map/MapDecoders.field(Ljava/lang/String;Lnet/unit8/raoh/Decoder;)": {
    owner: "net/unit8/raoh/map/MapDecoders",
    returnType: { className: "net/unit8/raoh/FieldDecoder" },
    paramTypes: ["String", { className: "net/unit8/raoh/Decoder" }],
    isStatic: true,
  },
  "net/unit8/raoh/map/MapDecoders.combine(Lnet/unit8/raoh/Decoder;Lnet/unit8/raoh/Decoder;)": {
    owner: "net/unit8/raoh/map/MapDecoders",
    returnType: { className: "net/unit8/raoh/combinator/Combiner2" },
    paramTypes: [{ className: "net/unit8/raoh/Decoder" }, { className: "net/unit8/raoh/Decoder" }],
    isStatic: true,
  },
  "net/unit8/raoh/combinator/Combiner2.map(Ljava/util/function/BiFunction;)": {
    owner: "net/unit8/raoh/combinator/Combiner2",
    returnType: { className: "net/unit8/raoh/Decoder" },
    paramTypes: [{ className: "java/util/function/BiFunction" }],
  },
  "net/unit8/raoh/Decoder.decode(Ljava/lang/Object;)": {
    owner: "net/unit8/raoh/Decoder",
    returnType: { className: "net/unit8/raoh/Result" },
    paramTypes: [{ className: "java/lang/Object" }],
    isInterface: true,
  },
  "net/unit8/raoh/Result.getOrThrow()": {
    owner: "net/unit8/raoh/Result",
    returnType: { className: "java/lang/Object" },
    paramTypes: [],
    isInterface: true,
  },
  // PrintStream
  "java/io/PrintStream.println(Ljava/lang/String;)": { owner: "java/io/PrintStream", returnType: "void", paramTypes: ["String"] },
  "java/io/PrintStream.println(I)": { owner: "java/io/PrintStream", returnType: "void", paramTypes: ["int"] },
  "java/io/PrintStream.println(Ljava/lang/Object;)": { owner: "java/io/PrintStream", returnType: "void", paramTypes: [{ className: "java/lang/Object" }] },
  "java/io/PrintStream.print(Ljava/lang/String;)": { owner: "java/io/PrintStream", returnType: "void", paramTypes: ["String"] },
  "java/io/PrintStream.print(I)": { owner: "java/io/PrintStream", returnType: "void", paramTypes: ["int"] },
  // Class / reflection
  "java/lang/Class.getName()": {
    owner: "java/lang/Class",
    returnType: "String",
    paramTypes: [],
  },
  "java/lang/Class.isInstance(Ljava/lang/Object;)": {
    owner: "java/lang/Class",
    returnType: "boolean",
    paramTypes: [{ className: "java/lang/Object" }],
  },
  "java/lang/Class.getDeclaredFields()": {
    owner: "java/lang/Class",
    returnType: { array: { className: "java/lang/reflect/Field" } },
    paramTypes: [],
  },
  "java/lang/Class.getDeclaredMethods()": {
    owner: "java/lang/Class",
    returnType: { array: { className: "java/lang/reflect/Method" } },
    paramTypes: [],
  },
  "java/lang/Class.getDeclaredConstructors()": {
    owner: "java/lang/Class",
    returnType: { array: { className: "java/lang/reflect/Constructor" } },
    paramTypes: [],
  },
  "java/lang/Class.getInterfaces()": {
    owner: "java/lang/Class",
    returnType: { array: { className: "java/lang/Class" } },
    paramTypes: [],
  },
  "java/lang/Class.getSuperclass()": {
    owner: "java/lang/Class",
    returnType: { className: "java/lang/Class" },
    paramTypes: [],
  },
  "java/lang/Class.isInterface()": {
    owner: "java/lang/Class",
    returnType: "boolean",
    paramTypes: [],
  },
  "java/lang/Class.isAssignableFrom(Ljava/lang/Class;)": {
    owner: "java/lang/Class",
    returnType: "boolean",
    paramTypes: [{ className: "java/lang/Class" }],
  },
  "java/lang/Class.getModifiers()": {
    owner: "java/lang/Class",
    returnType: "int",
    paramTypes: [],
  },
  "java/lang/Class.isRecord()": {
    owner: "java/lang/Class",
    returnType: "boolean",
    paramTypes: [],
  },
  "java/lang/Class.getRecordComponents()": {
    owner: "java/lang/Class",
    returnType: { array: { className: "java/lang/reflect/RecordComponent" } },
    paramTypes: [],
  },
};

let knownMethods: Record<string, MethodSig> = { ...BASE_KNOWN_METHODS };

/** Merge an externally-built method registry into the known methods table. */
export function setMethodRegistry(reg: Record<string, MethodSig>): void {
  knownMethods = { ...BASE_KNOWN_METHODS, ...reg };
}

/** Reset the method registry to the built-in defaults (useful for test isolation). */
export function resetMethodRegistry(): void {
  knownMethods = { ...BASE_KNOWN_METHODS };
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
      return aRef && bRef;
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
