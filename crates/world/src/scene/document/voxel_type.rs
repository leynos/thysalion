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

/// Whether an agent may cross one named voxel face.
///
/// Serde and JSON Schema represent this domain marker as its underlying
/// boolean, so each [`Passability`] field remains a flat boolean on the wire.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
// Keep the documented wrapper's schema inline at each named face property.
#[schemars(inline)]
#[serde(transparent)]
pub struct FacePassability(bool);

impl FacePassability {
    /// Creates a face passability value from its wire boolean.
    ///
    /// # Examples
    ///
    /// ```
    /// use thysalion_world::scene::document::FacePassability;
    ///
    /// let face = FacePassability::new(true);
    /// assert!(face.is_passable());
    /// ```
    #[must_use]
    pub const fn new(is_passable: bool) -> Self { Self(is_passable) }

    /// Whether an agent may cross this face.
    ///
    /// # Examples
    ///
    /// ```
    /// use thysalion_world::scene::document::FacePassability;
    ///
    /// let face = FacePassability::new(false);
    /// assert!(!face.is_passable());
    /// ```
    #[must_use]
    pub const fn is_passable(self) -> bool { self.0 }
}

/// Converts a wire boolean into its face-specific domain value.
///
/// # Examples
///
/// ```
/// use thysalion_world::scene::document::FacePassability;
///
/// let face = FacePassability::from(true);
/// assert!(face.is_passable());
/// ```
impl From<bool> for FacePassability {
    fn from(is_passable: bool) -> Self { Self::new(is_passable) }
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
pub struct Passability {
    /// Whether an agent may cross the `+x` face.
    pub pos_x: FacePassability,
    /// Whether an agent may cross the `-x` face.
    pub neg_x: FacePassability,
    /// Whether an agent may cross the `+y` face.
    pub pos_y: FacePassability,
    /// Whether an agent may cross the `-y` face.
    pub neg_y: FacePassability,
    /// Whether an agent may cross the `+z` face.
    pub pos_z: FacePassability,
    /// Whether an agent may cross the `-z` face.
    pub neg_z: FacePassability,
}

impl Passability {
    /// Creates a value with the same passability on every face.
    ///
    /// # Examples
    ///
    /// ```
    /// use thysalion_world::scene::document::Passability;
    ///
    /// let solid = Passability::closed();
    /// assert!(!solid.pos_x.is_passable());
    /// assert!(!solid.neg_z.is_passable());
    /// ```
    #[must_use]
    const fn uniform(is_passable: bool) -> Self {
        let face = FacePassability::new(is_passable);
        Self {
            pos_x: face,
            neg_x: face,
            pos_y: face,
            neg_y: face,
            pos_z: face,
            neg_z: face,
        }
    }

    /// Every face passable, as air is.
    #[must_use]
    pub const fn open() -> Self { Self::uniform(true) }

    /// No face passable, as a solid block is.
    #[must_use]
    pub const fn closed() -> Self { Self::uniform(false) }

    /// Whether any face admits passage.
    #[must_use]
    pub const fn is_any_passable(&self) -> bool {
        self.pos_x.is_passable()
            || self.neg_x.is_passable()
            || self.pos_y.is_passable()
            || self.neg_y.is_passable()
            || self.pos_z.is_passable()
            || self.neg_z.is_passable()
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
