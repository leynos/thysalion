//! The validated palette and the voxel types in it.
//!
//! These are *domain* types. They derive no `serde` traits, and that is the
//! mechanism rather than a style choice: a derived `Deserialize` is a public
//! constructor reaching every private field, so it would let a caller
//! materialize a palette whose entry zero is not air and whose names collide —
//! two invariants this crate declares load-time-enforced.
//! `#[non_exhaustive]` does not close that hole either; it is documented as
//! not restricting deserialization.

use std::collections::BTreeSet;

use smol_str::SmolStr;

use crate::{
    grid::VoxelIndex,
    scene::{
        concept::{ConceptIri, NamespaceTable},
        document::{
            EmissionDocument,
            MaterialClass,
            Passability,
            SimProperties,
            SlopeDirection,
            VoxelTypeDocument,
        },
        validation::diagnostics::{
            DiagnosticCode,
            DocumentLocation,
            DocumentSection,
            SceneDiagnostic,
            describe_concept_problem,
        },
    },
};

/// The largest palette a 16-bit index can address.
pub const MAX_PALETTE_ENTRIES: usize = u16::MAX as usize + 1;

/// Light emitted by a voxel kind, on design §9.2's 0–15 scale.
///
/// Validated on construction, so a value of this type is evidence the range
/// check ran.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[non_exhaustive]
pub struct LightEmission {
    intensity: u8,
    colour: [u8; 3],
}

impl LightEmission {
    /// The largest emission the flood-fill field represents.
    pub const MAX_INTENSITY: u8 = 15;

    /// A voxel that emits nothing.
    #[must_use]
    pub const fn dark() -> Self {
        Self {
            intensity: 0,
            colour: [0, 0, 0],
        }
    }

    /// Checks `intensity` against the 0–15 scale.
    ///
    /// # Errors
    ///
    /// Returns the offending intensity when it is above the scale. Validation
    /// rejects rather than clamping, so an authoring mistake stays visible.
    ///
    /// # Examples
    ///
    /// ```
    /// use thysalion_world::scene::palette::LightEmission;
    ///
    /// let sconce = LightEmission::new(12, [255, 180, 90])?;
    /// assert_eq!(sconce.intensity(), 12);
    /// assert_eq!(sconce.colour(), [255, 180, 90]);
    /// assert!(sconce.is_emissive());
    ///
    /// // Nothing inert emits, whatever colour it is given.
    /// assert!(!LightEmission::dark().is_emissive());
    ///
    /// // Above the 0-15 scale the intensity is returned, not clamped: an
    /// // author who wrote 16 sees the value they wrote.
    /// assert_eq!(LightEmission::new(16, [0, 0, 0]), Err(16));
    /// # Ok::<(), u8>(())
    /// ```
    pub const fn new(intensity: u8, colour: [u8; 3]) -> Result<Self, u8> {
        if intensity > Self::MAX_INTENSITY {
            return Err(intensity);
        }
        Ok(Self { intensity, colour })
    }

    /// The emitted light level, 0 to 15.
    #[must_use]
    pub const fn intensity(self) -> u8 { self.intensity }

    /// The emitted colour, as 8-bit red, green, and blue.
    #[must_use]
    pub const fn colour(self) -> [u8; 3] { self.colour }

    /// Whether this voxel kind emits any light at all.
    #[must_use]
    pub const fn is_emissive(self) -> bool { self.intensity > 0 }
}

/// One palette entry: the unit of material meaning (design §7.2).
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct VoxelType {
    /// Unique name within the scene's palette.
    pub name: SmolStr,
    /// Material class, selecting texture set and shading parameters.
    pub material: MaterialClass,
    /// Per-face passability for pathfinding.
    pub passable: Passability,
    /// Direction of rise for ramps and stairs.
    pub slope: SlopeDirection,
    /// Emitted light, range-checked.
    pub emission: LightEmission,
    /// Material-field coefficients, Q8.8 fixed point (design §10.5).
    pub sim: SimProperties,
    /// The ontology concept this kind instantiates, parsed and namespaced.
    pub concept: Option<ConceptIri>,
}

/// The ordered voxel types belonging to one scene.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct Palette {
    entries: Vec<VoxelType>,
}

impl Palette {
    /// The only constructor: validates a wire palette into a domain one.
    ///
    /// # Errors
    ///
    /// Returns every problem found, and no palette. Accumulating rather than
    /// short-circuiting is deliberate: an author fixing one palette entry
    /// should not have to reload to discover the next.
    pub fn from_document(
        entries: &[VoxelTypeDocument],
        namespaces: &NamespaceTable,
    ) -> Result<Self, Vec<SceneDiagnostic>> {
        let mut problems = Vec::new();
        check_palette_shape(entries, &mut problems);

        let mut seen: BTreeSet<&str> = BTreeSet::new();
        let mut built = Vec::with_capacity(entries.len());
        for (ordinal, entry) in entries.iter().enumerate() {
            let at = location(ordinal);
            if !seen.insert(entry.name.as_str()) {
                problems.push(SceneDiagnostic::structural(
                    DiagnosticCode::DuplicateVoxelTypeName,
                    at,
                    format!("two palette entries are named {:?}", entry.name),
                ));
            }
            match convert_entry(entry, at, namespaces) {
                Ok(voxel_type) => built.push(voxel_type),
                Err(found) => problems.extend(found),
            }
        }

        if problems.is_empty() {
            Ok(Self { entries: built })
        } else {
            Err(problems)
        }
    }

