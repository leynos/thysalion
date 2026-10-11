//! Resolve static and matrix workflow runner labels to supported operating systems.
//! Matrix lookup keeps its validated axis and mapping together in a parser-local context.

use yaml_rust2::{Yaml, yaml::Hash};

use super::{WorkflowField, field};

/// A literal runner label from workflow YAML.
#[derive(Clone, Copy)]
struct RunnerLabel<'a>(&'a str);

/// The name of a matrix axis used by a runner expression.
#[derive(Clone, Copy)]
struct MatrixAxis<'a>(&'a str);

/// A parser-local context for one validated matrix and runner axis.
struct MatrixRunners<'matrix, 'axis> {
    /// The validated workflow matrix mapping.
    matrix: &'matrix Hash,
    /// The axis whose values select runner labels.
    axis: MatrixAxis<'axis>,
}

impl<'matrix, 'axis> MatrixRunners<'matrix, 'axis> {
    /// Validates a job's matrix mapping and retains the requested axis.
    fn new(job: &'matrix Yaml, axis: MatrixAxis<'axis>) -> Result<Self, String> {
        let matrix = field(job, WorkflowField::Strategy)
            .and_then(|strategy| field(strategy, WorkflowField::Matrix))
            .and_then(Yaml::as_hash)
            .ok_or_else(|| format!("matrix.{} has no matrix mapping", axis.0))?;
        if matrix.contains_key(&Yaml::String(WorkflowField::Exclude.as_str().to_owned())) {
            return Err(format!("matrix.{} uses unsupported exclusions", axis.0));
        }
        Ok(Self { matrix, axis })
    }

    /// Reads axis values followed by matching included values, preserving order.
    fn values(&self) -> Result<Vec<String>, String> {
        let mut found = self.axis_values()?;
        found.extend(self.included_axis_values()?);
        if found.is_empty() {
            return Err(format!(
                "matrix.{} has no literal or include values",
                self.axis.0
            ));
        }
        Ok(found)
    }

    /// Reads the literal values declared for the selected matrix axis.
    fn axis_values(&self) -> Result<Vec<String>, String> {
        let Some(values) = self.matrix.get(&Yaml::String(self.axis.0.to_owned())) else {
            return Ok(Vec::new());
        };
        values
            .as_vec()
            .ok_or_else(|| format!("matrix.{} is not a sequence", self.axis.0))?
            .iter()
            .map(|value| {
                value
                    .as_str()
                    .map(str::to_owned)
                    .ok_or_else(|| format!("matrix.{} has a non-string runner", self.axis.0))
            })
            .collect()
    }

    /// Reads matching runner values from matrix include rows.
    fn included_axis_values(&self) -> Result<Vec<String>, String> {
        let Some(include) = self
            .matrix
            .get(&Yaml::String(WorkflowField::Include.as_str().to_owned()))
        else {
            return Ok(Vec::new());
        };
        let rows = include
            .as_vec()
            .ok_or_else(|| "matrix.include is not a sequence".to_owned())?;
        let mut found = Vec::new();
        for row in rows {
            if let Some(value) = field(row, WorkflowField::Axis(self.axis.0)) {
                found.push(
                    value
                        .as_str()
                        .ok_or_else(|| format!("matrix.include {} is not a string", self.axis.0))?
                        .to_owned(),
                );
            }
        }
        Ok(found)
    }
}

/// The YAML sequence that supplied a list of runner labels.
#[derive(Clone, Copy)]
enum LabelSequence {
    RunsOn,
    RunnerLabels,
}

impl LabelSequence {
    const fn error(self) -> &'static str {
        match self {
            Self::RunsOn => "runs-on sequence has a non-string label",
            Self::RunnerLabels => "runs-on labels has a non-string value",
        }
    }
}

/// A runner label resolved either literally or from a matrix axis.
enum ResolvedRunnerLabel {
    Literal(String),
    Matrix(Vec<String>),
}

/// Resolves static, scalar, sequence, mapping, and one-axis matrix runners.
pub(super) fn runner_systems(job: &Yaml, runs_on: &Yaml) -> Result<bool, String> {
    let labels = runner_labels(runs_on)?;
    let mut static_labels = Vec::new();
    let mut matrix_labels = None;
    for label in labels {
        match resolve_runner_label(job, label)? {
            ResolvedRunnerLabel::Literal(literal) => static_labels.push(literal),
            ResolvedRunnerLabel::Matrix(values) => {
                if matrix_labels.replace(values).is_some() {
                    return Err("multiple matrix expressions in runs-on are unsupported".to_owned());
                }
            }
        }
    }
    let candidates = matrix_labels.map_or_else(
        || vec![None],
        |values| values.into_iter().map(Some).collect(),
    );
    candidates
        .into_iter()
        .try_fold(false, |has_linux, candidate| {
            let candidate_labels = candidate.map_or_else(
                || static_labels.clone(),
                |label| static_labels.iter().cloned().chain([label]).collect(),
            );
            classify_runner_labels(&candidate_labels)
                .map(|candidate_is_linux| has_linux || candidate_is_linux)
        })
}

