# ============================================================
# 199xVM — Central build system
# ============================================================

# --- Versioned JAR filenames (update here when versions change) ---
RAOH_JAR             := raoh-0.4.0.jar
RAOH_JSON_JAR        := raoh-json-0.4.0.jar
JACKSON_ANN_JAR      := jackson-annotations-3.0-rc5.jar
JACKSON_CORE_JAR     := jackson-core-3.1.0.jar
JACKSON_DATABIND_JAR := jackson-databind-3.1.0.jar

JAR_NAMES := $(RAOH_JAR) $(RAOH_JSON_JAR) $(JACKSON_ANN_JAR) \
             $(JACKSON_CORE_JAR) $(JACKSON_DATABIND_JAR)

WEB_JARS := $(addprefix web/,$(JAR_NAMES))

.PHONY: all dev-jars shim test-bundle clj-smoke-bundle clj-smoke-run clj-smoke-test clj-smoke-docker clj-smoke-test-docker javac wasm dist test clean deploy docker-playground dist-docker

# ============================================================
# all — build everything needed for local development
# ============================================================
all: dev-jars shim javac wasm

# ============================================================
# dev-jars — download versioned JARs to web/ via Maven
# ============================================================
web/$(RAOH_JAR):
	mvn dependency:copy \
	  -Dartifact=net.unit8.raoh:raoh:0.4.0 \
	  -DoutputDirectory=web \
	  -Dmdep.stripVersion=false

web/$(RAOH_JSON_JAR):
	mvn dependency:copy \
	  -Dartifact=net.unit8.raoh:raoh-json:0.4.0 \
	  -DoutputDirectory=web \
	  -Dmdep.stripVersion=false

web/$(JACKSON_ANN_JAR):
	mvn dependency:copy \
	  -Dartifact=com.fasterxml.jackson.core:jackson-annotations:3.0-rc5 \
	  -DoutputDirectory=web \
	  -Dmdep.stripVersion=false

web/$(JACKSON_CORE_JAR):
	mvn dependency:copy \
	  -Dartifact=tools.jackson.core:jackson-core:3.1.0 \
	  -DoutputDirectory=web \
	  -Dmdep.stripVersion=false

web/$(JACKSON_DATABIND_JAR):
	mvn dependency:copy \
	  -Dartifact=tools.jackson.core:jackson-databind:3.1.0 \
	  -DoutputDirectory=web \
	  -Dmdep.stripVersion=false

dev-jars: $(WEB_JARS)

# ============================================================
# shim — compile JDK shim classes → jdk-shim/bundle.bin
# ============================================================
jdk-shim/bundle.bin: build-shim.sh tools/BundleWriter.java $(shell find jdk-shim -name '*.java' 2>/dev/null)
	./build-shim.sh

shim: jdk-shim/bundle.bin

# ============================================================
# test-bundle — compile test Java classes → test-classes/bundle.bin
# ============================================================
test-classes/bundle.bin: build-test-bundle.sh tools/BundleWriter.java web/javac.js $(shell find test-sources -name '*.java' 2>/dev/null)
	./build-test-bundle.sh

test-bundle: test-classes/bundle.bin

# ============================================================
# clj-smoke-bundle — compile isolated Clojure smoke classes → clj-smoke/bundle.bin
# ============================================================
clj-smoke/bundle.bin: build-clj-smoke.sh tools/BundleWriter.java test-sources/clojure/deps.edn $(shell find test-sources/clojure -type f 2>/dev/null)
	./build-clj-smoke.sh

clj-smoke-bundle: clj-smoke/bundle.bin

# ============================================================
# clj-smoke-run — run the isolated Clojure smoke bundle against the VM
# ============================================================
clj-smoke-run: clj-smoke/bundle.bin jdk-shim/bundle.bin
	cargo run --package jvm-core --bin run_bundle jdk-shim/bundle.bin clj-smoke/bundle.bin ClojureSmokeEntry run '()Ljava/lang/String;'

