#!/usr/bin/env bash
set -euo pipefail

SCRIPT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)/commerce_rollout_report.sh"

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

if match_suite "commerce_rollout_middleware_test"; then
  echo "commerce_rollout_middleware_test ok"
  exit 0
fi
if match_suite "commerce_openapi_contract"; then
  echo "commerce_openapi_contract ok"
  exit 0
fi
if match_suite "commerce_legacy_openapi_contract"; then
  echo "commerce_legacy_openapi_contract ok"
  exit 0
fi
if match_suite "ecommerce_schema_smoke"; then
  echo "ecommerce_schema_smoke ok"
  exit 0
fi
if match_suite "startup_smoke_builds_router_and_runtime_shared_state"; then
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

  cat > "$dir/evidence/metrics.prom" <<'EOF_METRICS'
rustok_runtime_guardrail_commerce_surface_enabled{surface="legacy"} 1
rustok_runtime_guardrail_commerce_surface_canary_percent{surface="legacy"} 100
rustok_runtime_guardrail_commerce_surface_restricted{surface="legacy"} 0
rustok_runtime_guardrail_commerce_surface_enabled{surface="store"} 1
rustok_runtime_guardrail_commerce_surface_canary_percent{surface="store"} 100
rustok_runtime_guardrail_commerce_surface_restricted{surface="store"} 0
rustok_runtime_guardrail_commerce_surface_enabled{surface="admin"} 0
rustok_runtime_guardrail_commerce_surface_canary_percent{surface="admin"} 100
rustok_runtime_guardrail_commerce_surface_restricted{surface="admin"} 1
rustok_module_entrypoint_calls_total{module="commerce",entry_point="http_legacy",path="bypass"} 12
rustok_module_entrypoint_calls_total{module="commerce",entry_point="http_store",path="core_runtime"} 120
rustok_module_entrypoint_calls_total{module="commerce",entry_point="http_admin",path="core_runtime"} 24
EOF_METRICS

  cat > "$dir/evidence/runtime.json" <<'EOF_RUNTIME'
{"status":"ok","observed_status":"ok","rollout":"observe","reasons":["commerce surface `admin` disabled by rollout policy"],"ecommerce_rollout":{"surfaces":[{"surface":"legacy","enabled":true,"canary_percent":100,"restricted":false},{"surface":"store","enabled":true,"canary_percent":100,"restricted":false},{"surface":"admin","enabled":false,"canary_percent":100,"restricted":true}]}}
EOF_RUNTIME

  cat > "$dir/evidence/parity.md" <<'EOF_PARITY'
# parity ok
EOF_PARITY
}

extract_path() {
  local label="$1"
  local log_file="$2"
  rg -o "${label}: .*" "$log_file" | sed "s/${label}: //"
}

test_report_collects_operator_evidence() {
  local tmp
  tmp="$(mktemp -d)"
  make_mock_cargo "$tmp"
  make_evidence_bundle "$tmp"

  RUSTOK_CARGO_BIN="$tmp/mock-cargo" "$SCRIPT" \
    --artifacts-dir "$tmp/artifacts" \
    --metrics-snapshot "$tmp/evidence/metrics.prom" \
    --runtime-snapshot "$tmp/evidence/runtime.json" \
    --parity-report "$tmp/evidence/parity.md" >"$tmp/out.log"

  local report summary
  report="$(extract_path "Report" "$tmp/out.log")"
  summary="$(extract_path "Summary" "$tmp/out.log")"
  [[ -n "$report" && -f "$report" ]] || fail "report file missing"
  [[ -n "$summary" && -f "$summary" ]] || fail "summary file missing"

  rg -q 'Overall status: done' "$report" || fail "report should mark overall status done"
  rg -q 'Legacy share 7\.69% is within threshold 10\.00%' "$report" || fail "report should summarize legacy share"
  rg -q '| http_legacy | 0 | 0 | 12 | 12 |' "$report" || fail "report should include legacy entry point row"
  rg -q '| admin | 0 | 0 | 100 | 100 | 1 | 1 |' "$report" || fail "report should include admin rollout row"
  rg -q '"overall_status": "done"' "$summary" || fail "summary should record overall status"
  rg -q '"status": "done"' "$summary" || fail "summary should contain done statuses"
  rg -q '"legacy_share_percent": 7\.6923076923076925' "$summary" || fail "summary should expose precise legacy share"
  pass "operator report collects parity, usage and rollout evidence"
}

test_report_marks_pending_without_parity_report() {
  local tmp
  tmp="$(mktemp -d)"
  make_mock_cargo "$tmp"
  make_evidence_bundle "$tmp"

  RUSTOK_CARGO_BIN="$tmp/mock-cargo" "$SCRIPT" \
    --artifacts-dir "$tmp/artifacts" \
    --metrics-snapshot "$tmp/evidence/metrics.prom" \
    --runtime-snapshot "$tmp/evidence/runtime.json" >"$tmp/out.log"

  local report summary
  report="$(extract_path "Report" "$tmp/out.log")"
  summary="$(extract_path "Summary" "$tmp/out.log")"
  [[ -n "$report" && -f "$report" ]] || fail "report file missing"
  [[ -n "$summary" && -f "$summary" ]] || fail "summary file missing"

  rg -q '| Parity evidence | Pending |' "$report" || fail "parity should be pending without report"
  rg -q '"overall_status": "pending"' "$summary" || fail "summary should be pending without parity evidence"
  pass "operator report leaves parity pending when external evidence is absent"
}

test_report_detects_snapshot_mismatch() {
  local tmp
  tmp="$(mktemp -d)"
  make_mock_cargo "$tmp"
  make_evidence_bundle "$tmp"

  cat > "$tmp/evidence/runtime.json" <<'EOF_RUNTIME'
{"status":"ok","observed_status":"ok","rollout":"observe","reasons":[],"ecommerce_rollout":{"surfaces":[{"surface":"legacy","enabled":true,"canary_percent":100,"restricted":false},{"surface":"store","enabled":true,"canary_percent":50,"restricted":true},{"surface":"admin","enabled":false,"canary_percent":100,"restricted":true}]}}
EOF_RUNTIME

  RUSTOK_CARGO_BIN="$tmp/mock-cargo" "$SCRIPT" \
    --artifacts-dir "$tmp/artifacts" \
    --metrics-snapshot "$tmp/evidence/metrics.prom" \
    --runtime-snapshot "$tmp/evidence/runtime.json" \
    --parity-report "$tmp/evidence/parity.md" >"$tmp/out.log"

  local report summary
  report="$(extract_path "Report" "$tmp/out.log")"
  summary="$(extract_path "Summary" "$tmp/out.log")"
  [[ -n "$report" && -f "$report" ]] || fail "report file missing"
  [[ -n "$summary" && -f "$summary" ]] || fail "summary file missing"

  rg -q '| Snapshot consistency | Failed |' "$report" || fail "report should mark snapshot mismatch"
  rg -q '"overall_status": "failed"' "$summary" || fail "summary should fail on mismatched rollout snapshots"
  pass "operator report catches runtime vs metrics rollout mismatch"
}

test_report_collects_operator_evidence
test_report_marks_pending_without_parity_report
test_report_detects_snapshot_mismatch

echo "commerce_rollout_report tests passed"
