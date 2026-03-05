#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
SCRIPT="$ROOT_DIR/scripts/rbac_cutover_baseline.sh"

fail() {
  echo "[FAIL] $*" >&2
  exit 1
}

pass() {
  echo "[PASS] $*"
}

make_mock_curl() {
  local dir="$1"
  cat > "$dir/mock-curl" <<'MOCK'
#!/usr/bin/env bash
set -euo pipefail

state_file="${MOCK_CURL_STATE_FILE:-}"
if [[ -z "$state_file" ]]; then
  echo "MOCK_CURL_STATE_FILE is required" >&2
  exit 1
fi

count=0
if [[ -f "$state_file" ]]; then
  count="$(cat "$state_file")"
fi
count=$((count + 1))
printf '%s' "$count" > "$state_file"

profile="${MOCK_CURL_PROFILE:-steady}"
case "$profile" in
  shadow-fail)
    mismatch="0"
    shadow_fail="$count"
    ;;
  mismatch)
    mismatch="$count"
    shadow_fail="0"
    ;;
  steady|*)
    mismatch="0"
    shadow_fail="0"
    ;;
esac

cat <<METRICS
rustok_rbac_decision_mismatch_total ${mismatch}
rustok_rbac_shadow_compare_failures_total ${shadow_fail}
rustok_rbac_permission_checks_denied $((2 * count))
rustok_rbac_permission_checks_allowed $((10 * count))
METRICS
MOCK
  chmod +x "$dir/mock-curl"
}

test_baseline_fails_when_shadow_failures_change() {
  local tmp
  tmp="$(mktemp -d)"
  make_mock_curl "$tmp"

  set +e
  MOCK_CURL_STATE_FILE="$tmp/state" MOCK_CURL_PROFILE=shadow-fail RUSTOK_CURL_BIN="$tmp/mock-curl" "$SCRIPT" \
    --samples 3 --interval-sec 0 --artifacts-dir "$tmp/artifacts" >"$tmp/out.log" 2>&1
  code=$?
  set -e

  [[ "$code" -eq 1 ]] || fail "expected non-zero exit when shadow failure delta is non-zero"
  rg -q "Shadow compare failures delta is" "$tmp/out.log" || fail "expected shadow failures gate message"
  pass "baseline helper enforces zero shadow failures gate"
}

test_allow_shadow_failures_disables_strict_gate() {
  local tmp
  tmp="$(mktemp -d)"
  make_mock_curl "$tmp"

  MOCK_CURL_STATE_FILE="$tmp/state" MOCK_CURL_PROFILE=shadow-fail RUSTOK_CURL_BIN="$tmp/mock-curl" "$SCRIPT" \
    --samples 3 --interval-sec 0 --allow-shadow-failures --artifacts-dir "$tmp/artifacts" >"$tmp/out.log" 2>&1

  rg -q "Done. Report:" "$tmp/out.log" || fail "expected successful output with --allow-shadow-failures"
  pass "allow-shadow-failures flag bypasses strict shadow gate"
}

test_baseline_passes_when_mismatch_is_stable() {
  local tmp
  tmp="$(mktemp -d)"
  make_mock_curl "$tmp"

  MOCK_CURL_STATE_FILE="$tmp/state" MOCK_CURL_PROFILE=steady RUSTOK_CURL_BIN="$tmp/mock-curl" "$SCRIPT" \
    --samples 3 --interval-sec 0 --artifacts-dir "$tmp/artifacts" >"$tmp/out.log" 2>&1

  rg -q "Done. Report:" "$tmp/out.log" || fail "expected report output"
  report="$(rg -o 'Done\. Report: .*' "$tmp/out.log" | sed 's/Done\. Report: //')"
  rg -q "status: pass" "$report" || fail "expected pass gate in report"
  pass "baseline report passes with stable mismatch"
}

