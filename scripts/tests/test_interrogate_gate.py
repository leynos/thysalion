"""Behaviour tests for the ``make interrogate`` docstring-coverage gate.

The tests replace the Interrogate command with a stub script, so they check
what the target does with a report and an exit status rather than the wording
of the recipe. A gap hidden by a rounded percentage, a tool that fails, and a
clean report each have their own case, and a dry run of ``make lint`` shows the
gate is a prerequisite of the lint target.
"""

from __future__ import annotations

import os
import shutil
import stat
import subprocess
from pathlib import Path

import pytest

REPOSITORY_ROOT = Path(__file__).resolve().parents[2]
MAKE = shutil.which("make")

pytestmark = pytest.mark.skipif(MAKE is None, reason="make is not installed")

CLEAN_REPORT = (
    "| scripts/build.py | COVERED |\nRESULT: PASSED (minimum: 100.0%, actual: 100.0%)\n"
)
MISSED_REPORT = (
    "| scripts/build.py |  MISSED |\nRESULT: PASSED (minimum: 100.0%, actual: 100.0%)\n"
)


def write_stub(directory: Path, report: str, exit_code: int) -> Path:
    """Write an executable stand-in that prints ``report`` and exits ``exit_code``."""
    report_file = directory / "report.txt"
    report_file.write_text(report, encoding="utf-8")
    script = directory / "interrogate-stub"
    script.write_text(
        f'#!/bin/sh\ncat "{report_file}"\nexit {exit_code}\n', encoding="utf-8"
    )
    script.chmod(script.stat().st_mode | stat.S_IXUSR)
    return script


def run_gate(stub: Path) -> subprocess.CompletedProcess[str]:
    """Run ``make interrogate`` with the Interrogate command replaced by ``stub``."""
    assert MAKE is not None  # narrowed by the module skip marker
    return subprocess.run(
        [MAKE, "-s", "-C", str(REPOSITORY_ROOT), "interrogate", f"INTERROGATE={stub}"],
        capture_output=True,
        text=True,
        check=False,
    )


def test_clean_report_passes(tmp_path: Path) -> None:
    """A fully documented report with a zero exit status passes the gate."""
    result = run_gate(write_stub(tmp_path, CLEAN_REPORT, 0))

    assert result.returncode == 0, result.stderr


def test_missed_row_fails_despite_successful_exit(tmp_path: Path) -> None:
    """A MISSED row fails the gate even when the tool itself exits zero."""
    result = run_gate(write_stub(tmp_path, MISSED_REPORT, 0))

    assert result.returncode != 0
    assert "undocumented definitions remain" in result.stderr


def test_tool_failure_status_propagates(tmp_path: Path) -> None:
    """A non-zero tool exit fails the gate and make reports that status."""
    result = run_gate(write_stub(tmp_path, CLEAN_REPORT, 3))

    assert result.returncode != 0
    assert "Error 3" in result.stderr


def test_long_report_with_early_missed_row_fails_cleanly(tmp_path: Path) -> None:
    """A MISSED row ahead of a long report fails the gate with its own message."""
    padding = "| scripts/build.py | COVERED |\n" * 50_000
    result = run_gate(write_stub(tmp_path, MISSED_REPORT + padding, 0))

    assert result.returncode != 0
    assert "undocumented definitions remain" in result.stderr


def test_long_clean_report_passes(tmp_path: Path) -> None:
    """A long report with no MISSED row passes the gate."""
    padding = "| scripts/build.py | COVERED |\n" * 50_000
    result = run_gate(write_stub(tmp_path, padding + CLEAN_REPORT, 0))

    assert result.returncode == 0, result.stderr


def test_report_scanner_failure_fails_the_gate(tmp_path: Path) -> None:
    """A scanner error other than 'no match' fails the gate, not a clean pass."""
    stub_directory = tmp_path / "bin"
    stub_directory.mkdir()
    failing_grep = stub_directory / "grep"
    failing_grep.write_text("#!/bin/sh\nexit 2\n", encoding="utf-8")
    failing_grep.chmod(failing_grep.stat().st_mode | stat.S_IXUSR)
    environment = {**os.environ, "PATH": f"{stub_directory}{os.pathsep}{os.environ['PATH']}"}

    assert MAKE is not None  # narrowed by the module skip marker
    stub = write_stub(tmp_path, CLEAN_REPORT, 0)
    result = subprocess.run(
        [MAKE, "-s", "-C", str(REPOSITORY_ROOT), "interrogate", f"INTERROGATE={stub}"],
        capture_output=True,
        text=True,
        check=False,
        env=environment,
    )

    assert result.returncode != 0
    assert "MISSED check failed" in result.stderr


def test_lint_runs_the_gate_first() -> None:
    """A dry run of ``make lint`` reaches the interrogate recipe before clippy."""
    assert MAKE is not None  # narrowed by the module skip marker
    marker = "interrogate-gate-marker"
    result = subprocess.run(
        [MAKE, "-n", "-C", str(REPOSITORY_ROOT), "lint", f"INTERROGATE={marker}"],
        capture_output=True,
        text=True,
        check=False,
    )

    assert result.returncode == 0, result.stderr
    assert marker in result.stdout
    assert result.stdout.index(marker) < result.stdout.index("clippy")
