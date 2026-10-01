//! Verify that this repository does not request Cranelift from Whitaker.

use yaml_rust2::{Yaml, YamlLoader};

use super::reading::manifest_dir;

/// The shared installer action, independent of its Dependabot-managed pin.
const INSTALLER: &str = "leynos/shared-actions/.github/actions/install-whitaker@";

/// Looks up a named key in a YAML mapping.
fn field<'a>(value: &'a Yaml, name: &str) -> Option<&'a Yaml> {
    value.as_hash()?.get(&Yaml::String(name.to_owned()))
}

/// Requires exactly one installer step and no Cranelift input in it.
fn inspect(source: &str) -> Result<(), String> {
    let documents = YamlLoader::load_from_str(source).map_err(|error| error.to_string())?;
    let [workflow] = documents.as_slice() else {
        return Err("CI must contain one YAML document".to_owned());
    };
    let steps = field(workflow, "jobs")
        .and_then(|jobs| field(jobs, "build-test"))
        .and_then(|job| field(job, "steps"))
        .and_then(Yaml::as_vec)
        .ok_or("build-test steps are missing")?;
    let installers: Vec<&Yaml> = steps
        .iter()
        .filter(|step| {
            field(step, "uses")
                .and_then(Yaml::as_str)
                .is_some_and(|action| action.starts_with(INSTALLER))
        })
        .collect();
    let [installer] = installers.as_slice() else {
        return Err("CI must install Whitaker exactly once".to_owned());
    };
    if let Some(inputs) = field(installer, "with") {
        let input_mapping = inputs
            .as_hash()
            .ok_or("Whitaker inputs must be a mapping")?;
        if input_mapping.contains_key(&Yaml::String("cranelift".to_owned())) {
            return Err("Thysalion must not request Whitaker Cranelift".to_owned());
        }
    }
    Ok(())
}

#[test]
fn ci_does_not_request_whitaker_cranelift() {
    let source = manifest_dir()
        .expect("failed to open the repository")
        .read_to_string(".github/workflows/ci.yml")
        .expect("failed to read CI workflow");
    assert!(
        inspect(&source).is_ok(),
        "Whitaker must use its LLVM default"
    );
}

#[test]
fn explicit_whitaker_cranelift_inputs_are_rejected() {
    let source = manifest_dir()
        .expect("failed to open the repository")
        .read_to_string(".github/workflows/ci.yml")
        .expect("failed to read CI workflow");
    let installer_line = source
        .lines()
        .find(|line| line.contains("uses: ") && line.contains(INSTALLER))
        .expect("failed to find Whitaker installer");
    for input in [
        "with:\n          cranelift: 'true'",
        "with: { cranelift: true }",
    ] {
        let replacement = format!("{installer_line}\n        {input}");
        let mutation = source.replacen(installer_line, &replacement, 1);
        assert_ne!(mutation, source, "the mutation must change CI");
        assert!(
            inspect(&mutation).is_err(),
            "explicit Whitaker Cranelift input must fail the contract"
        );
    }
}
