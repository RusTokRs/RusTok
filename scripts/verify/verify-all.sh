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
    echo "  unsafe-code        Check unwrap, panic, blocking ops, println, global state"
    echo "  rbac-coverage      Check RBAC extractors on handlers/resolvers"
    echo "  api-quality        Check GraphQL/REST quality, N+1, OpenAPI, parity"
    echo "  events             Check event publishing, DLQ, outbox, transport, serde"
    echo "  code-quality       Check PII, secrets, metrics, dependencies, observability"
    echo "  security           Check argon2, headers, CORS, SSRF, JWT, rate limiting"
    echo "  architecture       Check module registry, Loco hooks, MCP, DI, telemetry"
    echo "  deployment-profiles  Smoke-check monolith, server+admin, headless-api builds"
    echo "  anti-bypass       Audit domain bypass patterns and duplicated business logic"
    echo "  storefront-module-routes  Verify storefront module route contract"
    echo "  i18n-contract     Verify i18n contract drift (repo-side)"
    echo "  ui-i18n-parity    Verify module UI i18n parity"
    echo "  flex-multilingual-contract  Verify Flex multilingual live contract guardrails"
    echo "  module-lifecycle-bypass-usage  Verify lifecycle bypass helper is blocked in production paths"
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
    "verify-unsafe-code.sh:Unsafe Code Patterns"
    "verify-rbac-coverage.sh:RBAC Coverage"
    "verify-api-quality.sh:API Quality (REST + GraphQL)"
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
    "verify-module-lifecycle-bypass-usage.mjs:Module Lifecycle Bypass Usage"
)

# Filter to selected script if specified
if [[ -n "$SELECTED_SCRIPT" ]]; then
    FILTERED=()
    for entry in "${SCRIPTS[@]}"; do
        script_file="${entry%%:*}"
        script_name="${script_file%.sh}"
        script_name="${script_name%.mjs}"
        script_name="${script_name#verify-}"
        if [[ "$script_name" == "$SELECTED_SCRIPT" || "$script_file" == "$SELECTED_SCRIPT" ]]; then
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
        runner=(node "$script_path")
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
echo -e "${SEPARATOR}"
TOTAL=$((TOTAL_PASSED + TOTAL_FAILED))
echo -e "  Total: $TOTAL suites | ${GREEN}$TOTAL_PASSED passed${NC} | ${RED}$TOTAL_FAILED failed${NC}"

if [[ $TOTAL_FAILED -eq 0 ]]; then
    echo ""
    echo -e "  ${GREEN}${BOLD}All verification suites passed!${NC}"
    exit 0
else
    echo ""
    echo -e "  ${RED}${BOLD}$TOTAL_FAILED suite(s) have errors. Review output above.${NC}"
    echo -e "  Run with ${BOLD}-v${NC} for full output: ${BOLD}./scripts/verify/verify-all.sh -v${NC}"
    # POSIX process exit codes are limited to 0..255.
    # Preserve "N errors" semantics while avoiding wraparound ambiguity.
    if [[ $TOTAL_ERRORS -gt 255 ]]; then
        echo -e "  ${YELLOW}Warning:${NC} aggregated error count $TOTAL_ERRORS exceeds exit-code limit; returning 255."
        exit 255
    fi
    exit "$TOTAL_ERRORS"
fi
