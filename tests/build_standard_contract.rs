//! Contract tests verify evaluated Cargo and Make backend, flags, exclusions and gate ordering.
//! Helpers live in a child module so this test file stays below the 400-line policy limit.

#[path = "build_standard_contract/fmt_route.rs"]
mod fmt_route;
#[path = "build_standard_contract/helpers.rs"]
mod helpers;
#[path = "build_standard_contract/stable_cross.rs"]
mod stable_cross;

use std::process::Command;

use helpers::{
    LINUX_TABLES,
    MOLD_FLAG,
    PROFILE_BACKEND_SELECTORS,
    THREADS_FLAG,
    build_override_mutations,
    cargo_lines,
    coverage_mutations,
    coverage_route_matches,
    development_route_matches,
    make_output,
    names,
    preflight_precedes_cargo,
    release_route_matches,
    sources,
    whitaker_route_matches,
};

/// Cargo defaults retain the parallel frontend and Linux-only mold.
#[test]
fn cargo_defaults_keep_platform_flags() {
    let found = sources().expect("read Cargo rustflag sources");
    assert!(
        found.iter().any(|(key, _)| key == "build"),
        "Cargo rustflags must come from the build table"
    );
    let missing: Vec<_> = found
        .iter()
        .filter(|(_, flags)| !names(flags, THREADS_FLAG))
        .map(|(key, _)| key)
        .collect();
    assert!(
        missing.is_empty(),
        "{THREADS_FLAG} missing from {missing:?}"
    );
    let linux: Vec<_> = found
        .iter()
        .filter(|(key, _)| LINUX_TABLES.contains(&key.as_str()))
        .collect();
    assert!(!linux.is_empty(), "no Linux rustflags source carries mold");
    assert!(
        linux.iter().all(|(_, flags)| names(flags, MOLD_FLAG)),
        "Linux rustflags sources must select mold"
    );
    let wider: Vec<_> = found
        .iter()
        .filter(|(key, flags)| !LINUX_TABLES.contains(&key.as_str()) && names(flags, MOLD_FLAG))
        .map(|(key, _)| key)
        .collect();
    assert!(
        wider.is_empty(),
        "mold is configured beyond Linux: {wider:?}"
    );
    let unique_sources: std::collections::BTreeSet<Vec<&String>> = found
        .iter()
        .map(|(_, flags)| flags.iter().filter(|flag| *flag != MOLD_FLAG).collect())
        .collect();
    assert_eq!(unique_sources.len(), 1, "rustflag sources disagree");
}

/// Evaluated dev routes keep flags and preflight with caller LLVM overrides.
#[test]
fn development_make_routes_restate_flags_and_run_preflight_first() {
    for host in ["Linux", "Darwin"] {
        for (target, expected) in [
            ("build", 1),
            ("typecheck", 1),
            ("lint-clippy", 2),
            ("demo", 1),
        ] {
            let output = make_output(
                target,
                &[
                    "CARGO=probe-cargo",
                    "DEMO=empty",
                    &format!("BUILD_HOST_OS={host}"),
                ],
            )
            .expect("evaluate Make target");
            let commands = cargo_lines(&output);
            assert_eq!(
                commands.len(),
                expected,
                "make {target} must keep every Cargo invocation: {output}"
            );
            for command in commands {
                assert!(
                    development_route_matches(command, host).expect("read effective flags"),
                    "make {target} on {host} loses its standard: {command}"
                );
            }
            assert!(
                preflight_precedes_cargo(&output),
                "preflight order changed: {output}"
            );
            let without_preflight = output.replace("scripts/check-build-tools.sh\n", "");
            assert!(
                !preflight_precedes_cargo(&without_preflight),
                "accepted route after removing build-tool preflight: {without_preflight}"
            );
        }
    }
}

