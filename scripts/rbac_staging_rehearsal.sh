#!/usr/bin/env bash
set -euo pipefail

usage() {
  cat <<'USAGE'
Usage:
  scripts/rbac_staging_rehearsal.sh [options]

Options:
  --artifacts-dir <dir>      Output folder for staging artifacts (default: artifacts/rbac-staging)
  --rehearsal-cmd <cmd>      Optional shell command to run between pre/post RBAC reports
  --report-task <name>       Loco task name used for RBAC report collection (default: cleanup)
  --cargo-bin <path>         Cargo executable to use (default: cargo or $RUSTOK_CARGO_BIN)
  --help                     Show this message

Outputs:
  - rbac_report_pre_<ts>.json
  - rbac_report_post_<ts>.json
  - rbac_relation_stage_report_<ts>.md
  - rehearsal command log when --rehearsal-cmd is provided
USAGE
}

ARTIFACTS_DIR="artifacts/rbac-staging"
REHEARSAL_CMD=""
REPORT_TASK_NAME="cleanup"
CARGO_BIN="${RUSTOK_CARGO_BIN:-cargo}"

while [[ $# -gt 0 ]]; do
  case "$1" in
    --artifacts-dir)
      ARTIFACTS_DIR="$2"; shift 2 ;;
    --rehearsal-cmd)
      REHEARSAL_CMD="$2"; shift 2 ;;
    --report-task)
      REPORT_TASK_NAME="$2"; shift 2 ;;
    --cargo-bin)
      CARGO_BIN="$2"; shift 2 ;;
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
PRE_JSON="$ARTIFACTS_DIR/rbac_report_pre_${TS}.json"
POST_JSON="$ARTIFACTS_DIR/rbac_report_post_${TS}.json"
REPORT_MD="$ARTIFACTS_DIR/rbac_relation_stage_report_${TS}.md"
REPORT_JSON="$ARTIFACTS_DIR/rbac_relation_stage_report_${TS}.json"
REHEARSAL_LOG="$ARTIFACTS_DIR/rbac_rehearsal_${TS}.log"

run_rbac_report() {
  local output_path="$1"
  "$CARGO_BIN" loco task --name "$REPORT_TASK_NAME" --args "target=rbac-report output=${output_path}"
}

json_field() {
  local file="$1"
  local key="$2"
  python - "$file" "$key" <<'PY'
import json
import sys
with open(sys.argv[1], 'r', encoding='utf-8') as fh:
    payload = json.load(fh)
value = payload.get(sys.argv[2])
if not isinstance(value, int):
    raise SystemExit(f"field must be integer: {sys.argv[2]}")
print(value)
PY
}

run_rbac_report "$PRE_JSON"

rehearsal_status="skipped"
rehearsal_exit_code=0
if [[ -n "$REHEARSAL_CMD" ]]; then
  rehearsal_status="passed"
  if bash -lc "$REHEARSAL_CMD" >"$REHEARSAL_LOG" 2>&1; then
    :
  else
    rehearsal_exit_code=$?
    rehearsal_status="failed"
  fi
fi

run_rbac_report "$POST_JSON"

pre_users_without_roles="$(json_field "$PRE_JSON" "users_without_roles_total")"
pre_orphan_user_roles="$(json_field "$PRE_JSON" "orphan_user_roles_total")"
pre_orphan_role_permissions="$(json_field "$PRE_JSON" "orphan_role_permissions_total")"
post_users_without_roles="$(json_field "$POST_JSON" "users_without_roles_total")"
post_orphan_user_roles="$(json_field "$POST_JSON" "orphan_user_roles_total")"
post_orphan_role_permissions="$(json_field "$POST_JSON" "orphan_role_permissions_total")"

users_without_roles_delta="$((post_users_without_roles - pre_users_without_roles))"
orphan_user_roles_delta="$((post_orphan_user_roles - pre_orphan_user_roles))"
orphan_role_permissions_delta="$((post_orphan_role_permissions - pre_orphan_role_permissions))"

