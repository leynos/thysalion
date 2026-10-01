//! Inspect one job's suite and installer routes while preserving diagnostics.

use yaml_rust2::Yaml;

use super::{
    COVERAGE_ACTION,
    INSTALL_TARGET,
    JobLocation,
    Report,
    StepLocation,
    StepRoute,
    StepRoutes,
    WorkflowCommand,
    WorkflowField,
    field,
    runner::runner_systems,
};
use crate::reading::Command;

/// Inspects one job while retaining its runner and step ordering.
pub(super) fn inspect_job(
    location: JobLocation<'_>,
    value: &Yaml,
    report: &mut Report,
) -> Result<(), String> {
    let job = job_mapping(value, location)?;
    let steps = job_steps(value, location)?;
    let routes = classify_steps(steps, job, location)?;
    if routes.suite.is_empty() {
        return Ok(());
    }
    let runners =
        field(value, WorkflowField::RunsOn).ok_or_else(|| format!("{location} has no runs-on"))?;
    let has_linux_runner =
        runner_systems(value, runners).map_err(|error| format!("{location}: {error}"))?;
    if has_linux_runner {
        record_linux_suite_entries(location, routes, report);
    }
    Ok(())
}

fn job_mapping<'a>(
    value: &'a Yaml,
    location: JobLocation<'_>,
) -> Result<&'a yaml_rust2::yaml::Hash, String> {
    let job = value
        .as_hash()
        .ok_or_else(|| format!("{location} is not a job mapping"))?;
    if field(value, WorkflowField::Uses).is_some() {
        return Err(format!("{location} reusable workflow path is unresolved"));
    }
    Ok(job)
}

fn job_steps<'a>(value: &'a Yaml, location: JobLocation<'_>) -> Result<&'a [Yaml], String> {
    let steps = field(value, WorkflowField::Steps)
        .and_then(Yaml::as_vec)
        .ok_or_else(|| format!("{location} has no steps sequence"))?;
    if steps.is_empty() {
        return Err(format!("{location} has empty steps"));
    }
    Ok(steps)
}

fn classify_steps(
    steps: &[Yaml],
    job: &yaml_rust2::yaml::Hash,
    location: JobLocation<'_>,
) -> Result<StepRoutes, String> {
    let mut routes = StepRoutes::default();
    for (index, step) in steps.iter().enumerate() {
        let step_route = classify_step(
            step,
            job,
            StepLocation {
                job: location,
                index,
            },
        )?;
        if step_route.is_suite {
            routes.suite.push(index);
        }
        if step_route.is_installer {
            routes.installer.push(index);
        }
    }
    Ok(routes)
}

fn record_linux_suite_entries(location: JobLocation<'_>, routes: StepRoutes, report: &mut Report) {
    report.linux_suite_entries += routes.suite.len();
    for suite_step in routes.suite {
        if routes.installer.iter().all(|index| *index >= suite_step) {
            report
                .missing_installers
                .push(format!("{location}:step-{suite_step}"));
        }
    }
}

/// Classifies one step, rejecting routes the literal reader cannot resolve.
fn classify_step(
    step_value: &Yaml,
    job: &yaml_rust2::yaml::Hash,
    location: StepLocation<'_>,
) -> Result<StepRoute, String> {
    let step = step_value
        .as_hash()
        .ok_or_else(|| format!("{location} is not a mapping"))?;
    let run = optional_text(step, WorkflowField::Run, location)?;
    let uses = optional_text(step, WorkflowField::Uses, location)?;
    match (run, uses) {
        (Some(_), Some(_)) => Err(format!("{location} has both run and uses")),
        (Some(command), None) => classify_run_step(step, job, WorkflowCommand(command), location),
        (None, Some(action)) if action.starts_with("./") => {
            Err(format!("{location} local action unresolved"))
        }
        (None, Some(action)) => Ok(StepRoute {
            is_suite: action.starts_with(COVERAGE_ACTION),
            is_installer: false,
        }),
        (None, None) => Ok(StepRoute::default()),
    }
}

