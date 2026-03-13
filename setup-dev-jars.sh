#!/usr/bin/env bash
# setup-dev-jars.sh — symlink JAR files from ~/.m2 into web/ for local development.
#
# Usage:
#   ./setup-dev-jars.sh
#
# After running this, `npx serve .` will serve the JARs at the paths
# the frontend expects (e.g. ./raoh.jar, ./jackson-core.jar).

set -euo pipefail

M2_REPO="${HOME}/.m2/repository"
WEB_DIR="web"

find_latest_artifact_jar() {
  local group_path="$1"
  local artifact="$2"
  local base="${M2_REPO}/${group_path}/${artifact}"
  if [ ! -d "$base" ]; then
    return 1
  fi
  find "$base" -type f -name "${artifact}-*.jar" \
    | grep -Ev -- '-(sources|javadoc)\.jar$' \
    | sort -V \
    | tail -n 1
}

find_preferred_or_latest_jar() {
  local group_path="$1"
  local artifact="$2"
  local preferred_version="$3"
  local preferred="${M2_REPO}/${group_path}/${artifact}/${preferred_version}/${artifact}-${preferred_version}.jar"
  if [ -f "$preferred" ]; then
    echo "$preferred"
    return 0
  fi
  find_latest_artifact_jar "$group_path" "$artifact"
}

link_jar() {
  local jar_path="$1"
  local link_name="$2"
  local target="${WEB_DIR}/${link_name}"

  if [ -z "$jar_path" ] || [ ! -f "$jar_path" ]; then
    echo "  SKIP  ${link_name} (not found in ${M2_REPO})"
    return 1
  fi

  # Remove existing file/link
  rm -f "$target"
  ln -s "$jar_path" "$target"
  echo "  OK    ${link_name} -> ${jar_path}"
}

echo "Setting up development JARs in ${WEB_DIR}/..."
echo ""

RAOH_JAR="$(find_latest_artifact_jar "net/unit8/raoh" "raoh" || true)"
RAOH_JSON_JAR="$(find_latest_artifact_jar "net/unit8/raoh" "raoh-json" || true)"
JACKSON_DATABIND_JAR="$(find_preferred_or_latest_jar "tools/jackson/core" "jackson-databind" "3.0.0-rc4" || true)"
JACKSON_CORE_JAR="$(find_preferred_or_latest_jar "tools/jackson/core" "jackson-core" "3.0.0-rc4" || true)"
JACKSON_ANNOTATIONS_JAR="$(find_preferred_or_latest_jar "com/fasterxml/jackson/core" "jackson-annotations" "3.0-rc4" || true)"

ok=0
fail=0

link_jar "$RAOH_JAR" "raoh.jar" && ok=$((ok+1)) || fail=$((fail+1))
link_jar "$RAOH_JSON_JAR" "raoh-json.jar" && ok=$((ok+1)) || fail=$((fail+1))
link_jar "$JACKSON_CORE_JAR" "jackson-core.jar" && ok=$((ok+1)) || fail=$((fail+1))
link_jar "$JACKSON_DATABIND_JAR" "jackson-databind.jar" && ok=$((ok+1)) || fail=$((fail+1))
link_jar "$JACKSON_ANNOTATIONS_JAR" "jackson-annotations.jar" && ok=$((ok+1)) || fail=$((fail+1))

echo ""
echo "Done: ${ok} linked, ${fail} skipped."

if [ "$fail" -gt 0 ]; then
  echo ""
  echo "Missing JARs need to be installed to Maven local repository first."
  echo "For raoh:    cd /path/to/raoh && mvn install -DskipTests"
  echo "For jackson: mvn dependency:copy -Dartifact=tools.jackson.core:jackson-databind:3.0.0-rc4"
fi
