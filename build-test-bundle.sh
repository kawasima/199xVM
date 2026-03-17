#!/usr/bin/env bash
# build-test-bundle.sh — compile test Java classes and create bundle.bin
set -euo pipefail

SRC_DIR="test-sources"
OUT_DIR="test-classes"
OUT_FILE="$OUT_DIR/bundle.bin"
BENCH_CLASS_DIR="test-bench-classes"
BUILD_HELPER="tools/BundleWriter.java"

rm -rf "$OUT_DIR" "$BENCH_CLASS_DIR"
mkdir -p "$OUT_DIR"

# Ensure web/javac.js exists for compact source compilation
if [ ! -f "web/javac.js" ]; then
  echo "web/javac.js not found — building..."
  npm run build:javac
fi

# Separate compact source files (no top-level class/interface/enum/record)
# from normal Java files. A file is "normal" if any non-import/package/comment
# line contains a class-like declaration keyword.
NORMAL_SOURCES=()
COMPACT_SOURCES=()
for f in "$SRC_DIR"/*.java; do
  # Strip package, import, blank, and single-line comment lines, then check for class keyword
  if sed -E '/^\s*$/d; /^\s*\/\//d; /^\s*package\s/d; /^\s*import\s/d; /^\s*\/?\*/d' "$f" \
     | grep -qE '^\s*(public\s+|abstract\s+|final\s+)*(class|interface|enum|record|@interface)\s'; then
    NORMAL_SOURCES+=("$f")
  else
    COMPACT_SOURCES+=("$f")
  fi
done

# Compile normal Java files with javac
if [ ${#NORMAL_SOURCES[@]} -gt 0 ]; then
  javac "${NORMAL_SOURCES[@]}" -d "$OUT_DIR"
fi

# Compile compact source files with 199xVM's web compiler
for f in "${COMPACT_SOURCES[@]}"; do
  classname=$(basename "$f" .java)
  node compile-compact.mjs "$f" "$OUT_DIR/$classname.class"
done

java "$BUILD_HELPER" "$OUT_FILE" --class-root "$OUT_DIR"

# Benchmark bundle
BENCH_SRC="$SRC_DIR/bench"
BENCH_OUT="$OUT_DIR/bench-bundle.bin"

rm -rf "$BENCH_CLASS_DIR"
mkdir -p "$BENCH_CLASS_DIR"

javac "$BENCH_SRC"/*.java -d "$BENCH_CLASS_DIR"

java "$BUILD_HELPER" "$BENCH_OUT" --class-root "$BENCH_CLASS_DIR"
