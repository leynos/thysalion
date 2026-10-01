"""Contracts for the pinned build-tool checker."""

from __future__ import annotations

import os
import re
import subprocess
from collections.abc import Callable
from pathlib import Path

import pytest

CHECKER = Path(__file__).resolve().parents[1] / "check-build-tools.sh"
TOOLCHAIN_PIN = "stable-1.98.1"
HOST_TRIPLE = "x86_64-unknown-linux-gnu"


@pytest.fixture
def check_build_tools(tmp_path: Path) -> Callable[[str], subprocess.CompletedProcess[str]]:
    """Runs the checker with controlled rustup output and a temporary pin."""
    fake_bin = tmp_path / "bin"
    fake_bin.mkdir()

    toolchain_file = tmp_path / "rust-toolchain.toml"
    toolchain_file.write_text(
        f'[toolchain]\nchannel = "{TOOLCHAIN_PIN}"\n', encoding="utf-8"
    )

    fake_rustup = tmp_path / "rustup"
    fake_rustup.write_text(
        """#!/usr/bin/env bash
set -euo pipefail
case "$1" in
  show) printf 'Default host: %s\\n' "$FAKE_RUSTUP_HOST" ;;
  toolchain)
    [[ ${2:-} == list ]]
    printf '%s\\n' "$FAKE_TOOLCHAIN_ROW"
    ;;
  component)
    printf 'clippy-x86_64-unknown-linux-gnu\\n'
    printf 'llvm-tools-x86_64-unknown-linux-gnu\\n'
    printf 'rustfmt-x86_64-unknown-linux-gnu\\n'
    printf 'rust-analyzer-x86_64-unknown-linux-gnu\\n'
    ;;
  *) exit 2 ;;
esac
""",
        encoding="utf-8",
    )
    fake_rustup.chmod(0o755)

    fake_uname = fake_bin / "uname"
    fake_uname.write_text(
        "#!/usr/bin/env bash\nprintf 'Darwin\\n'\n",
        encoding="utf-8",
    )
    fake_uname.chmod(0o755)

    def run(toolchain_row: str) -> subprocess.CompletedProcess[str]:
        environment = os.environ.copy()
        environment.update(
            {
                "FAKE_RUSTUP_HOST": HOST_TRIPLE,
                "FAKE_TOOLCHAIN_ROW": toolchain_row,
                "PATH": os.pathsep.join((str(fake_bin), environment.get("PATH", os.defpath))),
                "RUSTUP": str(fake_rustup),
                "RUST_TOOLCHAIN_FILE": str(toolchain_file),
            }
        )
        return subprocess.run(
            [str(CHECKER)],
            check=False,
            capture_output=True,
            env=environment,
            text=True,
        )

    return run


def test_dotted_pin_rejects_near_match_that_the_old_regex_accepts(
    check_build_tools: Callable[[str], subprocess.CompletedProcess[str]],
) -> None:
    near_match = f"stable-1x98.1-{HOST_TRIPLE}"
    assert re.match(r"^stable-1.98.1(-|$)", near_match)

    result = check_build_tools(f"{near_match} (installed)")

    assert result.returncode == 1
    assert f"toolchain {TOOLCHAIN_PIN} is not installed" in result.stderr


def test_dotted_pin_accepts_its_exact_host_qualified_name(
    check_build_tools: Callable[[str], subprocess.CompletedProcess[str]],
) -> None:
    result = check_build_tools(f"{TOOLCHAIN_PIN}-{HOST_TRIPLE} (installed)")

    assert result.returncode == 0, result.stderr
    assert f"toolchain {TOOLCHAIN_PIN} is installed" in result.stderr


def test_dotted_pin_accepts_its_exact_bare_name(
    check_build_tools: Callable[[str], subprocess.CompletedProcess[str]],
) -> None:
    result = check_build_tools(f"{TOOLCHAIN_PIN} (installed)")

    assert result.returncode == 0, result.stderr
    assert f"toolchain {TOOLCHAIN_PIN} is installed" in result.stderr


def test_dotted_pin_rejects_an_unrelated_suffix(
    check_build_tools: Callable[[str], subprocess.CompletedProcess[str]],
) -> None:
    result = check_build_tools(f"{TOOLCHAIN_PIN}-custom-suffix (installed)")

    assert result.returncode == 1
    assert f"toolchain {TOOLCHAIN_PIN} is not installed" in result.stderr