fn classify_run_step(
    step: &yaml_rust2::yaml::Hash,
    job: &yaml_rust2::yaml::Hash,
    command: WorkflowCommand<'_>,
    location: StepLocation<'_>,
) -> Result<StepRoute, String> {
    let is_suite = Command::from_line(command.0).runs_suite();
    if !is_suite && has_unresolved_suite_route(command) {
        return Err(format!("{location} suite command is unresolved"));
    }
    Ok(StepRoute {
        is_suite,
        is_installer: is_unconditional_installer(step, job, command),
    })
}

/// Looks up a string key in a YAML mapping.
fn mapping_field<'a>(
    mapping: &'a yaml_rust2::yaml::Hash,
    name: WorkflowField<'_>,
) -> Option<&'a Yaml> {
    mapping.get(&Yaml::String(name.as_str().to_owned()))
}

/// Reads an optional workflow-step text field without accepting other YAML types.
fn optional_text<'a>(
    step: &'a yaml_rust2::yaml::Hash,
    key: WorkflowField<'_>,
    location: StepLocation<'_>,
) -> Result<Option<&'a str>, String> {
    mapping_field(step, key)
        .map(|value| {
            value
                .as_str()
                .ok_or_else(|| format!("{location} field {} is not a string", key.as_str()))
        })
        .transpose()
}

/// Refuses computed commands and shell or local-script routes whose suite use is unprovable.
fn has_unresolved_suite_route(command: WorkflowCommand<'_>) -> bool {
    let command_text = command.0;
    let computed = command_text.contains("${{")
        && (command_text.contains("make") || command_text.contains("cargo"));
    computed
        || command_text
            .split(['\n', ';', '&', '|'])
            .flat_map(str::split_whitespace)
            .any(|word| {
                word.starts_with("./")
                    || word
                        .rsplit_once('.')
                        .is_some_and(|(_, extension)| extension.eq_ignore_ascii_case("sh"))
                    || matches!(word, "bash" | "sh" | "dash" | "zsh" | "source" | ".")
            })
}

/// Accepts only a standalone, unguarded Make invocation of the installer target.
fn is_unconditional_installer(
    step: &yaml_rust2::yaml::Hash,
    job: &yaml_rust2::yaml::Hash,
    command: WorkflowCommand<'_>,
) -> bool {
    let is_unconditional = mapping_field(step, WorkflowField::If).is_none()
        && mapping_field(job, WorkflowField::If).is_none()
        && matches!(
            mapping_field(step, WorkflowField::ContinueOnError),
            None | Some(Yaml::Boolean(false))
        );
    is_unconditional && direct_installer_command(command)
}

/// Recognizes a direct Make target line while refusing shell control flow.
fn direct_installer_command(command: WorkflowCommand<'_>) -> bool {
    let command_text = command.0;
    if ["&&", "||", "|", ";"]
        .iter()
        .any(|operator| command_text.contains(operator))
    {
        return false;
    }
    let controls = [
        "if ", "then ", "else", "fi", "for ", "while ", "until ", "case ", "exit ", "return ",
        "exec ",
    ];
    if command_text.lines().any(|line| {
        controls
            .iter()
            .any(|prefix| line.trim_start().starts_with(prefix))
    }) {
        return false;
    }
    command_text
        .lines()
        .any(|line| line_runs_installer(WorkflowCommand(line)))
}

/// Returns whether one shell line invokes `make install-build-tools` directly.
fn line_runs_installer(line: WorkflowCommand<'_>) -> bool {
    let line_text = line.0;
    let uncommented = line_text.split('#').next().unwrap_or_default();
    let words: Vec<_> = uncommented
        .split_whitespace()
        .map(|word| word.trim_matches(|character| character == '\'' || character == '"'))
        .collect();
    if words
        .first()
        .is_none_or(|word| word.rsplit('/').next() != Some("make"))
    {
        return false;
    }
    let value_options = [
        "-C",
        "-f",
        "-I",
        "-j",
        "-o",
        "-W",
        "--directory",
        "--file",
        "--jobs",
        "--makefile",
    ];
    let mut index = 1;
    while let Some(word) = words.get(index) {
        if value_options.contains(word) {
            index += 2;
        } else if word.starts_with('-') || word.contains('=') {
            index += 1;
        } else {
            return *word == INSTALL_TARGET;
        }
    }
    false
}
