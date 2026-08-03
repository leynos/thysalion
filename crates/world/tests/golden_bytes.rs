//! Golden bytes: the only test in this suite capable of noticing that the
//! encoding drifted between builds.
//!
//! The round-trip properties prove symmetry *within one compilation*, and the
//! generated fixtures are regenerated from their sources so they are
//! current-version by construction. Neither can fail when a field is added, a
//! variant renamed, or a struct silently switched to positional encoding — but
//! bytes committed by an earlier build can.
//!
//! Refresh these deliberately. A change here is a wire-format change, and it
//! must arrive with a `DocumentVersion` bump and an entry in the format
//! reference's version history. Regenerate with:
//!
//! ```text
//! cargo test -p thysalion-world --test golden_bytes -- --ignored regenerate
//! ```

use std::io;

use cap_std::{ambient_authority, fs_utf8::Dir};
use thysalion_world::codec::{Encoding, decode_document, encode_document};

mod support;

use support::minimal_document;

const GOLDEN_JSON: &str = "tests/fixtures/minimal.scene.json";
const GOLDEN_MSGPACK: &str = "tests/fixtures/golden/minimal.scene.msgpack";

/// Opens this crate's root as a capability, per AGENTS.md's filesystem policy.
///
/// Ambient authority is taken once, here, rather than by every call site: the
/// point of `cap_std` is that a reader can see the whole filesystem surface a
/// module touches by reading one function.
fn crate_dir() -> io::Result<Dir> {
    Dir::open_ambient_dir(env!("CARGO_MANIFEST_DIR"), ambient_authority())
}

/// Reads a fixture relative to the crate root.
///
/// Fallible rather than panicking: an `expect` here would sit outwith a test
/// function, which `allow-expect-in-tests` does not cover (AGENTS.md).
fn read(relative: &str) -> io::Result<Vec<u8>> { crate_dir()?.read(relative) }

#[test]
fn the_checked_in_json_fixture_decodes_to_the_minimal_document() {
    let bytes = read(GOLDEN_JSON).expect("read the golden JSON");
    let decoded = decode_document(&bytes, Encoding::Json).expect("decode the golden JSON");
    assert_eq!(
        decoded,
        minimal_document(),
        "the checked-in fixture and the constructor in tests/support have diverged; one of them \
         is wrong, and which one depends on whether the wire format was meant to change"
    );
}

#[test]
fn the_checked_in_messagepack_fixture_is_byte_identical_to_a_fresh_encoding() {
    let expected = read(GOLDEN_MSGPACK).expect("read the golden MessagePack");
    let actual = encode_document(&minimal_document(), Encoding::MessagePack).expect("encode");
    assert_eq!(
        actual, expected,
        "the MessagePack encoding has drifted from the committed golden bytes. This is a \
         wire-format change: it needs a DocumentVersion bump, not a regenerated fixture"
    );
}

#[test]
fn the_content_hash_input_agrees_across_the_two_encodings() {
    // Phase 8 hashes the canonical MessagePack encoding of the validated
    // document, independent of the encoding actually read (design §12.3). A
    // scene loaded from JSON and the same scene loaded from MessagePack must
    // therefore produce identical bytes to hash, or a save taken in a
    // development build would refuse itself in a shipped one.
    let json = read(GOLDEN_JSON).expect("read json");
    let msgpack = read(GOLDEN_MSGPACK).expect("read msgpack");
    let from_json = decode_document(&json, Encoding::Json).expect("decode json");
    let from_msgpack = decode_document(&msgpack, Encoding::MessagePack).expect("decode msgpack");
    assert_eq!(from_json, from_msgpack);

    let via_json = encode_document(&from_json, Encoding::MessagePack).expect("encode");
    let via_msgpack = encode_document(&from_msgpack, Encoding::MessagePack).expect("encode");
    assert_eq!(
        via_json, via_msgpack,
        "the canonical form must not depend on the input form"
    );
}

/// Rewrites the golden MessagePack fixture from the current encoder.
///
/// Ignored by default: running it is a deliberate act of changing the wire
/// format, not a way to make a failing test pass.
#[test]
#[ignore = "regenerating a golden fixture is a deliberate wire-format change"]
fn regenerate() {
    let bytes = encode_document(&minimal_document(), Encoding::MessagePack).expect("encode");
    let dir = crate_dir().expect("open crate root");
    dir.create_dir_all("tests/fixtures/golden")
        .expect("create fixture directory");
    dir.write(GOLDEN_MSGPACK, bytes)
        .expect("write golden fixture");
}
