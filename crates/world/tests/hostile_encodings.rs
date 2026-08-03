//! Hostile *encodings*: bytes that are malformed, truncated, or not one
//! document.
//!
//! Split from `hostile_inputs.rs`, which keeps the cases where a well-formed
//! document *declares* an enormous quantity. The two fail differently and are
//! defended differently — a declared quantity is refused by a bound in the
//! header phase, whereas these never reach validation at all — so keeping them
//! apart says which layer is under test.

use std::sync::Arc;

use thysalion_world::{
    codec::{Encoding, encode_document},
    loader::{SceneLoadError, SceneLoader},
    source::MemorySceneSource,
};

mod support;

#[path = "support/hostile.rs"]
mod hostile;

use hostile::within_bound;
use support::minimal_document;

#[test]
fn deeply_nested_json_does_not_overflow_the_stack() {
    // `serde_json` bounds recursion depth itself, and the assertion is only that
    // this returns an error rather than aborting the process — a stack overflow
    // is not a `Result` and cannot be reported as one.
    let depth = 100_000;
    let mut bytes = Vec::with_capacity(depth * 2);
    bytes.extend(std::iter::repeat_n(b'[', depth));
    bytes.extend(std::iter::repeat_n(b']', depth));

    let loader = SceneLoader::new(Arc::new(MemorySceneSource::new()));
    let outcome = within_bound("100,000 levels of nested JSON", || {
        loader.load_bytes(&bytes, Encoding::Json)
    });
    assert!(outcome.is_err(), "nested JSON must not load as a scene");
}

#[test]
fn a_truncated_messagepack_payload_is_refused() {
    let document = minimal_document();
    let bytes = match encode_document(&document, Encoding::MessagePack) {
        Ok(bytes) => bytes,
        Err(error) => panic!("the fixture must encode: {error}"),
    };
    let Some(truncated) = bytes.get(..bytes.len().div_euclid(2)) else {
        panic!("the encoding must be long enough to halve");
    };

    let loader = SceneLoader::new(Arc::new(MemorySceneSource::new()));
    let outcome = within_bound("a half-length MessagePack payload", || {
        loader.load_bytes(truncated, Encoding::MessagePack)
    });
    assert!(outcome.is_err(), "a truncated payload must not load");
}

#[test]
fn a_messagepack_payload_declaring_a_gigantic_array_is_refused() {
    // `0xdd` is `array32`, followed by a big-endian length. This claims four
    // billion elements and then supplies none. A decoder that pre-allocates from
    // the declared length exhausts memory before reading a byte of content,
    // which is why the length is a claim to be checked rather than a size to
    // trust.
    let bytes = vec![0xdd, 0xff, 0xff, 0xff, 0xff];

    let loader = SceneLoader::new(Arc::new(MemorySceneSource::new()));
    let outcome = within_bound("a MessagePack array32 claiming 4.29 billion items", || {
        loader.load_bytes(&bytes, Encoding::MessagePack)
    });
    assert!(outcome.is_err(), "a gigantic declared array must not load");
}

#[test]
fn an_empty_input_is_refused_rather_than_treated_as_an_empty_scene() {
    let loader = SceneLoader::new(Arc::new(MemorySceneSource::new()));
    for encoding in [Encoding::Json, Encoding::MessagePack] {
        let outcome = loader.load_bytes(&[], encoding);
        assert!(outcome.is_err(), "{encoding}: empty input must not load");
    }
}

#[test]
fn a_second_document_appended_to_the_first_is_refused() {
    // A document is one value, not the first value of a stream. Accepting a
    // prefix means a file holding two scenes — or one scene and the tail of an
    // interrupted write — loads as whichever came first, silently, and the
    // content hash then describes bytes nobody can reproduce from the file.
    let document = minimal_document();
    let Ok(json) = encode_document(&document, Encoding::Json) else {
        panic!("the minimal document must encode as JSON");
    };
    let Ok(msgpack) = encode_document(&document, Encoding::MessagePack) else {
        panic!("the minimal document must encode as MessagePack");
    };

    for (encoding, one) in [(Encoding::Json, json), (Encoding::MessagePack, msgpack)] {
        let mut bytes = one.clone();
        bytes.extend_from_slice(&one);
        let loader = SceneLoader::new(Arc::new(MemorySceneSource::new()));
        let outcome = loader.load_bytes(&bytes, encoding).map(|_| ());
        let Err(SceneLoadError::Malformed { message, .. }) = outcome else {
            panic!("{encoding}: a doubled document must be refused as malformed");
        };
        assert!(
            message.contains("trailing input"),
            "{encoding}: got {message}"
        );
    }
}
