//! Wire types for the chunk-keyed voxel payload.
//!
//! The payload is keyed by chunk rather than being one global Z-major run
//! stream, so encoding, decoding, hashing, and diffing all scale with
//! populated volume instead of declared extent. A global stream fragments on
//! every raster row of a spatially localized region — see the 1.2 execution
//! plan's `Risks` for the arithmetic — and rewrites wholesale on a one-chunk
//! edit.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// A position in voxels, used for extents and spawn positions.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, JsonSchema,
)]
#[serde(deny_unknown_fields)]
pub struct VoxelPosDocument {
    /// Position along the `x` axis, in voxels.
    pub x: u32,
    /// Position along the `y` axis, in voxels.
    pub y: u32,
    /// Position along the `z` axis, in voxels.
    pub z: u32,
}

/// A scene's bounds, in voxels.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ExtentDocument {
    /// Extent along the `x` axis, in voxels.
    pub x: u32,
    /// Extent along the `y` axis, in voxels.
    pub y: u32,
    /// Extent along the `z` axis, in voxels.
    pub z: u32,
}

/// A chunk's position, counted in chunks rather than in voxels.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, JsonSchema,
)]
#[serde(deny_unknown_fields)]
pub struct ChunkCoordDocument {
    /// Chunk index along the `x` axis.
    pub x: u32,
    /// Chunk index along the `y` axis.
    pub y: u32,
    /// Chunk index along the `z` axis.
    pub z: u32,
}

/// One run of identical voxels in a chunk's Z-major stream.
///
/// A named struct, never a tuple struct. `rmp_serde`'s `serialize_tuple_struct`
/// writes a positional array unconditionally — it ignores the struct-map
/// configuration — so a tuple struct here could never gain a field, and adding
/// one would misalign the MessagePack stream while JSON errored cleanly. The
/// divergent failure mode is the objection, not the size.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct VoxelRunDocument {
    /// Consecutive voxels this run covers. Never zero in a valid document.
    pub length: u32,
    /// Palette index repeated across the run.
    pub index: u16,
}

/// A chunk's voxels: either one repeated type, or an explicit run stream.
///
/// Externally tagged with *newtype* variants, so it reads `{"uniform": 1}` or
/// `{"runs": [...]}`. Struct variants would double the key to
/// `{"runs":{"runs":[...]}}`, and `#[serde(flatten)]` on the payload is banned
/// because it buffers through serde's `Content`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields, rename_all = "snake_case")]
pub enum ChunkPayloadDocument {
    /// Every voxel in the chunk is this palette index; no run stream stored.
    Uniform(u16),
    /// Chunk-local Z-major runs, summing to the chunk volume, canonical.
    Runs(Vec<VoxelRunDocument>),
}

/// One populated chunk: where it is, and what is in it.
///
/// A chunk absent from the document is entirely air, which is what keeps a
/// mostly-empty wilderness extent affordable.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ChunkEntryDocument {
    /// Chunk coordinate, counted in chunks rather than in voxels.
    pub at: ChunkCoordDocument,
    /// The chunk's contents.
    pub payload: ChunkPayloadDocument,
}

impl ChunkPayloadDocument {
    /// How many runs the payload declares. A uniform chunk declares none.
    ///
    /// Used by the header phase to bound a chunk before anything decodes it.
    #[must_use]
    pub const fn run_count(&self) -> usize {
        match self {
            Self::Uniform(_) => 0,
            Self::Runs(runs) => runs.len(),
        }
    }
}
