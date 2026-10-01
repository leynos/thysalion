//! Coverage ownership and `CodeScene` access contracts for all GitHub workflows.

/// Parses workflow YAML and checks publisher ownership and reachability.
#[path = "codescene_publisher/workflow.rs"]
mod workflow;

use std::collections::BTreeMap;

use cap_std::{ambient_authority, fs_utf8::Dir};

/// Reads every checked workflow, including a newly added `.yaml` file.
fn committed_workflows() -> Result<BTreeMap<String, String>, String> {
    let root = Dir::open_ambient_dir(env!("CARGO_MANIFEST_DIR"), ambient_authority())
        .map_err(|error| format!("open repository root: {error}"))?;
    let directory = root
        .open_dir(".github/workflows")
        .map_err(|error| format!("open workflows: {error}"))?;
    let mut workflows = BTreeMap::new();
    for result in directory
        .entries()
        .map_err(|error| format!("list workflows: {error}"))?
    {
        let entry = result.map_err(|error| format!("read workflow directory entry: {error}"))?;
        let filename = entry
            .file_name()
            .map_err(|error| format!("read workflow file name: {error}"))?;
        let name = filename.as_str();
        let is_yaml = name.rsplit_once('.').is_some_and(|(_, extension)| {
            extension.eq_ignore_ascii_case("yml") || extension.eq_ignore_ascii_case("yaml")
        });
        if is_yaml {
            workflows.insert(
                name.to_owned(),
                directory
                    .read_to_string(name)
                    .map_err(|error| format!("read {name}: {error}"))?,
            );
        }
    }
    Ok(workflows)
}

/// The checked-in workflows satisfy the combined publisher contract.
#[test]
fn committed_workflows_satisfy_cv005() -> Result<(), String> {
    workflow::validate(&committed_workflows()?)
}

/// Applies one in-memory mutation, such as removing a required uploader.
fn changed(
    name: &str,
    edit: impl FnOnce(&str) -> String,
) -> Result<BTreeMap<String, String>, String> {
    let mut workflows = committed_workflows()?;
    let original = workflows
        .get(name)
        .ok_or_else(|| format!("workflow fixture {name} is missing"))?;
    workflows.insert(name.to_owned(), edit(original));
    Ok(workflows)
}

/// Replaces one known fixture span and asserts that it was present.
fn replace_once(source: &str, old: &str, new: &str) -> String {
    assert!(source.contains(old), "mutation source must contain {old:?}");
    source.replacen(old, new, 1)
}

/// Asserts that a mutation fails for the intended contract reason.
#[track_caller]
fn rejected(workflows: &BTreeMap<String, String>, reason: &str) {
    let error = match workflow::validate(workflows) {
        Ok(()) => panic!("mutation must fail"),
        Err(error) => error,
    };
    assert!(error.contains(reason), "expected {reason:?}, got {error}");
}

/// Both string and boolean spellings of `on` in one file are ambiguous.
#[test]
fn ambiguous_trigger_keys_are_rejected() -> Result<(), String> {
    let workflows = changed("ci.yml", |source| {
        replace_once(source, "jobs:\n", "true: push\njobs:\n")
    })?;
    rejected(&workflows, "trigger");
    Ok(())
}

/// A direct PR token binding must fail the reachability check.
#[test]
fn pr_token_route_is_rejected() -> Result<(), String> {
    let workflows = changed("ci.yml", |source| {
        replace_once(
            source,
            "jobs:\n",
            "env:\n  TOKEN: ${{ secrets.CS_ACCESS_TOKEN }}\njobs:\n",
        )
    })?;
    rejected(&workflows, "PR-reachable");
    Ok(())
}

/// A PR-local reusable workflow inherits the same `CodeScene` prohibition.
#[test]
fn indirect_reusable_workflow_token_route_is_rejected() -> Result<(), String> {
    let mut workflows = changed("ci.yml", |source| {
        replace_once(
            source,
            "jobs:\n",
            "jobs:\n  indirect:\n    uses: ./.github/workflows/indirect.yml\n",
        )
    })?;
    workflows.insert(
        "indirect.yml".into(),
        concat!(
            "name: Indirect\non: workflow_call\njobs:\n  leak:\n    runs-on: ubuntu-latest\n    ",
            "steps:\n      - run: echo ${{ secrets.CS_ACCESS_TOKEN }}\n"
        )
        .into(),
    );
    rejected(&workflows, "PR-reachable");
    Ok(())
}

