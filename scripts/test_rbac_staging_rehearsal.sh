#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
WORK_DIR="$(mktemp -d)"
trap 'rm -rf "$WORK_DIR"' EXIT

FAKE_CARGO="$WORK_DIR/fake-cargo.sh"
ARTIFACTS_DIR="$WORK_DIR/artifacts"
STATE_FILE="$WORK_DIR/state"
CMD_LOG="$WORK_DIR/cmd.log"

cat > "$FAKE_CARGO" <<'FAKE'
#!/usr/bin/env bash
set -euo pipefail

echo "$*" >> "${MOCK_CMD_LOG:?}"
count=0
if [[ -f "${MOCK_STATE_FILE:?}" ]]; then
  count="$(cat "$MOCK_STATE_FILE")"
fi
count=$((count + 1))
printf '%s' "$count" > "$MOCK_STATE_FILE"

args="$6"
output_path="${args##*output=}"
cat > "$output_path" <<JSON
{"users_without_roles_total":0,"orphan_user_roles_total":0,"orphan_role_permissions_total":0}
JSON
FAKE
chmod +x "$FAKE_CARGO"

MOCK_STATE_FILE="$STATE_FILE" MOCK_CMD_LOG="$CMD_LOG" RUSTOK_CARGO_BIN="$FAKE_CARGO" \
  "$ROOT_DIR/scripts/rbac_staging_rehearsal.sh" \
  --artifacts-dir "$ARTIFACTS_DIR" \
  --rehearsal-cmd "printf 'stage-ok'" >/dev/null

REPORT_FILE="$(find "$ARTIFACTS_DIR" -maxdepth 1 -name 'rbac_relation_stage_report_*.md' | head -n 1)"
REPORT_JSON="$(find "$ARTIFACTS_DIR" -maxdepth 1 -name 'rbac_relation_stage_report_*.json' | head -n 1)"
LOG_FILE="$(find "$ARTIFACTS_DIR" -maxdepth 1 -name 'rbac_rehearsal_*.log' | head -n 1)"
[[ -n "$REPORT_FILE" && -f "$REPORT_FILE" ]] || { echo "missing stage report" >&2; exit 1; }
[[ -n "$REPORT_JSON" && -f "$REPORT_JSON" ]] || { echo "missing stage report json" >&2; exit 1; }
[[ -n "$LOG_FILE" && -f "$LOG_FILE" ]] || { echo "missing rehearsal log" >&2; exit 1; }

grep -Fq "rehearsal_status: passed" "$REPORT_FILE"
python - "$REPORT_JSON" <<'PY'
import json, pathlib, sys
payload = json.loads(pathlib.Path(sys.argv[1]).read_text())
assert payload['rehearsal_status'] == 'passed'
assert payload['rehearsal_exit_code'] == 0
assert payload['invariants']['users_without_roles_total']['delta'] == 0
PY
grep -Fq "stage-ok" "$LOG_FILE"

echo "rbac_staging_rehearsal smoke test passed"
