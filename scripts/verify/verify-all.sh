#!/usr/bin/env bash
# RusTok — Master verification runner
# Запускает все скрипты верификации и выводит итоговый отчёт
set -uo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"

RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m'
BOLD='\033[1m'

# ─── Parse args ───
VERBOSE=${VERBOSE:-0}
SELECTED_SCRIPT=""

usage() {
    echo "Usage: $0 [OPTIONS] [SCRIPT_NAME]"
    echo ""
    echo "Options:"
    echo "  -v, --verbose    Show full output from each script"
    echo "  -h, --help       Show this help"
    echo ""
    echo "Scripts (run individually):"
    echo "  tenant-isolation   Check tenant_id in queries, entities, migrations"
    echo "  tenant-resolution-architecture  Verify typed tenant resolution ownership"
    echo "  unsafe-code        Check unwrap, panic, blocking ops, println, global state"
    echo "  rbac-coverage      Check RBAC extractors on handlers/resolvers"
    echo "  api-quality        Check GraphQL/REST quality, N+1, OpenAPI, parity"
    echo "  api-surface-contract  Check manifest-driven GraphQL/REST composition without compiling"
    echo "  api-compatibility-self-test  Verify OpenAPI/GraphQL semantic comparator fixtures"
    echo "  api-compatibility-exception-approval-self-test  Verify breaking-exception approval policy fixtures"
    echo "  api-compatibility-infra-self-test  Verify contract-infrastructure approval policy fixtures"
    echo "  api-compatibility-exceptions-local  Verify local API compatibility exception register"
    echo "  api-compatibility-contract  Verify API exporter, diff workflow and exception governance"
    echo "  migration-plan-self-test  Verify append-only migration-plan comparator fixtures"
    echo "  migration-backfill-self-test  Verify appended-migration backfill contract fixtures"
    echo "  migration-infra-self-test  Verify migration-harness approval policy fixtures"
    echo "  repository-ruleset-self-test  Verify required migration approval ruleset fixtures"
    echo "  migration-compatibility-contract  Verify fresh/incremental/N-1 migration harness structure"
    echo "  release-tooling-self-test  Verify deterministic release tooling fixtures"
    echo "  release-infra-self-test  Verify release-infrastructure approval policy fixtures"
    echo "  release-supply-chain-contract  Verify signed, reproducible and attested release structure"
    echo "  rust-host-browser-contract  Verify embedded Rust UI browser smoke structure"
    echo "  development-container-topology  Verify truthful Rust development container topology"
    echo "  events             Check event publishing, DLQ, outbox, transport, serde"
    echo "  code-quality       Check PII, secrets, metrics, dependencies, observability"
    echo "  security           Check argon2, headers, CORS, SSRF, JWT, rate limiting"
    echo "  architecture       Check module registry, Axum composition, MCP, DI, telemetry"
    echo "  deployment-profiles  Smoke-check monolith, server+admin, headless-api builds"
    echo "  anti-bypass       Audit domain bypass patterns and duplicated business logic"
    echo "  storefront-module-routes  Verify storefront module route contract"
    echo "  i18n-contract     Verify i18n contract drift (repo-side)"
    echo "  ui-i18n-parity    Verify module UI i18n parity"
    echo "  flex-multilingual-contract  Verify Flex multilingual live contract guardrails"
    echo "  runtime-context-invariants  Verify runtime context/cache-key source invariants"
    echo "  module-manifest-docs-drift  Verify modules.toml, example and overview topology parity"
    echo "  advisory-exceptions  Verify cargo-deny advisory exceptions are registered and unexpired"
    echo "  dependency-feature-hygiene  Verify unused vulnerable dependency defaults stay disabled"
    echo "  csp-reporting-contract  Verify CSP report collection, telemetry minimization and inventory"
    echo "  csp-inline-style-exceptions  Verify the Rust-hosted inline style ratchet remains zero"
    echo "  csp-next-style-boundary  Verify registered Next style props, runtime style elements, and classic admin bootstrap"
    echo "  inventory-admin-boundary  Verify inventory admin native write/read boundary invariants"
    echo "  ai-domain-verticals  Verify AI product/content/order domain vertical ownership"
    echo "  module-lifecycle-bypass-usage  Verify lifecycle bypass helper is blocked in production paths"
    echo "  page-builder-contract-parity  Verify page-builder provider/consumer contract version parity"
    echo "  page-builder-contract-registry  Verify machine-readable page-builder provider/consumer registry"
    echo "  page-builder-fallback-profiles  Verify required page-builder fallback/toggle profile structure"
    echo "  page-builder-toggle-profiles-consistency  Verify page-builder toggle profile flag combinations"
    echo "  page-builder-fba-baseline  Run full page-builder FBA baseline gate (parity + fallback + toggle consistency)"
    echo "  page-builder-consumer-readiness  Verify module-level consumer readiness for page-builder (uses PBC_MODULE)"
    echo ""
    echo "Without arguments, runs all scripts."
}

