"""Process contracts for the pinned build-tool installer."""

from __future__ import annotations

import hashlib
import os
import shutil
import subprocess
import tarfile
from dataclasses import dataclass
from io import BytesIO
from pathlib import Path

import pytest

INSTALLER = Path(__file__).resolve().parents[1] / "install-build-tools.sh"
MOLD_VERSION = "2.41.0"
TOOLCHAIN_PIN = "nightly-2026-05-28"
ARCHIVE_NAME = f"mold-{MOLD_VERSION}-x86_64-linux.tar.gz"
MOLD_CONTENT = b"test-only mold executable\n"


@dataclass(frozen=True)
class InstallerHarness:
    """Run the installer with fake external commands and isolated paths."""

    environment: dict[str, str]
    checksum_file: Path
    curl_arguments: Path
    staging_evidence: Path
    event_log: Path
    github_path: Path
    installed_prefix: Path

    def run(self, **overrides: str) -> subprocess.CompletedProcess[str]:
        environment = self.environment | overrides
        return subprocess.run(
            [str(INSTALLER)],
            check=False,
            capture_output=True,
            env=environment,
            text=True,
        )

    def staging_directory_existed(self) -> bool:
        """Check curl observed the staging directory for its output archive."""
        arguments = self.curl_arguments.read_text(encoding="utf-8").splitlines()
        archive_directory = Path(arguments[-2]).parent
        return self.staging_evidence.read_text(encoding="utf-8") == (
            f"{archive_directory}\n"
        )

    def work_directories(self) -> list[Path]:
        """List installer staging directories remaining in the isolated temporary root."""
        return list(Path(self.environment["TMPDIR"]).glob("thysalion-build-tools.*"))

    def events(self) -> list[str]:
        return self.event_log.read_text(encoding="utf-8").splitlines()


@pytest.fixture
def installer(tmp_path: Path) -> InstallerHarness:
    fake_bin = tmp_path / "bin"
    fake_bin.mkdir()
    _install_fake_commands(fake_bin)

    archive = tmp_path / ARCHIVE_NAME
    _write_archive(archive)
    checksum_file = tmp_path / "SHA256SUMS"
    _write_checksum(checksum_file, archive)

    mold_version_file = tmp_path / "mold-version"
    mold_version_file.write_text(f"{MOLD_VERSION}\n", encoding="utf-8")
    toolchain_file = tmp_path / "rust-toolchain.toml"
    toolchain_file.write_text(
        f'[toolchain]\nchannel = "{TOOLCHAIN_PIN}"\n', encoding="utf-8"
    )

    curl_arguments = tmp_path / "curl-arguments"
    staging_evidence = tmp_path / "staging-evidence"
    event_log = tmp_path / "events.log"
    event_log.touch()
    github_path = tmp_path / "github-path"
    github_path.touch()
    installed_prefix = tmp_path / "installed"
    installed_prefix.mkdir()

    real_tar = shutil.which("tar")
    assert real_tar is not None, "the host tar command is needed to make the fixture archive"
    environment = {
        "BUILD_TOOLS_PREFIX": str(installed_prefix),
        "CURL_ARGUMENT_LOG": str(curl_arguments),
        "STAGING_EVIDENCE_LOG": str(staging_evidence),
        "EVENT_LOG": str(event_log),
        "FAKE_ARCHIVE": str(archive),
        "GITHUB_PATH": str(github_path),
        "HOME": str(tmp_path / "home"),
        "MOLD_RELEASE_BASE_URL": "https://offline.invalid/releases",
        "MOLD_SHA256SUMS_FILE": str(checksum_file),
        "MOLD_VERSION_FILE": str(mold_version_file),
        "PATH": os.pathsep.join((str(fake_bin), os.defpath)),
        "REAL_TAR": real_tar,
        "RUST_TOOLCHAIN_FILE": str(toolchain_file),
        "RUSTUP": "rustup",
        "TMPDIR": str(tmp_path),
    }
    return InstallerHarness(
        environment=environment,
        checksum_file=checksum_file,
        curl_arguments=curl_arguments,
        staging_evidence=staging_evidence,
        event_log=event_log,
        github_path=github_path,
        installed_prefix=installed_prefix,
    )


