#!/usr/bin/env bash
# build-test-bundle.sh — compile test Java classes and create bundle.bin
set -euo pipefail

SRC_DIR="test-classes"
OUT_FILE="$SRC_DIR/bundle.bin"

# Compile
javac "$SRC_DIR"/*.java -d "$SRC_DIR"

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
done < <(find "$SRC_DIR" -name '*.class' -print0 | sort -z)

total=$(wc -c < "$OUT_FILE")
echo "Bundled $count test classes → $OUT_FILE ($total bytes)"

# Benchmark bundle
BENCH_SRC="$SRC_DIR/bench"
BENCH_OUT="$SRC_DIR/bench-bundle.bin"

javac "$BENCH_SRC"/*.java -d "$BENCH_SRC"

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
done < <(find "$BENCH_SRC" -name '*.class' -print0 | sort -z)

bench_total=$(wc -c < "$BENCH_OUT")
echo "Bundled $bench_count bench classes → $BENCH_OUT ($bench_total bytes)"
