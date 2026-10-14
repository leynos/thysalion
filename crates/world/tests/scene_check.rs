//! The `scene-check` contract: exit codes, output formats, and strictness.
//!
//! These run against a [`MemorySceneSource`] rather than the filesystem, which
//! is the whole reason the tool's logic lives in `thysalion_world::check` and
//! not in the example wrapper. The four exit codes are what continuous
//! integration and the fixture generator's test suite depend on, and a contract
//! nothing exercises is a contract nothing keeps.

use std::sync::Arc;

use camino::Utf8Path;
use serde::Deserialize;
use thysalion_world::{
    check::{self, ExitCode, Invocation, Options, Outcome, OutputFormat},
    codec::{CodecError, Encoding, encode_document},
    loader::SceneLoader,
    scene::{
        document::{ChunkPayloadDocument, SceneDocument, VoxelPosDocument},
        validation::Strictness,
    },
    source::MemorySceneSource,
};

mod support;

use support::minimal_document;

/// Where the fixture document sits in the in-memory source.
const DOCUMENT: &str = "minimal.scene.json";

/// The resource the minimal fixture names.
const RESOURCE: &str = "knowledge/minimal.trig";

/// A source holding `document` and the knowledge resource it names.
fn source_with(document: &SceneDocument) -> Result<MemorySceneSource, CodecError> {
    let mut source = MemorySceneSource::new();
    let bytes = encode_document(document, Encoding::Json)?;
    source.insert(DOCUMENT, bytes);
    source.insert(RESOURCE, b"# empty for now\n".to_vec());
    Ok(source)
}

/// Runs the tool over `source` with the given options.
fn check(source: MemorySceneSource, options: Options) -> Outcome {
    let loader = SceneLoader::new(Arc::new(source));
    check::run(&loader, Utf8Path::new(DOCUMENT), "<memory>", options)
}

/// The default options, adjusted by a closure. Keeps each test to its variable.
fn options(adjust: impl FnOnce(&mut Options)) -> Options {
    let mut options = Options::default();
    adjust(&mut options);
    options
}

/// The JSON report as a consumer sees it.
///
/// Deserialized into a typed shape rather than inspected as a
/// `serde_json::Value`. The field names *are* the published contract — the
/// fixture generator's suite reads them — so a rename must fail the test, and a
/// `Value` lookup of a renamed key reads as absent instead.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ReportShape {
    document: String,
    source_root: String,
    ok: bool,
    errors: Vec<DiagnosticShape>,
    warnings: Vec<DiagnosticShape>,
    stats: Option<StatsShape>,
    failure: Option<FailureShape>,
}

/// A failure with no place in the document, as a consumer sees it.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct FailureShape {
    kind: String,
    message: String,
}

/// One diagnostic as a consumer sees it.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct DiagnosticShape {
    code: String,
    section: String,
    index: u32,
    site: Option<SiteShape>,
    resource: Option<String>,
    detail: String,
}

/// A position in the world as a consumer sees it.
#[derive(Debug, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
struct SiteShape {
    chunk: [u32; 3],
    local: [u32; 3],
}

/// The measurements as a consumer sees them.
#[derive(Debug, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
struct StatsShape {
    palette_entries: usize,
    spawns: usize,
    populated_chunks: usize,
    uniform_chunks: usize,
    runs: usize,
    non_air_voxels: u64,
    declared_voxels: u64,
    json_bytes: u64,
    msgpack_bytes: u64,
    decoded_bytes: u64,
}

/// Runs the tool over a source holding no document at all, as `--json`.
fn missing_document() -> Outcome {
    check(
        MemorySceneSource::new(),
        options(|o| o.format = OutputFormat::Json),
    )
}

/// The failure a report carries, or an error naming what was rendered.
fn failure_of(outcome: &Outcome) -> Result<FailureShape, String> {
    parse_report(&outcome.output)?
        .failure
        .ok_or_else(|| format!("a failure must be reported, got:\n{}", outcome.output))
}

/// Parses the `--json` output into the published report shape.
fn parse_report(output: &str) -> Result<ReportShape, String> {
    serde_json::from_str(output).map_err(|error| {
        format!("the --json form must match the published shape: {error}\n{output}")
    })
}

#[test]
fn a_valid_scene_exits_zero_and_names_its_source_root() {
    let source = source_with(&minimal_document()).expect("minimal fixture must encode");
    let outcome = check(source, Options::default());
    assert_eq!(
        outcome.code,
        ExitCode::Valid,
        "valid scene should exit as valid"
    );
    assert_eq!(
        outcome.code.get(),
        0,
        "valid scene should use exit code zero"
    );
    // The root is in every report because running from the wrong directory
    // otherwise produces "knowledge resource absent" — the same diagnostic a
    // genuinely broken scene produces.
    assert!(
        outcome.output.contains("source root: <memory>"),
        "the report must name the resolved source root, got:\n{}",
        outcome.output
    );
    assert!(
        outcome.output.contains("0 error(s), 0 warning(s)"),
        "valid scene should report no errors or warnings"
    );
}

