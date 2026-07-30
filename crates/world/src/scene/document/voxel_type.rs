//! Wire types for one palette entry: everything the engine knows about a
//! voxel kind (design §7.2).
//!
//! These are *document* types. They derive `Serialize`, `Deserialize`, and
//! `JsonSchema` and nothing else, they carry `deny_unknown_fields`, and they
//! never carry `#[non_exhaustive]` — the domain types they validate into do
//! that instead. A derived `Deserialize` on a domain type would be a public
//! constructor that bypasses validation, which is why the two are separate.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use smol_str::SmolStr;

/// Which texture set and physically-based rendering parameters a voxel uses.
///
/// The vocabulary follows the material families in the reference style guide.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields, rename_all = "snake_case")]
pub enum MaterialClass {
    /// Empty space. Palette index zero is always this class.
    Air,
    /// Masonry, rock, and cobble.
    Stone,
    /// Planks, beams, and frames.
    Timber,
    /// Tiles, thatch, and shingles.
    Roofing,
    /// Banners, awnings, and fabric.
    Cloth,
    /// Soil, mud, grass, and paths.
    Ground,
    /// Foliage, reeds, and fungus.
    Natural,
    /// Standing or flowing water.
    Water,
}

/// The direction in which a ramp or stair rises.
///
/// A closed set of four compass directions plus flat, rather than design
/// §7.2's `IVec2`: a vector admits meaningless values such as `(7, -3)` that
/// validation would then have to reject.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields, rename_all = "snake_case")]
pub enum SlopeDirection {
    /// No rise; the voxel is a full cube or a flat slab.
    Flat,
    /// Rises towards increasing `x`.
    PosX,
    /// Rises towards decreasing `x`.
    NegX,
    /// Rises towards increasing `y`.
    PosY,
    /// Rises towards decreasing `y`.
    NegY,
}

/// One of the six axis-aligned faces of a voxel.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields, rename_all = "snake_case")]
pub enum Face {
    /// The face whose normal points towards increasing `x`.
    PosX,
    /// The face whose normal points towards decreasing `x`.
    NegX,
    /// The face whose normal points towards increasing `y`.
    PosY,
    /// The face whose normal points towards decreasing `y`.
    NegY,
    /// The face whose normal points towards increasing `z`.
    PosZ,
    /// The face whose normal points towards decreasing `z`.
    NegZ,
}

/// Per-face passability for pathfinding.
///
/// Six named fields rather than design §7.2's `[bool; 6]`. An array requires
/// every reader to agree on the index-to-face mapping and offers no way to
/// notice when one does not; lille's equivalent used a dictionary keyed by
/// unit-normal strings with no completeness guarantee, and was never
/// implemented.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
#[expect(
    clippy::struct_excessive_bools,
    reason = "a voxel has exactly six faces, each independently passable. The lint's remedy is to \
              collapse booleans into a state enum, which does not apply here, and both \
              alternative encodings were considered and rejected: an array loses the face names, \
              and a bitmask loses the legibility of a hand-authored `pos_x: true`."
)]
pub struct Passability {
    /// Whether an agent may cross the `+x` face.
    pub pos_x: bool,
    /// Whether an agent may cross the `-x` face.
    pub neg_x: bool,
    /// Whether an agent may cross the `+y` face.
    pub pos_y: bool,
    /// Whether an agent may cross the `-y` face.
    pub neg_y: bool,
    /// Whether an agent may cross the `+z` face.
    pub pos_z: bool,
    /// Whether an agent may cross the `-z` face.
    pub neg_z: bool,
}

impl Passability {
    /// Every face passable, as air is.
    #[must_use]
    pub const fn open() -> Self {
        Self {
            pos_x: true,
            neg_x: true,
            pos_y: true,
            neg_y: true,
            pos_z: true,
            neg_z: true,
        }
    }

    /// No face passable, as a solid block is.
    #[must_use]
    pub const fn closed() -> Self {
        Self {
            pos_x: false,
            neg_x: false,
            pos_y: false,
            neg_y: false,
            pos_z: false,
            neg_z: false,
        }
    }

    /// Whether any face admits passage.
    #[must_use]
    pub const fn is_any_passable(&self) -> bool {
        self.pos_x || self.neg_x || self.pos_y || self.neg_y || self.pos_z || self.neg_z
    }
}

/// Light emitted by a voxel kind.
///
/// `intensity` is on design §9.2's 0–15 scale; validation rejects anything
/// above 15 rather than clamping, so an authoring mistake is visible.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct EmissionDocument {
    /// Emitted light level, 0 to 15 inclusive. Zero means inert.
    pub intensity: u8,
    /// Emitted colour as 8-bit red, green, and blue channels.
    pub colour: [u8; 3],
}

impl EmissionDocument {
    /// A voxel that emits nothing.
    #[must_use]
    pub const fn dark() -> Self {
        Self {
            intensity: 0,
            colour: [0, 0, 0],
        }
    }
}

/// Material-field coefficients for a voxel kind (design §10.5).
///
/// All three are Q8.8 fixed point: the stored integer divided by 256. Design
/// §10.5 mandates a 16-bit fixed-point representation for the runtime fields
/// but states no scale, so this step fixes one; ADR 006 records it and the
/// design document is amended to match. `ignition_point` of `u16::MAX` means
/// the material does not ignite.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct SimProperties {
    /// How much the material can burn, Q8.8.
    pub fuel: u16,
    /// Temperature at which the material ignites, Q8.8; `u16::MAX` never.
    pub ignition_point: u16,
    /// How much moisture the material can hold, Q8.8.
    pub moisture_capacity: u16,
}

impl SimProperties {
    /// A material that neither burns nor holds moisture.
    #[must_use]
    pub const fn inert() -> Self {
        Self {
            fuel: 0,
            ignition_point: u16::MAX,
            moisture_capacity: 0,
        }
    }
}

/// One palette entry: the unit of material meaning (design §7.2).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct VoxelTypeDocument {
    /// Unique name within the scene's palette.
    pub name: SmolStr,
    /// Material class, selecting the texture set and shading parameters.
    pub material: MaterialClass,
    /// Per-face passability for pathfinding.
    pub passable: Passability,
    /// Direction of rise for ramps and stairs.
    pub slope: SlopeDirection,
    /// Emitted light.
    pub emission: EmissionDocument,
    /// Material-field coefficients.
    pub sim: SimProperties,
    /// The ontology concept this voxel kind instantiates, when it has one.
    ///
    /// Checked for syntax and project-namespace membership only. Resolving it
    /// against the ontology is the knowledge plane's work at roadmap step 5.1;
    /// the dependency edge runs `knowledge -> world`, never the reverse.
    pub concept: Option<SmolStr>,
}
