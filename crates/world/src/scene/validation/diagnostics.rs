//! The diagnostic vocabulary: one named problem per corruption class.
//!
//! Two properties matter more than the wording.
//!
//! Each variant carries a stable [`DiagnosticCode`]. Tests assert on the code
//! rather than on the message, so rewording a message is not a contract
//! change — and the `insta` snapshots that pin the *rendered* report exist to
//! catch a change in wording, ordering, or location, deliberately.
//!
//! Each variant carries a [`DocumentLocation`], and the reported list is
//! sorted by it. Without an explicit order, running one function per check
//! yields rule-registration order, the snapshots pin that accident, and adding
//! a rule churns unrelated snapshots.

use camino::Utf8PathBuf;
use smol_str::SmolStr;

use crate::{
    grid::{ChunkCoord, VoxelPos},
    scene::concept::ConceptIriProblem,
};

/// A stable, machine-readable name for a corruption class.
///
/// Serialized by `scene-check --json` and asserted on by tests. Renaming a
/// variant is a contract change; adding one is not.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[non_exhaustive]
pub enum DiagnosticCode {
    /// The document declares a version this build cannot read.
    UnsupportedVersion,
    /// An axis of the declared extent is zero.
    ZeroDimension,
    /// The declared extent's volume overflows.
    DimensionsOverflow,
    /// An axis is not a whole number of chunks.
    DimensionsUnaligned,
    /// The chunk size is not the one design §7.1 fixes.
    ChunkSizeNotDesign,
    /// The palette has no entries.
    PaletteEmpty,
    /// The palette holds more entries than a 16-bit index can address.
    PaletteTooLarge,
    /// Palette entry zero is not the air material class.
    PaletteZeroNotAir,
    /// Two palette entries share a name.
    DuplicateVoxelTypeName,
    /// Light emission is above the 0–15 scale design §9.2 fixes.
    EmissionOutOfRange,
    /// A concept identifier is malformed or outwith the project namespaces.
    ConceptIriInvalid,
    /// The document holds more chunk entries than the bound permits.
    TooManyChunks,
    /// A chunk coordinate lies outwith the declared extent.
    ChunkOutwithExtent,
    /// Two chunk entries share a coordinate.
    DuplicateChunk,
    /// Chunk entries are not sorted by coordinate.
    ChunksOutOfOrder,
    /// A chunk holds more runs than the bound permits.
    TooManyRuns,
    /// A voxel run names a palette index the palette cannot resolve.
    UnknownPaletteIndex,
    /// A run declares a length of zero.
    ZeroLengthRun,
    /// Two adjacent runs share an index, so the stream is not canonical.
    AdjacentDuplicateRuns,
    /// A chunk's run lengths do not sum to the chunk volume.
    RunLengthMismatch,
    /// A spawn stands outwith the declared extent.
    SpawnOutwithGrid,
    /// Two spawns share a name.
    DuplicateSpawnName,
    /// A spawn or prototype extends a prototype that does not exist.
    UnknownPrototype,
    /// A prototype chain forms a cycle.
    PrototypeCycle,
    /// A prototype chain is deeper than the bound permits.
    PrototypeTooDeep,
    /// The scene's named-graph identifier is missing or malformed.
    SceneGraphIriInvalid,
    /// A knowledge resource is named but absent.
    KnowledgeResourceAbsent,
    /// A knowledge resource is present but could not be read.
    KnowledgeResourceUnreadable,
    /// A knowledge resource path is absolute or escapes the scene directory.
    KnowledgeResourceUnsafePath,
    /// A spawn stands inside a voxel no face admits passage through.
    SpawnObstructed,
    /// A spawn has no supporting voxel beneath it and is not marked airborne.
    SpawnUnsupported,
}

