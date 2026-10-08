#!/usr/bin/env bash
#
# Focused checks for the Fly page builder.
#
# Why this exists
# ---------------
# `scripts/verify` holds over 1300 scripts and the workspace holds dozens of crates. Checking a
# Fly change by running "the tests" means wading through output that has nothing to do with Fly.
# This runs exactly the checks the Fly CI workflow runs, and nothing else.
#
# It also does not stop at the first failure. One run reports every problem, so a fix cycle is
# one pass instead of one pass per defect.
#
# Usage
#   scripts/fly-check.sh            # everything (gates, tests, features, lint, format)
#   scripts/fly-check.sh gates      # source guards only — node, no cargo, a few seconds
#   scripts/fly-check.sh test       # crate tests only
#   scripts/fly-check.sh features   # non-default feature combinations, incl. wasm32
#   scripts/fly-check.sh lint       # clippy (-D warnings) and rustfmt
#   scripts/fly-check.sh consumers  # the Page Builder / Pages crates that depend on Fly
#
# Any failing step leaves the whole run non-zero.

set -uo pipefail

cd "$(dirname "${BASH_SOURCE[0]}")/.." || exit 1

MODE="${1:-all}"

if [[ -t 1 ]]; then
  RED=$'\e[31m'; GREEN=$'\e[32m'; YELLOW=$'\e[33m'; BOLD=$'\e[1m'; DIM=$'\e[2m'; OFF=$'\e[0m'
else
  RED=''; GREEN=''; YELLOW=''; BOLD=''; DIM=''; OFF=''
fi

PASSED=(); FAILED=(); SKIPPED=()
LOG_DIR="$(mktemp -d)"
trap 'rm -rf "$LOG_DIR"' EXIT

have() { command -v "$1" >/dev/null 2>&1; }

# Run one labelled step, capturing output so a passing step stays silent.
step() {
  local label="$1"; shift
  local log="$LOG_DIR/$(echo "$label" | tr -c 'a-zA-Z0-9' '_').log"
  printf '%s  %-52s%s' "$DIM" "$label" "$OFF"
  if "$@" >"$log" 2>&1; then
    printf '%s ok %s\n' "$GREEN" "$OFF"
    PASSED+=("$label")
  else
    printf '%s FAIL %s\n' "$RED" "$OFF"
    FAILED+=("$label|$log")
  fi
}

skip() {
  printf '%s  %-52s%s' "$DIM" "$1" "$OFF"
  printf '%s skip (%s)%s\n' "$YELLOW" "$2" "$OFF"
  SKIPPED+=("$1")
}

heading() { printf '\n%s%s%s\n' "$BOLD" "$1" "$OFF"; }

# Syntax-check the browser asset.
#
# `node --check path/to/fly-browser.js` is a no-op here and silently exits 0 no matter what the
# file contains. The file is `.js` but uses `export`, so Node detects it as an ES module, and in
# that path `--check` reports nothing. Feeding it on stdin with an explicit `--input-type=module`
# is the spelling that actually parses it. Verified in both directions: a deliberate syntax error
# exits 1, the real file exits 0.
check_browser_asset() {
  node --input-type=module --check < crates/ui/fly/browser/assets/fly-browser.js
}

# --------------------------------------------------------------------------------------------
# Source guards. Pure node, no toolchain, seconds. Run these first: they are the cheapest signal
# and they catch the drift that compiling cannot.
# --------------------------------------------------------------------------------------------
run_gates() {
  heading 'Source guards'
  if ! have node; then
    skip 'all source guards' 'node not installed'
    return
  fi
  local gate
  for gate in \
    verify-fly-admin-runtime \
    verify-fly-admin-browser-runtime \
    verify-fly-command-transactions \
    verify-fly-snapshots \
    verify-fly-interaction-capabilities \
    verify-fly-multilingual \
    verify-fly-gates-are-wired \
    verify-fly-standalone-workspace \
    verify-fly-dependency-boundaries \
    verify-pages-current-only \
    verify-fly-ssr-first \
    verify-fly-ui-contributions \
    verify-fly-landing-readiness \
    verify-fly-internal-links \
    verify-fly-actions-forms \
    verify-fly-ssr-assets \
    verify-fly-ui-capability-policy \
    verify-fly-capability-denial-response
  do
    step "$gate" node "scripts/verify/$gate.mjs"
  done
  step 'fly-browser.js parses' check_browser_asset
  # Crate-local Page Builder gate: the scenario-baseline journal contract lives with the
  # module it guards, so it is invoked by path rather than from scripts/verify.
  step 'pages-scenario-baseline-promotion' node crates/modules/rustok-pages/scripts/verify/verify-pages-builder-scenario-promotion.mjs
}

# --------------------------------------------------------------------------------------------
# Crate tests.
# --------------------------------------------------------------------------------------------
run_tests() {
  heading 'Fly crate tests'
  step 'cargo test -p fly'                 cargo test -p fly --all-targets
  step 'cargo test -p fly-browser'         cargo test -p fly-browser --all-targets
  step 'cargo test -p fly-ui -p fly-web'   cargo test -p fly-ui -p fly-web --all-targets
  step 'cargo check adapters'              cargo check -p fly-leptos -p fly-dioxus --all-targets
}