test_baseline_fails_when_mismatch_changes() {
  local tmp
  tmp="$(mktemp -d)"
  make_mock_curl "$tmp"

  set +e
  MOCK_CURL_STATE_FILE="$tmp/state" MOCK_CURL_PROFILE=mismatch RUSTOK_CURL_BIN="$tmp/mock-curl" "$SCRIPT" \
    --samples 3 --interval-sec 0 --artifacts-dir "$tmp/artifacts" >"$tmp/out.log" 2>&1
  code=$?
  set -e

  [[ "$code" -eq 1 ]] || fail "expected non-zero exit when mismatch delta is non-zero"
  rg -q "Mismatch delta is" "$tmp/out.log" || fail "expected mismatch gate message"
  pass "baseline helper enforces zero mismatch gate"
}

test_allow_mismatch_disables_strict_gate() {
  local tmp
  tmp="$(mktemp -d)"
  make_mock_curl "$tmp"

  MOCK_CURL_STATE_FILE="$tmp/state" MOCK_CURL_PROFILE=mismatch RUSTOK_CURL_BIN="$tmp/mock-curl" "$SCRIPT" \
    --samples 2 --interval-sec 0 --allow-mismatch --artifacts-dir "$tmp/artifacts" >"$tmp/out.log" 2>&1

  rg -q "Done. Report:" "$tmp/out.log" || fail "expected successful output with --allow-mismatch"
  pass "allow-mismatch flag bypasses strict gate"
}

test_baseline_fails_when_decision_volume_is_too_low() {
  local tmp
  tmp="$(mktemp -d)"

  cat > "$tmp/mock-curl" <<'MOCK'
#!/usr/bin/env bash
set -euo pipefail
cat <<METRICS
rustok_rbac_decision_mismatch_total 0
rustok_rbac_shadow_compare_failures_total 0
rustok_rbac_permission_checks_denied 0
rustok_rbac_permission_checks_allowed 0
METRICS
MOCK
  chmod +x "$tmp/mock-curl"

  set +e
  RUSTOK_CURL_BIN="$tmp/mock-curl" "$SCRIPT" \
    --samples 2 --interval-sec 0 --artifacts-dir "$tmp/artifacts" >"$tmp/out.log" 2>&1
  code=$?
  set -e

  [[ "$code" -eq 1 ]] || fail "expected non-zero exit when decision volume is too low"
  rg -q "Decision delta is" "$tmp/out.log" || fail "expected low-decision gate message"
  pass "baseline helper enforces minimum decision volume"
}

test_min_decision_delta_zero_allows_idle_windows() {
  local tmp
  tmp="$(mktemp -d)"

  cat > "$tmp/mock-curl" <<'MOCK'
#!/usr/bin/env bash
set -euo pipefail
cat <<METRICS
rustok_rbac_decision_mismatch_total 0
rustok_rbac_shadow_compare_failures_total 0
rustok_rbac_permission_checks_denied 0
rustok_rbac_permission_checks_allowed 0
METRICS
MOCK
  chmod +x "$tmp/mock-curl"

  RUSTOK_CURL_BIN="$tmp/mock-curl" "$SCRIPT" \
    --samples 2 --interval-sec 0 --min-decision-delta 0 --artifacts-dir "$tmp/artifacts" >"$tmp/out.log" 2>&1

  rg -q "Done. Report:" "$tmp/out.log" || fail "expected successful output when min decision delta is zero"
  pass "min-decision-delta=0 allows idle windows"
}

test_baseline_fails_on_counter_reset() {
  local tmp
  tmp="$(mktemp -d)"

  cat > "$tmp/mock-curl" <<'MOCK'
#!/usr/bin/env bash
set -euo pipefail

state_file="${MOCK_CURL_STATE_FILE:?}"
count=0
if [[ -f "$state_file" ]]; then
  count="$(cat "$state_file")"
fi
count=$((count + 1))
printf '%s' "$count" > "$state_file"

if [[ "$count" -eq 1 ]]; then
  allowed=50
else
  allowed=10
fi

cat <<METRICS
rustok_rbac_decision_mismatch_total 0
rustok_rbac_shadow_compare_failures_total 0
rustok_rbac_permission_checks_denied 0
rustok_rbac_permission_checks_allowed ${allowed}
METRICS
MOCK
  chmod +x "$tmp/mock-curl"

  set +e
  MOCK_CURL_STATE_FILE="$tmp/state" RUSTOK_CURL_BIN="$tmp/mock-curl" "$SCRIPT" \
    --samples 2 --interval-sec 0 --artifacts-dir "$tmp/artifacts" >"$tmp/out.log" 2>&1
  code=$?
  set -e

  [[ "$code" -eq 1 ]] || fail "expected non-zero exit when a counter reset is detected"
  rg -q "Counter reset detected" "$tmp/out.log" || fail "expected counter reset gate message"
  pass "baseline helper fails fast on counter reset"
}

