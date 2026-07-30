//! Scene bounds and chunk sizing, with checked arithmetic throughout.
//!
//! Every product here is computed on `u64`. The wilderness scene class is
//! 1024 x 1024 x 128, which is 134 million voxels — already a third of
//! `u32`'s range — and a malformed document may declare far more. An
//! unchecked multiply would wrap and yield a plausible-looking small volume,
//! which is the worst possible failure for a bound that exists to refuse
//! oversized input.

use thiserror::Error;

/// The chunk edge length design §7.1 fixes for Thysalion scenes.
pub const DESIGN_CHUNK_SIZE: u32 = 32;

/// Why an extent or chunk size could not be accepted.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
#[non_exhaustive]
pub enum ExtentError {
    /// An axis was zero, so the scene has no volume.
    #[error("extent axis {axis} is zero")]
    ZeroAxis {
        /// The offending axis, as `x`, `y`, or `z`.
        axis: char,
    },
    /// The extent's volume overflows a `u64`.
    #[error("extent volume overflows")]
    VolumeOverflow,
    /// The chunk size was zero.
    #[error("chunk size is zero")]
    ZeroChunkSize,
    /// An axis is not a whole number of chunks.
    #[error("extent axis {axis} ({length}) is not a multiple of chunk size {chunk_size}")]
    Unaligned {
        /// The offending axis, as `x`, `y`, or `z`.
        axis: char,
        /// The axis length, in voxels.
        length: u32,
        /// The chunk edge length, in voxels.
        chunk_size: u32,
    },
}

/// A chunk's edge length, in voxels.
///
/// Non-zero by construction. Design §7.1 fixes the value at 32; the type
/// admits other sizes so the check that it *is* 32 can be a validation
/// diagnostic with its own message, rather than an unrepresentable state that
/// forces the parser to fail first with something less helpful.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ChunkSize(u32);

impl ChunkSize {
    /// The size design §7.1 fixes.
    pub const DESIGN: Self = Self(DESIGN_CHUNK_SIZE);

    /// Creates a chunk size.
    ///
    /// # Errors
    ///
    /// Returns [`ExtentError::ZeroChunkSize`] when `size` is zero.
    pub const fn new(size: u32) -> Result<Self, ExtentError> {
        if size == 0 {
            return Err(ExtentError::ZeroChunkSize);
        }
        Ok(Self(size))
    }

    /// The edge length, in voxels.
    #[must_use]
    pub const fn get(self) -> u32 { self.0 }

    /// The number of voxels in one chunk.
    #[must_use]
    pub const fn volume(self) -> u64 {
        let side = self.0 as u64;
        side * side * side
    }
}

/// A scene's bounds, in voxels.
///
/// Every axis is non-zero and a whole number of chunks; both are guaranteed by
/// construction rather than checked at each use.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Extent {
    x: u32,
    y: u32,
    z: u32,
}

impl Extent {
    /// Creates an extent, checking that it is non-zero and chunk-aligned.
    ///
    /// # Errors
    ///
    /// Returns [`ExtentError`] describing the first axis that fails.
    pub const fn new(x: u32, y: u32, z: u32, chunk_size: ChunkSize) -> Result<Self, ExtentError> {
        if let Err(error) = check_axis('x', x, chunk_size) {
            return Err(error);
        }
        if let Err(error) = check_axis('y', y, chunk_size) {
            return Err(error);
        }
        if let Err(error) = check_axis('z', z, chunk_size) {
            return Err(error);
        }
        Ok(Self { x, y, z })
    }

    /// Extent along the `x` axis, in voxels.
    #[must_use]
    pub const fn x(self) -> u32 { self.x }

    /// Extent along the `y` axis, in voxels.
    #[must_use]
    pub const fn y(self) -> u32 { self.y }

    /// Extent along the `z` axis, in voxels.
    #[must_use]
    pub const fn z(self) -> u32 { self.z }

    /// The number of voxels the extent declares.
    ///
    /// # Errors
    ///
    /// Returns [`ExtentError::VolumeOverflow`] when the product exceeds `u64`.
    pub const fn volume(self) -> Result<u64, ExtentError> {
        let Some(area) = (self.x as u64).checked_mul(self.y as u64) else {
            return Err(ExtentError::VolumeOverflow);
        };
        let Some(volume) = area.checked_mul(self.z as u64) else {
            return Err(ExtentError::VolumeOverflow);
        };
        Ok(volume)
    }

    /// The extent measured in chunks rather than voxels.
    #[must_use]
    pub const fn in_chunks(self, chunk_size: ChunkSize) -> (u32, u32, u32) {
        let size = chunk_size.get();
        (
            self.x.div_euclid(size),
            self.y.div_euclid(size),
            self.z.div_euclid(size),
        )
    }
}

/// Checks one axis for non-zero length and chunk alignment.
const fn check_axis(axis: char, length: u32, chunk_size: ChunkSize) -> Result<(), ExtentError> {
    if length == 0 {
        return Err(ExtentError::ZeroAxis { axis });
    }
    if !length.is_multiple_of(chunk_size.get()) {
        return Err(ExtentError::Unaligned {
            axis,
            length,
            chunk_size: chunk_size.get(),
        });
    }
    Ok(())
}
