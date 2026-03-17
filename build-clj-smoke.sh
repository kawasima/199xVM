#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "$0")" && pwd)"
SMOKE_DIR="$ROOT_DIR/clj-smoke"
CLJ_SRC_DIR="$ROOT_DIR/test-sources/clojure"
TARGET_DIR="$SMOKE_DIR/target"
AOT_DIR="$TARGET_DIR/aot"
UNPACK_DIR="$TARGET_DIR/unpacked"
BUNDLE_FILE="$SMOKE_DIR/bundle.bin"
BUILD_HELPER="$ROOT_DIR/tools/BundleWriter.java"
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

bundle_args=(
  "$BUNDLE_FILE"
  --class-root "$AOT_DIR"
  --resource-root "$CLJ_SRC_DIR/src"
)
for jar_file in "${cp_entries[@]}"; do
  [[ "$jar_file" == *.jar ]] || continue
  [[ "$jar_file" = /* ]] || jar_file="$ROOT_DIR/$jar_file"
  jar_name="$(basename "$jar_file" .jar)"
  jar_root="$UNPACK_DIR/$jar_name"
  bundle_args+=(--class-root "$jar_root" --resource-root "$jar_root")
done

java "$BUILD_HELPER" "${bundle_args[@]}"
echo "Entry point: ClojureSmokeEntry.run()Ljava/lang/String;"
