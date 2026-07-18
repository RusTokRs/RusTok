#!/usr/bin/env bash
set -euo pipefail

usage() {
  cat <<'USAGE'
Usage: build-embedded-admin.sh [--tool-root DIR] [--target-dir DIR] [--public-url /path/] [--skip-tool-install]
USAGE
}

repo_root=$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)
tool_root="$repo_root/.tools/trunk-0.21.14"
target_dir="${CARGO_TARGET_DIR:-$repo_root/target}"
public_url="/admin/"
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
    --public-url)
      [[ $# -ge 2 ]] || { usage >&2; exit 2; }
      public_url=$2
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

[[ "$public_url" =~ ^/([A-Za-z0-9._-]+/)*$ ]] || {
  echo "--public-url must be / or a canonical absolute path ending in /" >&2
  exit 2
}

tool_root=$(mkdir -p "$tool_root" && cd "$tool_root" && pwd)
target_dir=$(mkdir -p "$target_dir" && cd "$target_dir" && pwd)
trunk="$tool_root/bin/trunk"

if [[ $skip_tool_install -eq 0 ]]; then
  CARGO_TARGET_DIR="$tool_root/target" \
    cargo install trunk --version 0.21.14 --locked --root "$tool_root"
fi
[[ -x "$trunk" ]] || {
  echo "exact Trunk binary is missing: $trunk" >&2
  exit 1
}
[[ "$($trunk --version)" == "trunk 0.21.14" ]] || {
  echo "unexpected Trunk version: $($trunk --version)" >&2
  exit 1
}

rustup target add wasm32-unknown-unknown
npm ci --prefix "$repo_root/apps/admin" --no-audit --no-fund
rm -rf "$repo_root/apps/admin/dist"

(
  cd "$repo_root/apps/admin"
  CARGO_TARGET_DIR="$target_dir" \
  TRUNK_BUILD_PUBLIC_URL="$public_url" \
  TRUNK_BUILD_LOCKED="true" \
  "$trunk" build --release
)

index="$repo_root/apps/admin/dist/index.html"
css="$repo_root/apps/admin/dist/output.css"
[[ -s "$index" ]] || { echo "embedded admin index is missing" >&2; exit 1; }
[[ -s "$css" ]] || { echo "embedded admin stylesheet is missing" >&2; exit 1; }
grep -Fq '<title>RusToK Admin</title>' "$index" || {
  echo "embedded admin title marker is missing" >&2
  exit 1
}
grep -Fq 'href="output.css"' "$repo_root/apps/admin/index.html" || {
  echo "admin source stylesheet must remain mount-relative" >&2
  exit 1
}
if [[ "$public_url" != "/" ]]; then
  if grep -Eq '(src|href)="/(rustok-admin|snippets|output\.css)' "$index"; then
    echo "embedded admin output contains a root-mounted asset URL" >&2
    exit 1
  fi
  grep -Fq "$public_url" "$index" || {
    echo "embedded admin output does not contain public URL $public_url" >&2
    exit 1
  }
fi

echo "✔ built deterministic admin assets for $public_url with Trunk 0.21.14"
