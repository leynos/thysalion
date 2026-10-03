//! Byte-identity for the replay envelope: roadmap task 1.3.2's criterion.
//!
//! Three assertions, and each catches something the others cannot.
//!
//! The round trip — record, replay, re-record — proves symmetry *within one
//! compilation*, and nothing more: a wire type that switched to positional
//! encoding would round-trip perfectly and still have broken every recording
//! ever made. The wire-shape assertion is what notices that, by looking at the
//! encoder's output as MessagePack rather than as a Rust value. And the golden
//! bytes are what notice a change that is symmetric, named, and still a wire
//! break — a renamed field, a reordered struct, an added header.
//!
//! Refresh the golden bytes deliberately. A change there is a wire-format
//! change and must arrive with a `FormatVersion` bump and an entry in ADR
//! 007's version history. Regenerate with:
//!
//! ```text
//! cargo test -p thysalion-test-support --test replay_round_trip -- --ignored regenerate
//! ```

use std::io;

use cap_std::{ambient_authority, fs_utf8::Dir};
use thysalion_test_support::replay::{
    RecordedSession,
    ReplayEncodeError,
    SUPPORTED_VERSION,
    SceneRef,
    SessionHeader,
    SessionRecorder,
    SessionReplayer,
    TickRecord,
};

const GOLDEN_EMPTY: &str = "tests/fixtures/golden/empty.session.msgpack";

/// The session every assertion below starts from: a header and no ticks.
///
/// Empty deliberately: roadmap task 1.3.2 names the empty-session round trip
/// as the initial byte-identity criterion. The format can express populated
/// headers and tick records too; `InputRecord` remains uninhabited until
/// roadmap 4.1.2, so recorded ticks currently have empty input lists.
const fn empty_header() -> SessionHeader {
    SessionHeader {
        version: SUPPORTED_VERSION,
        tick_rate_hz: 60,
        scene: None,
    }
}

/// Records the empty session.
///
/// Fallible rather than panicking: an `expect` here would sit outwith a test
/// function, which `allow-expect-in-tests` does not cover (AGENTS.md).
fn record_empty() -> Result<Vec<u8>, thysalion_test_support::replay::ReplayEncodeError> {
    SessionRecorder::new(empty_header()).finish()
}

/// Opens this crate's root as a capability, per AGENTS.md's filesystem policy.
fn crate_dir() -> io::Result<Dir> {
    Dir::open_ambient_dir(env!("CARGO_MANIFEST_DIR"), ambient_authority())
}

#[test]
fn an_empty_session_round_trips_byte_identically() {
    let recorded = record_empty().expect("record the empty session");
    let session: RecordedSession = SessionReplayer::open(&recorded).expect("replay the recording");
    assert_eq!(session.header(), &empty_header());
    assert_eq!(session.ticks().count(), 0);

    let re_encoded = session.re_encode().expect("re-encode the replayed session");
    assert_eq!(
        re_encoded, recorded,
        "record, replay, and re-record must produce identical bytes"
    );
}

#[test]
fn a_populated_session_round_trips_byte_identically() {
    let header = SessionHeader {
        version: SUPPORTED_VERSION,
        tick_rate_hz: 120,
        scene: Some(SceneRef {
            name: "bare-cell".into(),
            content_hash_hex: "a".repeat(64),
        }),
    };
    let ticks = vec![
        TickRecord {
            tick: 3,
            inputs: Vec::new(),
        },
        TickRecord {
            tick: 11,
            inputs: Vec::new(),
        },
    ];
    let mut session_recorder = SessionRecorder::new(header.clone());
    for tick in &ticks {
        session_recorder.record_tick(tick.clone());
    }
    let recording_bytes = session_recorder
        .finish()
        .expect("record the populated session");

    let session = SessionReplayer::open(&recording_bytes).expect("replay the populated session");
    assert_eq!(session.header(), &header);
    assert_eq!(session.ticks().cloned().collect::<Vec<_>>(), ticks);
    assert_eq!(
        session
            .re_encode()
            .expect("re-encode the populated session"),
        recording_bytes,
        "record, replay, and re-record must preserve populated session bytes"
    );
}

#[test]
fn the_checked_in_golden_bytes_match_a_fresh_recording() {
    let expected = crate_dir()
        .and_then(|dir| dir.read(GOLDEN_EMPTY))
        .expect("read the golden session");
    let actual = record_empty().expect("record the empty session");
    assert_eq!(
        actual, expected,
        "the replay encoding has drifted from the committed golden bytes. This is a wire-format \
         change: it needs a FormatVersion bump and an ADR 007 version-history entry, not a \
         regenerated fixture"
    );
}

#[test]
fn the_session_is_written_as_named_maps_rather_than_positional_arrays() {
    // `rmp_serde` writes structs as positional arrays by default and its
    // decoder accepts either shape silently, so `deny_unknown_fields` would
    // have no field names to act on. Only looking at the bytes as MessagePack
    // catches an accidental `to_vec`; every Rust-level round trip passes
    // either way. See ADR 006, which made the same rule for scene documents.
    let bytes = record_empty().expect("record the empty session");
    let value: rmpv::Value = rmp_serde::from_slice(&bytes).expect("decode as a MessagePack value");
    let Some(fields) = value.as_map() else {
        panic!("the session must encode as a map, found: {value:?}");
    };
    let keys: Vec<&str> = fields.iter().filter_map(|(key, _)| key.as_str()).collect();
    assert_eq!(
        keys,
        vec!["header", "ticks"],
        "the session must encode as a named map in declaration order"
    );
}

#[test]
fn a_version_this_build_cannot_read_back_is_refused_at_the_encode_boundary() {
    // A recorder that writes a version its own decoder refuses produces a
    // corpus entry nothing can read, which is the worst outcome available to a
    // format whose recordings are meant to outlive the build that made them.
    let mut header = empty_header();
    header.version.minor = SUPPORTED_VERSION.minor.saturating_add(1);
    match SessionRecorder::new(header).finish() {
        Err(ReplayEncodeError::UnsupportedVersion { found, supported }) => {
            assert_eq!(found.minor, SUPPORTED_VERSION.minor.saturating_add(1));
            assert_eq!(supported, SUPPORTED_VERSION);
        }
        other => panic!("expected the recorder to refuse the version, got: {other:?}"),
    }
}

#[test]
fn ticks_that_do_not_increase_are_refused_at_the_encode_boundary() {
    let mut recorder = SessionRecorder::new(empty_header());
    for _ in 0..2 {
        recorder.record_tick(TickRecord {
            tick: 7,
            inputs: Vec::new(),
        });
    }
    let Err(error) = recorder.finish() else {
        panic!("a repeated tick must not encode");
    };
    assert!(
        matches!(
            error,
            ReplayEncodeError::NonMonotonicTick {
                previous: 7,
                found: 7
            }
        ),
        "the failure must name the offending pair, got: {error}"
    );
}

/// Rewrites the golden session fixture from the current encoder.
///
/// Ignored by default: running it is a deliberate act of changing the wire
/// format, not a way to make a failing test pass.
#[test]
#[ignore = "regenerating a golden fixture is a deliberate wire-format change"]
fn regenerate() {
    let bytes = record_empty().expect("record the empty session");
    let dir = crate_dir().expect("open crate root");
    dir.create_dir_all("tests/fixtures/golden")
        .expect("create fixture directory");
    dir.write(GOLDEN_EMPTY, bytes)
        .expect("write golden fixture");
}
