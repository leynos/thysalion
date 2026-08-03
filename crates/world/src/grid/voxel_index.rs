//! The 16-bit palette index a voxel stores.
//!
//! This lives in `grid` rather than in `scene::palette` because the grid
//! *stores* an index while the palette *interprets* one, and `grid` may not
//! depend on `scene`. The type carries no palette identity on purpose: an
//! index read from a document is palette-relative and unvalidated, and
//! `Palette::get` is the only way to learn whether it resolves.

/// Index into a scene's palette.
///
/// Index zero is air in every scene. Reserving it gives run-length encoding
/// and sparse chunk storage a shared meaning for "nothing here", so an absent
/// chunk and a long air run agree without a lookup.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
pub struct VoxelIndex(u16);

impl VoxelIndex {
    /// The reserved air index present in every scene palette.
    pub const AIR: Self = Self(0);

    /// Creates an index from its wire representation.
    ///
    /// Infallible by design: any `u16` is a syntactically valid index, and
    /// whether it *resolves* is a question for a particular palette.
    #[must_use]
    pub const fn new(raw: u16) -> Self { Self(raw) }

    /// The wire representation.
    #[must_use]
    pub const fn get(self) -> u16 { self.0 }

    /// Whether this is the reserved air index.
    #[must_use]
    pub const fn is_air(self) -> bool { self.0 == Self::AIR.0 }
}

impl From<u16> for VoxelIndex {
    fn from(raw: u16) -> Self { Self::new(raw) }
}

impl From<VoxelIndex> for u16 {
    fn from(index: VoxelIndex) -> Self { index.get() }
}
