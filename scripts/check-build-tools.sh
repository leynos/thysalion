#!/usr/bin/env bash
# Check that the repository's development build prerequisites are available.
#
# This is the fast preflight paired with install-build-tools.sh. It checks the
# selected pins and required Linux linker tools without starting a build.

set -euo pipefail

script_dir=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)
# shellcheck source=scripts/build-tools-common.sh
. "$script_dir/build-tools-common.sh"

check_linux_linker() {
  local mold_pin=$1 resolved output installed
  if ! is_linux; then
    note "clang and mold are Linux development prerequisites; skipping on $(uname -s)"
    return 0
  fi

  if ! resolved=$(command -v clang 2>/dev/null); then
    note 'clang was not found on PATH; install it with your operating-system package manager'
    note 'make install-build-tools supplies mold and the pinned Rust components'
    return 1
  fi
  if ! output=$(clang --version 2>&1); then
    note "clang at $resolved cannot report its version"
    note 'install or repair clang, then run make check-build-tools again'
    return 1
  fi
  note "clang at $resolved: ${output%%$'\n'*}"

  if ! resolved=$(command -v mold 2>/dev/null); then
    note "mold not found on PATH (pinned $mold_pin)"
    note 'install it with: make install-build-tools'
    return 1
  fi
  if ! output=$(mold --version 2>/dev/null); then
    note "mold at $resolved cannot report its version"
    note 'reinstall it with: make install-build-tools'
    return 1
  fi
  installed=$(printf '%s\n' "$output" | awk 'NR == 1 { print $2 }')
  if [[ $installed != "$mold_pin" ]]; then
    note "mold $installed at $resolved does not match the pin $mold_pin"
    note 'run make install-build-tools to match'
    return 1
  fi
  note "mold $installed at $resolved"
}

check_toolchain() {
  local toolchain=$1 installed installed_components host_output host component prefix status=0
  if ! command -v "$RUSTUP" >/dev/null 2>&1; then
    note "rustup '$RUSTUP' was not found on PATH"
    note 'install rustup from https://rustup.rs, then run make install-build-tools'
    return 1
  fi
  if ! installed=$("$RUSTUP" toolchain list 2>/dev/null); then
    note "could not list installed rustup toolchains with $RUSTUP"
    note 'run make install-build-tools, then make check-build-tools'
    return 1
  fi
  if ! host_output=$("$RUSTUP" show 2>/dev/null); then
    note "could not determine the rustup host triple with $RUSTUP show"
    note 'run make install-build-tools, then make check-build-tools'
    return 1
  fi
  host=$(printf '%s\n' "$host_output" | awk -F ': ' '$1 == "Default host" { print $2; exit }')
  if [[ -z $host || $host == *[!a-zA-Z0-9_-]* ]]; then
    note "could not read a valid rustup host triple from $RUSTUP show"
    note 'run make install-build-tools, then make check-build-tools'
    return 1
  fi
  if ! printf '%s\n' "$installed" | awk -v pin="$toolchain" -v host="$host" \
    '$1 == pin || $1 == pin "-" host { found = 1 } END { exit !found }'; then
    note "toolchain $toolchain is not installed"
    note 'install it with: make install-build-tools'
    return 1
  fi
  if ! installed_components=$("$RUSTUP" component list --toolchain "$toolchain" --installed 2>/dev/null); then
    note "could not inspect components for toolchain $toolchain"
    note 'run make install-build-tools, then make check-build-tools'
    return 1
  fi
  note "toolchain $toolchain is installed"

  for component in "${BUILD_TOOLS_COMPONENTS[@]}"; do
    prefix=${component%-preview}
    if ! printf '%s\n' "$installed_components" | grep -Eq "^${prefix}-"; then
      note "toolchain $toolchain is missing component $component"
      status=1
    fi
  done
  if [[ $status -ne 0 ]]; then
    note 'install missing components with: make install-build-tools'
    return 1
  fi
  note 'required components are installed: clippy, llvm-tools-preview, rustfmt, rust-analyzer'
}

main() {
  local status=0 mold_pin toolchain_pin
  mold_pin=$(mold_version) || return 1
  toolchain_pin=$(pinned_toolchain) || return 1
  check_linux_linker "$mold_pin" || status=1
  check_toolchain "$toolchain_pin" || status=1
  if [[ $status -ne 0 ]]; then
    note 'capability check failed; address the messages above before building'
  fi
  return "$status"
}

main "$@"
