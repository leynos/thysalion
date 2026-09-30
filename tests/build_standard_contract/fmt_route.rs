//! Verify that formatting uses Cargo injection and the repository toolchain pin.

use super::helpers::make_output;

#[test]
fn formatting_routes_use_injected_cargo_without_a_floating_toolchain() {
    for (target, expected) in [
        ("fmt", "probe-cargo fmt --all"),
        ("check-fmt", "probe-cargo fmt --all -- --check"),
    ] {
        let output = make_output(target, &["CARGO=probe-cargo"])
            .expect("evaluate the formatting Make target");
        let cargo_lines: Vec<_> = output
            .lines()
            .filter(|line| line.starts_with("probe-cargo "))
            .collect();
        assert_eq!(
            cargo_lines,
            vec![expected],
            "{target} must use the injected Cargo command"
        );
        assert!(
            !output.contains("+nightly"),
            "{target} must use rust-toolchain.toml rather than a floating alias"
        );
    }
}
