#!/usr/bin/env bash
set -euo pipefail

usage() {
  cat <<'USAGE'
Usage: build-pages-inline-edit-server.sh [--tool-root DIR] [--target-dir DIR] [--profile debug|release] [--skip-tool-install]
USAGE
}

repo_root=$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)
tool_root="$repo_root/.tools/wasm-bindgen-cli"
target_dir="${CARGO_TARGET_DIR:-$repo_root/target}"
profile="release"
skip_tool_install=0

while [[ $# -gt 0 ]]; do
  case "$1" in
    --tool-root)
      [[ $# -ge 2 ]] || { usage >&2; exit 2; }
      tool_root=$2
      shift 2
      ;;
    --target-dir)
      [[ $# -ge 2 ]] || { usage >&2; exit 2; }
      target_dir=$2
      shift 2
      ;;
    --profile)
      [[ $# -ge 2 ]] || { usage >&2; exit 2; }
      profile=$2
      shift 2
      ;;
    --skip-tool-install)
      skip_tool_install=1
      shift
      ;;
    --help|-h)
      usage
      exit 0
      ;;
    *)
      echo "unknown argument: $1" >&2
      usage >&2
      exit 2
      ;;
  esac
done

[[ "$profile" == "debug" || "$profile" == "release" ]] || {
  echo "--profile must be debug or release" >&2
  exit 2
}

tool_root=$(mkdir -p "$tool_root" && cd "$tool_root" && pwd)
target_dir=$(mkdir -p "$target_dir" && cd "$target_dir" && pwd)
client_builder="$repo_root/apps/storefront/scripts/build-pages-inline-edit-client.mjs"
wasm_bindgen_version=$(node "$client_builder" --print-wasm-bindgen-version)
[[ "$wasm_bindgen_version" =~ ^0\.[0-9]+\.[0-9]+$ ]] || {
  echo "Cargo.lock returned an invalid wasm-bindgen version: $wasm_bindgen_version" >&2
  exit 1
}

if [[ $skip_tool_install -eq 0 ]]; then
  CARGO_TARGET_DIR="$tool_root/target" \
    cargo install wasm-bindgen-cli \
      --version "=$wasm_bindgen_version" \
      --locked \
      --root "$tool_root"
fi
wasm_bindgen="$tool_root/bin/wasm-bindgen"
[[ -x "$wasm_bindgen" ]] || {
  echo "exact wasm-bindgen binary is missing: $wasm_bindgen" >&2
  exit 1
}
[[ "$($wasm_bindgen --version)" == "wasm-bindgen $wasm_bindgen_version" ]] || {
  echo "unexpected wasm-bindgen version: $($wasm_bindgen --version)" >&2
  exit 1
}

server_rustflags="${RUSTFLAGS:-}"
client_rustflags="${RUSTOK_PAGES_INLINE_EDIT_CLIENT_RUSTFLAGS:-$server_rustflags}"
rustup target add wasm32-unknown-unknown
CARGO_TARGET_DIR="$target_dir" \
RUSTFLAGS="$client_rustflags" \
RUSTOK_PAGES_INLINE_EDIT_PROFILE="$profile" \
RUSTOK_WASM_BINDGEN_BIN="$wasm_bindgen" \
node "$client_builder"

test -s "$repo_root/target/site/assets/pages-inline-edit-bootstrap.js"
test -s "$repo_root/target/site/assets/pages-inline-edit/rustok_storefront.js"
test -s "$repo_root/target/site/assets/pages-inline-edit/rustok_storefront_bg.wasm"

cargo_args=(
  build
  --locked
  -p rustok-server
  --bin rustok-server
  --features pages-inline-edit-assets
)
if [[ "$profile" == "release" ]]; then
  cargo_args+=(--release)
fi
CARGO_TARGET_DIR="$target_dir" RUSTFLAGS="$server_rustflags" cargo "${cargo_args[@]}"
test -x "$target_dir/$profile/rustok-server"

echo "✔ built rustok-server with embedded Pages inline edit assets ($profile, wasm-bindgen $wasm_bindgen_version)"
