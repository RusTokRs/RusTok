#!/usr/bin/env bash
set -euo pipefail

lcov_file="${1:?usage: check-coverage.sh <lcov-file> <minimum-percent>}"
if [[ $# -ge 2 ]]; then
  minimum="$2"
else
  # shellcheck source=/dev/null
  source "$(dirname "$0")/coverage-threshold.env"
  minimum="${RUSTOK_MIN_COVERAGE_PERCENT:?coverage threshold is not set}"
fi

if [[ ! -f "$lcov_file" ]]; then
  echo "coverage file not found: $lcov_file" >&2
  exit 1
fi

read -r found hit < <(
  awk '
    /^LF:/ { found += substr($0, 4) }
    /^LH:/ { hit += substr($0, 4) }
    END { printf "%d %d\n", found, hit }
  ' "$lcov_file"
)

if [[ "$found" -eq 0 ]]; then
  echo "coverage file contains no instrumented lines" >&2
  exit 1
fi

coverage="$(awk -v hit="$hit" -v found="$found" 'BEGIN { printf "%.2f", (hit / found) * 100 }')"
awk -v coverage="$coverage" -v minimum="$minimum" 'BEGIN { exit !(coverage + 0 >= minimum + 0) }' || {
  echo "coverage ${coverage}% is below required ${minimum}%" >&2
  exit 1
}

echo "coverage ${coverage}% meets required ${minimum}%"
