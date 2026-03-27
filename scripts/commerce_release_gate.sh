#!/usr/bin/env bash
set -euo pipefail

usage() {
  cat <<'USAGE'
Usage:
  scripts/commerce_release_gate.sh [options]

Options:
  --artifacts-dir <dir>         Output folder for logs/report (default: artifacts/commerce-release-gate)
  --env <name>                  Environment label in report (default: local)
  --skip-local-tests            Skip local test execution and mark integration gate as pending
  --parity-report <file>        Path to existing staging parity report evidence
  --legacy-usage-report <file>  Path to existing legacy usage decline / migration evidence
  --metrics-snapshot <file>     Path to captured /metrics snapshot with ecommerce rollout metrics
  --runtime-snapshot <file>     Path to captured /health/runtime snapshot with ecommerce rollout state
  --require-all-gates           Exit non-zero unless all gates are marked done
  --help                        Show this message

Environment:
  RUSTOK_CARGO_BIN              Override cargo executable path (default: cargo)

Examples:
  scripts/commerce_release_gate.sh
  scripts/commerce_release_gate.sh --require-all-gates \
    --parity-report artifacts/staging/commerce-parity.md \
    --legacy-usage-report artifacts/staging/commerce-legacy-usage.md \
    --metrics-snapshot artifacts/staging/commerce-metrics.prom \
    --runtime-snapshot artifacts/staging/commerce-runtime.json
USAGE
}

ARTIFACTS_DIR="artifacts/commerce-release-gate"
ENV_NAME="local"
SKIP_LOCAL_TESTS="false"
PARITY_REPORT=""
LEGACY_USAGE_REPORT=""
METRICS_SNAPSHOT=""
RUNTIME_SNAPSHOT=""
REQUIRE_ALL_GATES="false"
CARGO_BIN="${RUSTOK_CARGO_BIN:-cargo}"
LOCAL_TEST_FAILURE="false"

while [[ $# -gt 0 ]]; do
  case "$1" in
    --artifacts-dir)
      ARTIFACTS_DIR="$2"; shift 2 ;;
    --env)
      ENV_NAME="$2"; shift 2 ;;
    --skip-local-tests)
      SKIP_LOCAL_TESTS="true"; shift ;;
    --parity-report)
      PARITY_REPORT="$2"; shift 2 ;;
    --legacy-usage-report)
      LEGACY_USAGE_REPORT="$2"; shift 2 ;;
    --metrics-snapshot)
      METRICS_SNAPSHOT="$2"; shift 2 ;;
    --runtime-snapshot)
      RUNTIME_SNAPSHOT="$2"; shift 2 ;;
    --require-all-gates)
      REQUIRE_ALL_GATES="true"; shift ;;
    --help)
      usage; exit 0 ;;
    *)
      echo "Unknown option: $1" >&2
      usage
      exit 1 ;;
  esac
done

mkdir -p "$ARTIFACTS_DIR"
TS="$(date -u +%Y%m%dT%H%M%SZ)"
REPORT_FILE="$ARTIFACTS_DIR/commerce_release_gate_${TS}.md"

ROLL_OUT_LOG="$ARTIFACTS_DIR/commerce_rollout_middleware_${TS}.log"
OPENAPI_LOG="$ARTIFACTS_DIR/commerce_openapi_contract_${TS}.log"
LEGACY_OPENAPI_LOG="$ARTIFACTS_DIR/commerce_legacy_openapi_contract_${TS}.log"
SCHEMA_SMOKE_LOG="$ARTIFACTS_DIR/ecommerce_schema_smoke_${TS}.log"
STARTUP_LOG="$ARTIFACTS_DIR/startup_router_smoke_${TS}.log"

integration_status="Pending"
integration_note="Skipped by flag --skip-local-tests"
declare -a failed_suites=()

run_local_suite() {
  local log_file="$1"
  shift
  if ! "$CARGO_BIN" test "$@" >"$log_file" 2>&1; then
    failed_suites+=("$(basename "$log_file" | sed -E 's/_[0-9]{8}T[0-9]{6}Z\.log$//')")
    LOCAL_TEST_FAILURE="true"
  fi
}

evaluate_existence_gate() {
  local file_path="$1"
  local pending_note="$2"

  if [[ -z "$file_path" ]]; then
    echo "Pending"
    echo "$pending_note"
    return
  fi

  if [[ -f "$file_path" ]]; then
    echo "Done"
    echo "Evidence: $file_path"
    return
  fi

  echo "Failed"
  echo "Provided evidence is missing: $file_path"
}

