#!/usr/bin/env bash
# Install the pinned tools needed by the repository's development build.
#
# The installer owns only the repository-local mold archive and the pinned
# rustup toolchain components. It does not install system packages or alter
# other toolchains; Linux clang remains an operating-system prerequisite.

set -euo pipefail

script_dir=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)
# shellcheck source=scripts/build-tools-common.sh
. "$script_dir/build-tools-common.sh"

MOLD_RELEASE_BASE_URL=${MOLD_RELEASE_BASE_URL:-https://github.com/rui314/mold/releases/download}
CURL_CONNECT_TIMEOUT=${CURL_CONNECT_TIMEOUT:-15}
CURL_MIN_BYTES_PER_SECOND=${CURL_MIN_BYTES_PER_SECOND:-1024}
CURL_STALL_SECONDS=${CURL_STALL_SECONDS:-60}
BUILD_TOOLS_WORKDIR=

cleanup_workdir() {
  local status=$?
  trap - EXIT
  if [[ -n $BUILD_TOOLS_WORKDIR ]]; then
    rm -rf -- "$BUILD_TOOLS_WORKDIR" || status=1
  fi
  exit "$status"
}
trap cleanup_workdir EXIT

verify_mold_archive() {
  local archive=$1 name=$2
  local -a expected=()
  [[ -f $MOLD_SHA256SUMS_FILE ]] || fail "missing checksum file: $MOLD_SHA256SUMS_FILE"
  mapfile -t expected < <(awk -v name="$name" '$2 == name { print $1 }' "$MOLD_SHA256SUMS_FILE")
  [[ ${#expected[@]} -eq 1 ]] ||
    fail "expected one checksum for $name in $MOLD_SHA256SUMS_FILE"
  [[ ${expected[0]} =~ ^[[:xdigit:]]{64}$ ]] ||
    fail "invalid SHA-256 entry for $name in $MOLD_SHA256SUMS_FILE"
  printf '%s  %s\n' "${expected[0]}" "$archive" | sha256sum --check --status ||
    fail "checksum mismatch for $name; refusing to install it"
  note "verified $name against $MOLD_SHA256SUMS_FILE"
}

install_mold() {
  local version=$1 arch name url stage
  if ! is_linux; then
    note "mold is Linux-only; skipping on $(uname -s)"
    return 0
  fi
  command -v curl >/dev/null 2>&1 || fail 'curl is required to download the pinned mold binary'
  command -v sha256sum >/dev/null 2>&1 || fail 'sha256sum is required to verify mold'
  command -v tar >/dev/null 2>&1 || fail 'tar is required to unpack mold'

  arch=$(mold_arch) || return 1
  name="mold-$version-$arch-linux.tar.gz"
  url="$MOLD_RELEASE_BASE_URL/v$version/$name"
  BUILD_TOOLS_WORKDIR=$(mktemp -d "${TMPDIR:-/tmp}/thysalion-build-tools.XXXXXX")
  stage="$BUILD_TOOLS_WORKDIR/unpacked"
  mkdir -p -- "$stage"

  note "downloading $url"
  curl --fail --silent --show-error --location \
    --connect-timeout "$CURL_CONNECT_TIMEOUT" \
    --speed-limit "$CURL_MIN_BYTES_PER_SECOND" --speed-time "$CURL_STALL_SECONDS" \
    --output "$BUILD_TOOLS_WORKDIR/$name" "$url" ||
    fail "failed to download $name"
  verify_mold_archive "$BUILD_TOOLS_WORKDIR/$name" "$name"

  tar --extract --gzip --strip-components=1 --directory "$stage" \
    --file "$BUILD_TOOLS_WORKDIR/$name" || fail "failed to unpack $name"
  mkdir -p -- "$BUILD_TOOLS_PREFIX"
  cp -a -- "$stage/." "$BUILD_TOOLS_PREFIX/" ||
    fail "failed to install mold into $BUILD_TOOLS_PREFIX"
  note "installed mold $version into $BUILD_TOOLS_PREFIX"
}

install_toolchain() {
  local toolchain=$1
  command -v "$RUSTUP" >/dev/null 2>&1 ||
    fail 'rustup not found on PATH; install it from https://rustup.rs'
  note "installing toolchain $toolchain without changing the default"
  "$RUSTUP" toolchain install "$toolchain" --profile minimal --no-self-update ||
    fail "failed to install toolchain $toolchain"
  "$RUSTUP" component add --toolchain "$toolchain" "${BUILD_TOOLS_COMPONENTS[@]}" ||
    fail "failed to install required components for $toolchain"
}

main() {
  local mold_pin toolchain_pin
  mold_pin=$(mold_version) || return 1
  toolchain_pin=$(pinned_toolchain) || return 1
  install_mold "$mold_pin"
  install_toolchain "$toolchain_pin"
  append_build_tools_to_github_path
  note 'build prerequisites installed; verify with: make check-build-tools'
}

main "$@"
