#!/usr/bin/env bash
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
RUNNER_SRC="$REPO_ROOT/scripts/verify/run-control-plane-remediation-minimal.sh"

if [[ ! -x "$RUNNER_SRC" ]]; then
  echo "runner not executable: $RUNNER_SRC" >&2
  exit 1
fi

FIXTURE_ROOT="$(mktemp -d)"
trap 'rm -rf "$FIXTURE_ROOT"' EXIT

mkdir -p "$FIXTURE_ROOT/scripts/verify" "$FIXTURE_ROOT/scripts/ci" "$FIXTURE_ROOT/target" "$FIXTURE_ROOT/fakebin"
cp "$RUNNER_SRC" "$FIXTURE_ROOT/scripts/verify/run-control-plane-remediation-minimal.sh"
chmod +x "$FIXTURE_ROOT/scripts/verify/run-control-plane-remediation-minimal.sh"

# Minimal stub for terminal check command in runner.
cat > "$FIXTURE_ROOT/scripts/ci/check-dependabot-directories.py" <<'PY'
#!/usr/bin/env python3
print("dependabot stub: PASS")
PY
chmod +x "$FIXTURE_ROOT/scripts/ci/check-dependabot-directories.py"

# Fake tools to avoid heavyweight repo-wide builds and keep test deterministic.
cat > "$FIXTURE_ROOT/fakebin/cargo" <<'SH'
#!/usr/bin/env bash
set -euo pipefail
printf "fake cargo called: %s\n" "$*"
exit 0
SH
chmod +x "$FIXTURE_ROOT/fakebin/cargo"

cat > "$FIXTURE_ROOT/fakebin/python3" <<'SH'
#!/usr/bin/env bash
set -euo pipefail
exec /usr/bin/python3 "$@"
SH
chmod +x "$FIXTURE_ROOT/fakebin/python3"

cat > "$FIXTURE_ROOT/fakebin/flock" <<'SH'
#!/usr/bin/env bash
set -euo pipefail
exec /usr/bin/flock "$@"
SH
chmod +x "$FIXTURE_ROOT/fakebin/flock"

RUNNER="$FIXTURE_ROOT/scripts/verify/run-control-plane-remediation-minimal.sh"

bash -n "$RUNNER"

# lock guard: pre-acquire lock and assert runner exits with lock message
LOCK_FILE="$FIXTURE_ROOT/target/.control-plane-remediation-minimal.lock"
exec 8>"$LOCK_FILE"
flock -n 8

LOCK_OUTPUT="$(mktemp)"
if (cd "$FIXTURE_ROOT" && PATH="$FIXTURE_ROOT/fakebin:$PATH" RUSTOK_VERIFY_SKIP_FMT=1 /bin/bash "$RUNNER" >"$LOCK_OUTPUT" 2>&1); then
  echo "runner unexpectedly succeeded while lock is held" >&2
  cat "$LOCK_OUTPUT" >&2
  exit 1
fi
if ! rg -q "Another remediation verification run is already active" "$LOCK_OUTPUT"; then
  echo "runner did not report active lock" >&2
  cat "$LOCK_OUTPUT" >&2
  exit 1
fi

# release lock and ensure runner executes full command chain in skip-fmt mode
flock -u 8
STEP_OUTPUT="$(mktemp)"
(cd "$FIXTURE_ROOT" && PATH="$FIXTURE_ROOT/fakebin:$PATH" RUSTOK_VERIFY_SKIP_FMT=1 /bin/bash "$RUNNER" >"$STEP_OUTPUT" 2>&1)

for pattern in \
  "Skipping format check because RUSTOK_VERIFY_SKIP_FMT=1" \
  "==> migration tests" \
  "==> module lifecycle tests" \
  "==> platform composition tests" \
  "==> manifest validation" \
  "==> module contract validation" \
  "==> dependabot directory contract" \
  "Control-plane remediation minimal verification: PASS" \
  "--> migration tests: PASS"
do
  if ! rg -q -- "$pattern" "$STEP_OUTPUT"; then
    echo "expected pattern missing: $pattern" >&2
    cat "$STEP_OUTPUT" >&2
    exit 1
  fi
done

if ! rg -q -- "Control-plane remediation minimal verification: PASS \([0-9]{2}h:[0-9]{2}m:[0-9]{2}s\)" "$STEP_OUTPUT"; then
  echo "success scenario missing total duration suffix" >&2
  cat "$STEP_OUTPUT" >&2
  exit 1
fi

if ! rg -q -- "--> migration tests: PASS \([0-9]{2}h:[0-9]{2}m:[0-9]{2}s\)" "$STEP_OUTPUT"; then
  echo "success scenario missing per-step duration suffix" >&2
  cat "$STEP_OUTPUT" >&2
  exit 1
