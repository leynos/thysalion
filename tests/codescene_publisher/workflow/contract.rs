//! Publisher ownership, coverage parity, and baseline-writer checks.

use std::collections::{BTreeMap, BTreeSet};

use yaml_rust2::{Yaml as Value, yaml::Hash as Mapping};

use super::{
    COVERAGE_ACTION,
    ISOLATION_COMMAND,
    TOKEN_COMMAND,
    UPLOAD_ACTION,
    UPLOAD_GUARD,
    action_step,
    check_extra_baseline,
    check_keys,
    check_pr_closure,
    document_text,
    environment_name,
    field,
    jobs,
    mapping,
    on_value,
    one_action,
    parse,
    pinned_action,
    reachable,
    required,
    sequence,
    steps,
    string,
    triggers,
};

/// Shared-actions revision whose coverage and uploader contract was reviewed.
const APPROVED_CV005_ACTION_REVISION: &str = "d4d248bbbecdcf7b4f5bc79ffd4d6caee370bd79";

/// Checks the publisher's main-only, protected upload and token sequence.
fn check_publisher(documents: &BTreeMap<String, Value>) -> Result<(), String> {
    let publisher = documents
        .get("coverage-main.yml")
        .ok_or("missing publisher workflow")?;
    check_publisher_shape(publisher)?;
    let job = jobs(publisher)?
        .values()
        .next()
        .ok_or("missing publisher job")?;
    let all_steps = steps(job)?;
    let coverage = one_action(publisher, COVERAGE_ACTION)?;
    let upload = one_action(publisher, UPLOAD_ACTION)?;
    check_publisher_provisioning(all_steps, coverage)?;
    check_token_upload(all_steps, upload)?;
    check_publisher_secrets(publisher, all_steps, coverage, upload)
}

/// Requires the main-only trigger, queue, environment and permissions.
fn check_publisher_shape(publisher: &Value) -> Result<(), String> {
    let events = triggers(publisher)?;
    let push = field(on_value(publisher)?, "push").ok_or("publisher needs push")?;
    let branches = sequence(required(push, "branches")?, "publisher branches")?;
    if (
        branches.len(),
        branches.first().and_then(Value::as_str),
        events.contains("push"),
    ) != (1, Some("main"), true)
    {
        return Err("publisher must push only from main".into());
    }
    let concurrency = required(publisher, "concurrency")?;
    if field(concurrency, "group").and_then(Value::as_str)
        != Some("${{ github.workflow }}-${{ github.ref }}")
        || field(concurrency, "cancel-in-progress").and_then(Value::as_bool) != Some(false)
    {
        return Err("publisher concurrency must queue per workflow and ref".into());
    }
    let publisher_jobs = jobs(publisher)?;
    if publisher_jobs.len() != 1 {
        return Err("publisher must have one job".into());
    }
    let job = publisher_jobs
        .values()
        .next()
        .ok_or("missing publisher job")?;
    if environment_name(job)? != Some("codescene") {
        return Err("publisher environment must be codescene".into());
    }
    let permissions = mapping(required(job, "permissions")?, "publisher permissions")?;
    if permissions.len() != 1
        || permissions
            .get(&Value::String("contents".into()))
            .and_then(Value::as_str)
            != Some("read")
    {
        return Err("publisher permissions must be contents: read".into());
    }
    Ok(())
}

/// Requires the installer and encoded-flag guard ahead of the suite.
fn check_publisher_provisioning(all_steps: &[Value], coverage: &Value) -> Result<(), String> {
    let tools_index = all_steps
        .iter()
        .position(|step| field(step, "name").and_then(Value::as_str) == Some("Check build tools"))
        .ok_or("missing publisher build-tool installation")?;
    let coverage_index = all_steps
        .iter()
        .position(|step| std::ptr::eq(step, coverage))
        .ok_or("missing publisher coverage")?;
    check_coverage_isolation(all_steps, coverage, "publisher")?;
    let tools = all_steps
        .get(tools_index)
        .ok_or("missing publisher build tools")?;
    if (
        tools_index < coverage_index,
        field(tools, "run").and_then(Value::as_str),
        field(tools, "if").is_none(),
        field(tools, "continue-on-error").is_none(),
    ) != (true, Some("make install-build-tools"), true, true)
    {
        return Err(
            "publisher build-tool installation must precede coverage unconditionally".into(),
        );
    }
    Ok(())
}

/// Requires an unconditional encoded-flags guard immediately before coverage.
fn check_coverage_isolation(
    all_steps: &[Value],
    coverage: &Value,
    lane: &str,
) -> Result<(), String> {
    let coverage_index = all_steps
        .iter()
        .position(|step| std::ptr::eq(step, coverage))
        .ok_or_else(|| format!("{lane}: missing coverage step"))?;
    let isolation_index = all_steps
        .iter()
        .position(|step| {
            field(step, "name").and_then(Value::as_str) == Some("Check coverage flag isolation")
        })
        .ok_or_else(|| format!("missing {lane} coverage flag isolation"))?;
    let isolation = all_steps
        .get(isolation_index)
        .ok_or_else(|| format!("missing {lane} coverage flag isolation"))?;
    if (
        isolation_index + 1 == coverage_index,
        field(isolation, "run").and_then(Value::as_str),
        field(isolation, "if").is_none(),
        field(isolation, "continue-on-error").is_none(),
    ) != (true, Some(ISOLATION_COMMAND), true, true)
    {
        return Err(format!(
            "{lane} coverage flag isolation must immediately precede coverage unconditionally"
        ));
    }
    Ok(())
}