def test_installer_downloads_verifies_extracts_and_provisions_the_pinned_tools(
    installer: InstallerHarness,
) -> None:
    result = installer.run()

    assert result.returncode == 0, result.stderr
    arguments = installer.curl_arguments.read_text(encoding="utf-8").splitlines()
    archive_path = Path(arguments[-2])
    assert archive_path.name == ARCHIVE_NAME
    assert archive_path.parent.parent == Path(installer.environment["TMPDIR"])
    assert archive_path.parent.name.startswith("thysalion-build-tools.")
    assert arguments == [
        "--fail",
        "--silent",
        "--show-error",
        "--location",
        "--connect-timeout",
        "15",
        "--speed-limit",
        "1024",
        "--speed-time",
        "60",
        "--output",
        str(archive_path),
        f"https://offline.invalid/releases/v{MOLD_VERSION}/{ARCHIVE_NAME}",
    ]
    assert installer.staging_directory_existed()
    assert not installer.work_directories()
    assert (installer.installed_prefix / "bin/mold").read_bytes() == MOLD_CONTENT
    assert installer.github_path.read_text(encoding="utf-8") == (
        f"{installer.installed_prefix / 'bin'}\n"
    )
    assert installer.events() == [
        "curl",
        "tar",
        f"rustup toolchain install {TOOLCHAIN_PIN} --profile minimal --no-self-update",
        "rustup component add --toolchain "
        f"{TOOLCHAIN_PIN} clippy llvm-tools-preview rustfmt rust-analyzer",
    ]


def test_installer_reports_download_failure_before_unpacking(
    installer: InstallerHarness,
) -> None:
    result = installer.run(FAKE_CURL_FAIL="1")

    assert result.returncode == 1
    assert installer.staging_directory_existed()
    assert not installer.work_directories()
    assert "failed to download " in result.stderr
    assert installer.events() == ["curl"]
    assert installer.github_path.read_text(encoding="utf-8") == ""


def test_installer_rejects_a_checksum_mismatch_before_unpacking(
    installer: InstallerHarness,
) -> None:
    installer.checksum_file.write_text(f"{'0' * 64}  {ARCHIVE_NAME}\n", encoding="utf-8")

    result = installer.run()

    assert result.returncode == 1
    assert installer.staging_directory_existed()
    assert not installer.work_directories()
    assert f"checksum mismatch for {ARCHIVE_NAME}" in result.stderr
    assert installer.events() == ["curl"]
    assert installer.github_path.read_text(encoding="utf-8") == ""


def test_installer_reports_extraction_failure_before_rustup(
    installer: InstallerHarness,
) -> None:
    result = installer.run(FAKE_TAR_FAIL="1")

    assert result.returncode == 1
    assert installer.staging_directory_existed()
    assert not installer.work_directories()
    assert "failed to unpack " in result.stderr
    assert installer.events() == ["curl", "tar"]
    assert installer.github_path.read_text(encoding="utf-8") == ""


def test_installer_stops_when_the_pinned_toolchain_install_fails(
    installer: InstallerHarness,
) -> None:
    result = installer.run(FAKE_RUSTUP_FAIL="toolchain")

    assert result.returncode == 1
    assert installer.staging_directory_existed()
    assert not installer.work_directories()
    assert f"failed to install toolchain {TOOLCHAIN_PIN}" in result.stderr
    assert installer.events() == [
        "curl",
        "tar",
        f"rustup toolchain install {TOOLCHAIN_PIN} --profile minimal --no-self-update",
    ]
    assert installer.github_path.read_text(encoding="utf-8") == ""


