//! Contract for the wiring of the shared CV-005 contract check.
//!
//! The CV-005 clauses live in `leynos/shared-actions` (`cv005-contracts`) and
//! the library's own suite proves them. What the library cannot prove is that
//! this repository calls it: that the Makefile names a full commit, runs
//! `check --repository .` under the Python the library needs, that
//! `.github/cv005.toml` names this repository, that `make all` includes the
//! target, and that CI runs it. The commands are read from `make -n` and the
//! workflow from its committed text, so removing or misspelling any of them
//! fails a test here.

use std::process::Command;

/// The Makefile, as committed.
const MAKEFILE: &str = include_str!("../Makefile");

/// The checker's parameters, as committed.
const CONFIG: &str = include_str!("../.github/cv005.toml");

/// The merge-gate workflow, as committed.
const CI: &str = include_str!("../.github/workflows/ci.yml");

/// The repository the parameters must name.
const REPOSITORY: &str = "leynos/thysalion";

/// The target that runs the shared checker.
const TARGET: &str = "test-workflow-contracts";

/// The result of one contract check; the error says what is wrong.
type Check = Result<(), String>;

/// Returns the commands `make -n TARGET` would run.
///
/// # Examples
///
/// `make_dry_run(TARGET)` returns the `uv tool run` line that runs
/// `cv005-contracts check --repository .`.
fn make_dry_run(target: &str) -> Result<String, String> {
    let output = Command::new("make")
        .args(["-n", target])
        .current_dir(env!("CARGO_MANIFEST_DIR"))
        .output()
        .map_err(|error| format!("run make -n {target}: {error}"))?;
    if !output.status.success() {
        return Err(format!("make -n {target} failed: {output:?}"));
    }
    String::from_utf8(output.stdout).map_err(|error| format!("make -n prints UTF-8: {error}"))
}

/// Returns the commit `CV005_CONTRACTS_REF` names in the Makefile.
///
/// # Examples
///
/// With `CV005_CONTRACTS_REF ?= 8897779...` in the Makefile, this returns the
/// forty-character hash, which the tests compare with the checker's source.
fn pinned_commit() -> Result<&'static str, String> {
    MAKEFILE
        .lines()
        .find_map(|line| line.strip_prefix("CV005_CONTRACTS_REF ?= "))
        .map(str::trim)
        .ok_or_else(|| "the Makefile must set CV005_CONTRACTS_REF".to_owned())
}

/// Returns the one checker invocation in a dry run.
fn checker_run(commands: &str) -> Result<String, String> {
    let runs: Vec<&str> = commands
        .lines()
        .filter(|line| line.contains("cv005-contracts"))
        .collect();
    match runs.as_slice() {
        [run] => Ok((*run).to_owned()),
        _ => Err(format!("expected one checker invocation, got {runs:?}")),
    }
}

/// Checks that a dry-run line runs the pinned checker on this repository.
fn pinned_checker(run: &str) -> Check {
    let source = format!(
        "--from 'git+https://github.com/leynos/shared-actions@{}#subdirectory=packages/cv005-contracts'",
        pinned_commit()?
    );
    let wanted = [
        ("uv tool run", "run through uv tool run"),
        ("--python 3.13", "run under Python 3.13"),
        (source.as_str(), "run from the pinned source"),
    ];
    for (needle, what) in wanted {
        if !run.contains(needle) {
            return Err(format!("the checker must {what}: {run}"));
        }
    }
    if run
        .trim_end()
        .ends_with("cv005-contracts check --repository .")
    {
        Ok(())
    } else {
        Err(format!(
            "the checker must end in `check --repository .`: {run}"
        ))
    }
}

#[test]
fn the_pin_is_a_full_commit() -> Check {
    let pin = pinned_commit()?;
    if pin.len() == 40
        && pin
            .chars()
            .all(|digit| matches!(digit, '0'..='9' | 'a'..='f'))
    {
        Ok(())
    } else {
        Err(format!("{pin:?} is not a full commit hash"))
    }
}

#[test]
fn the_target_runs_the_pinned_checker_on_this_repository() -> Check {
    pinned_checker(&checker_run(&make_dry_run(TARGET)?)?)
}

#[test]
fn the_repository_parameter_names_this_repository() -> Check {
    let declared = CONFIG
        .lines()
        .find_map(|line| line.strip_prefix("repository = "))
        .map(|value| value.trim().trim_matches('"'));
    if declared == Some(REPOSITORY) {
        Ok(())
    } else {
        Err(format!("cv005.toml must name {REPOSITORY}: {CONFIG}"))
    }
}

#[test]
fn make_all_includes_the_target() -> Check { pinned_checker(&checker_run(&make_dry_run("all")?)?) }

/// Returns a line's indentation and its trimmed text.
fn indented(line: &str) -> (usize, &str) {
    let trimmed = line.trim_start();
    (line.len() - trimmed.len(), trimmed)
}

/// Returns whether a step's key line is a condition, `- if:` or `if:`.
fn is_condition(key: &str) -> bool { key.trim_start_matches("- ").starts_with("if:") }

/// Returns whether the step whose `run:` line sits at `position` has a condition.
///
/// A step's keys sit at the `run:` key's indentation, from the `- ` line that
/// opens it (two columns to the left, and itself a key) to the next such line.
fn step_is_conditional(lines: &[(usize, &str)], position: usize, indent: usize) -> bool {
    let before = lines
        .iter()
        .take(position)
        .rev()
        .take_while(|(column, key)| {
            *column >= indent && !(*column + 2 == indent && key.starts_with("- "))
        })
        .any(|(column, key)| *column == indent && is_condition(key));
    let after = lines
        .iter()
        .skip(position + 1)
        .take_while(|(column, _)| *column >= indent)
        .any(|(column, key)| *column == indent && is_condition(key));
    let opener = lines
        .iter()
        .take(position)
        .rev()
        .find(|(column, key)| *column + 2 == indent && key.starts_with("- "))
        .is_some_and(|(_, key)| is_condition(key));
    before || after || opener
}

#[test]
fn ci_runs_the_target_unconditionally() -> Check {
    let lines: Vec<(usize, &str)> = CI.lines().map(indented).collect();
    let wanted = format!("run: make {TARGET}");
    let mut found = false;
    for (position, (indent, key)) in lines.iter().enumerate() {
        if *key != wanted && *key != format!("- {wanted}") {
            continue;
        }
        found = true;
        if step_is_conditional(&lines, position, *indent) {
            return Err(format!("the {TARGET} step must carry no condition"));
        }
    }
    if found {
        Ok(())
    } else {
        Err(format!("ci.yml must run `make {TARGET}` in a step"))
    }
}
