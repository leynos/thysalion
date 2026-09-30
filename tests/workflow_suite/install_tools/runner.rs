//! Resolve static and matrix workflow runner labels to supported operating systems.

use yaml_rust2::Yaml;

use super::field;

/// Resolves scalar, sequence, mapping, and one-axis matrix runner forms.
pub(super) fn runner_systems(job: &Yaml, runs_on: &Yaml) -> Result<bool, String> {
    let labels = runner_labels(runs_on)?;
    let mut static_labels = Vec::new();
    let mut matrix_labels = None;
    for label in labels {
        let (is_matrix, resolved) = resolve_runner_label(job, label)?;
        if is_matrix {
            if matrix_labels.replace(resolved).is_some() {
                return Err("multiple matrix expressions in runs-on are unsupported".to_owned());
            }
        } else {
            static_labels.push(label.to_owned());
        }
    }
    let candidates = matrix_labels.unwrap_or_else(|| vec![String::new()]);
    let mut has_linux = false;
    for candidate in candidates {
        let mut candidate_labels = static_labels.clone();
        if !candidate.is_empty() {
            candidate_labels.push(candidate);
        }
        has_linux |= classify_runner_labels(&candidate_labels)?;
    }
    Ok(has_linux)
}

/// Reads scalar, sequence, and mapping labels without accepting computed shapes.
fn runner_labels(runs_on: &Yaml) -> Result<Vec<&str>, String> {
    match runs_on {
        Yaml::String(label) => Ok(vec![label.as_str()]),
        Yaml::Array(values) => sequence_labels(values, "runs-on sequence has a non-string label"),
        Yaml::Hash(mapping) => mapping_labels(mapping),
        _ => Err("runs-on is neither a scalar, sequence, nor mapping".to_owned()),
    }
}

/// Reads labels from a runner mapping after validating its supported keys.
fn mapping_labels(mapping: &yaml_rust2::yaml::Hash) -> Result<Vec<&str>, String> {
    if mapping.keys().any(|key| {
        key.as_str()
            .is_none_or(|name| !["group", "labels"].contains(&name))
    }) {
        return Err("runs-on mapping has unsupported keys".to_owned());
    }
    if mapping
        .get(&Yaml::String("group".to_owned()))
        .is_some_and(|group| group.as_str().is_none())
    {
        return Err("runs-on group is not a string".to_owned());
    }
    let labels = mapping
        .get(&Yaml::String("labels".to_owned()))
        .ok_or_else(|| "runs-on mapping has no labels".to_owned())?;
    match labels {
        Yaml::String(label) => Ok(vec![label.as_str()]),
        Yaml::Array(values) => sequence_labels(values, "runs-on labels has a non-string value"),
        _ => Err("runs-on labels is neither a string nor a sequence".to_owned()),
    }
}

/// Requires every label in a sequence to be a string.
fn sequence_labels<'a>(values: &'a [Yaml], error: &str) -> Result<Vec<&'a str>, String> {
    values
        .iter()
        .map(|value| value.as_str().ok_or_else(|| error.to_owned()))
        .collect()
}

/// Resolves a literal runner label or one complete `${{ matrix.axis }}` value.
fn resolve_runner_label(job: &Yaml, label: &str) -> Result<(bool, Vec<String>), String> {
    let trimmed = label.trim();
    if !trimmed.contains("${{") {
        return Ok((false, vec![trimmed.to_owned()]));
    }
    let expression = trimmed
        .strip_prefix("${{")
        .and_then(|value| value.strip_suffix("}}"))
        .map(str::trim)
        .ok_or_else(|| format!("unsupported runner expression {label:?}"))?;
    let axis = expression
        .strip_prefix("matrix.")
        .filter(|name| {
            !name.is_empty() && name.chars().all(|c| c.is_ascii_alphanumeric() || c == '_')
        })
        .ok_or_else(|| format!("unsupported runner expression {label:?}"))?;
    Ok((true, matrix_values(job, axis)?))
}

/// Reads literal matrix-axis and include values; excludes are rejected as ambiguous.
fn matrix_values(job: &Yaml, axis: &str) -> Result<Vec<String>, String> {
    let matrix = field(job, "strategy")
        .and_then(|strategy| field(strategy, "matrix"))
        .and_then(Yaml::as_hash)
        .ok_or_else(|| format!("matrix.{axis} has no matrix mapping"))?;
    if matrix.contains_key(&Yaml::String("exclude".to_owned())) {
        return Err(format!("matrix.{axis} uses unsupported exclusions"));
    }
    let mut found = Vec::new();
    if let Some(axis_values) = matrix.get(&Yaml::String(axis.to_owned())) {
        let values = axis_values
            .as_vec()
            .ok_or_else(|| format!("matrix.{axis} is not a sequence"))?;
        found.extend(
            values
                .iter()
                .map(|value| {
                    value
                        .as_str()
                        .map(str::to_owned)
                        .ok_or_else(|| format!("matrix.{axis} has a non-string runner"))
                })
                .collect::<Result<Vec<_>, _>>()?,
        );
    }
    if let Some(include) = matrix.get(&Yaml::String("include".to_owned())) {
        let rows = include
            .as_vec()
            .ok_or_else(|| "matrix.include is not a sequence".to_owned())?;
        for row in rows {
            if let Some(value) = field(row, axis) {
                found.push(
                    value
                        .as_str()
                        .ok_or_else(|| format!("matrix.include {axis} is not a string"))?
                        .to_owned(),
                );
            }
        }
    }
    if found.is_empty() {
        return Err(format!("matrix.{axis} has no literal or include values"));
    }
    Ok(found)
}

/// Classifies one runner label set, rejecting sets without a known OS label.
fn classify_runner_labels(labels: &[String]) -> Result<bool, String> {
    let mut detected = None;
    for label in labels {
        let normalized = label.to_ascii_lowercase();
        let current = if is_linux_label(&normalized) {
            Some(true)
        } else if is_non_linux_label(&normalized) {
            Some(false)
        } else {
            None
        };
        if let Some(is_linux) = current {
            if detected.is_some_and(|prior| prior != is_linux) {
                return Err(format!("conflicting runner OS labels: {labels:?}"));
            }
            detected = Some(is_linux);
        }
    }
    detected.ok_or_else(|| format!("runner labels do not identify an OS: {labels:?}"))
}

/// Identifies Linux runner labels accepted by the workflow contract.
fn is_linux_label(label: &str) -> bool {
    label == "linux"
        || ["linux-", "ubuntu", "debian"]
            .iter()
            .any(|prefix| label.starts_with(prefix))
}

/// Identifies supported runners that must not count as Linux suite routes.
fn is_non_linux_label(label: &str) -> bool {
    ["windows", "win-", "macos", "mac-", "darwin", "freebsd"]
        .iter()
        .any(|prefix| label.starts_with(prefix))
}
