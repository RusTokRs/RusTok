#!/usr/bin/env bash
set -euo pipefail

SCRIPT="scripts/rbac_cutover_gate.sh"

pass() { echo "[PASS] $1"; }
fail() { echo "[FAIL] $1" >&2; exit 1; }

make_artifacts() {
  local root="$1"
  mkdir -p "$root/staging" "$root/cutover" "$root/auth"

  cat > "$root/staging/rbac_relation_stage_report_20260305T010101Z.md" <<'MD'
# stage report
MD
  cat > "$root/staging/rbac_report_pre_20260305T010101Z.json" <<'JSON'
{"users_without_roles_total":0,"orphan_user_roles_total":0,"orphan_role_permissions_total":0}
JSON
  cat > "$root/staging/rbac_backfill_dry_run_20260305T010101Z.json" <<'JSON'
{"dry_run":true,"candidates_total":3}
JSON
  cat > "$root/staging/rbac_backfill_apply_20260305T010101Z.json" <<'JSON'
{"dry_run":false,"fixed_users":3}
JSON
  cat > "$root/staging/rbac_backfill_rollback_apply_20260305T010101Z.json" <<'JSON'
{"dry_run":false,"reverted":3}
JSON
  cat > "$root/staging/rbac_report_post_rollback_20260305T010101Z.json" <<'JSON'
{"users_without_roles_total":0,"orphan_user_roles_total":0,"orphan_role_permissions_total":0}
JSON

  cat > "$root/cutover/rbac_cutover_baseline_20260305T020202Z.md" <<'MD'
# baseline
MD
  cat > "$root/cutover/rbac_cutover_baseline_20260305T020202Z.json" <<'JSON'
{"gate_status":"pass","mismatch_delta":0,"shadow_compare_failures_delta":0,"total_decisions_delta":10}
JSON

  cat > "$root/auth/auth_release_gate_20260305.md" <<'MD'
# auth gate
MD
}

test_passes_with_required_artifacts() {
  local tmp
  tmp="$(mktemp -d)"
  make_artifacts "$tmp"

  "$SCRIPT" \
    --staging-artifacts-dir "$tmp/staging" \
    --cutover-artifacts-dir "$tmp/cutover" \
    --auth-gate-report "$tmp/auth/auth_release_gate_20260305.md" >"$tmp/out.log" 2>&1

  rg -q "RBAC cutover gate: PASS" "$tmp/out.log" || fail "expected PASS output"
  rg -q "decision_output:" "$tmp/out.log" || fail "expected decision output path in stdout"
  rg -q "decision_json_output:" "$tmp/out.log" || fail "expected decision json output path in stdout"
  [[ -f "$tmp/cutover/gate-decision.md" ]] || fail "expected default gate markdown decision artifact"
  [[ -f "$tmp/cutover/gate-decision.json" ]] || fail "expected default gate json decision artifact"
  [[ -f "$tmp/cutover/mismatch-sample.jsonl" ]] || fail "expected default mismatch sample artifact"
  rg -q "# RBAC Gate Decision" "$tmp/cutover/gate-decision.md" || fail "expected template markdown header"
  rg -q -- "- decision: go" "$tmp/cutover/gate-decision.md" || fail "expected go decision in markdown"
  rg -q -- "- decision_volume_source: total_decisions_delta" "$tmp/cutover/gate-decision.md" || fail "expected decision_volume_source in markdown"
  python - "$tmp/cutover/gate-decision.json" <<'PY' || fail "expected valid json decision artifact"
import json
import sys
with open(sys.argv[1], 'r', encoding='utf-8') as fh:
    payload = json.load(fh)
if payload.get('decision') != 'go':
    raise SystemExit('decision must be go')
if payload.get('decision_volume_delta') != 10:
    raise SystemExit('decision_volume_delta must be 10')
if payload.get('decision_volume_source') != 'total_decisions_delta':
    raise SystemExit('decision_volume_source must be total_decisions_delta')
PY
  pass "gate passes when required artifacts are valid"
}

