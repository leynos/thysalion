//! CI doctest flags cannot borrow another workflow step's environment.

use cap_std::{ambient_authority, fs::Dir};
use super::helpers::{normalized, names, THREADS_FLAG, MOLD_FLAG};

/// Returns the `RUSTFLAGS` a named workflow step assigns.
///
/// The step runs from its `- name:` line to the next line indented no deeper
/// than that list item, so a re-indented workflow, or a later step that also
/// assigns `RUSTFLAGS`, cannot lend the step flags it does not carry.
fn step_rustflags(workflow: &str, name: &str) -> Option<String> {
    let marker = format!("- name: {name}");
    let mut lines = workflow.lines().skip_while(|line| line.trim() != marker);
    let item_indent = {
        let first = lines.next()?;
        first.len() - first.trim_start().len()
    };
    lines
        .take_while(|line| {
            line.trim().is_empty() || line.len() - line.trim_start().len() > item_indent
        })
        .find_map(|line| line.trim().strip_prefix("RUSTFLAGS:"))
        .map(|value| value.trim().to_owned())
}

/// A later step assigning `RUSTFLAGS` must not be read as the named step's.
#[test]
fn a_step_does_not_borrow_a_later_steps_rustflags() {
    let workflow = "  steps:\n    - name: First\n      run: true\n    - name: Second\n      \
                    env:\n        RUSTFLAGS: -Zthreads=8\n";
    assert_eq!(step_rustflags(workflow, "First"), None);
    assert_eq!(
        step_rustflags(workflow, "Second").as_deref(),
        Some("-Zthreads=8")
    );
}

/// CI runs the doctests directly, outside `make`, so the step's own assignment
/// is the only thing that keeps the standard's flags on that build: the toolchain
/// action's `RUSTFLAGS` would otherwise replace every configuration source.
#[test]
fn the_ci_doctest_step_restates_both_flags() {
    let root = Dir::open_ambient_dir(env!("CARGO_MANIFEST_DIR"), ambient_authority())
        .expect("open the manifest directory");
    let workflow = root
        .read_to_string(".github/workflows/ci.yml")
        .expect("read ci.yml");
    let value =
        step_rustflags(&workflow, "Run doctests").expect("the doctest step assigns RUSTFLAGS");
    let words: Vec<String> = value.split_whitespace().map(str::to_owned).collect();
    let flags = normalized(&words);
    assert!(
        names(&flags, THREADS_FLAG),
        "the doctest step drops {THREADS_FLAG}: {flags:?}"
    );
    assert!(
        names(&flags, MOLD_FLAG),
        "the doctest step drops {MOLD_FLAG}: {flags:?}"
    );
    let denies_warnings =
        names(&flags, "-Dwarnings") || flags.windows(2).any(|pair| pair == ["-D", "warnings"]);
    assert!(
        denies_warnings,
        "the doctest step stops denying warnings: {flags:?}"
    );
}
