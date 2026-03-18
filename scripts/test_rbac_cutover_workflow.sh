#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
WORK_DIR="$(mktemp -d)"
trap 'rm -rf "$WORK_DIR"' EXIT

FAKE_CARGO="$WORK_DIR/fake-cargo.sh"
FAKE_CURL="$WORK_DIR/fake-curl.sh"
AUTH_REPORT="$WORK_DIR/auth.md"
STAGING_DIR="$WORK_DIR/staging"
CUTOVER_DIR="$WORK_DIR/cutover"
CURL_STATE="$WORK_DIR/curl.state"

cat > "$FAKE_CARGO" <<'FAKE'
#!/usr/bin/env bash
set -euo pipefail
args="$6"
output_path="${args##*output=}"
cat > "$output_path" <<JSON
{"users_without_roles_total":0,"orphan_user_roles_total":0,"orphan_role_permissions_total":0}
JSON
FAKE
chmod +x "$FAKE_CARGO"

cat > "$FAKE_CURL" <<'FAKE'
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
FAKE
chmod +x "$FAKE_CURL"

cat > "$AUTH_REPORT" <<'MD'
# auth gate
MD

MOCK_CURL_STATE_FILE="$CURL_STATE" RUSTOK_CARGO_BIN="$FAKE_CARGO" RUSTOK_CURL_BIN="$FAKE_CURL" \
  "$ROOT_DIR/scripts/rbac_cutover_workflow.sh" \
  --auth-gate-report "$AUTH_REPORT" \
  --staging-artifacts-dir "$STAGING_DIR" \
  --cutover-artifacts-dir "$CUTOVER_DIR" \
  --samples 2 \
  --interval-sec 0 \
  --rehearsal-cmd "printf 'smoke-ok'" >/dev/null

[[ -f "$CUTOVER_DIR/gate-decision.md" ]] || { echo "missing gate decision markdown" >&2; exit 1; }
[[ -f "$CUTOVER_DIR/gate-decision.json" ]] || { echo "missing gate decision json" >&2; exit 1; }

grep -Fq -- '- decision: go' "$CUTOVER_DIR/gate-decision.md"
python - "$CUTOVER_DIR/gate-decision.json" <<'PY'
import json, pathlib, sys
payload = json.loads(pathlib.Path(sys.argv[1]).read_text())
assert payload['decision'] == 'go'
PY

echo "rbac_cutover_workflow smoke test passed"
