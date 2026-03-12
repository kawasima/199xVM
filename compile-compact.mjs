#!/usr/bin/env node
// compile-compact.mjs <source.java> <output.class>
// Compiles a compact source file (no class declaration) using 199xVM's web compiler.
import { readFileSync, writeFileSync } from "node:fs";
import { basename } from "node:path";
import { compile } from "./web/javac.js";

const [src, out] = process.argv.slice(2);
if (!src || !out) {
  console.error("Usage: compile-compact.mjs <source.java> <output.class>");
  process.exit(1);
}

const source = readFileSync(src, "utf8");
const className = basename(src, ".java");
const bytes = compile(source, className);
writeFileSync(out, bytes);
console.log(`Compiled compact source ${src} → ${out} (class: ${className})`);
