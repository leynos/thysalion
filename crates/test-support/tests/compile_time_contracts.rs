//! Compile-time contracts for the test-support crate's public surface.

#[test]
fn replay_errors_require_non_exhaustive_matches() {
    let cases = trybuild::TestCases::new();
    cases.compile_fail("tests/ui/replay_encode_error_exhaustive_match.rs");
    cases.compile_fail("tests/ui/replay_decode_error_exhaustive_match.rs");
}

#[test]
fn input_record_remains_uninhabited() {
    trybuild::TestCases::new().pass("tests/ui/input_record_is_uninhabited.rs");
}

#[cfg(feature = "bevy")]
#[test]
fn bevy_harness_is_available_when_feature_enabled() {
    trybuild::TestCases::new().pass("tests/ui/bevy_harness_available.rs");
}

#[cfg(not(feature = "bevy"))]
#[test]
fn bevy_harness_is_hidden_without_feature() {
    trybuild::TestCases::new().compile_fail("tests/ui/bevy_harness_requires_feature.rs");
}
