#!/usr/bin/env bash

set -euo pipefail

ROOT_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)"
RUNNER_FILE="$ROOT_DIR/test-sources/clojure/src/upstream/runner.clj"
UPSTREAM_INFO_FILE="$ROOT_DIR/clj-smoke/clojure-upstream.txt"

percent() {
  awk -v n="$1" -v d="$2" 'BEGIN { if (d == 0) printf "0.0"; else printf "%.1f", (100.0 * n / d) }'
}

if [[ ! -f "$RUNNER_FILE" ]]; then
  echo "missing runner: $RUNNER_FILE" >&2
  exit 1
fi

checkout=""
test_root=""
if [[ -f "$UPSTREAM_INFO_FILE" ]]; then
  checkout="$(sed -n 's/^checkout=//p' "$UPSTREAM_INFO_FILE" | head -n 1)"
  test_root="$(sed -n 's/^test_root=//p' "$UPSTREAM_INFO_FILE" | head -n 1)"
fi

if [[ -z "$checkout" ]]; then
  checkout="${CLOJURE_UPSTREAM_DIR:-/tmp/clojure-upstream}"
fi
if [[ -z "$test_root" ]]; then
  test_root="$checkout/test"
fi

if [[ "$checkout" != /* ]]; then
  checkout="$ROOT_DIR/$checkout"
fi
if [[ "$test_root" != /* ]]; then
  test_root="$ROOT_DIR/$test_root"
fi

suite_root="$test_root/clojure/test_clojure"
if [[ ! -d "$suite_root" ]]; then
  echo "missing upstream test root: $suite_root" >&2
  echo "run ./build-clj-smoke.sh first or set CLOJURE_UPSTREAM_DIR" >&2
  exit 1
fi

mapfile -t selected_namespaces < <(
  rg --no-filename -o 'clojure\.test-clojure\.[[:alnum:].-]+' "$RUNNER_FILE" | awk '!seen[$0]++'
)

if [[ ${#selected_namespaces[@]} -eq 0 ]]; then
  echo "no selected namespaces found in $RUNNER_FILE" >&2
  exit 1
fi

missing_files=()
selected_details=()
selected_deftests=0

for ns in "${selected_namespaces[@]}"; do
  relative="${ns#clojure.test-clojure.}"
  relative="${relative//./\/}"
  file="$suite_root/${relative//-/_}.clj"
  if [[ ! -f "$file" ]]; then
    missing_files+=("$file")
    continue
  fi
  deftests="$(rg -n '^\(deftest' "$file" | wc -l | tr -d ' ')"
  selected_deftests=$((selected_deftests + deftests))
  selected_details+=("$ns:$deftests")
done

if [[ ${#missing_files[@]} -gt 0 ]]; then
  printf 'missing selected files:\n' >&2
  printf '  %s\n' "${missing_files[@]}" >&2
  exit 1
fi

total_namespaces="$(
  rg --no-filename -o '^\(ns clojure\.test-clojure\.[[:alnum:].-]+' "$suite_root" -g '*.clj' |
    awk '{print $2}' |
    sort -u |
    wc -l |
    tr -d ' '
)"
selected_namespace_count="${#selected_namespaces[@]}"

total_deftests="$(rg -n '^\(deftest' "$suite_root" -g '*.clj' | wc -l | tr -d ' ')"

echo "upstream_checkout=$checkout"
echo "suite_root=$suite_root"
echo "selected_namespaces=$selected_namespace_count/$total_namespaces ($(percent "$selected_namespace_count" "$total_namespaces")%)"
echo "selected_deftests=$selected_deftests/$total_deftests ($(percent "$selected_deftests" "$total_deftests")%)"
echo "selected_namespace_list=${selected_namespaces[*]}"
printf 'selected_deftests_by_namespace='
first=1
for detail in "${selected_details[@]}"; do
  if [[ $first -eq 0 ]]; then
    printf ','
  fi
  first=0
  printf '%s' "$detail"
done
printf '\n'
echo "note=this is suite-selection coverage, not line/branch coverage"
