//! Strict decoding boundaries for recordings supplied by another build.
//!
//! These checks build MessagePack directly because the current encoder must
//! refuse malformed tick order and versions it cannot write. Keeping them
//! separate from the byte-identity tests also keeps each test file focused on
//! one boundary.

use proptest::prelude::{any, prop_assert, prop_assert_eq};
use rmpv::Value;
use rstest::rstest;
use thysalion_test_support::replay::{
    FormatVersion,
    ReplayDecodeError,
    ReplayEncodeError,
    SUPPORTED_VERSION,
    SessionHeader,
    SessionRecorder,
    SessionReplayer,
    TickRecord,
};

const fn empty_header() -> SessionHeader {
    SessionHeader {
        version: SUPPORTED_VERSION,
        tick_rate_hz: 60,
        scene: None,
    }
}

/// Encodes a session this build's recorder would refuse to write.
///
/// Built from `rmpv` values rather than from the recorder: these tests model a
/// recording another build supplied to the decoder.
fn foreign_session_value(version: (u16, u16), ticks: &[u64]) -> Value {
    let (major, minor) = version;
    let header = Value::Map(vec![
        (
            Value::from("version"),
            Value::Map(vec![
                (Value::from("major"), Value::from(major)),
                (Value::from("minor"), Value::from(minor)),
            ]),
        ),
        (Value::from("tick_rate_hz"), Value::from(60)),
        (Value::from("scene"), Value::Nil),
    ]);
    let recorded_ticks = Value::Array(
        ticks
            .iter()
            .map(|tick| {
                Value::Map(vec![
                    (Value::from("tick"), Value::from(*tick)),
                    (Value::from("inputs"), Value::Array(Vec::new())),
                ])
            })
            .collect(),
    );
    Value::Map(vec![
        (Value::from("header"), header),
        (Value::from("ticks"), recorded_ticks),
    ])
}

/// Encodes a hand-built named-map session.
fn encode_foreign_session(session: &Value) -> Vec<u8> {
    match rmp_serde::to_vec_named(session) {
        Ok(bytes) => bytes,
        Err(error) => panic!("the hand-built session must encode: {error}"),
    }
}

fn foreign_session(version: (u16, u16), ticks: &[u64]) -> Vec<u8> {
    encode_foreign_session(&foreign_session_value(version, ticks))
}

#[derive(Debug, Clone, Copy)]
enum UnknownFieldLocation {
    Session,
    Header,
    Tick,
}

fn map_field_mut<'a>(value: &'a mut Value, name: &str) -> Option<&'a mut Value> {
    let Value::Map(fields) = value else {
        return None;
    };
    fields
        .iter_mut()
        .find_map(|(key, field_value)| (key.as_str() == Some(name)).then_some(field_value))
}

fn unknown_field_target_mut(
    session: &mut Value,
    location: UnknownFieldLocation,
) -> Option<&mut Value> {
    match location {
        UnknownFieldLocation::Session => Some(session),
        UnknownFieldLocation::Header => map_field_mut(session, "header"),
        UnknownFieldLocation::Tick => {
            let Value::Array(ticks) = map_field_mut(session, "ticks")? else {
                return None;
            };
            ticks.first_mut()
        }
    }
}

fn foreign_session_with_unknown_field(
    version: (u16, u16),
    ticks: &[u64],
    location: UnknownFieldLocation,
) -> Option<Vec<u8>> {
    let mut session = foreign_session_value(version, ticks);
    let Value::Map(fields) = unknown_field_target_mut(&mut session, location)? else {
        return None;
    };
    fields.push((Value::from("future_field"), Value::Nil));
    Some(encode_foreign_session(&session))
}

#[test]
fn the_foreign_session_helper_agrees_with_the_recorder() {
    let built = foreign_session((SUPPORTED_VERSION.major, SUPPORTED_VERSION.minor), &[]);
    let recorded = SessionRecorder::new(empty_header())
        .finish()
        .expect("record the empty session");

    assert_eq!(
        built, recorded,
        "the hand-built wire shape has drifted from the recorder's"
    );
}

#[test]
fn a_session_from_a_future_major_version_is_refused_as_one() {
    let future = SUPPORTED_VERSION.major.saturating_add(1);
    let bytes = foreign_session((future, 0), &[]);
    match SessionReplayer::open(&bytes) {
        Err(ReplayDecodeError::UnsupportedVersion { found, supported }) => {
            assert_eq!(found.major, future);
            assert_eq!(supported, SUPPORTED_VERSION);
        }
        other => panic!("expected an unsupported-version refusal, got: {other:?}"),
    }
}

