//! Mutations that must violate the coverage and publisher contracts.

use rstest::rstest;

use super::super::{
    super::{changed, committed_workflows, rejected, replace_once},
    parse,
    triggers,
};

/// Covers the four supported representations of a pull-request trigger.
#[test]
fn scalar_sequence_mapping_and_boolean_on_are_recognized() {
    for (source, expected) in [
        ("on: pull_request", "pull_request"),
        ("on: [push, pull_request]", "pull_request"),
        ("on:\n  pull_request:\n", "pull_request"),
        ("true: pull_request", "pull_request"),
    ] {
        let workflow = parse(source).expect("parse trigger example");
        let names = triggers(&workflow).expect("read trigger example");
        assert!(
            names.contains(expected),
            "{source} did not contain {expected}"
        );
    }
}

/// Flow-style action env must not hide encoded flags from parsed inspection.
#[test]
fn flow_style_encoded_flags_are_rejected() -> Result<(), String> {
    let mut workflows = committed_workflows()?;
    let block = concat!(
        "        env:\n",
        "          CARGO_TARGET_X86_64_UNKNOWN_LINUX_GNU_LINKER: clang\n",
        "          CARGO_UNSTABLE_CODEGEN_BACKEND: 'true'\n",
        "          CARGO_PROFILE_DEV_CODEGEN_BACKEND: llvm\n",
        "          CARGO_PROFILE_DEV_BUILD_OVERRIDE_CODEGEN_BACKEND: llvm\n",
        "          CARGO_PROFILE_TEST_BUILD_OVERRIDE_CODEGEN_BACKEND: llvm\n",
        "          RUSTFLAGS: -C link-arg=-fuse-ld=lld\n",
        "          CFLAGS: -fuse-ld=lld\n",
        "          LDFLAGS: -fuse-ld=lld\n",
    );
    let flow = concat!(
        "        env: { CARGO_TARGET_X86_64_UNKNOWN_LINUX_GNU_LINKER: clang, ",
        "CARGO_UNSTABLE_CODEGEN_BACKEND: 'true', CARGO_PROFILE_DEV_CODEGEN_BACKEND: llvm, ",
        "CARGO_PROFILE_DEV_BUILD_OVERRIDE_CODEGEN_BACKEND: llvm, ",
        "CARGO_PROFILE_TEST_BUILD_OVERRIDE_CODEGEN_BACKEND: llvm, ",
        "RUSTFLAGS: '-C link-arg=-fuse-ld=lld', CFLAGS: -fuse-ld=lld, LDFLAGS: -fuse-ld=lld, ",
        "CARGO_ENCODED_RUSTFLAGS: '' }\n",
    );
    for name in ["ci.yml", "coverage-main.yml"] {
        let source = workflows
            .get(name)
            .ok_or_else(|| format!("missing {name}"))?;
        workflows.insert(name.into(), replace_once(source, block, flow));
    }
    rejected(&workflows, "LLVM backend");
    Ok(())
}

/// Both action lanes must retain LLVM for build dependencies and proc macros.
#[test]
fn build_override_backends_are_explicit_and_binding() -> Result<(), String> {
    let workflows = committed_workflows()?;
    super::validate(&workflows)?;
    for key in [
        "CARGO_PROFILE_DEV_BUILD_OVERRIDE_CODEGEN_BACKEND",
        "CARGO_PROFILE_TEST_BUILD_OVERRIDE_CODEGEN_BACKEND",
    ] {
        for replacement in ["", "cranelift"] {
            let mut mutated = workflows.clone();
            let original = format!("          {key}: llvm\n");
            let changed_line = if replacement.is_empty() {
                String::new()
            } else {
                format!("          {key}: {replacement}\n")
            };
            for name in ["ci.yml", "coverage-main.yml"] {
                let source = mutated.get(name).ok_or_else(|| format!("missing {name}"))?;
                mutated.insert(name.into(), replace_once(source, &original, &changed_line));
            }
            rejected(&mutated, "LLVM backend");
        }
    }
    Ok(())
}

/// A duplicate job key must never silently replace its earlier value.
#[test]
fn duplicate_yaml_mapping_keys_are_rejected() -> Result<(), String> {
    let workflows = changed("ci.yml", |source| {
        replace_once(
            source,
            "jobs:\n",
            "jobs:\n  duplicate: {}\n  duplicate: {}\n",
        )
    })?;
    rejected(&workflows, "ci.yml");
    Ok(())
}

#[rstest]
#[case::pr_guard("ci.yml", "if: false")]
#[case::pr_soft_failure("ci.yml", "continue-on-error: true")]
#[case::publisher_guard("coverage-main.yml", "if: false")]
#[case::publisher_soft_failure("coverage-main.yml", "continue-on-error: true")]
fn coverage_actions_cannot_skip_or_soft_fail(#[case] workflow: &str, #[case] guard: &str) {
    let workflows = changed(workflow, |source| {
        let original = "      - name: Test and Measure Coverage\n";
        let mutated = replace_once(source, original, &format!("{original}        {guard}\n"));
        assert_ne!(mutated, source, "coverage mutation must change the fixture");
        mutated
    })
    .expect("read workflow mutation fixture");
    rejected(&workflows, "coverage action must run unconditionally");
}

