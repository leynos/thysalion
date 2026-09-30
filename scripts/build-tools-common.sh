#!/usr/bin/env bash
# Shared pin, platform, path-persistence and diagnostic helpers for the two
# build-tools entry points.
#
# This helper is owned by install-build-tools.sh and check-build-tools.sh. Keep
# it limited to those scripts' version resolution, platform checks and
# preserving the installed mold path for later GitHub Actions steps; it is not a
# general-purpose shell library.

set -euo pipefail

BUILD_TOOLS_SCRIPT_DIR=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)
BUILD_TOOLS_REPO_ROOT=$(cd -- "$BUILD_TOOLS_SCRIPT_DIR/.." && pwd)

MOLD_VERSION_FILE=${MOLD_VERSION_FILE:-$BUILD_TOOLS_REPO_ROOT/tools/mold/VERSION}
MOLD_SHA256SUMS_FILE=${MOLD_SHA256SUMS_FILE:-$BUILD_TOOLS_REPO_ROOT/tools/mold/SHA256SUMS}
RUST_TOOLCHAIN_FILE=${RUST_TOOLCHAIN_FILE:-$BUILD_TOOLS_REPO_ROOT/rust-toolchain.toml}
BUILD_TOOLS_PREFIX=${BUILD_TOOLS_PREFIX:-$HOME/.local}
RUSTUP=${RUSTUP:-rustup}

# Mirror the components in rust-toolchain.toml and include rust-analyzer as a
# required maintenance component. Keep this list synchronized with that pin.
BUILD_TOOLS_COMPONENTS=(
  clippy
  llvm-tools-preview
  rustfmt
  rust-analyzer
)

note() { printf 'build-tools: %s\n' "$*" >&2; }

fail() {
  note "$*"
  exit 1
}

read_pin() {
  local file=$1 value
  local -a lines=()
  [[ -f $file ]] || fail "missing version pin: $file"
  mapfile -t lines < "$file"
  [[ ${#lines[@]} -eq 1 ]] || fail "expected one version line in $file"
  value=${lines[0]}
  value=${value#"${value%%[![:space:]]*}"}
  value=${value%"${value##*[![:space:]]}"}
  [[ -n $value ]] || fail "empty version pin: $file"
  [[ $value != *[[:space:]]* ]] || fail "version pin contains whitespace: $file"
  printf '%s' "$value"
}

mold_version() { read_pin "$MOLD_VERSION_FILE"; }

pinned_toolchain() {
  local -a channels=()
  [[ -f $RUST_TOOLCHAIN_FILE ]] || fail "missing toolchain pin: $RUST_TOOLCHAIN_FILE"
  mapfile -t channels < <(
    awk -F '"' '/^[[:space:]]*channel[[:space:]]*=/ { print $2 }' "$RUST_TOOLCHAIN_FILE"
  )
  [[ ${#channels[@]} -eq 1 && -n ${channels[0]} ]] ||
    fail "expected one channel in $RUST_TOOLCHAIN_FILE"
  [[ ${channels[0]} != *[!a-zA-Z0-9._-]* ]] ||
    fail "invalid toolchain channel in $RUST_TOOLCHAIN_FILE"
  printf '%s' "${channels[0]}"
}

is_linux() { [[ $(uname -s) == Linux ]]; }

mold_arch() {
  case $(uname -m) in
    x86_64 | amd64) printf 'x86_64' ;;
    aarch64 | arm64) printf 'aarch64' ;;
    *) fail "mold 2.41.0 has no pinned archive for Linux architecture $(uname -m)" ;;
  esac
}

append_build_tools_to_github_path() {
  [[ ${GITHUB_PATH+x} == x ]] || return 0
  [[ -n $GITHUB_PATH ]] || fail 'GITHUB_PATH is set but empty; cannot persist the mold path'
  is_linux || return 0

  local path_entry
  path_entry=$(cd -- "$BUILD_TOOLS_PREFIX/bin" && pwd -P) ||
    fail "mold installation directory is unavailable: $BUILD_TOOLS_PREFIX/bin"
  [[ $path_entry != *$'\n'* && $path_entry != *$'\r'* ]] ||
    fail 'mold installation path contains a newline; refusing a GitHub Actions path record'
  [[ $GITHUB_PATH != *$'\n'* && $GITHUB_PATH != *$'\r'* ]] ||
    fail 'GITHUB_PATH contains a newline; refusing to write an ambiguous path record'

  printf '%s\n' "$path_entry" >> "$GITHUB_PATH" ||
    fail 'could not append the mold installation directory to GITHUB_PATH'
  note 'added the pinned mold directory to GITHUB_PATH for later workflow steps'
}
