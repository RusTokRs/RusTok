#!/usr/bin/env bash
set -euo pipefail

usage() {
  cat <<'USAGE'
Usage:
  scripts/rbac_cutover_baseline.sh [options]

Options:
  --metrics-url <url>         Prometheus text endpoint (default: http://127.0.0.1:5150/metrics)
  --samples <N>               Number of scrapes to collect (default: 7)
  --interval-sec <N>          Delay between scrapes in seconds (default: 60)
  --artifacts-dir <dir>       Output folder for baseline artifacts (default: artifacts/rbac-cutover)
  --min-decision-delta <N>    Minimum total permission decision delta required for a valid baseline (default: 1)
  --save-samples              Persist raw /metrics snapshots per sample (default: enabled)
  --no-save-samples           Do not persist raw /metrics snapshots in artifacts
  --require-zero-mismatch     Exit non-zero if mismatch counter delta is not zero (default: enabled)
  --allow-mismatch            Disable strict mismatch gate
  --allow-shadow-failures     Disable strict shadow-failures gate
  --help                      Show this message

Environment:
  RUSTOK_CURL_BIN             Override curl executable path (default: curl)

Outputs:
  - JSON snapshot with per-sample metrics and deltas
  - Markdown report with baseline summary and gate checks
USAGE
}

METRICS_URL="http://127.0.0.1:5150/metrics"
SAMPLES=7
INTERVAL_SEC=60
ARTIFACTS_DIR="artifacts/rbac-cutover"
MIN_DECISION_DELTA=1
SAVE_SAMPLES="true"
REQUIRE_ZERO_MISMATCH="true"
ALLOW_SHADOW_FAILURES="false"
REQUIRE_ZERO_SHADOW_FAILURES="true"
CURL_BIN="${RUSTOK_CURL_BIN:-curl}"

while [[ $# -gt 0 ]]; do
  case "$1" in
    --metrics-url)
      METRICS_URL="$2"; shift 2 ;;
    --samples)
      SAMPLES="$2"; shift 2 ;;
    --interval-sec)
      INTERVAL_SEC="$2"; shift 2 ;;
    --artifacts-dir)
      ARTIFACTS_DIR="$2"; shift 2 ;;
    --min-decision-delta)
      MIN_DECISION_DELTA="$2"; shift 2 ;;
    --save-samples)
      SAVE_SAMPLES="true"; shift ;;
    --no-save-samples)
      SAVE_SAMPLES="false"; shift ;;
    --require-zero-mismatch)
      REQUIRE_ZERO_MISMATCH="true"; shift ;;
    --allow-mismatch)
      REQUIRE_ZERO_MISMATCH="false"; shift ;;
    --allow-shadow-failures)
      ALLOW_SHADOW_FAILURES="true"; REQUIRE_ZERO_SHADOW_FAILURES="false"; shift ;;
    --help)
      usage; exit 0 ;;
    *)
      echo "Unknown option: $1" >&2
      usage
      exit 1 ;;
  esac
done

if ! [[ "$SAMPLES" =~ ^[0-9]+$ ]] || [[ "$SAMPLES" -lt 1 ]]; then
  echo "--samples must be a positive integer." >&2
  exit 1
fi

if ! [[ "$INTERVAL_SEC" =~ ^[0-9]+$ ]]; then
  echo "--interval-sec must be a non-negative integer." >&2
  exit 1
fi

if ! [[ "$MIN_DECISION_DELTA" =~ ^[0-9]+$ ]]; then
  echo "--min-decision-delta must be a non-negative integer." >&2
  exit 1
fi

mkdir -p "$ARTIFACTS_DIR"
TS="$(date -u +%Y%m%dT%H%M%SZ)"
JSON_FILE="$ARTIFACTS_DIR/rbac_cutover_baseline_${TS}.json"
REPORT_FILE="$ARTIFACTS_DIR/rbac_cutover_baseline_${TS}.md"

tmp_dir="$(mktemp -d)"
SAMPLES_DIR="$ARTIFACTS_DIR/rbac_cutover_samples_${TS}"
if [[ "$SAVE_SAMPLES" == "true" ]]; then
  mkdir -p "$SAMPLES_DIR"
else
  SAMPLES_DIR=""
fi
trap 'rm -rf "$tmp_dir"' EXIT

