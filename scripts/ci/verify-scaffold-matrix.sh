#!/usr/bin/env bash
set -euo pipefail

description="Generated fixture for native module scaffold matrix verification."

create_module() {
  local slug="$1"
  local title="$2"
  shift 2

  cargo xtask scaffold "$slug" \
    --title "$title" \
    --description "$description" \
    --category collaboration \
    --no-db \
    "$@"
}

echo "== Scaffold renderer golden tests =="
cargo test -p xtask

echo "== Generate the four supported native scaffold modes =="
create_module scaffold_matrix_headless "Scaffold Matrix Headless" --no-ui
create_module scaffold_matrix_admin "Scaffold Matrix Admin" --ui admin
create_module scaffold_matrix_storefront "Scaffold Matrix Storefront" --ui storefront
create_module scaffold_matrix_dual "Scaffold Matrix Dual" --ui dual

echo "== Verify unsupported persistence fails before writing =="
if cargo xtask scaffold scaffold_matrix_with_db \
  --title "Scaffold Matrix With DB" \
  --description "$description" \
  --category collaboration \
  --with-db; then
  echo "ERROR: --with-db unexpectedly succeeded" >&2
  exit 1
fi

if [[ -e crates/modules/rustok-scaffold-matrix-with-db ]]; then
  echo "ERROR: rejected --with-db request created a crate directory" >&2
  exit 1
fi
if grep -Eq '^[[:space:]]*scaffold_matrix_with_db[[:space:]]*=' modules.toml; then
  echo "ERROR: rejected --with-db request modified modules.toml" >&2
  exit 1
fi
if grep -Fq 'rustok-scaffold-matrix-with-db = ' Cargo.toml; then
  echo "ERROR: rejected --with-db request modified workspace dependencies" >&2
  exit 1
fi

echo "== Compile and run tests for every generated backend and UI crate =="
cargo test --all-targets \
  -p rustok-scaffold-matrix-headless \
  -p rustok-scaffold-matrix-admin \
  -p rustok-scaffold-matrix-admin-admin \
  -p rustok-scaffold-matrix-storefront \
  -p rustok-scaffold-matrix-storefront-storefront \
  -p rustok-scaffold-matrix-dual \
  -p rustok-scaffold-matrix-dual-admin \
  -p rustok-scaffold-matrix-dual-storefront

echo "== Compile both UI surfaces under each Leptos build profile =="
for package in \
  rustok-scaffold-matrix-dual-admin \
  rustok-scaffold-matrix-dual-storefront; do
  cargo check -p "$package" --features csr
  cargo check -p "$package" --features hydrate
  cargo check -p "$package" --features ssr
done

git diff --check
echo "SCAFFOLD MATRIX PASSED"
