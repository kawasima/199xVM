#!/usr/bin/env bash
# build-test-bundle.sh — compile test Java classes and create bundle.bin
set -euo pipefail

SRC_DIR="test-sources"
OUT_DIR="test-classes"
OUT_FILE="$OUT_DIR/bundle.bin"

mkdir -p "$OUT_DIR"

# Compile
javac "$SRC_DIR"/*.java -d "$OUT_DIR"

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
