#!/usr/bin/env bash
set -euo pipefail

SCRIPT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)/commerce_release_gate.sh"

fail() {
  echo "[FAIL] $1" >&2
  exit 1
}

pass() {
  echo "[PASS] $1"
}

make_mock_cargo() {
  local dir="$1"
  cat > "$dir/mock-cargo" <<'MOCK'
#!/usr/bin/env bash
set -euo pipefail

if [[ "$1" != "test" ]]; then
  echo "unexpected command" >&2
  exit 2
fi

args=" $* "

match_suite() {
  local needle="$1"
  [[ "$args" == *" $needle "* ]]
}

maybe_fail() {
  local flag="$1"
  local message="$2"
  if [[ "${!flag:-0}" == "1" ]]; then
    echo "$message" >&2
    exit 1
  fi
}

if match_suite "commerce_rollout_middleware_test"; then
  maybe_fail MOCK_FAIL_COMMERCE_ROLLOUT "simulated commerce rollout middleware failure"
  echo "commerce_rollout_middleware_test ok"
  exit 0
fi

if match_suite "commerce_openapi_contract"; then
  maybe_fail MOCK_FAIL_COMMERCE_OPENAPI "simulated commerce openapi failure"
  echo "commerce_openapi_contract ok"
  exit 0
fi

if match_suite "commerce_legacy_openapi_contract"; then
  maybe_fail MOCK_FAIL_COMMERCE_LEGACY_OPENAPI "simulated commerce legacy openapi failure"
  echo "commerce_legacy_openapi_contract ok"
  exit 0
fi

if match_suite "ecommerce_schema_smoke"; then
  maybe_fail MOCK_FAIL_ECOMMERCE_SCHEMA "simulated ecommerce schema smoke failure"
  echo "ecommerce_schema_smoke ok"
  exit 0
fi

if match_suite "startup_smoke_builds_router_and_runtime_shared_state"; then
  maybe_fail MOCK_FAIL_STARTUP_SMOKE "simulated startup smoke failure"
  echo "startup_smoke_builds_router_and_runtime_shared_state ok"
  exit 0
fi

echo "unexpected suite: $*" >&2
exit 3
MOCK
  chmod +x "$dir/mock-cargo"
}

make_evidence_bundle() {
  local dir="$1"
  mkdir -p "$dir/evidence"

  cat > "$dir/evidence/parity.md" <<'EOF_PARITY'
# parity ok
EOF_PARITY

  cat > "$dir/evidence/legacy-usage.md" <<'EOF_USAGE'
# legacy usage decline
- http_legacy requests are below rollout threshold
EOF_USAGE

  cat > "$dir/evidence/metrics.prom" <<'EOF_METRICS'
rustok_runtime_guardrail_commerce_surface_enabled{surface="legacy"} 1
rustok_runtime_guardrail_commerce_surface_canary_percent{surface="legacy"} 100
rustok_runtime_guardrail_commerce_surface_restricted{surface="legacy"} 0
EOF_METRICS

  cat > "$dir/evidence/runtime.json" <<'EOF_RUNTIME'
{"status":"ok","observed_status":"ok","rollout":"observe","reasons":[],"ecommerce_rollout":{"surfaces":[{"surface":"legacy","enabled":true,"canary_percent":100,"restricted":false},{"surface":"store","enabled":true,"canary_percent":100,"restricted":false},{"surface":"admin","enabled":true,"canary_percent":100,"restricted":false}]}}
EOF_RUNTIME
}

extract_report_path() {
  local log_file="$1"
  rg -o 'Done\. Report: .*|Report: .*' "$log_file" | tail -n 1 | sed -E 's/Done\. Report: |Report: //'
}

test_default_run_marks_external_gates_pending() {
  local tmp
  tmp="$(mktemp -d)"
  make_mock_cargo "$tmp"

  RUSTOK_CARGO_BIN="$tmp/mock-cargo" "$SCRIPT" --artifacts-dir "$tmp/artifacts" >"$tmp/out.log"

  local report
  report="$(extract_report_path "$tmp/out.log")"
  [[ -n "$report" && -f "$report" ]] || fail "report file missing"
  rg -q '| Local commerce integration slice | Done |' "$report" || fail "integration gate should be done"
  rg -q '| REST/GraphQL parity \(staging\) | Pending |' "$report" || fail "parity gate should be pending"
  rg -q '| Legacy usage evidence | Pending |' "$report" || fail "usage gate should be pending"
  rg -Fq '| Rollout metrics snapshot (`/metrics`) | Pending |' "$report" || fail "metrics gate should be pending"
  rg -Fq '| Runtime guardrail snapshot (`/health/runtime`) | Pending |' "$report" || fail "runtime gate should be pending"
  pass "default run executes local commerce tests and leaves external gates pending"
}

