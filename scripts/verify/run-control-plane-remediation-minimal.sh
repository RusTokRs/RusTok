#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
LOCK_FILE="${ROOT_DIR}/target/.control-plane-remediation-minimal.lock"

mkdir -p "${ROOT_DIR}/target"
exec 9>"${LOCK_FILE}"
if ! flock -n 9; then
  echo "Another remediation verification run is already active (lock: ${LOCK_FILE})." >&2
  echo "Wait for the active run to finish and retry." >&2
  exit 1
fi

cd "${ROOT_DIR}"

run_step() {
  local title="$1"
  shift
  printf "\n==> %s\n" "${title}"
  printf "$ %s\n" "$*"
  "$@"
}

if [[ "${RUSTOK_VERIFY_SKIP_FMT:-0}" == "1" ]]; then
  echo "Skipping format check because RUSTOK_VERIFY_SKIP_FMT=1"
else
  run_step "format check" cargo fmt --all -- --check
fi

run_step "migration tests" cargo test -p migration
run_step "module lifecycle tests" cargo test -p rustok-server module_lifecycle
run_step "platform composition tests" cargo test -p rustok-server platform_composition
run_step "manifest validation" cargo xtask validate-manifest
run_step "module contract validation" cargo xtask module validate
run_step "dependabot directory contract" python3 scripts/ci/check-dependabot-directories.py

printf "\nControl-plane remediation minimal verification: PASS\n"