#[test]
fn a_future_minor_with_an_unknown_field_is_refused_before_strict_decoding() {
    let future_minor = SUPPORTED_VERSION.minor.saturating_add(1);
    let Some(bytes) = foreign_session_with_unknown_field(
        (SUPPORTED_VERSION.major, future_minor),
        &[],
        UnknownFieldLocation::Session,
    ) else {
        panic!("the future-version session must have a root map");
    };

    match SessionReplayer::open(&bytes) {
        Err(ReplayDecodeError::UnsupportedVersion { found, supported }) => {
            assert_eq!(
                found,
                FormatVersion {
                    major: SUPPORTED_VERSION.major,
                    minor: future_minor,
                }
            );
            assert_eq!(supported, SUPPORTED_VERSION);
        }
        other => panic!("expected an unsupported-version refusal, got: {other:?}"),
    }
}

#[rstest]
#[case::session(UnknownFieldLocation::Session, vec![])]
#[case::header(UnknownFieldLocation::Header, vec![])]
#[case::tick(UnknownFieldLocation::Tick, vec![3])]
fn unknown_fields_are_rejected_at_the_strict_decode_boundary(
    #[case] location: UnknownFieldLocation,
    #[case] ticks: Vec<u64>,
) {
    let Some(bytes) = foreign_session_with_unknown_field(
        (SUPPORTED_VERSION.major, SUPPORTED_VERSION.minor),
        &ticks,
        location,
    ) else {
        panic!("the selected {location:?} map must exist");
    };

    assert!(
        matches!(
            SessionReplayer::open(&bytes),
            Err(ReplayDecodeError::Malformed { .. })
        ),
        "unknown fields at {location:?} must be malformed"
    );
}

#[test]
fn trailing_messagepack_values_are_rejected() {
    let mut bytes = SessionRecorder::new(empty_header())
        .finish()
        .expect("record the empty session");
    bytes.push(0xc0); // MessagePack nil, a second value after the session.

    assert!(
        matches!(
            SessionReplayer::open(&bytes),
            Err(ReplayDecodeError::Malformed { .. })
        ),
        "the decoder must consume exactly one session value"
    );
}

#[test]
fn ticks_that_do_not_increase_are_refused_at_the_decode_boundary() {
    let bytes = foreign_session((SUPPORTED_VERSION.major, SUPPORTED_VERSION.minor), &[7, 7]);
    match SessionReplayer::open(&bytes) {
        Err(ReplayDecodeError::NonMonotonicTick { previous, found }) => {
            assert_eq!((previous, found), (7, 7));
        }
        other => panic!("expected the decoder to refuse the tick order, got: {other:?}"),
    }
}

fn first_non_increasing_pair(ticks: &[u64]) -> Option<(u64, u64)> {
    ticks
        .iter()
        .zip(ticks.iter().skip(1))
        .find_map(|(previous, next)| (next <= previous).then_some((*previous, *next)))
}

proptest::proptest! {
    #[test]
    fn tick_order_is_enforced_at_encode_and_decode_boundaries(
        ticks in proptest::collection::vec(any::<u64>(), 0..32)
    ) {
        let first_violation = first_non_increasing_pair(&ticks);
        let mut recorder = SessionRecorder::new(empty_header());
        for tick in &ticks {
            recorder.record_tick(TickRecord {
                tick: *tick,
                inputs: Vec::new(),
            });
        }

        match (first_violation, recorder.finish()) {
            (None, Ok(_)) => {}
            (
                Some((expected_previous, expected_found)),
                Err(ReplayEncodeError::NonMonotonicTick { previous, found }),
            ) => prop_assert_eq!((previous, found), (expected_previous, expected_found)),
            (expected, actual) => prop_assert!(
                false,
                "encode result did not match first non-increasing pair {expected:?}: {actual:?}"
            ),
        }

        let bytes = foreign_session(
            (SUPPORTED_VERSION.major, SUPPORTED_VERSION.minor),
            &ticks,
        );
        match (first_violation, SessionReplayer::open(&bytes)) {
            (None, Ok(session)) => {
                let decoded_ticks: Vec<u64> = session.ticks().map(|record| record.tick).collect();
                prop_assert_eq!(decoded_ticks, ticks);
                let reencoded = session.re_encode().map_err(|error| {
                    proptest::test_runner::TestCaseError::fail(format!(
                        "valid ordered session must re-encode: {error}"
                    ))
                })?;
                prop_assert_eq!(reencoded, bytes);
            }
            (
                Some((expected_previous, expected_found)),
                Err(ReplayDecodeError::NonMonotonicTick { previous, found }),
            ) => prop_assert_eq!((previous, found), (expected_previous, expected_found)),
            (expected, actual) => prop_assert!(
                false,
                "decode result did not match first non-increasing pair {expected:?}: {actual:?}"
            ),
        }
    }
}