fi

# timeout mode: ensure step timeout wiring is active and surfaces timeout failure
cat > "$FIXTURE_ROOT/fakebin/cargo" <<'SH'
#!/usr/bin/env bash
set -euo pipefail
if [[ "$*" == *"test -p migration"* ]]; then
  sleep 2
fi
printf "fake cargo called (timeout phase): %s\n" "$*"
exit 0
SH
chmod +x "$FIXTURE_ROOT/fakebin/cargo"

TIMEOUT_OUTPUT="$(mktemp)"
if (cd "$FIXTURE_ROOT" && PATH="$FIXTURE_ROOT/fakebin:$PATH" RUSTOK_VERIFY_SKIP_FMT=1 RUSTOK_VERIFY_STEP_TIMEOUT=1s /bin/bash "$RUNNER" >"$TIMEOUT_OUTPUT" 2>&1); then
  echo "runner unexpectedly succeeded with strict timeout" >&2
  cat "$TIMEOUT_OUTPUT" >&2
  exit 1
fi
if ! rg -q "==> migration tests" "$TIMEOUT_OUTPUT"; then
  echo "timeout scenario did not reach migration step" >&2
  cat "$TIMEOUT_OUTPUT" >&2
  exit 1
fi

if rg -q "==> module lifecycle tests" "$TIMEOUT_OUTPUT"; then
  echo "timeout scenario unexpectedly progressed past migration step" >&2
  cat "$TIMEOUT_OUTPUT" >&2
  exit 1
fi

if ! rg -q "Failed step: migration tests" "$TIMEOUT_OUTPUT"; then
  echo "timeout scenario did not report failed step summary" >&2
  cat "$TIMEOUT_OUTPUT" >&2
  exit 1
fi

if ! rg -q "Failed command: cargo test -p migration" "$TIMEOUT_OUTPUT"; then
  echo "timeout scenario did not report failed command summary" >&2
  cat "$TIMEOUT_OUTPUT" >&2
  exit 1
fi

if ! rg -q "Control-plane remediation minimal verification: FAIL" "$TIMEOUT_OUTPUT"; then
  echo "timeout scenario did not report fail summary" >&2
  cat "$TIMEOUT_OUTPUT" >&2
  exit 1
fi

if ! rg -q "Exit code: [0-9]+" "$TIMEOUT_OUTPUT"; then
  echo "timeout scenario did not report exit code" >&2
  cat "$TIMEOUT_OUTPUT" >&2
  exit 1
fi

if ! rg -q "Elapsed: [0-9]{2}h:[0-9]{2}m:[0-9]{2}s" "$TIMEOUT_OUTPUT"; then
  echo "timeout scenario did not report elapsed duration" >&2
  cat "$TIMEOUT_OUTPUT" >&2
  exit 1
fi


# continue-on-fmt-fail mode: fmt fails, other steps still run, exit code 2
cat > "$FIXTURE_ROOT/fakebin/cargo" <<'SH'
#!/usr/bin/env bash
set -euo pipefail
if [[ "$*" == *"fmt --all -- --check"* ]]; then
  echo "fake fmt drift" >&2
  exit 1
fi
printf "fake cargo called (continue-on-fmt): %s\n" "$*"
exit 0
SH
chmod +x "$FIXTURE_ROOT/fakebin/cargo"

CONTINUE_FMT_OUTPUT="$(mktemp)"
set +e
(cd "$FIXTURE_ROOT" && PATH="$FIXTURE_ROOT/fakebin:$PATH" RUSTOK_VERIFY_CONTINUE_ON_FMT_FAIL=1 /bin/bash "$RUNNER" >"$CONTINUE_FMT_OUTPUT" 2>&1)
continue_exit=$?
set -e
if [[ "$continue_exit" -ne 2 ]]; then
  echo "continue-on-fmt scenario must exit with code 2, got $continue_exit" >&2
  cat "$CONTINUE_FMT_OUTPUT" >&2
  exit 1
fi
for pattern in \
  "Format failure continuation enabled" \
  "WARNING: format check failed; continuing" \
  "==> migration tests" \
  "==> dependabot directory contract" \
  "PARTIAL PASS"
do
  if ! rg -q -- "$pattern" "$CONTINUE_FMT_OUTPUT"; then
    echo "continue-on-fmt scenario missing pattern: $pattern" >&2
    cat "$CONTINUE_FMT_OUTPUT" >&2
    exit 1
  fi
done

echo "control_plane_remediation_minimal_runner_test.sh: PASS"
