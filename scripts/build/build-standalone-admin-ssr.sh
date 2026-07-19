#!/usr/bin/env bash
set -euo pipefail

usage() {
  cat <<'USAGE'
Usage: build-standalone-admin-ssr.sh [--tool-root DIR] [--target-dir DIR] [--skip-tool-install]
USAGE
}

repo_root=$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)
tool_root="$repo_root/.tools/cargo-leptos-0.3.6"
target_dir="${CARGO_TARGET_DIR:-$repo_root/target}"
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

tool_root=$(mkdir -p "$tool_root" && cd "$tool_root" && pwd)
target_dir=$(mkdir -p "$target_dir" && cd "$target_dir" && pwd)

if [[ $skip_tool_install -eq 0 ]]; then
  CARGO_TARGET_DIR="$tool_root/target" \
    cargo install cargo-leptos --version 0.3.6 --locked --root "$tool_root"
fi
[[ -x "$tool_root/bin/cargo-leptos" ]] || {
  echo "exact cargo-leptos binary is missing: $tool_root/bin/cargo-leptos" >&2
  exit 1
}
version=$(PATH="$tool_root/bin:$PATH" cargo leptos --version)
[[ "$version" == *"0.3.6"* ]] || {
  echo "unexpected cargo-leptos version: $version" >&2
  exit 1
}

rustup target add wasm32-unknown-unknown
npm ci --prefix "$repo_root/apps/admin" --no-audit --no-fund
rm -rf "$target_dir/site" "$target_dir/server"

(
  cd "$repo_root"
  PATH="$tool_root/bin:$PATH" \
  CARGO_TARGET_DIR="$target_dir" \
  cargo leptos build --release --locked -p rustok-admin
)

site_pkg="$target_dir/site/pkg"
server_binary="$target_dir/server/release/rustok-admin"
mkdir -p "$site_pkg"
TRUNK_STAGING_DIR="$site_pkg" node "$repo_root/apps/admin/scripts/tailwind-build.mjs"
mv "$site_pkg/output.css" "$site_pkg/rustok-admin.css"

[[ -x "$server_binary" ]] || {
  echo "standalone SSR binary is missing: $server_binary" >&2
  exit 1
}
[[ -s "$site_pkg/rustok-admin.css" ]] || {
  echo "standalone SSR stylesheet is missing" >&2
  exit 1
}
find "$site_pkg" -maxdepth 1 -type f -name '*.js' -size +0c -print -quit | grep -q . || {
  echo "standalone SSR JavaScript hydration artifact is missing" >&2
  exit 1
}
find "$site_pkg" -maxdepth 1 -type f -name '*.wasm' -size +0c -print -quit | grep -q . || {
  echo "standalone SSR WebAssembly hydration artifact is missing" >&2
  exit 1
}
grep -Fq 'href="/pkg/rustok-admin.css"' "$repo_root/apps/admin/src/app/shell.rs" || {
  echo "standalone SSR shell stylesheet contract is missing" >&2
  exit 1
}

echo "✔ built standalone admin SSR binary, hydration bundle and Tailwind CSS with cargo-leptos 0.3.6"
