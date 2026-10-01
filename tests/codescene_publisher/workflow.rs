//! Strict YAML and reachability checks for the main coverage publisher.

use std::collections::{BTreeMap, BTreeSet};

use yaml_rust2::{Yaml as Value, YamlLoader, yaml::Hash as Mapping};

/// Prefix selecting the shared measurement action at any full-SHA revision.
const COVERAGE_ACTION: &str = "leynos/shared-actions/.github/actions/generate-coverage@";
/// Prefix selecting the shared `CodeScene` uploader at any full-SHA revision.
const UPLOAD_ACTION: &str = "leynos/shared-actions/.github/actions/upload-codescene-coverage@";
/// The token check emits only a boolean, not the token itself.
const TOKEN_COMMAND: &str =
    "echo \"available=${{ secrets.CS_ACCESS_TOKEN != '' }}\" >> \"$GITHUB_OUTPUT\"";
/// The upload needs both token availability and the exact permitted ref.
const UPLOAD_GUARD: &str =
    "steps.codescene-token.outputs.available == 'true' && github.ref == 'refs/heads/main'";
/// A set encoded-flags value overrides the coverage step's plain `RUSTFLAGS`.
const ISOLATION_COMMAND: &str = concat!(
    "if [ \"${CARGO_ENCODED_RUSTFLAGS+x}\" = x ]; then\n",
    "  echo 'CARGO_ENCODED_RUSTFLAGS must be unset for coverage' >&2\n",
    "  exit 1\n",
    "fi\n",
);

/// Looks up a YAML mapping member, such as `jobs` on a workflow document.
fn field<'a>(value: &'a Value, name: &str) -> Option<&'a Value> {
    value.as_hash()?.get(&Value::String(name.into()))
}

/// Requires a member: a missing `jobs` key becomes an error.
fn required<'a>(value: &'a Value, name: &str) -> Result<&'a Value, String> {
    field(value, name).ok_or_else(|| format!("missing {name}"))
}

/// Requires a scalar string, such as an action's `uses` value.
fn string<'a>(value: &'a Value, label: &str) -> Result<&'a str, String> {
    value
        .as_str()
        .ok_or_else(|| format!("{label} must be a string"))
}

/// Requires a mapping, such as a workflow's `jobs` value.
fn mapping<'a>(value: &'a Value, label: &str) -> Result<&'a Mapping, String> {
    value
        .as_hash()
        .ok_or_else(|| format!("{label} must be a mapping"))
}

/// Rejects a non-string key such as `jobs: {true: ...}` outside the root `on` alias.
fn check_keys(value: &Value, is_root: bool) -> Result<(), String> {
    match value {
        Value::Hash(entries) => {
            for (key, child) in entries {
                let is_root_on = is_root && key == &Value::Boolean(true);
                if key.as_str().is_none() && !is_root_on {
                    return Err("non-string YAML mapping key".into());
                }
                check_keys(child, false)?;
            }
        }
        Value::Array(values) => {
            for child in values {
                check_keys(child, false)?;
            }
        }
        Value::Alias(_) | Value::BadValue => {
            return Err("unsupported YAML value is indeterminate".into());
        }
        _ => {}
    }
    Ok(())
}

/// Requires a sequence, such as a job's `steps` value.
fn sequence<'a>(value: &'a Value, label: &str) -> Result<&'a [Value], String> {
    value
        .as_vec()
        .map(Vec::as_slice)
        .ok_or_else(|| format!("{label} must be a sequence"))
}

/// Selects exactly one `on` key, including parsers that render it as `true`.
fn on_value(document: &Value) -> Result<&Value, String> {
    let root = mapping(document, "workflow")?;
    let on = root.get(&Value::String("on".into()));
    let boolean_on = root.get(&Value::Boolean(true));
    if on.is_some() == boolean_on.is_some() {
        return Err("ambiguous or missing trigger key".into());
    }
    on.or(boolean_on).ok_or_else(|| "missing trigger".into())
}

/// Reads scalar, list, or map events: `on: push` yields `push`.
fn triggers(document: &Value) -> Result<BTreeSet<String>, String> {
    let value = on_value(document)?;
    let mut names = BTreeSet::new();
    match value {
        Value::String(name) => {
            names.insert(name.clone());
        }
        Value::Array(values) => {
            for event in values {
                names.insert(string(event, "trigger")?.to_owned());
            }
        }
        Value::Hash(events) => {
            for key in events.keys() {
                names.insert(string(key, "trigger key")?.to_owned());
            }
        }
        _ => return Err("indeterminate trigger representation".into()),
    }
    if names.is_empty() {
        return Err("empty trigger set".into());
    }
    Ok(names)
}

