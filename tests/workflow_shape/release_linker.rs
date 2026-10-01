//! Contracts for release linker selection in Cross build containers.

use super::read_workflow;

// Cargo's exact-triple environment setting must win over the Linux cfg linker.
fn release_build_uses_cross_linkers(workflow: &str) -> Result<(), &'static str> {
    let (_, build_and_later_steps) = workflow
        .split_once("      - name: Build release binary\n")
        .ok_or("release build step is missing")?;
    let (build_step, _) = build_and_later_steps
        .split_once("      - name: Prepare artifact\n")
        .ok_or("artifact step is missing after release build")?;
    let (_, environment_and_run) = build_step
        .split_once("        env:\n")
        .ok_or("release build environment is missing")?;
    let (environment, _) = environment_and_run
        .split_once("        run:")
        .ok_or("release build command is missing")?;

    for (name, required) in [
        (
            "CARGO_TARGET_X86_64_UNKNOWN_LINUX_GNU_LINKER",
            "CARGO_TARGET_X86_64_UNKNOWN_LINUX_GNU_LINKER: gcc",
        ),
        (
            "CARGO_TARGET_AARCH64_UNKNOWN_LINUX_GNU_LINKER",
            "CARGO_TARGET_AARCH64_UNKNOWN_LINUX_GNU_LINKER: aarch64-linux-gnu-gcc",
        ),
    ] {
        let overrides: Vec<_> = environment
            .lines()
            .map(str::trim)
            .filter(|line| line.starts_with(name))
            .collect();
        if overrides.len() != 1 || overrides.first() != Some(&required) {
            return Err("release build lacks a unique Cross image linker override");
        }
    }
    Ok(())
}

#[test]
fn release_build_overrides_the_development_linux_linker() {
    let workflow = read_workflow("release.yml").expect("read release.yml");
    assert!(
        include_str!("../../.cargo/config.toml")
            .contains("[target.'cfg(target_os = \"linux\")']\nlinker = \"clang\""),
        "development Linux linker configuration changed"
    );
    assert_eq!(
        release_build_uses_cross_linkers(&workflow),
        Ok(()),
        "release build must use the Cross image linker overrides"
    );
}

#[test]
fn release_build_linker_contract_rejects_removed_or_wrong_overrides() {
    let workflow = read_workflow("release.yml").expect("read release.yml");
    for (configured, invalid) in [
        ("CARGO_TARGET_X86_64_UNKNOWN_LINUX_GNU_LINKER: gcc", ""),
        (
            "CARGO_TARGET_AARCH64_UNKNOWN_LINUX_GNU_LINKER: aarch64-linux-gnu-gcc",
            "CARGO_TARGET_AARCH64_UNKNOWN_LINUX_GNU_LINKER: clang",
        ),
    ] {
        let mutated = workflow.replacen(configured, invalid, 1);
        assert_ne!(
            mutated, workflow,
            "test mutation did not change the workflow"
        );
        assert!(
            release_build_uses_cross_linkers(&mutated).is_err(),
            "release linker contract accepted an invalid override: {invalid}"
        );
    }
}
