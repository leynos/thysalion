//! The runtime voxel grid: the state plane's primary data structure.
//!
//! This is a peer of [`crate::scene`], not part of it. Design §7.1 calls the
//! voxel grid "the state plane's data" and design §12.1 gives it its own
//! authority row; it is what survives after the document is dropped, and
//! phase 2's mesher and phase 4's pathfinder import it. Filing it under a
//! module named after the file format would announce the wrong thing.
//! `scene` may depend on `grid`; `grid` never depends on `scene` beyond the
//! wire types it converts to and from.
//!
//! Storage is sparse by chunk. An absent chunk is entirely air, and a chunk
//! holding one voxel type stores that type rather than 32,768 copies of it —
//! materializing a uniform chunk would undo the very elision the wire format
//! performs. The wilderness scene class is 256 MiB dense and roughly 4 MiB
//! this way.
//!
//! The grid is **read-only** by design. The loaded scene is the immutable
//! authored baseline, which is what design §12.3 content-hashes; runtime voxel
//! edits live in the state plane's overlay, which is what gets saved. ADR 006
//! records that division so phase 2 wires the overlay rather than inventing a
//! parallel grid.

mod coords;
mod extent;
pub mod runs;
mod voxel_index;

use std::collections::BTreeMap;

pub use coords::{ChunkCoord, VoxelPos};
pub use extent::{ChunkSize, DESIGN_CHUNK_SIZE, Extent, ExtentError};
pub use voxel_index::VoxelIndex;

use crate::scene::document::{ChunkCoordDocument, ChunkEntryDocument, ChunkPayloadDocument};

/// How one populated chunk is held.
#[derive(Debug, Clone, PartialEq, Eq)]
enum ChunkStore {
    /// Every voxel in the chunk is this index.
    Uniform(VoxelIndex),
    /// A dense chunk-local Z-major array of exactly `ChunkSize::volume`.
    Dense(Box<[VoxelIndex]>),
}

impl ChunkStore {
    /// The voxel at a chunk-local index, or `None` when out of range.
    fn get(&self, local: u64) -> Option<VoxelIndex> {
        match self {
            Self::Uniform(index) => Some(*index),
            Self::Dense(voxels) => {
                let offset = usize::try_from(local).ok()?;
                voxels.get(offset).copied()
            }
        }
    }

    /// Whether every voxel in the chunk is air, in which case the chunk is
    /// omitted from the document entirely.
    fn is_all_air(&self) -> bool {
        match self {
            Self::Uniform(index) => index.is_air(),
            Self::Dense(voxels) => voxels.iter().all(|voxel| voxel.is_air()),
        }
    }

    /// The number of voxels that are not air.
    fn non_air_count(&self, volume: u64) -> u64 {
        match self {
            Self::Uniform(index) => {
                if index.is_air() {
                    0
                } else {
                    volume
                }
            }
            Self::Dense(voxels) => voxels.iter().filter(|voxel| !voxel.is_air()).count() as u64,
        }
    }

    /// The canonical wire payload for this chunk.
    fn to_payload(&self) -> ChunkPayloadDocument {
        match self {
            Self::Uniform(index) => ChunkPayloadDocument::Uniform(index.get()),
            Self::Dense(voxels) => runs::uniform_index(voxels).map_or_else(
                || ChunkPayloadDocument::Runs(runs::collapse(voxels)),
                |index| ChunkPayloadDocument::Uniform(index.get()),
            ),
        }
    }
}

/// A scene's voxels, stored sparsely by chunk.
///
/// Backed by a `BTreeMap` so iteration order is deterministic, which is what
/// makes re-encoding byte-stable and therefore hashable (design §12.3).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VoxelGrid {
    extent: Extent,
    chunk_size: ChunkSize,
    chunks: BTreeMap<ChunkCoord, ChunkStore>,
}

impl VoxelGrid {
    /// An entirely empty grid of the given bounds.
    #[must_use]
    pub const fn empty(extent: Extent, chunk_size: ChunkSize) -> Self {
        Self {
            extent,
            chunk_size,
            chunks: BTreeMap::new(),
        }
    }

    /// The scene's bounds, in voxels.
    #[must_use]
    pub const fn extent(&self) -> Extent { self.extent }

    /// The chunk edge length, in voxels.
    #[must_use]
    pub const fn chunk_size(&self) -> ChunkSize { self.chunk_size }

    /// The number of chunks actually stored.
    #[must_use]
    pub fn populated_chunks(&self) -> usize { self.chunks.len() }

    /// Whether `pos` lies inside the declared extent.
    #[must_use]
    pub const fn contains(&self, pos: VoxelPos) -> bool {
        pos.x < self.extent.x() && pos.y < self.extent.y() && pos.z < self.extent.z()
    }

    /// Returns the voxel at `pos`, or `None` when `pos` lies outwith the scene
    /// extent.
    ///
    /// A position inside the extent but in an unpopulated chunk yields
    /// [`VoxelIndex::AIR`]: `None` means "not part of this scene", never
    /// "empty here". Conflating the two is how callers end up treating the
    /// outside of the world as air and walking off the edge of it.
    #[must_use]
    pub fn get(&self, pos: VoxelPos) -> Option<VoxelIndex> {
        if !self.contains(pos) {
            return None;
        }
        let Some(store) = self.chunks.get(&pos.chunk(self.chunk_size)) else {
            return Some(VoxelIndex::AIR);
        };
        let local = pos
            .within_chunk(self.chunk_size)
            .local_index(self.chunk_size);
        store.get(local).or(Some(VoxelIndex::AIR))
    }

    /// The number of voxels that are not air.
    #[must_use]
    pub fn non_air_count(&self) -> u64 {
        let volume = self.chunk_size.volume();
        self.chunks
            .values()
            .map(|store| store.non_air_count(volume))
            .sum()
    }

    /// Inserts a uniform chunk, replacing whatever was there.
    pub fn insert_uniform(&mut self, at: ChunkCoord, index: VoxelIndex) {
        self.chunks.insert(at, ChunkStore::Uniform(index));
    }

    /// Inserts a dense chunk, replacing whatever was there.
    ///
    /// # Errors
    ///
    /// Returns the supplied vector unchanged when its length is not exactly
    /// one chunk volume, so a caller cannot install a short chunk that would
    /// read as air past its end.
    pub fn insert_dense(
        &mut self,
        at: ChunkCoord,
        voxels: Vec<VoxelIndex>,
    ) -> Result<(), Vec<VoxelIndex>> {
        if u64::try_from(voxels.len()) != Ok(self.chunk_size.volume()) {
            return Err(voxels);
        }
        self.chunks
            .insert(at, ChunkStore::Dense(voxels.into_boxed_slice()));
        Ok(())
    }

    /// Rebuilds the document's chunk entries in canonical form.
    ///
    /// Sorted by chunk coordinate, all-air chunks omitted, single-valued
    /// chunks elided to a uniform payload, and runs maximal. Costs time
    /// proportional to populated chunks, never to declared extent.
    #[must_use]
    pub fn to_chunks(&self) -> Vec<ChunkEntryDocument> {
        self.chunks
            .iter()
            .filter(|(_, store)| !store.is_all_air())
            .map(|(at, store)| ChunkEntryDocument {
                at: ChunkCoordDocument {
                    x: at.x,
                    y: at.y,
                    z: at.z,
                },
                payload: store.to_payload(),
            })
            .collect()
    }
}