test_passes_with_custom_decision_output() {
  local tmp
  tmp="$(mktemp -d)"
  make_artifacts "$tmp"
  local out_file="$tmp/out/gate-decision.md"
  local out_json="$tmp/out/gate-decision.json"

  "$SCRIPT" \
    --staging-artifacts-dir "$tmp/staging" \
    --cutover-artifacts-dir "$tmp/cutover" \
    --auth-gate-report "$tmp/auth/auth_release_gate_20260305.md" \
    --decision-output "$out_file" \
    --decision-json-output "$out_json" >"$tmp/out.log" 2>&1

  [[ -f "$out_file" ]] || fail "expected custom decision output file"
  [[ -f "$out_json" ]] || fail "expected custom decision json output file"
  rg -q -- "- decision: go" "$out_file" || fail "expected go decision in custom output"
  rg -q "auth_gate_report:" "$out_file" || fail "expected auth gate path in custom output"
  python - "$out_json" <<'PY' || fail "expected custom decision json payload"
import json
import sys
with open(sys.argv[1], 'r', encoding='utf-8') as fh:
    payload = json.load(fh)
if payload.get('decision') != 'go':
    raise SystemExit('decision must be go')
if 'auth_gate_report' not in payload:
    raise SystemExit('auth_gate_report must be present')
PY
  pass "gate writes decision artifact to custom output path"
}

test_passes_with_explicit_timestamps() {
  local tmp
  tmp="$(mktemp -d)"
  make_artifacts "$tmp"

  "$SCRIPT" \
    --staging-artifacts-dir "$tmp/staging" \
    --cutover-artifacts-dir "$tmp/cutover" \
    --auth-gate-report "$tmp/auth/auth_release_gate_20260305.md" \
    --stage-ts 20260305T010101Z \
    --cutover-ts 20260305T020202Z >"$tmp/out.log" 2>&1

  rg -q "staging_ts: 20260305T010101Z" "$tmp/out.log" || fail "expected explicit staging ts in output"
  rg -q "baseline_ts: 20260305T020202Z" "$tmp/out.log" || fail "expected explicit cutover ts in output"
  pass "gate passes with explicit stage/cutover timestamps"
}

test_fails_on_invalid_explicit_stage_ts_format() {
  local tmp
  tmp="$(mktemp -d)"
  make_artifacts "$tmp"

  set +e
  "$SCRIPT"     --staging-artifacts-dir "$tmp/staging"     --cutover-artifacts-dir "$tmp/cutover"     --auth-gate-report "$tmp/auth/auth_release_gate_20260305.md"     --stage-ts bad-ts >"$tmp/out.log" 2>&1
  code=$?
  set -e

  [[ "$code" -ne 0 ]] || fail "expected non-zero exit for invalid stage ts format"
  rg -q "Invalid stage timestamp format" "$tmp/out.log" || fail "expected invalid stage ts message"
  pass "gate fails on invalid explicit stage ts format"
}


test_fails_on_invalid_explicit_cutover_ts_format() {
  local tmp
  tmp="$(mktemp -d)"
  make_artifacts "$tmp"

  set +e
  "$SCRIPT"     --staging-artifacts-dir "$tmp/staging"     --cutover-artifacts-dir "$tmp/cutover"     --auth-gate-report "$tmp/auth/auth_release_gate_20260305.md"     --cutover-ts 2026-03-05 >"$tmp/out.log" 2>&1
  code=$?
  set -e

  [[ "$code" -ne 0 ]] || fail "expected non-zero exit for invalid cutover ts format"
  rg -q "Invalid cutover timestamp format" "$tmp/out.log" || fail "expected invalid cutover ts message"
  pass "gate fails on invalid explicit cutover ts format"
}


test_fails_when_auth_gate_report_missing() {
  local tmp
  tmp="$(mktemp -d)"
  make_artifacts "$tmp"

  set +e
  "$SCRIPT" \
    --staging-artifacts-dir "$tmp/staging" \
    --cutover-artifacts-dir "$tmp/cutover" \
    --auth-gate-report "$tmp/auth/missing.md" >"$tmp/out.log" 2>&1
  code=$?
  set -e

  [[ "$code" -ne 0 ]] || fail "expected non-zero exit when auth report is missing"
  rg -q "Missing required artifact: auth release gate report" "$tmp/out.log" || fail "expected missing auth report message"
  pass "gate fails when auth gate report is missing"
}