evaluate_content_gate() {
  local file_path="$1"
  local pending_note="$2"
  shift 2

  if [[ -z "$file_path" ]]; then
    echo "Pending"
    echo "$pending_note"
    return
  fi

  if [[ ! -f "$file_path" ]]; then
    echo "Failed"
    echo "Provided evidence is missing: $file_path"
    return
  fi

  local missing=()
  local pattern
  for pattern in "$@"; do
    if ! grep -Fq "$pattern" "$file_path"; then
      missing+=("$pattern")
    fi
  done

  if [[ ${#missing[@]} -gt 0 ]]; then
    echo "Failed"
    echo "Evidence is present but missing expected markers (${missing[*]}): $file_path"
    return
  fi

  echo "Done"
  echo "Evidence: $file_path"
}

if [[ "$SKIP_LOCAL_TESTS" != "true" ]]; then
  integration_status="Done"
  integration_note="cargo test ecommerce rollout middleware + store/legacy OpenAPI + migration smoke + router startup"

  run_local_suite "$ROLL_OUT_LOG" -p rustok-server --test commerce_rollout_middleware_test
  run_local_suite "$OPENAPI_LOG" -p rustok-server --test commerce_openapi_contract
  run_local_suite "$LEGACY_OPENAPI_LOG" -p rustok-server --test commerce_legacy_openapi_contract
  run_local_suite "$SCHEMA_SMOKE_LOG" -p migration --test ecommerce_schema_smoke
  run_local_suite "$STARTUP_LOG" -p rustok-server startup_smoke_builds_router_and_runtime_shared_state --lib

  if [[ ${#failed_suites[@]} -gt 0 ]]; then
    integration_status="Failed"
    integration_note="Failed suites: ${failed_suites[*]} (see logs)"
  fi
else
  echo "Skipped (--skip-local-tests)." >"$ROLL_OUT_LOG"
  echo "Skipped (--skip-local-tests)." >"$OPENAPI_LOG"
  echo "Skipped (--skip-local-tests)." >"$LEGACY_OPENAPI_LOG"
  echo "Skipped (--skip-local-tests)." >"$SCHEMA_SMOKE_LOG"
  echo "Skipped (--skip-local-tests)." >"$STARTUP_LOG"
fi

readarray -t parity_gate < <(evaluate_existence_gate "$PARITY_REPORT" "Attach staging parity report via --parity-report")
parity_status="${parity_gate[0]}"
parity_note="${parity_gate[1]}"

readarray -t usage_gate < <(evaluate_existence_gate "$LEGACY_USAGE_REPORT" "Attach legacy usage decline evidence via --legacy-usage-report")
usage_status="${usage_gate[0]}"
usage_note="${usage_gate[1]}"

readarray -t metrics_gate < <(evaluate_content_gate \
  "$METRICS_SNAPSHOT" \
  "Attach /metrics snapshot via --metrics-snapshot" \
  "rustok_runtime_guardrail_commerce_surface_enabled" \
  "rustok_runtime_guardrail_commerce_surface_canary_percent" \
  "rustok_runtime_guardrail_commerce_surface_restricted")
metrics_status="${metrics_gate[0]}"
metrics_note="${metrics_gate[1]}"

readarray -t runtime_gate < <(evaluate_content_gate \
  "$RUNTIME_SNAPSHOT" \
  "Attach /health/runtime snapshot via --runtime-snapshot" \
  "ecommerce_rollout" \
  "\"surface\":\"legacy\"" \
  "\"surface\":\"store\"" \
  "\"surface\":\"admin\"" \
  "\"restricted\"")
runtime_status="${runtime_gate[0]}"
runtime_note="${runtime_gate[1]}"

cat > "$REPORT_FILE" <<REPORT
# Commerce release gate report

- Timestamp (UTC): $TS
- Environment: $ENV_NAME

| Gate | Status | Details |
| --- | --- | --- |
| Local commerce integration slice | $integration_status | $integration_note |
| REST/GraphQL parity (staging) | $parity_status | $parity_note |
| Legacy usage evidence | $usage_status | $usage_note |
| Rollout metrics snapshot (`/metrics`) | $metrics_status | $metrics_note |
| Runtime guardrail snapshot (`/health/runtime`) | $runtime_status | $runtime_note |

## Local artifacts

- rollout middleware log: $ROLL_OUT_LOG
- store OpenAPI log: $OPENAPI_LOG
- legacy OpenAPI log: $LEGACY_OPENAPI_LOG
- migration smoke log: $SCHEMA_SMOKE_LOG
- startup router log: $STARTUP_LOG

## Next actions

1. Capture parity evidence before disabling legacy `/api/commerce/*`.
2. Attach a legacy usage report that shows sustained traffic decline on `http_legacy`.
3. Capture `/metrics` and `/health/runtime` snapshots from the rollout environment and verify ecommerce surface policy there.
4. Use --require-all-gates in pre-release automation to enforce go/no-go.
REPORT

if [[ "$LOCAL_TEST_FAILURE" == "true" ]]; then
  echo "Integration gate failed: local commerce test slice failed." >&2
  echo "Report: $REPORT_FILE"
  exit 1
fi

if [[ "$REQUIRE_ALL_GATES" == "true" ]]; then
  if [[ "$integration_status" != "Done" || "$parity_status" != "Done" || "$usage_status" != "Done" || "$metrics_status" != "Done" || "$runtime_status" != "Done" ]]; then
    echo "Gate check failed (require-all-gates): integration=$integration_status parity=$parity_status usage=$usage_status metrics=$metrics_status runtime=$runtime_status" >&2
    echo "Report: $REPORT_FILE"
    exit 1
  fi
fi

echo "Done. Report: $REPORT_FILE"
