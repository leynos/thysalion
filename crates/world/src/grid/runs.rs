//! The chunk-local run codec.
//!
//! Runs are explicit `(length, index)` structure with no in-band escape
//! values. Qubicle Binary encodes runs with escapes built from reserved colour
//! values, which collide with real data and produce exactly the corruption
//! class that cannot be diagnosed afterwards; explicitness costs bytes and
//! buys diagnosability.
//!
//! Canonical form has three rules, and encoding always produces it while
//! decoding never assumes it: no zero-length run, no two adjacent runs sharing
//! an index, and lengths summing to the chunk volume. Validation rejects a
//! non-canonical stream rather than normalizing it, so a generator bug is
//! visible rather than absorbed.

use crate::{grid::voxel_index::VoxelIndex, scene::document::VoxelRunDocument};

/// Why a run stream could not be decoded into a chunk.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
#[non_exhaustive]
pub enum RunDecodeError {
    /// A run declared a length of zero.
    #[error("run {ordinal} has zero length")]
    ZeroLength {
        /// Position of the run in the stream.
        ordinal: usize,
    },
    /// Two adjacent runs share an index, so the stream is not canonical.
    #[error("runs {ordinal} and {} share index {index}", ordinal + 1)]
    AdjacentDuplicate {
        /// Position of the first of the two runs.
        ordinal: usize,
        /// The shared palette index.
        index: u16,
    },
    /// The decoded chunk will not fit this machine's address space, or the
    /// allocator refused it.
    ///
    /// Distinct from `LengthMismatch`: the stream is internally consistent and
    /// agrees with the declared volume. It is this target that cannot hold the
    /// result — on a 32-bit build, two canonical `u32::MAX` runs describe a
    /// volume larger than `usize::MAX`. Reported rather than aborted, because
    /// `Vec::with_capacity` on an impossible size ends the process instead of
    /// the load.
    #[error("a chunk of {volume} voxels cannot be allocated on this target")]
    Unallocatable {
        /// The volume that could not be held.
        volume: u64,
    },
    /// The run lengths do not sum to the chunk volume.
    #[error("run lengths sum to {actual}, but the chunk holds {expected} voxels")]
    LengthMismatch {
        /// What the lengths summed to.
        actual: u64,
        /// What the chunk volume requires.
        expected: u64,
    },
}

/// Expands a canonical run stream into a dense chunk of `volume` voxels.
///
/// # Errors
///
/// Returns [`RunDecodeError`] when the stream is not canonical or does not
/// cover the chunk exactly. The length check runs *before* any allocation
/// proportional to the declared lengths, so a stream claiming a vast volume is
/// refused rather than exhausting memory.
pub fn expand(runs: &[VoxelRunDocument], volume: u64) -> Result<Vec<VoxelIndex>, RunDecodeError> {
    check_canonical(runs)?;
    let total = total_length(runs);
    if total != volume {
        return Err(RunDecodeError::LengthMismatch {
            actual: total,
            expected: volume,
        });
    }
    // Checked, then reserved fallibly. `usize::try_from(...).unwrap_or(MAX)`
    // followed by `with_capacity` turns an impossible volume into an abort
    // rather than a diagnostic, which is the one failure mode a loader must
    // never have: the caller still owns a valid previous scene.
    let capacity = usize::try_from(volume).map_err(|_| RunDecodeError::Unallocatable { volume })?;
    let mut out = Vec::new();
    out.try_reserve_exact(capacity)
        .map_err(|_| RunDecodeError::Unallocatable { volume })?;
    for run in runs {
        let index = VoxelIndex::new(run.index);
        out.extend(core::iter::repeat_n(index, run.length as usize));
    }
    Ok(out)
}

/// Sums run lengths on `u64`, so a stream of `u32::MAX` lengths cannot wrap.
fn total_length(runs: &[VoxelRunDocument]) -> u64 {
    runs.iter().map(|run| u64::from(run.length)).sum()
}

/// Rejects zero-length runs and adjacent runs sharing an index.
///
/// Separate from [`expand`] so the caller may check a stream without paying
/// for its expansion, which is what the header validation phase does.
fn check_canonical(runs: &[VoxelRunDocument]) -> Result<(), RunDecodeError> {
    for (ordinal, run) in runs.iter().enumerate() {
        if run.length == 0 {
            return Err(RunDecodeError::ZeroLength { ordinal });
        }
    }
    for (ordinal, pair) in runs.windows(2).enumerate() {
        let [first, second] = pair else { continue };
        if first.index == second.index {
            return Err(RunDecodeError::AdjacentDuplicate {
                ordinal,
                index: first.index,
            });
        }
    }
    Ok(())
}

/// Collapses a dense chunk into a canonical run stream.
///
/// Maximal runs, no zero-length run, no adjacent duplicates — the form
/// [`expand`] accepts, and the form the content hash is taken over.
#[must_use]
pub fn collapse(voxels: &[VoxelIndex]) -> Vec<VoxelRunDocument> {
    let mut runs: Vec<VoxelRunDocument> = Vec::new();
    for &voxel in voxels {
        match runs.last_mut() {
            Some(run) if run.index == voxel.get() => run.length += 1,
            _ => runs.push(VoxelRunDocument {
                length: 1,
                index: voxel.get(),
            }),
        }
    }
    runs
}

/// The single index every voxel shares, when they all share one.
///
/// A dense chunk that happens to hold one value must encode as a uniform
/// payload, or two documents describing the same world would hash differently.
#[must_use]
pub fn uniform_index(voxels: &[VoxelIndex]) -> Option<VoxelIndex> {
    let first = *voxels.first()?;
    voxels.iter().all(|&voxel| voxel == first).then_some(first)
}
