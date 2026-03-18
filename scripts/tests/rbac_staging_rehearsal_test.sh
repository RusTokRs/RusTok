#!/usr/bin/env bash
set -euo pipefail

SCRIPT="scripts/rbac_staging_rehearsal.sh"

pass() { echo "[PASS] $1"; }
fail() { echo "[FAIL] $1" >&2; exit 1; }

make_mock_cargo() {
  local dir="$1"
  cat > "$dir/mock-cargo" <<'MOCK'
#!/usr/bin/env bash
set -euo pipefail

state_file="${MOCK_CARGO_STATE_FILE:?}"
profile="${MOCK_CARGO_PROFILE:-stable}"
cmd_log="${MOCK_CARGO_CMD_LOG:?}"

echo "$*" >> "$cmd_log"

count=0
if [[ -f "$state_file" ]]; then
  count="$(cat "$state_file")"
fi
count=$((count + 1))
printf '%s' "$count" > "$state_file"

if [[ "$1" != "loco" || "$2" != "task" || "$3" != "--name" ]]; then
  echo "unexpected cargo invocation: $*" >&2
  exit 1
fi

args="$6"
output_path="${args##*output=}"

case "$profile" in
  drift)
    users_without_roles=$((count - 1))
    orphan_user_roles=$((count - 1))
    orphan_role_permissions=$((count - 1))
    ;;
  stable|*)
    users_without_roles=0
    orphan_user_roles=0
    orphan_role_permissions=0
    ;;
esac

cat > "$output_path" <<JSON
{"users_without_roles_total":${users_without_roles},"orphan_user_roles_total":${orphan_user_roles},"orphan_role_permissions_total":${orphan_role_permissions}}
JSON
MOCK
  chmod +x "$dir/mock-cargo"
}

test_collects_pre_and_post_reports_without_rehearsal_command() {
  local tmp
  tmp="$(mktemp -d)"
  make_mock_cargo "$tmp"

  MOCK_CARGO_STATE_FILE="$tmp/state" \
  MOCK_CARGO_CMD_LOG="$tmp/cmd.log" \
  RUSTOK_CARGO_BIN="$tmp/mock-cargo" \
  "$SCRIPT" --artifacts-dir "$tmp/artifacts" >"$tmp/out.log" 2>&1

  report_file="$(find "$tmp/artifacts" -maxdepth 1 -name 'rbac_relation_stage_report_*.md' | head -n 1)"
  report_json="$(find "$tmp/artifacts" -maxdepth 1 -name 'rbac_relation_stage_report_*.json' | head -n 1)"
  pre_json="$(find "$tmp/artifacts" -maxdepth 1 -name 'rbac_report_pre_*.json' | head -n 1)"
  post_json="$(find "$tmp/artifacts" -maxdepth 1 -name 'rbac_report_post_*.json' | head -n 1)"

  [[ -n "$report_file" && -f "$report_file" ]] || fail "expected stage report markdown"
  [[ -n "$report_json" && -f "$report_json" ]] || fail "expected stage report json"
  [[ -n "$pre_json" && -f "$pre_json" ]] || fail "expected pre report json"
  [[ -n "$post_json" && -f "$post_json" ]] || fail "expected post report json"
  rg -q 'rehearsal_cmd: not provided' "$report_file" || fail "expected report to note missing rehearsal command"
  python - "$report_json" <<'PY' || fail "expected machine-readable stage report json"
import json
import pathlib
import sys
payload = json.loads(pathlib.Path(sys.argv[1]).read_text())
if payload.get('rehearsal_status') != 'skipped':
    raise SystemExit('rehearsal_status must be skipped')
if payload.get('rehearsal_exit_code') != 0:
    raise SystemExit('rehearsal_exit_code must be 0')
if payload['invariants']['users_without_roles_total']['pre'] != 0:
    raise SystemExit('users_without_roles_total.pre must be 0')
if payload['invariants']['users_without_roles_total']['post'] != 0:
    raise SystemExit('users_without_roles_total.post must be 0')
if payload['invariants']['users_without_roles_total']['delta'] != 0:
    raise SystemExit('users_without_roles_total.delta must be 0')
PY
  [[ "$(wc -l < "$tmp/cmd.log")" -eq 2 ]] || fail "expected exactly two cargo report invocations"
  pass "staging rehearsal helper collects pre/post reports without rehearsal command"
}

test_runs_rehearsal_command_and_writes_log() {
  local tmp
  tmp="$(mktemp -d)"
  make_mock_cargo "$tmp"

  MOCK_CARGO_STATE_FILE="$tmp/state" \
  MOCK_CARGO_CMD_LOG="$tmp/cmd.log" \
  RUSTOK_CARGO_BIN="$tmp/mock-cargo" \
  "$SCRIPT" \
    --artifacts-dir "$tmp/artifacts" \
    --rehearsal-cmd "printf 'rehearsal-ok'" >"$tmp/out.log" 2>&1

  log_file="$(find "$tmp/artifacts" -maxdepth 1 -name 'rbac_rehearsal_*.log' | head -n 1)"
  report_file="$(find "$tmp/artifacts" -maxdepth 1 -name 'rbac_relation_stage_report_*.md' | head -n 1)"
  [[ -f "$log_file" ]] || fail "expected rehearsal log file"
  rg -q 'rehearsal-ok' "$log_file" || fail "expected rehearsal output in log"
  rg -q 'rehearsal_status: passed' "$report_file" || fail "expected passed rehearsal status"
  pass "staging rehearsal helper runs rehearsal command and stores log"
}

