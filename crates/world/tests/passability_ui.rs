//! Compile-time contracts for the public passability API.

#[test]
fn passability_faces_use_the_typed_domain_value() {
    let cases = trybuild::TestCases::new();
    cases.pass("tests/ui/passability/typed_face.rs");
    cases.compile_fail("tests/ui/passability/legacy_boolean_face.rs");
}
