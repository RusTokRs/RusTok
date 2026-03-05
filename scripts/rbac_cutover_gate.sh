#!/usr/bin/env bash
set -euo pipefail

usage() {
  cat <<'USAGE'
Usage:
  scripts/rbac_cutover_gate.sh [options]

Options:
  --staging-artifacts-dir <dir>   Directory with rbac_relation_staging artifacts (default: artifacts/rbac-staging)
  --cutover-artifacts-dir <dir>   Directory with rbac_cutover_baseline artifacts (default: artifacts/rbac-cutover)
  --auth-gate-report <file>       Path to auth_release_gate report artifact (required)
  --decision-output <file>        Optional markdown output file for go/no-go gate decision
  --decision-json-output <file>   Optional JSON output file for go/no-go gate decision
  --stage-ts <ts>                 Use explicit staging rehearsal timestamp instead of latest (format: YYYYMMDDTHHMMSSZ)
  --cutover-ts <ts>               Use explicit cutover baseline timestamp instead of latest (format: YYYYMMDDTHHMMSSZ)
  --help                          Show this message

Gate checks:
  1) Staging artifacts are validated as one timestamp-consistent rehearsal bundle
  2) Staging post-rollback invariants are zero (users_without_roles/orphan_user_roles/orphan_role_permissions)
  3) Cutover baseline artifacts are validated as one timestamp-consistent bundle (md+json)
  4) Baseline json has gate_status=pass
  5) Baseline json deltas mismatch/shadow failures are zero
  6) Auth gate report artifact exists
USAGE
}

STAGING_ARTIFACTS_DIR="artifacts/rbac-staging"
CUTOVER_ARTIFACTS_DIR="artifacts/rbac-cutover"
AUTH_GATE_REPORT=""
DECISION_OUTPUT=""
DECISION_JSON_OUTPUT=""
STAGE_TS=""
CUTOVER_TS=""

while [[ $# -gt 0 ]]; do
  case "$1" in
    --staging-artifacts-dir)
      STAGING_ARTIFACTS_DIR="$2"; shift 2 ;;
    --cutover-artifacts-dir)
      CUTOVER_ARTIFACTS_DIR="$2"; shift 2 ;;
    --auth-gate-report)
      AUTH_GATE_REPORT="$2"; shift 2 ;;
    --decision-output)
      DECISION_OUTPUT="$2"; shift 2 ;;
    --decision-json-output)
      DECISION_JSON_OUTPUT="$2"; shift 2 ;;
    --stage-ts)
      STAGE_TS="$2"; shift 2 ;;
    --cutover-ts)
      CUTOVER_TS="$2"; shift 2 ;;
    --help)
      usage; exit 0 ;;
    *)
      echo "Unknown option: $1" >&2
      usage
      exit 1 ;;
  esac
done

if [[ -z "$AUTH_GATE_REPORT" ]]; then
  echo "--auth-gate-report is required." >&2
  exit 1
fi

latest_file() {
  local dir="$1"
  local pattern="$2"
  find "$dir" -maxdepth 1 -type f -name "$pattern" | sort | tail -n 1
}

require_file() {
  local path="$1"
  local label="$2"
  if [[ -z "$path" || ! -f "$path" ]]; then
    echo "Missing required artifact: ${label}" >&2
    exit 1
  fi
}

extract_ts() {
  local path="$1"
  local prefix="$2"
  local suffix="$3"
  local base
  base="$(basename "$path")"
  if [[ "$base" =~ ^${prefix}_(.+)\.${suffix}$ ]]; then
    printf '%s' "${BASH_REMATCH[1]}"
    return 0
  fi
  return 1
}

validate_ts() {
  local ts="$1"
  local label="$2"
  if [[ ! "$ts" =~ ^[0-9]{8}T[0-9]{6}Z$ ]]; then
    echo "Invalid ${label} timestamp format: ${ts} (expected YYYYMMDDTHHMMSSZ)." >&2
    exit 1
  fi
}

