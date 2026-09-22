#!/usr/bin/env bash
# Run the complete rustok-seo test suite and retain durable local evidence.
# This script is intentionally not registered in verify-all.sh or GitHub Actions.
set -uo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/../.." && pwd)"
OUTPUT_DIR="${RUSTOK_SEO_VERIFY_OUTPUT_DIR:-$REPO_ROOT/target/verification/seo-full-suite}"
LOG_FILE="$OUTPUT_DIR/cargo-test.log"
FAILURES_FILE="$OUTPUT_DIR/failed-tests.txt"
SUMMARY_FILE="$OUTPUT_DIR/summary.txt"

usage() {
    echo "Usage: bash scripts/verify/run-seo-full-suite.sh"
    echo
    echo "Runs: cargo test -p rustok-seo -- --nocapture"
    echo "Writes evidence under: target/verification/seo-full-suite/"
}

if [[ $# -ne 0 ]]; then
    usage >&2
    exit 2
fi

mkdir -p "$OUTPUT_DIR"
: > "$LOG_FILE"
: > "$FAILURES_FILE"
: > "$SUMMARY_FILE"

if ! command -v cargo >/dev/null 2>&1; then
    {
        echo "command: cargo test -p rustok-seo -- --nocapture"
        echo "exit_code: 127"
        echo "error: cargo is not available on PATH"
    } | tee "$SUMMARY_FILE" >&2
    exit 127
fi

echo "Running complete rustok-seo test suite..."
echo "Full log: $LOG_FILE"

set +e
(
    cd "$REPO_ROOT"
    CARGO_TERM_COLOR=never cargo test -p rustok-seo -- --nocapture
) 2>&1 | tee "$LOG_FILE"
status=${PIPESTATUS[0]}
set -e

# Cargo prints a canonical failure-name block between `failures:` and
# `test result:` for each failed test binary. Keep only the indented names.
awk '
    /^failures:$/ {
        in_failures = 1
        next
    }
    /^test result:/ {
        in_failures = 0
        next
    }
    in_failures && /^    [^ ]/ {
        line = $0
        sub(/^    /, "", line)
        if (line != "") print line
    }
' "$LOG_FILE" | sort -u > "$FAILURES_FILE"

{
    echo "command: cargo test -p rustok-seo -- --nocapture"
    echo "exit_code: $status"
    echo "log_file: $LOG_FILE"
    echo "failed_tests_file: $FAILURES_FILE"
    echo
    echo "test_results:"
    grep -E '^test result:' "$LOG_FILE" || echo "(no test result lines; inspect compiler/setup errors)"
    echo
    echo "failed_tests:"
    if [[ -s "$FAILURES_FILE" ]]; then
        cat "$FAILURES_FILE"
    else
        echo "(none extracted)"
    fi
    echo
    echo "compiler_or_setup_errors:"
    grep -E '^error(\[[^]]+\])?:|^error:' "$LOG_FILE" | head -20 || true
} > "$SUMMARY_FILE"

echo
echo "SEO full-suite evidence summary:"
cat "$SUMMARY_FILE"

exit "$status"
