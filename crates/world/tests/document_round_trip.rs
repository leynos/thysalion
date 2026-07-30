//! Wire-format contract for the scene document (roadmap task 1.2.1).
//!
//! Four distinct properties, none of which subsumes another:
//!
//! - each encoding round-trips itself,
//! - the two encodings agree with each other,
//! - MessagePack ships as a map rather than a positional array, and
//! - encoding is byte-deterministic.
//!
//! Every divergence class that matters here — struct-as-array versus
//! struct-as-map, tuple-struct arity, variant index versus variant name —
//! round-trips perfectly *within* one format while the two formats disagree
//! about the bytes, so a chain that crosses formats only once cannot separate
//! them. See the 1.2 execution plan, Stage B.

use proptest::{prelude::*, test_runner::FileFailurePersistence};
use thysalion_world::{
    codec::{Encoding, decode_document, encode_document},
    scene::document::{
        ChunkCoordDocument,
        ChunkEntryDocument,
        ChunkPayloadDocument,
        DocumentVersion,
        SceneDocument,
        VoxelRunDocument,
    },
};

#[path = "support/strategy.rs"]
mod strategy;
mod support;

use support::minimal_document;

/// A `version` field value, for tests that rewrite a document's version.
///
/// A free function rather than a mutating helper: `Value`'s `Index`
/// implementation panics on a type mismatch, which `clippy::indexing_slicing`
/// objects to, and a helper that unwrapped `as_object_mut` itself would be an
/// `expect` outwith a test function — which `allow-expect-in-tests` does not
/// cover. Keeping the unwrap in the caller keeps it where it is permitted.
fn version_value(major: u16, minor: u16) -> serde_json::Value {
    serde_json::json!({ "major": major, "minor": minor })
}

/// Renders any error as a `proptest` failure, so helpers can use `?` rather
/// than `expect`. `allow-expect-in-tests` covers test *functions*, not the
/// fixtures and helpers they call (AGENTS.md), and a panicking helper also
/// robs `proptest` of the chance to shrink a counterexample.
fn fail<E: core::fmt::Display>(error: E) -> TestCaseError { TestCaseError::fail(error.to_string()) }

/// Encodes to both forms and asserts every parity property at once.
fn assert_wire_contract(document: &SceneDocument) -> Result<(), TestCaseError> {
    let json = encode_document(document, Encoding::Json).map_err(fail)?;
    let msgpack = encode_document(document, Encoding::MessagePack).map_err(fail)?;

    let from_json = decode_document(&json, Encoding::Json).map_err(fail)?;
    let from_msgpack = decode_document(&msgpack, Encoding::MessagePack).map_err(fail)?;
    prop_assert_eq!(&from_json, document, "JSON did not round-trip itself");
    prop_assert_eq!(
        &from_msgpack,
        document,
        "MessagePack did not round-trip itself"
    );

    let re_encoded = encode_document(&from_json, Encoding::MessagePack).map_err(fail)?;
    let crossed = decode_document(&re_encoded, Encoding::MessagePack).map_err(fail)?;
    prop_assert_eq!(&crossed, document, "the two encodings disagree");

    let shape: rmpv::Value = rmp_serde::from_slice(&msgpack).map_err(fail)?;
    prop_assert!(
        shape.is_map(),
        "scene documents must ship as struct-as-map; an array-encoded document decodes silently \
         into an equal value, so only this assertion catches an accidental `to_vec`"
    );

    let again = encode_document(document, Encoding::MessagePack).map_err(fail)?;
    prop_assert_eq!(&msgpack, &again, "encoding is not byte-deterministic");
    Ok(())
}

proptest! {
    // `proptest`'s default persistence looks for a `lib.rs` or `main.rs`
    // beside the source, which an integration test has not got, so it warns
    // and then discards every counterexample it finds. Naming the file keeps
    // a shrunk failure reproducible across runs, which is most of the value of
    // a property test that only fails occasionally.
    #![proptest_config(ProptestConfig {
        failure_persistence: Some(Box::new(FileFailurePersistence::Direct(
            "tests/proptest-regressions/document_round_trip.txt",
        ))),
        ..ProptestConfig::default()
    })]

    /// The wire contract holds for arbitrary documents, not just the fixture.
    #[test]
    fn wire_contract_holds_for_generated_documents(document in strategy::scene_document()) {
        assert_wire_contract(&document)?;
    }
}

#[test]
fn wire_contract_holds_for_the_minimal_document() -> Result<(), TestCaseError> {
    assert_wire_contract(&minimal_document())
}

