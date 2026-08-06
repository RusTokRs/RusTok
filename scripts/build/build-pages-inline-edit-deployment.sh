#!/usr/bin/env bash
set -euo pipefail

usage() {
  cat <<'USAGE'
Usage: build-pages-inline-edit-deployment.sh [--trunk-tool-root DIR] [--wasm-bindgen-tool-root DIR] [--admin-target-dir DIR] [--server-target-dir DIR] [--skip-trunk-tool-install] [--skip-wasm-bindgen-tool-install]
USAGE
}

repo_root=$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)
trunk_tool_root="$repo_root/.tools/trunk-0.21.14"
wasm_bindgen_tool_root="$repo_root/.tools/wasm-bindgen-cli"
admin_target_dir="$repo_root/target/admin-assets"
server_target_dir="${CARGO_TARGET_DIR:-$repo_root/target}"
skip_trunk_tool_install=0
skip_wasm_bindgen_tool_install=0

while [[ $# -gt 0 ]]; do
  case "$1" in
    --trunk-tool-root)
      [[ $# -ge 2 ]] || { usage >&2; exit 2; }
      trunk_tool_root=$2
      shift 2
      ;;
    --wasm-bindgen-tool-root)
      [[ $# -ge 2 ]] || { usage >&2; exit 2; }
      wasm_bindgen_tool_root=$2
      shift 2
      ;;
    --admin-target-dir)
      [[ $# -ge 2 ]] || { usage >&2; exit 2; }
      admin_target_dir=$2
      shift 2
      ;;
    --server-target-dir)
      [[ $# -ge 2 ]] || { usage >&2; exit 2; }
      server_target_dir=$2
      shift 2
      ;;
    --skip-trunk-tool-install)
      skip_trunk_tool_install=1
      shift
      ;;
    --skip-wasm-bindgen-tool-install)
      skip_wasm_bindgen_tool_install=1
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

trunk_tool_root=$(mkdir -p "$trunk_tool_root" && cd "$trunk_tool_root" && pwd)
wasm_bindgen_tool_root=$(mkdir -p "$wasm_bindgen_tool_root" && cd "$wasm_bindgen_tool_root" && pwd)
admin_target_dir=$(mkdir -p "$admin_target_dir" && cd "$admin_target_dir" && pwd)
server_target_dir=$(mkdir -p "$server_target_dir" && cd "$server_target_dir" && pwd)

admin_builder="$repo_root/scripts/build/build-embedded-admin.sh"
server_builder="$repo_root/scripts/build/build-pages-inline-edit-server.sh"
admin_args=(
  --tool-root "$trunk_tool_root"
  --target-dir "$admin_target_dir"
  --pages-inline-edit-launch
)
if [[ $skip_trunk_tool_install -eq 1 ]]; then
  admin_args+=(--skip-tool-install)
fi

server_args=(
  --tool-root "$wasm_bindgen_tool_root"
  --target-dir "$server_target_dir"
  --profile release
)
if [[ $skip_wasm_bindgen_tool_install -eq 1 ]]; then
  server_args+=(--skip-tool-install)
fi

server_rustflags="${RUSTFLAGS:-}"
admin_rustflags="${RUSTOK_EMBEDDED_ADMIN_RUSTFLAGS:-$server_rustflags}"
RUSTFLAGS="$admin_rustflags" bash "$admin_builder" "${admin_args[@]}"
RUSTFLAGS="$server_rustflags" bash "$server_builder" "${server_args[@]}"

test -s "$repo_root/apps/admin/dist/index.html"
test -s "$repo_root/apps/admin/dist/output.css"
test -s "$repo_root/target/site/assets/pages-inline-edit-bootstrap.js"
test -s "$repo_root/target/site/assets/pages-inline-edit/rustok_storefront.js"
test -s "$repo_root/target/site/assets/pages-inline-edit/rustok_storefront_bg.wasm"
test -x "$server_target_dir/release/rustok-server"

echo "✔ built same-origin Pages inline edit deployment composition"
