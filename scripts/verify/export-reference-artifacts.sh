#!/usr/bin/env bash
set -euo pipefail

BASE_URL="${RUSTOK_BASE_URL:-http://127.0.0.1:5150}"
OUT_DIR="${1:-artifacts/reference}"
TIMESTAMP="$(date -u +%Y%m%dT%H%M%SZ)"
TARGET_DIR="${OUT_DIR%/}/${TIMESTAMP}"
OPENAPI_DIR="${TARGET_DIR}/openapi"
GRAPHQL_DIR="${TARGET_DIR}/graphql"
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
INTROSPECTION_PAYLOAD=""

cleanup() {
  if [[ -n "${INTROSPECTION_PAYLOAD}" && -f "${INTROSPECTION_PAYLOAD}" ]]; then
    rm -f "${INTROSPECTION_PAYLOAD}"
  fi
}
trap cleanup EXIT

mkdir -p "$OPENAPI_DIR" "$GRAPHQL_DIR"

if [[ "${SKIP_RUSTDOC:-0}" != "1" ]]; then
  echo "[reference] generating rustdoc JSON artifacts"
  cargo doc --no-deps -p rustok-server -p rustok-workflow
fi

echo "[reference] exporting OpenAPI JSON/YAML from ${BASE_URL}"
curl -fsS "${BASE_URL}/api/openapi.json" -o "${OPENAPI_DIR}/openapi.json"
curl -fsS "${BASE_URL}/api/openapi.yaml" -o "${OPENAPI_DIR}/openapi.yaml"

echo "[reference] exporting GraphQL schema SDL"
curl -fsS "${BASE_URL}/api/graphql/schema.graphql" -o "${GRAPHQL_DIR}/schema.graphql"

echo "[reference] exporting full GraphQL schema introspection"
INTROSPECTION_PAYLOAD="$(mktemp)"
python3 - "$SCRIPT_DIR/graphql-introspection-query.graphql" "$INTROSPECTION_PAYLOAD" <<'PY'
import json
import sys
from pathlib import Path

query = Path(sys.argv[1]).read_text(encoding="utf-8")
Path(sys.argv[2]).write_text(json.dumps({"query": query}), encoding="utf-8")
PY
curl -fsS "${BASE_URL}/api/graphql" \
  -H 'content-type: application/json' \
  --data-binary "@${INTROSPECTION_PAYLOAD}" \
  -o "${GRAPHQL_DIR}/introspection.json"
cleanup
INTROSPECTION_PAYLOAD=""

echo "[reference] writing manifest"
GIT_COMMIT="$(git rev-parse HEAD 2>/dev/null || true)"
cat > "${TARGET_DIR}/manifest.json" <<MANIFEST
{
  "schema": "rustok.reference_artifacts.v1",
  "created_at_utc": "${TIMESTAMP}",
  "base_url": "${BASE_URL}",
  "git_commit": "${GIT_COMMIT}",
  "rustdoc_skipped": "${SKIP_RUSTDOC:-0}",
  "files": [
    "openapi/openapi.json",
    "openapi/openapi.yaml",
    "graphql/introspection.json",
    "graphql/schema.graphql"
  ]
}
MANIFEST

cat > "${TARGET_DIR}/manifest.txt" <<MANIFEST
created_at_utc=${TIMESTAMP}
base_url=${BASE_URL}
git_commit=${GIT_COMMIT}
rustdoc_skipped=${SKIP_RUSTDOC:-0}
files=openapi/openapi.json,openapi/openapi.yaml,graphql/introspection.json,graphql/schema.graphql,manifest.json
MANIFEST

echo "[reference] verifying artifact completeness"
node "${SCRIPT_DIR}/verify-reference-artifacts.mjs" "${TARGET_DIR}"

echo "[reference] done: ${TARGET_DIR}"
