#!/usr/bin/env bash
set -euo pipefail

usage() {
  cat <<'USAGE'
Usage:
  scripts/auth_release_gate.sh [options]

Options:
  --artifacts-dir <dir>       Output folder for logs/report (default: artifacts/auth-release-gate)
  --env <name>                Environment label in report (default: local)
  --skip-local-tests          Skip local test execution and mark integration gate as pending
  --parity-report <file>      Path to existing staging parity report evidence
  --security-signoff <file>   Path to existing security checklist/sign-off evidence
  --require-all-gates         Exit non-zero unless all gates are marked done
  --help                      Show this message

Environment:
  RUSTOK_CARGO_BIN            Override cargo executable path (default: cargo)

Examples:
  scripts/auth_release_gate.sh
  scripts/auth_release_gate.sh --parity-report artifacts/staging/parity.md --security-signoff artifacts/staging/security.md --require-all-gates
USAGE
}

ARTIFACTS_DIR="artifacts/auth-release-gate"
ENV_NAME="local"
SKIP_LOCAL_TESTS="false"
PARITY_REPORT=""
SECURITY_SIGNOFF=""
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
    --security-signoff)
      SECURITY_SIGNOFF="$2"; shift 2 ;;
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
REPORT_FILE="$ARTIFACTS_DIR/auth_release_gate_${TS}.md"
AUTH_LIFECYCLE_LOG="$ARTIFACTS_DIR/auth_lifecycle_${TS}.log"
AUTH_LOG="$ARTIFACTS_DIR/auth_${TS}.log"

integration_status="Pending"
integration_note="Skipped by flag --skip-local-tests"

if [[ "$SKIP_LOCAL_TESTS" != "true" ]]; then
  integration_status="Done"
  integration_note="cargo test -p rustok-server auth_lifecycle + cargo test -p rustok-server auth"

  lifecycle_failed="false"
  auth_failed="false"

  if ! "$CARGO_BIN" test -p rustok-server auth_lifecycle >"$AUTH_LIFECYCLE_LOG" 2>&1; then
    lifecycle_failed="true"
    LOCAL_TEST_FAILURE="true"
  fi

  if ! "$CARGO_BIN" test -p rustok-server auth >"$AUTH_LOG" 2>&1; then
    auth_failed="true"
    LOCAL_TEST_FAILURE="true"
  fi

  if [[ "$lifecycle_failed" == "true" && "$auth_failed" == "true" ]]; then
    integration_status="Failed"
    integration_note="auth_lifecycle and auth suites failed (see logs)"
  elif [[ "$lifecycle_failed" == "true" ]]; then
    integration_status="Failed"
    integration_note="auth_lifecycle suite failed (see log)"
  elif [[ "$auth_failed" == "true" ]]; then
    integration_status="Failed"
    integration_note="auth suite failed (see log)"
  fi
else
  echo "Skipped (--skip-local-tests)." >"$AUTH_LIFECYCLE_LOG"
  echo "Skipped (--skip-local-tests)." >"$AUTH_LOG"
fi

parity_status="Pending"
parity_note="Attach staging parity report via --parity-report"
if [[ -n "$PARITY_REPORT" ]]; then
  if [[ -f "$PARITY_REPORT" ]]; then
    parity_status="Done"
    parity_note="Evidence: $PARITY_REPORT"
  else
    parity_status="Failed"
    parity_note="Provided parity report is missing: $PARITY_REPORT"
  fi
fi

security_status="Pending"
security_note="Attach security sign-off via --security-signoff"
if [[ -n "$SECURITY_SIGNOFF" ]]; then
  if [[ -f "$SECURITY_SIGNOFF" ]]; then
    security_status="Done"
    security_note="Evidence: $SECURITY_SIGNOFF"
  else
    security_status="Failed"
    security_note="Provided security sign-off is missing: $SECURITY_SIGNOFF"
  fi
fi

cat > "$REPORT_FILE" <<REPORT
# Auth release gate report

- Timestamp (UTC): $TS
- Environment: $ENV_NAME

| Gate | Status | Details |
| --- | --- | --- |
| Integration (auth_lifecycle + auth) | $integration_status | $integration_note |
| REST/GraphQL parity (staging) | $parity_status | $parity_note |
| Security review sign-off | $security_status | $security_note |

## Local artifacts

- auth_lifecycle log: $AUTH_LIFECYCLE_LOG
- auth log: $AUTH_LOG

## Next actions

1. Ensure parity evidence is attached before release.
2. Ensure security checklist/sign-off is attached before release.
3. Use --require-all-gates in pre-release automation to enforce go/no-go.
REPORT

if [[ "$LOCAL_TEST_FAILURE" == "true" ]]; then
  echo "Integration gate failed: local auth test suite failed." >&2
  echo "Report: $REPORT_FILE"
  exit 1
fi

if [[ "$REQUIRE_ALL_GATES" == "true" ]]; then
  if [[ "$integration_status" != "Done" || "$parity_status" != "Done" || "$security_status" != "Done" ]]; then
    echo "Gate check failed (require-all-gates): integration=$integration_status parity=$parity_status security=$security_status" >&2
    echo "Report: $REPORT_FILE"
    exit 1
  fi
fi

echo "Done. Report: $REPORT_FILE"
