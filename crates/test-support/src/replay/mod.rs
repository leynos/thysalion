//! The deterministic replay envelope (design §14, invariant I1).
//!
//! Design §14 commits to a replay harness that re-executes recorded play
//! sessions in continuous integration, and to a replay *corpus* that
//! accumulates as the combinatorial-coverage strategy. Recorded sessions are
//! therefore long-lived artefacts rather than throwaway scaffolding, which is
//! why the storage format is versioned from the first byte.
//!
//! # What exists here, and what deliberately does not
//!
//! This is the *envelope only*: a versioned [`SessionHeader`] followed by a
//! stream of tick-stamped [`TickRecord`]s. The payload type [`InputRecord`] is
//! uninhabited, so [`TickRecord::inputs`] can only ever be empty until roadmap
//! step 4.1.2 defines the first real variants with a minor version bump.
//!
//! That is a deliberate refusal rather than an omission. `thysalion-sim` is an
//! empty skeleton until phase 4, so any record vocabulary invented now would be
//! speculation — and in a corpus format, speculation hardens into permanent
//! wire-compatibility burden. Even a `Placeholder` variant would be baggage no
//! later version could drop. An uninhabited enum lets the envelope, the version
//! rule, and the byte-identity test all exist while making a non-empty payload
//! unrepresentable.
//!
//! # Rules the format inherits
//!
//! `docs/adr-007-replay-record-format.md` owns these; they are restated where
//! they bite so a reader editing a wire type sees them:
//!
//! - MessagePack is written with `rmp_serde::to_vec_named`, never `to_vec`.
//! - Every wire type carries `#[serde(deny_unknown_fields)]`; none uses `flatten`, `untagged`, a
//!   tuple struct, or a hand-written `serde` implementation.
//! - Every quantity is an integer, and every map would be a `BTreeMap`.
//! - The version is a `{major, minor}` pair read by a permissive probe before the full document,
//!   exactly as ADR 006 does for scene documents.
//!
//! # The byte-identity promise
//!
//! Recording a session, replaying it, and re-encoding it yields identical
//! bytes *within one build*. That is roadmap task 1.3.2's criterion, and
//! `tests/replay_round_trip.rs` asserts it alongside checked-in golden bytes.
//! Cross-*version* wire byte-determinism is explicitly not promised, exactly as
//! ADR 006 declines to promise it for scenes; the golden bytes exist so a
//! change to the encoding is noticed in review rather than shipped silently.

mod format;
mod probe;
mod session;

pub use format::{
    FormatVersion,
    InputRecord,
    SUPPORTED_VERSION,
    SceneRef,
    SessionHeader,
    TickRecord,
};
pub use session::{
    RecordedSession,
    ReplayDecodeError,
    ReplayEncodeError,
    SessionRecorder,
    SessionReplayer,
};
