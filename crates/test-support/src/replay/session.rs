//! Recording and replaying sessions.
//!
//! Three types, one round trip: [`SessionRecorder`] accumulates ticks and
//! encodes them, [`SessionReplayer`] decodes a byte stream into a
//! [`RecordedSession`], and [`RecordedSession::re_encode`] writes the same
//! bytes back. The equality of those two byte strings is roadmap task 1.3.2's
//! criterion and `tests/replay_round_trip.rs` asserts it.
//!
//! MessagePack is written with `to_vec_named`, never `to_vec`. As ADR 006
//! records for scene documents, that is a *correctness* requirement rather
//! than a preference for evolvability: `rmp_serde` writes structs as
//! positional arrays by default, its decoder accepts either shape silently,
//! and `deny_unknown_fields` has no field names to act on in the array form.
//! An accidental `to_vec` would ship a corpus that is positional, permanently
//! unevolvable, and rejects nothing, while looking healthy to any round-trip
//! test — so the round-trip test alone cannot catch it, and the checked-in
//! golden bytes are what do.

use serde::{Deserialize, Serialize};
use smol_str::SmolStr;

use super::{
    format::{SUPPORTED_VERSION, SessionHeader, TickRecord},
    probe::SessionProbe,
};

/// One recorded session, as it appears on the wire.
///
/// Field order is the declaration order below and `serde` preserves it, which
/// is what makes the encoding byte-stable. Private to this module: the header
/// and the ticks reach callers through [`RecordedSession`], so there is no
/// route to a session document that skipped the tick-ordering check.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct SessionDocument {
    /// Everything a replayer needs before the first tick.
    header: SessionHeader,
    /// The recorded ticks, in strictly increasing tick order.
    ticks: Vec<TickRecord>,
}

/// Why a session could not be encoded.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[non_exhaustive]
pub enum ReplayEncodeError {
    /// The underlying encoder failed.
    #[error("cannot encode the replay session: {message}")]
    Encode {
        /// The encoder's message.
        message: SmolStr,
    },
    /// The accumulated ticks are not in strictly increasing order.
    #[error("tick {found} does not follow tick {previous}: a session's ticks must increase")]
    NonMonotonicTick {
        /// The tick recorded immediately before the offending one.
        previous: u64,
        /// The offending tick.
        found: u64,
    },
}

/// Why a session could not be decoded.
///
/// Distinct from an encode failure, and deliberately not merged with it: a
/// decode failure is a statement about bytes somebody else wrote, and the
/// replay corpus is exactly the place where those bytes are older than the
/// build reading them.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[non_exhaustive]
pub enum ReplayDecodeError {
    /// The bytes are not a well-formed session.
    #[error("malformed replay session: {message}")]
    Malformed {
        /// The decoder's message.
        message: SmolStr,
    },
    /// The session declares a version this build cannot read.
    #[error("unsupported replay session version {found}; this build supports {supported}")]
    UnsupportedVersion {
        /// The version the session declares.
        found: super::format::FormatVersion,
        /// The version this build writes and understands.
        supported: super::format::FormatVersion,
    },
    /// The decoded ticks are not in strictly increasing order.
    #[error("tick {found} does not follow tick {previous}: a session's ticks must increase")]
    NonMonotonicTick {
        /// The tick recorded immediately before the offending one.
        previous: u64,
        /// The offending tick.
        found: u64,
    },
}

/// The first adjacent pair of ticks that does not strictly increase.
///
/// Returns the pair rather than a boolean so both boundaries can name the
/// offending ticks: "some tick is out of order" sends a reader looking through
/// a session that may be millions of ticks long.
fn first_non_increasing(ticks: &[TickRecord]) -> Option<(u64, u64)> {
    ticks.windows(2).find_map(|pair| match pair {
        [previous, next] if next.tick <= previous.tick => Some((previous.tick, next.tick)),
        _ => None,
    })
}

/// Encodes a session document in the canonical MessagePack form.
fn encode(document: &SessionDocument) -> Result<Vec<u8>, ReplayEncodeError> {
    if let Some((previous, found)) = first_non_increasing(&document.ticks) {
        return Err(ReplayEncodeError::NonMonotonicTick { previous, found });
    }
    rmp_serde::to_vec_named(document).map_err(|error| ReplayEncodeError::Encode {
        message: SmolStr::new(error.to_string()),
    })
}