    /// Returns the voxel type at `index`, or `None` when it is out of range.
    #[must_use]
    pub fn get(&self, index: VoxelIndex) -> Option<&VoxelType> {
        self.entries.get(usize::from(index.get()))
    }

    /// The number of entries, which is also the exclusive index bound.
    #[must_use]
    pub const fn len(&self) -> usize { self.entries.len() }

    /// Whether the palette has no entries. A valid palette never does.
    #[must_use]
    pub const fn is_empty(&self) -> bool { self.entries.is_empty() }

    /// The entries, in index order.
    pub fn iter(&self) -> impl Iterator<Item = &VoxelType> { self.entries.iter() }

    /// The index of the entry named `name`, when there is one.
    #[must_use]
    pub fn index_of(&self, name: &str) -> Option<VoxelIndex> {
        let position = self.entries.iter().position(|entry| entry.name == name)?;
        u16::try_from(position).ok().map(VoxelIndex::new)
    }
}

impl<'a> IntoIterator for &'a Palette {
    type Item = &'a VoxelType;
    type IntoIter = core::slice::Iter<'a, VoxelType>;

    fn into_iter(self) -> Self::IntoIter { self.entries.iter() }
}

/// The document location of palette entry `ordinal`.
fn location(ordinal: usize) -> DocumentLocation {
    DocumentLocation::new(
        DocumentSection::Palette,
        u32::try_from(ordinal).unwrap_or(u32::MAX),
    )
}

/// Checks the palette as a whole: emptiness, size, and the reserved air entry.
fn check_palette_shape(entries: &[VoxelTypeDocument], problems: &mut Vec<SceneDiagnostic>) {
    let whole = DocumentLocation::section(DocumentSection::Palette);
    if entries.is_empty() {
        problems.push(SceneDiagnostic::structural(
            DiagnosticCode::PaletteEmpty,
            whole,
            "a scene palette must have at least the air entry",
        ));
        return;
    }
    if entries.len() > MAX_PALETTE_ENTRIES {
        problems.push(SceneDiagnostic::structural(
            DiagnosticCode::PaletteTooLarge,
            whole,
            format!(
                "{} entries exceeds the {MAX_PALETTE_ENTRIES} a 16-bit index can address",
                entries.len()
            ),
        ));
    }
    let zero_is_air = entries
        .first()
        .is_some_and(|entry| entry.material == MaterialClass::Air);
    if !zero_is_air {
        problems.push(SceneDiagnostic::structural(
            DiagnosticCode::PaletteZeroNotAir,
            location(0),
            "palette index zero is reserved for air, so an absent chunk and a long air run agree \
             without a lookup",
        ));
    }
}

/// Validates one entry's emission and concept identifier.
fn convert_entry(
    entry: &VoxelTypeDocument,
    at: DocumentLocation,
    namespaces: &NamespaceTable,
) -> Result<VoxelType, Vec<SceneDiagnostic>> {
    let mut problems = Vec::new();

    let emission = LightEmission::new(entry.emission.intensity, entry.emission.colour);
    if let Err(found) = emission {
        problems.push(SceneDiagnostic::structural(
            DiagnosticCode::EmissionOutOfRange,
            at,
            format!(
                "emission intensity {found} is above the 0-{} scale",
                LightEmission::MAX_INTENSITY
            ),
        ));
    }

    let concept = parse_concept(entry.concept.as_ref(), at, namespaces, &mut problems);

    match (emission, problems.is_empty()) {
        (Ok(checked), true) => Ok(VoxelType {
            name: entry.name.clone(),
            material: entry.material,
            passable: entry.passable,
            slope: entry.slope,
            emission: checked,
            sim: entry.sim,
            concept,
        }),
        _ => Err(problems),
    }
}

/// Parses an optional concept identifier, recording rather than raising.
fn parse_concept(
    raw: Option<&SmolStr>,
    at: DocumentLocation,
    namespaces: &NamespaceTable,
    problems: &mut Vec<SceneDiagnostic>,
) -> Option<ConceptIri> {
    let candidate = raw?;
    match ConceptIri::parse(candidate, namespaces) {
        Ok(iri) => Some(iri),
        Err(problem) => {
            problems.push(SceneDiagnostic::structural(
                DiagnosticCode::ConceptIriInvalid,
                at,
                describe_concept_problem(candidate, &problem),
            ));
            None
        }
    }
}

/// The wire form of an emission, for callers converting in the other
/// direction. Kept here so the two forms sit side by side.
#[must_use]
pub const fn emission_to_document(emission: LightEmission) -> EmissionDocument {
    EmissionDocument {
        intensity: emission.intensity(),
        colour: emission.colour(),
    }
}
