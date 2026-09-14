//! The replay session's wire types.
//!
//! Every type here is a *wire* type in ADR 006's sense: it derives
//! `Serialize` and `Deserialize`, carries `#[serde(deny_unknown_fields)]`, and
//! holds only integers, `SmolStr`, `String`, `Option`, `Vec`, and other wire
//! types. There is no domain form to convert to, because the envelope carries
//! no invariant a constructor could enforce — the one rule it does have
//! (strictly increasing ticks) is checked at the encode and decode boundaries
//! in `super::session`, where an untrusted byte stream actually arrives.

use serde::{Deserialize, Serialize};
use smol_str::SmolStr;

/// The replay format version this build writes and understands.
pub const SUPPORTED_VERSION: FormatVersion = FormatVersion { major: 1, minor: 0 };

/// The replay format's schema version.
///
/// A `{major, minor}` pair with an accepted *range*, copied from ADR 006's
/// `DocumentVersion` rather than reinvented: adding a field — or the first
/// [`InputRecord`] variant at roadmap 4.1.2 — bumps the minor and the field
/// carries `#[serde(default)]`; removing, retyping, or re-meaning a field
/// bumps the major.
///
/// Not a semantic version. A file format has one axis of change, and semantic
/// versioning only invites arguments about which component to bump.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FormatVersion {
    /// Incompatible reinterpretation of an existing field.
    pub major: u16,
    /// Additive change; readers of an older minor still work.
    pub minor: u16,
}

impl FormatVersion {
    /// Whether a reader supporting `self` can read a session declaring
    /// `session`.
    ///
    /// # Examples
    ///
    /// ```
    /// use thysalion_test_support::replay::FormatVersion;
    ///
    /// let reader = FormatVersion { major: 1, minor: 2 };
    /// assert!(reader.accepts(FormatVersion { major: 1, minor: 0 }));
    /// assert!(!reader.accepts(FormatVersion { major: 1, minor: 3 }));
    /// assert!(!reader.accepts(FormatVersion { major: 2, minor: 0 }));
    /// ```
    #[must_use]
    pub const fn accepts(self, session: Self) -> bool {
        self.major == session.major && session.minor <= self.minor
    }
}

impl core::fmt::Display for FormatVersion {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "{}.{}", self.major, self.minor)
    }
}

/// Names the scene a session ran against.
///
/// Both halves are needed and neither is sufficient. The name locates the
/// fixture a human would reach for; the content hash — the BLAKE3 digest
/// `thysalion_world` already computes over a scene's canonical MessagePack
/// form — is what tells a replayer that the fixture on disk is still the one
/// the session was recorded against. Replaying a recorded session against a
/// silently edited scene is the failure mode this field exists to refuse.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SceneRef {
    /// The scene's stable machine name, as `SceneDocument::name` spells it.
    pub name: SmolStr,
    /// Lowercase hexadecimal of the scene's content hash.
    ///
    /// A `String` rather than a `SmolStr` because a BLAKE3 digest is 64 hex
    /// characters, well past `SmolStr`'s inline capacity: the small-string
    /// optimization would never apply and the extra type would buy nothing.
    pub content_hash_hex: String,
}

/// Everything a replayer needs before the first tick.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SessionHeader {
    /// Schema version. Read by a permissive probe before this structure is
    /// deserialized, so a session from a future build reports itself as an
    /// unsupported version rather than as an unknown field.
    pub version: FormatVersion,
    /// The fixed simulation tick rate the session was recorded at, in hertz.
    ///
    /// Recorded rather than assumed: a session replayed at a different rate
    /// reproduces different outputs, so the rate is part of what makes the
    /// recording self-describing.
    pub tick_rate_hz: u32,
    /// The scene the session ran against, when it ran against one.
    ///
    /// `None` is legitimate: a session that exercises the circuit with no
    /// loaded scene — the empty session this step's byte-identity test
    /// records — has no scene to name.
    pub scene: Option<SceneRef>,
}

/// One tick's recorded inputs.
///
/// Tick numbers are strictly increasing across a session, which
/// the recorder and the replayer both enforce. They are not required to be
/// contiguous: a tick at which nothing was input need not be recorded, and
/// omitting it is what keeps a long idle session small.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TickRecord {
    /// The simulation tick these inputs precede.
    pub tick: u64,
    /// The inputs recorded at this tick.
    ///
    /// Necessarily empty until roadmap step 4.1.2: [`InputRecord`] is
    /// uninhabited, so no value can be pushed here. See the module comment on
    /// [`super`] for why that is deliberate.
    pub inputs: Vec<InputRecord>,
}

/// One domain-level input, as design §6.2's input phase writes it.
///
/// **Uninhabited, deliberately.** No input vocabulary exists before roadmap
/// step 4.1.2 defines the circuit boundary, and inventing one now would put
/// speculation on the wire of a format whose recordings are meant to outlive
/// the build that wrote them. The first real variants arrive with a minor
/// version bump.
///
/// When they do, they record *domain-level* inputs — commands, intents,
/// waypoints — and never raw window or key events. Design §14's I1 is stated
/// over input records at the circuit boundary, and recording operating-system
/// events instead would pin the invariant to a seam it is not about; ADR 007
/// records that rejection and its grounds.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub enum InputRecord {}
