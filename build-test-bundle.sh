#!/usr/bin/env bash
# build-test-bundle.sh — compile test Java classes and create bundle.bin
set -euo pipefail

SRC_DIR="test-sources"
OUT_DIR="test-classes"
OUT_FILE="$OUT_DIR/bundle.bin"
JAR_FILE="$OUT_DIR/test.jar"

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

# Bundle
: > "$OUT_FILE"
count=0
while IFS= read -r -d '' classfile; do
  size=$(wc -c < "$classfile")
  printf "$(printf '\\x%02x\\x%02x\\x%02x\\x%02x' \
    $(( (size >> 24) & 0xff )) \
    $(( (size >> 16) & 0xff )) \
    $(( (size >>  8) & 0xff )) \
    $(( size & 0xff )))" >> "$OUT_FILE"
  cat "$classfile" >> "$OUT_FILE"
  count=$((count + 1))
done < <(find "$OUT_DIR" -maxdepth 1 -name '*.class' -print0 | sort -z)

total=$(wc -c < "$OUT_FILE")
echo "Bundled $count test classes → $OUT_FILE ($total bytes)"

# JAR — same classes packaged as a JAR for the JAR loader path
rm -f "$JAR_FILE"
(cd "$OUT_DIR" && jar cf test.jar *.class)
jar_total=$(wc -c < "$JAR_FILE")
echo "Packed $count test classes → $JAR_FILE ($jar_total bytes)"

# Benchmark bundle
BENCH_SRC="$SRC_DIR/bench"
BENCH_OUT="$OUT_DIR/bench-bundle.bin"

mkdir -p "$OUT_DIR/bench"

javac "$BENCH_SRC"/*.java -d "$OUT_DIR/bench"

: > "$BENCH_OUT"
bench_count=0
while IFS= read -r -d '' classfile; do
  size=$(wc -c < "$classfile")
  printf "$(printf '\\x%02x\\x%02x\\x%02x\\x%02x' \
    $(( (size >> 24) & 0xff )) \
    $(( (size >> 16) & 0xff )) \
    $(( (size >>  8) & 0xff )) \
    $(( size & 0xff )))" >> "$BENCH_OUT"
  cat "$classfile" >> "$BENCH_OUT"
  bench_count=$((bench_count + 1))
done < <(find "$OUT_DIR/bench" -name '*.class' -print0 | sort -z)

bench_total=$(wc -c < "$BENCH_OUT")
echo "Bundled $bench_count bench classes → $BENCH_OUT ($bench_total bytes)"