#[test]
fn publisher_permissions_reject_extra_write_scope() {
    let workflows = changed("coverage-main.yml", |source| {
        let original = "    permissions:\n      contents: read\n";
        let mutated = replace_once(
            source,
            original,
            &format!("{original}      actions: write\n"),
        );
        assert_ne!(
            mutated, source,
            "permission mutation must change the fixture"
        );
        mutated
    })
    .expect("read publisher permission fixture");
    rejected(&workflows, "publisher permissions must be contents: read");
}

/// The publisher must install build tools unconditionally before its suite.
#[test]
fn publisher_build_tool_step_must_run_before_coverage() -> Result<(), String> {
    let missing_install = changed("coverage-main.yml", |source| {
        source.replace(
            "      - name: Check build tools\n        run: make install-build-tools\n",
            "",
        )
    })?;
    rejected(&missing_install, "build-tool installation");
    let conditional_install = changed("coverage-main.yml", |source| {
        replace_once(
            source,
            "      - name: Check build tools\n        run: make install-build-tools",
            "      - name: Check build tools\n        if: always()\n        run: make \
             install-build-tools",
        )
    })?;
    rejected(&conditional_install, "build-tool installation");
    Ok(())
}

/// Encoded rustflags must not be allowed to override coverage's LLVM route.
#[test]
fn publisher_coverage_flag_isolation_is_binding() -> Result<(), String> {
    let original = "      - name: Check coverage flag isolation\n        run: |\n          if [ \
                    \"${CARGO_ENCODED_RUSTFLAGS+x}\" = x ]; then\n            echo \
                    'CARGO_ENCODED_RUSTFLAGS must be unset for coverage' >&2\n            exit \
                    1\n          fi\n";
    let missing = changed("coverage-main.yml", |source| {
        replace_once(source, original, "")
    })?;
    rejected(&missing, "flag isolation");
    let conditional = changed("coverage-main.yml", |source| {
        replace_once(
            source,
            original,
            &original.replace("        run: |", "        if: false\n        run: |"),
        )
    })?;
    rejected(&conditional, "flag isolation");
    let missing_pr_guard = changed("ci.yml", |source| replace_once(source, original, ""))?;
    rejected(&missing_pr_guard, "PR coverage flag isolation");
    Ok(())
}

/// Coverage inputs must match and the shared uploader must remain present.
#[test]
fn coverage_input_mismatch_and_missing_uploader_are_rejected() -> Result<(), String> {
    let unmatched_features = changed("coverage-main.yml", |source| {
        replace_once(
            source,
            "features: thysalion-demos/demo-empty",
            "features: thysalion-demos/demo-other",
        )
    })?;
    rejected(&unmatched_features, "coverage parity");
    let missing_uploader = changed("coverage-main.yml", |source| {
        replace_once(
            source,
            "/.github/actions/upload-codescene-coverage@",
            "/.github/actions/other@",
        )
    })?;
    rejected(&missing_uploader, "publisher");
    Ok(())
}

/// Manual dispatch must not write the baseline or use a development backend.
#[test]
fn dispatch_baseline_override_and_development_backend_are_rejected() -> Result<(), String> {
    let dispatch_writer = changed("coverage-main.yml", |source| {
        replace_once(
            source,
            "with-ratchet: 'true'",
            "with-ratchet: 'true'\n          publish-baseline: always",
        )
    })?;
    rejected(&dispatch_writer, "publisher baseline");
    let development_backend = changed("coverage-main.yml", |source| {
        replace_once(
            source,
            "CARGO_PROFILE_DEV_CODEGEN_BACKEND: llvm",
            "CARGO_PROFILE_DEV_CODEGEN_BACKEND: cranelift",
        )
    })?;
    rejected(&development_backend, "coverage parity");
    Ok(())
}

/// Matching PR and publisher linker drift must still fail the LLVM route.
#[test]
fn matching_lld_to_mold_drift_is_rejected() -> Result<(), String> {
    let mut workflows = committed_workflows()?;
    for name in ["ci.yml", "coverage-main.yml"] {
        let source = workflows
            .get(name)
            .ok_or_else(|| format!("missing {name}"))?;
        workflows.insert(
            name.into(),
            replace_once(
                source,
                "\n          RUSTFLAGS: -C link-arg=-fuse-ld=lld\n",
                "\n          RUSTFLAGS: -C link-arg=-fuse-ld=mold\n",
            ),
        );
    }
    rejected(&workflows, "LLVM backend");
    Ok(())
}

/// Matching full SHAs must still name the approved CV-005 action revision.
#[test]
fn matching_but_unapproved_coverage_action_pin_is_rejected() {
    let mut workflows = committed_workflows().expect("checked workflows must be readable");
    super::validate(&workflows).expect("checked workflows must satisfy the coverage contract");
    let approved = super::APPROVED_CV005_ACTION_REVISION;
    let unapproved = "a".repeat(40);
    for (name, expected_count) in [("ci.yml", 1), ("coverage-main.yml", 2)] {
        let source = workflows
            .get(name)
            .expect("both coverage workflows must exist");
        assert_eq!(
            source.matches(approved).count(),
            expected_count,
            "{name} must contain the expected CV-005 action references"
        );
        workflows.insert(name.into(), source.replace(approved, &unapproved));
    }
    rejected(&workflows, "unapproved CodeScene action pin");
}
