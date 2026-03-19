#!/usr/bin/env bash
# build-clj-smoke.sh — build Clojure validation artifacts for 199xVM.
#
# Output:
#   clj-smoke/smoke.jar            — AOT-compiled minimal smoke entry point
#   clj-smoke/upstream-tests.jar   — selected upstream Clojure self-tests + runner
#   clj-smoke/clojure-jars.txt     — local copied Clojure runtime JAR paths
#   clj-smoke/clojure-upstream.txt — upstream checkout metadata
#
# The VM loads these JARs directly via Vm::load_jar().
set -euo pipefail

require_cmd() {
  if ! command -v "$1" >/dev/null 2>&1; then
    echo "Missing required command: $1" >&2
    exit 1
  fi
}

metadata_path() {
  local path="$1"
  case "$path" in
    "$ROOT_DIR"/*) printf '%s\n' "${path#"$ROOT_DIR"/}" ;;
    *) printf '%s\n' "$path" ;;
  esac
}

selected_upstream_test_namespaces() {
  local runner_file="$1"
  grep -oE 'clojure\.test-clojure\.[[:alnum:].-]+' "$runner_file" | awk '!seen[$0]++'
}

resolve_upstream_test_source() {
  local upstream_test_dir="$1"
  local ns="$2"
  local rel="${ns//./\/}"
  rel="${rel//-/_}"
  local candidate="$upstream_test_dir/$rel"
  if [ -f "$candidate.clj" ]; then
    printf '%s\n' "$candidate.clj"
    return 0
  fi
  if [ -f "$candidate.cljc" ]; then
    printf '%s\n' "$candidate.cljc"
    return 0
  fi
  return 1
}

stage_upstream_test_subset() {
  local runner_file="$1"
  local upstream_test_dir="$2"
  local stage_dir="$3"
  local -a queue=()
  local ns=""
  while IFS= read -r ns; do
    [ -n "$ns" ] && queue+=("$ns")
  done < <(selected_upstream_test_namespaces "$runner_file")

  if [ "${#queue[@]}" -eq 0 ]; then
    echo "No selected upstream test namespaces found in $runner_file" >&2
    exit 1
  fi

  declare -A staged=()
  while [ "${#queue[@]}" -gt 0 ]; do
    ns="${queue[0]}"
    queue=("${queue[@]:1}")
    if [ -n "${staged[$ns]:-}" ]; then
      continue
    fi

    local src=""
    if ! src="$(resolve_upstream_test_source "$upstream_test_dir" "$ns")"; then
      continue
    fi

    staged["$ns"]=1
    local rel_path="${src#"$upstream_test_dir"/}"
    mkdir -p "$stage_dir/$(dirname "$rel_path")"
    cp "$src" "$stage_dir/$rel_path"

    local dep=""
    while IFS= read -r dep; do
      [ -n "$dep" ] || continue
      [ -n "${staged[$dep]:-}" ] && continue
      if resolve_upstream_test_source "$upstream_test_dir" "$dep" >/dev/null; then
        queue+=("$dep")
      fi
    done < <(grep -oE 'clojure\.[[:alnum:].-]+' "$src" | awk '!seen[$0]++')
  done
}

ROOT_DIR="$(cd "$(dirname "$0")" && pwd)"
SMOKE_DIR="$ROOT_DIR/clj-smoke"
TARGET_DIR="$SMOKE_DIR/target"
CLJ_ROOT_DIR="$ROOT_DIR/test-sources/clojure"
CLJ_SRC_DIR="$CLJ_ROOT_DIR/src"
UPSTREAM_RUNNER_SRC="$CLJ_SRC_DIR/upstream/runner.clj"
SMOKE_CLASSES_DIR="$TARGET_DIR/smoke-classes"
UPSTREAM_STAGE_DIR="$TARGET_DIR/upstream-stage"
LOCAL_JARS_DIR="$SMOKE_DIR/jars"
SMOKE_JAR="$SMOKE_DIR/smoke.jar"
UPSTREAM_JAR="$SMOKE_DIR/upstream-tests.jar"
JARS_LIST="$SMOKE_DIR/clojure-jars.txt"
UPSTREAM_META="$SMOKE_DIR/clojure-upstream.txt"
UPSTREAM_CACHE_DIR="$TARGET_DIR/upstream-src"

mkdir -p "$SMOKE_DIR" "$TARGET_DIR"
TMP_DIR="$(mktemp -d "$TARGET_DIR/.build-clj-smoke.XXXXXX")"
cleanup() {
  rm -rf "$TMP_DIR"
}
trap cleanup EXIT

TMP_SMOKE_CLASSES_DIR="$TMP_DIR/smoke-classes"
TMP_UPSTREAM_STAGE_DIR="$TMP_DIR/upstream-stage"
TMP_LOCAL_JARS_DIR="$TMP_DIR/jars"
TMP_SMOKE_JAR="$TMP_DIR/smoke.jar"
TMP_UPSTREAM_JAR="$TMP_DIR/upstream-tests.jar"
TMP_JARS_LIST="$TMP_DIR/clojure-jars.txt"
TMP_UPSTREAM_META="$TMP_DIR/clojure-upstream.txt"

mkdir -p "$TMP_SMOKE_CLASSES_DIR" "$TMP_UPSTREAM_STAGE_DIR" "$TMP_LOCAL_JARS_DIR"

DETECTED_CLOJURE_VERSION="$(sed -nE 's/.*org\.clojure\/clojure \{:mvn\/version "([^"]+)".*/\1/p' "$CLJ_ROOT_DIR/deps.edn")"
CLOJURE_RUNTIME_VERSION="${CLOJURE_RUNTIME_VERSION:-$DETECTED_CLOJURE_VERSION}"
if [ -z "$CLOJURE_RUNTIME_VERSION" ]; then
  echo "Failed to detect Clojure version from $CLJ_ROOT_DIR/deps.edn" >&2
  exit 1
fi

UPSTREAM_TAG="${CLOJURE_UPSTREAM_TAG:-clojure-$CLOJURE_RUNTIME_VERSION}"
UPSTREAM_URL="${CLOJURE_UPSTREAM_URL:-https://github.com/clojure/clojure}"
UPSTREAM_DIR="${CLOJURE_UPSTREAM_DIR:-$UPSTREAM_CACHE_DIR}"

for cmd in clojure java jar; do
  require_cmd "$cmd"
done

RESOLVED_UPSTREAM_DIR=""
if [ -n "${CLOJURE_UPSTREAM_DIR:-}" ]; then
  RESOLVED_UPSTREAM_DIR="$(cd "$UPSTREAM_DIR" && pwd)"
  if [ ! -d "$RESOLVED_UPSTREAM_DIR/test" ]; then
    echo "CLOJURE_UPSTREAM_DIR does not look like a clojure/clojure checkout: $RESOLVED_UPSTREAM_DIR" >&2
    exit 1
  fi
  echo "Using existing Clojure upstream checkout: $RESOLVED_UPSTREAM_DIR"
else
  require_cmd git
  current_tag=""
  if [ -d "$UPSTREAM_CACHE_DIR/.git" ]; then
    current_tag="$(git -C "$UPSTREAM_CACHE_DIR" describe --tags --exact-match 2>/dev/null || true)"
  fi

  if [ "$current_tag" = "$UPSTREAM_TAG" ]; then
    RESOLVED_UPSTREAM_DIR="$UPSTREAM_CACHE_DIR"
    echo "Reusing Clojure upstream checkout: $RESOLVED_UPSTREAM_DIR ($current_tag)"
  else
    RESOLVED_UPSTREAM_DIR="$TMP_DIR/upstream-src"
    echo "Fetching Clojure upstream sources ($UPSTREAM_TAG)..."
    git clone --depth 1 --branch "$UPSTREAM_TAG" "$UPSTREAM_URL" "$RESOLVED_UPSTREAM_DIR"
  fi
fi

UPSTREAM_TEST_DIR="$RESOLVED_UPSTREAM_DIR/test"
if [ ! -d "$UPSTREAM_TEST_DIR/clojure" ]; then
  echo "Missing upstream Clojure test directory: $UPSTREAM_TEST_DIR/clojure" >&2
  exit 1
fi

echo "Resolving Clojure runtime dependencies..."
CLJ_DEPS="$(tr '\n' ' ' < "$CLJ_ROOT_DIR/deps.edn")"
(cd "$ROOT_DIR" && clojure -Sdeps "$CLJ_DEPS" -P)
CLASSPATH="$(cd "$ROOT_DIR" && clojure -Sdeps "$CLJ_DEPS" -Spath)"

echo "Copying Clojure runtime JARs into $TMP_LOCAL_JARS_DIR..."
IFS=':' read -r -a cp_entries <<< "$CLASSPATH"
: > "$TMP_JARS_LIST"
jar_count=0
for entry in "${cp_entries[@]}"; do
  [[ "$entry" == *.jar ]] || continue
  [[ "$entry" = /* ]] || entry="$ROOT_DIR/$entry"
  dest="$TMP_LOCAL_JARS_DIR/$(basename "$entry")"
  cp "$entry" "$dest"
  echo "jars/$(basename "$entry")" >> "$TMP_JARS_LIST"
  jar_count=$((jar_count + 1))
done

if [ "$jar_count" -eq 0 ]; then
  echo "No runtime JARs were resolved from the Clojure classpath" >&2
  exit 1
fi

echo "AOT-compiling smoke.core..."
java \
  -Dclojure.compile.path="$TMP_SMOKE_CLASSES_DIR" \
  -cp "$CLASSPATH" \
  clojure.lang.Compile \
  smoke.core

echo "Packaging smoke classes into JAR..."
(cd "$TMP_SMOKE_CLASSES_DIR" && jar cf "$TMP_SMOKE_JAR" .)

echo "Staging selected upstream Clojure test sources..."
stage_upstream_test_subset "$UPSTREAM_RUNNER_SRC" "$UPSTREAM_TEST_DIR" "$TMP_UPSTREAM_STAGE_DIR"

UPSTREAM_COMPILE_CLASSPATH="$TMP_UPSTREAM_STAGE_DIR:$CLJ_SRC_DIR:$CLASSPATH"
echo "AOT-compiling upstream runner..."
java \
  -Dclojure.compile.path="$TMP_UPSTREAM_STAGE_DIR" \
  -cp "$UPSTREAM_COMPILE_CLASSPATH" \
  clojure.lang.Compile \
  upstream.runner

echo "Packaging upstream test resources into JAR..."
(cd "$TMP_UPSTREAM_STAGE_DIR" && jar cf "$TMP_UPSTREAM_JAR" .)

if [ -z "${CLOJURE_UPSTREAM_DIR:-}" ] && [ "$RESOLVED_UPSTREAM_DIR" = "$TMP_DIR/upstream-src" ]; then
  rm -rf "$UPSTREAM_CACHE_DIR"
  mv "$RESOLVED_UPSTREAM_DIR" "$UPSTREAM_CACHE_DIR"
  RESOLVED_UPSTREAM_DIR="$UPSTREAM_CACHE_DIR"
  UPSTREAM_TEST_DIR="$RESOLVED_UPSTREAM_DIR/test"
fi

{
  echo "version=$CLOJURE_RUNTIME_VERSION"
  echo "tag=$UPSTREAM_TAG"
  echo "checkout=$(metadata_path "$RESOLVED_UPSTREAM_DIR")"
  echo "test_root=$(metadata_path "$UPSTREAM_TEST_DIR")"
} > "$TMP_UPSTREAM_META"

smoke_count=$(find "$TMP_SMOKE_CLASSES_DIR" -name '*.class' | wc -l | tr -d ' ')
upstream_class_count=$(find "$TMP_UPSTREAM_STAGE_DIR" -name '*.class' | wc -l | tr -d ' ')
upstream_source_count=$(find "$TMP_UPSTREAM_STAGE_DIR" \( -name '*.clj' -o -name '*.cljc' \) | wc -l | tr -d ' ')

rm -rf "$SMOKE_CLASSES_DIR" "$UPSTREAM_STAGE_DIR" "$LOCAL_JARS_DIR"
rm -f "$SMOKE_JAR" "$UPSTREAM_JAR" "$JARS_LIST" "$UPSTREAM_META"

mv "$TMP_SMOKE_CLASSES_DIR" "$SMOKE_CLASSES_DIR"
mv "$TMP_UPSTREAM_STAGE_DIR" "$UPSTREAM_STAGE_DIR"
mv "$TMP_LOCAL_JARS_DIR" "$LOCAL_JARS_DIR"
mv "$TMP_SMOKE_JAR" "$SMOKE_JAR"
mv "$TMP_UPSTREAM_JAR" "$UPSTREAM_JAR"
mv "$TMP_JARS_LIST" "$JARS_LIST"
mv "$TMP_UPSTREAM_META" "$UPSTREAM_META"

echo "Built $smoke_count smoke classes -> $SMOKE_JAR"
echo "Built $upstream_class_count upstream classes/resources -> $UPSTREAM_JAR"
echo "Clojure runtime: $jar_count JARs copied into $LOCAL_JARS_DIR and listed in $JARS_LIST"
echo "Upstream source snapshot: $UPSTREAM_TAG ($upstream_source_count source files)"
