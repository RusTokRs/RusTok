#!/usr/bin/env bash
set -euo pipefail

METRICS_URL="${METRICS_URL:-http://localhost:5150/metrics}"
OUT_FILE=""
BASELINE_FILE=""
SNAPSHOT_OUT=""
WINDOW_LABEL="weekly"

usage() {
  cat <<USAGE
Usage: $0 [options]

Build a module path adoption report from Prometheus metrics.

Options:
  --metrics-url <url>      Metrics endpoint (default: ${METRICS_URL})
  --out <file>             Output markdown report file (default: stdout)
  --baseline <file>        Previous snapshot JSON to detect new bypass points
  --snapshot-out <file>    Write current snapshot JSON for next period diff
  --window <label>         Report window label (default: weekly)
  -h, --help               Show this help

Metric source:
  rustok_module_entrypoint_calls_total{module,entry_point,path}

Path semantics:
  library      - shared rustok module/library API path
  core_runtime - platform kernel path (apps/server + core crates)
  bypass       - direct/legacy path outside shared contracts
USAGE
}

while [[ $# -gt 0 ]]; do
  case "$1" in
    --metrics-url)
      METRICS_URL="$2"; shift 2 ;;
    --out)
      OUT_FILE="$2"; shift 2 ;;
    --baseline)
      BASELINE_FILE="$2"; shift 2 ;;
    --snapshot-out)
      SNAPSHOT_OUT="$2"; shift 2 ;;
    --window)
      WINDOW_LABEL="$2"; shift 2 ;;
    -h|--help)
      usage; exit 0 ;;
    *)
      echo "Unknown option: $1" >&2
      usage
      exit 1 ;;
  esac
done

TMP_METRICS="$(mktemp)"
trap 'rm -f "$TMP_METRICS"' EXIT
curl -fsSL "$METRICS_URL" -o "$TMP_METRICS"

PYTHON_OUTPUT=$(python3 - "$TMP_METRICS" "$BASELINE_FILE" "$WINDOW_LABEL" <<'PY'
import datetime as dt
import json
import re
import sys
from collections import defaultdict
from pathlib import Path

metrics_path = Path(sys.argv[1])
baseline_path = Path(sys.argv[2]) if sys.argv[2] else None
window = sys.argv[3]

line_re = re.compile(r'^rustok_module_entrypoint_calls_total\{([^}]*)\}\s+([0-9]+(?:\.[0-9]+)?)$')
label_re = re.compile(r'(\w+)="([^"]*)"')

rows = []
with metrics_path.open() as f:
    for raw in f:
        raw = raw.strip()
        m = line_re.match(raw)
        if not m:
            continue
        labels_raw, value_raw = m.groups()
        labels = {k: v for k, v in label_re.findall(labels_raw)}
        rows.append((labels.get("module", ""), labels.get("entry_point", ""), labels.get("path", ""), float(value_raw)))

agg = defaultdict(lambda: {"library": 0.0, "core_runtime": 0.0, "bypass": 0.0})
for module, entry, path, value in rows:
    if not module or not entry:
        continue
    if path not in ("library", "core_runtime", "bypass"):
        continue
    agg[(module, entry)][path] += value

module_totals = defaultdict(lambda: {"library": 0.0, "core_runtime": 0.0, "bypass": 0.0})
for (module, _entry), vals in agg.items():
    module_totals[module]["library"] += vals["library"]
    module_totals[module]["core_runtime"] += vals["core_runtime"]
    module_totals[module]["bypass"] += vals["bypass"]

snapshot = {
    "generated_at_utc": dt.datetime.utcnow().replace(microsecond=0).isoformat() + "Z",
    "metric": "rustok_module_entrypoint_calls_total",
    "entries": [
        {
            "module": module,
            "entry_point": entry,
            "library": vals["library"],
            "core_runtime": vals["core_runtime"],
            "bypass": vals["bypass"],
        }
        for (module, entry), vals in sorted(agg.items())
    ]
}

baseline = {"entries": []}
if baseline_path and baseline_path.exists():
    baseline = json.loads(baseline_path.read_text())

baseline_bypass = {
    (item.get("module"), item.get("entry_point"))
    for item in baseline.get("entries", [])
    if float(item.get("bypass", 0)) > 0
}
current_bypass = {
    (item["module"], item["entry_point"])
    for item in snapshot["entries"]
    if item["bypass"] > 0
}
new_bypass = sorted(current_bypass - baseline_bypass)

lines = []
lines.append("# Module path adoption report")
lines.append("")
lines.append(f"- Window: {window}")
lines.append(f"- Generated (UTC): {snapshot['generated_at_utc']}")
lines.append("")
lines.append("## % scenarios through rustok libraries")
lines.append("")
lines.append("| Module | Library calls | Core runtime calls | Bypass calls | Library adoption %* |")
lines.append("|---|---:|---:|---:|---:|")
for module in sorted(module_totals.keys()):
    library = module_totals[module]["library"]
    core_runtime = module_totals[module]["core_runtime"]
    bypass = module_totals[module]["bypass"]
    eligible_total = library + bypass
    adoption = (library / eligible_total * 100.0) if eligible_total > 0 else 0.0
    lines.append(f"| {module} | {library:.0f} | {core_runtime:.0f} | {bypass:.0f} | {adoption:.2f}% |")
if not module_totals:
    lines.append("| n/a | 0 | 0 | 0 | 0.00% |")

lines.append("")
lines.append("\* adoption% uses only library vs bypass calls; core_runtime is tracked separately and is not treated as bypass.")

lines.append("")
lines.append("## Library vs bypass ratio by entry point")
lines.append("")
lines.append("| Module | Entry point | Library | Core runtime | Bypass | Ratio (library:bypass) |")
lines.append("|---|---|---:|---:|---:|---:|")
for (module, entry), vals in sorted(agg.items()):
    lib = vals["library"]
    core_runtime = vals["core_runtime"]
    byp = vals["bypass"]
    ratio = "∞" if byp == 0 and lib > 0 else ("0" if lib == 0 and byp > 0 else ("0" if lib == 0 and byp == 0 else f"{lib/byp:.2f}"))
    lines.append(f"| {module} | {entry} | {lib:.0f} | {core_runtime:.0f} | {byp:.0f} | {ratio} |")
if not agg:
    lines.append("| n/a | n/a | 0 | 0 | 0 | 0 |")

lines.append("")
lines.append("## New bypass points in period")
lines.append("")
if new_bypass:
    for module, entry in new_bypass:
        lines.append(f"- `{module}:{entry}`")
else:
    lines.append("- none")

print("\n".join(lines))
print("\n__SNAPSHOT_JSON__")
print(json.dumps(snapshot, ensure_ascii=False))
PY
)

REPORT_CONTENT="${PYTHON_OUTPUT%$'\n__SNAPSHOT_JSON__'*}"
SNAPSHOT_JSON="${PYTHON_OUTPUT##*$'\n__SNAPSHOT_JSON__'$'\n'}"

if [[ -n "$OUT_FILE" ]]; then
  mkdir -p "$(dirname "$OUT_FILE")"
  printf '%s\n' "$REPORT_CONTENT" > "$OUT_FILE"
  echo "Report written to $OUT_FILE"
else
  printf '%s\n' "$REPORT_CONTENT"
fi

if [[ -n "$SNAPSHOT_OUT" ]]; then
  mkdir -p "$(dirname "$SNAPSHOT_OUT")"
  printf '%s\n' "$SNAPSHOT_JSON" > "$SNAPSHOT_OUT"
  echo "Snapshot written to $SNAPSHOT_OUT"
fi
