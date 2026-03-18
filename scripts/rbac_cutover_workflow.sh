#!/usr/bin/env bash
set -euo pipefail

usage() {
  cat <<'USAGE'
Usage:
  scripts/rbac_cutover_workflow.sh [options]

Options:
  --auth-gate-report <file>       Path to auth release gate report (required)
  --staging-artifacts-dir <dir>   Staging artifacts directory (default: artifacts/rbac-staging)
  --cutover-artifacts-dir <dir>   Cutover artifacts directory (default: artifacts/rbac-cutover)
  --rehearsal-cmd <cmd>           Optional command passed to rbac_staging_rehearsal.sh
  --metrics-url <url>             Metrics endpoint passed to rbac_cutover_baseline.sh
  --samples <N>                   Baseline samples count (default: 7)
  --interval-sec <N>              Baseline scrape interval seconds (default: 60)
  --min-decision-delta <N>        Baseline minimum decision delta (default: 1)
  --allow-engine-mismatch         Forwarded to baseline helper
  --allow-shadow-failures         Forwarded to baseline helper
  --no-save-samples               Forwarded to baseline helper
  --decision-output <file>        Optional gate markdown output
  --decision-json-output <file>   Optional gate JSON output
  --phase <value>                 Gate phase label (default: C2)
  --owner <value>                 Gate owner label (default: platform/backend)
  --help                          Show this message
USAGE
}

AUTH_GATE_REPORT=""
STAGING_ARTIFACTS_DIR="artifacts/rbac-staging"
CUTOVER_ARTIFACTS_DIR="artifacts/rbac-cutover"
REHEARSAL_CMD=""
METRICS_URL="http://127.0.0.1:5150/metrics"
SAMPLES=7
INTERVAL_SEC=60
MIN_DECISION_DELTA=1
ALLOW_ENGINE_MISMATCH="false"
ALLOW_SHADOW_FAILURES="false"
SAVE_SAMPLES="true"
DECISION_OUTPUT=""
DECISION_JSON_OUTPUT=""
PHASE="C2"
OWNER="platform/backend"

while [[ $# -gt 0 ]]; do
  case "$1" in
    --auth-gate-report)
      AUTH_GATE_REPORT="$2"; shift 2 ;;
    --staging-artifacts-dir)
      STAGING_ARTIFACTS_DIR="$2"; shift 2 ;;
    --cutover-artifacts-dir)
      CUTOVER_ARTIFACTS_DIR="$2"; shift 2 ;;
    --rehearsal-cmd)
      REHEARSAL_CMD="$2"; shift 2 ;;
    --metrics-url)
      METRICS_URL="$2"; shift 2 ;;
    --samples)
      SAMPLES="$2"; shift 2 ;;
    --interval-sec)
      INTERVAL_SEC="$2"; shift 2 ;;
    --min-decision-delta)
      MIN_DECISION_DELTA="$2"; shift 2 ;;
    --allow-engine-mismatch)
      ALLOW_ENGINE_MISMATCH="true"; shift ;;
    --allow-shadow-failures)
      ALLOW_SHADOW_FAILURES="true"; shift ;;
    --no-save-samples)
      SAVE_SAMPLES="false"; shift ;;
    --decision-output)
      DECISION_OUTPUT="$2"; shift 2 ;;
    --decision-json-output)
      DECISION_JSON_OUTPUT="$2"; shift 2 ;;
    --phase)
      PHASE="$2"; shift 2 ;;
    --owner)
      OWNER="$2"; shift 2 ;;
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

mkdir -p "$STAGING_ARTIFACTS_DIR" "$CUTOVER_ARTIFACTS_DIR"

stage_cmd=(scripts/rbac_staging_rehearsal.sh --artifacts-dir "$STAGING_ARTIFACTS_DIR")
if [[ -n "$REHEARSAL_CMD" ]]; then
  stage_cmd+=(--rehearsal-cmd "$REHEARSAL_CMD")
fi

baseline_cmd=(
  scripts/rbac_cutover_baseline.sh
  --metrics-url "$METRICS_URL"
  --samples "$SAMPLES"
  --interval-sec "$INTERVAL_SEC"
  --artifacts-dir "$CUTOVER_ARTIFACTS_DIR"
  --min-decision-delta "$MIN_DECISION_DELTA"
)
if [[ "$ALLOW_ENGINE_MISMATCH" == "true" ]]; then
  baseline_cmd+=(--allow-engine-mismatch)
fi
if [[ "$ALLOW_SHADOW_FAILURES" == "true" ]]; then
  baseline_cmd+=(--allow-shadow-failures)
fi
if [[ "$SAVE_SAMPLES" == "false" ]]; then
  baseline_cmd+=(--no-save-samples)
fi

gate_cmd=(
  scripts/rbac_cutover_gate.sh
  --staging-artifacts-dir "$STAGING_ARTIFACTS_DIR"
  --cutover-artifacts-dir "$CUTOVER_ARTIFACTS_DIR"
  --auth-gate-report "$AUTH_GATE_REPORT"
  --phase "$PHASE"
  --owner "$OWNER"
)
if [[ -n "$DECISION_OUTPUT" ]]; then
  gate_cmd+=(--decision-output "$DECISION_OUTPUT")
fi
if [[ -n "$DECISION_JSON_OUTPUT" ]]; then
  gate_cmd+=(--decision-json-output "$DECISION_JSON_OUTPUT")
fi

echo "[workflow] running staging rehearsal"
"${stage_cmd[@]}"

echo "[workflow] running cutover baseline"
"${baseline_cmd[@]}"

echo "[workflow] running cutover gate"
"${gate_cmd[@]}"
