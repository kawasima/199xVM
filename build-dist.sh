#!/usr/bin/env bash
# build-dist.sh — assemble deployable static files into dist/ and upload to GCS
#
# Usage:
#   ./build-dist.sh                        # build only
#   ./build-dist.sh gs://my-bucket/1       # build + deploy to GCS path

set -euo pipefail

DIST="dist"
GCS_TARGET="${1:-}"   # e.g. gs://mapper/bucket
BUILD_TS="$(date +%s)"
M2_REPO="${HOME}/.m2/repository"

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

echo "==> Cleaning $DIST/"
rm -rf "$DIST"
mkdir -p "$DIST/pkg" "$DIST/bundle"

echo "==> Copying web assets..."
sed "s/__BUILD_TIMESTAMP__/${BUILD_TS}/g" web/index.html > "$DIST/index.html"
cp web/javac.js   "$DIST/javac.js"

echo "==> Copying WASM package..."
cp jvm-core/pkg/jvm_core.js       "$DIST/pkg/jvm_core.js"
cp jvm-core/pkg/jvm_core_bg.wasm  "$DIST/pkg/jvm_core_bg.wasm"

echo "==> Copying class bundles..."
cp jdk-shim/bundle.bin      "$DIST/bundle/shim.bin"

echo "==> Copying Raoh jars from ~/.m2..."
RAOH_JAR="$(find_latest_artifact_jar "net/unit8/raoh" "raoh" || true)"
RAOH_JSON_JAR="$(find_latest_artifact_jar "net/unit8/raoh" "raoh-json" || true)"
JACKSON_DATABIND_JAR="$(find_preferred_or_latest_jar "tools/jackson/core" "jackson-databind" "3.0.0-rc4" || true)"
JACKSON_CORE_JAR="$(find_preferred_or_latest_jar "tools/jackson/core" "jackson-core" "3.0.0-rc4" || true)"
JACKSON_ANNOTATIONS_JAR="$(find_preferred_or_latest_jar "com/fasterxml/jackson/core" "jackson-annotations" "3.0-rc4" || true)"
if [ -z "${RAOH_JAR}" ]; then
  echo "Error: raoh jar not found in ${M2_REPO}/net/unit8/raoh/raoh"
  exit 1
fi
if [ -z "${RAOH_JSON_JAR}" ]; then
  echo "Error: raoh-json jar not found in ${M2_REPO}/net/unit8/raoh/raoh-json"
  exit 1
fi
if [ -z "${JACKSON_DATABIND_JAR}" ]; then
  echo "Error: jackson-databind jar not found in ${M2_REPO}/tools/jackson/core/jackson-databind"
  exit 1
fi
if [ -z "${JACKSON_CORE_JAR}" ]; then
  echo "Error: jackson-core jar not found in ${M2_REPO}/tools/jackson/core/jackson-core"
  exit 1
fi
if [ -z "${JACKSON_ANNOTATIONS_JAR}" ]; then
  echo "Error: jackson-annotations jar not found in ${M2_REPO}/com/fasterxml/jackson/core/jackson-annotations"
  exit 1
fi
cp "$RAOH_JAR" "$DIST/raoh.jar"
cp "$RAOH_JSON_JAR" "$DIST/raoh-json.jar"
cp "$JACKSON_DATABIND_JAR" "$DIST/jackson-databind.jar"
cp "$JACKSON_CORE_JAR" "$DIST/jackson-core.jar"
cp "$JACKSON_ANNOTATIONS_JAR" "$DIST/jackson-annotations.jar"
echo "  - raoh      : $RAOH_JAR"
echo "  - raoh-json : $RAOH_JSON_JAR"
echo "  - jackson-databind   : $JACKSON_DATABIND_JAR"
echo "  - jackson-core       : $JACKSON_CORE_JAR"
echo "  - jackson-annotations: $JACKSON_ANNOTATIONS_JAR"

echo "==> Patching paths in index.html..."
sed -i '' \
  -e 's|../jvm-core/pkg/|./pkg/|g' \
  -e 's|../jdk-shim/bundle.bin|./bundle/shim.bin|g' \
  "$DIST/index.html"

echo ""
echo "==> dist/ contents:"
find "$DIST" -type f | sort
echo "Total size: $(du -sh "$DIST" | cut -f1)"

if [ -z "$GCS_TARGET" ]; then
  echo ""
  echo "To deploy: ./build-dist.sh gs://mapper/bucket"
  exit 0
fi

echo ""
echo "==> Uploading to $GCS_TARGET ..."

# Upload static assets with long cache (1 year)
gcloud storage cp "$DIST/javac.js"                "${GCS_TARGET}/javac.js"               --cache-control="public,max-age=31536000"
gcloud storage cp "$DIST/pkg/jvm_core.js"         "${GCS_TARGET}/pkg/jvm_core.js"        --cache-control="public,max-age=31536000"
gcloud storage cp "$DIST/pkg/jvm_core_bg.wasm"    "${GCS_TARGET}/pkg/jvm_core_bg.wasm"   --cache-control="public,max-age=31536000" --content-type="application/wasm"
gcloud storage cp "$DIST/bundle/shim.bin"         "${GCS_TARGET}/bundle/shim.bin"        --cache-control="public,max-age=31536000" --content-type="application/octet-stream"
gcloud storage cp "$DIST/raoh.jar"                "${GCS_TARGET}/raoh.jar"               --cache-control="public,max-age=31536000" --content-type="application/java-archive"
gcloud storage cp "$DIST/raoh-json.jar"           "${GCS_TARGET}/raoh-json.jar"          --cache-control="public,max-age=31536000" --content-type="application/java-archive"
gcloud storage cp "$DIST/jackson-databind.jar"    "${GCS_TARGET}/jackson-databind.jar"   --cache-control="public,max-age=31536000" --content-type="application/java-archive"
gcloud storage cp "$DIST/jackson-core.jar"        "${GCS_TARGET}/jackson-core.jar"       --cache-control="public,max-age=31536000" --content-type="application/java-archive"
gcloud storage cp "$DIST/jackson-annotations.jar" "${GCS_TARGET}/jackson-annotations.jar" --cache-control="public,max-age=31536000" --content-type="application/java-archive"

# Upload index.html with no-cache (always fresh)
gcloud storage cp "$DIST/index.html"              "${GCS_TARGET}/index.html"             --cache-control="no-cache" --content-type="text/html; charset=utf-8"

echo ""
echo "==> Done: ${GCS_TARGET}/"
