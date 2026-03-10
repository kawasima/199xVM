#!/usr/bin/env bash
# build-shim.sh — compile the 199xVM JDK shim classes and produce a class bundle.
#
# Output: jdk-shim/out/   (compiled .class files)
#         jdk-shim/bundle.bin  (bundle format: repeated [u32 len][class bytes])
#
# Usage:
#   ./build-shim.sh

set -euo pipefail

SHIM_SRC="jdk-shim"
OUT_DIR="$SHIM_SRC/out"
BUNDLE="$SHIM_SRC/bundle.bin"

# Entry points — javac resolves transitive deps via -sourcepath.
ENTRY_POINTS=(
  "$SHIM_SRC/java/util/ArrayList.java"
  "$SHIM_SRC/java/util/HashMap.java"
  "$SHIM_SRC/java/util/Optional.java"
  "$SHIM_SRC/java/util/Map.java"
  "$SHIM_SRC/java/util/Arrays.java"
  "$SHIM_SRC/java/lang/Record.java"
  "$SHIM_SRC/java/lang/FunctionalInterface.java"
  "$SHIM_SRC/java/util/stream/Collectors.java"
  "$SHIM_SRC/java/util/stream/StreamImpl.java"
)

rm -rf "$OUT_DIR"
mkdir -p "$OUT_DIR"

echo "Compiling JDK shim classes..."
javac --patch-module java.base="$SHIM_SRC" \
      -sourcepath "$SHIM_SRC" \
      -d "$OUT_DIR" \
      "${ENTRY_POINTS[@]}"

# Build bundle.bin from compiled classes.
: > "$BUNDLE"
count=0
while IFS= read -r -d '' classfile; do
  size=$(wc -c < "$classfile")
  printf "$(printf '\\x%02x\\x%02x\\x%02x\\x%02x' \
    $(( (size >> 24) & 0xff )) \
    $(( (size >> 16) & 0xff )) \
    $(( (size >>  8) & 0xff )) \
    $(( size & 0xff )))" >> "$BUNDLE"
  cat "$classfile" >> "$BUNDLE"
  count=$((count + 1))
done < <(find "$OUT_DIR" -name '*.class' -print0 | sort -z)

total=$(wc -c < "$BUNDLE")
echo "Bundled $count shim classes → $BUNDLE ($total bytes)"