test_fails_when_baseline_not_pass() {
  local tmp
  tmp="$(mktemp -d)"
  make_artifacts "$tmp"

  cat > "$tmp/cutover/rbac_cutover_baseline_20260305T020202Z.json" <<'JSON'
{"gate_status":"fail","mismatch_delta":0,"shadow_compare_failures_delta":0,"total_decisions_delta":10}
JSON

  set +e
  "$SCRIPT" \
    --staging-artifacts-dir "$tmp/staging" \
    --cutover-artifacts-dir "$tmp/cutover" \
    --auth-gate-report "$tmp/auth/auth_release_gate_20260305.md" >"$tmp/out.log" 2>&1
  code=$?
  set -e

  [[ "$code" -ne 0 ]] || fail "expected non-zero exit when baseline gate_status is fail"
  rg -q "baseline gate_status must be 'pass'" "$tmp/out.log" || fail "expected baseline gate_status failure message"
  pass "gate fails when baseline gate_status is not pass"
}

test_fails_when_post_rollback_invariants_nonzero() {
  local tmp
  tmp="$(mktemp -d)"
  make_artifacts "$tmp"

  cat > "$tmp/staging/rbac_report_post_rollback_20260305T010101Z.json" <<'JSON'
{"users_without_roles_total":1,"orphan_user_roles_total":0,"orphan_role_permissions_total":0}
JSON

  set +e
  "$SCRIPT" \
    --staging-artifacts-dir "$tmp/staging" \
    --cutover-artifacts-dir "$tmp/cutover" \
    --auth-gate-report "$tmp/auth/auth_release_gate_20260305.md" >"$tmp/out.log" 2>&1
  code=$?
  set -e

  [[ "$code" -ne 0 ]] || fail "expected non-zero exit when post-rollback invariants are non-zero"
  rg -q "staging post-rollback invariant must be 0" "$tmp/out.log" || fail "expected post-rollback invariant failure message"
  pass "gate fails when post-rollback invariants are non-zero"
}

test_fails_when_stage_bundle_timestamp_mismatch() {
  local tmp
  tmp="$(mktemp -d)"
  make_artifacts "$tmp"

  mv "$tmp/staging/rbac_report_pre_20260305T010101Z.json" "$tmp/staging/rbac_report_pre_20260305T999999Z.json"

  set +e
  "$SCRIPT" \
    --staging-artifacts-dir "$tmp/staging" \
    --cutover-artifacts-dir "$tmp/cutover" \
    --auth-gate-report "$tmp/auth/auth_release_gate_20260305.md" >"$tmp/out.log" 2>&1
  code=$?
  set -e

  [[ "$code" -ne 0 ]] || fail "expected non-zero exit when stage bundle timestamps are mismatched"
  rg -q "same timestamp as stage report" "$tmp/out.log" || fail "expected stage bundle timestamp mismatch message"
  pass "gate fails when stage bundle timestamps are mismatched"
}

test_fails_when_cutover_bundle_timestamp_mismatch() {
  local tmp
  tmp="$(mktemp -d)"
  make_artifacts "$tmp"

  mv "$tmp/cutover/rbac_cutover_baseline_20260305T020202Z.json" "$tmp/cutover/rbac_cutover_baseline_20260305T999999Z.json"

  set +e
  "$SCRIPT" \
    --staging-artifacts-dir "$tmp/staging" \
    --cutover-artifacts-dir "$tmp/cutover" \
    --auth-gate-report "$tmp/auth/auth_release_gate_20260305.md" >"$tmp/out.log" 2>&1
  code=$?
  set -e

  [[ "$code" -ne 0 ]] || fail "expected non-zero exit when cutover bundle timestamps are mismatched"
  rg -q "same timestamp as markdown" "$tmp/out.log" || fail "expected cutover bundle timestamp mismatch message"
  pass "gate fails when cutover bundle timestamps are mismatched"
}

