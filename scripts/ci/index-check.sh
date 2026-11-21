#!/usr/bin/env bash
set -euo pipefail

case "${1-}:${2-}" in
  cargo:*|git:diff) ;;
  *) echo 'usage: index-check.sh cargo <arguments...> | git diff <arguments...>' >&2; exit 64 ;;
esac

log=$(mktemp)
trap 'rm -f "$log"' EXIT
if CARGO_TERM_COLOR=never "$@" 2>&1 | tee "$log"; then
  exit 0
else
  statuses=("${PIPESTATUS[@]}")
fi
status=${statuses[0]}
if (( status == 0 )); then
  status=${statuses[1]}
fi

# Keep logs intact, and include a bounded excerpt in the GitHub check annotation.
# Annotations can be read without downloading the complete runner logs.
if [[ "$1" == cargo ]]; then
  detail=$(awk '
    /^test .* \.\.\. FAILED$/ && ++failed_tests <= 32 { print substr($0, 1, 240); next }
    /^test result: FAILED\./ { print substr($0, 1, 240); next }
    /^error(\[|:)/ || /^---- .* stdout ----$/ {
      remaining = ++blocks <= 3 ? 8 : 0
    }
    remaining > 0 { print substr($0, 1, 400); --remaining }
  ' "$log")
else
  # The only Git invocation is the diff of the generated contract digest file.
  detail=$(awk '/^[-+][[:blank:]]+"/ { print substr($0, 1, 400); if (++lines == 16) exit }' "$log")
fi
detail=${detail:-Check exited with status $status; see the job log.}
detail=${detail//%/%25}
detail=${detail//$'\r'/%0D}
detail=${detail//$'\n'/%0A}
printf '::error title=Index CI diagnostics::%s\n' "$detail"
exit "$status"
