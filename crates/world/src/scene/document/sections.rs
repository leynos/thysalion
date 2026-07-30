//! Wire types for the entity, lighting, and knowledge sections (design §7.3).
//!
//! Two things here are worth knowing before reading the fields.
//!
//! Entity spawns carry a *closed, typed* field set rather than an untyped
//! component bag. `serde_json::Value` is precisely the type whose behaviour
//! differs between the two encodings, and there is no Entity Component System
//! component vocabulary to author against until phase 4. Deferring the
//! vocabulary to the phase that owns it costs nothing here and keeps the
//! round-trip guarantee provable.
//!
//! Every quantity is an integer. Angles are centi-degrees and lengths are
//! millimetres, so the whole document tree keeps `Eq`, `Ord`, and `Hash`; NaN
//! and the infinities — which JSON cannot represent and `serde_json` writes as
//! `null` and then fails to read back — cannot arise; and design §10.5's
//! fixed-point mandate for simulation coefficients is not half-honoured.

use std::collections::BTreeMap;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use smol_str::SmolStr;

use super::{voxel_type::Face, voxels::VoxelPosDocument};

/// A named prototype that spawns may extend.
///
/// Resolution is field-wise: a `Some` on the child wins over the parent, and a
/// `None` inherits. Chains are resolved iteratively against a depth bound,
/// never recursively — cycle detection alone does not save a recursive
/// resolver from an acyclic chain thousands deep, and the resulting stack
/// overflow is a signal rather than a `Result`.
///
/// The inheritable field set is deliberately narrow. A prototype can only
/// usefully default a field a spawn may omit, and [`SpawnDocument`] makes
/// `facing` and `airborne` mandatory — a default for either could never take
/// effect, so neither is offered. Phase 4's component vocabulary is what will
/// give prototypes weight; until then `concept` is the field with a real
/// authoring cost to amortize.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct PrototypeDocument {
    /// The prototype this one extends, if any.
    pub extends: Option<SmolStr>,
    /// Default ontology concept for spawns using this prototype.
    pub concept: Option<SmolStr>,
}

/// One entity placed in the scene.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct SpawnDocument {
    /// Name, unique within the scene.
    pub name: SmolStr,
    /// The prototype this spawn extends, if any.
    pub prototype: Option<SmolStr>,
    /// Where the entity stands, in voxels.
    pub at: VoxelPosDocument,
    /// Which way the entity faces.
    pub facing: Face,
    /// Whether the entity is expected to have no support beneath it.
    ///
    /// Load emits a warning for an unsupported spawn unless this is set, which
    /// catches the "loads clean, is nonsense" class cheaply without forbidding
    /// a scene that legitimately wants a floating entity.
    pub airborne: bool,
    /// The ontology concept this entity instantiates, when it has one.
    pub concept: Option<SmolStr>,
}

/// The scene's entity section.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct EntitiesDocument {
    /// Named prototypes, ordered so the encoding is byte-stable.
    pub prototypes: BTreeMap<SmolStr, PrototypeDocument>,
    /// The spawn list.
    pub spawns: Vec<SpawnDocument>,
}

/// The authored sun path (design §9.4).
///
/// No `Document` suffix, and shared between the wire and the domain: this step
/// enforces no invariant over a sun path, so there is no second form for a
/// domain type to be. Phase 3 owns the lighting semantics and is the phase with
/// a use for the values; when it adds a rule, the domain form arrives with it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct SunPath {
    /// Compass bearing of the sun at noon, in centi-degrees.
    pub azimuth_centidegrees: i32,
    /// Elevation of the sun at noon, in centi-degrees.
    pub elevation_centidegrees: i32,
}

/// One time-of-day band in the scene's ambient palette (design §9.4).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct AmbientBand {
    /// Band name, such as `dawn` or `blue-hour`.
    pub name: SmolStr,
    /// Sun elevation at which this band applies, in centi-degrees.
    pub at_centidegrees: i32,
    /// Ambient fill colour as 8-bit red, green, and blue channels.
    pub colour: [u8; 3],
}

/// The scene's lighting section.
///
/// Shared between the wire and the domain, for the reason given on [`SunPath`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Lighting {
    /// The authored sun path.
    pub sun_path: SunPath,
    /// Ambient palette bands, in authoring order.
    pub ambient_bands: Vec<AmbientBand>,
    /// Probe-volume spacing override, in millimetres (design §9.3).
    pub probe_spacing_mm: u32,
}

/// The scene's knowledge section (design §7.3, §11.5).
///
/// The TriG sources are named but not parsed at this step: load confirms each
/// resolves through the scene source and nothing more. Parsing them is the
/// knowledge plane's work at roadmap step 5.1.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct KnowledgeDocument {
    /// The Internationalized Resource Identifier of the scene's named graph.
    pub graph: SmolStr,
    /// TriG resources to load, relative to the scene document's directory.
    pub sources: Vec<SmolStr>,
}
