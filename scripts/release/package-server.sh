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

package_once() (
  set -euo pipefail
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
  [[ -f "$source_config/production.redis.example.yaml" ]] || fail "production config template is missing: $source_config/production.redis.example.yaml"
  [[ -f "$source_license" ]] || fail "license file does not exist: $source_license"
  if find "$source_config" -type l -print -quit | grep -q .; then
    fail "config directory must not contain symlinks"
  fi

  mkdir -p "$destination"
  local stage
  stage=$(mktemp -d)
  trap 'rm -rf "$stage"' EXIT
  local package_name="rustok-server-${release_version}-linux-x86_64"
  local package_root="$stage/$package_name"
  mkdir -p "$package_root/config"
  install -m 0755 "$source_binary" "$package_root/rustok-server"
  install -m 0644 "$source_license" "$package_root/LICENSE"
  install -m 0644 "$source_config/production.redis.example.yaml" "$package_root/config/production.yaml.example"
  cat > "$package_root/README.md" <<'README'
# RusTok Server release package

This archive intentionally includes only a production configuration template.
Development, test and browser-smoke configurations are not shipped.

1. Copy `config/production.yaml.example` to `config/production.yaml`.
2. Replace the database URI, JWT secret, SMTP credentials and example hostnames.
3. Set `RUSTOK_ENV=production`, `RUSTOK_CONFIG_DIR` to this config directory,
   and `RUSTOK_HTTPS=true` so the host emits HSTS.
4. If Redis-backed rate limiting is enabled, provide `RUSTOK_REDIS_URL`.
5. Start `./rustok-server`.

The production template contains placeholders and is rejected by production
secret validation until it has been configured with deployment-specific values.
README
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
)

if [[ $self_test -eq 1 ]]; then
  root=$(mktemp -d)
  trap 'rm -rf "$root"' EXIT
  mkdir -p "$root/config" "$root/out-a" "$root/out-b"
  printf '#!/usr/bin/env sh\necho rustok\n' > "$root/rustok-server"
  chmod 0755 "$root/rustok-server"
  printf 'production template\n' > "$root/config/production.redis.example.yaml"
  printf 'development-only secret\n' > "$root/config/development.yaml"
  printf 'test-only secret\n' > "$root/config/test.yaml"
  printf 'browser-smoke-only secret\n' > "$root/config/browser-smoke.yaml"
  printf 'license\n' > "$root/LICENSE"
  first=$(package_once "$root/rustok-server" "1.2.3" "1784332800" "$root/config" "$root/LICENSE" "$root/out-a")
  second=$(package_once "$root/rustok-server" "1.2.3" "1784332800" "$root/config" "$root/LICENSE" "$root/out-b")
  package_root="rustok-server-1.2.3-linux-x86_64"
  tar -tzf "$first" | grep -Fqx "$package_root/config/production.yaml.example" || fail "production config template is missing from archive"
  tar -tzf "$first" | grep -Fqx "$package_root/README.md" || fail "release package README is missing from archive"
  if tar -tzf "$first" | grep -Eq '/(development|test|browser-smoke)\.yaml
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
; then
    fail "development/test/browser-smoke config leaked into release archive"
  fi
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
