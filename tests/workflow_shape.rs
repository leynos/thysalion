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
    for name in [
        "ci.yml",
        "release.yml",
        "coverage-main.yml",
        "act-validation.yml",
    ] {
        let workflow = read_workflow(name).expect("read workflow file");
        for line in workflow.lines() {
            let Some((_, reference)) = line.split_once("leynos/shared-actions/") else {
                continue;
            };
            let (_, sha) = reference
                .split_once('@')
                .expect("shared-actions references must include @<commit-sha>");
            let trimmed = sha.trim();
            assert!(
                trimmed.len() == 40 && trimmed.chars().all(|c| c.is_ascii_hexdigit()),
                "{name}: shared-actions ref must be a 40-hex commit SHA, found: {trimmed}"
            );
        }
    }
}

/// Extracts a top-level block from a workflow, from its `key:` line to the
/// line before the next top-level key.
///
/// A textual extraction rather than a parse, deliberately: adding a YAML
/// dependency to the root package's dev-dependencies to assert four facts
/// about three files buys a parser this repository would otherwise never need.
/// The cost is that the assertions match formatting as well as meaning, which
/// is acceptable for files a human edits by hand and a linter formats.
fn top_level_block(workflow: &str, key: &str) -> Option<String> {
    let is_key = |line: &str| {
        line.strip_prefix(key)
            .is_some_and(|rest| rest.starts_with(':'))
    };
    let mut lines = workflow.lines().skip_while(|line| !is_key(line));
    let mut block = String::from(lines.next()?);
    for line in lines {
        let is_continuation =
            line.is_empty() || line.starts_with(char::is_whitespace) || line.starts_with('#');
        if !is_continuation {
            break;
        }
        block.push('\n');
        block.push_str(line);
    }
    Some(block)
}

#[test]
fn ci_runs_on_every_push_to_every_branch() {
    // Roadmap 1.3.1's success criterion is that the headless behavioural suite
    // runs in continuous integration "on every push". Without a push trigger
    // the suite runs only once a pull request exists, and the criterion is
    // quietly unmet.
    let workflow = read_workflow("ci.yml").expect("read ci.yml");
    let triggers = top_level_block(&workflow, "'on'").expect("ci.yml must declare triggers");
    assert!(
        triggers.contains("push:"),
        "ci.yml must run on push; found triggers:\n{triggers}"
    );
    assert!(
        triggers.contains("branches: ['**']"),
        "ci.yml's push trigger must cover every branch; found triggers:\n{triggers}"
    );
    assert!(
        triggers.contains("pull_request:"),
        "ci.yml must keep its pull_request trigger: fork pull requests generate no push event \
         here, so dropping it would stop testing them; found triggers:\n{triggers}"
    );
}

#[test]
fn ci_keeps_one_run_per_ref() {
    // Push and pull_request together would double every run on an open pull
    // request. The concurrency group is half of what stops that; the job guard
    // below is the other half.
    let workflow = read_workflow("ci.yml").expect("read ci.yml");
    let concurrency =
        top_level_block(&workflow, "concurrency").expect("ci.yml must declare a concurrency group");
    assert!(
        concurrency.contains("group: ${{ github.workflow }}-${{ github.ref }}"),
        "the concurrency group must be keyed on workflow and ref; found:\n{concurrency}"
    );
    assert!(
        concurrency.contains("cancel-in-progress: true"),
        "superseded runs must be cancelled; found:\n{concurrency}"
    );
}

#[test]
fn ci_skips_the_redundant_pull_request_run_for_same_repository_branches() {
    // A same-repository pull request's head push already ran this workflow, so
    // its pull_request event carries no new information. Required status
    // checks match on job name regardless of the triggering event, so the
    // push-triggered run on the same head commit satisfies the check.
    let workflow = read_workflow("ci.yml").expect("read ci.yml");
    assert!(
        workflow.contains("github.event_name != 'pull_request'"),
        "ci.yml's job must guard the redundant pull_request run"
    );
    assert!(
        workflow.contains("github.event.pull_request.head.repo.full_name != github.repository"),
        "the guard must still run fork pull requests, which generate no push event here"
    );
}
