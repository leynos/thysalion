//! Bounded `proptest` generators for scene documents.
//!
//! The bounds matter. Generation cost is superlinear in extent, and an
//! unbounded generator would make the wire-contract property the slowest thing
//! in continuous integration for no extra coverage: the properties under test
//! are about *encoding shape*, which a small document exercises exactly as well
//! as a large one.
//!
//! Documents generated here are well-*formed* but not necessarily
//! well-*formed-and-valid*: run lengths need not sum to a chunk volume, and
//! palette indices may be out of range. That is deliberate. These generators
//! feed the wire contract, which must hold for any document the decoder
//! accepts; semantic validity is Stage C2's subject and has its own fixtures.

use proptest::{
    collection::{btree_map, vec},
    prelude::*,
};
use smol_str::SmolStr;
use thysalion_world::scene::document::{
    AmbientBand,
    ChunkCoordDocument,
    ChunkEntryDocument,
    ChunkPayloadDocument,
    EmissionDocument,
    EntitiesDocument,
    ExtentDocument,
    Face,
    KnowledgeDocument,
    Lighting,
    MaterialClass,
    Passability,
    PrototypeDocument,
    SUPPORTED_VERSION,
    SceneDocument,
    SimProperties,
    SlopeDirection,
    SpawnDocument,
    SunPath,
    VoxelPosDocument,
    VoxelRunDocument,
    VoxelTypeDocument,
};

const MAX_CHUNKS: usize = 8;
const MAX_RUNS: usize = 64;
const MAX_PALETTE: usize = 16;

fn name() -> impl Strategy<Value = SmolStr> { "[a-z][a-z0-9-]{0,15}".prop_map(SmolStr::new) }

fn concept() -> impl Strategy<Value = Option<SmolStr>> {
    prop_oneof![
        1 => Just(None),
        3 => "thy:[A-Za-z][A-Za-z0-9]{0,12}".prop_map(|s| Some(SmolStr::new(s))),
    ]
}

fn material() -> impl Strategy<Value = MaterialClass> {
    prop_oneof![
        Just(MaterialClass::Air),
        Just(MaterialClass::Stone),
        Just(MaterialClass::Timber),
        Just(MaterialClass::Roofing),
        Just(MaterialClass::Cloth),
        Just(MaterialClass::Ground),
        Just(MaterialClass::Natural),
        Just(MaterialClass::Water),
    ]
}

fn slope() -> impl Strategy<Value = SlopeDirection> {
    prop_oneof![
        Just(SlopeDirection::Flat),
        Just(SlopeDirection::PosX),
        Just(SlopeDirection::NegX),
        Just(SlopeDirection::PosY),
        Just(SlopeDirection::NegY),
    ]
}

fn face() -> impl Strategy<Value = Face> {
    prop_oneof![
        Just(Face::PosX),
        Just(Face::NegX),
        Just(Face::PosY),
        Just(Face::NegY),
        Just(Face::PosZ),
        Just(Face::NegZ),
    ]
}

fn passability() -> impl Strategy<Value = Passability> {
    (
        any::<bool>(),
        any::<bool>(),
        any::<bool>(),
        any::<bool>(),
        any::<bool>(),
        any::<bool>(),
    )
        .prop_map(|(pos_x, neg_x, pos_y, neg_y, pos_z, neg_z)| Passability {
            pos_x,
            neg_x,
            pos_y,
            neg_y,
            pos_z,
            neg_z,
        })
}

fn voxel_type() -> impl Strategy<Value = VoxelTypeDocument> {
    (
        name(),
        material(),
        passability(),
        slope(),
        (0u8..=15, any::<[u8; 3]>()),
        (any::<u16>(), any::<u16>(), any::<u16>()),
        concept(),
    )
        .prop_map(
            |(name, material, passable, slope, (intensity, colour), sim, concept)| {
                VoxelTypeDocument {
                    name,
                    material,
                    passable,
                    slope,
                    emission: EmissionDocument { intensity, colour },
                    sim: SimProperties {
                        fuel: sim.0,
                        ignition_point: sim.1,
                        moisture_capacity: sim.2,
                    },
                    concept,
                }
            },
        )
}

fn payload() -> impl Strategy<Value = ChunkPayloadDocument> {
    prop_oneof![
        any::<u16>().prop_map(ChunkPayloadDocument::Uniform),
        vec((1u32..4096, any::<u16>()), 1..MAX_RUNS).prop_map(|runs| {
            ChunkPayloadDocument::Runs(
                runs.into_iter()
                    .map(|(length, index)| VoxelRunDocument { length, index })
                    .collect(),
            )
        }),
    ]
}

fn chunk_entry() -> impl Strategy<Value = ChunkEntryDocument> {
    ((0u32..8, 0u32..8, 0u32..4), payload()).prop_map(|((x, y, z), payload)| ChunkEntryDocument {
        at: ChunkCoordDocument { x, y, z },
        payload,
    })
}

fn spawn() -> impl Strategy<Value = SpawnDocument> {
    (
        name(),
        proptest::option::of(name()),
        (0u32..256, 0u32..256, 0u32..64),
        face(),
        any::<bool>(),
        concept(),
    )
        .prop_map(
            |(name, prototype, (x, y, z), facing, airborne, concept)| SpawnDocument {
                name,
                prototype,
                at: VoxelPosDocument { x, y, z },
                facing,
                airborne,
                concept,
            },
        )
}

fn prototype() -> impl Strategy<Value = PrototypeDocument> {
    (proptest::option::of(name()), concept())
        .prop_map(|(extends, concept)| PrototypeDocument { extends, concept })
}

fn lighting() -> impl Strategy<Value = Lighting> {
    (
        -36_000i32..=36_000,
        -9_000i32..=9_000,
        vec((name(), -36_000i32..=36_000, any::<[u8; 3]>()), 0..3),
        1u32..10_000,
    )
        .prop_map(|(azimuth, elevation, bands, spacing)| Lighting {
            sun_path: SunPath {
                azimuth_centidegrees: azimuth,
                elevation_centidegrees: elevation,
            },
            ambient_bands: bands
                .into_iter()
                .map(|(name, at, colour)| AmbientBand {
                    name,
                    at_centidegrees: at,
                    colour,
                })
                .collect(),
            probe_spacing_mm: spacing,
        })
}

/// A well-formed, bounded scene document.
pub fn scene_document() -> impl Strategy<Value = SceneDocument> {
    (
        name(),
        vec(voxel_type(), 1..MAX_PALETTE),
        vec(chunk_entry(), 0..MAX_CHUNKS),
        (btree_map(name(), prototype(), 0..3), vec(spawn(), 0..4)),
        lighting(),
        (name(), vec(name(), 0..3)),
    )
        .prop_map(
            |(name, palette, voxels, (prototypes, spawns), lighting, knowledge)| {
                SceneDocument {
                    // Pinned to what this build accepts: the property under
                    // test is wire parity, and version gating has its own
                    // dedicated tests.
                    version: SUPPORTED_VERSION,
                    name,
                    dimensions: ExtentDocument {
                        x: 256,
                        y: 256,
                        z: 128,
                    },
                    chunk_size: 32,
                    palette,
                    voxels,
                    entities: EntitiesDocument { prototypes, spawns },
                    lighting,
                    knowledge: KnowledgeDocument {
                        graph: knowledge.0,
                        sources: knowledge.1,
                    },
                }
            },
        )
}
