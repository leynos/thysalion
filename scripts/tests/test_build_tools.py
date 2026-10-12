"""Contracts for the pinned build-tool checker."""

from __future__ import annotations

import os
import re
import shutil
import subprocess
from dataclasses import dataclass
from pathlib import Path

import pytest

CHECKER = Path(__file__).resolve().parents[1] / "check-build-tools.sh"
TOOLCHAIN_PIN = "stable-1.98.1"
HOST_TRIPLE = "x86_64-unknown-linux-gnu"


@dataclass(frozen=True)
class CheckerHarness:
    """Run the checker with isolated pins and controlled external commands."""

    environment: dict[str, str]
    fake_bin: Path
    mold_pin: Path
    toolchain_pin: Path

    def run(self, **overrides: str) -> subprocess.CompletedProcess[str]:
        """Execute the real checker with only the scenario's fake tool output."""
        return subprocess.run(
            [str(CHECKER)],
            check=False,
            capture_output=True,
            env=self.environment | overrides,
            text=True,
        )


@pytest.fixture
def check_build_tools(tmp_path: Path) -> CheckerHarness:
    """Provide the checker's utility commands without ambient clang or mold."""
    fake_bin = tmp_path / "bin"
    fake_bin.mkdir()
    for command in ("bash", "dirname", "awk", "grep"):
        resolved = shutil.which(command)
        assert resolved is not None, f"the fixture needs the host {command} utility"
        (fake_bin / command).symlink_to(resolved)

    toolchain_file = tmp_path / "rust-toolchain.toml"
    toolchain_file.write_text(
        f'[toolchain]\nchannel = "{TOOLCHAIN_PIN}"\n', encoding="utf-8"
    )
    mold_pin = tmp_path / "mold-version"
    mold_pin.write_text("2.41.0\n", encoding="utf-8")

    commands = {
        "rustup": """#!/usr/bin/env bash
set -euo pipefail
case "$1" in
  show)
    printf '%s\\n' "$FAKE_HOST_OUTPUT"
    exit "$FAKE_HOST_STATUS"
    ;;
  toolchain)
    [[ ${2:-} == list ]]
    printf '%s\\n' "$FAKE_TOOLCHAIN_ROW"
    exit "$FAKE_TOOLCHAIN_STATUS"
    ;;
  component)
    printf '%s\\n' "$FAKE_COMPONENTS"
    exit "$FAKE_COMPONENT_STATUS"
    ;;
  *) exit 2 ;;
esac
""",
        "uname": '#!/usr/bin/env bash\nprintf "%s\\n" "$FAKE_SYSTEM"\n',
        "clang": (
            '#!/usr/bin/env bash\nprintf "clang version 20.0.0\\n"\n'
            'exit "$FAKE_CLANG_STATUS"\n'
        ),
        "mold": (
            '#!/usr/bin/env bash\nprintf "mold %s\\n" "$FAKE_MOLD_VERSION"\n'
            'exit "$FAKE_MOLD_STATUS"\n'
        ),
    }
    for name, contents in commands.items():
        executable = fake_bin / name
        executable.write_text(contents, encoding="utf-8")
        executable.chmod(0o755)

    environment = os.environ.copy()
    environment.update(
        {
            "FAKE_SYSTEM": "Linux",
            "FAKE_CLANG_STATUS": "0",
            "FAKE_MOLD_STATUS": "0",
            "FAKE_MOLD_VERSION": "2.41.0",
            "FAKE_HOST_OUTPUT": f"Default host: {HOST_TRIPLE}",
            "FAKE_HOST_STATUS": "0",
            "FAKE_TOOLCHAIN_ROW": f"{TOOLCHAIN_PIN}-{HOST_TRIPLE} (installed)",
            "FAKE_TOOLCHAIN_STATUS": "0",
            "FAKE_COMPONENTS": "\n".join(
                f"{component}-{HOST_TRIPLE}"
                for component in ("clippy", "llvm-tools", "rustfmt", "rust-analyzer")
            ),
            "FAKE_COMPONENT_STATUS": "0",
            "PATH": str(fake_bin),
            "RUSTUP": str(fake_bin / "rustup"),
            "RUST_TOOLCHAIN_FILE": str(toolchain_file),
            "MOLD_VERSION_FILE": str(mold_pin),
        }
    )
    return CheckerHarness(environment, fake_bin, mold_pin, toolchain_file)


def test_dotted_pin_rejects_near_match_that_the_old_regex_accepts(
    check_build_tools: CheckerHarness,
) -> None:
    """Reject a near-match channel that dots in the old regex admitted."""
    near_match = f"stable-1x98.1-{HOST_TRIPLE}"
    assert re.match(r"^stable-1.98.1(-|$)", near_match)

    result = check_build_tools.run(FAKE_TOOLCHAIN_ROW=f"{near_match} (installed)")

    assert result.returncode == 1
    assert f"toolchain {TOOLCHAIN_PIN} is not installed" in result.stderr


