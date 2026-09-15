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

/// The CI workflow's text, or a panic naming what could not be read.
///
/// One reader for the three tests below rather than three `expect`s: they all
/// assert about the same file, and a shared accessor is one place for the
/// failure message to say which file and why.
fn ci_workflow() -> String {
    match read_workflow("ci.yml") {
        Ok(text) => text,
        Err(error) => panic!("ci.yml must be readable: {error}"),
    }
}

/// Whether a line carries content, as opposed to a comment or blank space.
///
/// Load-bearing for every assertion below. Without it a test could be
/// satisfied by the prose in a comment that happens to mention the key it is
/// looking for — and the comments in `ci.yml` discuss exactly the triggers
/// these tests assert on.
fn is_content(line: &str) -> bool {
    let trimmed = line.trim();
    !trimmed.is_empty() && !trimmed.starts_with('#')
}

/// A line's indentation depth, in spaces.
fn indent_of(line: &str) -> usize { line.len().saturating_sub(line.trim_start().len()) }

/// Whether `line` declares `key` at `indent`.
fn declares(line: &str, indent: usize, key: &str) -> bool {
    indent_of(line) == indent
        && line
            .trim_start()
            .strip_prefix(key)
            .is_some_and(|rest| rest.starts_with(':'))
}

/// The value written after `key:` at `indent` within `block`.
///
/// An empty string means the key is present but its value is on the lines
/// below it, which is how a nested mapping or a folded scalar appears.
fn entry<'a>(block: &'a str, indent: usize, key: &str) -> Option<&'a str> {
    block
        .lines()
        .filter(|line| is_content(line))
        .find(|line| declares(line, indent, key))
        .and_then(|line| line.trim_start().strip_prefix(key)?.strip_prefix(':'))
        .map(str::trim)
}

/// The lines nested under `key:` at `indent` within `block`.
fn child_block(block: &str, indent: usize, key: &str) -> Option<String> {
    let mut lines = block
        .lines()
        .filter(|line| is_content(line))
        .skip_while(|line| !declares(line, indent, key));
    lines.next()?;
    let nested: Vec<&str> = lines.take_while(|line| indent_of(line) > indent).collect();
    Some(nested.join("\n"))
}

/// Extracts a top-level block, from its `key:` line to the next top-level key.
///
/// A textual extraction rather than a parse, deliberately: adding a YAML
/// dependency to the root package to assert a handful of facts about one file
/// buys a parser this repository would otherwise never need. The helpers above
/// recover the part of a parse these assertions actually rely on — comments
/// excluded, and keys matched at a known indentation rather than anywhere in
/// the text.
fn top_level_block(workflow: &str, key: &str) -> Option<String> {
    let mut lines = workflow.lines().skip_while(|line| !declares(line, 0, key));
    let mut block = String::from(lines.next()?);
    for line in lines {
        if is_content(line) && indent_of(line) == 0 {
            break;
        }
        block.push('\n');
        block.push_str(line);
    }
    Some(block)
}

/// The `'on':` block of the CI workflow.
fn ci_triggers() -> String {
    let workflow = ci_workflow();
    let Some(block) = top_level_block(&workflow, "'on'") else {
        panic!("ci.yml must declare triggers");
    };
    block
}

#[test]
fn ci_runs_on_every_push_to_every_branch() {
    // Roadmap 1.3.1's success criterion is that the headless behavioural suite
    // runs in continuous integration "on every push". Without a push trigger
    // the suite runs only once a pull request exists, and the criterion is
    // quietly unmet.
    let triggers = ci_triggers();
    let push = child_block(&triggers, 2, "push").expect("ci.yml must run on push");
    assert_eq!(
        entry(&push, 4, "branches"),
        Some("['**']"),
        "ci.yml's push trigger must cover every branch; found:\n{push}"
    );
}

#[test]
fn ci_keeps_testing_fork_pull_requests() {
    // Fork pull requests raise no push event in this repository, so dropping
    // the pull_request trigger would silently stop testing them.
    let triggers = ci_triggers();
    assert!(
        entry(&triggers, 2, "pull_request").is_some(),
        "ci.yml must keep its pull_request trigger; found triggers:\n{triggers}"
    );
}

#[test]
fn ci_keeps_one_run_per_ref() {
    // Push and pull_request together would double every run on an open pull
    // request. The concurrency group is half of what stops that; the job guard
    // below is the other half.
    let workflow = ci_workflow();
    let concurrency =
        top_level_block(&workflow, "concurrency").expect("ci.yml must declare a concurrency group");
    assert_eq!(
        entry(&concurrency, 2, "group"),
        Some("${{ github.workflow }}-${{ github.ref }}"),
        "the concurrency group must be keyed on workflow and ref; found:\n{concurrency}"
    );
    assert_eq!(
        entry(&concurrency, 2, "cancel-in-progress"),
        Some("true"),
        "superseded runs must be cancelled; found:\n{concurrency}"
    );
}

#[test]
fn ci_skips_the_redundant_pull_request_run_for_same_repository_branches() {
    // A same-repository pull request's head push already ran this workflow, so
    // its pull_request event carries no new information. Required status
    // checks match on job name regardless of the triggering event, so the
    // push-triggered run on the same head commit satisfies the check.
    let workflow = ci_workflow();
    let jobs = top_level_block(&workflow, "jobs").expect("ci.yml must declare jobs");
    let job = child_block(&jobs, 2, "build-test").expect("ci.yml must declare a build-test job");
    let guard = child_block(&job, 4, "if").expect("the build-test job must carry a guard");
    let condition = guard.split_whitespace().collect::<Vec<&str>>().join(" ");
    assert_eq!(
        condition,
        "github.event_name != 'pull_request' || github.event.pull_request.head.repo.full_name != \
         github.repository",
        "the guard must skip same-repository pull requests and still run fork ones"
    );
}
