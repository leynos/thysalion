//! Voxel and chunk coordinates, and the mapping between a chunk-local
//! position and its index in that chunk's run stream.
//!
//! The mapping is Z-major: within a chunk of side `s`, chunk-local
//! `(x, y, z)` sits at `z * s * s + y * s + x`. It is stated here, once, and
//! in the format reference. Every voxel format that left this implicit
//! produced incompatible third-party readers.

use super::extent::ChunkSize;

/// A position in the scene, in voxels.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct VoxelPos {
    /// Position along the `x` axis, in voxels.
    pub x: u32,
    /// Position along the `y` axis, in voxels.
    pub y: u32,
    /// Position along the `z` axis, in voxels.
    pub z: u32,
}

impl VoxelPos {
    /// Creates a position.
    #[must_use]
    pub const fn new(x: u32, y: u32, z: u32) -> Self { Self { x, y, z } }

    /// The chunk containing this position.
    #[must_use]
    pub const fn chunk(self, chunk_size: ChunkSize) -> ChunkCoord {
        let size = chunk_size.get();
        ChunkCoord {
            x: self.x.div_euclid(size),
            y: self.y.div_euclid(size),
            z: self.z.div_euclid(size),
        }
    }

    /// This position relative to the origin of its own chunk.
    #[must_use]
    pub const fn within_chunk(self, chunk_size: ChunkSize) -> Self {
        let size = chunk_size.get();
        Self {
            x: self.x.rem_euclid(size),
            y: self.y.rem_euclid(size),
            z: self.z.rem_euclid(size),
        }
    }

    /// The index of this chunk-local position in its chunk's Z-major stream.
    ///
    /// The caller is responsible for `self` already being chunk-local; a
    /// position outwith `0..chunk_size` on any axis yields an index outwith
    /// the chunk, which callers detect by bounds-checking the result against
    /// [`ChunkSize::volume`].
    #[must_use]
    pub const fn local_index(self, chunk_size: ChunkSize) -> u64 {
        // Saturating, because this is documented to accept a position outwith
        // the chunk and to report that by returning an index the caller's
        // bounds check rejects. An arbitrary `u32` position against a large
        // chunk size overflows the products; saturating keeps the result
        // outwith every chunk, which is exactly what the contract promises,
        // where wrapping could land back inside one.
        let size = chunk_size.get() as u64;
        let plane = size.saturating_mul(size);
        (self.z as u64)
            .saturating_mul(plane)
            .saturating_add((self.y as u64).saturating_mul(size))
            .saturating_add(self.x as u64)
    }

    /// The chunk-local position at `index` in a chunk's Z-major stream.
    ///
    /// The inverse of [`VoxelPos::local_index`]. Returns `None` when `index`
    /// lies outwith the chunk, or when an axis will not fit a `u32` — which
    /// cannot happen for a chunk size that fits one, but is checked rather
    /// than assumed because this crate takes no cast allowances.
    #[must_use]
    pub fn from_local_index(index: u64, chunk_size: ChunkSize) -> Option<Self> {
        if index >= chunk_size.volume() {
            return None;
        }
        let size = u64::from(chunk_size.get());
        let x = u32::try_from(index.rem_euclid(size)).ok()?;
        let y = u32::try_from(index.div_euclid(size).rem_euclid(size)).ok()?;
        let z = u32::try_from(index.div_euclid(size * size)).ok()?;
        Some(Self { x, y, z })
    }
}

/// A chunk's position, counted in chunks rather than in voxels.
///
/// Ordered, and that order is the document's chunk-entry order. Sorting is
/// what makes re-encoding byte-stable, which design §12.3's content hashes
/// depend on.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ChunkCoord {
    /// Chunk index along the `z` axis. First, so ordering is Z-major and
    /// matches the order a reader walks layers in.
    pub z: u32,
    /// Chunk index along the `y` axis.
    pub y: u32,
    /// Chunk index along the `x` axis.
    pub x: u32,
}

impl ChunkCoord {
    /// Creates a chunk coordinate from `x`, `y`, and `z` in that reading
    /// order, whatever the field order the type sorts by.
    #[must_use]
    pub const fn new(x: u32, y: u32, z: u32) -> Self { Self { z, y, x } }

    /// The scene position of this chunk's origin corner.
    #[must_use]
    pub const fn origin(self, chunk_size: ChunkSize) -> VoxelPos {
        // Saturating for the same reason `VoxelPos::local_index` is: a chunk
        // coordinate is a public `u32` triple, and a coordinate past the grid
        // must yield a position past the grid rather than panic in debug and
        // wrap into a valid-looking one in release.
        let size = chunk_size.get();
        VoxelPos {
            x: self.x.saturating_mul(size),
            y: self.y.saturating_mul(size),
            z: self.z.saturating_mul(size),
        }
    }
}