test_fails_when_rehearsal_command_fails() {
  local tmp
  tmp="$(mktemp -d)"
  make_mock_cargo "$tmp"

  set +e
  MOCK_CARGO_STATE_FILE="$tmp/state" \
  MOCK_CARGO_CMD_LOG="$tmp/cmd.log" \
  RUSTOK_CARGO_BIN="$tmp/mock-cargo" \
  "$SCRIPT" \
    --artifacts-dir "$tmp/artifacts" \
    --rehearsal-cmd "echo boom >&2; exit 7" >"$tmp/out.log" 2>&1
  code=$?
  set -e

  [[ "$code" -ne 0 ]] || fail "expected non-zero exit when rehearsal command fails"
  rg -q 'Rehearsal command failed' "$tmp/out.log" || fail "expected failure message"
  report_file="$(find "$tmp/artifacts" -maxdepth 1 -name 'rbac_relation_stage_report_*.md' | head -n 1)"
  post_json="$(find "$tmp/artifacts" -maxdepth 1 -name 'rbac_report_post_*.json' | head -n 1)"
  [[ -f "$report_file" ]] || fail "expected report file even on rehearsal failure"
  [[ -f "$post_json" ]] || fail "expected post report json even on rehearsal failure"
  rg -q 'rehearsal_status: failed' "$report_file" || fail "expected failed rehearsal status in report"
  rg -q 'rehearsal_exit_code:' "$report_file" || fail "expected rehearsal exit code in report"
  pass "staging rehearsal helper fails after persisting failure artifacts"
}



test_report_render_does_not_reexecute_rehearsal_command() {
  local tmp
  tmp="$(mktemp -d)"
  make_mock_cargo "$tmp"

  local marker="$tmp/rehearsal-count.txt"
  MOCK_CARGO_STATE_FILE="$tmp/state" \
  MOCK_CARGO_CMD_LOG="$tmp/cmd.log" \
  RUSTOK_CARGO_BIN="$tmp/mock-cargo" \
  "$SCRIPT" \
    --artifacts-dir "$tmp/artifacts" \
    --rehearsal-cmd "printf 'x' >> '$marker'" >"$tmp/out.log" 2>&1

  [[ -f "$marker" ]] || fail "expected rehearsal side-effect marker"
  [[ "$(cat "$marker")" == "x" ]] || fail "expected rehearsal command to run exactly once"
  report_file="$(find "$tmp/artifacts" -maxdepth 1 -name 'rbac_relation_stage_report_*.md' | head -n 1)"
  python - "$report_file" "$marker" <<'PY' || fail "expected literal rehearsal command in report"
import pathlib
import sys
report = pathlib.Path(sys.argv[1]).read_text()
marker = sys.argv[2]
expected = f"rehearsal_cmd: `printf 'x' >> '{marker}'`"
if expected not in report:
    raise SystemExit('literal rehearsal command missing from report')
PY
  pass "staging rehearsal report rendering does not re-execute rehearsal command"
}

test_report_includes_invariant_deltas() {
  local tmp
  tmp="$(mktemp -d)"
  make_mock_cargo "$tmp"

  MOCK_CARGO_STATE_FILE="$tmp/state" \
  MOCK_CARGO_CMD_LOG="$tmp/cmd.log" \
  MOCK_CARGO_PROFILE=drift \
  RUSTOK_CARGO_BIN="$tmp/mock-cargo" \
  "$SCRIPT" --artifacts-dir "$tmp/artifacts" >"$tmp/out.log" 2>&1

  report_file="$(find "$tmp/artifacts" -maxdepth 1 -name 'rbac_relation_stage_report_*.md' | head -n 1)"
  rg -q 'users_without_roles_total: 0 -> 1 \(delta 1\)' "$report_file" || fail "expected users_without_roles delta in report"
  rg -q 'orphan_user_roles_total: 0 -> 1 \(delta 1\)' "$report_file" || fail "expected orphan_user_roles delta in report"
  rg -q 'orphan_role_permissions_total: 0 -> 1 \(delta 1\)' "$report_file" || fail "expected orphan_role_permissions delta in report"
  pass "staging rehearsal helper reports invariant deltas"
}

test_collects_pre_and_post_reports_without_rehearsal_command
test_runs_rehearsal_command_and_writes_log
test_fails_when_rehearsal_command_fails
test_report_render_does_not_reexecute_rehearsal_command
test_report_includes_invariant_deltas

echo "All rbac_staging_rehearsal.sh tests passed."