test_fails_when_mismatch_delta_nonzero() {
  local tmp
  tmp="$(mktemp -d)"
  make_artifacts "$tmp"

  cat > "$tmp/cutover/rbac_cutover_baseline_20260305T020202Z.json" <<'JSON'
{"gate_status":"pass","mismatch_delta":1,"shadow_compare_failures_delta":0,"total_decisions_delta":10}
JSON

  set +e
  "$SCRIPT" \
    --staging-artifacts-dir "$tmp/staging" \
    --cutover-artifacts-dir "$tmp/cutover" \
    --auth-gate-report "$tmp/auth/auth_release_gate_20260305.md" >"$tmp/out.log" 2>&1
  code=$?
  set -e

  [[ "$code" -ne 0 ]] || fail "expected non-zero exit when mismatch delta is non-zero"
  rg -q "mismatch_delta=1" "$tmp/out.log" || fail "expected mismatch delta failure message"
  pass "gate fails when mismatch delta is non-zero"
}

test_passes_with_permission_checks_total_delta_key() {
  local tmp
  tmp="$(mktemp -d)"
  make_artifacts "$tmp"

  cat > "$tmp/cutover/rbac_cutover_baseline_20260305T020202Z.json" <<'JSON'
{"gate_status":"pass","mismatch_delta":0,"shadow_compare_failures_delta":0,"permission_checks_total_delta":14}
JSON

  "$SCRIPT" \
    --staging-artifacts-dir "$tmp/staging" \
    --cutover-artifacts-dir "$tmp/cutover" \
    --auth-gate-report "$tmp/auth/auth_release_gate_20260305.md" >"$tmp/out.log" 2>&1

  rg -q -- "- decision_volume_delta: 14" "$tmp/cutover/gate-decision.md" || fail "expected permission_checks_total_delta propagated to markdown"
  rg -q -- "- decision_volume_source: permission_checks_total_delta" "$tmp/cutover/gate-decision.md" || fail "expected permission_checks_total_delta source in markdown"
  python - "$tmp/cutover/gate-decision.json" <<'PY' || fail "expected permission_checks_total_delta propagated to json"
import json
import sys
with open(sys.argv[1], 'r', encoding='utf-8') as fh:
    payload = json.load(fh)
if payload.get('decision_volume_delta') != 14:
    raise SystemExit('decision_volume_delta must be 14')
if payload.get('decision_volume_source') != 'permission_checks_total_delta':
    raise SystemExit('decision_volume_source must be permission_checks_total_delta')
PY
  pass "gate accepts permission_checks_total_delta as decision volume"
}

test_fails_when_total_decisions_delta_non_integer() {
  local tmp
  tmp="$(mktemp -d)"
  make_artifacts "$tmp"

  cat > "$tmp/cutover/rbac_cutover_baseline_20260305T020202Z.json" <<'JSON'
{"gate_status":"pass","mismatch_delta":0,"shadow_compare_failures_delta":0,"total_decisions_delta":"10"}
JSON

  set +e
  "$SCRIPT" \
    --staging-artifacts-dir "$tmp/staging" \
    --cutover-artifacts-dir "$tmp/cutover" \
    --auth-gate-report "$tmp/auth/auth_release_gate_20260305.md" >"$tmp/out.log" 2>&1
  code=$?
  set -e

  [[ "$code" -ne 0 ]] || fail "expected non-zero exit when total_decisions_delta is non-integer"
  rg -q "baseline field must be integer when present: total_decisions_delta" "$tmp/out.log" || fail "expected total_decisions_delta integer validation message"
  pass "gate fails when total_decisions_delta is non-integer"
}