/// Deserializes `T` from a complete MessagePack value.
///
/// A session is one value, not a prefix of a stream. MessagePack has no
/// closing delimiter, so an appended second session is indistinguishable from
/// a longer first one unless the consumed length is checked.
fn decode<T: serde::de::DeserializeOwned>(bytes: &[u8]) -> Result<T, ReplayDecodeError> {
    // A `Cursor` rather than the slice directly: `position` — the only way to
    // tell a complete session from a prefix of one — is implemented for the
    // cursor-backed reader alone.
    let mut deserializer = rmp_serde::Deserializer::new(std::io::Cursor::new(bytes));
    let value =
        T::deserialize(&mut deserializer).map_err(|error| ReplayDecodeError::Malformed {
            message: SmolStr::new(error.to_string()),
        })?;
    let consumed = usize::try_from(deserializer.position()).unwrap_or(usize::MAX);
    if consumed != bytes.len() {
        return Err(ReplayDecodeError::Malformed {
            message: SmolStr::new(format!(
                "trailing input after the session: {} byte(s) unread",
                bytes.len().saturating_sub(consumed)
            )),
        });
    }
    Ok(value)
}

/// Accumulates a session's ticks and encodes them.
///
/// Ticks must be recorded in strictly increasing order. [`Self::record_tick`]
/// stays infallible so a recording loop reads as a loop; the ordering rule is
/// checked once, at [`Self::finish`], where a violation can be reported
/// without the caller having to handle a `Result` on every tick.
#[derive(Debug, Clone)]
pub struct SessionRecorder {
    document: SessionDocument,
}

impl SessionRecorder {
    /// Starts a recording against `header`.
    #[must_use]
    pub const fn new(header: SessionHeader) -> Self {
        Self {
            document: SessionDocument {
                header,
                ticks: Vec::new(),
            },
        }
    }

    /// Appends one tick's inputs to the recording.
    pub fn record_tick(&mut self, record: TickRecord) { self.document.ticks.push(record); }

    /// Encodes the recording.
    ///
    /// # Errors
    ///
    /// Returns [`ReplayEncodeError::NonMonotonicTick`] when the recorded ticks
    /// do not strictly increase, and [`ReplayEncodeError::Encode`] when the
    /// underlying encoder fails.
    pub fn finish(self) -> Result<Vec<u8>, ReplayEncodeError> { encode(&self.document) }
}

/// Opens recorded sessions.
///
/// A unit struct rather than a free function so the decode half of the format
/// has a name a reader can search for, and so a future replayer that needs
/// configuration — a tick budget, a scene-hash policy — has somewhere to put
/// it without changing every call site.
#[derive(Debug, Clone, Copy, Default)]
pub struct SessionReplayer;

impl SessionReplayer {
    /// Decodes a recorded session, probing its version before deserializing.
    ///
    /// # Errors
    ///
    /// Returns [`ReplayDecodeError::UnsupportedVersion`] when the session
    /// declares a version outwith this build's range,
    /// [`ReplayDecodeError::NonMonotonicTick`] when its ticks do not strictly
    /// increase, and [`ReplayDecodeError::Malformed`] when the bytes do not
    /// parse.
    pub fn open(bytes: &[u8]) -> Result<RecordedSession, ReplayDecodeError> {
        let probed = decode::<SessionProbe>(bytes)?.header.version;
        if !SUPPORTED_VERSION.accepts(probed) {
            return Err(ReplayDecodeError::UnsupportedVersion {
                found: probed,
                supported: SUPPORTED_VERSION,
            });
        }
        let document: SessionDocument = decode(bytes)?;
        if let Some((previous, found)) = first_non_increasing(&document.ticks) {
            return Err(ReplayDecodeError::NonMonotonicTick { previous, found });
        }
        Ok(RecordedSession { document })
    }
}

/// A decoded session, ready to be replayed or re-encoded.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RecordedSession {
    document: SessionDocument,
}

impl RecordedSession {
    /// Everything the replayer needs before the first tick.
    #[must_use]
    pub const fn header(&self) -> &SessionHeader { &self.document.header }

    /// The recorded ticks, in strictly increasing tick order.
    pub fn ticks(&self) -> impl Iterator<Item = &TickRecord> { self.document.ticks.iter() }

    /// Re-encodes the session.
    ///
    /// The bytes must equal those the recorder produced. That equality is the
    /// whole of roadmap task 1.3.2's byte-identity criterion, and it holds
    /// *within one build*: cross-version byte-determinism is not promised, for
    /// the reason ADR 006 gives for scene documents.
    ///
    /// # Errors
    ///
    /// Returns [`ReplayEncodeError::Encode`] when the underlying encoder
    /// fails. The tick-ordering arm is unreachable for a session that came
    /// from [`SessionReplayer::open`], which already checked it.
    pub fn re_encode(&self) -> Result<Vec<u8>, ReplayEncodeError> { encode(&self.document) }
}