# ============================================================
# clj-smoke-test — assert the isolated Clojure smoke bundle returns "ok"
# ============================================================
clj-smoke-test: clj-smoke/bundle.bin jdk-shim/bundle.bin
	@out="$$(cargo run --quiet --package jvm-core --bin run_bundle jdk-shim/bundle.bin clj-smoke/bundle.bin ClojureSmokeEntry run '()Ljava/lang/String;')"; \
	if [ "$$out" != "ok" ]; then \
	  printf '%s\n' "$$out"; \
	  exit 1; \
	fi; \
	printf '%s\n' "$$out"

# ============================================================
# clj-smoke-docker — build + run the isolated Clojure smoke flow in containers
# ============================================================
clj-smoke-docker:
	docker-compose run --rm java make shim
	docker-compose run --rm clj make clj-smoke-bundle
	docker-compose run --rm rust make clj-smoke-run

# ============================================================
# clj-smoke-test-docker — build + assert the isolated Clojure smoke flow in containers
# ============================================================
clj-smoke-test-docker:
	docker-compose run --rm java make shim
	docker-compose run --rm clj make clj-smoke-bundle
	docker-compose run --rm rust make clj-smoke-test

# ============================================================
# javac — build web/javac.js from web/javac.ts
# ============================================================
web/javac.js: $(shell find web -name '*.ts' 2>/dev/null)
	npm run build:javac

javac: web/javac.js

# ============================================================
# wasm — compile Rust core to WebAssembly → jvm-core/pkg/
# ============================================================
jvm-core/pkg/jvm_core_bg.wasm: $(shell find jvm-core/src -name '*.rs' 2>/dev/null) jvm-core/Cargo.toml
	wasm-pack build jvm-core --target web

wasm: jvm-core/pkg/jvm_core_bg.wasm

# ============================================================
# test — run compiler tests (npm test)
# ============================================================
test: web/javac.js
	node --experimental-strip-types --test web/javac.test.ts

# ============================================================
# dist — assemble deployable static files in dist/
# ============================================================
dist: web/javac.js jdk-shim/bundle.bin jvm-core/pkg/jvm_core_bg.wasm $(WEB_JARS)
	rm -rf dist
	mkdir -p dist/pkg dist/bundle
	sed \
	  -e "s/__BUILD_TIMESTAMP__/$$(date +%s)/g" \
	  -e 's|\.\./jvm-core/pkg/|./pkg/|g' \
	  -e 's|\.\./jdk-shim/bundle\.bin|./bundle/shim.bin|g' \
	  web/index.html > dist/index.html
	cp web/javac.js                       dist/javac.js
	cp jvm-core/pkg/jvm_core.js          dist/pkg/jvm_core.js
	cp jvm-core/pkg/jvm_core_bg.wasm     dist/pkg/jvm_core_bg.wasm
	cp jdk-shim/bundle.bin               dist/bundle/shim.bin
	cp web/$(RAOH_JAR)                   dist/$(RAOH_JAR)
	cp web/$(RAOH_JSON_JAR)              dist/$(RAOH_JSON_JAR)
	cp web/$(JACKSON_ANN_JAR)            dist/$(JACKSON_ANN_JAR)
	cp web/$(JACKSON_CORE_JAR)           dist/$(JACKSON_CORE_JAR)
	cp web/$(JACKSON_DATABIND_JAR)       dist/$(JACKSON_DATABIND_JAR)
	@echo ""
	@echo "==> dist/ contents:"
	@find dist -type f | sort
	@echo "Total size: $$(du -sh dist | cut -f1)"
	@echo ""
	@echo "To deploy: make deploy GCS=gs://bucket/path"

# ============================================================
# deploy — upload dist/ to GCS (requires GCS= argument)
# ============================================================
# Usage: make deploy GCS=gs://my-bucket/path
# Compares local MD5 with remote before uploading (incremental).

GCS ?=

