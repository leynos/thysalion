//! Workflow-shape contracts for pinned CI and release routes.
//! Scope root releases to thysalion or Bevy's six tag targets fail to cross-compile (ADR-005).

use std::io;

use cap_std::{ambient_authority, fs_utf8::Dir};
#[path = "workflow_shape/backend_env.rs"]
mod backend_env;
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

/// Checks the cached tool route before installing `cross` with stable Cargo.
#[test]
fn cross_install_uses_the_stable_external_route() {
    let workflow = read_workflow("release.yml").expect("read release.yml");
    let cache = workflow
        .find("      - name: Cache cross binary")
        .expect("release.yml must cache the cross binary");
    let install = workflow
        .find("      - name: Install cross")
        .expect("release.yml must define the cross installer");
    let build = workflow
        .find("      - name: Build release binary")
        .expect("release.yml must define the release build");
    assert!(
        cache < install && install < build,
        "the cross cache and installer must precede the release build"
    );
    let (step, run_lines) = step_and_run_lines(&workflow, "Install cross")
        .expect("the cross installer must have a shell run block");
    let run = run_lines.join("\n");
    let cache_test = run
        .find("if [ -x \"$HOME/.cargo/bin/cross\" ]; then")
        .expect("the installer must check the cached cross binary");
    let cache_exit = run
        .find("exit 0")
        .expect("the installer must skip installation on a cache hit");
    let cache_end = run
        .find("\nfi\n")
        .expect("the cache check must end before installation");
    let unset =
        backend_env::cleanup_end(&run).expect("the installer must clear inherited Cargo settings");
    let cd = run
        .find("cd /")
        .expect("the installer must leave the repository before using stable Cargo");
    let install_command = run
        .find(concat!(
            "cargo +stable install cross --git ",
            "https://github.com/cross-rs/cross --rev ",
            "\"${CROSS_REVISION}\""
        ))
        .expect("the installer must use stable Cargo and the pinned cross revision");
    assert!(
        cache_test < cache_exit
            && cache_exit < cache_end
            && cache_end < unset
            && unset < cd
            && cd < install_command,
        "check the cache, clear inherited settings, leave the repository, then install cross"
    );
    assert!(
        step.iter().any(|line| line.trim() == "RUSTFLAGS: \"\""),
        "the installer step must clear development rustflags"
    );
}

/// Checks the stable release build from outside the workspace.
#[test]
fn release_build_uses_the_stable_external_manifest_route() {
    let workflow = read_workflow("release.yml").expect("read release.yml");
    let (step, run_lines) = step_and_run_lines(&workflow, "Build release binary")
        .expect("release build must define a shell run block");
    let run = run_lines.join("\n");
    let unset =
        backend_env::cleanup_end(&run).expect("release build must clear inherited Cargo settings");
    let cd = run
        .find("cd /")
        .expect("release build must run outside the repository");
    let build = run
        .find("cross +stable build --manifest-path")
        .expect("release build must select stable and pass the manifest to cross");
    let manifest = run
        .find("--manifest-path \"${GITHUB_WORKSPACE}/Cargo.toml\"")
        .expect("release build must select the workspace manifest explicitly");
    let package_scope = run
        .find("--release -p thysalion")
        .expect("release build must remain scoped to the root package");
    assert!(
        unset < cd && cd < build && build < manifest && manifest < package_scope,
        "clear inherited settings, change to /, then build with stable and explicit root scope"
    );
    assert!(
        step.iter().any(|line| line.trim() == "RUSTFLAGS: \"\""),
        "the release step must clear development rustflags"
    );
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
