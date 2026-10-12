//! Coverage isolation contracts shared by PR measurement and the main publisher.

use rstest::rstest;

use super::{WorkflowStep, named_step_lines, read_workflow, step_and_run_lines};

/// Explicit action overrides keep measurement independent of development settings.
const COVERAGE_ENVIRONMENT: [(&str, &str); 8] = [
    ("CARGO_TARGET_X86_64_UNKNOWN_LINUX_GNU_LINKER", "clang"),
    ("CARGO_UNSTABLE_CODEGEN_BACKEND", "true"),
    ("CARGO_PROFILE_DEV_CODEGEN_BACKEND", "llvm"),
    ("CARGO_PROFILE_DEV_BUILD_OVERRIDE_CODEGEN_BACKEND", "llvm"),
    ("CARGO_PROFILE_TEST_BUILD_OVERRIDE_CODEGEN_BACKEND", "llvm"),
    ("RUSTFLAGS", "-C link-arg=-fuse-ld=lld"),
    ("CFLAGS", "-fuse-ld=lld"),
    ("LDFLAGS", "-fuse-ld=lld"),
];

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
    /// Evaluates guard ordering, failure propagation, and LLVM coverage routing.
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
            self.coverage_action_uses_llvm_linker(),
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
        let commands: Vec<_> = self
            .run
            .iter()
            .copied()
            .filter(|line| !line.is_empty() && !line.starts_with('#'))
            .collect();
        commands == required
    }

    /// Rejects the encoded-flags key in block-style or flow-style action env maps.
    fn coverage_action_excludes_encoded_flags(&self) -> bool {
        self.coverage_step
            .iter()
            .flat_map(|line| line.split(['{', '}', ',']))
            .filter_map(|entry| entry.trim().split_once(':'))
            .all(|(key, _)| key.trim().trim_matches(['"', '\'']) != "CARGO_ENCODED_RUSTFLAGS")
    }

    /// Requires each coverage override exactly once within the action environment.
    fn coverage_action_uses_llvm_linker(&self) -> bool {
        let Some(env_start) = self
            .coverage_step
            .iter()
            .position(|line| *line == "        env:")
        else {
            return false;
        };
        let environment: Vec<_> = self
            .coverage_step
            .iter()
            .skip(env_start + 1)
            .take_while(|line| line.starts_with("          "))
            .filter_map(|line| line.trim().split_once(':'))
            .collect();
        COVERAGE_ENVIRONMENT.iter().all(|(name, required)| {
            let values: Vec<_> = environment
                .iter()
                .filter(|(key, _)| key == name)
                .map(|(_, value)| value.trim().trim_matches(['"', '\'']))
                .collect();
            values.as_slice() == [*required]
        })
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

/// Checks that each measurement route rejects encoded flags and selects LLVM/lld.
#[rstest]
#[case::pull_request("ci.yml")]
#[case::main_publisher("coverage-main.yml")]
fn coverage_flag_guard_is_binding_before_measurement(#[case] name: &str) {
    let workflow = read_workflow(name).expect("read coverage workflow");
    assert!(
        coverage_guard_is_binding(&workflow),
        "{name}: coverage must follow a hard-failing guard and retain LLVM/lld overrides"
    );
}

/// Checks that removing or reordering the coverage guard breaks its contract.
#[rstest]
#[case::pull_request("ci.yml")]
#[case::main_publisher("coverage-main.yml")]
fn coverage_guard_rejects_removal_and_reordering(#[case] name: &str) {
    let workflow = read_workflow(name).expect("read coverage workflow");
    let guard_marker = "      - name: Check coverage flag isolation\n";
    let coverage_marker = "      - name: Test and Measure Coverage\n";
    let guard_start = workflow
        .find(guard_marker)
        .expect("coverage workflow must define the guard");
    let coverage_start = workflow
        .find(coverage_marker)
        .expect("coverage workflow must define the measurement step");
    let guard_step = workflow
        .get(guard_start..coverage_start)
        .expect("guard and coverage markers must bound a valid step");
    let removed = workflow.replacen(guard_step, "", 1);
    assert!(
        !coverage_guard_is_binding(&removed),
        "{name}: removing the guard must fail the coverage contract"
    );
    let without_guard = workflow.replacen(guard_step, "", 1);
    let action_start = without_guard
        .find(coverage_marker)
        .expect("coverage measurement must remain after removing the guard");
    let action_tail = without_guard
        .get(action_start + coverage_marker.len()..)
        .expect("coverage marker must end at a character boundary");
    let next_step = action_start
        + coverage_marker.len()
        + action_tail
            .find("      - ")
            .expect("coverage action must have a following step");
    let moved_after_coverage = format!(
        "{}{}{}",
        without_guard
            .get(..next_step)
            .expect("following step marker must start at a character boundary"),
        guard_step,
        without_guard
            .get(next_step..)
            .expect("following step marker must start at a character boundary")
    );
    assert!(
        !coverage_guard_is_binding(&moved_after_coverage),
        "{name}: moving the guard after coverage must fail the contract"
    );
}

/// Checks that conditional or masked failures do not satisfy the coverage guard.
#[rstest]
#[case::pull_request("ci.yml")]
#[case::main_publisher("coverage-main.yml")]
fn coverage_guard_rejects_conditional_and_soft_failure(#[case] name: &str) {
    let workflow = read_workflow(name).expect("read coverage workflow");
    let guard_marker = "      - name: Check coverage flag isolation\n";

    let conditional = workflow.replacen(
        guard_marker,
        "      - name: Check coverage flag isolation\n        if: always()\n",
        1,
    );
    assert!(
        !coverage_guard_is_binding(&conditional),
        "{name}: adding a workflow condition must fail the contract"
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
        "{name}: allowing the guard step to continue on error must fail the contract"
    );

    let shell_soft_failed =
        workflow.replacen("            exit 1\n", "            exit 1 || true\n", 1);
    assert!(
        !coverage_guard_is_binding(&shell_soft_failed),
        "{name}: masking the shell's failure status must fail the contract"
    );
}

/// Checks that the action cannot reintroduce encoded flags after the guard.
#[rstest]
#[case::pull_request("ci.yml")]
#[case::main_publisher("coverage-main.yml")]
fn coverage_action_rejects_empty_encoded_flags(#[case] name: &str) {
    let workflow = read_workflow(name).expect("read coverage workflow");
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
        "{name}: coverage contract accepted an empty encoded-flags override"
    );

    let action_with_quoted_encoded_flags = action_with_empty_encoded_flags.replace(
        "CARGO_ENCODED_RUSTFLAGS: ''",
        "\"CARGO_ENCODED_RUSTFLAGS\": ''",
    );
    assert!(
        !coverage_guard_is_binding(&action_with_quoted_encoded_flags),
        "{name}: coverage contract accepted a quoted empty encoded-flags key"
    );

    let flow = flow_style_coverage_mutation(&workflow).expect("coverage env block must be present");
    assert!(
        flow != workflow && !coverage_guard_is_binding(&flow),
        "{name}: a flow-style action env mapping must not bypass the contract"
    );
}