metric_value() {
  local file="$1"
  local metric="$2"
  local value
  value="$(awk -v metric="$metric" '$1 == metric { print $2; exit }' "$file")"
  if [[ -z "$value" ]]; then
    value="0"
  fi
  printf '%s' "$value"
}

to_int() {
  python - "$1" <<'PY'
import sys
raw = sys.argv[1].strip()
try:
    value = float(raw)
except Exception as exc:
    raise SystemExit(f"invalid numeric metric value '{raw}': {exc}")
print(int(value))
PY
}

sample_json_lines=()
first_mismatch=""
last_mismatch=""
first_shadow_fail=""
last_shadow_fail=""
first_denied=""
last_denied=""
first_allowed=""
last_allowed=""
counter_reset_detected="false"
counter_reset_reason=""

for ((i = 1; i <= SAMPLES; i++)); do
  sample_ts="$(date -u +%Y-%m-%dT%H:%M:%SZ)"
  sample_file="$tmp_dir/sample_${i}.prom"
  "$CURL_BIN" -fsS "$METRICS_URL" > "$sample_file"
  sample_artifact=""
  if [[ "$SAVE_SAMPLES" == "true" ]]; then
    sample_artifact="$SAMPLES_DIR/sample_${i}.prom"
    cp "$sample_file" "$sample_artifact"
  fi

  mismatch_raw="$(metric_value "$sample_file" "rustok_rbac_decision_mismatch_total")"
  shadow_fail_raw="$(metric_value "$sample_file" "rustok_rbac_shadow_compare_failures_total")"
  denied_raw="$(metric_value "$sample_file" "rustok_rbac_permission_checks_denied")"
  allowed_raw="$(metric_value "$sample_file" "rustok_rbac_permission_checks_allowed")"

  mismatch="$(to_int "$mismatch_raw")"
  shadow_fail="$(to_int "$shadow_fail_raw")"
  denied="$(to_int "$denied_raw")"
  allowed="$(to_int "$allowed_raw")"

  if [[ -z "$first_mismatch" ]]; then
    first_mismatch="$mismatch"
    first_shadow_fail="$shadow_fail"
    first_denied="$denied"
    first_allowed="$allowed"
  fi

  last_mismatch="$mismatch"
  last_shadow_fail="$shadow_fail"
  last_denied="$denied"
  last_allowed="$allowed"

  if [[ "$i" -gt 1 ]]; then
    if [[ "$mismatch" -lt "$prev_mismatch" || "$shadow_fail" -lt "$prev_shadow_fail" || "$denied" -lt "$prev_denied" || "$allowed" -lt "$prev_allowed" ]]; then
      counter_reset_detected="true"
      counter_reset_reason="one or more counters decreased between samples"
    fi
  fi

  prev_mismatch="$mismatch"
  prev_shadow_fail="$shadow_fail"
  prev_denied="$denied"
  prev_allowed="$allowed"

  sample_json_lines+=("{\"sample\":${i},\"timestamp\":\"${sample_ts}\",\"mismatch_total\":${mismatch},\"shadow_compare_failures_total\":${shadow_fail},\"permission_checks_denied\":${denied},\"permission_checks_allowed\":${allowed}}")

  if [[ "$i" -lt "$SAMPLES" && "$INTERVAL_SEC" -gt 0 ]]; then
    sleep "$INTERVAL_SEC"
  fi
done

mismatch_delta="$((last_mismatch - first_mismatch))"
shadow_fail_delta="$((last_shadow_fail - first_shadow_fail))"
denied_delta="$((last_denied - first_denied))"
allowed_delta="$((last_allowed - first_allowed))"
total_decisions_delta="$((denied_delta + allowed_delta))"

gate_status="pass"
gate_message="Mismatch delta is zero in observed window."
if [[ "$REQUIRE_ZERO_MISMATCH" == "true" && "$mismatch_delta" -ne 0 ]]; then
  gate_status="fail"
  gate_message="Mismatch delta is ${mismatch_delta}; investigate before relation-only cutover."
fi

if [[ "$gate_status" == "pass" && "$ALLOW_SHADOW_FAILURES" != "true" && "$shadow_fail_delta" -ne 0 ]]; then
  gate_status="fail"
  gate_message="Shadow compare failures delta is ${shadow_fail_delta}; stabilize shadow path before relation-only cutover."