/// Requires the exact token check and guarded, input-only upload.
fn check_token_upload(all_steps: &[Value], upload: &Value) -> Result<(), String> {
    let check_index = all_steps
        .iter()
        .position(|step| {
            field(step, "name").and_then(Value::as_str) == Some("Check CodeScene token")
        })
        .ok_or("missing token check")?;
    let check = all_steps.get(check_index).ok_or("missing token check")?;
    if (
        mapping(check, "token check")?.len(),
        field(check, "id").and_then(Value::as_str),
        field(check, "run").and_then(Value::as_str),
    ) != (3, Some("codescene-token"), Some(TOKEN_COMMAND))
    {
        return Err("token check must have the exact name, id and sole command".into());
    }
    let upload_index = all_steps
        .iter()
        .position(|step| std::ptr::eq(step, upload))
        .ok_or("missing publisher upload")?;
    if check_index >= upload_index {
        return Err("token check must precede upload".into());
    }
    if field(upload, "if").and_then(Value::as_str) != Some(UPLOAD_GUARD) {
        return Err("upload guard must require token and exact main ref".into());
    }
    if field(upload, "env").is_some() {
        return Err("upload secret must not enter env".into());
    }
    let inputs = required(upload, "with")?;
    if (
        field(inputs, "access-token").and_then(Value::as_str),
        field(inputs, "mode").and_then(Value::as_str),
        field(inputs, "format").and_then(Value::as_str),
        field(inputs, "installer-checksum").is_none(),
    ) != (
        Some("${{ secrets.CS_ACCESS_TOKEN }}"),
        Some("upload"),
        Some("lcov"),
        true,
    ) {
        return Err("upload secret/input contract is invalid".into());
    }
    Ok(())
}

/// Rejects parallel secret routes and pins the checkout and shared actions.
fn check_publisher_secrets(
    publisher: &Value,
    all_steps: &[Value],
    coverage: &Value,
    upload: &Value,
) -> Result<(), String> {
    let text = document_text(publisher);
    let lower = text.to_ascii_lowercase();
    if lower.contains("cs-coverage") || lower.contains("codescene.io") {
        return Err("publisher must use only the shared CodeScene uploader".into());
    }
    if text.matches("CS_ACCESS_TOKEN").count() != 2 || lower.matches("secrets").count() != 2 {
        return Err("publisher secret must have exactly two uses".into());
    }
    let checkout = all_steps
        .iter()
        .find(|step| {
            field(step, "uses")
                .and_then(Value::as_str)
                .is_some_and(|uses| uses.starts_with("actions/checkout@"))
        })
        .ok_or("publisher checkout missing")?;
    if field(required(checkout, "with")?, "persist-credentials").and_then(Value::as_bool)
        != Some(false)
    {
        return Err("publisher checkout must not persist credentials".into());
    }
    let upload_pin = pinned_action(upload, UPLOAD_ACTION)?;
    let coverage_pin = pinned_action(coverage, COVERAGE_ACTION)?;
    if upload_pin != coverage_pin {
        return Err("publisher action pins must agree".into());
    }
    if upload_pin != APPROVED_CV005_ACTION_REVISION {
        return Err("unapproved CodeScene action pin".into());
    }
    Ok(())
}

/// Extracts comparable coverage inputs and compiler environment from a step.
fn measurement(step: &Value) -> Result<(String, Mapping, Mapping), String> {
    let pin = pinned_action(step, COVERAGE_ACTION)?;
    if pin != APPROVED_CV005_ACTION_REVISION {
        return Err("unapproved CodeScene action pin".into());
    }
    let inputs = mapping(required(step, "with")?, "coverage inputs")?;
    let mut comparable = inputs.clone();
    for input in ["with-ratchet", "publish-artefact", "publish-baseline"] {
        comparable.remove(&Value::String(input.into()));
    }
    let env = mapping(required(step, "env")?, "coverage env")?.clone();
    Ok((pin, comparable, env))
}

