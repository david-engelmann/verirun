#!/bin/sh
# Standalone release installer for verirun.
# Downloads the platform archive and SHA256SUMS from GitHub Releases,
# verifies the archive SHA-256 digest before extraction, and installs
# the verirun executable with mode 0755.

set -eu

DEFAULT_BASE_URL="https://github.com/david-engelmann/verirun/releases"

usage() {
  cat <<'EOF'
Install a prebuilt verirun release binary with SHA-256 verification.

Usage:
  install.sh [OPTIONS]

Options:
  --version <vX.Y.Z>     Release tag to install (defaults to latest release)
  --install-dir <DIR>    Destination directory (default: $INSTALL_DIR or $HOME/.local/bin)
  --base-url <URL>       Release root URL (default: https://github.com/david-engelmann/verirun/releases)
  -h, --help             Print this help text and exit

Environment:
  VERIRUN_VERSION        Default version when --version is omitted
  INSTALL_DIR            Default destination directory when --install-dir is omitted
  VERIRUN_BASE_URL       Default release root URL when --base-url is omitted
EOF
}

err() {
  printf 'install.sh: error: %s\n' "$*" >&2
  exit 1
}

need_cmd() {
  if ! command -v "$1" >/dev/null 2>&1; then
    err "required command '$1' was not found in PATH"
  fi
}

fetch_to_file() {
  src_url="$1"
  dst_file="$2"
  if command -v curl >/dev/null 2>&1; then
    curl -fsSL "$src_url" -o "$dst_file" || err "failed to download $src_url"
  elif command -v wget >/dev/null 2>&1; then
    case "$src_url" in
      file://*)
        src_path="${src_url#file://}"
        [ -f "$src_path" ] || err "local release file not found: $src_path"
        cp "$src_path" "$dst_file"
        ;;
      *)
        wget -qO "$dst_file" "$src_url" || err "failed to download $src_url"
        ;;
    esac
  else
    err "neither 'curl' nor 'wget' is available to download release assets"
  fi
}

resolve_latest_tag() {
  root_url="${1%/}"
  case "$root_url" in
    file://*)
      local_root="${root_url#file://}"
      if [ -f "$local_root/latest" ]; then
        tr -d ' \t\r\n' <"$local_root/latest"
        return 0
      fi
      err "--version <vX.Y.Z> is required when installing from a local file:// base URL"
      ;;
  esac

  latest_url="$root_url/latest"
  if command -v curl >/dev/null 2>&1; then
    effective=$(curl -fsSL -o /dev/null -w '%{url_effective}' "$latest_url") ||
      err "failed to resolve latest release from $latest_url"
  elif command -v wget >/dev/null 2>&1; then
    effective=$(wget -S --spider "$latest_url" 2>&1 | awk '/^  Location: / { loc=$2 } END { print loc }' | tr -d '\r')
    [ -n "$effective" ] || err "failed to resolve latest release from $latest_url"
  else
    err "neither 'curl' nor 'wget' is available to query the latest release"
  fi

  resolved_tag="${effective##*/}"
  case "$resolved_tag" in
    v[0-9]*.[0-9]*.[0-9]*)
      printf '%s\n' "$resolved_tag"
      ;;
    *)
      err "could not parse release tag from redirect URL: $effective"
      ;;
  esac
}

compute_sha256() {
  target_file="$1"
  if command -v sha256sum >/dev/null 2>&1; then
    sha256sum "$target_file" | awk '{ print $1 }' | tr 'A-F' 'a-f'
  elif command -v shasum >/dev/null 2>&1; then
    shasum -a 256 "$target_file" | awk '{ print $1 }' | tr 'A-F' 'a-f'
  elif command -v openssl >/dev/null 2>&1; then
    openssl dgst -sha256 "$target_file" | awk '{ print $NF }' | tr 'A-F' 'a-f'
  else
    err "no SHA-256 tool found (expected 'sha256sum', 'shasum', or 'openssl')"
  fi
}

detect_label() {
  uname_s=$(uname -s 2>/dev/null || printf 'unknown')
  uname_m=$(uname -m 2>/dev/null || printf 'unknown')

  case "$uname_s" in
    Linux) os_name="linux" ;;
    Darwin) os_name="macos" ;;
    *) err "unsupported operating system '$uname_s' (supported: Linux, Darwin)" ;;
  esac

  case "$uname_m" in
    x86_64 | amd64) arch_name="x86_64" ;;
    aarch64 | arm64) arch_name="aarch64" ;;
    *) err "unsupported CPU architecture '$uname_m' (supported: x86_64, aarch64)" ;;
  esac

  printf '%s-%s\n' "$os_name" "$arch_name"
}

