#!/usr/bin/env bash
# build-bundle.sh — pack Raoh .class files into raoh-classes/bundle.bin
#
# Format: repeated [ u32 length (big-endian) ][ raw .class bytes ]
#
# Usage:
#   ./build-bundle.sh [path-to-raoh-classes-dir]
#
# Default source: ../raoh/raoh/target/classes

set -euo pipefail

CLASSES_DIR="${1:-../raoh/raoh/target/classes}"
OUT_DIR="raoh-classes"
OUT_FILE="$OUT_DIR/bundle.bin"

if [ ! -d "$CLASSES_DIR" ]; then
  echo "Error: classes directory not found: $CLASSES_DIR"
  echo "Run: mvn -f ../raoh/raoh/pom.xml compile"
  exit 1
fi

mkdir -p "$OUT_DIR"
: > "$OUT_FILE"   # truncate

count=0
while IFS= read -r -d '' classfile; do
  size=$(wc -c < "$classfile")
  # Write 4-byte big-endian length
  printf "$(printf '\\x%02x\\x%02x\\x%02x\\x%02x' \
    $(( (size >> 24) & 0xff )) \
    $(( (size >> 16) & 0xff )) \
    $(( (size >>  8) & 0xff )) \
    $(( size & 0xff )))" >> "$OUT_FILE"
  cat "$classfile" >> "$OUT_FILE"
  count=$((count + 1))
done < <(find "$CLASSES_DIR" -name '*.class' -print0 | sort -z)

total=$(wc -c < "$OUT_FILE")
echo "Bundled $count classes → $OUT_FILE ($total bytes)"