while [[ $# -gt 0 ]]; do
    case $1 in
        -v|--verbose) VERBOSE=1; shift ;;
        -h|--help) usage; exit 0 ;;
        *) SELECTED_SCRIPT="$1"; shift ;;
    esac
done

SCRIPTS=(
    "verify-tenant-isolation.sh:Tenant Isolation"
    "verify-tenant-resolution-architecture.mjs:Tenant Resolution Architecture"
    "verify-unsafe-code.sh:Unsafe Code Patterns"
    "verify-rbac-coverage.sh:RBAC Coverage"
    "verify-api-quality.sh:API Quality (REST + GraphQL)"
    "verify-api-surface-contract.mjs:API Surface Contract"
    "verify-api-compatibility-self-test.mjs:API Compatibility Comparator Fixtures"
    "verify-api-compatibility-exception-approval-self-test.mjs:API Compatibility Exception Approval Fixtures"
    "verify-api-compatibility-infra-self-test.mjs:API Compatibility Infrastructure Approval Fixtures"
    "verify-api-compatibility-exceptions-local.mjs:API Compatibility Exceptions"
    "verify-api-compatibility-contract.mjs:API Compatibility Gate Structure"
    "verify-migration-plan-self-test.mjs:Migration Plan Comparator Fixtures"
    "verify-migration-backfill-self-test.mjs:Migration Backfill Contract Fixtures"
    "verify-migration-infra-self-test.mjs:Migration Infrastructure Approval Fixtures"
    "verify-repository-ruleset-self-test.mjs:Repository Ruleset Contract Fixtures"
    "verify-migration-compatibility-contract.mjs:Migration Compatibility Gate Structure"
    "verify-release-tooling-self-test.mjs:Release Tooling Fixtures"
    "verify-release-infra-self-test.mjs:Release Infrastructure Approval Fixtures"
    "verify-release-supply-chain-contract.mjs:Release Supply-chain Gate Structure"
    "verify-rust-host-browser-contract.mjs:Rust-host Browser Smoke Structure"
    "verify-development-container-topology.mjs:Development Container Topology"
    "verify-events.sh:Event System"
    "verify-code-quality.sh:Code Quality"
    "verify-security.sh:Security"
    "verify-architecture.sh:Architecture"
    "verify-deployment-profiles.sh:Deployment Profiles"
    "verify-anti-bypass.sh:Anti-bypass Audit"
    "verify-storefront-module-routes.mjs:Storefront Module Routes"
    "verify-i18n-contract.mjs:i18n Contract"
    "verify-ui-i18n-parity.mjs:UI i18n Parity"
    "verify-flex-multilingual-contract.mjs:Flex Multilingual Contract"
    "verify-runtime-context-invariants.mjs:Runtime Context Invariants"
    "verify-module-manifest-docs-drift.mjs:Module Manifest Documentation Drift"
    "verify-advisory-exceptions.mjs:Security Advisory Exceptions"
    "verify-dependency-feature-hygiene.mjs:Dependency Feature Hygiene"
    "verify-csp-reporting-contract.mjs:CSP Reporting Contract"
    "verify-csp-inline-style-exceptions.mjs:Rust CSP Style Boundary"
    "verify-csp-next-style-boundary.mjs:Next CSP Style Boundary"
    "verify-inventory-admin-boundary.mjs:Inventory Admin Boundary"
    "verify-ai-domain-verticals.mjs:AI Domain Verticals"
    "verify-module-lifecycle-bypass-usage.mjs:Module Lifecycle Bypass Usage"
    "verify-module-control-plane-write-path.mjs:Module Control-plane Write Path"
    "verify-module-build-worker-isolation.mjs:Module Build Worker Isolation"
    "../../crates/rustok-page-builder/scripts/verify/verify-page-builder-contract-parity.mjs:Page Builder Contract Parity"
    "../../crates/rustok-page-builder/scripts/verify/verify-page-builder-contract-registry.mjs:Page Builder Contract Registry"
    "../../crates/rustok-page-builder/scripts/verify/verify-page-builder-fallback-profiles.mjs:Page Builder Fallback Profiles"
    "../../crates/rustok-page-builder/scripts/verify/verify-page-builder-toggle-profiles-consistency.mjs:Page Builder Toggle Profiles Consistency"
    "../../crates/rustok-page-builder/scripts/verify/verify-page-builder-fba-baseline.mjs:Page Builder FBA Baseline Gate"
    "../../crates/rustok-page-builder/scripts/verify/verify-page-builder-consumer-readiness.mjs:Page Builder Consumer Readiness"
)