impl DiagnosticCode {
    /// The dotted code as it appears in a report and in `--json` output.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::UnsupportedVersion => "scene.version.unsupported",
            Self::ZeroDimension => "scene.dimensions.zero",
            Self::DimensionsOverflow => "scene.dimensions.overflow",
            Self::DimensionsUnaligned => "scene.dimensions.unaligned",
            Self::ChunkSizeNotDesign => "scene.chunk-size.not-design",
            Self::PaletteEmpty => "scene.palette.empty",
            Self::PaletteTooLarge => "scene.palette.too-large",
            Self::PaletteZeroNotAir => "scene.palette.zero-not-air",
            Self::DuplicateVoxelTypeName => "scene.palette.duplicate-name",
            Self::EmissionOutOfRange => "scene.palette.emission-out-of-range",
            Self::ConceptIriInvalid => "scene.concept.invalid",
            Self::TooManyChunks => "scene.voxels.too-many-chunks",
            Self::ChunkOutwithExtent => "scene.voxels.chunk-outwith-extent",
            Self::DuplicateChunk => "scene.voxels.duplicate-chunk",
            Self::ChunksOutOfOrder => "scene.voxels.chunks-out-of-order",
            Self::TooManyRuns => "scene.voxels.too-many-runs",
            Self::UnknownPaletteIndex => "scene.voxels.unknown-palette-index",
            Self::ZeroLengthRun => "scene.voxels.zero-length-run",
            Self::AdjacentDuplicateRuns => "scene.voxels.adjacent-duplicate-runs",
            Self::RunLengthMismatch => "scene.voxels.run-length-mismatch",
            Self::SpawnOutwithGrid => "scene.entities.spawn-outwith-grid",
            Self::DuplicateSpawnName => "scene.entities.duplicate-spawn-name",
            Self::UnknownPrototype => "scene.entities.unknown-prototype",
            Self::PrototypeCycle => "scene.entities.prototype-cycle",
            Self::PrototypeTooDeep => "scene.entities.prototype-too-deep",
            Self::SceneGraphIriInvalid => "scene.knowledge.graph-invalid",
            Self::KnowledgeResourceAbsent => "scene.knowledge.resource-absent",
            Self::KnowledgeResourceUnreadable => "scene.knowledge.resource-unreadable",
            Self::KnowledgeResourceUnsafePath => "scene.knowledge.resource-unsafe-path",
            Self::SpawnObstructed => "scene.entities.spawn-obstructed",
            Self::SpawnUnsupported => "scene.entities.spawn-unsupported",
        }
    }

    /// Whether this class is advisory rather than fatal.
    ///
    /// A warning describes a scene that loads but may not be what its author
    /// meant. `scene-check --strict` promotes warnings to failures, and
    /// continuous integration runs strict.
    #[must_use]
    pub const fn is_warning(self) -> bool {
        matches!(self, Self::SpawnObstructed | Self::SpawnUnsupported)
    }
}

impl core::fmt::Display for DiagnosticCode {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.write_str(self.as_str())
    }
}

/// The document sections, in the order the document declares them.
///
/// The ordinal *is* the reporting order, so a reader walks a report in the
/// order they would walk the file.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[non_exhaustive]
pub enum DocumentSection {
    /// The `version` field.
    Version,
    /// The `dimensions` and `chunk_size` fields.
    Dimensions,
    /// The `palette` array.
    Palette,
    /// The `voxels` array.
    Voxels,
    /// The `entities` section.
    Entities,
    /// The `lighting` section.
    Lighting,
    /// The `knowledge` section.
    Knowledge,
}

impl DocumentSection {
    /// The section's name as it appears in the document.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Version => "version",
            Self::Dimensions => "dimensions",
            Self::Palette => "palette",
            Self::Voxels => "voxels",
            Self::Entities => "entities",
            Self::Lighting => "lighting",
            Self::Knowledge => "knowledge",
        }
    }
}

/// Where in the document a diagnostic was found.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct DocumentLocation {
    /// The document section, ordered as the document declares its fields.
    pub section: DocumentSection,
    /// Position within the section — a palette entry, a chunk entry, a spawn.
    pub index: u32,
}

impl DocumentLocation {
    /// A location in `section` at `index`.
    #[must_use]
    pub const fn new(section: DocumentSection, index: u32) -> Self { Self { section, index } }

    /// A location naming a whole section.
    #[must_use]
    pub const fn section(section: DocumentSection) -> Self { Self::new(section, 0) }
}

impl core::fmt::Display for DocumentLocation {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "{}[{}]", self.section.as_str(), self.index)
    }
}

/// Where in the *world* a diagnostic was found.
///
/// Positional diagnostics carry this because a run ordinal is useless in a
/// 134-million-voxel scene: the reader needs a place to look, not an index
/// into a stream no human wrote. The generator's provenance sidecar carries it
/// the last step, to the authoring layer file and line.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct VoxelSite {
    /// Which chunk.
    pub chunk: ChunkCoord,
    /// Where within that chunk.
    pub local: VoxelPos,
}

