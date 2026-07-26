//! Compile-time contract tests (trybuild).
//!
//! `HarnessConfig` is `#[non_exhaustive]` so the harness can grow fields
//! without breaking existing demos ("no scaffolding rework" — ADR-005).
//! That promise only holds if external crates are forced through
//! `HarnessConfig::new` and the `with_*` builders; this test pins the
//! compiler's rejection of external struct-literal construction, with the
//! expected diagnostic checked in beside the case.

#[test]
fn non_exhaustive_config_rejects_external_struct_literals() {
    let cases = trybuild::TestCases::new();
    cases.compile_fail("tests/ui/*.rs");
}
