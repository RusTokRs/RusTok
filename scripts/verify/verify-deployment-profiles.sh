#!/usr/bin/env bash
# RusTok - deployment profile smoke validation
# Verifies the supported server build/runtime surfaces:
# - monolith
# - server+admin
# - headless-api
# - registry-only host mode
set -uo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT_DIR="$(cd "$SCRIPT_DIR/../.." && pwd)"
cd "$ROOT_DIR"

RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m'
BOLD='\033[1m'

ERRORS=0

header() { echo -e "\n${BOLD}=== $1 ===${NC}"; }
pass()   { echo -e "  ${GREEN}PASS${NC} $1"; }
fail()   { echo -e "  ${RED}FAIL${NC} $1"; ERRORS=$((ERRORS + 1)); }
skip()   { echo -e "  ${YELLOW}SKIP${NC} $1"; }
run_cmd() {
    local label="$1"
    shift
    if "$@"; then
        pass "$label"
    else
        fail "$label"
    fi
}

curl_status() {
    local method="$1"
    local url="$2"
    local body="${3:-}"

    if [[ -n "$body" ]]; then
        curl -sS --max-time 20 -o /dev/null -w "%{http_code}" \
            -X "$method" -H "content-type: application/json" --data "$body" "$url"
    else
        curl -sS --max-time 20 -o /dev/null -w "%{http_code}" -X "$method" "$url"
    fi
}

curl_capture() {
    local method="$1"
    local url="$2"
    local headers_file="$3"
    local body_file="$4"

    curl -sS --max-time 20 -D "$headers_file" -o "$body_file" -X "$method" "$url"
}

status_is() {
    local method="$1"
    local url="$2"
    local expected="$3"
    local body="${4:-}"
    [[ "$(curl_status "$method" "$url" "$body")" == "$expected" ]]
}

header_present() {
    local url="$1"
    local header_name="$2"
    local headers_file="$3"
    local body_file="$4"

    curl_capture GET "$url" "$headers_file" "$body_file" &&
        grep -Eiq "^${header_name}:" "$headers_file"
}

body_matches() {
    local url="$1"
    local pattern="$2"
    local body_file="$3"
    local headers_file="$4"

    curl_capture GET "$url" "$headers_file" "$body_file" &&
        grep -Eiq "$pattern" "$body_file"
}

body_not_contains() {
    local url="$1"
    local pattern="$2"
    local body_file="$3"
    local headers_file="$4"

    curl_capture GET "$url" "$headers_file" "$body_file" &&
        ! grep -Eiq "$pattern" "$body_file"
}

header "Deployment profile smoke validation"

run_cmd \
  "monolith cargo check" \
  cargo check --manifest-path "$ROOT_DIR/Cargo.toml" -p rustok-server --lib --bins

run_cmd \
  "monolith startup smoke" \
  cargo test --manifest-path "$ROOT_DIR/Cargo.toml" -p rustok-server \
    app::tests::startup_smoke_builds_router_and_runtime_shared_state --lib

run_cmd \
  "server+admin cargo check" \
  cargo check --manifest-path "$ROOT_DIR/Cargo.toml" -p rustok-server --lib --bins \
    --no-default-features --features redis-cache,embed-admin

run_cmd \
  "server+admin router smoke" \
  cargo test --manifest-path "$ROOT_DIR/Cargo.toml" -p rustok-server \
    services::app_router::tests::mount_application_shell_supports_server_with_admin_profile --lib \
    --no-default-features --features redis-cache,embed-admin

run_cmd \
  "headless-api cargo check" \
  cargo check --manifest-path "$ROOT_DIR/Cargo.toml" -p rustok-server --lib --bins \
    --no-default-features --features redis-cache

run_cmd \
  "headless-api router smoke" \
  cargo test --manifest-path "$ROOT_DIR/Cargo.toml" -p rustok-server \
    services::app_router::tests::mount_application_shell_skips_admin_and_storefront_for_headless_profile --lib \
    --no-default-features --features redis-cache

run_cmd \
  "registry-only env override parse" \
  cargo test --manifest-path "$ROOT_DIR/Cargo.toml" -p rustok-server \
    common::settings::tests::env_overrides_runtime_host_mode --lib \
    --no-default-features --features redis-cache

run_cmd \
  "registry-only runtime smoke" \
  cargo test --manifest-path "$ROOT_DIR/Cargo.toml" -p rustok-server \
    app::tests::registry_only_host_mode_limits_exposed_surface --lib \
    --no-default-features --features redis-cache

run_cmd \
  "registry v1 detail smoke" \
  cargo test --manifest-path "$ROOT_DIR/Cargo.toml" -p rustok-server \
    app::tests::registry_catalog_detail_endpoint_serves_module_contract --lib \
    --no-default-features --features redis-cache

run_cmd \
  "registry v1 cache smoke" \
  cargo test --manifest-path "$ROOT_DIR/Cargo.toml" -p rustok-server \
    app::tests::registry_catalog_endpoint_honors_if_none_match --lib \
    --no-default-features --features redis-cache

run_cmd \
  "registry-only openapi smoke" \
  cargo test --manifest-path "$ROOT_DIR/Cargo.toml" -p rustok-server \
    controllers::swagger::tests::registry_only_openapi_filters_non_registry_surface --lib \
    --no-default-features --features redis-cache