/// Both suite launchers keep development flags and preflight for doctests.
#[test]
fn test_make_routes_restate_flags_and_run_preflight_first() {
    for (test_cmd, expected_first) in [("nextest run", "nextest run"), ("test", "test ")] {
        let output = make_output(
            "test",
            &[
                "CARGO=probe-cargo",
                "BUILD_HOST_OS=Linux",
                "BUILD_JOBS=--jobs=3",
                &format!("TEST_CMD={test_cmd}"),
            ],
        )
        .expect("evaluate Make test target");
        let commands = cargo_lines(&output);
        assert_eq!(
            commands.len(),
            2,
            "test and doctest must both invoke Cargo: {output}"
        );
        let suite = commands.first().expect("suite command must be present");
        let doctest = commands.get(1).expect("doctest command must be present");
        assert!(suite.contains(expected_first), "wrong suite route: {suite}");
        assert!(
            doctest.contains("probe-cargo test --doc"),
            "missing doctest: {doctest}"
        );
        assert!(
            suite.contains("--jobs=3") && doctest.contains("--jobs=3"),
            "BUILD_JOBS must reach suite and doctests: {commands:?}"
        );
        for command in commands {
            assert!(
                development_route_matches(command, "Linux").expect("read effective flags"),
                "test route loses its standard flags: {command}"
            );
        }
        assert!(
            preflight_precedes_cargo(&output),
            "test preflight must precede Cargo: {output}"
        );
    }
}

/// Removing any one development requirement invalidates the route.
#[test]
fn development_route_rejects_missing_requirements() {
    let output = make_output("build", &["CARGO=probe-cargo", "BUILD_HOST_OS=Linux"])
        .expect("evaluate development route with hostile backend environment");
    let clean = cargo_lines(&output)
        .into_iter()
        .find(|line| line.contains("probe-cargo build"))
        .expect("development Cargo invocation must be present");
    for missing_requirement in [
        "-u CARGO_ENCODED_RUSTFLAGS ",
        "-u CARGO_UNSTABLE_CODEGEN_BACKEND ",
        "-Zthreads=8 ",
        "-Clink-arg=-fuse-ld=mold",
    ] {
        let injected = clean.replace(missing_requirement, "");
        assert_ne!(
            injected, clean,
            "mutation must change {missing_requirement}"
        );
        assert!(
            !development_route_matches(&injected, "Linux").expect("read mutated route"),
            "accepted development route missing {missing_requirement}"
        );
    }
    for selector in PROFILE_BACKEND_SELECTORS {
        let missing = clean.replace(&format!("-u {selector} "), "");
        let later = clean.replace(
            "RUSTFLAGS=\"",
            &format!("{selector}=cranelift RUSTFLAGS=\""),
        );
        assert_ne!(missing, clean, "missing-{selector} mutation was a no-op");
        assert_ne!(later, clean, "later-{selector} mutation was a no-op");
        assert!(
            !development_route_matches(&missing, "Linux").expect("read missing selector"),
            "development route accepted removal of {selector}"
        );
        assert!(
            !development_route_matches(&later, "Linux").expect("read later override"),
            "development route accepted a later {selector} override"
        );
    }
}

/// Coverage explicitly selects LLVM, lld, and no development flags.
#[test]
fn coverage_make_route_uses_llvm_and_clears_higher_precedence_flags() {
    let output = make_output("coverage", &["CARGO=probe-cargo", "BUILD_HOST_OS=Linux"])
        .expect("evaluate coverage target");
    let commands = cargo_lines(&output);
    assert_eq!(
        commands.len(),
        1,
        "coverage must invoke one Cargo route: {output}"
    );
    let command = commands.first().expect("coverage command must be present");
    assert!(
        coverage_route_matches(command),
        "coverage route is not isolated: {command}"
    );
    assert!(
        preflight_precedes_cargo(&output),
        "coverage preflight must precede Cargo"
    );
    for (label, mutated) in coverage_mutations(command)
        .into_iter()
        .chain(build_override_mutations(command))
    {
        assert_ne!(
            mutated.as_str(),
            *command,
            "{label} mutation must change the route"
        );
        assert!(
            !coverage_route_matches(&mutated),
            "accepted {label} route: {mutated}"
        );
    }
}

/// Release invokes Cargo outside the repository with ambient Rust flags cleared.
#[test]
fn release_make_route_uses_external_manifest_and_clears_flags() {
    let output = make_output("release", &["CARGO=probe-cargo"]).expect("evaluate release target");
    let commands = cargo_lines(&output);
    assert_eq!(
        commands.len(),
        1,
        "release must invoke one Cargo route: {output}"
    );
    let command = commands.first().expect("release command must be present");
    let manifest = format!("{}/Cargo.toml", env!("CARGO_MANIFEST_DIR"));
    assert!(
        release_route_matches(command, &manifest),
        "wrong release route: {command}"
    );
    for selector in PROFILE_BACKEND_SELECTORS {
        let missing = command.replace(&format!("-u {selector} "), "");
        let later = command.replace(
            "RUSTFLAGS=\"",
            &format!("{selector}=cranelift RUSTFLAGS=\""),
        );
        assert_ne!(missing, *command, "missing-{selector} mutation was a no-op");
        assert_ne!(later, *command, "later-{selector} mutation was a no-op");
        assert!(
            !release_route_matches(&missing, &manifest),
            "accepted missing {selector}"
        );
        assert!(
            !release_route_matches(&later, &manifest),
            "accepted later {selector}"
        );
    }
}