def test_dotted_pin_accepts_its_exact_host_qualified_name(
    check_build_tools: CheckerHarness,
) -> None:
    """Accept the pinned channel when rustup qualifies it with the host."""
    result = check_build_tools.run(
        FAKE_TOOLCHAIN_ROW=f"{TOOLCHAIN_PIN}-{HOST_TRIPLE} (installed)"
    )

    assert result.returncode == 0, result.stderr
    assert f"toolchain {TOOLCHAIN_PIN} is installed" in result.stderr


def test_dotted_pin_accepts_its_exact_bare_name(
    check_build_tools: CheckerHarness,
) -> None:
    """Accept the pinned channel without a host suffix."""
    result = check_build_tools.run(FAKE_TOOLCHAIN_ROW=f"{TOOLCHAIN_PIN} (installed)")

    assert result.returncode == 0, result.stderr
    assert f"toolchain {TOOLCHAIN_PIN} is installed" in result.stderr


def test_dotted_pin_rejects_an_unrelated_suffix(
    check_build_tools: CheckerHarness,
) -> None:
    """Reject an installed channel with an unrelated suffix."""
    result = check_build_tools.run(
        FAKE_TOOLCHAIN_ROW=f"{TOOLCHAIN_PIN}-custom-suffix (installed)"
    )

    assert result.returncode == 1
    assert f"toolchain {TOOLCHAIN_PIN} is not installed" in result.stderr


def test_linux_checker_accepts_all_pinned_prerequisites(
    check_build_tools: CheckerHarness,
) -> None:
    """Accept a Linux installation with the pinned tools and components."""
    result = check_build_tools.run()

    assert result.returncode == 0, result.stderr
    assert "clang version 20.0.0" in result.stderr
    assert "mold 2.41.0" in result.stderr
    assert "required components are installed" in result.stderr
    assert "skipping" not in result.stderr


def test_non_linux_checker_skips_unavailable_linker_tools(
    check_build_tools: CheckerHarness,
) -> None:
    """Allow non-Linux hosts to omit the Linux linker prerequisites."""
    (check_build_tools.fake_bin / "clang").unlink()
    (check_build_tools.fake_bin / "mold").unlink()

    result = check_build_tools.run(FAKE_SYSTEM="Darwin")

    assert result.returncode == 0, result.stderr
    assert "skipping on Darwin" in result.stderr
    assert "required components are installed" in result.stderr


@pytest.mark.parametrize(
    ("command", "diagnostic"),
    [
        pytest.param("clang", "clang was not found on PATH", id="missing-clang"),
        pytest.param("mold", "mold not found on PATH (pinned 2.41.0)", id="missing-mold"),
        pytest.param("rustup", "was not found on PATH", id="missing-rustup"),
    ],
)
def test_checker_rejects_missing_tools(
    check_build_tools: CheckerHarness, command: str, diagnostic: str
) -> None:
    """Report the unavailable tool instead of finding ambient host tooling."""
    (check_build_tools.fake_bin / command).unlink()

    result = check_build_tools.run()

    assert result.returncode == 1
    assert diagnostic in result.stderr
    assert "capability check failed" in result.stderr


@pytest.mark.parametrize("command", ["clang", "mold"])
def test_checker_rejects_failed_linker_version_commands(
    check_build_tools: CheckerHarness, command: str
) -> None:
    """Reject an executable that fails to report its linker version."""
    result = check_build_tools.run(**{f"FAKE_{command.upper()}_STATUS": "7"})

    assert result.returncode == 1
    assert f"{command} at {check_build_tools.fake_bin / command}" in result.stderr
    assert "cannot report its version" in result.stderr
    assert "capability check failed" in result.stderr


def test_checker_rejects_mold_version_mismatching_its_pin(
    check_build_tools: CheckerHarness,
) -> None:
    """Explain how to repair a mold installation at the wrong version."""
    result = check_build_tools.run(FAKE_MOLD_VERSION="2.40.0")

    assert result.returncode == 1
    assert "mold 2.40.0" in result.stderr
    assert "does not match the pin 2.41.0" in result.stderr
    assert "run make install-build-tools to match" in result.stderr


