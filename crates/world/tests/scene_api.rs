//! The scene format's public surface, exercised directly rather than through a
//! whole document load.
//!
//! The other integration tests here drive the loader end to end, which is the
//! right shape for the format's behaviour but leaves parts of the API these
//! types publish untouched: the accessors a consumer reads a parsed identifier
//! with, the branches that reject a malformed one, and the two header faults
//! the codec's version probe intercepts before validation ever sees them.
//!
//! That last case is the reason this file exists rather than another corrupt
//! fixture. [`thysalion_world::scene::validation::validate`] is public and takes
//! a document, so a rule can be reached without first persuading the codec to
//! hand the document over — and `check_version` cannot be reached any other
//! way, because [`SceneLoader`] rejects an unreadable version during decode.

use std::sync::Arc;

use camino::Utf8Path;
use rstest::rstest;
use smol_str::SmolStr;
use thysalion_world::{
    codec::Encoding,
    grid::{ChunkCoord, ChunkSize, VoxelPos},
    loader::{SceneLoadError, SceneLoader},
    scene::{
        concept::{
            ConceptIri,
            ConceptIriProblem,
            NamespaceTable,
            SCENE_PREFIX,
            THYSALION_BASE,
            THYSALION_PREFIX,
        },
        document::SceneDocument,
        validation::{Bounds, DiagnosticCode, Policy, SceneDiagnostic},
    },
    source::MemorySceneSource,
};

mod support;

use support::minimal_document;

/// The one resource `minimal_document` names.
const RESOURCE: &str = "knowledge/minimal.trig";

/// A source holding the resource the minimal document expects.
fn source() -> MemorySceneSource {
    let mut source = MemorySceneSource::new();
    source.insert(RESOURCE, b"# empty for now\n".to_vec());
    source
}

/// Whether `diagnostics` carries `code`, wherever it sits in the list.
fn carries(diagnostics: &[SceneDiagnostic], code: DiagnosticCode) -> bool {
    diagnostics.iter().any(|diagnostic| match diagnostic {
        SceneDiagnostic::Structural { code: found, .. }
        | SceneDiagnostic::Positioned { code: found, .. }
        | SceneDiagnostic::Resource { code: found, .. } => *found == code,
        // `SceneDiagnostic` is `#[non_exhaustive]`, so a new shape must not
        // silently satisfy an assertion that it carries the expected code.
        _ => false,
    })
}

/// Validates `document` against the project namespaces and shipped bounds.
fn validate(document: &SceneDocument) -> Result<(), Vec<SceneDiagnostic>> {
    let namespaces = NamespaceTable::default();
    let bounds = Bounds::DEFAULT;
    let policy = Policy::new(&namespaces, &bounds);
    thysalion_world::scene::validation::validate(document, &source(), &policy).map(|_| ())
}

#[test]
fn a_parsed_identifier_exposes_its_parts() {
    let table = NamespaceTable::default();
    let Ok(iri) = ConceptIri::parse("thy:OakDoor", &table) else {
        panic!("`thy:OakDoor` must parse against the project namespaces");
    };
    assert_eq!(iri.as_str(), "thy:OakDoor");
    assert_eq!(iri.prefix(), "thy");
    assert_eq!(iri.local(), "OakDoor");
    // `Display` is what a diagnostic interpolates, so it must not gain quoting
    // or a prefix expansion that the wire form does not carry.
    assert_eq!(iri.to_string(), "thy:OakDoor");
}

#[rstest]
#[case::no_separator("thyOakDoor", ConceptIriProblem::Malformed)]
#[case::empty_prefix(":OakDoor", ConceptIriProblem::Malformed)]
#[case::empty_local("thy:", ConceptIriProblem::EmptyLocal)]
#[case::space_in_local("thy:Oak Door", ConceptIriProblem::IllegalCharacter(' '))]
fn a_malformed_identifier_names_its_fault(#[case] raw: &str, #[case] expected: ConceptIriProblem) {
    let table = NamespaceTable::default();
    assert_eq!(ConceptIri::parse(raw, &table), Err(expected));
}

#[test]
fn an_unpublished_prefix_is_rejected_by_name() {
    let table = NamespaceTable::default();
    let Err(problem) = ConceptIri::parse("zzz:Thing", &table) else {
        panic!("a prefix outwith the table must not parse");
    };
    // The prefix is reported back so a diagnostic can name what the document
    // wrote, rather than only that something was wrong.
    assert_eq!(
        problem,
        ConceptIriProblem::UnknownPrefix {
            found: SmolStr::new("zzz"),
        }
    );
}

#[test]
fn the_project_table_publishes_both_namespaces() {
    let table = NamespaceTable::default();
    assert!(table.contains(THYSALION_PREFIX));
    assert_eq!(table.base(THYSALION_PREFIX), Some(THYSALION_BASE));
    assert_eq!(table.base("zzz"), None);
    let prefixes: Vec<&str> = table.prefixes().collect();
    // Sorted, because the table is a `BTreeMap` and a report that lists the
    // permitted prefixes must not reorder between runs.
    assert_eq!(prefixes, vec![SCENE_PREFIX, THYSALION_PREFIX]);
}

#[test]
fn an_empty_table_rejects_every_prefix() {
    let table = NamespaceTable::empty();
    assert!(!table.contains(THYSALION_PREFIX));
    assert!(ConceptIri::parse("thy:OakDoor", &table).is_err());
}

#[rstest]
#[case::json("scene.json", Some(Encoding::Json))]
#[case::message_pack("scene.msgpack", Some(Encoding::MessagePack))]
#[case::unrecognized("scene.txt", None)]
#[case::absent("scene", None)]
fn an_encoding_is_inferred_from_the_extension(
    #[case] path: &str,
    #[case] expected: Option<Encoding>,
) {
    assert_eq!(Encoding::from_path(Utf8Path::new(path)), expected);
}