def test_installer_stops_when_a_required_component_cannot_be_added(
    installer: InstallerHarness,
) -> None:
    result = installer.run(FAKE_RUSTUP_FAIL="component")

    assert result.returncode == 1
    assert installer.staging_directory_existed()
    assert not installer.work_directories()
    assert f"failed to install required components for {TOOLCHAIN_PIN}" in result.stderr
    assert installer.events() == [
        "curl",
        "tar",
        f"rustup toolchain install {TOOLCHAIN_PIN} --profile minimal --no-self-update",
        "rustup component add --toolchain "
        f"{TOOLCHAIN_PIN} clippy llvm-tools-preview rustfmt rust-analyzer",
    ]
    assert installer.github_path.read_text(encoding="utf-8") == ""


def test_installer_propagates_a_github_path_append_failure(
    installer: InstallerHarness, tmp_path: Path
) -> None:
    github_path_directory = tmp_path / "github-path-directory"
    github_path_directory.mkdir()

    result = installer.run(GITHUB_PATH=str(github_path_directory))

    assert result.returncode == 1
    assert installer.staging_directory_existed()
    assert not installer.work_directories()
    assert "could not append the mold installation directory to GITHUB_PATH" in result.stderr
    assert installer.events() == [
        "curl",
        "tar",
        f"rustup toolchain install {TOOLCHAIN_PIN} --profile minimal --no-self-update",
        "rustup component add --toolchain "
        f"{TOOLCHAIN_PIN} clippy llvm-tools-preview rustfmt rust-analyzer",
    ]
    assert not installer.github_path.read_text(encoding="utf-8")


def _install_fake_commands(fake_bin: Path) -> None:
    """Create the offline commands used by installer subprocess tests."""
    _write_executable(
        fake_bin / "uname",
        """#!/usr/bin/env bash
set -euo pipefail
case "${1:-}" in
  -s) printf 'Linux\\n' ;;
  -m) printf 'x86_64\\n' ;;
  *) exit 2 ;;
esac
""",
    )
    _write_executable(
        fake_bin / "curl",
        """#!/usr/bin/env bash
set -euo pipefail
printf 'curl\\n' >> "$EVENT_LOG"
printf '%s\\n' "$@" > "$CURL_ARGUMENT_LOG"
output=
while (($#)); do
  if [[ $1 == --output ]]; then
    output=$2
    shift 2
  else
    shift
  fi
done
[[ -n $output ]]
if [[ -d ${output%/*} ]]; then
  printf '%s\\n' "${output%/*}" > "$STAGING_EVIDENCE_LOG"
fi
if [[ ${FAKE_CURL_FAIL:-0} == 1 ]]; then exit 22; fi
cp -- "$FAKE_ARCHIVE" "$output"
""",
    )
    _write_executable(
        fake_bin / "tar",
        """#!/usr/bin/env bash
set -euo pipefail
printf 'tar\\n' >> "$EVENT_LOG"
if [[ ${FAKE_TAR_FAIL:-0} == 1 ]]; then exit 2; fi
exec "$REAL_TAR" "$@"
""",
    )
    _write_executable(
        fake_bin / "rustup",
        """#!/usr/bin/env bash
set -euo pipefail
printf 'rustup %s\\n' "$*" >> "$EVENT_LOG"
if [[ ${FAKE_RUSTUP_FAIL:-} == "${1:-}" ]]; then exit 17; fi
""",
    )


def _write_archive(archive: Path) -> None:
    metadata = tarfile.TarInfo("mold/bin/mold")
    metadata.size = len(MOLD_CONTENT)
    with tarfile.open(archive, mode="w:gz") as mold_archive:
        mold_archive.addfile(metadata, BytesIO(MOLD_CONTENT))


def _write_checksum(checksum_file: Path, archive: Path) -> None:
    digest = hashlib.sha256(archive.read_bytes()).hexdigest()
    checksum_file.write_text(f"{digest}  {ARCHIVE_NAME}\n", encoding="utf-8")


def _write_executable(path: Path, source: str) -> None:
    path.write_text(source, encoding="utf-8")
    path.chmod(0o755)
