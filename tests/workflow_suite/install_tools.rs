//! Strictly resolve workflow suite entrypoints and their Linux runner shapes.

use std::{collections::BTreeSet, fmt};

use yaml_rust2::{Yaml, YamlLoader};

#[path = "install_tools/runner.rs"]
mod runner;
#[path = "install_tools/tests.rs"]
mod tests;

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
#[derive(Default)]
struct StepRoute {
    is_suite: bool,
    is_installer: bool,
}

/// Names of supported workflow fields, with matrix axes kept dynamic.
#[derive(Clone, Copy)]
pub(super) enum WorkflowField<'a> {
    Jobs,
    Uses,
    Steps,
    RunsOn,
    Run,
    If,
    ContinueOnError,
    Strategy,
    Matrix,
    Exclude,
    Include,
    Group,
    Labels,
    Axis(&'a str),
}

impl<'a> WorkflowField<'a> {
    const fn as_str(self) -> &'a str {
        match self {
            Self::Jobs => "jobs",
            Self::Uses => "uses",
            Self::Steps => "steps",
            Self::RunsOn => "runs-on",
            Self::Run => "run",
            Self::If => "if",
            Self::ContinueOnError => "continue-on-error",
            Self::Strategy => "strategy",
            Self::Matrix => "matrix",
            Self::Exclude => "exclude",
            Self::Include => "include",
            Self::Group => "group",
            Self::Labels => "labels",
            Self::Axis(axis) => axis,
        }
    }
}

/// Looks up a workflow field from a YAML mapping.
pub(super) fn field<'a>(value: &'a Yaml, name: WorkflowField<'_>) -> Option<&'a Yaml> {
    value
        .as_hash()
        .and_then(|mapping| mapping.get(&Yaml::String(name.as_str().to_owned())))
}

/// Identifies a job in one workflow file for diagnostics.
#[derive(Clone, Copy)]
struct JobLocation<'a> {
    workflow: &'a str,
    name: &'a str,
}

impl fmt::Display for JobLocation<'_> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{}:{}", self.workflow, self.name)
    }
}

/// Identifies one step within a job for diagnostics.
#[derive(Clone, Copy)]
struct StepLocation<'a> {
    job: JobLocation<'a>,
    index: usize,
}

impl fmt::Display for StepLocation<'_> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{} step {}", self.job, self.index)
    }
}

/// A workflow shell command passed to literal route checks.
#[derive(Clone, Copy)]
struct WorkflowCommand<'a>(&'a str);

/// Suite and installer step positions gathered from one job.
#[derive(Default)]
struct StepRoutes {
    suite: Vec<usize>,
    installer: Vec<usize>,
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
        let jobs = field(&document, WorkflowField::Jobs)
            .and_then(Yaml::as_hash)
            .ok_or_else(|| format!("{workflow_name} has no jobs mapping"))?;
        if jobs.is_empty() {
            return Err(format!("{workflow_name} has an empty jobs mapping"));
        }
        for (job_key, job_value) in jobs {
            let job_name = job_key
                .as_str()
                .ok_or_else(|| format!("{workflow_name} has a non-string job key"))?;
            inspect_job(
                JobLocation {
                    workflow: workflow_name,
                    name: job_name,
                },
                job_value,
                &mut report,
            )?;
        }
    }
    if report.linux_suite_entries == 0 {
        return Err("workflow scan found no Linux suite entries".to_owned());
    }
    Ok(report)
}

#[path = "install_tools/inspection.rs"]
mod inspection;
use inspection::inspect_job;

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