/// Requires the workflow's jobs mapping; an absent mapping is indeterminate.
fn jobs(document: &Value) -> Result<&Mapping, String> {
    mapping(required(document, "jobs")?, "jobs")
}

/// Reads either `environment: codescene` or its named mapping form.
fn environment_name(job: &Value) -> Result<Option<&str>, String> {
    let Some(environment) = field(job, "environment") else {
        return Ok(None);
    };
    let name = match environment {
        Value::String(name) => name.as_str(),
        Value::Hash(_) => {
            let name = field(environment, "name").ok_or("environment mapping needs a name")?;
            string(name, "environment name")?
        }
        _ => return Err("environment must be a name or mapping".into()),
    };
    if name.contains("${{") {
        return Err("computed environment is unprovable".into());
    }
    Ok(Some(name))
}

/// Reads a normal job's step list; reusable-call jobs have no steps.
fn steps(job: &Value) -> Result<&[Value], String> { sequence(required(job, "steps")?, "steps") }

/// Finds direct calls to an action, such as `generate-coverage`, in normal jobs.
fn action_step<'a>(document: &'a Value, action: &str) -> Result<Vec<&'a Value>, String> {
    let job_steps = jobs(document)?
        .values()
        .filter(|job| field(job, "uses").is_none())
        .map(steps)
        .collect::<Result<Vec<_>, _>>()?;
    Ok(job_steps
        .into_iter()
        .flatten()
        .filter(|step| {
            field(step, "uses")
                .and_then(Value::as_str)
                .is_some_and(|uses| uses.starts_with(action))
        })
        .collect())
}

/// Requires exactly one matching action; two uploaders are an error.
fn one_action<'a>(document: &'a Value, action: &str) -> Result<&'a Value, String> {
    let found = action_step(document, action)?;
    if found.len() != 1 {
        return Err(format!("publisher or PR lane must have one {action} step"));
    }
    found
        .into_iter()
        .next()
        .ok_or_else(|| "missing action".into())
}

/// Returns the full SHA from a shared action reference such as `...@0123…`.
fn pinned_action(step: &Value, prefix: &str) -> Result<String, String> {
    let uses = string(required(step, "uses")?, "action uses")?;
    let sha = uses.strip_prefix(prefix).ok_or("wrong action path")?;
    if sha.len() != 40 || !sha.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return Err(format!("action {uses} must use a full SHA"));
    }
    Ok(sha.to_owned())
}

/// Reads `workflow_run.workflows` as one name or a list of names.
fn names(value: &Value) -> Result<BTreeSet<String>, String> {
    let names = match value {
        Value::String(name) => Ok(BTreeSet::from([name.clone()])),
        Value::Array(values) => values
            .iter()
            .map(|entry| string(entry, "workflow name").map(str::to_owned))
            .collect(),
        _ => Err("workflow_run.workflows must name workflows".into()),
    }?;
    if names.is_empty() {
        return Err("workflow_run.workflows must not be empty".into());
    }
    Ok(names)
}

/// Loads exactly one YAML document; the loader refuses duplicate mapping keys.
fn parse(source: &str) -> Result<Value, String> {
    let mut documents = YamlLoader::load_from_str(source).map_err(|error| error.to_string())?;
    if documents.len() != 1 {
        return Err("expected exactly one YAML document".into());
    }
    documents
        .pop()
        .ok_or_else(|| "missing YAML document".into())
}

/// Includes mapping keys and scalar values in the route scan without losing YAML types.
fn document_text(value: &Value) -> String {
    fn collect(value: &Value, text: &mut String) {
        match value {
            Value::Hash(entries) => {
                for (key, child) in entries {
                    collect(key, text);
                    collect(child, text);
                }
            }
            Value::Array(values) => {
                for child in values {
                    collect(child, text);
                }
            }
            Value::String(content) => {
                text.push_str(content);
                text.push('\n');
            }
            _ => {}
        }
    }
    let mut text = String::new();
    collect(value, &mut text);
    text
}

