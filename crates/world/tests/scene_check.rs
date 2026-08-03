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
    codec::{Encoding, encode_document},
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
fn source_with(document: &SceneDocument) -> MemorySceneSource {
    let mut source = MemorySceneSource::new();
    let bytes = match encode_document(document, Encoding::Json) {
        Ok(bytes) => bytes,
        Err(error) => panic!("the fixture document must encode: {error}"),
    };
    source.insert(DOCUMENT, bytes);
    source.insert(RESOURCE, b"# empty for now\n".to_vec());
    source
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
#[expect(
    dead_code,
    reason = "`index`, `resource`, and `detail` are asserted structurally rather than by value: \
              `deny_unknown_fields` plus a missing-field error means a rename or removal fails \
              the parse, which is the contract. Their text is pinned by the insta snapshots \
              instead, where a wording change is meant to be reviewed."
)]
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

/// The failure a report carries, or a panic naming what was rendered instead.
fn failure_of(outcome: &Outcome) -> FailureShape {
    match parse_report(&outcome.output).failure {
        Some(failure) => failure,
        None => panic!("a failure must be reported, got:\n{}", outcome.output),
    }
}

/// Parses the `--json` output, failing loudly on a shape change.
fn parse_report(output: &str) -> ReportShape {
    match serde_json::from_str(output) {
        Ok(report) => report,
        Err(error) => panic!("the --json form must match the published shape: {error}\n{output}"),
    }
}

#[test]
fn a_valid_scene_exits_zero_and_names_its_source_root() {
    let outcome = check(source_with(&minimal_document()), Options::default());
    assert_eq!(outcome.code, ExitCode::Valid);
    assert_eq!(outcome.code.get(), 0);
    // The root is in every report because running from the wrong directory
    // otherwise produces "knowledge resource absent" — the same diagnostic a
    // genuinely broken scene produces.
    assert!(
        outcome.output.contains("source root: <memory>"),
        "the report must name the resolved source root, got:\n{}",
        outcome.output
    );
    assert!(outcome.output.contains("0 error(s), 0 warning(s)"));
}