req_version="${VERIRUN_VERSION:-}"
install_dir="${INSTALL_DIR:-${HOME:-}/.local/bin}"
base_url="${VERIRUN_BASE_URL:-$DEFAULT_BASE_URL}"

while [ "$#" -gt 0 ]; do
  case "$1" in
    -h | --help)
      usage
      exit 0
      ;;
    --version)
      [ "$#" -ge 2 ] || err "--version requires a value"
      req_version="$2"
      shift 2
      ;;
    --version=*)
      req_version="${1#--version=}"
      shift
      ;;
    --install-dir)
      [ "$#" -ge 2 ] || err "--install-dir requires a value"
      install_dir="$2"
      shift 2
      ;;
    --install-dir=*)
      install_dir="${1#--install-dir=}"
      shift
      ;;
    --base-url)
      [ "$#" -ge 2 ] || err "--base-url requires a value"
      base_url="$2"
      shift 2
      ;;
    --base-url=*)
      base_url="${1#--base-url=}"
      shift
      ;;
    *)
      err "unknown option '$1' (run with --help for usage)"
      ;;
  esac
done

[ -n "$install_dir" ] || err "install directory must not be empty"
[ -n "$base_url" ] || err "base URL must not be empty"

need_cmd tar
need_cmd awk
need_cmd tr
need_cmd mktemp

if [ -z "$req_version" ]; then
  req_version=$(resolve_latest_tag "$base_url")
fi

version_num="${req_version#v}"
case "$version_num" in
  [0-9]*.[0-9]*.[0-9]*) ;;
  *) err "invalid version '$req_version' (expected vX.Y.Z or X.Y.Z)" ;;
esac
case "$version_num" in
  */* | *" "* | *"	"*)
    err "invalid characters in version '$req_version'"
    ;;
esac
tag="v$version_num"

label=$(detect_label)
stage="verirun-v${version_num}-${label}"
archive="${stage}.tar.gz"

trimmed_base="${base_url%/}"
release_url="${trimmed_base}/download/${tag}"
case "$trimmed_base" in
  file://*)
    local_dir="${trimmed_base#file://}"
    if [ ! -d "$local_dir/download/$tag" ] && [ -f "$local_dir/SHA256SUMS" ]; then
      release_url="$trimmed_base"
    fi
    ;;
esac

tmp_dir=""
cleanup() {
  if [ -n "$tmp_dir" ] && [ -d "$tmp_dir" ]; then
    rm -rf "$tmp_dir"
  fi
}
trap cleanup EXIT INT TERM HUP
tmp_dir=$(mktemp -d "${TMPDIR:-/tmp}/verirun-install.XXXXXX")

archive_path="$tmp_dir/$archive"
sums_path="$tmp_dir/SHA256SUMS"

fetch_to_file "$release_url/$archive" "$archive_path"
fetch_to_file "$release_url/SHA256SUMS" "$sums_path"

expected_sha=$(
  awk -v target="$archive" '
    NF >= 2 {
      name = $2
      sub(/^\*/, "", name)
      if (name == target) {
        print $1
        exit
      }
    }
  ' "$sums_path" | tr 'A-F' 'a-f'
)

[ -n "$expected_sha" ] || err "archive '$archive' was not listed in SHA256SUMS"
case "$expected_sha" in
  *[!0-9a-f]*) err "malformed SHA-256 digest in SHA256SUMS for '$archive'" ;;
esac
[ "${#expected_sha}" -eq 64 ] || err "SHA-256 digest in SHA256SUMS for '$archive' is not 64 hex characters"

actual_sha=$(compute_sha256 "$archive_path")
if [ "$actual_sha" != "$expected_sha" ]; then
  err "SHA-256 verification failed for '$archive' (expected $expected_sha, got $actual_sha)"
fi

tar -xzf "$archive_path" -C "$tmp_dir"
extracted_bin="$tmp_dir/$stage/verirun"
[ -f "$extracted_bin" ] || err "expected binary '$stage/verirun' was not found inside '$archive'"

mkdir -p "$install_dir"
staged_bin="$install_dir/.verirun.install.$$"
cp "$extracted_bin" "$staged_bin"
chmod 0755 "$staged_bin"
mv -f "$staged_bin" "$install_dir/verirun"

printf 'Installed verirun %s to %s/verirun (SHA-256 verified: %s)\n' "$tag" "$install_dir" "$actual_sha"
case ":${PATH:-}:" in
  *":$install_dir:"*) ;;
  *)
    printf 'Note: %s is not currently on your PATH.\n' "$install_dir"
    ;;
esac
