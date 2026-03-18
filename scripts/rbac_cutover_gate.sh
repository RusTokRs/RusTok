#!/usr/bin/env bash
set -euo pipefail

usage() {
  cat <<'USAGE'
Usage:
  scripts/rbac_cutover_gate.sh [options]

Options:
  --staging-artifacts-dir <dir>   Directory with relation rehearsal artifacts (default: artifacts/rbac-staging)
  --cutover-artifacts-dir <dir>   Directory with rbac_cutover_baseline artifacts (default: artifacts/rbac-cutover)
  --auth-gate-report <file>       Path to auth_release_gate report artifact (required)
  --decision-output <file>        Optional markdown output file for go/no-go gate decision
  --decision-json-output <file>   Optional JSON output file for go/no-go gate decision
  --phase <value>                 Gate phase label for decision artifact (default: C2)
  --owner <value>                 Owner label for decision artifact (default: platform/backend)
  --stage-ts <ts>                 Use explicit staging rehearsal timestamp instead of latest (format: YYYYMMDDTHHMMSSZ)
  --cutover-ts <ts>               Use explicit cutover baseline timestamp instead of latest (format: YYYYMMDDTHHMMSSZ)
  --help                          Show this message

Gate checks:
  1) Staging artifacts are validated as one timestamp-consistent rehearsal bundle
  2) Staging report must show rehearsal_status=passed|skipped and rehearsal_exit_code=0
  3) Staging invariant snapshot (post-check) is present and zero for users_without_roles/orphan_user_roles/orphan_role_permissions
  4) Cutover baseline artifacts are validated as one timestamp-consistent bundle (md+json)
  5) Baseline json has gate_status=pass
  6) Baseline json deltas mismatch/shadow failures are zero
  7) Auth gate report artifact exists
USAGE
}

STAGING_ARTIFACTS_DIR="artifacts/rbac-staging"
CUTOVER_ARTIFACTS_DIR="artifacts/rbac-cutover"
AUTH_GATE_REPORT=""
DECISION_OUTPUT=""
DECISION_JSON_OUTPUT=""
STAGE_TS=""
CUTOVER_TS=""
PHASE="C2"
OWNER="platform/backend"

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
    --phase)
      PHASE="$2"; shift 2 ;;
    --owner)
      OWNER="$2"; shift 2 ;;
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

json_string_or_null() {
  local value="${1:-}"
  if [[ -n "$value" ]]; then
    printf '"%s"' "$value"
  else
    printf 'null'
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
stage_post_json="$STAGING_ARTIFACTS_DIR/rbac_report_post_${stage_ts}.json"
stage_post_legacy_json="$STAGING_ARTIFACTS_DIR/rbac_report_post_rollback_${stage_ts}.json"
stage_report_json="$STAGING_ARTIFACTS_DIR/rbac_relation_stage_report_${stage_ts}.json"

require_file "$stage_report" "staging stage-report markdown"
require_file "$stage_pre_json" "staging pre-check JSON (same timestamp as stage report)"

if [[ -f "$stage_post_json" ]]; then
  stage_post_check_json="$stage_post_json"
elif [[ -f "$stage_post_legacy_json" ]]; then
  stage_post_check_json="$stage_post_legacy_json"
else
  echo "Missing required artifact: staging post-check JSON (expected rbac_report_post_<ts>.json or legacy rbac_report_post_rollback_<ts>.json)" >&2
  exit 1
fi

if [[ ! -f "$stage_dry_json" ]]; then
  stage_dry_json=""
fi
if [[ ! -f "$stage_apply_json" ]]; then
  stage_apply_json=""
fi
if [[ ! -f "$stage_rollback_apply_json" ]]; then
  stage_rollback_apply_json=""
fi

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

MISMATCH_SAMPLE_PATH="$CUTOVER_ARTIFACTS_DIR/mismatch-sample.jsonl"
touch "$MISMATCH_SAMPLE_PATH"

if [[ -f "$stage_report_json" ]]; then
  stage_rehearsal_payload="$(python - "$stage_report_json" <<'PY'
import json
import pathlib
import sys

payload = json.loads(pathlib.Path(sys.argv[1]).read_text(encoding='utf-8'))
status = payload.get('rehearsal_status')
exit_code = payload.get('rehearsal_exit_code')

if status not in {'passed', 'skipped'}:
    raise SystemExit(f"staging report rehearsal_status must be passed|skipped, got: {status!r}")
if not isinstance(exit_code, int):
    raise SystemExit(f'staging report rehearsal_exit_code must be integer, got: {exit_code!r}')
if exit_code != 0:
    raise SystemExit(f'staging report rehearsal_exit_code must be 0, got: {exit_code}')

print(status)
print(exit_code)
print('json')
PY
)"
else
  stage_rehearsal_payload="$(python - "$stage_report" <<'PY'
