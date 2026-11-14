//! Workflow-shape contracts for pinned CI and release routes.
//! Scope root releases to thysalion or Bevy's six tag targets fail to cross-compile (ADR-005).

use std::io;

use cap_std::{ambient_authority, fs_utf8::Dir};
#[path = "workflow_shape/coverage_isolation.rs"]
mod coverage_isolation;
#[path = "workflow_shape/release_linker.rs"]
mod release_linker;

/// Reads a workflow file from the repository's `.github/workflows`.
fn read_workflow(name: &str) -> io::Result<String> {
    let dir = Dir::open_ambient_dir(env!("CARGO_MANIFEST_DIR"), ambient_authority())?;
    dir.read_to_string(format!(".github/workflows/{name}"))
}

/// Identifies one named step in borrowed workflow text.
struct WorkflowStep<'workflow, 'name> {
    /// Complete workflow source searched for this step.
    workflow: &'workflow str,
    /// Exact YAML step name used to find the step boundary.
    step_name: &'name str,
}

/// Returns the named step's lines through its environment, before the next step.
fn named_step_lines<'workflow>(step: &WorkflowStep<'workflow, '_>) -> Option<Vec<&'workflow str>> {
    let marker = format!("      - name: {}\n", step.step_name);
    let lines = step.workflow.split_once(&marker)?.1;
    Some(
        lines
            .lines()
            .take_while(|line| !line.starts_with("      - "))
            .collect(),
    )
}

/// Returns one named step and its trimmed shell body before the next step.
/// For example, `Install cross` includes its `run: |` commands.
fn step_and_run_lines<'workflow>(
    workflow: &'workflow str,
    step_name: &str,
) -> Option<(Vec<&'workflow str>, Vec<&'workflow str>)> {
    let step = named_step_lines(&WorkflowStep {
        workflow,
        step_name,
    })?;
    let run_start = step.iter().position(|line| line.trim() == "run: |")?;
    let shell_lines = step.iter().skip(run_start + 1);
    let run = shell_lines.map(|line| line.trim()).collect();
    Some((step, run))
}

/// Returns whether a release step's `run:` command is `cross build --release` scoped to the
/// root package alone: exactly one `-p`, naming `thysalion` and no other package. Only a `run:`
/// line counts, so a commented-out command satisfies nothing.
fn release_command_is_scoped(step: &[&str]) -> bool {
    let command: Vec<&str> = step
        .iter()
        .filter_map(|line| line.trim_start().strip_prefix("run:"))
        .flat_map(str::split_whitespace)
        .collect();
    let packages: Vec<&str> = command
        .iter()
        .zip(command.iter().skip(1))
        .filter(|(flag, _)| **flag == "-p")
        .map(|(_, package)| *package)
        .collect();
    command.starts_with(&["cross", "build", "--release"]) && packages == ["thysalion"]
}

/// Checks that the release build stays scoped to the root package: the demo crates
/// must never enter the release build graph (ADR-005).
#[test]
fn release_build_is_scoped_to_the_root_package() {
    let workflow = read_workflow("release.yml").expect("read release.yml");
    let step = named_step_lines(&WorkflowStep {
        workflow: &workflow,
        step_name: "Build release binary",
    })
    .expect("release.yml must define the release build");
    assert!(
        release_command_is_scoped(&step),
        "the release build must remain scoped to the root package alone: {step:?}"
    );
    assert!(
        step.iter().any(|line| line.trim() == "RUSTFLAGS: \"\""),
        "the release step must clear development rustflags"
    );
}

/// Release commands that reach the demo crates, or are not commands at all, must be refused.
#[test]
fn release_scope_refuses_demo_packages_and_commented_commands() {
    for (run, scoped) in [
        (
            "        run: cross build --release -p thysalion --target x",
            true,
        ),
        (
            "        run: cross build --release -p thysalion-demos --target x",
            false,
        ),
        (
            "        run: cross build --release -p thysalion -p thysalion-demos",
            false,
        ),
        ("        run: cross build --release --workspace", false),
        (
            "        # run: cross build --release -p thysalion --target x",
            false,
        ),
        ("        run: cross build --release", false),
    ] {
        assert_eq!(
            release_command_is_scoped(&[run]),
            scoped,
            "wrong verdict for: {run}"
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
/// One reader for the four tests below rather than four `expect`s — two reach
/// it through [`ci_triggers`] and two call it directly. They all assert about
/// the same file, and a shared accessor is one place for the failure message
/// to say which file and why.
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
    // Concurrency cancels superseded runs only when their workflow and ref
    // match. A branch push and its pull_request merge-result run use distinct
    // refs, so both remain eligible to run.
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
fn ci_runs_the_headless_suite_on_push_and_manual_dispatch() {
    // Coverage is pull-request-only because main owns the push baseline, but
    // pushes and manual dispatches must run the fixture-loading suite.
    let triggers = ci_triggers();
    assert!(
        entry(&triggers, 2, "workflow_dispatch").is_some(),
        "ci.yml must retain its manual-dispatch trigger; found triggers:\n{triggers}"
    );
    let workflow = ci_workflow();
    let jobs = top_level_block(&workflow, "jobs").expect("ci.yml must declare jobs");
    let job =
        child_block(&jobs, 2, "headless-test").expect("ci.yml must declare a headless-test job");
    assert_eq!(
        entry(&job, 4, "if"),
        Some("github.event_name == 'push' || github.event_name == 'workflow_dispatch'"),
        "headless-test must run for branch pushes and manual dispatches"
    );
    assert!(
        job.contains("run: make test"),
        "the headless job must run the behavioural suite: {job}"
    );
}

#[test]
fn ci_runs_build_test_for_pull_request_merge_results() {
    // A push checks the branch head, while the pull_request event checks the
    // synthetic merge commit. Skipping the latter can let an unchecked merge
    // result satisfy required status checks.
    let workflow = ci_workflow();
    let jobs = top_level_block(&workflow, "jobs").expect("ci.yml must declare jobs");
    let job = child_block(&jobs, 2, "build-test").expect("ci.yml must declare a build-test job");
    assert_eq!(
        entry(&job, 4, "if"),
        Some("github.event_name == 'pull_request'"),
        "build-test must run on every pull_request, without a same-repository skip"
    );
}