fi

if [[ "$gate_status" == "pass" && "$counter_reset_detected" == "true" ]]; then
  gate_status="fail"
  gate_message="Counter reset detected: ${counter_reset_reason}."
fi

if [[ "$gate_status" == "pass" && "$total_decisions_delta" -lt "$MIN_DECISION_DELTA" ]]; then
  gate_status="fail"
  gate_message="Decision delta is ${total_decisions_delta}; requires at least ${MIN_DECISION_DELTA} decisions in baseline window."
fi

{
  echo "{" 
  echo "  \"metrics_url\": \"${METRICS_URL}\"," 
  echo "  \"samples\": ${SAMPLES},"
  echo "  \"interval_sec\": ${INTERVAL_SEC},"
  echo "  \"mismatch_total_start\": ${first_mismatch},"
  echo "  \"mismatch_total_end\": ${last_mismatch},"
  echo "  \"mismatch_delta\": ${mismatch_delta},"
  echo "  \"shadow_compare_failures_total_start\": ${first_shadow_fail},"
  echo "  \"shadow_compare_failures_total_end\": ${last_shadow_fail},"
  echo "  \"shadow_compare_failures_delta\": ${shadow_fail_delta},"
  echo "  \"permission_checks_denied_delta\": ${denied_delta},"
  echo "  \"permission_checks_allowed_delta\": ${allowed_delta},"
  echo "  \"permission_checks_total_delta\": ${total_decisions_delta},"
  echo "  \"total_decisions_delta\": ${total_decisions_delta},"
  echo "  \"counter_reset_detected\": ${counter_reset_detected},"
  echo "  \"min_decision_delta\": ${MIN_DECISION_DELTA},"
  echo "  \"require_zero_mismatch\": ${REQUIRE_ZERO_MISMATCH},"
  echo "  \"save_samples\": ${SAVE_SAMPLES},"
  echo "  \"samples_dir\": \"${SAMPLES_DIR}\","
  echo "  \"gate_status\": \"${gate_status}\","
  echo "  \"gate_message\": \"${gate_message}\","
  echo "  \"samples_data\": ["
  for idx in "${!sample_json_lines[@]}"; do
    comma=","
    if [[ "$idx" -eq "$((${#sample_json_lines[@]} - 1))" ]]; then
      comma=""
    fi
    echo "    ${sample_json_lines[$idx]}${comma}"
  done
  echo "  ]"
  echo "}"
} > "$JSON_FILE"

{
  echo "# RBAC cutover baseline report"
  echo
  echo "- metrics_url: ${METRICS_URL}"
  echo "- samples: ${SAMPLES}"
  echo "- interval_sec: ${INTERVAL_SEC}"
  echo "- min_decision_delta: ${MIN_DECISION_DELTA}"
  echo "- require_zero_mismatch: ${REQUIRE_ZERO_MISMATCH}"
  echo "- require_zero_shadow_failures: ${REQUIRE_ZERO_SHADOW_FAILURES}"
  echo "- save_samples: ${SAVE_SAMPLES}"
  echo "- mismatch_total: ${first_mismatch} -> ${last_mismatch} (delta ${mismatch_delta})"
  echo "- shadow_compare_failures_total: ${first_shadow_fail} -> ${last_shadow_fail} (delta ${shadow_fail_delta})"
  echo "- permission_checks_denied delta: ${denied_delta}"
  echo "- permission_checks_allowed delta: ${allowed_delta}"
  echo "- permission_checks_total delta: ${total_decisions_delta}"
  echo "- counter_reset_detected: ${counter_reset_detected}"
  echo
  echo "## Gate"
  echo
  echo "- status: ${gate_status}"
  echo "- message: ${gate_message}"
  echo
  echo "## Artifacts"
  echo
  echo "- json: ${JSON_FILE}"
  echo "- report: ${REPORT_FILE}"
  if [[ "$SAVE_SAMPLES" == "true" ]]; then
    echo "- samples_dir: ${SAMPLES_DIR}"
  fi
} > "$REPORT_FILE"

echo "Done. Report: ${REPORT_FILE}"
echo "JSON: ${JSON_FILE}"

if [[ "$gate_status" == "fail" ]]; then
  echo "$gate_message" >&2
  exit 1
fi
