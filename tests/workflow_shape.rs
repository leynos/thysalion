//! Workflow-shape contracts for pinned CI and release routes.
//! Scope root releases to thysalion or Bevy's six tag targets fail to cross-compile (ADR-005).

use std::io;

use cap_std::{ambient_authority, fs_utf8::Dir};
#[path = "workflow_shape/backend_env.rs"]
mod backend_env;
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

/// Stores a borrowed workflow and selected lines for composed coverage checks.
struct CoverageGuard<'workflow> {
    /// Workflow text used for step order and the pinned action check.
    workflow: &'workflow str,
    /// Selected YAML lines for the shell guard step.
    guard_step: Vec<&'workflow str>,
    /// Trimmed shell lines checked for failure propagation and guard order.
    run: Vec<&'workflow str>,
    /// Selected YAML lines for the coverage action environment.
    coverage_step: Vec<&'workflow str>,
}

impl CoverageGuard<'_> {
    /// Evaluates all five coverage guard requirements in their contract order.
    fn is_binding(&self) -> bool {
        let step_starts: Vec<_> = self
            .workflow
            .lines()
            .filter(|line| line.starts_with("      - "))
            .collect();
        let guard = step_starts
            .iter()
            .position(|line| *line == "      - name: Check coverage flag isolation");
        let coverage = step_starts
            .iter()
            .position(|line| *line == "      - name: Test and Measure Coverage");
        [
            guard
                .zip(coverage)
                .is_some_and(|(guard_index, coverage_index)| coverage_index == guard_index + 1),
            self.workflow.contains(concat!(
                "      - name: Test and Measure Coverage\n",
                "        uses: leynos/shared-actions/.github/actions/generate-coverage@"
            )),
            self.guard_step.iter().all(|line| {
                let key = line.trim();
                !key.starts_with("if:") && !key.starts_with("continue-on-error:")
            }),
            self.shell_guard_is_hard_failing(),
            self.coverage_action_excludes_encoded_flags(),
        ]
        .into_iter()
        .all(|condition| condition)
    }

    /// Requires ordered shell checks and an unmasked failing status.
    fn shell_guard_is_hard_failing(&self) -> bool {
        let required = [
            "if [ \"${CARGO_ENCODED_RUSTFLAGS+x}\" = x ]; then",
            "echo 'CARGO_ENCODED_RUSTFLAGS must be unset for coverage' >&2",
            "exit 1",
            "fi",
        ];
        let positions = required
            .iter()
            .map(|expected| self.run.iter().position(|line| line == expected))
            .collect::<Option<Vec<_>>>();
        !self.run.iter().any(|line| line.contains("|| true"))
            && positions.is_some_and(|ordered_positions| {
                ordered_positions
                    .windows(2)
                    .all(|pair| matches!(pair, [left, right] if left < right))
            })
    }

    /// Rejects the encoded-flags key in block-style or flow-style action env maps.
    fn coverage_action_excludes_encoded_flags(&self) -> bool {
        self.coverage_step
            .iter()
            .flat_map(|line| line.split(['{', '}', ',']))
            .filter_map(|entry| entry.trim().split_once(':'))
            .all(|(key, _)| key.trim().trim_matches(['"', '\'']) != "CARGO_ENCODED_RUSTFLAGS")
    }
}

/// Requires an immediate hard-failing guard before coverage measurement.
fn coverage_guard_is_binding(workflow: &str) -> bool {
    let Some((guard_step, run)) = step_and_run_lines(workflow, "Check coverage flag isolation")
    else {
        return false;
    };
    let Some(coverage_step) = named_step_lines(&WorkflowStep {
        workflow,
        step_name: "Test and Measure Coverage",
    }) else {
        return false;
    };
    let guard = CoverageGuard {
        workflow,
        guard_step,
        run,
        coverage_step,
    };
    guard.is_binding()
}

