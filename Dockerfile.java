# 199xVM — JDK 25 + Maven + Node (shim, dev-jars, test-bundle). Node is required for build-test-bundle.sh (compile-compact.mjs).
FROM eclipse-temurin:25-jdk

RUN apt-get update && apt-get install -y --no-install-recommends \
    make \
    maven \
    nodejs \
    npm \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /app

# Default: keep container alive for docker-compose run; override with make shim / make dev-jars / make test-bundle
CMD ["sleep", "infinity"]
