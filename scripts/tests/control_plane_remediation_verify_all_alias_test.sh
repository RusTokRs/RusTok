#!/usr/bin/env bash
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
VERIFY_ALL_SRC="$REPO_ROOT/scripts/verify/verify-all.sh"
RUNNER_SRC="$REPO_ROOT/scripts/verify/run-control-plane-remediation-minimal.sh"

FIXTURE_ROOT="$(mktemp -d)"
trap 'rm -rf "$FIXTURE_ROOT"' EXIT

mkdir -p "$FIXTURE_ROOT/scripts/verify" "$FIXTURE_ROOT/scripts/ci" "$FIXTURE_ROOT/target" "$FIXTURE_ROOT/fakebin"
cp "$VERIFY_ALL_SRC" "$FIXTURE_ROOT/scripts/verify/verify-all.sh"
cp "$RUNNER_SRC" "$FIXTURE_ROOT/scripts/verify/run-control-plane-remediation-minimal.sh"
chmod +x "$FIXTURE_ROOT/scripts/verify/verify-all.sh" "$FIXTURE_ROOT/scripts/verify/run-control-plane-remediation-minimal.sh"

# Stub all other verify suites so alias filtering still runs inside isolated fixture.
for stub in \
  verify-tenant-isolation.sh verify-unsafe-code.sh verify-rbac-coverage.sh verify-api-quality.sh \
  verify-events.sh verify-code-quality.sh verify-security.sh verify-architecture.sh \
  verify-deployment-profiles.sh verify-anti-bypass.sh
 do
  cat > "$FIXTURE_ROOT/scripts/verify/$stub" <<'SH'
#!/usr/bin/env bash
exit 0
SH
  chmod +x "$FIXTURE_ROOT/scripts/verify/$stub"
done
for stub in \
  verify-storefront-module-routes.mjs verify-i18n-contract.mjs verify-ui-i18n-parity.mjs \
  verify-flex-multilingual-contract.mjs verify-module-lifecycle-bypass-usage.mjs \
  verify-page-builder-contract-parity.mjs verify-page-builder-fallback-profiles.mjs \
  verify-page-builder-toggle-profiles-consistency.mjs verify-page-builder-fba-baseline.mjs \
  verify-page-builder-consumer-readiness.mjs
 do
  cat > "$FIXTURE_ROOT/scripts/verify/$stub" <<'JS'
#!/usr/bin/env node
process.exit(0)
JS
  chmod +x "$FIXTURE_ROOT/scripts/verify/$stub"
done

cat > "$FIXTURE_ROOT/scripts/ci/check-dependabot-directories.py" <<'PY'
#!/usr/bin/env python3
print("dependabot stub: PASS")
PY
chmod +x "$FIXTURE_ROOT/scripts/ci/check-dependabot-directories.py"

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

OUTPUT_FILE="$(mktemp)"
(cd "$FIXTURE_ROOT" && PATH="$FIXTURE_ROOT/fakebin:$PATH" RUSTOK_VERIFY_SKIP_FMT=1 ./scripts/verify/verify-all.sh -v control-plane-remediation-minimal >"$OUTPUT_FILE" 2>&1)

if ! rg -q "Control Plane Remediation Minimal" "$OUTPUT_FILE"; then
  echo "verify-all did not pick control-plane-remediation-minimal alias" >&2
  cat "$OUTPUT_FILE" >&2
  exit 1
fi

if ! rg -q "==> migration tests" "$OUTPUT_FILE"; then
  echo "runner did not start through verify-all alias" >&2
  cat "$OUTPUT_FILE" >&2
  exit 1
fi

if ! rg -q "All verification suites passed!" "$OUTPUT_FILE"; then
  echo "verify-all did not report success for alias run" >&2
  cat "$OUTPUT_FILE" >&2
  exit 1
fi

echo "control_plane_remediation_verify_all_alias_test.sh: PASS"