/// For example, it retains linker and LLVM settings while adding an empty encoded-flags key.
fn flow_style_coverage_mutation(workflow: &str) -> Option<String> {
    let marker = "      - name: Test and Measure Coverage\n";
    let action = workflow.split_once(marker)?.1;
    let before_inputs = action.split_once("        with:\n")?.0;
    let env_lines = before_inputs.split_once("        env:\n")?.1;
    let entries = env_lines.lines().map(str::trim).collect::<Vec<_>>();
    let joined_entries = entries.join(", ");
    Some(workflow.replacen(
        &format!("        env:\n{env_lines}"),
        &format!("        env: {{ \"CARGO_ENCODED_RUSTFLAGS\": '', {joined_entries} }}\n"),
        1,
    ))
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

/// Checks that PR coverage rejects caller-provided encoded flags first.
#[test]
fn coverage_flag_guard_is_binding_before_measurement() {
    let workflow = read_workflow("ci.yml").expect("read ci.yml");
    assert!(
        coverage_guard_is_binding(&workflow),
        "coverage must follow an unconditional guard that fails on encoded flags"
    );
}

/// Checks that removing or reordering the coverage guard breaks its contract.
#[test]
fn coverage_guard_rejects_removal_and_reordering() {
    let workflow = read_workflow("ci.yml").expect("read ci.yml");
    let guard_marker = "      - name: Check coverage flag isolation\n";
    let coverage_marker = "      - name: Test and Measure Coverage\n";
    let guard_start = workflow
        .find(guard_marker)
        .expect("ci.yml must define the coverage guard");
    let coverage_start = workflow
        .find(coverage_marker)
        .expect("ci.yml must define the coverage measurement step");
    let guard_step = workflow
        .get(guard_start..coverage_start)
        .expect("guard and coverage markers must bound a valid step");
    let removed = workflow.replacen(guard_step, "", 1);
    assert!(
        !coverage_guard_is_binding(&removed),
        "removing the guard must fail the coverage contract"
    );
    let without_guard = workflow.replacen(guard_step, "", 1);
    let doctest_start = without_guard
        .find("      - name: Run doctests\n")
        .expect("ci.yml must define the doctest step after coverage");
    let moved_after_coverage = format!(
        "{}{}{}",
        without_guard
            .get(..doctest_start)
            .expect("doctest marker must start at a character boundary"),
        guard_step,
        without_guard
            .get(doctest_start..)
            .expect("doctest marker must start at a character boundary")
    );
    assert!(
        !coverage_guard_is_binding(&moved_after_coverage),
        "moving the guard after coverage must fail the contract"
    );
}

/// Checks that conditional or masked failures do not satisfy the coverage guard.
#[test]
fn coverage_guard_rejects_conditional_and_soft_failure() {
    let workflow = read_workflow("ci.yml").expect("read ci.yml");
    let guard_marker = "      - name: Check coverage flag isolation\n";

    let conditional = workflow.replacen(
        guard_marker,
        "      - name: Check coverage flag isolation\n        if: always()\n",
        1,
    );
    assert!(
        !coverage_guard_is_binding(&conditional),
        "adding a workflow condition must fail the contract"
    );

    let soft_failed = workflow.replacen(
        concat!(
            "      - name: Check coverage flag isolation\n",
            "        run: |\n"
        ),
        concat!(
            "      - name: Check coverage flag isolation\n",
            "        continue-on-error: true\n",
            "        run: |\n"
        ),
        1,
    );
    assert!(
        !coverage_guard_is_binding(&soft_failed),
        "allowing the guard step to continue on error must fail the contract"
    );

    let shell_soft_failed =
        workflow.replacen("            exit 1\n", "            exit 1 || true\n", 1);
    assert!(
        !coverage_guard_is_binding(&shell_soft_failed),
        "masking the shell's failure status must fail the contract"
    );
}

/// Checks that the action cannot reintroduce encoded flags after the guard.
#[test]
fn coverage_action_rejects_empty_encoded_flags() {
    let workflow = read_workflow("ci.yml").expect("read ci.yml");
    let action_with_empty_encoded_flags = workflow.replacen(
        concat!(
            "        env:\n",
            "          CARGO_TARGET_X86_64_UNKNOWN_LINUX_GNU_LINKER: clang\n"
        ),
        concat!(
            "        env:\n",
            "          CARGO_ENCODED_RUSTFLAGS: ''\n",
            "          CARGO_TARGET_X86_64_UNKNOWN_LINUX_GNU_LINKER: clang\n"
        ),
        1,
    );
    assert!(
        !coverage_guard_is_binding(&action_with_empty_encoded_flags),
        "coverage contract accepted an empty encoded-flags override"
    );

    let action_with_quoted_encoded_flags = action_with_empty_encoded_flags.replace(
        "CARGO_ENCODED_RUSTFLAGS: ''",
        "\"CARGO_ENCODED_RUSTFLAGS\": ''",
    );
    assert!(
        !coverage_guard_is_binding(&action_with_quoted_encoded_flags),
        "coverage contract accepted a quoted empty encoded-flags key"
    );

    let flow = flow_style_coverage_mutation(&workflow).expect("coverage env block must be present");
    assert!(
        flow != workflow && !coverage_guard_is_binding(&flow),
        "a flow-style action env mapping must not bypass the contract"
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