if [[ -n "${RUSTOK_REGISTRY_BASE_URL:-}" ]]; then
  header "External registry-only smoke"

  BASE_URL="${RUSTOK_REGISTRY_BASE_URL%/}"
  SMOKE_SLUG="${RUSTOK_REGISTRY_SMOKE_SLUG:-blog}"
  EVIDENCE_DIR="${RUSTOK_REGISTRY_EVIDENCE_DIR:-}"
  if [[ -n "$EVIDENCE_DIR" ]]; then
    mkdir -p "$EVIDENCE_DIR"
    TMP_DIR="$EVIDENCE_DIR"
  else
    TMP_DIR="$(mktemp -d)"
    trap 'rm -rf "$TMP_DIR"' EXIT
  fi

  run_cmd \
    "external /health/ready returns 200" \
    status_is GET "$BASE_URL/health/ready" 200

  run_cmd \
    "external /health/modules returns 200" \
    status_is GET "$BASE_URL/health/modules" 200

  run_cmd \
    "external /health/runtime advertises registry_only" \
    body_matches "$BASE_URL/health/runtime" '"host_mode"[[:space:]]*:[[:space:]]*"registry_only"' \
      "$TMP_DIR/runtime-body.json" "$TMP_DIR/runtime-headers.txt"

  run_cmd \
    "external /health/runtime disables runtime dependencies" \
    body_matches "$BASE_URL/health/runtime" '"runtime_dependencies_enabled"[[:space:]]*:[[:space:]]*false' \
      "$TMP_DIR/runtime-body.json" "$TMP_DIR/runtime-headers.txt"

  run_cmd \
    "external /v1/catalog exposes ETag" \
    header_present "$BASE_URL/v1/catalog?limit=1" "etag" \
      "$TMP_DIR/catalog-headers.txt" "$TMP_DIR/catalog-body.json"

  run_cmd \
    "external /v1/catalog exposes Cache-Control" \
    header_present "$BASE_URL/v1/catalog?limit=1" "cache-control" \
      "$TMP_DIR/catalog-headers.txt" "$TMP_DIR/catalog-body.json"

  run_cmd \
    "external /v1/catalog exposes X-Total-Count" \
    header_present "$BASE_URL/v1/catalog?limit=1" "x-total-count" \
      "$TMP_DIR/catalog-headers.txt" "$TMP_DIR/catalog-body.json"

  run_cmd \
    "external /v1/catalog/{slug} returns 200" \
    status_is GET "$BASE_URL/v1/catalog/$SMOKE_SLUG" 200

  run_cmd \
    "external reduced OpenAPI keeps catalog detail path" \
    body_matches "$BASE_URL/api/openapi.json" '"/v1/catalog/\{slug\}"' \
      "$TMP_DIR/openapi-body.json" "$TMP_DIR/openapi-headers.txt"

  run_cmd \
    "external reduced OpenAPI YAML keeps catalog detail path" \
    body_matches "$BASE_URL/api/openapi.yaml" '/v1/catalog/\{slug\}' \
      "$TMP_DIR/openapi-yaml-body.yaml" "$TMP_DIR/openapi-yaml-headers.txt"

  run_cmd \
    "external reduced OpenAPI hides V2 publish routes" \
    body_not_contains "$BASE_URL/api/openapi.json" '"/v2/catalog/publish"' \
      "$TMP_DIR/openapi-body.json" "$TMP_DIR/openapi-headers.txt"

  run_cmd \
    "external reduced OpenAPI hides GraphQL/auth routes" \
    body_not_contains "$BASE_URL/api/openapi.json" '"/api/graphql"|"/api/auth/login"' \
      "$TMP_DIR/openapi-body.json" "$TMP_DIR/openapi-headers.txt"

  run_cmd \
    "external write publish path returns 404" \
    status_is POST "$BASE_URL/v2/catalog/publish" 404 '{}'

  run_cmd \
    "external write validate path returns 404" \
    status_is POST "$BASE_URL/v2/catalog/publish/rpr_smoke/validate" 404 '{}'

  run_cmd \
    "external write stages path returns 404" \
    status_is POST "$BASE_URL/v2/catalog/publish/rpr_smoke/stages" 404 '{}'

  run_cmd \
    "external request-changes path returns 404" \
    status_is POST "$BASE_URL/v2/catalog/publish/rpr_smoke/request-changes" 404 '{}'

  run_cmd \
    "external hold path returns 404" \
    status_is POST "$BASE_URL/v2/catalog/publish/rpr_smoke/hold" 404 '{}'

  run_cmd \
    "external resume path returns 404" \
    status_is POST "$BASE_URL/v2/catalog/publish/rpr_smoke/resume" 404 '{}'

  run_cmd \
    "external runner claim path returns 404" \
    status_is POST "$BASE_URL/v2/catalog/runner/claim" 404 '{}'

  run_cmd \
    "external owner-transfer path returns 404" \
    status_is POST "$BASE_URL/v2/catalog/owner-transfer" 404 '{}'

  run_cmd \
    "external yank path returns 404" \
    status_is POST "$BASE_URL/v2/catalog/yank" 404 '{}'

  run_cmd \
    "external /admin returns 404" \
    status_is GET "$BASE_URL/admin" 404

  if [[ -n "$EVIDENCE_DIR" ]]; then
    {
      echo "base_url=$BASE_URL"
      echo "smoke_slug=$SMOKE_SLUG"
      echo "captured_at_utc=$(date -u +"%Y-%m-%dT%H:%M:%SZ")"
    } > "$EVIDENCE_DIR/registry-smoke-metadata.txt"
    pass "external smoke evidence saved to $EVIDENCE_DIR"
  fi
else
  header "External registry-only smoke"
  skip "set RUSTOK_REGISTRY_BASE_URL=https://modules.rustok.dev to verify a deployed dedicated catalog host"
fi

echo ""
if [[ $ERRORS -eq 0 ]]; then
    echo -e "${GREEN}${BOLD}All deployment profile smoke checks passed.${NC}"
    exit 0
fi

echo -e "${RED}${BOLD}$ERRORS deployment profile check(s) failed.${NC}"
exit 1