#[test]
fn an_invalid_scene_exits_one_and_names_the_class() {
    let mut document = minimal_document();
    document.chunk_size = 16;
    let outcome = check(source_with(&document), Options::default());
    assert_eq!(outcome.code, ExitCode::Invalid);
    assert_eq!(outcome.code.get(), 1);
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
    assert_eq!(outcome.code, ExitCode::SourceFailure);
    assert_eq!(outcome.code.get(), 2);
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
    let mut source = source_with(&minimal_document());
    source.remove(Utf8Path::new(RESOURCE));
    let outcome = check(source, Options::default());
    assert_eq!(outcome.code, ExitCode::Invalid);
    assert!(
        outcome.output.contains("scene.knowledge.resource-absent"),
        "got:\n{}",
        outcome.output
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

    let lenient = check(source_with(&document), Options::default());
    assert_eq!(lenient.code, ExitCode::Valid);
    assert!(
        lenient
            .output
            .contains("warning scene.entities.spawn-obstructed")
    );

    let strict = check(
        source_with(&document),
        options(|o| o.strictness = Strictness::Strict),
    );
    assert_eq!(strict.code, ExitCode::Invalid);
    // Strictness changes the verdict, never the report: the finding is still a
    // warning, and calling it an error under one flag would make two runs
    // disagree about what is wrong with the same file.
    assert!(
        strict
            .output
            .contains("warning scene.entities.spawn-obstructed")
    );
}

#[test]
fn the_json_envelope_names_the_document_and_the_root() {
    let mut document = minimal_document();
    document.chunk_size = 16;
    let outcome = check(
        source_with(&document),
        options(|o| o.format = OutputFormat::Json),
    );
    let report = parse_report(&outcome.output);
    assert!(!report.ok);
    assert_eq!(report.document, DOCUMENT);
    assert_eq!(report.source_root, "<memory>");
    assert_eq!(report.warnings.len(), 0);
}

#[test]
fn a_json_error_entry_carries_its_class_and_section() {
    let mut document = minimal_document();
    document.chunk_size = 16;
    let outcome = check(
        source_with(&document),
        options(|o| o.format = OutputFormat::Json),
    );
    let report = parse_report(&outcome.output);
    let Some(first) = report.errors.first() else {
        panic!("an invalid scene must report at least one error");
    };
    assert_eq!(first.code, "scene.chunk-size.not-design");
    assert_eq!(first.section, "dimensions");
    assert_eq!(first.site, None);
}

#[test]
fn stats_are_null_rather_than_absent_when_not_asked_for() {
    // Present and null, so a consumer never has to ask whether the key exists.
    // Deserializing into a typed shape is what makes this an assertion at all:
    // a renamed field fails to parse, whereas a `Value` lookup would read as
    // absent and pass.
    let outcome = check(
        source_with(&minimal_document()),
        options(|o| o.format = OutputFormat::Json),
    );
    assert_eq!(parse_report(&outcome.output).stats, None);
}

#[test]
fn a_positional_diagnostic_carries_its_chunk_and_local_position() {
    // A run ordinal is useless in a 134-million-voxel scene, so the JSON form
    // has to carry the place as well as the class. This is what the fixture
    // generator's provenance sidecar joins against.
    let mut document = minimal_document();
    if let Some(entry) = document.voxels.first_mut() {
        entry.payload = ChunkPayloadDocument::Uniform(99);
    }
    let outcome = check(
        source_with(&document),
        options(|o| o.format = OutputFormat::Json),
    );
    let report = parse_report(&outcome.output);
    let Some(first) = report.errors.first() else {
        panic!("an unresolvable index must report an error");
    };
    assert_eq!(first.code, "scene.voxels.unknown-palette-index");
    assert_eq!(
        first.site,
        Some(SiteShape {
            chunk: [0, 0, 0],
            local: [0, 0, 0],
        })
    );
}

/// The measurements over the minimal fixture, as JSON.
fn minimal_stats() -> StatsShape {
    let outcome = check(
        source_with(&minimal_document()),
        options(|o| {
            o.format = OutputFormat::Json;
            o.stats = true;
        }),
    );
    let Some(stats) = parse_report(&outcome.output).stats else {
        panic!("--stats must populate the stats member");
    };
    stats
}

#[test]
fn stats_report_the_scene_counts() {
    let stats = minimal_stats();
    assert_eq!(stats.palette_entries, 3);
    assert_eq!(stats.spawns, 1);
    assert_eq!(stats.populated_chunks, 2);
    assert_eq!(stats.uniform_chunks, 1);
    assert_eq!(stats.non_air_voxels, 32_784);
    assert_eq!(stats.declared_voxels, 65_536);
}

#[test]
fn stats_report_the_measured_encoding_sizes() {
    let stats = minimal_stats();
    // MessagePack is the shipping encoding and must be the smaller of the two,
    // or the reason for having a second encoding at all has evaporated.
    assert!(
        stats.msgpack_bytes < stats.json_bytes,
        "msgpack ({}) must be smaller than json ({})",
        stats.msgpack_bytes,
        stats.json_bytes
    );
    // One dense chunk of 32,768 voxels at two bytes each. The uniform chunk
    // costs nothing, which is the elision that makes a wilderness extent
    // affordable.
    assert_eq!(stats.decoded_bytes, 65_536);
}

#[test]
fn stats_also_render_as_text() {
    let outcome = check(
        source_with(&minimal_document()),
        options(|o| o.stats = true),
    );
    assert_eq!(outcome.code, ExitCode::Valid);
    assert!(outcome.output.contains("populated chunks: 2 (1 uniform)"));
    assert!(
        outcome
            .output
            .contains("voxels: 32784 non-air of 65536 declared")
    );
}

#[test]
fn a_bare_path_parses_with_the_lenient_text_defaults() {
    let parsed = check::parse(["scene.json".to_owned()]);
    let Invocation::Check { path, options } = parsed else {
        panic!("a bare path must parse as a check, got {parsed:?}");
    };
    assert_eq!(path, "scene.json");
    assert_eq!(options, Options::default());
}

#[test]
fn flags_parse_in_any_order_relative_to_the_path() {
    let before = check::parse(["--strict".to_owned(), "a.json".to_owned()]);
    let after = check::parse(["a.json".to_owned(), "--strict".to_owned()]);
    assert_eq!(before, after);
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
    assert_eq!(ExitCode::Usage.get(), 64);
}

#[test]
fn a_missing_document_still_renders_parseable_json() {
    // The regression this guards: a `SceneLoadError` carrying no diagnostics
    // used to have its label and message appended *after* the rendered report,
    // which under `--json` put a bare line after a closed object. Every
    // consumer of this contract parses the whole stream, so that broke them on
    // exactly the paths they most need to read — the ones where nothing loaded.
    let outcome = missing_document();
    assert_eq!(outcome.code, ExitCode::SourceFailure);
    let failure = failure_of(&outcome);
    assert_eq!(failure.kind, "source failure");
    assert!(!failure.message.is_empty());
}

#[test]
fn a_missing_document_is_not_reported_as_a_document_fault() {
    let report = parse_report(&missing_document().output);
    // `ok` is the field a consumer branches on, and a failure is a fatal
    // problem: it must not read as a pass while the process exits non-zero.
    assert!(!report.ok);
    // Nor as an error: an unreadable document has no place *within* a
    // document, and the generator's suite reads `errors` as located faults.
    assert!(report.errors.is_empty(), "{:?}", report.errors);
}

#[test]
fn a_malformed_document_still_renders_parseable_json() {
    let mut source = MemorySceneSource::new();
    source.insert(DOCUMENT, b"{ this is not a scene ".to_vec());
    let outcome = check(source, options(|o| o.format = OutputFormat::Json));
    assert_eq!(outcome.code, ExitCode::Invalid);
    let report = parse_report(&outcome.output);
    let Some(failure) = report.failure else {
        panic!(
            "a malformed document must be reported, got:\n{}",
            outcome.output
        );
    };
    assert_eq!(failure.kind, "malformed document");
    assert!(!report.ok);
}

#[test]
fn a_valid_scene_reports_no_failure() {
    let outcome = check(
        source_with(&minimal_document()),
        options(|o| o.format = OutputFormat::Json),
    );
    // Present and null rather than absent, for the reason `stats` is: a
    // consumer that must test for a key's existence reads `undefined` as a
    // value one refactor later.
    let report = parse_report(&outcome.output);
    assert!(report.failure.is_none());
    assert!(report.ok);
}
