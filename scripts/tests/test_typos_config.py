"""Tests the committed spelling configuration at the ``typos`` boundary.

The shared dictionary stopped excluding inline code spans, so identifiers and
quoted US spellings inside backticks now reach the checker and are recorded in
``typos.local.toml``. A recorded pattern that is too wide silently disables the
gate for a whole word, and a pattern that is too narrow leaves the gate red;
neither shows up in a diff of the generated file. This drives the pinned
``typos`` release against the committed ``typos.toml`` so both mistakes fail.
"""

from __future__ import annotations

import re
import shutil
import subprocess
import typing as typ
from pathlib import Path

import pytest

if typ.TYPE_CHECKING:
    from collections.abc import Sequence

REPOSITORY_ROOT = Path(__file__).resolve().parents[2]
TYPOS_CONFIG = REPOSITORY_ROOT / "typos.toml"
MAKEFILE = REPOSITORY_ROOT / "Makefile"


def _pinned_typos_version() -> str:
    """Read the ``typos`` pin from the Makefile so the test cannot drift."""
    match = re.search(
        r"^TYPOS_VERSION \?= (\S+)$", MAKEFILE.read_text(encoding="utf-8"), re.MULTILINE
    )
    if match is None:
        pytest.fail("TYPOS_VERSION is not pinned in the Makefile")
    return match.group(1)


def _run_typos(markdown: str, tmp_path: Path) -> subprocess.CompletedProcess[str]:
    """Check one Markdown fragment with the pinned tool and committed config."""
    page = tmp_path / "page.md"
    page.write_text(markdown, encoding="utf-8")
    argv: Sequence[str] = (
        "uv",
        "tool",
        "run",
        f"typos@{_pinned_typos_version()}",
        "--config",
        str(TYPOS_CONFIG),
        str(page),
    )
    return subprocess.run(  # noqa: S603 - pinned tool, fixed arguments
        argv, capture_output=True, text=True, check=False
    )


pytestmark = pytest.mark.skipif(
    shutil.which("uv") is None, reason="uv is not on PATH"
)


def test_recorded_backticked_terms_are_accepted(tmp_path: Path) -> None:
    """Both recorded terms pass in the backticked form the documentation uses.

    Parameters
    ----------
    tmp_path : Path
        Pytest-provided directory the Markdown fragment is written into, so the
        checker runs against a real file rather than standard input.
    """
    result = _run_typos(
        "Keep US spelling when used in an API, for example, `color`.\n"
        "Serialization requires a custom `serde_json::ser::Formatter` here.\n",
        tmp_path,
    )

    assert result.returncode == 0, result.stdout + result.stderr


def test_the_same_words_are_still_caught_in_prose(tmp_path: Path) -> None:
    """Dropping the backticks must not drop the finding.

    This is what separates a scoped exception from a blanket one: the recorded
    patterns exist for the quoted identifier, not for the word.

    Parameters
    ----------
    tmp_path : Path
        Pytest-provided directory the Markdown fragment is written into, so the
        checker runs against a real file rather than standard input.
    """
    result = _run_typos("The color of the ser is wrong.\n", tmp_path)

    assert result.returncode != 0
    combined = result.stdout + result.stderr
    assert "colour" in combined
    assert "`ser`" in combined
