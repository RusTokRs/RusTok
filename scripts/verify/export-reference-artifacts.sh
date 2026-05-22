#!/usr/bin/env bash
set -euo pipefail

BASE_URL="${RUSTOK_BASE_URL:-http://127.0.0.1:5150}"
OUT_DIR="${1:-artifacts/reference}"
TIMESTAMP="$(date -u +%Y%m%dT%H%M%SZ)"
TARGET_DIR="${OUT_DIR%/}/${TIMESTAMP}"

mkdir -p "$TARGET_DIR"

if [[ "${SKIP_RUSTDOC:-0}" != "1" ]]; then
  echo "[reference] generating rustdoc JSON artifacts"
  cargo doc --no-deps -p rustok-server -p rustok-workflow
fi

echo "[reference] exporting OpenAPI JSON/YAML from ${BASE_URL}"
curl -fsS "${BASE_URL}/api/openapi.json" -o "${TARGET_DIR}/openapi.json"
curl -fsS "${BASE_URL}/api/openapi.yaml" -o "${TARGET_DIR}/openapi.yaml"

echo "[reference] exporting GraphQL schema introspection"
curl -fsS "${BASE_URL}/api/graphql" \
  -H 'content-type: application/json' \
  --data '{"query":"query IntrospectionQuery { __schema { types { name } } }"}' \
  -o "${TARGET_DIR}/graphql-introspection.json"

echo "[reference] writing manifest"
cat > "${TARGET_DIR}/manifest.txt" <<MANIFEST
created_at_utc=${TIMESTAMP}
base_url=${BASE_URL}
rustdoc_skipped=${SKIP_RUSTDOC:-0}
files=openapi.json,openapi.yaml,graphql-introspection.json
MANIFEST

echo "[reference] done: ${TARGET_DIR}"