/// A qualified self-call cannot be assumed to run the checked local file.
#[test]
fn qualified_self_reusable_call_is_unprovable() -> Result<(), String> {
    let workflows = changed("ci.yml", |source| {
        replace_once(
            source,
            "jobs:\n",
            "jobs:\n  indirect:\n    uses: leynos/thysalion/.github/workflows/indirect.yml@main\n",
        )
    })?;
    rejected(&workflows, "unprovable reusable workflow call");
    Ok(())
}

/// A workflow-run successor of the PR CI cannot call the `CodeScene` CLI.
#[test]
fn workflow_run_chain_is_checked() -> Result<(), String> {
    let mut workflows = committed_workflows()?;
    workflows.insert(
        "indirect.yml".into(),
        concat!(
            "name: Indirect\non:\n  workflow_run:\n    workflows: [CI]\n    types: ",
            "[completed]\njobs:\n  leak:\n    runs-on: ubuntu-latest\n    steps:\n      - run: ",
            "cs-coverage upload\n"
        )
        .into(),
    );
    rejected(&workflows, "PR-reachable");
    Ok(())
}

/// Inherited and computed secret references cannot bypass the token check.
#[test]
fn inherited_or_computed_secrets_are_rejected() -> Result<(), String> {
    for route in [
        "secrets: inherit",
        "with:\n      token: ${{ secrets[format('CS_{0}', 'ACCESS_TOKEN')] }}",
        concat!(
            "with:\n      known: ${{ secrets.GITHUB_TOKEN }}\n      hidden: ${{ ",
            "secrets[env.TOKEN_NAME] }}"
        ),
        "run: echo '${{ toJSON(secrets) }}'",
    ] {
        let mut workflows = changed("ci.yml", |source| {
            replace_once(
                source,
                "jobs:\n",
                &format!(
                    "jobs:\n  leak:\n    uses: ./.github/workflows/indirect.yml\n    {route}\n"
                ),
            )
        })?;
        workflows.insert(
            "indirect.yml".into(),
            "name: Indirect\non: workflow_call\njobs: {}\n".into(),
        );
        rejected(&workflows, "PR-reachable");
    }
    Ok(())
}

/// Removal, renaming, or reordering of the canonical token check fails.
#[test]
fn missing_or_renamed_token_check_is_rejected() -> Result<(), String> {
    let missing_check = changed("coverage-main.yml", |source| {
        source.replace(
            concat!(
                "      - name: Check CodeScene token\n",
                "        id: codescene-token\n",
                "        run: echo ",
                "\"available=${{ secrets.CS_ACCESS_TOKEN != '' }}\" >> ",
                "\"$GITHUB_OUTPUT\"\n"
            ),
            "",
        )
    })?;
    rejected(&missing_check, "token check");
    let renamed_check = changed("coverage-main.yml", |source| {
        source.replace(
            "      - name: Check CodeScene token\n",
            "      - name: Check CodeScene token availability\n",
        )
    })?;
    rejected(&renamed_check, "token check");
    let wrong_id = changed("coverage-main.yml", |source| {
        replace_once(source, "id: codescene-token", "id: codescene_token")
    })?;
    rejected(&wrong_id, "token check");
    let wrong_command = changed("coverage-main.yml", |source| {
        replace_once(
            source,
            "run: echo \"available=${{ secrets.CS_ACCESS_TOKEN != '' }}\" >> \"$GITHUB_OUTPUT\"",
            "run: echo available=true",
        )
    })?;
    rejected(&wrong_command, "token check");
    let reordered_check = changed("coverage-main.yml", |source| {
        let check = concat!(
            "      - name: Check CodeScene token\n        id: codescene-token\n        ",
            "run: echo \"available=${{ secrets.CS_ACCESS_TOKEN != '' }}\" >> ",
            "\"$GITHUB_OUTPUT\"\n"
        );
        format!("{}\n{check}", source.replacen(check, "", 1).trim_end())
    })?;
    rejected(&reordered_check, "token check");
    Ok(())
}

