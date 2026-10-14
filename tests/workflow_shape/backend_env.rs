//! Contracts for isolating stable release steps from caller backend settings.

use super::{read_workflow, step_and_run_lines};

/// Cargo's built-in profile and build-override backend selectors.
const BACKEND_SELECTORS: [&str; 10] = [
    "CARGO_ENCODED_RUSTFLAGS",
    "CARGO_UNSTABLE_CODEGEN_BACKEND",
    "CARGO_PROFILE_DEV_CODEGEN_BACKEND",
    "CARGO_PROFILE_DEV_BUILD_OVERRIDE_CODEGEN_BACKEND",
    "CARGO_PROFILE_TEST_CODEGEN_BACKEND",
    "CARGO_PROFILE_TEST_BUILD_OVERRIDE_CODEGEN_BACKEND",
    "CARGO_PROFILE_RELEASE_CODEGEN_BACKEND",
    "CARGO_PROFILE_RELEASE_BUILD_OVERRIDE_CODEGEN_BACKEND",
    "CARGO_PROFILE_BENCH_CODEGEN_BACKEND",
    "CARGO_PROFILE_BENCH_BUILD_OVERRIDE_CODEGEN_BACKEND",
];

/// The exact cleanup commands, in the order each release step executes them.
const CLEANUP_LINES: [&str; 5] = [
    "unset CARGO_ENCODED_RUSTFLAGS CARGO_UNSTABLE_CODEGEN_BACKEND",
    "unset CARGO_PROFILE_DEV_CODEGEN_BACKEND CARGO_PROFILE_DEV_BUILD_OVERRIDE_CODEGEN_BACKEND",
    "unset CARGO_PROFILE_TEST_CODEGEN_BACKEND CARGO_PROFILE_TEST_BUILD_OVERRIDE_CODEGEN_BACKEND",
    "unset CARGO_PROFILE_RELEASE_CODEGEN_BACKEND \
     CARGO_PROFILE_RELEASE_BUILD_OVERRIDE_CODEGEN_BACKEND",
    "unset CARGO_PROFILE_BENCH_CODEGEN_BACKEND CARGO_PROFILE_BENCH_BUILD_OVERRIDE_CODEGEN_BACKEND",
];

/// Returns the end of a unique, ordered cleanup block in one shell step.
pub(super) fn cleanup_end(run: &str) -> Option<usize> {
    let mut end = 0;
    for line in CLEANUP_LINES {
        if run.lines().filter(|candidate| *candidate == line).count() != 1 {
            return None;
        }
        end += run.get(end..)?.find(line)? + line.len();
    }
    Some(end)
}

/// No release workflow setting may reintroduce a cleaned backend selector.
fn only_cleanup_lines(workflow: &str) -> bool {
    let actual: Vec<_> = workflow
        .lines()
        .map(str::trim)
        .filter(|line| BACKEND_SELECTORS.iter().any(|name| line.contains(*name)))
        .collect();
    let expected: Vec<_> = CLEANUP_LINES.iter().copied().cycle().take(10).collect();
    actual == expected
}

/// Both stable-Cargo steps clear every selector before leaving the checkout.
#[test]
fn release_steps_clear_hostile_profile_overrides() {
    let workflow = read_workflow("release.yml").expect("read release workflow");
    assert!(
        only_cleanup_lines(&workflow),
        "backend selectors belong only in cleanup"
    );
    assert_step_level_backend_override_is_rejected(&workflow);
    for step in ["Install cross", "Build release binary"] {
        let (_, run_lines) =
            step_and_run_lines(&workflow, step).expect("release step has a run block");
        assert_step_clears_backend_selectors(&run_lines.join("\n"), step);
    }
    assert_release_omits_development_flags(&workflow);
}

/// A backend selector added to a release step's environment must fail review.
fn assert_step_level_backend_override_is_rejected(workflow: &str) {
    let injected = workflow.replacen(
        "          CARGO_TARGET_X86_64_UNKNOWN_LINUX_GNU_LINKER: gcc\n",
        "          CARGO_PROFILE_RELEASE_CODEGEN_BACKEND: cranelift\n          \
         CARGO_TARGET_X86_64_UNKNOWN_LINUX_GNU_LINKER: gcc\n",
        1,
    );
    assert_ne!(injected, workflow, "step-env mutation was a no-op");
    assert!(
        !only_cleanup_lines(&injected),
        "release accepted a step-level backend override"
    );
}

/// Every selector removal from a stable release step must be detected.
fn assert_step_clears_backend_selectors(run: &str, step: &str) {
    assert!(cleanup_end(run).is_some(), "{step} has no complete cleanup");
    for selector in BACKEND_SELECTORS {
        let mutated = run.replacen(selector, "REMOVED_BACKEND_SELECTOR", 1);
        assert_ne!(mutated, run, "{step}: {selector} mutation was a no-op");
        assert!(
            cleanup_end(&mutated).is_none(),
            "{step} accepted missing {selector}"
        );
    }
}

/// Stable releases must not inherit development-only linker or frontend flags.
fn assert_release_omits_development_flags(workflow: &str) {
    for flag in ["-Zthreads=8", "-Clink-arg=-fuse-ld=mold"] {
        assert!(
            !workflow.contains(flag),
            "release inherited development flag {flag}"
        );
    }
}
