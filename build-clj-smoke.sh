#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "$0")" && pwd)"
SMOKE_DIR="$ROOT_DIR/clj-smoke"
CLJ_SRC_DIR="$ROOT_DIR/test-sources/clojure"
TARGET_DIR="$SMOKE_DIR/target"
AOT_DIR="$TARGET_DIR/aot"
UNPACK_DIR="$TARGET_DIR/unpacked"
BUNDLE_FILE="$SMOKE_DIR/bundle.bin"
CLJ_DEPS="$(tr '\n' ' ' < "$CLJ_SRC_DIR/deps.edn")"

rm -rf "$AOT_DIR" "$UNPACK_DIR"
mkdir -p "$AOT_DIR" "$UNPACK_DIR"

echo "Resolving Clojure smoke dependencies with tools.deps..."
(
  cd "$ROOT_DIR"
  clojure -Sdeps "$CLJ_DEPS" -P
)
CLASSPATH="$(
  cd "$ROOT_DIR"
  clojure -Sdeps "$CLJ_DEPS" -Spath
)"

echo "AOT-compiling smoke namespace..."
java \
  -Dclojure.compile.path="$AOT_DIR" \
  -cp "$CLASSPATH" \
  clojure.main \
  -e "(compile 'smoke.core)"

echo "Unpacking runtime classes..."
IFS=':' read -r -a cp_entries <<< "$CLASSPATH"
for jar_file in "${cp_entries[@]}"; do
  [[ "$jar_file" == *.jar ]] || continue
  [[ "$jar_file" = /* ]] || jar_file="$ROOT_DIR/$jar_file"
  jar_name="$(basename "$jar_file" .jar)"
  jar_out="$UNPACK_DIR/$jar_name"
  mkdir -p "$jar_out"
  (
    cd "$jar_out"
    jar xf "$jar_file"
  )
done

: > "$BUNDLE_FILE"
count=0
while IFS= read -r -d '' classfile; do
  size=$(wc -c < "$classfile")
  printf "$(printf '\\x%02x\\x%02x\\x%02x\\x%02x' \
    $(( (size >> 24) & 0xff )) \
    $(( (size >> 16) & 0xff )) \
    $(( (size >>  8) & 0xff )) \
    $(( size & 0xff )))" >> "$BUNDLE_FILE"
  cat "$classfile" >> "$BUNDLE_FILE"
  count=$((count + 1))
done < <(
  {
    find "$AOT_DIR" -type f -name '*.class' \( -path "$AOT_DIR/smoke/*" -o -name 'ClojureSmokeEntry.class' \) -print0
    find "$UNPACK_DIR" -type f -name '*.class' -print0
  } | sort -z
)

total=$(wc -c < "$BUNDLE_FILE")
echo "Bundled $count Clojure smoke classes → $BUNDLE_FILE ($total bytes)"
echo "Entry point: ClojureSmokeEntry.run()Ljava/lang/String;"
