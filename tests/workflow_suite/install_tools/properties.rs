//! Bounded properties for workflow runner and suite-order contracts.
//!
//! The runner oracle classifies generated runner tags by their source category.
//! The ordering oracle scans generated steps once, recording whether an
//! installer has already run before each suite entry.

use proptest::prelude::*;
use yaml_rust2::{Yaml, yaml::Hash};

use super::{inspect_routing, runner::runner_systems};

proptest! {
    #![proptest_config(ProptestConfig {
        cases: 128,
        rng_seed: proptest::test_runner::RngSeed::Fixed(0x5eed),
        ..ProptestConfig::default()
    })]

    #[test]
    fn matrix_runner_resolution_matches_the_candidate_oracle(
        axis in prop::collection::vec(0u8..5, 1..=4),
        include in prop::collection::vec(0u8..5, 0..=4),
    ) {
        let expected = axis
            .iter()
            .chain(&include)
            .any(|candidate| is_linux_candidate(*candidate));
        let job = matrix_job(&axis, &include);
        for shape in 0u8..3 {
            let runs_on = matrix_runs_on(shape);
            prop_assert_eq!(
                runner_systems(&job, &runs_on),
                Ok(expected),
                "supported runs-on shape {} changed the candidate classification",
                shape
            );
        }
    }

    #[test]
    fn suite_order_matches_the_prior_installer_oracle(
        generated in prop::collection::vec(0u8..3, 0..=16),
    ) {
        let steps = generated
            .into_iter()
            .map(workflow_step)
            .chain([WorkflowStep::Suite])
            .collect::<Vec<_>>();
        let expected = missing_installers_before_suite(&steps);
        let source = workflow_source(&steps);
        let report = inspect_routing(&source)
            .map(|report| report.missing_installers.len());

        prop_assert_eq!(report, Ok(expected));
    }
}

/// Maps stable tags to runner spellings without repeating the parser's prefix rules.
const fn runner_label(candidate: u8) -> &'static str {
    match candidate {
        0 => "ubuntu-latest",
        1 => "linux-x64",
        2 => "debian-12",
        3 => "windows-latest",
        _ => "macos-14",
    }
}

/// Classifies the source category used to create a runner candidate.
const fn is_linux_candidate(candidate: u8) -> bool { matches!(candidate, 0..=2) }

/// Builds the job-side matrix values consumed by the runner expression.
fn matrix_job(axis: &[u8], include: &[u8]) -> Yaml {
    let axis_values = Yaml::Array(
        axis.iter()
            .map(|candidate| Yaml::String(runner_label(*candidate).to_owned()))
            .collect(),
    );
    let include_values = Yaml::Array(
        include
            .iter()
            .map(|candidate| {
                yaml_mapping([("runner", Yaml::String(runner_label(*candidate).to_owned()))])
            })
            .collect(),
    );
    let matrix = yaml_mapping([("runner", axis_values), ("include", include_values)]);
    yaml_mapping([("strategy", yaml_mapping([("matrix", matrix)]))])
}

/// Builds each supported `runs-on` shape around the same matrix expression.
fn matrix_runs_on(shape: u8) -> Yaml {
    let expression = Yaml::String("${{ matrix.runner }}".to_owned());
    match shape {
        0 => expression,
        1 => Yaml::Array(vec![Yaml::String("self-hosted".to_owned()), expression]),
        _ => yaml_mapping([
            ("group", Yaml::String("hosted-pool".to_owned())),
            (
                "labels",
                Yaml::Array(vec![Yaml::String("self-hosted".to_owned()), expression]),
            ),
        ]),
    }
}

/// Constructs a YAML mapping for the private workflow-reader test inputs.
fn yaml_mapping(fields: impl IntoIterator<Item = (&'static str, Yaml)>) -> Yaml {
    Yaml::Hash(
        fields
            .into_iter()
            .map(|(name, value)| (Yaml::String(name.to_owned()), value))
            .collect::<Hash>(),
    )
}

/// A generated workflow step's effect on suite provisioning.
#[derive(Clone, Copy)]
enum WorkflowStep {
    /// Installs the required build tools.
    Installer,
    /// Runs a repository test suite.
    Suite,
    /// Does not install the tools or run the suite.
    Other,
}

/// Maps a generated tag to one of the workflow-step effects.
const fn workflow_step(tag: u8) -> WorkflowStep {
    match tag {
        0 => WorkflowStep::Installer,
        1 => WorkflowStep::Suite,
        _ => WorkflowStep::Other,
    }
}

/// Counts suite entries seen before any installer using a one-pass oracle.
fn missing_installers_before_suite(steps: &[WorkflowStep]) -> usize {
    let mut installer_seen = false;
    let mut missing = 0;
    for step in steps {
        match step {
            WorkflowStep::Installer => installer_seen = true,
            WorkflowStep::Suite if !installer_seen => missing += 1,
            WorkflowStep::Suite | WorkflowStep::Other => {}
        }
    }
    missing
}

/// Renders generated step effects into the minimal workflow accepted by the reader.
fn workflow_source(steps: &[WorkflowStep]) -> String {
    let rendered_steps = steps
        .iter()
        .map(|step| match step {
            WorkflowStep::Installer => "      - run: make install-build-tools",
            WorkflowStep::Suite => "      - run: cargo test --workspace",
            WorkflowStep::Other => "      - run: echo ready",
        })
        .collect::<Vec<_>>()
        .join("\n");
    format!(
        concat!(
            "name: generated\n",
            "on: push\n",
            "jobs:\n",
            "  test:\n",
            "    runs-on: ubuntu-latest\n",
            "    steps:\n",
            "{}\n",
        ),
        rendered_steps
    )
}
