//! Strictly resolve workflow suite entrypoints and their Linux runner shapes.

use std::collections::BTreeSet;

use yaml_rust2::{Yaml, YamlLoader};

use crate::reading::Command;

#[path = "install_tools/runner.rs"]
mod runner;
#[path = "install_tools/tests.rs"]
mod tests;

use runner::runner_systems;

/// The shared action runs the test suite as part of coverage generation.
const COVERAGE_ACTION: &str = "leynos/shared-actions/.github/actions/generate-coverage@";

/// The required unconditional build-tools provisioning target.
const INSTALL_TARGET: &str = "install-build-tools";

/// Workflow suite paths and Linux entries lacking earlier provisioning.
#[derive(Debug)]
pub(crate) struct Report {
    /// Number of suite steps whose runner can include Linux.
    pub(crate) linux_suite_entries: usize,
    /// Workflow/job/step paths without an unconditional earlier installer.
    pub(crate) missing_installers: Vec<String>,
}

/// Whether a step runs the suite and whether it provisions build tools.
struct StepRoute {
    is_suite: bool,
    is_installer: bool,
}

/// Inspects every workflow and fails closed on incomplete workflow structure.
pub(crate) fn inspect(files: &[(String, String)]) -> Result<Report, String> {
    if files.is_empty() {
        return Err("no workflow files were discovered".to_owned());
    }

    let mut report = Report {
        linux_suite_entries: 0,
        missing_installers: Vec::new(),
    };
    let mut seen_workflows = BTreeSet::new();
    for (workflow_name, source) in files {
        if !seen_workflows.insert(workflow_name) {
            return Err(format!("duplicate workflow file name {workflow_name:?}"));
        }
        let mut documents = YamlLoader::load_from_str(source)
            .map_err(|error| format!("cannot parse {workflow_name}: {error}"))?;
        if documents.len() != 1 {
            return Err(format!(
                "{workflow_name} has {} YAML documents; expected one",
                documents.len()
            ));
        }
        let document = documents
            .pop()
            .ok_or_else(|| format!("{workflow_name} has no YAML document"))?;
        let jobs = field(&document, "jobs")
            .and_then(Yaml::as_hash)
            .ok_or_else(|| format!("{workflow_name} has no jobs mapping"))?;
        if jobs.is_empty() {
            return Err(format!("{workflow_name} has an empty jobs mapping"));
        }
        for (job_key, job_value) in jobs {
            let job_name = job_key
                .as_str()
                .ok_or_else(|| format!("{workflow_name} has a non-string job key"))?;
            inspect_job(workflow_name, job_name, job_value, &mut report)?;
        }
    }
    if report.linux_suite_entries == 0 {
        return Err("workflow scan found no Linux suite entries".to_owned());
    }
    Ok(report)
}

/// Inspects one job while retaining its runner and step ordering.
fn inspect_job(
    workflow: &str,
    name: &str,
    value: &Yaml,
    report: &mut Report,
) -> Result<(), String> {
    let location = format!("{workflow}:{name}");
    let job = value
        .as_hash()
        .ok_or_else(|| format!("{location} is not a job mapping"))?;
    if field(value, "uses").is_some() {
        return Err(format!("{location} reusable workflow path is unresolved"));
    }
    let steps = field(value, "steps")
        .and_then(Yaml::as_vec)
        .ok_or_else(|| format!("{location} has no steps sequence"))?;
    if steps.is_empty() {
        return Err(format!("{location} has empty steps"));
    }
    let mut suite_steps = Vec::new();
    let mut installer_steps = Vec::new();
    for (index, step) in steps.iter().enumerate() {
        let route = classify_step(step, job, &format!("{location} step {index}"))?;
        if route.is_suite {
            suite_steps.push(index);
        }
        if route.is_installer {
            installer_steps.push(index);
        }
    }
    if suite_steps.is_empty() {
        return Ok(());
    }
    let runners = field(value, "runs-on").ok_or_else(|| format!("{location} has no runs-on"))?;
    let has_linux_runner =
        runner_systems(value, runners).map_err(|error| format!("{location}: {error}"))?;
    if !has_linux_runner {
        return Ok(());
    }
    for suite_step in suite_steps {
        report.linux_suite_entries += 1;
        if installer_steps.iter().all(|index| *index >= suite_step) {
            report
                .missing_installers
                .push(format!("{location}:step-{suite_step}"));
        }
    }
    Ok(())
}