impl core::fmt::Display for VoxelSite {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(
            f,
            "chunk ({}, {}, {}) at local ({}, {}, {})",
            self.chunk.x, self.chunk.y, self.chunk.z, self.local.x, self.local.y, self.local.z
        )
    }
}

/// One named problem found while validating a scene document.
///
/// `#[non_exhaustive]` sits on the enum and never on a variant: the
/// integration tests are a separate crate and could not otherwise construct
/// the variants they assert on.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[non_exhaustive]
pub enum SceneDiagnostic {
    /// A structural fault with no richer payload than its location.
    #[error("{code}: {detail}")]
    Structural {
        /// The corruption class.
        code: DiagnosticCode,
        /// Where in the document.
        at: DocumentLocation,
        /// A human-readable specifics line.
        detail: SmolStr,
    },
    /// A fault at a known place in the world.
    #[error("{code}: {site}: {detail}")]
    Positioned {
        /// The corruption class.
        code: DiagnosticCode,
        /// Where in the document.
        at: DocumentLocation,
        /// Where in the world.
        site: VoxelSite,
        /// A human-readable specifics line.
        detail: SmolStr,
    },
    /// A knowledge resource could not be resolved.
    #[error("{code}: {path}: {detail}")]
    Resource {
        /// The corruption class.
        code: DiagnosticCode,
        /// Where in the document.
        at: DocumentLocation,
        /// The resource path as the document wrote it.
        path: Utf8PathBuf,
        /// A human-readable specifics line.
        detail: SmolStr,
    },
}

impl SceneDiagnostic {
    /// A structural diagnostic.
    #[must_use]
    pub fn structural(
        code: DiagnosticCode,
        at: DocumentLocation,
        detail: impl Into<SmolStr>,
    ) -> Self {
        Self::Structural {
            code,
            at,
            detail: detail.into(),
        }
    }

    /// A diagnostic at a known place in the world.
    #[must_use]
    pub fn positioned(
        code: DiagnosticCode,
        at: DocumentLocation,
        site: VoxelSite,
        detail: impl Into<SmolStr>,
    ) -> Self {
        Self::Positioned {
            code,
            at,
            site,
            detail: detail.into(),
        }
    }

    /// A diagnostic about a named resource.
    #[must_use]
    pub fn resource(
        code: DiagnosticCode,
        at: DocumentLocation,
        path: Utf8PathBuf,
        detail: impl Into<SmolStr>,
    ) -> Self {
        Self::Resource {
            code,
            at,
            path,
            detail: detail.into(),
        }
    }

    /// The corruption class.
    #[must_use]
    pub const fn code(&self) -> DiagnosticCode {
        match self {
            Self::Structural { code, .. }
            | Self::Positioned { code, .. }
            | Self::Resource { code, .. } => *code,
        }
    }

    /// Where in the document the problem is.
    #[must_use]
    pub const fn location(&self) -> DocumentLocation {
        match self {
            Self::Structural { at, .. }
            | Self::Positioned { at, .. }
            | Self::Resource { at, .. } => *at,
        }
    }

    /// Where in the world the problem is, when that is known.
    #[must_use]
    pub const fn voxel_position(&self) -> Option<VoxelSite> {
        match self {
            Self::Positioned { site, .. } => Some(*site),
            Self::Structural { .. } | Self::Resource { .. } => None,
        }
    }

    /// The resource this diagnostic names, when it names one.
    #[must_use]
    pub fn resource_path(&self) -> Option<&camino::Utf8Path> {
        match self {
            Self::Resource { path, .. } => Some(path),
            Self::Structural { .. } | Self::Positioned { .. } => None,
        }
    }

    /// Whether this diagnostic is advisory rather than fatal.
    #[must_use]
    pub const fn is_warning(&self) -> bool { self.code().is_warning() }

    /// The specifics line, without the code or location prefix.
    #[must_use]
    pub fn detail(&self) -> &str {
        match self {
            Self::Structural { detail, .. }
            | Self::Positioned { detail, .. }
            | Self::Resource { detail, .. } => detail,
        }
    }

    /// The sort key that defines reporting order.
    #[must_use]
    pub const fn sort_key(&self) -> (DocumentLocation, DiagnosticCode) {
        (self.location(), self.code())
    }
}

/// Renders a concept-identifier problem as a diagnostic detail line.
#[must_use]
pub fn describe_concept_problem(raw: &str, problem: &ConceptIriProblem) -> SmolStr {
    SmolStr::new(format!("concept {raw:?} is invalid: {problem}"))
}