/// Whitaker clears ambient flags after checking that the executable exists.
#[test]
fn whitaker_route_is_isolated_and_ordered() {
    let leaf = make_output(
        "lint-whitaker",
        &["WHITAKER=probe-whitaker", "CARGO=probe-cargo"],
    )
    .expect("evaluate Whitaker target");
    let availability = leaf
        .find("command -v \"probe-whitaker\"")
        .expect("Whitaker availability check is required");
    let invocation = leaf
        .find("probe-whitaker --all")
        .expect("Whitaker invocation is required");
    assert!(
        availability < invocation,
        "check must precede suite: {leaf}"
    );
    let command = leaf
        .lines()
        .find(|line| line.contains("probe-whitaker --all"))
        .expect("Whitaker command must be present");
    assert!(
        whitaker_route_matches(command),
        "Whitaker inherited dev flags: {command}"
    );
    for selector in PROFILE_BACKEND_SELECTORS {
        let missing = command.replace(&format!("-u {selector} "), "");
        assert_ne!(missing, command, "missing-{selector} mutation was a no-op");
        assert!(
            !whitaker_route_matches(&missing),
            "accepted missing {selector}"
        );
    }
    for selector in PROFILE_BACKEND_SELECTORS.iter().take(4) {
        let wrong = command.replace(
            &format!("{selector}=llvm"),
            &format!("{selector}=cranelift"),
        );
        assert_ne!(wrong, command, "wrong-{selector} mutation was a no-op");
        assert!(
            !whitaker_route_matches(&wrong),
            "accepted hostile {selector}"
        );
    }
}

/// Parallel lint runs rustdoc, Clippy, and Whitaker in order.
#[test]
fn parallel_lint_route_preserves_flags_and_order() {
    let parallel = make_output(
        "lint",
        &[
            "-j",
            "CARGO=probe-cargo",
            "WHITAKER=probe-whitaker",
            "BUILD_HOST_OS=Linux",
        ],
    )
    .expect("evaluate parallel lint target");
    let cargo_commands = cargo_lines(&parallel);
    assert_eq!(
        cargo_commands.len(),
        2,
        "rustdoc and Clippy stay separate: {parallel}"
    );
    let docs_command = cargo_commands
        .iter()
        .find(|line| line.contains("probe-cargo doc"))
        .expect("lint must run rustdoc");
    assert!(
        docs_command.contains("RUSTDOCFLAGS=")
            && docs_command.contains("--cfg docsrs")
            && docs_command.contains("-D warnings"),
        "rustdoc must receive docsrs and denied warnings: {docs_command}"
    );
    let docs = parallel
        .find("probe-cargo doc")
        .expect("lint must run rustdoc");
    let clippy = parallel
        .find("probe-cargo clippy")
        .expect("lint must run Clippy");
    let suite = parallel
        .find("probe-whitaker --all")
        .expect("lint must run Whitaker");
    let preflight = parallel
        .find("scripts/check-build-tools.sh")
        .expect("Clippy must check build tools");
    assert!(
        preflight < docs && docs < clippy && clippy < suite,
        "lint order changed: {parallel}"
    );
    for command in cargo_commands {
        assert!(
            development_route_matches(command, "Linux").expect("read lint flags"),
            "lint Cargo invocation lost the standard: {command}"
        );
    }
}

/// Whitaker failures must propagate through the Make target.
#[test]
fn whitaker_route_propagates_failure() {
    let failing = Command::new("make")
        .args(["--silent", "lint-whitaker", "WHITAKER=false"])
        .current_dir(env!("CARGO_MANIFEST_DIR"))
        .output()
        .expect("run failing Whitaker probe");
    assert!(!failing.status.success(), "Whitaker's failure was ignored");
}
