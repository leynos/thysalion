//! JSON report and measured-statistics contracts for `scene-check`.

use super::*;

#[test]
fn the_json_envelope_names_the_document_and_the_root() {
    let mut document = minimal_document();
    document.chunk_size = 16;
    let source = source_with(&document).expect("invalid fixture must encode");
    let outcome = check(source, options(|o| o.format = OutputFormat::Json));
    let report = parse_report(&outcome.output).expect("envelope must parse as JSON");
    assert!(!report.ok, "invalid scene report should set ok to false");
    assert_eq!(
        report.document, DOCUMENT,
        "JSON report should name its document"
    );
    assert_eq!(
        report.source_root, "<memory>",
        "JSON report should retain its root"
    );
    assert_eq!(
        report.warnings.len(),
        0,
        "invalid report should contain no warnings"
    );
}

#[test]
fn a_json_error_entry_carries_its_class_and_section() {
    let mut document = minimal_document();
    document.chunk_size = 16;
    let source = source_with(&document).expect("invalid fixture must encode");
    let outcome = check(source, options(|o| o.format = OutputFormat::Json));
    let report = parse_report(&outcome.output).expect("error report must parse as JSON");
    let Some(first) = report.errors.first() else {
        panic!("an invalid scene must report at least one error");
    };
    assert_eq!(
        first.code, "scene.chunk-size.not-design",
        "error entry should preserve the diagnostic code"
    );
    assert_eq!(
        first.section, "dimensions",
        "error entry should identify its section"
    );
    assert_eq!(first.index, 0, "section-wide faults use index zero");
    assert_eq!(
        first.site, None,
        "section-wide fault should have no position"
    );
    assert_eq!(
        first.resource, None,
        "section-wide fault should have no resource"
    );
    assert!(
        !first.detail.is_empty(),
        "a diagnostic needs an explanation"
    );
}

#[test]
fn stats_are_null_rather_than_absent_when_not_asked_for() {
    // Present and null, so a consumer never has to ask whether the key exists.
    // Deserializing into a typed shape is what makes this an assertion at all:
    // a renamed field fails to parse, whereas a `Value` lookup would read as
    // absent and pass.
    let source = source_with(&minimal_document()).expect("minimal fixture must encode");
    let outcome = check(source, options(|o| o.format = OutputFormat::Json));
    let report = parse_report(&outcome.output).expect("report must parse as JSON");
    assert_eq!(
        report.stats, None,
        "stats should be null when not requested"
    );
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
    let source = source_with(&document).expect("invalid fixture must encode");
    let outcome = check(source, options(|o| o.format = OutputFormat::Json));
    let report = parse_report(&outcome.output).expect("positioned report must parse as JSON");
    let Some(first) = report.errors.first() else {
        panic!("an unresolvable index must report an error");
    };
    assert_eq!(
        first.code, "scene.voxels.unknown-palette-index",
        "unknown palette index diagnostic code mismatch"
    );
    assert_eq!(
        first.site,
        Some(SiteShape {
            chunk: [0, 0, 0],
            local: [0, 0, 0],
        }),
        "positional error should retain chunk and local coordinates"
    );
}

/// The measurements over the minimal fixture, as JSON.
fn minimal_stats() -> Result<StatsShape, String> {
    let source = source_with(&minimal_document())
        .map_err(|error| format!("minimal stats fixture must encode: {error}"))?;
    let outcome = check(
        source,
        options(|o| {
            o.format = OutputFormat::Json;
            o.stats = true;
        }),
    );
    parse_report(&outcome.output)?
        .stats
        .ok_or_else(|| "--stats must populate the stats member".to_owned())
}

#[test]
fn stats_report_the_scene_counts() {
    let stats = minimal_stats().expect("minimal stats report must be available");
    assert_eq!(
        stats.palette_entries, 3,
        "stats should count palette entries"
    );
    assert_eq!(stats.spawns, 1, "stats should count spawns");
    assert_eq!(
        stats.populated_chunks, 2,
        "stats should count populated chunks"
    );
    assert_eq!(stats.uniform_chunks, 1, "stats should count uniform chunks");
    assert_eq!(
        stats.non_air_voxels, 32_784,
        "stats should count non-air voxels"
    );
    assert_eq!(
        stats.declared_voxels, 65_536,
        "stats should count declared voxels"
    );
}

#[test]
fn stats_report_the_measured_encoding_sizes() {
    let stats = minimal_stats().expect("minimal stats report must be available");
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
    assert_eq!(
        stats.decoded_bytes, 65_536,
        "one dense chunk should use 65,536 bytes"
    );
}

#[test]
fn stats_also_render_as_text() {
    let source = source_with(&minimal_document()).expect("minimal fixture must encode");
    let outcome = check(source, options(|o| o.stats = true));
    assert_eq!(
        outcome.code,
        ExitCode::Valid,
        "text stats should preserve the valid status"
    );
    assert!(
        outcome.output.contains("populated chunks: 2 (1 uniform)"),
        "text stats should include chunk counts"
    );
    assert!(
        outcome
            .output
            .contains("voxels: 32784 non-air of 65536 declared"),
        "text stats should include non-air and declared voxel counts"
    );
}
