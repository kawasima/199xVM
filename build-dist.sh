#!/usr/bin/env bash
# build-dist.sh — assemble deployable static files into dist/ and upload to GCS
#
# Usage:
#   ./build-dist.sh                        # build only
#   ./build-dist.sh gs://my-bucket/1       # build + deploy to GCS path

set -euo pipefail

DIST="dist"
GCS_TARGET="${1:-}"   # e.g. gs://unit8-net/1

echo "==> Cleaning $DIST/"
rm -rf "$DIST"
mkdir -p "$DIST/pkg" "$DIST/bundle"

echo "==> Copying web assets..."
cp web/index.html "$DIST/index.html"
cp web/javac.js   "$DIST/javac.js"

echo "==> Copying WASM package..."
cp jvm-core/pkg/jvm_core.js       "$DIST/pkg/jvm_core.js"
cp jvm-core/pkg/jvm_core_bg.wasm  "$DIST/pkg/jvm_core_bg.wasm"

echo "==> Copying class bundles..."
cp jdk-shim/bundle.bin      "$DIST/bundle/shim.bin"
cp raoh-classes/bundle.bin  "$DIST/bundle/raoh.bin"

echo "==> Patching paths in index.html..."
sed -i '' \
  -e 's|../jvm-core/pkg/jvm_core.js|./pkg/jvm_core.js|g' \
  -e 's|../jdk-shim/bundle.bin|./bundle/shim.bin|g' \
  -e 's|../raoh-classes/bundle.bin|./bundle/raoh.bin|g' \
  "$DIST/index.html"

echo ""
echo "==> dist/ contents:"
find "$DIST" -type f | sort
echo "Total size: $(du -sh "$DIST" | cut -f1)"

if [ -z "$GCS_TARGET" ]; then
  echo ""
  echo "To deploy: ./build-dist.sh gs://unit8.net/199xVM"
  exit 0
fi

echo ""
echo "==> Uploading to $GCS_TARGET ..."

# Upload static assets with long cache (1 year)
gcloud storage cp "$DIST/javac.js"                "${GCS_TARGET}/javac.js"               --cache-control="public,max-age=31536000"
gcloud storage cp "$DIST/pkg/jvm_core.js"         "${GCS_TARGET}/pkg/jvm_core.js"        --cache-control="public,max-age=31536000"
gcloud storage cp "$DIST/pkg/jvm_core_bg.wasm"    "${GCS_TARGET}/pkg/jvm_core_bg.wasm"   --cache-control="public,max-age=31536000" --content-type="application/wasm"
gcloud storage cp "$DIST/bundle/shim.bin"         "${GCS_TARGET}/bundle/shim.bin"        --cache-control="public,max-age=31536000" --content-type="application/octet-stream"
gcloud storage cp "$DIST/bundle/raoh.bin"         "${GCS_TARGET}/bundle/raoh.bin"        --cache-control="public,max-age=31536000" --content-type="application/octet-stream"

# Upload index.html with no-cache (always fresh)
gcloud storage cp "$DIST/index.html"              "${GCS_TARGET}/index.html"             --cache-control="no-cache" --content-type="text/html; charset=utf-8"

echo ""
echo "==> Done: ${GCS_TARGET}/"
