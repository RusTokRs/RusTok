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
run_cmd() {
    local label="$1"
    shift
    if "$@"; then
        pass "$label"
    else
        fail "$label"
    fi
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

echo ""
if [[ $ERRORS -eq 0 ]]; then
    echo -e "${GREEN}${BOLD}All deployment profile smoke checks passed.${NC}"
    exit 0
fi

echo -e "${RED}${BOLD}$ERRORS deployment profile check(s) failed.${NC}"
exit 1