/// Compares PR and main measurements, then checks their distinct publication roles.
fn check_coverage(documents: &BTreeMap<String, Value>) -> Result<(), String> {
    let publisher = documents
        .get("coverage-main.yml")
        .ok_or("missing publisher")?;
    let ci = documents.get("ci.yml").ok_or("missing PR lane")?;
    let main_step = one_action(publisher, COVERAGE_ACTION)?;
    let pr_step = one_action(ci, COVERAGE_ACTION)?;
    for (lane, step) in [("PR", pr_step), ("publisher", main_step)] {
        if field(step, "if").is_some() || field(step, "continue-on-error").is_some() {
            return Err(format!(
                "{lane} coverage action must run unconditionally and fail on error"
            ));
        }
    }
    for job in jobs(ci)?.values() {
        if field(job, "uses").is_some() {
            continue;
        }
        let all_steps = steps(job)?;
        if all_steps.iter().any(|step| std::ptr::eq(step, pr_step)) {
            check_coverage_isolation(all_steps, pr_step, "PR")?;
        }
    }
    if measurement(main_step)? != measurement(pr_step)? {
        return Err("coverage parity differs between PR and publisher".into());
    }
    let inputs = required(pr_step, "with")?;
    if field(inputs, "with-ratchet").and_then(Value::as_str) != Some("true")
        || field(inputs, "publish-artefact").and_then(Value::as_str) != Some("false")
    {
        return Err("PR coverage ratchet or artefact setting is wrong".into());
    }
    let main_inputs = required(main_step, "with")?;
    if (
        field(main_inputs, "with-ratchet").and_then(Value::as_str),
        field(main_inputs, "publish-baseline")
            .and_then(Value::as_str)
            .is_none_or(|value| value == "auto"),
        field(main_inputs, "publish-artefact").and_then(Value::as_str) != Some("false"),
    ) != (Some("true"), true, true)
    {
        return Err("publisher baseline and ratchet settings are wrong".into());
    }
    let main_env = required(main_step, "env")?;
    if [
        field(main_env, "CARGO_PROFILE_DEV_CODEGEN_BACKEND").and_then(Value::as_str)
            == Some("llvm"),
        field(main_env, "CARGO_PROFILE_DEV_BUILD_OVERRIDE_CODEGEN_BACKEND").and_then(Value::as_str)
            == Some("llvm"),
        field(
            main_env,
            "CARGO_PROFILE_TEST_BUILD_OVERRIDE_CODEGEN_BACKEND",
        )
        .and_then(Value::as_str)
            == Some("llvm"),
        field(main_env, "CARGO_UNSTABLE_CODEGEN_BACKEND").and_then(Value::as_str) == Some("true"),
        field(main_env, "CARGO_ENCODED_RUSTFLAGS").is_none(),
        field(main_env, "CARGO_TARGET_X86_64_UNKNOWN_LINUX_GNU_LINKER").and_then(Value::as_str)
            == Some("clang"),
        field(main_env, "RUSTFLAGS").and_then(Value::as_str) == Some("-C link-arg=-fuse-ld=lld"),
        field(main_env, "CFLAGS").and_then(Value::as_str) == Some("-fuse-ld=lld"),
        field(main_env, "LDFLAGS").and_then(Value::as_str) == Some("-fuse-ld=lld"),
    ]
    .contains(&false)
    {
        return Err("coverage must select the LLVM backend".into());
    }
    Ok(())
}

/// Validates a complete workflow set; an empty map cannot satisfy the contract.
pub fn validate(workflows: &BTreeMap<String, String>) -> Result<(), String> {
    if workflows.is_empty() {
        return Err("empty workflow set".into());
    }
    let mut documents = BTreeMap::new();
    for (name, source) in workflows {
        let document =
            parse(source).map_err(|error| format!("{name}: invalid or duplicate YAML: {error}"))?;
        mapping(&document, name)?;
        check_keys(&document, true).map_err(|error| format!("{name}: {error}"))?;
        triggers(&document).map_err(|error| format!("{name}: {error}"))?;
        documents.insert(name.clone(), document);
    }
    let closure = reachable(&documents)?;
    check_pr_closure(&documents, &closure)?;
    let mut publishers = 0;
    let mut baseline_writers = 0;
    let mut environments = 0;
    for (file, document) in &documents {
        publishers += action_step(document, UPLOAD_ACTION)?.len();
        check_extra_baseline(file, document)?;
        if file != "coverage-main.yml" {
            let text = document_text(document).to_ascii_lowercase();
            if ["codescene", "cs-coverage", "cs_access_token"]
                .iter()
                .any(|marker| text.contains(marker))
            {
                return Err(format!("{file}: extra CodeScene route outside publisher"));
            }
        }
        if triggers(document)?.contains("push") {
            baseline_writers +=
                baseline::baseline_writes_from(file, &documents, &mut BTreeSet::new())?;
        }
        for job in jobs(document)?.values() {
            environments += usize::from(environment_name(job)? == Some("codescene"));
        }
    }
    if publishers != 1 {
        return Err("publisher must be unique and present".into());
    }
    if baseline_writers != 1 {
        return Err("publisher must be the only baseline writer".into());
    }
    if environments != 1 {
        return Err("codescene environment must be on publisher job only".into());
    }
    check_publisher(&documents)?;
    check_coverage(&documents)?;
    Ok(())
}

#[path = "contract/baseline.rs"]
mod baseline;

#[cfg(test)]
#[path = "contract/tests.rs"]
mod tests;