if [[ ! -d "$STAGING_ARTIFACTS_DIR" ]]; then
  echo "Staging artifacts directory does not exist: $STAGING_ARTIFACTS_DIR" >&2
  exit 1
fi
if [[ ! -d "$CUTOVER_ARTIFACTS_DIR" ]]; then
  echo "Cutover artifacts directory does not exist: $CUTOVER_ARTIFACTS_DIR" >&2
  exit 1
fi

if [[ -n "$STAGE_TS" ]]; then
  stage_ts="$STAGE_TS"
  validate_ts "$stage_ts" "stage"
  stage_report="$STAGING_ARTIFACTS_DIR/rbac_relation_stage_report_${stage_ts}.md"
else
  stage_report="$(latest_file "$STAGING_ARTIFACTS_DIR" 'rbac_relation_stage_report_*.md')"
  require_file "$stage_report" "staging stage-report markdown"
  stage_ts="$(extract_ts "$stage_report" "rbac_relation_stage_report" "md" || true)"
  if [[ -z "$stage_ts" ]]; then
    echo "Could not extract timestamp from staging report: $stage_report" >&2
    exit 1
  fi
  validate_ts "$stage_ts" "stage"
fi

stage_pre_json="$STAGING_ARTIFACTS_DIR/rbac_report_pre_${stage_ts}.json"
stage_dry_json="$STAGING_ARTIFACTS_DIR/rbac_backfill_dry_run_${stage_ts}.json"
stage_apply_json="$STAGING_ARTIFACTS_DIR/rbac_backfill_apply_${stage_ts}.json"
stage_rollback_apply_json="$STAGING_ARTIFACTS_DIR/rbac_backfill_rollback_apply_${stage_ts}.json"
stage_post_rollback_json="$STAGING_ARTIFACTS_DIR/rbac_report_post_rollback_${stage_ts}.json"

require_file "$stage_report" "staging stage-report markdown"
require_file "$stage_pre_json" "staging pre-check JSON (same timestamp as stage report)"
require_file "$stage_dry_json" "staging dry-run JSON (same timestamp as stage report)"
require_file "$stage_apply_json" "staging apply JSON (same timestamp as stage report)"
require_file "$stage_rollback_apply_json" "staging rollback-apply JSON (same timestamp as stage report)"
require_file "$stage_post_rollback_json" "staging post-rollback JSON (same timestamp as stage report)"

if [[ -n "$CUTOVER_TS" ]]; then
  cutover_ts="$CUTOVER_TS"
  validate_ts "$cutover_ts" "cutover"
  cutover_md="$CUTOVER_ARTIFACTS_DIR/rbac_cutover_baseline_${cutover_ts}.md"
else
  cutover_md="$(latest_file "$CUTOVER_ARTIFACTS_DIR" 'rbac_cutover_baseline_*.md')"
  require_file "$cutover_md" "cutover baseline markdown"
  cutover_ts="$(extract_ts "$cutover_md" "rbac_cutover_baseline" "md" || true)"
  if [[ -z "$cutover_ts" ]]; then
    echo "Could not extract timestamp from cutover baseline markdown: $cutover_md" >&2
    exit 1
  fi
  validate_ts "$cutover_ts" "cutover"
fi
cutover_json="$CUTOVER_ARTIFACTS_DIR/rbac_cutover_baseline_${cutover_ts}.json"

require_file "$cutover_md" "cutover baseline markdown"
require_file "$cutover_json" "cutover baseline JSON (same timestamp as markdown)"
require_file "$AUTH_GATE_REPORT" "auth release gate report"

if [[ -z "$DECISION_OUTPUT" ]]; then
  DECISION_OUTPUT="$CUTOVER_ARTIFACTS_DIR/gate-decision.md"
fi

if [[ -z "$DECISION_JSON_OUTPUT" ]]; then
  DECISION_JSON_OUTPUT="$CUTOVER_ARTIFACTS_DIR/gate-decision.json"