@pytest.mark.parametrize(
    ("status_variable", "diagnostic"),
    [
        pytest.param(
            "FAKE_TOOLCHAIN_STATUS",
            "could not list installed rustup toolchains",
            id="toolchain-list-fails",
        ),
        pytest.param(
            "FAKE_HOST_STATUS",
            "could not determine the rustup host triple",
            id="host-query-fails",
        ),
        pytest.param(
            "FAKE_COMPONENT_STATUS",
            "could not inspect components for toolchain",
            id="component-list-fails",
        ),
    ],
)
def test_checker_rejects_failed_rustup_queries(
    check_build_tools: CheckerHarness, status_variable: str, diagnostic: str
) -> None:
    """Treat failed rustup queries as actionable preflight failures."""
    result = check_build_tools.run(**{status_variable: "7"})

    assert result.returncode == 1
    assert diagnostic in result.stderr
    assert "make install-build-tools" in result.stderr
    assert "capability check failed" in result.stderr


@pytest.mark.parametrize(
    "host_output",
    [
        pytest.param("", id="missing-host"),
        pytest.param("Default host: ", id="empty-host"),
        pytest.param("Default host: invalid host!", id="malformed-host"),
    ],
)
def test_checker_rejects_missing_or_invalid_host_output(
    check_build_tools: CheckerHarness, host_output: str
) -> None:
    """Reject rustup output that does not identify a valid host triple."""
    result = check_build_tools.run(FAKE_HOST_OUTPUT=host_output)

    assert result.returncode == 1
    assert "could not read a valid rustup host triple" in result.stderr


@pytest.mark.parametrize(
    ("installed_name", "required_name"),
    [
        ("clippy", "clippy"),
        ("llvm-tools", "llvm-tools-preview"),
        ("rustfmt", "rustfmt"),
        ("rust-analyzer", "rust-analyzer"),
    ],
)
def test_checker_rejects_each_missing_required_component(
    check_build_tools: CheckerHarness, installed_name: str, required_name: str
) -> None:
    """Identify each required Rust component omitted from the installation."""
    components = check_build_tools.environment["FAKE_COMPONENTS"].splitlines()
    components.remove(f"{installed_name}-{HOST_TRIPLE}")

    result = check_build_tools.run(FAKE_COMPONENTS="\n".join(components))

    assert result.returncode == 1
    assert f"missing component {required_name}" in result.stderr
    assert "install missing components with: make install-build-tools" in result.stderr


def test_checker_rejects_invalid_component_listing(
    check_build_tools: CheckerHarness,
) -> None:
    """Reject unrecognized component output rather than accepting an empty match."""
    result = check_build_tools.run(FAKE_COMPONENTS="unrecognized-component-output")

    assert result.returncode == 1
    for component in ("clippy", "llvm-tools-preview", "rustfmt", "rust-analyzer"):
        assert f"missing component {component}" in result.stderr
    assert "required components are installed" not in result.stderr


@pytest.mark.parametrize(
    ("contents", "diagnostic"),
    [
        pytest.param(None, "missing version pin", id="missing-file"),
        pytest.param("", "expected one version line", id="no-lines"),
        pytest.param("\n", "empty version pin", id="empty-line"),
        pytest.param("2.41.0\n2.40.0\n", "expected one version line", id="multiple-lines"),
        pytest.param("2.41.0 extra\n", "version pin contains whitespace", id="whitespace"),
    ],
)
def test_checker_rejects_missing_or_malformed_mold_pin(
    check_build_tools: CheckerHarness, contents: str | None, diagnostic: str
) -> None:
    """Reject unusable mold pins and identify the offending pin file."""
    if contents is None:
        check_build_tools.mold_pin.unlink()
    else:
        check_build_tools.mold_pin.write_text(contents, encoding="utf-8")

    result = check_build_tools.run()

    assert result.returncode == 1
    assert diagnostic in result.stderr
    assert str(check_build_tools.mold_pin) in result.stderr


@pytest.mark.parametrize(
    ("contents", "diagnostic"),
    [
        pytest.param(None, "missing toolchain pin", id="missing-file"),
        pytest.param("[toolchain]\n", "expected one channel", id="missing-channel"),
        pytest.param('channel = ""\n', "expected one channel", id="empty-channel"),
        pytest.param(
            'channel = "stable"\nchannel = "nightly"\n',
            "expected one channel",
            id="multiple-channels",
        ),
        pytest.param(
            'channel = "invalid channel!"\n',
            "invalid toolchain channel",
            id="invalid-channel",
        ),
    ],
)
def test_checker_rejects_missing_or_malformed_toolchain_pin(
    check_build_tools: CheckerHarness, contents: str | None, diagnostic: str
) -> None:
    """Reject unusable toolchain pins and identify the offending pin file."""
    if contents is None:
        check_build_tools.toolchain_pin.unlink()
    else:
        check_build_tools.toolchain_pin.write_text(contents, encoding="utf-8")

    result = check_build_tools.run()

    assert result.returncode == 1
    assert diagnostic in result.stderr
    assert str(check_build_tools.toolchain_pin) in result.stderr
