#!/usr/bin/env bash
# build-clj-smoke.sh — AOT-compile a minimal Clojure smoke test and package as JARs.
#
# Output:
#   clj-smoke/smoke.jar         — AOT-compiled smoke entry point
#   clj-smoke/clojure-jars.txt  — list of Clojure runtime JAR paths
#
# The VM loads these JARs directly via Vm::load_jar().
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "$0")" && pwd)"
SMOKE_DIR="$ROOT_DIR/clj-smoke"
CLJ_SRC_DIR="$ROOT_DIR/test-sources/clojure"
AOT_DIR="$SMOKE_DIR/target/aot"

rm -rf "$AOT_DIR"
mkdir -p "$AOT_DIR" "$SMOKE_DIR"

echo "Resolving Clojure smoke dependencies..."
CLJ_DEPS="$(tr '\n' ' ' < "$CLJ_SRC_DIR/deps.edn")"
(cd "$ROOT_DIR" && clojure -Sdeps "$CLJ_DEPS" -P)
CLASSPATH="$(cd "$ROOT_DIR" && clojure -Sdeps "$CLJ_DEPS" -Spath)"

echo "AOT-compiling smoke.core..."
java \
  -Dclojure.compile.path="$AOT_DIR" \
  -cp "$CLASSPATH" \
  clojure.main \
  -e "(compile 'smoke.core)"

echo "Packaging smoke classes into JAR..."
(cd "$AOT_DIR" && jar cf "$SMOKE_DIR/smoke.jar" .)

echo "Recording Clojure runtime JAR paths..."
IFS=':' read -r -a cp_entries <<< "$CLASSPATH"
: > "$SMOKE_DIR/clojure-jars.txt"
for entry in "${cp_entries[@]}"; do
  [[ "$entry" == *.jar ]] || continue
  [[ "$entry" = /* ]] || entry="$ROOT_DIR/$entry"
  echo "$entry" >> "$SMOKE_DIR/clojure-jars.txt"
done

smoke_count=$(find "$AOT_DIR" -name '*.class' | wc -l | tr -d ' ')
jar_count=$(wc -l < "$SMOKE_DIR/clojure-jars.txt" | tr -d ' ')
echo "Built $smoke_count smoke classes → $SMOKE_DIR/smoke.jar"
echo "Clojure runtime: $jar_count JARs listed in $SMOKE_DIR/clojure-jars.txt"