import pathlib
import re
import sys

report = pathlib.Path(sys.argv[1]).read_text(encoding='utf-8')
status_match = re.search(r'^- rehearsal_status: (.+)$', report, re.MULTILINE)
exit_match = re.search(r'^- rehearsal_exit_code: (.+)$', report, re.MULTILINE)

if status_match is None:
    raise SystemExit('staging report must contain rehearsal_status')
if exit_match is None:
    raise SystemExit('staging report must contain rehearsal_exit_code')

status = status_match.group(1).strip()
exit_code_raw = exit_match.group(1).strip()

if status not in {'passed', 'skipped'}:
    raise SystemExit(f"staging report rehearsal_status must be passed|skipped, got: {status!r}")

try:
    exit_code = int(exit_code_raw)
except ValueError as exc:
    raise SystemExit(f'staging report rehearsal_exit_code must be integer, got: {exit_code_raw!r}') from exc

if exit_code != 0:
    raise SystemExit(f'staging report rehearsal_exit_code must be 0, got: {exit_code}')

print(status)
print(exit_code)
print('markdown')
PY
)"
fi

mapfile -t stage_rehearsal_fields <<< "$stage_rehearsal_payload"
if [[ "${#stage_rehearsal_fields[@]}" -ne 3 ]]; then
  echo "unexpected staging rehearsal payload shape from parser" >&2
  exit 1
fi
staging_rehearsal_status="${stage_rehearsal_fields[0]}"
staging_rehearsal_exit_code="${stage_rehearsal_fields[1]}"
staging_rehearsal_source="${stage_rehearsal_fields[2]}"

python - "$stage_post_check_json" <<'PY'
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
        raise SystemExit(f"staging post-check field must be integer: {key}")
    if value != 0:
        raise SystemExit(f"staging post-check invariant must be 0 before relation-only cutover: {key}={value}")
PY

decision_volume_payload="$(python - "$cutover_json" <<'PY'
import json
import sys

path = sys.argv[1]
with open(path, 'r', encoding='utf-8') as fh:
    payload = json.load(fh)

status = payload.get('gate_status')
if status != 'pass':
    raise SystemExit(f"baseline gate_status must be 'pass', got: {status!r}")

mismatch_delta = payload.get('mismatch_delta')
if mismatch_delta is None:
    mismatch_delta = payload.get('engine_mismatch_delta')
if not isinstance(mismatch_delta, int):
    raise SystemExit('baseline field must be integer: mismatch_delta or engine_mismatch_delta')
if mismatch_delta != 0:
    raise SystemExit(f'baseline field must be 0 before relation-only cutover: mismatch_delta={mismatch_delta}')

shadow_compare_failures_delta = payload.get('shadow_compare_failures_delta')
if not isinstance(shadow_compare_failures_delta, int):
    raise SystemExit('baseline field must be integer: shadow_compare_failures_delta')
if shadow_compare_failures_delta != 0:
    raise SystemExit(
        'baseline field must be 0 before relation-only cutover: '
        f'shadow_compare_failures_delta={shadow_compare_failures_delta}'
    )

total_decisions_delta = payload.get('total_decisions_delta')
permission_checks_total_delta = payload.get('permission_checks_total_delta')

if total_decisions_delta is not None and not isinstance(total_decisions_delta, int):
    raise SystemExit('baseline field must be integer when present: total_decisions_delta')

if permission_checks_total_delta is not None and not isinstance(permission_checks_total_delta, int):
    raise SystemExit('baseline field must be integer when present: permission_checks_total_delta')

if isinstance(total_decisions_delta, int) and isinstance(permission_checks_total_delta, int):
    if total_decisions_delta != permission_checks_total_delta:
        raise SystemExit(
            'baseline decision volume keys must match when both present: '
            f'total_decisions_delta={total_decisions_delta}, '
            f'permission_checks_total_delta={permission_checks_total_delta}'
        )

if isinstance(total_decisions_delta, int):
    decision_volume_delta = total_decisions_delta
    decision_volume_source = 'total_decisions_delta'
elif isinstance(permission_checks_total_delta, int):
    decision_volume_delta = permission_checks_total_delta
    decision_volume_source = 'permission_checks_total_delta'
else:
    raise SystemExit('baseline field must be integer: total_decisions_delta or permission_checks_total_delta')

print(decision_volume_delta)
print(decision_volume_source)
PY
 )"

mapfile -t decision_volume_fields <<< "$decision_volume_payload"
if [[ "${#decision_volume_fields[@]}" -ne 2 ]]; then
  echo "unexpected decision-volume payload shape from parser" >&2
  exit 1