/// Adds local calls and workflow-run successors to the PR entrypoint set.
fn reachable(documents: &BTreeMap<String, Value>) -> Result<BTreeSet<String>, String> {
    let mut closure = BTreeSet::new();
    let mut by_name = BTreeMap::new();
    for (file, document) in documents {
        let name = string(required(document, "name")?, "workflow name")?;
        if by_name.insert(name.to_owned(), file.clone()).is_some() {
            return Err(format!("duplicate workflow name {name}"));
        }
        let events = triggers(document)?;
        if events.contains("pull_request") || events.contains("pull_request_target") {
            closure.insert(file.clone());
        }
    }
    if closure.is_empty() {
        return Err("empty PR-reachable workflow set".into());
    }
    loop {
        let before = closure.len();
        for (file, document) in documents {
            include_workflow_run(file, document, &by_name, &mut closure)?;
        }
        let current: Vec<String> = closure.iter().cloned().collect();
        for file in current {
            let document = documents.get(&file).ok_or("missing reachable workflow")?;
            include_local_calls(&file, document, documents, &mut closure)?;
        }
        if closure.len() == before {
            break;
        }
    }
    Ok(closure)
}

/// Adds a workflow-run successor when its named predecessor is PR-reachable.
fn include_workflow_run(
    file: &str,
    document: &Value,
    by_name: &BTreeMap<String, String>,
    closure: &mut BTreeSet<String>,
) -> Result<(), String> {
    if !triggers(document)?.contains("workflow_run") {
        return Ok(());
    }
    let run = field(on_value(document)?, "workflow_run");
    let callers = run
        .and_then(|trigger| field(trigger, "workflows"))
        .ok_or_else(|| format!("{file}: unprovable workflow_run"))?;
    for name in names(callers)? {
        let called = by_name
            .get(&name)
            .ok_or_else(|| format!("{file}: unknown workflow_run source {name}"))?;
        if closure.contains(called) {
            closure.insert(file.to_owned());
        }
    }
    Ok(())
}

/// Adds locally called reusable workflows and refuses indeterminate calls.
fn include_local_calls(
    file: &str,
    document: &Value,
    documents: &BTreeMap<String, Value>,
    closure: &mut BTreeSet<String>,
) -> Result<(), String> {
    for job in jobs(document)?.values() {
        let Some(uses) = field(job, "uses") else {
            continue;
        };
        let target = string(uses, "reusable workflow call")?;
        let path = target
            .strip_prefix("./.github/workflows/")
            .ok_or_else(|| format!("{file}: unprovable reusable workflow call {target}"))?;
        if !documents.contains_key(path) {
            return Err(format!("{file}: missing local reusable workflow {path}"));
        }
        closure.insert(path.to_owned());
    }
    Ok(())
}

/// Rejects token and `CodeScene` routes in any PR-reachable workflow.
fn check_pr_closure(
    documents: &BTreeMap<String, Value>,
    closure: &BTreeSet<String>,
) -> Result<(), String> {
    for file in closure {
        let document = documents.get(file).ok_or("missing PR workflow")?;
        let text = document_text(document).to_ascii_lowercase();
        let forbidden = [
            "codescene",
            "cs-coverage",
            "cs_access_token",
            "tojson(secrets)",
        ];
        let other_secret_access = text.replace("secrets.github_token", "").contains("secrets");
        if forbidden.iter().any(|marker| text.contains(marker)) || other_secret_access {
            return Err(format!("{file}: PR-reachable CodeScene or secret route"));
        }
        for job in jobs(document)?.values() {
            if environment_name(job)? == Some("codescene") {
                return Err(format!("{file}: PR-reachable protected environment"));
            }
        }
    }
    Ok(())
}

/// Refuses baseline writers outside the single approved main publisher.
fn check_extra_baseline(file: &str, document: &Value) -> Result<(), String> {
    if file == "coverage-main.yml" {
        return Ok(());
    }
    for step in action_step(document, COVERAGE_ACTION)? {
        let baseline = field(required(step, "with")?, "publish-baseline").and_then(Value::as_str);
        let is_allowed = matches!((file, baseline), ("ci.yml", None) | (_, Some("false")));
        if !is_allowed {
            return Err(format!("{file}: extra publisher baseline writer"));
        }
    }
    Ok(())
}

/// Publisher policy composed from the strict reader above.
#[path = "workflow/contract.rs"]
mod contract;
pub use contract::validate;
