"""Contracts for local Markdown tool provisioning and Make invocation."""

from __future__ import annotations

import os
import re
import shutil
import subprocess
from pathlib import Path

import pytest

REPOSITORY_ROOT = Path(__file__).resolve().parents[2]


def has_pinned_setup_bun(step: str) -> bool:
    """Accept the setup action at any full commit SHA Dependabot selects."""
    actions = [
        line.strip()
        for line in step.splitlines()
        if line.strip().startswith("uses: oven-sh/setup-bun@")
    ]
    return len(actions) == 1 and re.fullmatch(
        r"uses: oven-sh/setup-bun@[0-9a-f]{40}", actions[0]
    ) is not None


def run_make(
    *arguments: str,
    environment: dict[str, str] | None = None,
) -> subprocess.CompletedProcess[str]:
    """Run one Make contract with captured output."""
    return subprocess.run(
        ["make", "--no-print-directory", "CARGO=probe-cargo", *arguments],
        cwd=REPOSITORY_ROOT,
        check=False,
        capture_output=True,
        env=environment,
        text=True,
    )


def test_local_installer_uses_the_exact_ci_package_version() -> None:
    result = run_make("--dry-run", "install-markdownlint", "BUN=probe-bun")

    assert result.returncode == 0, result.stderr
    assert "probe-bun add --global --exact markdownlint-cli2@0.22.1" in result.stdout


def test_markdownlint_commands_keep_the_mdlint_override() -> None:
    formatter = run_make(
        "--dry-run", "fmt", "MDLINT=probe-markdownlint"
    )
    linter = run_make(
        "--dry-run",
        "markdownlint",
        "MDLINT=probe-markdownlint",
        "TYPOS_CONFIG_BUILDER=probe-typos-config-builder",
    )

    assert formatter.returncode == 0, formatter.stderr
    assert "probe-markdownlint --fix \"**/*.md\"" in formatter.stdout
    assert linter.returncode == 0, linter.stderr
    assert "probe-markdownlint '**/*.md'" in linter.stdout


def test_missing_bun_has_an_actionable_install_error() -> None:
    result = run_make(
        "install-markdownlint",
        "BUN=missing-bun-for-markdownlint-contract",
    )

    assert result.returncode != 0
    assert "Bun is required; install Bun" in result.stderr
    assert "make install-markdownlint" in result.stderr


def test_markdownlint_failure_stops_before_spelling(tmp_path: Path) -> None:
    failing_linter = tmp_path / "failing-markdownlint"
    failing_linter.write_text(
        "#!/bin/sh\nprintf 'controlled markdownlint failure\\n' >&2\nexit 23\n",
        encoding="utf-8",
    )
    failing_linter.chmod(0o755)

    spelling_marker = tmp_path / "spelling-ran"
    spelling_probe = tmp_path / "spelling-probe"
    spelling_probe.write_text(
        "#!/bin/sh\n: > \"$SPELLING_MARKER\"\nexit 0\n",
        encoding="utf-8",
    )
    spelling_probe.chmod(0o755)

    environment = os.environ.copy()
    environment["SPELLING_MARKER"] = str(spelling_marker)
    result = run_make(
        "markdownlint",
        f"MDLINT={failing_linter}",
        f"TYPOS_CONFIG_BUILDER={spelling_probe}",
        environment=environment,
    )

    assert result.returncode != 0
    assert "controlled markdownlint failure" in result.stderr
    assert not spelling_marker.exists()


def test_markdownlint_ignores_generated_pytest_cache(tmp_path: Path) -> None:
    config = REPOSITORY_ROOT / ".markdownlint-cli2.jsonc"
    assert '"**/.pytest_cache/**"' in config.read_text(encoding="utf-8")

    linter = shutil.which("markdownlint-cli2")
    if linter is None:
        if os.environ.get("REQUIRE_MARKDOWNLINT_CLI") == "true":
            pytest.fail("CI must install markdownlint-cli2 before scripts-test")
        pytest.skip("run make install-markdownlint to check ignore behaviour")

    shutil.copyfile(config, tmp_path / config.name)
    for relative in [
        Path(".pytest_cache/invalid.md"),
        Path("nested/.pytest_cache/invalid.md"),
    ]:
        ignored = tmp_path / relative
        ignored.parent.mkdir(parents=True, exist_ok=True)
        ignored.write_text("not a heading\n", encoding="utf-8")
        result = subprocess.run(
            [linter, relative.as_posix()],
            cwd=tmp_path,
            check=False,
            capture_output=True,
            text=True,
        )
        assert result.returncode == 0, result.stdout + result.stderr
        assert "markdownlint-cli2 v0.22.1" in result.stdout
        assert "Linting: 0 file(s)" in result.stdout

    ordinary = tmp_path / "ordinary/invalid.md"
    ordinary.parent.mkdir(parents=True, exist_ok=True)
    ordinary.write_text("not a heading\n", encoding="utf-8")
    result = subprocess.run(
        [linter, "ordinary/invalid.md"],
        cwd=tmp_path,
        check=False,
        capture_output=True,
        text=True,
    )
    assert result.returncode != 0
    assert "MD041" in result.stdout + result.stderr


def test_ci_provisions_markdownlint_before_python_contracts() -> None:
    workflow = (REPOSITORY_ROOT / ".github/workflows/ci.yml").read_text(encoding="utf-8")
    setup = workflow.index("      - name: Setup Bun for Markdown contracts\n")
    install = workflow.index("      - name: Install Markdown test linter\n")
    scripts = workflow.index("      - name: Test the Python helper scripts\n")

    assert setup < install < scripts
    assert has_pinned_setup_bun(workflow[setup:install])
    assert "bun-version: '1.3.14'" in workflow[setup:install]
    assert "run: make install-markdownlint" in workflow[install:scripts]
    for step in [workflow[setup:install], workflow[install:scripts]]:
        assert "\n        if:" not in step
        assert "continue-on-error:" not in step
    script_step = workflow[scripts:].split("\n      - name: ", maxsplit=1)[0]
    assert "REQUIRE_MARKDOWNLINT_CLI: 'true'" in script_step
    assert "run: make scripts-test" in script_step


def test_setup_bun_contract_accepts_commit_bumps_but_rejects_mutable_refs() -> None:
    assert has_pinned_setup_bun("uses: oven-sh/setup-bun@" + "a" * 40)
    assert not has_pinned_setup_bun("uses: oven-sh/setup-bun@v2")
    assert not has_pinned_setup_bun("uses: oven-sh/setup-bun@" + "a" * 39)


def test_ci_mode_refuses_missing_markdownlint_cli(
    monkeypatch: pytest.MonkeyPatch, tmp_path: Path
) -> None:
    monkeypatch.setenv("REQUIRE_MARKDOWNLINT_CLI", "true")
    monkeypatch.setattr(shutil, "which", lambda _: None)

    with pytest.raises(pytest.fail.Exception, match="CI must install markdownlint-cli2"):
        test_markdownlint_ignores_generated_pytest_cache(tmp_path)