test_fails_when_decision_volume_key_missing() {
  local tmp
  tmp="$(mktemp -d)"
  make_artifacts "$tmp"

  cat > "$tmp/cutover/rbac_cutover_baseline_20260305T020202Z.json" <<'JSON'
{"gate_status":"pass","mismatch_delta":0,"shadow_compare_failures_delta":0}
JSON

  set +e
  "$SCRIPT" \
    --staging-artifacts-dir "$tmp/staging" \
    --cutover-artifacts-dir "$tmp/cutover" \
    --auth-gate-report "$tmp/auth/auth_release_gate_20260305.md" >"$tmp/out.log" 2>&1
  code=$?
  set -e

  [[ "$code" -ne 0 ]] || fail "expected non-zero exit when both decision volume keys are missing"
  rg -q "total_decisions_delta or permission_checks_total_delta" "$tmp/out.log" || fail "expected decision volume key validation message"
  pass "gate fails when both decision volume keys are missing"
}


test_fails_when_decision_volume_keys_disagree() {
  local tmp
  tmp="$(mktemp -d)"
  make_artifacts "$tmp"

  cat > "$tmp/cutover/rbac_cutover_baseline_20260305T020202Z.json" <<'JSON'
{"gate_status":"pass","mismatch_delta":0,"shadow_compare_failures_delta":0,"total_decisions_delta":10,"permission_checks_total_delta":14}
JSON

  set +e
  "$SCRIPT"     --staging-artifacts-dir "$tmp/staging"     --cutover-artifacts-dir "$tmp/cutover"     --auth-gate-report "$tmp/auth/auth_release_gate_20260305.md" >"$tmp/out.log" 2>&1
  code=$?
  set -e

  [[ "$code" -ne 0 ]] || fail "expected non-zero exit when decision volume keys disagree"
  rg -q "baseline decision volume keys must match when both present" "$tmp/out.log" || fail "expected disagreeing decision volume keys message"
  pass "gate fails when decision volume keys disagree"
}

test_fails_when_permission_checks_total_delta_non_integer() {
  local tmp
  tmp="$(mktemp -d)"
  make_artifacts "$tmp"

  cat > "$tmp/cutover/rbac_cutover_baseline_20260305T020202Z.json" <<'JSON'
{"gate_status":"pass","mismatch_delta":0,"shadow_compare_failures_delta":0,"permission_checks_total_delta":"14"}
JSON

  set +e
  "$SCRIPT"     --staging-artifacts-dir "$tmp/staging"     --cutover-artifacts-dir "$tmp/cutover"     --auth-gate-report "$tmp/auth/auth_release_gate_20260305.md" >"$tmp/out.log" 2>&1
  code=$?
  set -e

  [[ "$code" -ne 0 ]] || fail "expected non-zero exit when permission_checks_total_delta is non-integer"
  rg -q "baseline field must be integer when present: permission_checks_total_delta" "$tmp/out.log" || fail "expected permission_checks_total_delta integer validation message"
  pass "gate fails when permission_checks_total_delta is non-integer"
}

test_fails_without_required_flag() {
  local tmp
  tmp="$(mktemp -d)"
  make_artifacts "$tmp"

  set +e
  "$SCRIPT" --staging-artifacts-dir "$tmp/staging" --cutover-artifacts-dir "$tmp/cutover" >"$tmp/out.log" 2>&1
  code=$?
  set -e

  [[ "$code" -ne 0 ]] || fail "expected non-zero exit when --auth-gate-report is not provided"
  rg -q -- "--auth-gate-report is required" "$tmp/out.log" || fail "expected required flag message"
  pass "gate enforces --auth-gate-report"
}

test_passes_with_required_artifacts
test_passes_with_custom_decision_output
test_passes_with_explicit_timestamps
test_fails_on_invalid_explicit_stage_ts_format
test_fails_on_invalid_explicit_cutover_ts_format
test_fails_when_auth_gate_report_missing
test_fails_when_baseline_not_pass
test_fails_when_post_rollback_invariants_nonzero
test_fails_when_stage_bundle_timestamp_mismatch
test_fails_when_cutover_bundle_timestamp_mismatch
test_fails_when_mismatch_delta_nonzero
test_passes_with_permission_checks_total_delta_key
test_fails_when_decision_volume_key_missing
test_fails_when_decision_volume_keys_disagree
test_fails_when_permission_checks_total_delta_non_integer
test_fails_when_total_decisions_delta_non_integer
test_fails_without_required_flag

echo "All rbac_cutover_gate.sh tests passed."
