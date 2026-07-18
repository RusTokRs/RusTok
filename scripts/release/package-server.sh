#!/usr/bin/env bash
set -euo pipefail

usage() {
  cat <<'USAGE'
Usage:
  package-server.sh --binary FILE --version VERSION --created-epoch EPOCH \
    --config-dir DIR --license FILE --output-dir DIR [--github-output FILE]
  package-server.sh --self-test
USAGE
}

fail() {
  echo "release server packaging failed: $*" >&2
  exit 1
}

self_test=0
binary=""
version=""
created_epoch=""
config_dir=""
license_file=""
output_dir=""
github_output=""

while [[ $# -gt 0 ]]; do
  case "$1" in
    --self-test)
      self_test=1
      shift
      ;;
    --binary|--version|--created-epoch|--config-dir|--license|--output-dir|--github-output)
      [[ $# -ge 2 ]] || fail "$1 requires a value"
      key=${1#--}
      key=${key//-/_}
      printf -v "$key" '%s' "$2"
      shift 2
      ;;
    --help|-h)
      usage
      exit 0
      ;;
    *)
      fail "unknown argument: $1"
      ;;
  esac
done

package_once() {
  local source_binary=$1
  local release_version=$2
  local epoch=$3
  local source_config=$4
  local source_license=$5
  local destination=$6

  [[ "$release_version" =~ ^(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)(-[0-9A-Za-z.-]+)?$ ]] \
    || fail "version must be canonical SemVer without build metadata"
  [[ "$epoch" =~ ^(0|[1-9][0-9]*)$ ]] || fail "created epoch must be a non-negative integer"
  [[ -f "$source_binary" && -x "$source_binary" ]] || fail "binary must be an executable regular file: $source_binary"
  [[ -d "$source_config" ]] || fail "config directory does not exist: $source_config"
  [[ -f "$source_license" ]] || fail "license file does not exist: $source_license"
  if find "$source_config" -type l -print -quit | grep -q .; then
    fail "config directory must not contain symlinks"
  fi

  mkdir -p "$destination"
  local stage
  stage=$(mktemp -d)
  trap 'rm -rf "$stage"' RETURN
  local package_name="rustok-server-${release_version}-linux-x86_64"
  local package_root="$stage/$package_name"
  mkdir -p "$package_root/config"
  install -m 0755 "$source_binary" "$package_root/rustok-server"
  install -m 0644 "$source_license" "$package_root/LICENSE"
  cp -R "$source_config"/. "$package_root/config/"
  find "$package_root/config" -type f -exec chmod 0644 {} +
  find "$package_root/config" -type d -exec chmod 0755 {} +

  local archive="$destination/${package_name}.tar.gz"
  tar \
    --sort=name \
    --format=gnu \
    --mtime="@$epoch" \
    --owner=0 \
    --group=0 \
    --numeric-owner \
    -C "$stage" \
    -cf - "$package_name" \
    | gzip -n -9 > "$archive"
  [[ -s "$archive" ]] || fail "archive was not created"
  printf '%s\n' "$archive"
}

if [[ $self_test -eq 1 ]]; then
  root=$(mktemp -d)
  trap 'rm -rf "$root"' EXIT
  mkdir -p "$root/config" "$root/out-a" "$root/out-b"
  printf '#!/usr/bin/env sh\necho rustok\n' > "$root/rustok-server"
  chmod 0755 "$root/rustok-server"
  printf 'server:\n  port: 5150\n' > "$root/config/default.yaml"
  printf 'license\n' > "$root/LICENSE"
  first=$(package_once "$root/rustok-server" "1.2.3" "1784332800" "$root/config" "$root/LICENSE" "$root/out-a")
  second=$(package_once "$root/rustok-server" "1.2.3" "1784332800" "$root/config" "$root/LICENSE" "$root/out-b")
  first_digest=$(sha256sum "$first" | awk '{print $1}')
  second_digest=$(sha256sum "$second" | awk '{print $1}')
  [[ "$first_digest" == "$second_digest" ]] || fail "deterministic package self-test produced different digests"
  echo "✔ deterministic server packaging self-test passed"
  exit 0
fi

[[ -n "$binary" ]] || fail "--binary is required"
[[ -n "$version" ]] || fail "--version is required"
[[ -n "$created_epoch" ]] || fail "--created-epoch is required"
[[ -n "$config_dir" ]] || fail "--config-dir is required"
[[ -n "$license_file" ]] || fail "--license is required"
[[ -n "$output_dir" ]] || fail "--output-dir is required"

archive=$(package_once \
  "$binary" \
  "$version" \
  "$created_epoch" \
  "$config_dir" \
  "$license_file" \
  "$output_dir")
archive_name=$(basename "$archive")
archive_sha256=$(sha256sum "$archive" | awk '{print $1}')

if [[ -n "$github_output" ]]; then
  {
    echo "archive=$archive"
    echo "archive_name=$archive_name"
    echo "archive_sha256=$archive_sha256"
  } >> "$github_output"
fi

echo "✔ packaged $archive_name ($archive_sha256)"