test_json_report_includes_timestamps() {
  local tmp
  tmp="$(mktemp -d)"
  make_mock_curl "$tmp"

  MOCK_CURL_STATE_FILE="$tmp/state" MOCK_CURL_PROFILE=steady RUSTOK_CURL_BIN="$tmp/mock-curl" "$SCRIPT" \
    --samples 2 --interval-sec 0 --artifacts-dir "$tmp/artifacts" >"$tmp/out.log" 2>&1

  json_report="$(find "$tmp/artifacts" -maxdepth 1 -name 'rbac_cutover_baseline_*.json' | head -n 1)"
  [[ -n "$json_report" ]] || fail "expected json report artifact"
  rg -q '"timestamp"' "$json_report" || fail "expected per-sample timestamps in json report"
  rg -q '"permission_checks_total_delta"' "$json_report" || fail "expected permission_checks_total_delta in json report"
  rg -q '"total_decisions_delta"' "$json_report" || fail "expected total_decisions_delta alias in json report"
  python - "$json_report" <<'PY' || fail "expected decision delta aliases to match"
import json
import sys
with open(sys.argv[1], 'r', encoding='utf-8') as fh:
    payload = json.load(fh)
if payload.get('permission_checks_total_delta') != payload.get('total_decisions_delta'):
    raise SystemExit('decision delta aliases must match')
PY
  pass "json report includes timestamp and compatible decision delta aliases"
}



test_samples_are_persisted_by_default() {
  local tmp
  tmp="$(mktemp -d)"
  make_mock_curl "$tmp"

  MOCK_CURL_STATE_FILE="$tmp/state" MOCK_CURL_PROFILE=steady RUSTOK_CURL_BIN="$tmp/mock-curl" "$SCRIPT"     --samples 2 --interval-sec 0 --artifacts-dir "$tmp/artifacts" >"$tmp/out.log" 2>&1

  samples_dir="$(find "$tmp/artifacts" -maxdepth 1 -type d -name 'rbac_cutover_samples_*' | head -n 1)"
  [[ -n "$samples_dir" ]] || fail "expected persisted samples directory by default"
  [[ -f "$samples_dir/sample_1.prom" ]] || fail "expected persisted sample_1.prom"
  [[ -f "$samples_dir/sample_2.prom" ]] || fail "expected persisted sample_2.prom"
  pass "baseline helper persists raw samples by default"
}


test_no_save_samples_disables_raw_snapshot_artifacts() {
  local tmp
  tmp="$(mktemp -d)"
  make_mock_curl "$tmp"

  MOCK_CURL_STATE_FILE="$tmp/state" MOCK_CURL_PROFILE=steady RUSTOK_CURL_BIN="$tmp/mock-curl" "$SCRIPT"     --samples 2 --interval-sec 0 --no-save-samples --artifacts-dir "$tmp/artifacts" >"$tmp/out.log" 2>&1

  if find "$tmp/artifacts" -maxdepth 1 -type d -name 'rbac_cutover_samples_*' | rg -q .; then
    fail "did not expect samples directory when --no-save-samples is set"
  fi
  pass "no-save-samples disables raw sample artifacts"
}

test_baseline_passes_when_mismatch_is_stable
test_baseline_fails_when_mismatch_changes
test_baseline_fails_when_shadow_failures_change
test_allow_mismatch_disables_strict_gate
test_allow_shadow_failures_disables_strict_gate
test_baseline_fails_when_decision_volume_is_too_low
test_min_decision_delta_zero_allows_idle_windows
test_baseline_fails_on_counter_reset
test_json_report_includes_timestamps
test_samples_are_persisted_by_default
test_no_save_samples_disables_raw_snapshot_artifacts

echo "All rbac_cutover_baseline tests passed"