#[test]
fn each_encoding_names_itself() {
    assert_eq!(Encoding::Json.extension(), "json");
    assert_eq!(Encoding::MessagePack.extension(), "msgpack");
    assert_eq!(Encoding::Json.to_string(), "JSON");
    assert_eq!(Encoding::MessagePack.to_string(), "MessagePack");
}

#[test]
fn a_chunk_coordinate_locates_its_origin_corner() {
    let Ok(chunk_size) = ChunkSize::new(32) else {
        panic!("32 is the design chunk size and must construct");
    };
    // Constructed in reading order, whatever order the type sorts by: the
    // Z-major sort order is a serialization concern, not a constructor one.
    let coord = ChunkCoord::new(1, 2, 3);
    assert_eq!(coord.x, 1);
    assert_eq!(coord.y, 2);
    assert_eq!(coord.z, 3);
    assert_eq!(
        coord.origin(chunk_size),
        VoxelPos {
            x: 32,
            y: 64,
            z: 96,
        }
    );
}

#[test]
fn an_unrecognized_extension_is_the_callers_fault_not_the_documents() {
    let loader = SceneLoader::new(Arc::new(source()));
    let path = Utf8Path::new("minimal.scene.txt");
    let Err(SceneLoadError::UnknownEncoding { path: reported }) = loader.load(path) else {
        panic!("an unrecognized extension must not be guessed at");
    };
    // Reported rather than swallowed, because guessing produces a parse error
    // that blames the document for the caller's mistake.
    assert_eq!(reported, path);
}

#[test]
fn the_loader_reports_its_policy_when_formatted() {
    let loader = SceneLoader::new(Arc::new(source()));
    let rendered = format!("{loader:?}");
    // The source is a trait object and cannot be shown; the policy is what a
    // reader needs when a load fails unexpectedly.
    assert!(rendered.contains("SceneLoader"), "{rendered}");
    assert!(rendered.contains("namespaces"), "{rendered}");
    assert!(rendered.contains("bounds"), "{rendered}");
}

#[test]
fn a_replaced_namespace_table_is_the_one_enforced() {
    // The point of injecting the table is that phase 5 can add a namespace
    // without editing this crate. That only holds if the injected table is
    // what identifiers are actually checked against.
    let loader = SceneLoader::new(Arc::new(source())).with_namespaces(NamespaceTable::empty());
    let document = minimal_document();
    let Ok(bytes) = thysalion_world::codec::encode_document(&document, Encoding::Json) else {
        panic!("the minimal document must encode");
    };
    let Err(SceneLoadError::Invalid { diagnostics, .. }) =
        loader.load_bytes(&bytes, Encoding::Json)
    else {
        panic!("`thy:` must be unknown once the table is emptied");
    };
    assert!(carries(&diagnostics, DiagnosticCode::ConceptIriInvalid));
}

#[test]
fn a_document_from_a_future_build_is_refused_before_its_other_rules() {
    let mut document = minimal_document();
    document.version.major = document.version.major.saturating_add(1);
    let Err(diagnostics) = validate(&document) else {
        panic!("a major version this build does not support must not validate");
    };
    // Exactly one: a future document may legitimately break every other rule,
    // and reporting those would send the reader chasing consequences.
    assert_eq!(diagnostics.len(), 1, "{diagnostics:?}");
    assert!(carries(&diagnostics, DiagnosticCode::UnsupportedVersion));
}

#[test]
fn a_zero_chunk_size_is_refused_before_the_payload_is_sized_from_it() {
    let mut document = minimal_document();
    document.chunk_size = 0;
    let Err(diagnostics) = validate(&document) else {
        panic!("a zero chunk size must not validate");
    };
    // Both, and in this order: the design-conformance complaint is about the
    // value, and the zero complaint is why phase two cannot proceed.
    assert!(carries(&diagnostics, DiagnosticCode::ChunkSizeNotDesign));
    assert!(carries(&diagnostics, DiagnosticCode::ZeroDimension));
}

#[test]
fn a_chunk_edge_whose_cube_overflows_is_refused() {
    // `ChunkSize::volume` is `const` and infallible, so it has nowhere to
    // report an overflow: the edge has to be refused at construction or the
    // cube panics in debug builds and wraps in release ones. Every real size is
    // far below this — design §7.1 fixes 32.
    assert!(ChunkSize::new(32).is_ok());
    assert!(
        ChunkSize::new(2_642_245).is_ok(),
        "the largest cube that fits"
    );
    assert!(
        ChunkSize::new(2_642_246).is_err(),
        "the first that does not"
    );
    assert!(ChunkSize::new(u32::MAX).is_err());
    assert!(ChunkSize::new(0).is_err());
}

#[test]
fn coordinate_arithmetic_saturates_rather_than_wrapping() {
    let Ok(chunk_size) = ChunkSize::new(32) else {
        panic!("32 must construct");
    };
    // Both are public and take arbitrary `u32`s. Wrapping would put a position
    // far outwith the grid back inside it, which is worse than a large answer:
    // the caller's bounds check would pass.
    let far = ChunkCoord::new(u32::MAX, u32::MAX, u32::MAX);
    assert_eq!(
        far.origin(chunk_size),
        VoxelPos {
            x: u32::MAX,
            y: u32::MAX,
            z: u32::MAX,
        }
    );
    let outwith = VoxelPos {
        x: u32::MAX,
        y: u32::MAX,
        z: u32::MAX,
    };
    assert!(
        outwith.local_index(chunk_size) >= chunk_size.volume(),
        "a position outwith the chunk must yield an index the bounds check rejects"
    );
}
