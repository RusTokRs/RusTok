#!/usr/bin/env bash
set -euo pipefail

SCRIPT="scripts/rbac_cutover_workflow.sh"

pass() { echo "[PASS] $1"; }
fail() { echo "[FAIL] $1" >&2; exit 1; }

make_mock_cargo() {
  local dir="$1"
  cat > "$dir/mock-cargo" <<'MOCK'
#!/usr/bin/env bash
set -euo pipefail
args="$6"
output_path="${args##*output=}"
cat > "$output_path" <<JSON
{"users_without_roles_total":0,"orphan_user_roles_total":0,"orphan_role_permissions_total":0}
JSON
MOCK
  chmod +x "$dir/mock-cargo"
}

make_mock_curl() {
  local dir="$1"
  cat > "$dir/mock-curl" <<'MOCK'
#!/usr/bin/env bash
set -euo pipefail
state_file="${MOCK_CURL_STATE_FILE:?}"
count=0
if [[ -f "$state_file" ]]; then
  count="$(cat "$state_file")"
fi
count=$((count + 1))
printf '%s' "$count" > "$state_file"
cat <<METRICS
rustok_rbac_engine_mismatch_total 0
rustok_rbac_shadow_compare_failures_total 0
rustok_rbac_permission_checks_denied $((count * 2))
rustok_rbac_permission_checks_allowed $((count * 10))
METRICS
MOCK
  chmod +x "$dir/mock-curl"
}

test_workflow_runs_end_to_end() {
  local tmp
  tmp="$(mktemp -d)"
  make_mock_cargo "$tmp"
  make_mock_curl "$tmp"

  cat > "$tmp/auth.md" <<'MD'
# auth gate
MD

  MOCK_CURL_STATE_FILE="$tmp/curl.state" \
  RUSTOK_CARGO_BIN="$tmp/mock-cargo" \
  RUSTOK_CURL_BIN="$tmp/mock-curl" \
  "$SCRIPT" \
    --auth-gate-report "$tmp/auth.md" \
    --staging-artifacts-dir "$tmp/staging" \
    --cutover-artifacts-dir "$tmp/cutover" \
    --samples 2 \
    --interval-sec 0 \
    --rehearsal-cmd "printf 'workflow-ok'" >"$tmp/out.log" 2>&1

  rg -q '\[workflow\] running staging rehearsal' "$tmp/out.log" || fail "expected staging step log"
  rg -q '\[workflow\] running cutover baseline' "$tmp/out.log" || fail "expected baseline step log"
  rg -q '\[workflow\] running cutover gate' "$tmp/out.log" || fail "expected gate step log"
  [[ -f "$(find "$tmp/staging" -maxdepth 1 -name 'rbac_relation_stage_report_*.json' | head -n 1)" ]] || fail "expected stage summary json"
  [[ -f "$(find "$tmp/cutover" -maxdepth 1 -name 'rbac_cutover_baseline_*.json' | head -n 1)" ]] || fail "expected baseline json"
  [[ -f "$tmp/cutover/gate-decision.json" ]] || fail "expected gate decision json"
  python - "$tmp/cutover/gate-decision.json" <<'PY' || fail "expected workflow decision payload"
import json
import pathlib
import sys
payload = json.loads(pathlib.Path(sys.argv[1]).read_text())
if payload.get('decision') != 'go':
    raise SystemExit('decision must be go')
if payload.get('staging_rehearsal_status') != 'passed':
    raise SystemExit('staging_rehearsal_status must be passed')
PY
  pass "workflow runs staging baseline and gate end-to-end"
}

test_workflow_runs_end_to_end

echo "All rbac_cutover_workflow.sh tests passed."
