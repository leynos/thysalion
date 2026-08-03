//! The validated scene: what a successful load yields.
//!
//! A `Scene` is only obtainable from
//! [`crate::scene::validation::validate`], and that is the whole point. It
//! derives no `serde` traits, its fields are private, and its constructor is
//! crate-private, so there is no route to a `Scene` that has not passed every
//! rule. Design §7.3 promises "never a partially loaded scene"; this is the
//! mechanism rather than a convention.
//!
//! The scene is the immutable authored baseline. Runtime voxel edits belong to
//! the state plane's overlay, which is what a save archive records — see
//! [`crate::grid`] and ADR 006.

use smol_str::SmolStr;

use crate::{
    codec::{CodecError, Encoding, encode_document},
    grid::{ChunkSize, Extent, VoxelGrid},
    scene::{
        document::{
            EntitiesDocument,
            ExtentDocument,
            KnowledgeDocument,
            Lighting,
            SUPPORTED_VERSION,
            SceneDocument,
            SpawnDocument,
            VoxelPosDocument,
            VoxelTypeDocument,
        },
        entities::Entities,
        knowledge::SceneKnowledge,
        palette::{Palette, emission_to_document},
    },
};

/// A content hash over a scene's canonical encoding.
///
/// Design §12.3 requires save archives to record a hash over their scene
/// assets and to refuse a load on mismatch, and invariant I3 depends on it.
/// A 256-bit BLAKE3 digest: this is an integrity check against drift, not a
/// signature, and BLAKE3 is fast enough to run on every save without
/// thinking about it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct SceneContentHash([u8; 32]);

impl SceneContentHash {
    /// The digest bytes.
    #[must_use]
    pub const fn as_bytes(&self) -> &[u8; 32] { &self.0 }

    /// The digest as lowercase hexadecimal.
    #[must_use]
    pub fn to_hex(&self) -> String { self.to_string() }
}

impl core::fmt::Display for SceneContentHash {
    /// Lowercase hexadecimal, written a byte at a time.
    ///
    /// The formatter is the natural place for this: `write!` inside `fmt` can
    /// use `?`, whereas building the string separately means either discarding
    /// an infallible `Result` — which `clippy::let_underscore_must_use` refuses
    /// — or allocating a `String` per byte, which `clippy::format_collect`
    /// refuses.
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        for byte in self.0 {
            write!(f, "{byte:02x}")?;
        }
        Ok(())
    }
}

/// One validated scene.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct Scene {
    name: SmolStr,
    palette: Palette,
    voxels: VoxelGrid,
    entities: Entities,
    lighting: Lighting,
    knowledge: SceneKnowledge,
}

impl Scene {
    /// Assembles a scene from parts every rule has already accepted.
    ///
    /// Crate-private, and taking one struct rather than six arguments so that
    /// adding a section is not a signature change for the orchestrator.
    pub(crate) fn from_validated(parts: SceneParts) -> Self {
        Self {
            name: parts.name,
            palette: parts.palette,
            voxels: parts.voxels,
            entities: parts.entities,
            lighting: parts.lighting,
            knowledge: parts.knowledge,
        }
    }

    /// The scene's stable machine name.
    #[must_use]
    pub fn name(&self) -> &str { &self.name }

    /// The scene's palette.
    #[must_use]
    pub const fn palette(&self) -> &Palette { &self.palette }

    /// The scene's voxels.
    #[must_use]
    pub const fn voxels(&self) -> &VoxelGrid { &self.voxels }

    /// The scene's spawn points, with prototype chains resolved.
    #[must_use]
    pub const fn entities(&self) -> &Entities { &self.entities }

    /// The scene's sun path, ambient bands, and probe spacing.
    #[must_use]
    pub const fn lighting(&self) -> &Lighting { &self.lighting }

    /// The scene's named graph and its TriG resources.
    #[must_use]
    pub const fn knowledge(&self) -> &SceneKnowledge { &self.knowledge }

    /// The scene's bounds, in voxels.
    #[must_use]
    pub const fn extent(&self) -> Extent { self.voxels.extent() }

    /// The chunk edge length, in voxels.
    #[must_use]
    pub const fn chunk_size(&self) -> ChunkSize { self.voxels.chunk_size() }

    /// Count of voxels that are not air.
    #[must_use]
    pub fn non_air_count(&self) -> u64 { self.voxels.non_air_count() }