test_require_all_gates_fails_without_external_evidence() {
  local tmp
  tmp="$(mktemp -d)"
  make_mock_cargo "$tmp"

  set +e
  RUSTOK_CARGO_BIN="$tmp/mock-cargo" "$SCRIPT" --require-all-gates --artifacts-dir "$tmp/artifacts" >"$tmp/out.log" 2>&1
  local code=$?
  set -e

  [[ $code -eq 1 ]] || fail "expected --require-all-gates to fail"
  rg -q 'Gate check failed' "$tmp/out.log" || fail "missing gate failure message"
  pass "require-all-gates fails without external evidence"
}

test_require_all_gates_passes_with_full_evidence_bundle() {
  local tmp
  tmp="$(mktemp -d)"
  make_mock_cargo "$tmp"
  make_evidence_bundle "$tmp"

  RUSTOK_CARGO_BIN="$tmp/mock-cargo" "$SCRIPT" \
    --require-all-gates \
    --parity-report "$tmp/evidence/parity.md" \
    --legacy-usage-report "$tmp/evidence/legacy-usage.md" \
    --metrics-snapshot "$tmp/evidence/metrics.prom" \
    --runtime-snapshot "$tmp/evidence/runtime.json" \
    --artifacts-dir "$tmp/artifacts" >"$tmp/out.log"

  local report
  report="$(extract_report_path "$tmp/out.log")"
  [[ -n "$report" && -f "$report" ]] || fail "report file missing"
  rg -q '| REST/GraphQL parity \(staging\) | Done |' "$report" || fail "parity gate should be done"
  rg -q '| Legacy usage evidence | Done |' "$report" || fail "usage gate should be done"
  rg -Fq '| Rollout metrics snapshot (`/metrics`) | Done |' "$report" || fail "metrics gate should be done"
  rg -Fq '| Runtime guardrail snapshot (`/health/runtime`) | Done |' "$report" || fail "runtime gate should be done"
  pass "require-all-gates passes with parity, usage, metrics and runtime evidence"
}

test_invalid_metrics_snapshot_blocks_gate() {
  local tmp
  tmp="$(mktemp -d)"
  make_mock_cargo "$tmp"
  make_evidence_bundle "$tmp"
  cat > "$tmp/evidence/metrics.prom" <<'EOF_METRICS_INVALID'
rustok_runtime_guardrail_commerce_surface_enabled{surface="legacy"} 1
EOF_METRICS_INVALID

  set +e
  RUSTOK_CARGO_BIN="$tmp/mock-cargo" "$SCRIPT" \
    --require-all-gates \
    --parity-report "$tmp/evidence/parity.md" \
    --legacy-usage-report "$tmp/evidence/legacy-usage.md" \
    --metrics-snapshot "$tmp/evidence/metrics.prom" \
    --runtime-snapshot "$tmp/evidence/runtime.json" \
    --artifacts-dir "$tmp/artifacts" >"$tmp/out.log" 2>&1
  local code=$?
  set -e

  [[ $code -eq 1 ]] || fail "expected invalid metrics snapshot to fail"
  local report
  report="$(extract_report_path "$tmp/out.log")"
  [[ -n "$report" && -f "$report" ]] || fail "report file missing"
  rg -Fq '| Rollout metrics snapshot (`/metrics`) | Failed |' "$report" || fail "metrics gate should fail on missing markers"
  pass "metrics evidence must contain expected ecommerce rollout series"
}

test_local_suite_failure_exits_non_zero() {
  local tmp
  tmp="$(mktemp -d)"
  make_mock_cargo "$tmp"

  set +e
  MOCK_FAIL_COMMERCE_LEGACY_OPENAPI=1 RUSTOK_CARGO_BIN="$tmp/mock-cargo" "$SCRIPT" --artifacts-dir "$tmp/artifacts" >"$tmp/out.log" 2>&1
  local code=$?
  set -e

  [[ $code -eq 1 ]] || fail "expected non-zero exit when local suite fails"
  rg -q 'Integration gate failed: local commerce test slice failed.' "$tmp/out.log" || fail "missing local failure message"
  pass "local suite failure blocks release gate"
}

test_skip_local_tests_marks_integration_pending() {
  local tmp
  tmp="$(mktemp -d)"
  make_mock_cargo "$tmp"

  RUSTOK_CARGO_BIN="$tmp/mock-cargo" "$SCRIPT" --skip-local-tests --artifacts-dir "$tmp/artifacts" >"$tmp/out.log"

  local report
  report="$(extract_report_path "$tmp/out.log")"
  [[ -n "$report" && -f "$report" ]] || fail "report file missing"
  rg -q '| Local commerce integration slice | Pending | Skipped by flag --skip-local-tests |' "$report" || fail "integration gate should be pending when skipped"
  pass "skip-local-tests leaves integration gate pending"
}

test_default_run_marks_external_gates_pending
test_require_all_gates_fails_without_external_evidence
test_require_all_gates_passes_with_full_evidence_bundle
test_invalid_metrics_snapshot_blocks_gate
test_skip_local_tests_marks_integration_pending
test_local_suite_failure_exits_non_zero

echo "commerce_release_gate tests passed"