deploy: dist
ifndef GCS
	$(error Usage: make deploy GCS=gs://bucket/path)
endif
	@echo "==> Uploading to $(GCS) (incremental) ..."
	@upload_if_changed() { \
	  local src="$$1"; shift; \
	  local dst="$$1"; shift; \
	  local local_md5; \
	  local_md5="$$(md5 -q "$$src" 2>/dev/null || md5sum "$$src" | cut -d' ' -f1)"; \
	  local remote_md5; \
	  remote_md5="$$(gcloud storage objects describe "$$dst" --format='value(md5_hash)' 2>/dev/null || true)"; \
	  local local_md5_b64; \
	  local_md5_b64="$$(printf '%s' "$$local_md5" | xxd -r -p | base64)"; \
	  if [ "$$local_md5_b64" = "$$remote_md5" ]; then \
	    echo "  skip (unchanged): $$(basename "$$src")"; \
	    return 0; \
	  fi; \
	  echo "  upload: $$(basename "$$src")"; \
	  gcloud storage cp "$$src" "$$dst" "$$@"; \
	}; \
	upload_if_changed dist/javac.js                    "$(GCS)/javac.js"                    --cache-control="public,max-age=31536000"; \
	upload_if_changed dist/pkg/jvm_core.js             "$(GCS)/pkg/jvm_core.js"             --cache-control="public,max-age=31536000"; \
	upload_if_changed dist/pkg/jvm_core_bg.wasm        "$(GCS)/pkg/jvm_core_bg.wasm"        --cache-control="public,max-age=31536000" --content-type="application/wasm"; \
	upload_if_changed dist/bundle/shim.bin             "$(GCS)/bundle/shim.bin"             --cache-control="public,max-age=31536000" --content-type="application/octet-stream"; \
	upload_if_changed dist/$(RAOH_JAR)                 "$(GCS)/$(RAOH_JAR)"                 --cache-control="public,max-age=31536000" --content-type="application/java-archive"; \
	upload_if_changed dist/$(RAOH_JSON_JAR)            "$(GCS)/$(RAOH_JSON_JAR)"            --cache-control="public,max-age=31536000" --content-type="application/java-archive"; \
	upload_if_changed dist/$(JACKSON_ANN_JAR)          "$(GCS)/$(JACKSON_ANN_JAR)"          --cache-control="public,max-age=31536000" --content-type="application/java-archive"; \
	upload_if_changed dist/$(JACKSON_CORE_JAR)         "$(GCS)/$(JACKSON_CORE_JAR)"         --cache-control="public,max-age=31536000" --content-type="application/java-archive"; \
	upload_if_changed dist/$(JACKSON_DATABIND_JAR)     "$(GCS)/$(JACKSON_DATABIND_JAR)"     --cache-control="public,max-age=31536000" --content-type="application/java-archive"; \
	echo "  upload: index.html (always)"; \
	gcloud storage cp dist/index.html "$(GCS)/index.html" --cache-control="no-cache" --content-type="text/html; charset=utf-8"
	@echo ""
	@echo "==> Done: $(GCS)/"

# ============================================================
# docker-playground — build all artifacts via Docker then start web server
# ============================================================
# Requires: Docker (or OrbStack). Runs dev-jars, wasm, shim, javac, dist in containers, then docker-compose up (web only).
docker-playground: dist-docker
	@echo "==> Starting web server (docker-compose up)..."
	docker-compose up

# Build dist via Docker (all artifacts built in containers)
dist-docker:
	@echo "==> Building artifacts in Docker..."
	docker-compose run --rm java make dev-jars
	docker-compose run --rm rust make wasm
	docker-compose run --rm java make shim
	docker-compose run --rm node make javac
	docker-compose run --rm node make dist
	@echo "==> dist/ ready."

# ============================================================
# clean — remove generated artifacts
# ============================================================
clean:
	rm -rf dist
	rm -rf jdk-shim/out jdk-shim/bundle.bin
	rm -rf test-classes
	rm -rf test-bench-classes
	rm -rf clj-smoke/target clj-smoke/bundle.bin
	rm -f  web/javac.js
	rm -rf jvm-core/pkg
