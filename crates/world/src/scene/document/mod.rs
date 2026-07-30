//! The scene document: the wire form of an authored scene (design §7.3).
//!
//! One Rust structure serves both encodings — JSON for authoring and diffing,
//! MessagePack for shipping. That is only true because of a closed rule on
//! what a document type may contain: integers, booleans, `String`, `SmolStr`,
//! `Option`, `Vec`, `BTreeMap` with string keys, plain derived enums, and other
//! document types. No manual `serde` implementation, no `Uuid`, no `chrono`, no
//! network address type, no `serde_json::Value`.
//!
//! The rule exists because `serde` lets an implementation branch on
//! [`Serializer::is_human_readable`](serde::Serializer::is_human_readable), and
//! a type that does so encodes differently in the two formats while
//! round-tripping perfectly in each. The offenders are few — the `std::net`
//! address types, `uuid::Uuid`, some `chrono` types — but the failure is
//! invisible until a shipped scene decodes differently from the authored one.
//! Type discipline avoids it; the round-trip property in
//! `tests/document_round_trip.rs` enforces it.
//!
//! Two further rules keep the encoding canonical, which design §12.3's save
//! content hashes depend on: every map is a `BTreeMap` with a string key, and
//! no field carries `#[serde(skip)]` or `#[serde(skip_serializing_if)]`, both
//! of which would make the encoded map length value-dependent.

mod sections;
mod voxel_type;
mod voxels;

use schemars::JsonSchema;
pub use sections::{
    AmbientBandDocument,
    EntitiesDocument,
    KnowledgeDocument,
    LightingDocument,
    PrototypeDocument,
    SpawnDocument,
    SunPathDocument,
};
use serde::{Deserialize, Serialize};
use smol_str::SmolStr;
pub use voxel_type::{
    EmissionDocument,
    FaceDocument,
    MaterialClassDocument,
    PassabilityDocument,
    SimDocument,
    SlopeDocument,
    VoxelTypeDocument,
};
pub use voxels::{
    ChunkCoordDocument,
    ChunkEntryDocument,
    ChunkPayloadDocument,
    ExtentDocument,
    VoxelPosDocument,
    VoxelRunDocument,
};

/// The document schema version this build writes and understands.
pub const SUPPORTED_VERSION: DocumentVersion = DocumentVersion { major: 1, minor: 0 };

/// The scene format's schema version.
///
/// A major/minor pair with an accepted *range*, not a single monotonic integer
/// against a whitelist. Adding a field bumps the minor and the field carries
/// `#[serde(default)]`; removing, retyping, or re-meaning a field bumps the
/// major. A whitelist would make every additive change breaking, and the
/// collision is immediate — phase 3 adds a fog volume to the lighting section,
/// and the checked-in fixtures would fail to load until regenerated.
///
/// Not a semantic version: a file format has one axis of change, and semantic
/// versioning only invites arguments about which component to bump.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, JsonSchema,
)]
#[serde(deny_unknown_fields)]
pub struct DocumentVersion {
    /// Incompatible reinterpretation of an existing field.
    pub major: u16,
    /// Additive change; readers of an older minor still work.
    pub minor: u16,
}

impl DocumentVersion {
    /// Whether a reader supporting `self` can read a document declaring
    /// `document`.
    #[must_use]
    pub const fn accepts(self, document: Self) -> bool {
        self.major == document.major && document.minor <= self.minor
    }
}

impl core::fmt::Display for DocumentVersion {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "{}.{}", self.major, self.minor)
    }
}

/// One authored scene, as it appears on the wire.
///
/// Field order is the declaration order below, and `serde` preserves it, which
/// is half of what makes the encoding byte-stable; the other half is that every
/// map is sorted. Both are load-bearing for design §12.3.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct SceneDocument {
    /// Schema version. Read by a permissive probe before this structure is
    /// deserialized, so an unsupported version reports itself as one rather
    /// than as an unknown field.
    pub version: DocumentVersion,
    /// The scene's stable machine name.
    pub name: SmolStr,
    /// Grid bounds, in voxels. A multiple of `chunk_size` on every axis.
    pub dimensions: ExtentDocument,
    /// Chunk edge length, in voxels. Design §7.1 fixes this at 32.
    pub chunk_size: u32,
    /// The ordered palette. Index zero is always air.
    pub palette: Vec<VoxelTypeDocument>,
    /// Populated chunks, sorted by coordinate. An absent chunk is all air.
    pub voxels: Vec<ChunkEntryDocument>,
    /// Prototypes and spawns.
    pub entities: EntitiesDocument,
    /// Sun path, ambient bands, and probe spacing.
    pub lighting: LightingDocument,
    /// The scene's named graph and its TriG sources.
    pub knowledge: KnowledgeDocument,
}

/// A deliberately permissive view of a document's version field.
///
/// Decoding probes this *before* attempting [`SceneDocument`], so a document
/// from a future build is reported as an unsupported version rather than as a
/// confusing complaint about an unknown field. It therefore must **not** carry
/// `deny_unknown_fields`: its whole job is to ignore everything it does not
/// recognize.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
pub struct VersionProbe {
    /// The declared schema version.
    pub version: DocumentVersion,
}