# --------------------------------------------------------------------------------------------
# Feature combinations. The default build exercises none of these, which is how a configuration
# rots without anyone noticing.
# --------------------------------------------------------------------------------------------
run_features() {
  heading 'Feature combinations'
  step 'fly without platform-i18n (check)' cargo check -p fly --no-default-features --all-targets
  step 'fly without platform-i18n (test)'  cargo test -p fly --no-default-features --lib
  step 'fly-dioxus --features desktop'     cargo check -p fly-dioxus --features desktop
  step 'fly-leptos --no-default-features'  cargo check -p fly-leptos --no-default-features

  if rustup target list --installed 2>/dev/null | grep -q wasm32-unknown-unknown; then
    step 'wasm32 fly-web'    cargo check --target wasm32-unknown-unknown -p fly-web --features wasm-client
    step 'wasm32 fly-leptos' cargo check --target wasm32-unknown-unknown -p fly-leptos --features wasm-client
    step 'wasm32 fly-dioxus' cargo check --target wasm32-unknown-unknown -p fly-dioxus --features web
  else
    skip 'wasm32 targets' 'rustup target add wasm32-unknown-unknown'
  fi
}

# --------------------------------------------------------------------------------------------
# Lint and format. CI lints with `-D warnings`, so a warning here is a red build there.
# --------------------------------------------------------------------------------------------
run_lint() {
  heading 'Lint and format'
  step 'clippy (fly crates)' cargo clippy \
    -p fly -p fly-ui -p fly-web -p fly-leptos -p fly-dioxus \
    --all-targets -- -D warnings
  step 'clippy (integrations)' cargo clippy \
    -p fly-browser -p rustok-page-builder-admin -p rustok-pages \
    -p rustok-pages-admin -p rustok-pages-storefront \
    --lib -- -D warnings
  step 'rustfmt' cargo fmt \
    -p fly -p fly-ui -p fly-web -p fly-browser -p fly-leptos -p fly-dioxus \
    -p rustok-page-builder-admin -p rustok-pages -p rustok-pages-admin \
    -p rustok-pages-storefront -p rustok-admin \
    -- --check
}

# --------------------------------------------------------------------------------------------
# Downstream crates. Fly's consumers are where a signature change actually bites.
# --------------------------------------------------------------------------------------------
run_consumers() {
  heading 'Downstream consumers'
  step 'rustok-page-builder-admin'   cargo test -p rustok-page-builder-admin --lib
  step 'rustok-pages'                cargo test -p rustok-pages --lib
  step 'rustok-pages-admin'          cargo test -p rustok-pages-admin --lib
  step 'rustok-pages-storefront'     cargo check -p rustok-pages-storefront --lib
  step 'rustok-page-builder-storefront' cargo check -p rustok-page-builder-storefront --lib
  step 'rustok-admin (ssr)'          cargo check -p rustok-admin --bin rustok-admin --no-default-features --features ssr
}

needs_cargo() {
  if have cargo; then
    return 0
  fi
  skip "$1" 'cargo not installed'
  return 1
}

case "$MODE" in
  gates)     run_gates ;;
  test)      needs_cargo 'crate tests' && run_tests ;;
  features)  needs_cargo 'feature combinations' && run_features ;;
  lint)      needs_cargo 'lint and format' && run_lint ;;
  consumers) needs_cargo 'downstream consumers' && run_consumers ;;
  all)
    run_gates
    if needs_cargo 'everything requiring a toolchain'; then
      run_tests
      run_features
      run_consumers
      run_lint
    fi
    ;;
  *)
    printf 'unknown mode: %s\n' "$MODE" >&2
    printf 'use one of: all gates test features lint consumers\n' >&2
    exit 2
    ;;
esac

# --------------------------------------------------------------------------------------------
# Summary. Output from failing steps is printed here, together, so one run is one fix cycle.
# --------------------------------------------------------------------------------------------
printf '\n%s%d passed, %d failed, %d skipped%s\n' \
  "$BOLD" "${#PASSED[@]}" "${#FAILED[@]}" "${#SKIPPED[@]}" "$OFF"

if ((${#FAILED[@]} == 0)); then
  printf '%sFly checks are clean.%s\n' "$GREEN" "$OFF"
  exit 0
fi

for entry in "${FAILED[@]}"; do
  label="${entry%%|*}"
  log="${entry#*|}"
  printf '\n%s%s── %s %s\n' "$BOLD" "$RED" "$label" "$OFF"
  # Enough context to act on, not so much that the summary becomes the noise it replaces.
  tail -n 40 "$log" | sed 's/^/    /'
done

printf '\n%sRe-run a single area with: scripts/fly-check.sh {gates|test|features|lint|consumers}%s\n' \
  "$DIM" "$OFF"
exit 1
