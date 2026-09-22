#!/usr/bin/env bash
set -euo pipefail

# Sync Tailwind crates from upstream into RusToK vendored paths.
#
# Default upstream: https://github.com/oovm/tailwind-rs
# Synced crates:
#   upstream tailwind-rs      -> crates/tailwind-rs
#   upstream tailwind-css     -> crates/tailwind-css
#   upstream tailwind-ast     -> crates/tailwind-ast
#   upstream tailwind-error   -> third_party/patches/tailwind-error

usage() {
  cat <<'USAGE'
Usage:
  scripts/tailwind/vendor_tailwind_rs.sh [--repo URL] [--ref REF] [--check]

Options:
  --repo URL   Upstream git repository URL (default: https://github.com/oovm/tailwind-rs)
  --ref REF    Git ref to checkout (tag/branch/sha). If omitted, default branch is used.
  --check      Compare only (no file writes). Exits 1 if differences are found.
  -h, --help   Show this help.
USAGE
}

REPO_URL="https://github.com/oovm/tailwind-rs"
GIT_REF=""
CHECK_ONLY=0

while [[ $# -gt 0 ]]; do
  case "$1" in
    --repo)
      REPO_URL="$2"
      shift 2
      ;;
    --ref)
      GIT_REF="$2"
      shift 2
      ;;
    --check)
      CHECK_ONLY=1
      shift
      ;;
    -h|--help)
      usage
      exit 0
      ;;
    *)
      echo "Unknown option: $1" >&2
      usage >&2
      exit 2
      ;;
  esac
done

if ! command -v rsync >/dev/null 2>&1; then
  echo "rsync is required" >&2
  exit 2
fi

REPO_ROOT="$(git rev-parse --show-toplevel)"
TMP_DIR="$(mktemp -d)"
cleanup() {
  rm -rf "$TMP_DIR"
}
trap cleanup EXIT

if [[ -n "$GIT_REF" ]]; then
  git clone --depth 1 --branch "$GIT_REF" "$REPO_URL" "$TMP_DIR/upstream"
else
  git clone --depth 1 "$REPO_URL" "$TMP_DIR/upstream"
fi

resolve_src_dir() {
  local name="$1"
  local direct="$TMP_DIR/upstream/$name"
  local nested="$TMP_DIR/upstream/crates/$name"
  if [[ -d "$direct" ]]; then
    echo "$direct"
  elif [[ -d "$nested" ]]; then
    echo "$nested"
  else
    return 1
  fi
}

sync_crate() {
  local crate="$1"
  local target_rel="$2"
  local src
  src="$(resolve_src_dir "$crate")" || {
    echo "Cannot find crate '$crate' in upstream repository" >&2
    exit 1
  }

  local target="$REPO_ROOT/$target_rel"
  if [[ ! -d "$target" ]]; then
    echo "Target directory does not exist: $target" >&2
    exit 1
  fi

  if [[ "$CHECK_ONLY" -eq 1 ]]; then
    if ! diff -ruN --exclude .git --exclude target "$src" "$target" >/dev/null; then
      echo "DIFF: $crate -> $target_rel"
      return 1
    fi
    echo "OK: $crate matches $target_rel"
  else
    rsync -a --delete --exclude .git --exclude target "$src/" "$target/"
    echo "SYNCED: $crate -> $target_rel"
  fi
}

changed=0
sync_crate "tailwind-rs" "crates/tailwind-rs" || changed=1
sync_crate "tailwind-css" "crates/tailwind-css" || changed=1
sync_crate "tailwind-ast" "crates/tailwind-ast" || changed=1
sync_crate "tailwind-error" "third_party/patches/tailwind-error" || changed=1

if [[ "$CHECK_ONLY" -eq 1 && "$changed" -ne 0 ]]; then
  echo "Upstream and vendored copies differ." >&2
  exit 1
fi

if [[ "$CHECK_ONLY" -eq 0 ]]; then
  echo "Vendoring complete. Next steps:"
  echo "  1) cargo test -p tailwind-rs"
  echo "  2) cargo test -p tailwind-css"
  echo "  3) review git diff and keep local fixes in separate commits"
fi
