//! Counts `CodeScene` baseline writers reachable through push workflows.

use std::collections::{BTreeMap, BTreeSet};

use yaml_rust2::Yaml as Value;

use super::{COVERAGE_ACTION, action_step, field, jobs, required, string};

/// Counts baseline writers reachable from a push, including local reusable calls.
pub(super) fn baseline_writes_from(
    file: &str,
    documents: &BTreeMap<String, Value>,
    visiting: &mut BTreeSet<String>,
) -> Result<usize, String> {
    if !visiting.insert(file.to_owned()) {
        return Err(format!("{file}: reusable workflow cycle"));
    }
    let document = documents
        .get(file)
        .ok_or_else(|| format!("{file}: missing workflow"))?;
    let mut writes = 0;
    for step in action_step(document, COVERAGE_ACTION)? {
        let with = required(step, "with")?;
        if field(with, "publish-baseline").and_then(Value::as_str) != Some("false") {
            writes += 1;
        }
    }
    for job in jobs(document)?.values() {
        let Some(uses) = field(job, "uses") else {
            continue;
        };
        let target = string(uses, "push reusable workflow call")?;
        let path = target
            .strip_prefix("./.github/workflows/")
            .ok_or_else(|| format!("{file}: unprovable push reusable workflow call {target}"))?;
        writes += baseline_writes_from(path, documents, visiting)?;
    }
    visiting.remove(file);
    Ok(writes)
}