#[test]
fn an_invalid_scene_exits_one_and_names_the_class() {
    let mut document = minimal_document();
    document.chunk_size = 16;
    let source = source_with(&document).expect("invalid fixture must encode");
    let outcome = check(source, Options::default());
    assert_eq!(
        outcome.code,
        ExitCode::Invalid,
        "invalid scene should be rejected"
    );
    assert_eq!(
        outcome.code.get(),
        1,
        "invalid scene should use exit code one"
    );
    assert!(
        outcome.output.contains("scene.chunk-size.not-design"),
        "got:\n{}",
        outcome.output
    );
}

#[test]
fn a_missing_document_exits_two_rather_than_one() {
    // The distinction that earns four codes rather than "zero or non-zero": a
    // continuous-integration job that cannot tell these apart reports a
    // mistyped fixture path as a validation failure and sends someone editing a
    // correct scene.
    let outcome = check(MemorySceneSource::new(), Options::default());
    assert_eq!(
        outcome.code,
        ExitCode::SourceFailure,
        "missing document should fail at source"
    );
    assert_eq!(
        outcome.code.get(),
        2,
        "missing document should use exit code two"
    );
    assert!(
        outcome.output.contains("source failure"),
        "got:\n{}",
        outcome.output
    );
}

#[test]
fn a_missing_knowledge_resource_is_a_validation_failure_not_a_source_failure() {
    // The resource is named by the *document*, so its absence is a fault in the
    // document. Only the document itself being unreachable is a source failure.
    let mut source = source_with(&minimal_document()).expect("minimal fixture must encode");
    source.remove(Utf8Path::new(RESOURCE));
    let outcome = check(source, Options::default());
    assert_eq!(
        outcome.code,
        ExitCode::Invalid,
        "missing resource should invalidate the scene"
    );
    assert!(
        outcome.output.contains("scene.knowledge.resource-absent"),
        "got:\n{}",
        outcome.output
    );
}

#[test]
fn a_json_resource_diagnostic_names_the_missing_file() {
    let mut source = source_with(&minimal_document()).expect("minimal fixture must encode");
    source.remove(Utf8Path::new(RESOURCE));
    let outcome = check(source, options(|o| o.format = OutputFormat::Json));
    let report = parse_report(&outcome.output).expect("resource report must parse as JSON");
    let matching_error = report
        .errors
        .iter()
        .find(|error| error.code == "scene.knowledge.resource-absent");
    let Some(resource_error) = matching_error else {
        panic!("the missing knowledge file must produce a resource diagnostic");
    };
    assert_eq!(
        resource_error.resource.as_deref(),
        Some(RESOURCE),
        "wrong knowledge resource path"
    );
}

#[test]
fn a_warning_passes_lenient_and_fails_strict() {
    // The one behaviour `--strict` exists for. Continuous integration runs
    // strict so a spawn inside a wall does not reach the repository unnoticed; a
    // contributor iterating locally is not blocked by one.
    let mut document = minimal_document();
    if let Some(spawn) = document.entities.spawns.first_mut() {
        spawn.at = VoxelPosDocument { x: 40, y: 4, z: 4 };
    }

    let lenient_source = source_with(&document).expect("warning fixture must encode");
    let lenient = check(lenient_source, Options::default());
    assert_eq!(
        lenient.code,
        ExitCode::Valid,
        "lenient mode should accept warnings"
    );
    assert!(
        lenient
            .output
            .contains("warning scene.entities.spawn-obstructed"),
        "lenient mode should report the obstructed spawn"
    );

    let strict_source = source_with(&document).expect("warning fixture must encode");
    let strict = check(
        strict_source,
        options(|o| o.strictness = Strictness::Strict),
    );
    assert_eq!(
        strict.code,
        ExitCode::Invalid,
        "strict mode should reject warnings"
    );
    // Strictness changes the verdict, never the report: the finding is still a
    // warning, and calling it an error under one flag would make two runs
    // disagree about what is wrong with the same file.
    assert!(
        strict
            .output
            .contains("warning scene.entities.spawn-obstructed"),
        "strict mode should retain the warning classification"
    );
}

#[path = "scene_check/report_tests.rs"]
mod report_tests;

#[path = "scene_check/cli_tests.rs"]
mod cli_tests;
