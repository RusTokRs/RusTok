#!/usr/bin/env bash
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
REPORT_SCRIPT="$REPO_ROOT/scripts/verify/report-control-plane-remediation-progress.py"

FIXTURE_ROOT="$(mktemp -d)"
trap 'rm -rf "$FIXTURE_ROOT"' EXIT

PLAN_FIXTURE="$FIXTURE_ROOT/plan.md"
cat > "$PLAN_FIXTURE" <<'MD'
- [x] done one
- [~] in progress one
- [ ] pending one
- [~] in progress two
MD

OUTPUT="$(RUSTOK_REMEDIATION_PLAN_PATH="$PLAN_FIXTURE" python3 "$REPORT_SCRIPT")"

if ! grep -q "completed: 1" <<<"$OUTPUT"; then
  echo "expected completed count missing" >&2
  echo "$OUTPUT" >&2
  exit 1
fi
if ! grep -q "in_progress: 2" <<<"$OUTPUT"; then
  echo "expected in_progress count missing" >&2
  echo "$OUTPUT" >&2
  exit 1
fi
if ! grep -q "pending: 1" <<<"$OUTPUT"; then
  echo "expected pending count missing" >&2
  echo "$OUTPUT" >&2
  exit 1
fi


JSON_OUTPUT="$(RUSTOK_REMEDIATION_PLAN_PATH="$PLAN_FIXTURE" python3 "$REPORT_SCRIPT" --json)"
if ! grep -q '"completed": 1' <<<"$JSON_OUTPUT"; then
  echo "expected json completed count missing" >&2
  echo "$JSON_OUTPUT" >&2
  exit 1
fi
if ! grep -q '"in_progress": 2' <<<"$JSON_OUTPUT"; then
  echo "expected json in_progress count missing" >&2
  echo "$JSON_OUTPUT" >&2
  exit 1
fi

MISSING_PATH="$FIXTURE_ROOT/missing-plan.md"
MISSING_OUTPUT="$(RUSTOK_REMEDIATION_PLAN_PATH="$MISSING_PATH" python3 "$REPORT_SCRIPT" || true)"
if ! grep -q "ERROR: remediation plan not found" <<<"$MISSING_OUTPUT"; then
  echo "expected missing-plan error message" >&2
  echo "$MISSING_OUTPUT" >&2
  exit 1
fi


set +e
RUSTOK_REMEDIATION_PLAN_PATH="$PLAN_FIXTURE" python3 "$REPORT_SCRIPT" --fail-on-pending >/tmp/remediation_fail_on_pending.out 2>&1
FAIL_CODE=$?
set -e
if [[ "$FAIL_CODE" -ne 2 ]]; then
  echo "expected exit code 2 for --fail-on-pending with pending items, got $FAIL_CODE" >&2
  cat /tmp/remediation_fail_on_pending.out >&2
  exit 1
fi
if ! grep -q "FAIL: pending remediation items detected" /tmp/remediation_fail_on_pending.out; then
  echo "expected fail-on-pending message missing" >&2
  cat /tmp/remediation_fail_on_pending.out >&2
  exit 1
fi

echo "control_plane_remediation_progress_report_test.sh: PASS"
