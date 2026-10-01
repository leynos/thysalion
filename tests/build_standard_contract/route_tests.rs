//! Mutation contracts for coverage and stable release Make routes.

use super::helpers::{
    MakeTarget,
    PROFILE_BACKEND_SELECTORS,
    RouteText,
    build_override_mutations,
    cargo_lines,
    coverage_mutations,
    coverage_route_matches,
    make_output,
    preflight_precedes_cargo,
    release_route_matches,
};

/// Coverage explicitly selects LLVM, lld, and no development flags.
#[test]
fn coverage_make_route_uses_llvm_and_clears_higher_precedence_flags() {
    let output = make_output(
        MakeTarget::Coverage,
        &["CARGO=probe-cargo", "BUILD_HOST_OS=Linux"],
    )
    .expect("evaluate coverage target");
    let commands = cargo_lines(RouteText(&output));
    assert_eq!(
        commands.len(),
        1,
        "coverage must invoke one Cargo route: {output}"
    );
    let command = commands.first().expect("coverage command must be present");
    assert!(
        coverage_route_matches(RouteText(command)),
        "coverage route is not isolated: {command}"
    );
    assert!(
        preflight_precedes_cargo(RouteText(&output)),
        "coverage preflight must precede Cargo"
    );
    for (label, mutated) in coverage_mutations(RouteText(command))
        .into_iter()
        .chain(build_override_mutations(RouteText(command)))
    {
        assert_ne!(
            mutated.as_str(),
            *command,
            "{label} mutation must change the route"
        );
        assert!(
            !coverage_route_matches(RouteText(&mutated)),
            "accepted {label} route: {mutated}"
        );
    }
}

/// Release invokes Cargo outside the repository with ambient Rust flags cleared.
#[test]
fn release_make_route_uses_external_manifest_and_clears_flags() {
    let output =
        make_output(MakeTarget::Release, &["CARGO=probe-cargo"]).expect("evaluate release target");
    let commands = cargo_lines(RouteText(&output));
    assert_eq!(
        commands.len(),
        1,
        "release must invoke one Cargo route: {output}"
    );
    let command = commands.first().expect("release command must be present");
    let manifest = format!("{}/Cargo.toml", env!("CARGO_MANIFEST_DIR"));
    assert!(
        release_route_matches(RouteText(command), &manifest),
        "wrong release route: {command}"
    );
    for selector in PROFILE_BACKEND_SELECTORS {
        let missing = command.replace(&format!("-u {} ", selector.0), "");
        let later = command.replace(
            "RUSTFLAGS=\"",
            &format!("{}=cranelift RUSTFLAGS=\"", selector.0),
        );
        assert_ne!(
            missing, *command,
            "missing-{} mutation was a no-op",
            selector.0
        );
        assert_ne!(later, *command, "later-{} mutation was a no-op", selector.0);
        assert!(
            !release_route_matches(RouteText(&missing), &manifest),
            "accepted missing {}",
            selector.0
        );
        assert!(
            !release_route_matches(RouteText(&later), &manifest),
            "accepted later {}",
            selector.0
        );
    }
}