/// The upload cannot inherit the token through env or bypass the main guard.
#[test]
fn upload_token_env_and_guard_bypass_are_rejected() -> Result<(), String> {
    let token_env = changed("coverage-main.yml", |source| {
        replace_once(source, "        with:\n          format: lcov\n          mode: upload", "        env:\n          TOKEN: ${{ secrets.CS_ACCESS_TOKEN }}\n        with:\n          format: lcov\n          mode: upload")
    })?;
    rejected(&token_env, "secret");
    let bypassed_guard = changed("coverage-main.yml", |source| {
        replace_once(
            source,
            "github.ref == 'refs/heads/main'",
            "github.ref == 'refs/heads/main' || github.event_name == 'workflow_dispatch'",
        )
    })?;
    rejected(&bypassed_guard, "guard");
    let direct_cli = changed("coverage-main.yml", |source| {
        replace_once(
            source,
            "      - name: Upload coverage data to CodeScene",
            "      - run: cs-coverage upload\n      - name: Upload coverage data to CodeScene",
        )
    })?;
    rejected(&direct_cli, "shared CodeScene uploader");
    let obsolete_checksum = changed("coverage-main.yml", |source| {
        replace_once(
            source,
            "          mode: upload",
            "          mode: upload\n          installer-checksum: obsolete",
        )
    })?;
    rejected(&obsolete_checksum, "upload secret/input");
    Ok(())
}

/// A second direct or reusable baseline writer and cancelling queue fail.
#[test]
fn cancellation_and_second_writer_are_rejected() -> Result<(), String> {
    let cancellation = changed("coverage-main.yml", |source| {
        replace_once(
            source,
            "cancel-in-progress: false",
            "cancel-in-progress: true",
        )
    })?;
    rejected(&cancellation, "concurrency");
    let mut duplicate_publisher = committed_workflows()?;
    let second = duplicate_publisher
        .get("coverage-main.yml")
        .ok_or("publisher fixture is missing")?
        .replace("name: Coverage (main)", "name: Second coverage");
    duplicate_publisher.insert("second.yml".into(), second);
    rejected(&duplicate_publisher, "publisher");
    let mut second_writer = committed_workflows()?;
    let pin = "d4d248bbbecdcf7b4f5bc79ffd4d6caee370bd79";
    second_writer.insert(
        "second.yml".into(),
        format!("name: Second coverage\non:\n  push:\n    branches: [main]\njobs:\n  build:\n    runs-on: ubuntu-latest\n    steps:\n      - uses: leynos/shared-actions/.github/actions/generate-coverage@{pin}\n        with:\n          with-ratchet: 'true'\n"),
    );
    rejected(&second_writer, "publisher");
    let mut indirect_writer = committed_workflows()?;
    indirect_writer.insert(
        "second.yml".into(),
        concat!(
            "name: Second coverage\non:\n  push:\n    branches: [main]\n",
            "jobs:\n  build:\n    uses: ",
            "./.github/workflows/writer.yml\n"
        )
        .into(),
    );
    indirect_writer.insert(
        "writer.yml".into(),
        format!("name: Writer\non: workflow_call\njobs:\n  build:\n    runs-on: ubuntu-latest\n    steps:\n      - uses: leynos/shared-actions/.github/actions/generate-coverage@{pin}\n        with:\n          with-ratchet: 'true'\n"),
    );
    rejected(&indirect_writer, "publisher");
    let mut dispatch_writer = committed_workflows()?;
    dispatch_writer.insert(
        "dispatch.yml".into(),
        format!("name: Dispatch writer\non: workflow_dispatch\njobs:\n  build:\n    runs-on: ubuntu-latest\n    steps:\n      - uses: leynos/shared-actions/.github/actions/generate-coverage@{pin}\n        with:\n          publish-baseline: always\n"),
    );
    rejected(&dispatch_writer, "publisher");
    Ok(())
}

/// Removing protection, moving it to PR CI, or deleting all workflows fails.
#[test]
fn environment_changes_and_empty_workflows_are_rejected() -> Result<(), String> {
    let missing_environment = changed("coverage-main.yml", |source| {
        replace_once(source, "    environment: codescene\n", "")
    })?;
    rejected(&missing_environment, "environment");
    let pr_environment = changed("ci.yml", |source| {
        replace_once(
            source,
            "    runs-on: ubuntu-latest\n",
            "    runs-on: ubuntu-latest\n    environment: codescene\n",
        )
    })?;
    rejected(&pr_environment, "PR-reachable");
    let computed_environment = changed("ci.yml", |source| {
        replace_once(
            source,
            "    runs-on: ubuntu-latest\n",
            concat!(
                "    runs-on: ubuntu-latest\n    environment: ${{ ",
                "format('{0}{1}', 'code', 'scene') ",
                "}}\n"
            ),
        )
    })?;
    rejected(&computed_environment, "unprovable");
    rejected(&BTreeMap::new(), "empty");
    Ok(())
}