    /// Rebuilds the wire document this scene is equivalent to.
    ///
    /// Canonical by construction: the grid emits its chunks sorted with uniform
    /// chunks elided, and every map in the tree is a `BTreeMap`. Re-encoding
    /// the result therefore yields the same bytes for equal scenes, whichever
    /// encoding the scene was read from.
    #[must_use]
    pub fn to_document(&self) -> SceneDocument {
        SceneDocument {
            version: SUPPORTED_VERSION,
            name: self.name.clone(),
            dimensions: ExtentDocument {
                x: self.voxels.extent().x(),
                y: self.voxels.extent().y(),
                z: self.voxels.extent().z(),
            },
            chunk_size: self.voxels.chunk_size().get(),
            palette: self.palette.iter().map(voxel_type_to_document).collect(),
            voxels: self.voxels.to_chunks(),
            entities: self.entities_to_document(),
            lighting: self.lighting.clone(),
            knowledge: self.knowledge_to_document(),
        }
    }

    /// Canonical content hash over this scene's MessagePack encoding.
    ///
    /// Independent of the encoding the scene was loaded from: a scene read as
    /// JSON and the same scene read as MessagePack hash identically, which is
    /// what stops a save taken in a development build refusing itself in a
    /// shipped one. Well defined only because of the canonicality rules the
    /// document types enforce.
    ///
    /// # Errors
    ///
    /// Returns [`CodecError::Encode`] when the encoder fails. In practice it
    /// cannot — the writer is a `Vec` and the document tree holds no type that
    /// can refuse to serialize — but the encoder's signature is fallible and
    /// this crate forbids `expect`, so the caller sees the truth rather than a
    /// panic waiting to be discovered.
    pub fn content_hash(&self) -> Result<SceneContentHash, CodecError> {
        let bytes = encode_document(&self.to_document(), Encoding::MessagePack)?;
        Ok(SceneContentHash(blake3::hash(&bytes).into()))
    }

    /// The entity section in wire form.
    ///
    /// Prototypes do not round-trip: resolution flattens them into the spawns
    /// that used them, and re-deriving an inheritance structure from flat data
    /// would be invention. Authoring sources keep the prototypes; the shipped
    /// document does not need them, and the content hash is over the shipped
    /// form.
    fn entities_to_document(&self) -> EntitiesDocument {
        EntitiesDocument {
            prototypes: std::collections::BTreeMap::new(),
            spawns: self
                .entities
                .iter()
                .map(|spawn| SpawnDocument {
                    name: spawn.name.clone(),
                    prototype: None,
                    at: VoxelPosDocument {
                        x: spawn.at.x,
                        y: spawn.at.y,
                        z: spawn.at.z,
                    },
                    facing: spawn.facing,
                    airborne: spawn.airborne,
                    concept: spawn.concept.as_ref().map(|iri| SmolStr::new(iri.as_str())),
                })
                .collect(),
        }
    }

    /// The knowledge section in wire form.
    fn knowledge_to_document(&self) -> KnowledgeDocument {
        KnowledgeDocument {
            graph: SmolStr::new(self.knowledge.graph().as_str()),
            sources: self
                .knowledge
                .sources()
                .map(|path| SmolStr::new(path.as_str()))
                .collect(),
        }
    }
}

/// One palette entry in wire form.
fn voxel_type_to_document(kind: &crate::scene::palette::VoxelType) -> VoxelTypeDocument {
    VoxelTypeDocument {
        name: kind.name.clone(),
        material: kind.material,
        passable: kind.passable,
        slope: kind.slope,
        emission: emission_to_document(kind.emission),
        sim: kind.sim,
        concept: kind.concept.as_ref().map(|iri| SmolStr::new(iri.as_str())),
    }
}

/// The validated parts of a scene, ready to assemble.
///
/// Crate-private, and a struct rather than a parameter list: the orchestrator
/// builds these one section at a time, and six positional arguments of which
/// four are section types is exactly the signature that silently swaps two.
pub(crate) struct SceneParts {
    /// The scene's stable machine name.
    pub name: SmolStr,
    /// The validated palette.
    pub palette: Palette,
    /// The decoded voxels.
    pub voxels: VoxelGrid,
    /// The resolved spawn points.
    pub entities: Entities,
    /// The lighting section.
    pub lighting: Lighting,
    /// The validated knowledge section.
    pub knowledge: SceneKnowledge,
}