# Filter to selected script if specified
if [[ -n "$SELECTED_SCRIPT" ]]; then
    FILTERED=()
    for entry in "${SCRIPTS[@]}"; do
        script_file="${entry%%:*}"
        script_name="$(basename "${script_file%.sh}")"
        script_name="${script_name%.mjs}"
        script_name="${script_name#verify-}"
        alt_script_name="${script_name#run-}"
        if [[ "$script_name" == "$SELECTED_SCRIPT" || "$alt_script_name" == "$SELECTED_SCRIPT" || "$script_file" == "$SELECTED_SCRIPT" ]]; then
            FILTERED+=("$entry")
        fi
    done
    if [[ ${#FILTERED[@]} -eq 0 ]]; then
        echo -e "${RED}Unknown script: $SELECTED_SCRIPT${NC}"
        usage
        exit 1
    fi
    SCRIPTS=("${FILTERED[@]}")
fi

echo -e "${BOLD}╔══════════════════════════════════════════════╗${NC}"
echo -e "${BOLD}║   RusTok Platform Verification Suite         ║${NC}"
echo -e "${BOLD}╚══════════════════════════════════════════════╝${NC}"
echo ""
echo -e "  Date: $(date '+%Y-%m-%d %H:%M:%S')"
echo -e "  Scripts: ${#SCRIPTS[@]}"
echo ""

TOTAL_PASSED=0
TOTAL_FAILED=0
TOTAL_ERRORS=0
RESULTS=()
SEPARATOR="────────────────────────────────────────────────"

for entry in "${SCRIPTS[@]}"; do
    script_file="${entry%%:*}"
    script_label="${entry#*:}"
    script_path="$SCRIPT_DIR/$script_file"

    if [[ ! -f "$script_path" ]]; then
        echo -e "${RED}Script not found: $script_path${NC}"
        RESULTS+=("${RED}SKIP${NC} $script_label — script not found")
        continue
    fi

    echo -e "${BLUE}▶ Running: $script_label${NC}"
    echo -e "${SEPARATOR}"

    if [[ "$script_file" == *.mjs ]]; then
        if [[ "$script_file" == "../../crates/rustok-page-builder/scripts/verify/verify-page-builder-consumer-readiness.mjs" || "$script_file" == "../../crates/rustok-page-builder/scripts/verify/verify-page-builder-contract-registry.mjs" || "$script_file" == "../../crates/rustok-page-builder/scripts/verify/verify-page-builder-contract-parity.mjs" || "$script_file" == "../../crates/rustok-page-builder/scripts/verify/verify-page-builder-fba-baseline.mjs" ]]; then
            runner=(node "$script_path" "${PBC_MODULE:-pages}")
        else
            runner=(node "$script_path")
        fi
    else
        runner=(bash "$script_path")
    fi

    if [[ $VERBOSE -eq 1 ]]; then
        "${runner[@]}"
        exit_code=$?
    else
        output=$("${runner[@]}" 2>&1)
        exit_code=$?
        # Show compact summary lines when possible
        summary_lines="$(echo "$output" | grep -Ei "━━━|error|warning|passed|failed|✗|✔|summary" | tail -5 || true)"
        if [[ -n "$summary_lines" ]]; then
            echo "$summary_lines"
        fi
    fi

    if [[ $exit_code -eq 0 ]]; then
        RESULTS+=("${GREEN}PASS${NC} $script_label")
        TOTAL_PASSED=$((TOTAL_PASSED + 1))
    else
        RESULTS+=("${RED}FAIL${NC} $script_label ($exit_code error(s))")
        TOTAL_FAILED=$((TOTAL_FAILED + 1))
        TOTAL_ERRORS=$((TOTAL_ERRORS + exit_code))
        # In non-verbose mode, show errors
        if [[ $VERBOSE -eq 0 ]]; then
            fail_lines="$(echo "$output" | grep -Ei "✗|error|failed|violation" | head -10 || true)"
            if [[ -n "$fail_lines" ]]; then
                echo "$fail_lines"
            else
                # Fallback: print the tail so failures without standard markers are still visible.
                echo "$output" | tail -20
            fi
        fi
    fi

    echo ""
done

# ─── Final Report ───
echo -e "${BOLD}╔══════════════════════════════════════════════╗${NC}"
echo -e "${BOLD}║   Verification Report                        ║${NC}"
echo -e "${BOLD}╚══════════════════════════════════════════════╝${NC}"
echo ""

for result in "${RESULTS[@]}"; do
    echo -e "  $result"
done

echo ""
