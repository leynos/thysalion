//! Shared fixtures for the scene-format integration tests.
//!
//! `minimal_document` mirrors `tests/fixtures/minimal.scene.json` exactly; the
//! golden test asserts the two agree, so this constructor is the executable
//! statement of what that fixture means.
//!
//! `strategy.rs` sits beside this file but is *not* re-exported from it. An
//! integration test compiles each `tests/*.rs` as its own crate, so a helper
//! reachable from a module a test declares but never uses is dead code in that
//! crate — and `make test` runs with warnings denied. Each test therefore
//! declares only the helpers it uses.

use std::collections::BTreeMap;

use smol_str::SmolStr;
use thysalion_world::scene::document::{
    ChunkCoordDocument,
    ChunkEntryDocument,
    ChunkPayloadDocument,
    DocumentVersion,
    EmissionDocument,
    EntitiesDocument,
    ExtentDocument,
    Face,
    KnowledgeDocument,
    Lighting,
    MaterialClass,
    Passability,
    SceneDocument,
    SimProperties,
    SlopeDirection,
    SpawnDocument,
    SunPath,
    VoxelPosDocument,
    VoxelRunDocument,
    VoxelTypeDocument,
};

/// Air is index zero in every palette, and is the only fully passable entry.
fn air() -> VoxelTypeDocument {
    VoxelTypeDocument {
        name: SmolStr::new("air"),
        material: MaterialClass::Air,
        passable: Passability::open(),
        slope: SlopeDirection::Flat,
        emission: EmissionDocument::dark(),
        sim: SimProperties::inert(),
        concept: None,
    }
}

/// A solid, fully impassable voxel type with no emission and inert material.
fn solid(name: &str, material: MaterialClass) -> VoxelTypeDocument {
    VoxelTypeDocument {
        name: SmolStr::new(name),
        material,
        passable: Passability::closed(),
        slope: SlopeDirection::Flat,
        emission: EmissionDocument::dark(),
        sim: SimProperties::inert(),
        concept: None,
    }
}

/// Air, stone, and a wall sconce: enough to exercise emission and concepts.
fn minimal_palette() -> Vec<VoxelTypeDocument> {
    let mut stone = solid("stone-block", MaterialClass::Stone);
    stone.sim = SimProperties {
        fuel: 0,
        ignition_point: u16::MAX,
        moisture_capacity: 3277,
    };
    stone.concept = Some(SmolStr::new("thy:StoneBlock"));

    let mut sconce = solid("wall-sconce", MaterialClass::Stone);
    sconce.emission = EmissionDocument {
        intensity: 12,
        colour: [255, 180, 90],
    };
    sconce.concept = Some(SmolStr::new("thy:WallSconce"));

    vec![air(), stone, sconce]
}

/// Four rows of stone on the `z = 0` layer of chunk `(0, 0, 0)`, then air.
///
/// Chunk-local Z-major, so a row of four stone at `y = n` is four voxels of
/// stone followed by twenty-eight of air. The final air run absorbs the rest
/// of the chunk: `32768 - (4 * 4) - (28 * 3)`.
fn minimal_runs() -> Vec<VoxelRunDocument> {
    let mut runs = Vec::new();
    let mut row = |air_after: u32| {
        runs.push(VoxelRunDocument {
            length: 4,
            index: 1,
        });
        runs.push(VoxelRunDocument {
            length: air_after,
            index: 0,
        });
    };
    for _ in 0..3 {
        row(28);
    }
    row(32_668);
    runs
}

/// One run-encoded chunk and one uniform chunk of stone.
fn minimal_voxels() -> Vec<ChunkEntryDocument> {
    vec![
        ChunkEntryDocument {
            at: ChunkCoordDocument { x: 0, y: 0, z: 0 },
            payload: ChunkPayloadDocument::Runs(minimal_runs()),
        },
        ChunkEntryDocument {
            at: ChunkCoordDocument { x: 1, y: 0, z: 0 },
            payload: ChunkPayloadDocument::Uniform(1),
        },
    ]
}

/// One spawn and no prototypes.
fn minimal_entities() -> EntitiesDocument {
    EntitiesDocument {
        prototypes: BTreeMap::new(),
        spawns: vec![SpawnDocument {
            name: SmolStr::new("party-start"),
            prototype: None,
            at: VoxelPosDocument { x: 2, y: 2, z: 1 },
            facing: Face::PosY,
            airborne: false,
            concept: None,
        }],
    }
}

/// A mid-afternoon sun and default probe spacing.
const fn minimal_lighting() -> Lighting {
    Lighting {
        sun_path: SunPath {
            azimuth_centidegrees: 13_500,
            elevation_centidegrees: 3_000,
        },
        ambient_bands: Vec::new(),
        probe_spacing_mm: 2_000,
    }
}

/// The minimal hand-written scene, as specified by the 1.2 execution plan.
///
/// Two chunks: one run-encoded chunk carrying four rows of stone on its
/// `z = 0` layer, and one uniform chunk of stone. Sixteen non-air voxels in
/// the first and 32,768 in the second, for 32,784 in total.
#[must_use]
pub fn minimal_document() -> SceneDocument {
    SceneDocument {
        version: DocumentVersion { major: 1, minor: 0 },
        name: SmolStr::new("minimal"),
        dimensions: ExtentDocument {
            x: 64,
            y: 32,
            z: 32,
        },
        chunk_size: 32,
        palette: minimal_palette(),
        voxels: minimal_voxels(),
        entities: minimal_entities(),
        lighting: minimal_lighting(),
        knowledge: KnowledgeDocument {
            graph: SmolStr::new("thy:scene/minimal"),
            sources: vec![SmolStr::new("knowledge/minimal.trig")],
        },
    }
}