#[test]
fn json_re_encoding_reproduces_the_same_messagepack_bytes() {
    let document = minimal_document();
    let direct = encode_document(&document, Encoding::MessagePack).expect("encode");
    let json = encode_document(&document, Encoding::Json).expect("encode json");
    let via_json = decode_document(&json, Encoding::Json).expect("decode json");
    let indirect = encode_document(&via_json, Encoding::MessagePack).expect("re-encode");
    assert_eq!(
        direct, indirect,
        "a document round-tripped through JSON must re-encode to identical MessagePack bytes, or \
         the phase-8 content hash is not well defined"
    );
}

#[test]
fn messagepack_field_names_are_present_on_the_wire() {
    let document = minimal_document();
    let bytes = encode_document(&document, Encoding::MessagePack).expect("encode");
    let value: rmpv::Value = rmp_serde::from_slice(&bytes).expect("rmpv decode");
    let map = value.as_map().expect("top level is a map");
    let keys: Vec<&str> = map.iter().filter_map(|(k, _)| k.as_str()).collect();
    for expected in [
        "version",
        "name",
        "dimensions",
        "chunk_size",
        "palette",
        "voxels",
    ] {
        assert!(
            keys.contains(&expected),
            "missing wire key {expected}; keys were {keys:?}"
        );
    }
}

#[test]
fn an_unknown_field_is_rejected_under_both_encodings() {
    let document = minimal_document();
    let json = encode_document(&document, Encoding::Json).expect("encode");
    let mut value: serde_json::Value = serde_json::from_slice(&json).expect("parse");
    value
        .as_object_mut()
        .expect("object")
        .insert("fog_volume".to_owned(), serde_json::Value::Bool(true));

    let polluted_json = serde_json::to_vec(&value).expect("re-encode json");
    assert!(
        decode_document(&polluted_json, Encoding::Json).is_err(),
        "an unknown field must not be silently ignored"
    );
    let polluted_msgpack = rmp_serde::to_vec_named(&value).expect("re-encode msgpack");
    assert!(
        decode_document(&polluted_msgpack, Encoding::MessagePack).is_err(),
        "deny_unknown_fields is enforced under MessagePack only in the map form"
    );
}

#[test]
fn a_version_probe_precedes_full_deserialization() {
    let document = minimal_document();
    let json = encode_document(&document, Encoding::Json).expect("encode");
    let mut value: serde_json::Value = serde_json::from_slice(&json).expect("parse");
    // A future document: a newer minor *and* a field this build cannot know.
    value
        .as_object_mut()
        .expect("object")
        .insert("version".to_owned(), version_value(1, 7));
    value
        .as_object_mut()
        .expect("object")
        .insert("fog_volume".to_owned(), serde_json::Value::Bool(true));
    let future = serde_json::to_vec(&value).expect("re-encode");

    let error = decode_document(&future, Encoding::Json).expect_err("must reject");
    let rendered = error.to_string();
    assert!(
        rendered.contains("1.7"),
        "the version gate must run before the field check, so the reader is told the version is \
         unsupported rather than that a field is unknown; got: {rendered}"
    );
}

#[test]
fn a_newer_minor_version_is_accepted_when_the_document_is_otherwise_valid() {
    let mut document = minimal_document();
    document.version = DocumentVersion { major: 1, minor: 0 };
    let json = encode_document(&document, Encoding::Json).expect("encode");
    assert!(decode_document(&json, Encoding::Json).is_ok());
}

#[test]
fn a_newer_major_version_is_refused() {
    let document = minimal_document();
    let json = encode_document(&document, Encoding::Json).expect("encode");
    let mut value: serde_json::Value = serde_json::from_slice(&json).expect("parse");
    value
        .as_object_mut()
        .expect("object")
        .insert("version".to_owned(), version_value(2, 0));
    let future = serde_json::to_vec(&value).expect("re-encode");
    assert!(decode_document(&future, Encoding::Json).is_err());
}

#[test]
fn the_payload_enum_uses_newtype_variants() {
    let uniform = ChunkEntryDocument {
        at: ChunkCoordDocument { x: 1, y: 0, z: 0 },
        payload: ChunkPayloadDocument::Uniform(1),
    };
    let rendered = serde_json::to_string(&uniform).expect("encode");
    assert_eq!(
        rendered, r#"{"at":{"x":1,"y":0,"z":0},"payload":{"uniform":1}}"#,
        "struct variants would double the key to {{\"uniform\":{{\"voxel\":1}}}}"
    );

    let runs = ChunkPayloadDocument::Runs(vec![VoxelRunDocument {
        length: 4,
        index: 1,
    }]);
    let runs_rendered = serde_json::to_string(&runs).expect("encode");
    assert_eq!(runs_rendered, r#"{"runs":[{"length":4,"index":1}]}"#);
}