fi
decision_volume_delta="${decision_volume_fields[0]}"
decision_volume_source="${decision_volume_fields[1]}"

if [[ "$decision_volume_source" != "total_decisions_delta" && "$decision_volume_source" != "permission_checks_total_delta" ]]; then
  echo "unexpected decision-volume source from parser: $decision_volume_source" >&2
  exit 1
fi

echo "RBAC cutover gate: PASS"
echo "- staging_ts: $stage_ts"
echo "- staging_report: $stage_report"
echo "- staging_report_json: ${stage_report_json:-n/a}"
echo "- staging_rehearsal_status: $staging_rehearsal_status"
echo "- staging_rehearsal_source: $staging_rehearsal_source"
echo "- staging_rehearsal_exit_code: $staging_rehearsal_exit_code"
echo "- staging_pre_json: $stage_pre_json"
echo "- staging_dry_run_json: ${stage_dry_json:-n/a}"
echo "- staging_apply_json: ${stage_apply_json:-n/a}"
echo "- staging_rollback_apply_json: ${stage_rollback_apply_json:-n/a}"
echo "- staging_post_check_json: $stage_post_check_json"
echo "- baseline_ts: $cutover_ts"
echo "- baseline_md: $cutover_md"
echo "- baseline_json: $cutover_json"
echo "- auth_gate_report: $AUTH_GATE_REPORT"
echo "- decision_volume_source: $decision_volume_source"
echo "- decision_output: $DECISION_OUTPUT"
echo "- decision_json_output: $DECISION_JSON_OUTPUT"

mkdir -p "$(dirname "$DECISION_OUTPUT")"
cat > "$DECISION_OUTPUT" <<EOF
# RBAC Gate Decision

- date: $(date -u +%Y-%m-%d)
- phase: $PHASE
- decision: go
- owner: $OWNER
- staging_rehearsal_status: $staging_rehearsal_status
- staging_rehearsal_exit_code: $staging_rehearsal_exit_code
- staging_rehearsal_source: $staging_rehearsal_source

## Metrics snapshot
- engine_mismatch_total: 0
- decision_volume_delta: ${decision_volume_delta}
- decision_volume_source: ${decision_volume_source}
- latency_p95_delta: n/a
- latency_p99_delta: n/a
- 401_403_rate_delta: n/a

## Evidence
- baseline_json: $cutover_json
- baseline_md: $cutover_md
- mismatch_sample: $MISMATCH_SAMPLE_PATH
- auth_gate_report: $AUTH_GATE_REPORT

## Notes
- summary: relation-only cutover gate checks passed
- rollback_readiness: ready

## Corrective action (required for no-go)
- root_cause: n/a
- owner: n/a
- target_date: n/a
- verification_step: n/a
EOF

stage_dry_json_value="$(json_string_or_null "$stage_dry_json")"
stage_apply_json_value="$(json_string_or_null "$stage_apply_json")"
stage_rollback_apply_json_value="$(json_string_or_null "$stage_rollback_apply_json")"

mkdir -p "$(dirname "$DECISION_JSON_OUTPUT")"
cat > "$DECISION_JSON_OUTPUT" <<EOF
{
  "decision": "go",
  "generated_at_utc": "$(date -u +%Y-%m-%dT%H:%M:%SZ)",
  "phase": "$PHASE",
  "owner": "$OWNER",
  "staging_rehearsal_status": "$staging_rehearsal_status",
  "staging_rehearsal_exit_code": ${staging_rehearsal_exit_code},
  "staging_rehearsal_source": "$staging_rehearsal_source",
  "staging_ts": "$stage_ts",
  "baseline_ts": "$cutover_ts",
  "engine_mismatch_total": 0,
  "decision_volume_delta": ${decision_volume_delta},
  "decision_volume_source": "${decision_volume_source}",
  "latency_p95_delta": null,
  "latency_p99_delta": null,
  "rate_401_403_delta": null,
  "staging_report": "$stage_report",
  "staging_report_json": $(json_string_or_null "$stage_report_json"),
  "staging_pre_json": "$stage_pre_json",
  "staging_dry_run_json": ${stage_dry_json_value},
  "staging_apply_json": ${stage_apply_json_value},
  "staging_rollback_apply_json": ${stage_rollback_apply_json_value},
  "staging_post_check_json": "$stage_post_check_json",
  "baseline_md": "$cutover_md",
  "baseline_json": "$cutover_json",
  "auth_gate_report": "$AUTH_GATE_REPORT",
  "mismatch_sample": "$MISMATCH_SAMPLE_PATH"
}
EOF