/// Classifies one step, rejecting routes the literal reader cannot resolve.
fn classify_step(
    step_value: &Yaml,
    job: &yaml_rust2::yaml::Hash,
    location: &str,
) -> Result<StepRoute, String> {
    let step = step_value
        .as_hash()
        .ok_or_else(|| format!("{location} is not a mapping"))?;
    let run = optional_text(step, "run", location)?;
    let uses = optional_text(step, "uses", location)?;
    if run.is_some() && uses.is_some() {
        return Err(format!("{location} has both run and uses"));
    }
    let mut route = StepRoute {
        is_suite: false,
        is_installer: false,
    };
    if let Some(action) = uses {
        if action.starts_with("./") {
            return Err(format!("{location} local action unresolved"));
        }
        route.is_suite = action.starts_with(COVERAGE_ACTION);
    }
    if let Some(command) = run {
        route.is_suite = Command::from_line(command).runs_suite();
        if !route.is_suite && has_unresolved_suite_route(command) {
            return Err(format!("{location} suite command is unresolved"));
        }
        route.is_installer = is_unconditional_installer(step, job, command);
    }
    Ok(route)
}

/// Looks up a string key in a YAML value.
fn field<'a>(value: &'a Yaml, name: &str) -> Option<&'a Yaml> {
    value
        .as_hash()
        .and_then(|mapping| mapping_field(mapping, name))
}

/// Looks up a string key in a YAML mapping.
fn mapping_field<'a>(mapping: &'a yaml_rust2::yaml::Hash, name: &str) -> Option<&'a Yaml> {
    mapping.get(&Yaml::String(name.to_owned()))
}

/// Reads an optional workflow-step text field without accepting other YAML types.
fn optional_text<'a>(
    step: &'a yaml_rust2::yaml::Hash,
    key: &str,
    location: &str,
) -> Result<Option<&'a str>, String> {
    mapping_field(step, key)
        .map(|value| {
            value
                .as_str()
                .ok_or_else(|| format!("{location} field {key} is not a string"))
        })
        .transpose()
}

/// Refuses computed commands and shell or local-script routes whose suite use is unprovable.
fn has_unresolved_suite_route(command: &str) -> bool {
    let computed =
        command.contains("${{") && (command.contains("make") || command.contains("cargo"));
    computed
        || command
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
    command: &str,
) -> bool {
    let is_unconditional = mapping_field(step, "if").is_none()
        && mapping_field(job, "if").is_none()
        && matches!(
            mapping_field(step, "continue-on-error"),
            None | Some(Yaml::Boolean(false))
        );
    is_unconditional && direct_installer_command(command)
}

/// Recognizes a direct Make target line while refusing shell control flow.
fn direct_installer_command(command: &str) -> bool {
    if ["&&", "||", "|", ";"]
        .iter()
        .any(|operator| command.contains(operator))
    {
        return false;
    }
    let controls = [
        "if ", "then ", "else", "fi", "for ", "while ", "until ", "case ", "exit ", "return ",
        "exec ",
    ];
    if command.lines().any(|line| {
        controls
            .iter()
            .any(|prefix| line.trim_start().starts_with(prefix))
    }) {
        return false;
    }
    command.lines().any(line_runs_installer)
}

/// Returns whether one shell line invokes `make install-build-tools` directly.
fn line_runs_installer(line: &str) -> bool {
    let uncommented = line.split('#').next().unwrap_or_default();
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

const INSTALL_ROUTING_FIXTURE: &str = include_str!("install_tools/fixtures/install_routing.yml");

pub(super) const MINIMAL_INSTALL_ROUTING: &str = r"
name: routes
on: push
jobs:
  test:
    runs-on: ubuntu-latest
    steps:
      - name: Install build tools
        run: make install-build-tools
      - name: Run suite
        run: make test
";

pub(super) fn inspect_routing(source: &str) -> Result<Report, String> {
    inspect(&[("fixture.yml".to_owned(), source.to_owned())])
}
