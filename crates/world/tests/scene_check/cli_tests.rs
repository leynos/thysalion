//! CLI argument parsing and usage behaviour for `scene-check`.

use super::*;

#[test]
fn a_bare_path_parses_with_the_lenient_text_defaults() {
    let parsed = check::parse(["scene.json".to_owned()]);
    let Invocation::Check { path, options } = parsed else {
        panic!("a bare path must parse as a check, got {parsed:?}");
    };
    assert_eq!(path, "scene.json", "the bare path must be retained");
    assert_eq!(
        options,
        Options::default(),
        "bare paths must use default options"
    );
}

#[test]
fn flags_parse_in_any_order_relative_to_the_path() {
    let before = check::parse(["--strict".to_owned(), "a.json".to_owned()]);
    let after = check::parse(["a.json".to_owned(), "--strict".to_owned()]);
    assert_eq!(before, after, "flag order must not change parsing");
}

#[test]
fn an_unrecognized_flag_is_a_usage_error() {
    let parsed = check::parse(["--verbose".to_owned(), "a.json".to_owned()]);
    let Invocation::Usage { problem } = parsed else {
        panic!("an unknown flag must be a usage error, got {parsed:?}");
    };
    assert!(problem.contains("--verbose"), "got {problem:?}");
}

#[test]
fn a_second_document_is_a_usage_error_rather_than_a_guess() {
    // Guessing which was meant is how a check silently validates the wrong file
    // and reports success.
    let parsed = check::parse(["a.json".to_owned(), "b.json".to_owned()]);
    assert!(matches!(parsed, Invocation::Usage { .. }), "got {parsed:?}");
}

#[test]
fn no_document_is_a_usage_error() {
    let parsed = check::parse(["--strict".to_owned()]);
    assert!(matches!(parsed, Invocation::Usage { .. }), "got {parsed:?}");
}

#[test]
fn the_usage_exit_code_is_the_sysexits_one() {
    assert_eq!(
        ExitCode::Usage.get(),
        64,
        "usage errors must use sysexits code 64"
    );
}

#[test]
fn a_missing_document_still_renders_parseable_json() {
    // The regression this guards: a `SceneLoadError` carrying no diagnostics
    // used to have its label and message appended *after* the rendered report,
    // which under `--json` put a bare line after a closed object. Every
    // consumer of this contract parses the whole stream, so that broke them on
    // exactly the paths they most need to read — the ones where nothing loaded.
    let outcome = missing_document();
    assert_eq!(
        outcome.code,
        ExitCode::SourceFailure,
        "missing source must fail loading"
    );
    let failure = failure_of(&outcome).expect("missing document must report a JSON failure");
    assert_eq!(
        failure.kind, "source failure",
        "JSON must label the source failure"
    );
    assert!(
        !failure.message.is_empty(),
        "source failure must explain the problem"
    );
}

#[test]
fn a_missing_document_is_not_reported_as_a_document_fault() {
    let report = parse_report(&missing_document().output)
        .expect("missing-document report must parse as JSON");
    // `ok` is the field a consumer branches on, and a failure is a fatal
    // problem: it must not read as a pass while the process exits non-zero.
    assert!(!report.ok, "missing source must not report success");
    // Nor as an error: an unreadable document has no place *within* a
    // document, and the generator's suite reads `errors` as located faults.
    assert!(report.errors.is_empty(), "{:?}", report.errors);
}

#[test]
fn a_malformed_document_still_renders_parseable_json() {
    let mut source = MemorySceneSource::new();
    source.insert(DOCUMENT, b"{ this is not a scene ".to_vec());
    let outcome = check(source, options(|o| o.format = OutputFormat::Json));
    assert_eq!(
        outcome.code,
        ExitCode::Invalid,
        "malformed documents must be invalid"
    );
    let report = parse_report(&outcome.output).expect("malformed report must parse as JSON");
    let Some(failure) = report.failure else {
        panic!(
            "a malformed document must be reported, got:\n{}",
            outcome.output
        );
    };
    assert_eq!(
        failure.kind, "malformed document",
        "JSON must label malformed input"
    );
    assert!(!report.ok, "malformed input must not report success");
}

#[test]
fn a_valid_scene_reports_no_failure() {
    let source = source_with(&minimal_document()).expect("minimal fixture must encode");
    let outcome = check(source, options(|o| o.format = OutputFormat::Json));
    // Present and null rather than absent, for the reason `stats` is: a
    // consumer that must test for a key's existence reads `undefined` as a
    // value one refactor later.
    let report = parse_report(&outcome.output).expect("valid report must parse as JSON");
    assert!(
        report.failure.is_none(),
        "valid scenes must have no failure"
    );
    assert!(report.ok, "valid scenes must report success");
}
