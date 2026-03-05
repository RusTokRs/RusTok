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
  find "$tmp/cutover" -maxdepth 1 -name 'rbac_cutover_gate_decision_*.md' | rg -q . || fail "expected default gate decision artifact"
  pass "gate passes when required artifacts are valid"
}

test_passes_with_custom_decision_output() {
  local tmp
  tmp="$(mktemp -d)"
  make_artifacts "$tmp"
  local out_file="$tmp/out/gate-decision.md"

  "$SCRIPT" \
    --staging-artifacts-dir "$tmp/staging" \
    --cutover-artifacts-dir "$tmp/cutover" \
    --auth-gate-report "$tmp/auth/auth_release_gate_20260305.md" \
    --decision-output "$out_file" >"$tmp/out.log" 2>&1

  [[ -f "$out_file" ]] || fail "expected custom decision output file"
  rg -q "decision: PASS" "$out_file" || fail "expected PASS decision in custom output"
  rg -q "auth_gate_report:" "$out_file" || fail "expected auth gate path in custom output"
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
test_fails_without_required_flag

echo "All rbac_cutover_gate.sh tests passed."
