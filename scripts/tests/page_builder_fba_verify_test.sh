#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/../.." && pwd)"
VERIFY_DIR="$REPO_ROOT/scripts/verify"

test_baseline_passes_on_current_repo_state() {
  (cd "$REPO_ROOT" && node "$VERIFY_DIR/verify-page-builder-fba-baseline.mjs")
}

test_verify_all_alias_runs_page_builder_baseline() {
  (cd "$REPO_ROOT" && "$VERIFY_DIR/verify-all.sh" page-builder-fba-baseline >/tmp/page_builder_verify_all.out)
  grep -q "PASS" /tmp/page_builder_verify_all.out
}

test_baseline_passes_on_current_repo_state
test_verify_all_alias_runs_page_builder_baseline

echo "page_builder_fba_verify_test.sh: PASS"
