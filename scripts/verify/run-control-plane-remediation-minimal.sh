#!/usr/bin/env bash
set -Eeuo pipefail

SECONDS=0
CURRENT_STEP="bootstrap"
CURRENT_COMMAND="n/a"
FMT_FAILED=0

format_duration() {
  local total="$1"
  local h=$((total / 3600))
  local m=$(((total % 3600) / 60))
  local s=$((total % 60))
  printf "%02dh:%02dm:%02ds" "$h" "$m" "$s"
}

on_error() {
  local exit_code="$?"
  echo
  echo "Control-plane remediation minimal verification: FAIL" >&2
  echo "Failed step: ${CURRENT_STEP}" >&2
  echo "Failed command: ${CURRENT_COMMAND}" >&2
  echo "Exit code: ${exit_code}" >&2
  echo "Elapsed: $(format_duration "${SECONDS}")" >&2
  exit "${exit_code}"
}

trap on_error ERR

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
LOCK_FILE="${ROOT_DIR}/target/.control-plane-remediation-minimal.lock"

mkdir -p "${ROOT_DIR}/target"

if ! command -v flock >/dev/null 2>&1; then
  echo "Required tool missing: flock" >&2
  exit 1
fi

if [[ -n "${RUSTOK_VERIFY_STEP_TIMEOUT:-}" ]] && ! command -v timeout >/dev/null 2>&1; then
  echo "RUSTOK_VERIFY_STEP_TIMEOUT is set but required tool is missing: timeout" >&2
  exit 1
fi

exec 9>"${LOCK_FILE}"
if ! flock -n 9; then
  echo "Another remediation verification run is already active (lock: ${LOCK_FILE})." >&2
  echo "Wait for the active run to finish and retry." >&2
  exit 1
fi

cd "${ROOT_DIR}"

step_timeout() {
  if [[ -n "${RUSTOK_VERIFY_STEP_TIMEOUT:-}" ]]; then
    timeout "${RUSTOK_VERIFY_STEP_TIMEOUT}" "$@"
  else
    "$@"
  fi
}

run_step() {
  local title="$1"
  shift
  local step_start="${SECONDS}"
  CURRENT_STEP="${title}"
  printf "\n==> %s\n" "${title}"
  printf "$ %s\n" "$*"
  CURRENT_COMMAND="$*"
  if ! step_timeout "$@"; then
    return 1
  fi
  local step_elapsed=$((SECONDS - step_start))
  printf -- "--> %s: PASS (%s)\n" "${title}" "$(format_duration "${step_elapsed}")"
  CURRENT_COMMAND="n/a"
}

if [[ "${RUSTOK_VERIFY_SKIP_FMT:-0}" == "1" ]]; then
  echo "Skipping format check because RUSTOK_VERIFY_SKIP_FMT=1"
else
  if [[ "${RUSTOK_VERIFY_CONTINUE_ON_FMT_FAIL:-0}" == "1" ]]; then
    echo "Format failure continuation enabled: RUSTOK_VERIFY_CONTINUE_ON_FMT_FAIL=1"
    if ! run_step "format check" cargo fmt --all -- --check; then
      FMT_FAILED=1
      echo "WARNING: format check failed; continuing with remaining verification steps."
    fi
  else
    run_step "format check" cargo fmt --all -- --check
  fi
fi

run_step "migration tests" cargo test -p migration
run_step "module lifecycle tests" cargo test -p rustok-server module_lifecycle
run_step "platform composition tests" cargo test -p rustok-server platform_composition
run_step "manifest validation" cargo xtask validate-manifest
run_step "module contract validation" cargo xtask module validate
run_step "dependabot directory contract" python3 scripts/ci/check-dependabot-directories.py

if [[ "${FMT_FAILED}" == "1" ]]; then
  echo
  echo "Control-plane remediation minimal verification: PARTIAL PASS (non-blocking format failure) ($(format_duration "${SECONDS}"))"
  exit 2
fi

printf "\nControl-plane remediation minimal verification: PASS (%s)\n" "$(format_duration "${SECONDS}")"