{
  echo "# RBAC relation staging report"
  echo
  echo "- timestamp: ${TS}"
  echo "- report_task: ${REPORT_TASK_NAME}"
  echo "- rehearsal_status: ${rehearsal_status}"
  echo "- rehearsal_exit_code: ${rehearsal_exit_code}"
  echo "- pre_json: ${PRE_JSON}"
  echo "- post_json: ${POST_JSON}"
  if [[ -n "$REHEARSAL_CMD" ]]; then
    printf -- '- rehearsal_cmd: `%s`\n' "$REHEARSAL_CMD"
    echo "- rehearsal_log: ${REHEARSAL_LOG}"
  else
    echo "- rehearsal_cmd: not provided"
  fi
  echo
  echo "## Invariants"
  echo "- users_without_roles_total: ${pre_users_without_roles} -> ${post_users_without_roles} (delta ${users_without_roles_delta})"
  echo "- orphan_user_roles_total: ${pre_orphan_user_roles} -> ${post_orphan_user_roles} (delta ${orphan_user_roles_delta})"
  echo "- orphan_role_permissions_total: ${pre_orphan_role_permissions} -> ${post_orphan_role_permissions} (delta ${orphan_role_permissions_delta})"
} > "$REPORT_MD"

python - <<'PY' "$REPORT_JSON" "$TS" "$REPORT_TASK_NAME" "$rehearsal_status" "$rehearsal_exit_code" "$PRE_JSON" "$POST_JSON" "$REHEARSAL_CMD" "$REHEARSAL_LOG" "$pre_users_without_roles" "$post_users_without_roles" "$users_without_roles_delta" "$pre_orphan_user_roles" "$post_orphan_user_roles" "$orphan_user_roles_delta" "$pre_orphan_role_permissions" "$post_orphan_role_permissions" "$orphan_role_permissions_delta"
import json
import pathlib
import sys

report_json = pathlib.Path(sys.argv[1])
payload = {
    "timestamp": sys.argv[2],
    "report_task": sys.argv[3],
    "rehearsal_status": sys.argv[4],
    "rehearsal_exit_code": int(sys.argv[5]),
    "pre_json": sys.argv[6],
    "post_json": sys.argv[7],
    "rehearsal_cmd": sys.argv[8] or None,
    "rehearsal_log": sys.argv[9] if sys.argv[8] else None,
    "invariants": {
        "users_without_roles_total": {
            "pre": int(sys.argv[10]),
            "post": int(sys.argv[11]),
            "delta": int(sys.argv[12]),
        },
        "orphan_user_roles_total": {
            "pre": int(sys.argv[13]),
            "post": int(sys.argv[14]),
            "delta": int(sys.argv[15]),
        },
        "orphan_role_permissions_total": {
            "pre": int(sys.argv[16]),
            "post": int(sys.argv[17]),
            "delta": int(sys.argv[18]),
        },
    },
    "invariant_deltas": {
        "users_without_roles_total": int(sys.argv[12]),
        "orphan_user_roles_total": int(sys.argv[15]),
        "orphan_role_permissions_total": int(sys.argv[18]),
    },
}
report_json.write_text(json.dumps(payload, indent=2) + "\n", encoding='utf-8')
PY

echo "RBAC staging rehearsal bundle created"
echo "- timestamp: $TS"
echo "- pre_json: $PRE_JSON"
echo "- post_json: $POST_JSON"
echo "- report_md: $REPORT_MD"
echo "- report_json: $REPORT_JSON"
if [[ -n "$REHEARSAL_CMD" ]]; then
  echo "- rehearsal_log: $REHEARSAL_LOG"
fi

if [[ "$rehearsal_status" == "failed" ]]; then
  echo "Rehearsal command failed. See log: $REHEARSAL_LOG" >&2
  cat "$REHEARSAL_LOG" >&2
  exit "$rehearsal_exit_code"
fi