/// Reads scalar, sequence, and mapping labels without accepting computed shapes.
fn runner_labels(runs_on: &Yaml) -> Result<Vec<RunnerLabel<'_>>, String> {
    match runs_on {
        Yaml::String(label) => Ok(vec![RunnerLabel(label)]),
        Yaml::Array(values) => sequence_labels(values, LabelSequence::RunsOn),
        Yaml::Hash(mapping) => mapping_labels(mapping),
        _ => Err("runs-on is neither a scalar, sequence, nor mapping".to_owned()),
    }
}

/// Reads labels from a runner mapping after validating its supported keys.
fn mapping_labels(mapping: &Hash) -> Result<Vec<RunnerLabel<'_>>, String> {
    if mapping.keys().any(|key| {
        key.as_str()
            .is_none_or(|name| !["group", "labels"].contains(&name))
    }) {
        return Err("runs-on mapping has unsupported keys".to_owned());
    }
    if mapping
        .get(&Yaml::String(WorkflowField::Group.as_str().to_owned()))
        .is_some_and(|group| group.as_str().is_none())
    {
        return Err("runs-on group is not a string".to_owned());
    }
    let labels = mapping
        .get(&Yaml::String(WorkflowField::Labels.as_str().to_owned()))
        .ok_or_else(|| "runs-on mapping has no labels".to_owned())?;
    match labels {
        Yaml::String(label) => Ok(vec![RunnerLabel(label)]),
        Yaml::Array(values) => sequence_labels(values, LabelSequence::RunnerLabels),
        _ => Err("runs-on labels is neither a string nor a sequence".to_owned()),
    }
}

/// Requires every label in a runner sequence to be a string.
fn sequence_labels(values: &[Yaml], source: LabelSequence) -> Result<Vec<RunnerLabel<'_>>, String> {
    values
        .iter()
        .map(|value| {
            value
                .as_str()
                .map(RunnerLabel)
                .ok_or_else(|| source.error().to_owned())
        })
        .collect()
}

/// Resolves a literal runner label or one complete `${{ matrix.axis }}` value.
fn resolve_runner_label(job: &Yaml, label: RunnerLabel<'_>) -> Result<ResolvedRunnerLabel, String> {
    let trimmed = label.0.trim();
    if !trimmed.contains("${{") {
        return Ok(ResolvedRunnerLabel::Literal(trimmed.to_owned()));
    }
    let expression = trimmed
        .strip_prefix("${{")
        .and_then(|value| value.strip_suffix("}}"))
        .map(str::trim)
        .ok_or_else(|| format!("unsupported runner expression {:?}", label.0))?;
    let axis = expression
        .strip_prefix("matrix.")
        .filter(|name| {
            !name.is_empty() && name.chars().all(|c| c.is_ascii_alphanumeric() || c == '_')
        })
        .ok_or_else(|| format!("unsupported runner expression {:?}", label.0))?;
    let runners = MatrixRunners::new(job, MatrixAxis(axis))?;
    Ok(ResolvedRunnerLabel::Matrix(runners.values()?))
}

/// Classifies one runner label set, rejecting sets without a known OS label.
fn classify_runner_labels(labels: &[String]) -> Result<bool, String> {
    let detected = labels
        .iter()
        .filter_map(|label| runner_os(RunnerLabel(label)))
        .try_fold(None, |detected, current| {
            merge_runner_os(detected, current, labels)
        })?;
    detected.ok_or_else(|| format!("runner labels do not identify an OS: {labels:?}"))
}

fn merge_runner_os(
    detected: Option<bool>,
    current: bool,
    labels: &[String],
) -> Result<Option<bool>, String> {
    match detected {
        Some(previous) if previous != current => {
            Err(format!("conflicting runner OS labels: {labels:?}"))
        }
        _ => Ok(Some(current)),
    }
}

fn runner_os(label: RunnerLabel<'_>) -> Option<bool> {
    let normalized = label.0.to_ascii_lowercase();
    if is_linux_label(RunnerLabel(&normalized)) {
        Some(true)
    } else if is_non_linux_label(RunnerLabel(&normalized)) {
        Some(false)
    } else {
        None
    }
}

/// Identifies Linux runner labels accepted by the workflow contract.
fn is_linux_label(label: RunnerLabel<'_>) -> bool {
    label.0 == "linux"
        || ["linux-", "ubuntu", "debian"]
            .iter()
            .any(|prefix| label.0.starts_with(prefix))
}

/// Identifies supported runners that must not count as Linux suite routes.
fn is_non_linux_label(label: RunnerLabel<'_>) -> bool {
    ["windows", "win-", "macos", "mac-", "darwin", "freebsd"]
        .iter()
        .any(|prefix| label.0.starts_with(prefix))
}
