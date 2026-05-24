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

RUNNER="$FIXTURE_ROOT/scripts/verify/run-control-plane-remediation-minimal.sh"

bash -n "$RUNNER"

# lock guard: pre-acquire lock and assert runner exits with lock message
LOCK_FILE="$FIXTURE_ROOT/target/.control-plane-remediation-minimal.lock"
exec 8>"$LOCK_FILE"
flock -n 8

LOCK_OUTPUT="$(mktemp)"
if (cd "$FIXTURE_ROOT" && PATH="$FIXTURE_ROOT/fakebin:$PATH" RUSTOK_VERIFY_SKIP_FMT=1 "$RUNNER" >"$LOCK_OUTPUT" 2>&1); then
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
(cd "$FIXTURE_ROOT" && PATH="$FIXTURE_ROOT/fakebin:$PATH" RUSTOK_VERIFY_SKIP_FMT=1 "$RUNNER" >"$STEP_OUTPUT" 2>&1)

for pattern in \
  "Skipping format check because RUSTOK_VERIFY_SKIP_FMT=1" \
  "==> migration tests" \
  "==> module lifecycle tests" \
  "==> platform composition tests" \
  "==> manifest validation" \
  "==> module contract validation" \
  "==> dependabot directory contract" \
  "Control-plane remediation minimal verification: PASS"
do
  if ! rg -q "$pattern" "$STEP_OUTPUT"; then
    echo "expected pattern missing: $pattern" >&2
    cat "$STEP_OUTPUT" >&2
    exit 1
  fi
done

echo "control_plane_remediation_minimal_runner_test.sh: PASS"