/// Checks every required backend and linker override against removal and replacement.
#[rstest]
#[case::pull_request("ci.yml")]
#[case::main_publisher("coverage-main.yml")]
fn coverage_action_rejects_missing_or_changed_llvm_overrides(#[case] name: &str) {
    let workflow = read_workflow(name).expect("read coverage workflow");
    for (key, required) in COVERAGE_ENVIRONMENT {
        let configured = if key == "CARGO_UNSTABLE_CODEGEN_BACKEND" {
            format!("          {key}: '{required}'\n")
        } else {
            format!("          {key}: {required}\n")
        };
        for invalid in [String::new(), format!("          {key}: development\n")] {
            let mutated = workflow.replacen(&configured, &invalid, 1);
            assert_ne!(
                mutated, workflow,
                "{name}: mutation of {key} must change the workflow"
            );
            assert!(
                !coverage_guard_is_binding(&mutated),
                "{name}: coverage contract accepted {key} override: {invalid:?}"
            );
        }
    }
}

/// Checks that an extra step cannot separate the guard from measurement.
#[rstest]
#[case::pull_request("ci.yml")]
#[case::main_publisher("coverage-main.yml")]
fn coverage_guard_rejects_intervening_steps(#[case] name: &str) {
    let workflow = read_workflow(name).expect("read coverage workflow");
    let marker = "      - name: Test and Measure Coverage\n";
    let mutated = workflow.replacen(
        marker,
        &format!("      - name: Intervening step\n        run: true\n{marker}"),
        1,
    );
    assert_ne!(mutated, workflow, "{name}: mutation must insert a step");
    assert!(
        !coverage_guard_is_binding(&mutated),
        "{name}: coverage must immediately follow its guard"
    );
}

/// Checks that early success and altered failure commands cannot bypass the guard.
#[rstest]
#[case::early_success(
    "        run: |\n          if [",
    concat!("        run: |\n", "          exit 0\n", "          if [")
)]
#[case::success_status("            exit 1\n", "            exit 0\n")]
#[case::missing_failure("            exit 1\n", "")]
fn coverage_guard_rejects_shell_bypasses(
    #[case] configured: &str,
    #[case] invalid: &str,
    #[values("ci.yml", "coverage-main.yml")] name: &str,
) {
    let workflow = read_workflow(name).expect("read coverage workflow");
    let mutated = workflow.replacen(configured, invalid, 1);
    assert_ne!(
        mutated, workflow,
        "{name}: shell mutation must change the guard"
    );
    assert!(
        !coverage_guard_is_binding(&mutated),
        "{name}: coverage contract accepted shell bypass: {invalid:?}"
    );
}
