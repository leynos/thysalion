//! Workflow-shape contract tests.
//!
//! These tests assert the *shape* of the GitHub workflows this workspace
//! depends on, per the rules in docs/developers-guide.md ("Workflow pins
//! and Dependabot"): they match patterns, never literal commit SHAs, so
//! Dependabot bumps cannot break them.
//!
//! The release-scoping assertion is load-bearing: `cross build` without
//! `-p thysalion` would try to cross-compile the Bevy-dependent demo
//! crates for all six release targets and break the release pipeline at
//! tag time (see ADR-005 and the 1.1 execplan's pre-mortem).

use std::io;

use cap_std::{ambient_authority, fs_utf8::Dir};

/// Reads a workflow file from the repository's `.github/workflows`.
fn read_workflow(name: &str) -> io::Result<String> {
    let dir = Dir::open_ambient_dir(env!("CARGO_MANIFEST_DIR"), ambient_authority())?;
    dir.read_to_string(format!(".github/workflows/{name}"))
}

#[test]
fn release_build_is_scoped_to_the_root_package() {
    let workflow = read_workflow("release.yml").expect("read release.yml");
    let build_lines: Vec<&str> = workflow
        .lines()
        .filter(|line| line.contains("cross ") && line.contains("build"))
        .collect();
    assert!(
        !build_lines.is_empty(),
        "release.yml no longer contains a cross build line; update this test alongside the \
         workflow"
    );
    for line in build_lines {
        assert!(
            line.contains("-p thysalion"),
            "release cross build must be scoped with `-p thysalion` so demo crates never enter \
             the release graph, found: {line}"
        );
    }
}

#[test]
fn workflows_pin_shared_actions_to_full_length_shas() {
    for name in ["ci.yml", "release.yml", "coverage-main.yml"] {
        let workflow = read_workflow(name).expect("read workflow file");
        for line in workflow.lines() {
            let Some((_, reference)) = line.split_once("leynos/shared-actions/") else {
                continue;
            };
            let Some((_, sha)) = reference.split_once('@') else {
                continue;
            };
            let trimmed = sha.trim();
            assert!(
                trimmed.len() == 40 && trimmed.chars().all(|c| c.is_ascii_hexdigit()),
                "{name}: shared-actions ref must be a 40-hex commit SHA, found: {trimmed}"
            );
        }
    }
}