fi

python - "$stage_post_rollback_json" <<'PY'
import json
import sys

path = sys.argv[1]
with open(path, 'r', encoding='utf-8') as fh:
    payload = json.load(fh)

for key in (
    'users_without_roles_total',
    'orphan_user_roles_total',
    'orphan_role_permissions_total',
):
    value = payload.get(key)
    if not isinstance(value, int):
        raise SystemExit(f"staging post-rollback field must be integer: {key}")
    if value != 0:
        raise SystemExit(f"staging post-rollback invariant must be 0 before relation-only cutover: {key}={value}")
PY

python - "$cutover_json" <<'PY'
import json
import sys

path = sys.argv[1]
with open(path, 'r', encoding='utf-8') as fh:
    payload = json.load(fh)

status = payload.get('gate_status')
if status != 'pass':
    raise SystemExit(f"baseline gate_status must be 'pass', got: {status!r}")

for key in ('mismatch_delta', 'shadow_compare_failures_delta'):
    value = payload.get(key)
    if not isinstance(value, int):
        raise SystemExit(f"baseline field must be integer: {key}")
    if value != 0:
        raise SystemExit(f"baseline field must be 0 before relation-only cutover: {key}={value}")

if not isinstance(payload.get('total_decisions_delta'), int):
    raise SystemExit('baseline field must be integer: total_decisions_delta')
PY

echo "RBAC cutover gate: PASS"
echo "- staging_ts: $stage_ts"
echo "- staging_report: $stage_report"
echo "- staging_pre_json: $stage_pre_json"
echo "- staging_dry_run_json: $stage_dry_json"
echo "- staging_apply_json: $stage_apply_json"
echo "- staging_rollback_apply_json: $stage_rollback_apply_json"
echo "- staging_post_rollback_json: $stage_post_rollback_json"
echo "- baseline_ts: $cutover_ts"
echo "- baseline_md: $cutover_md"
echo "- baseline_json: $cutover_json"
echo "- auth_gate_report: $AUTH_GATE_REPORT"
echo "- decision_output: $DECISION_OUTPUT"
echo "- decision_json_output: $DECISION_JSON_OUTPUT"

mkdir -p "$(dirname "$DECISION_OUTPUT")"
cat > "$DECISION_OUTPUT" <<EOF
# RBAC relation-only cutover gate decision

- decision: PASS
- generated_at_utc: $(date -u +%Y-%m-%dT%H:%M:%SZ)
- staging_ts: $stage_ts
- baseline_ts: $cutover_ts
- staging_report: $stage_report
- staging_pre_json: $stage_pre_json
- staging_dry_run_json: $stage_dry_json
- staging_apply_json: $stage_apply_json
- staging_rollback_apply_json: $stage_rollback_apply_json
- staging_post_rollback_json: $stage_post_rollback_json
- baseline_md: $cutover_md
- baseline_json: $cutover_json
- auth_gate_report: $AUTH_GATE_REPORT
EOF

mkdir -p "$(dirname "$DECISION_JSON_OUTPUT")"
cat > "$DECISION_JSON_OUTPUT" <<EOF
{
  "decision": "pass",
  "generated_at_utc": "$(date -u +%Y-%m-%dT%H:%M:%SZ)",
  "staging_ts": "$stage_ts",
  "baseline_ts": "$cutover_ts",
  "staging_report": "$stage_report",
  "staging_pre_json": "$stage_pre_json",
  "staging_dry_run_json": "$stage_dry_json",
  "staging_apply_json": "$stage_apply_json",
  "staging_rollback_apply_json": "$stage_rollback_apply_json",
  "staging_post_rollback_json": "$stage_post_rollback_json",
  "baseline_md": "$cutover_md",
  "baseline_json": "$cutover_json",
  "auth_gate_report": "$AUTH_GATE_REPORT"
}
EOF
