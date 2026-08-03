//! The chunk-local run codec and coordinate mapping (roadmap task 1.2.1).
//!
//! Two properties carry most of the weight. The run codec must be a
//! *fixpoint* — expand then collapse returns what went in — and it must
//! produce the *canonical* form, which is strictly stronger: an encoding can
//! round-trip perfectly and still hash differently from another encoding of
//! the same world, which would break design §12.3's content hashes.

use proptest::{prelude::*, test_runner::FileFailurePersistence};
use rstest::rstest;
use thysalion_world::{
    grid::{
        ChunkCoord,
        ChunkSize,
        Extent,
        VoxelGrid,
        VoxelIndex,
        VoxelPos,
        runs::{RunDecodeError, collapse, expand, uniform_index},
    },
    scene::document::{ChunkPayloadDocument, VoxelRunDocument},
};

/// A four-voxel chunk edge, so a whole chunk is 64 voxels and the tests stay
/// cheap.
///
/// Validated at compile time rather than by an `expect` in a helper.
/// `allow-expect-in-tests` covers test *functions*, not the fixtures they
/// call (AGENTS.md), and making the helper fallible instead would force every
/// test to return `Result` — which `clippy::panic_in_result_fn` then forbids
/// from using `assert!`. A `const` sidesteps both: an invalid size fails the
/// build, not the run.
const SMALL_CHUNK: ChunkSize = match ChunkSize::new(4) {
    Ok(size) => size,
    Err(_) => panic!("4 is a valid chunk size"),
};

const fn run(length: u32, index: u16) -> VoxelRunDocument { VoxelRunDocument { length, index } }

#[rstest]
#[case(0, 0, 0)]
#[case(1, 0, 0)]
#[case(0, 1, 0)]
#[case(0, 0, 1)]
#[case(3, 3, 3)]
#[case(2, 1, 3)]
fn the_local_index_and_the_position_are_a_bijection(
    #[case] x: u32,
    #[case] y: u32,
    #[case] z: u32,
) {
    let size = SMALL_CHUNK;
    let pos = VoxelPos::new(x, y, z);
    let index = pos.local_index(size);
    let back = VoxelPos::from_local_index(index, size).expect("in range");
    assert_eq!(back, pos, "index {index} did not map back to {pos:?}");
}

#[rstest]
fn the_index_mapping_is_z_major() {
    let size = SMALL_CHUNK;
    // Within a chunk of side s, (x, y, z) sits at z*s*s + y*s + x.
    assert_eq!(VoxelPos::new(1, 0, 0).local_index(size), 1);
    assert_eq!(VoxelPos::new(0, 1, 0).local_index(size), 4);
    assert_eq!(VoxelPos::new(0, 0, 1).local_index(size), 16);
}

#[rstest]
fn an_index_past_the_chunk_has_no_position() {
    let size = SMALL_CHUNK;
    assert_eq!(VoxelPos::from_local_index(size.volume(), size), None);
}

#[rstest]
fn a_zero_length_run_is_rejected() {
    let error = expand(&[run(0, 1)], 4).expect_err("must reject");
    assert!(matches!(error, RunDecodeError::ZeroLength { ordinal: 0 }));
}

#[rstest]
fn adjacent_runs_sharing_an_index_are_rejected() {
    let error = expand(&[run(2, 1), run(2, 1)], 4).expect_err("must reject");
    assert!(matches!(
        error,
        RunDecodeError::AdjacentDuplicate {
            ordinal: 0,
            index: 1
        }
    ));
}

#[rstest]
#[case(&[run(2, 1)], 4)]
#[case(&[run(2, 1), run(3, 0)], 4)]
fn runs_must_sum_to_the_chunk_volume(#[case] runs: &[VoxelRunDocument], #[case] volume: u64) {
    let error = expand(runs, volume).expect_err("must reject");
    assert!(
        matches!(error, RunDecodeError::LengthMismatch { .. }),
        "got {error:?}"
    );
}

#[rstest]
fn a_run_stream_claiming_a_vast_volume_is_refused_before_allocating() {
    // The length check runs before any allocation proportional to the declared
    // lengths, so this is a cheap rejection rather than an allocation failure.
    let error = expand(&[run(u32::MAX, 1)], 64).expect_err("must reject");
    assert!(matches!(
        error,
        RunDecodeError::LengthMismatch {
            actual: 4_294_967_295,
            expected: 64
        }
    ));
}

#[rstest]
fn a_single_valued_chunk_reports_a_uniform_index() {
    let voxels = vec![VoxelIndex::new(3); 8];
    assert_eq!(uniform_index(&voxels), Some(VoxelIndex::new(3)));
    let mixed = vec![VoxelIndex::new(3), VoxelIndex::AIR];
    assert_eq!(uniform_index(&mixed), None);
}

#[rstest]
fn a_single_valued_dense_chunk_is_elided_to_a_uniform_payload() {
    // Canonicality, not an optimization: two documents describing the same
    // world must encode identically or their content hashes disagree.
    let size = SMALL_CHUNK;
    let extent = Extent::new(4, 4, 4, size).expect("aligned");
    let mut grid = VoxelGrid::empty(extent, size);
    let volume = usize::try_from(size.volume()).expect("small");
    grid.insert_dense(ChunkCoord::new(0, 0, 0), vec![VoxelIndex::new(2); volume])
        .expect("one chunk volume");

    let chunks = grid.to_chunks();
    assert_eq!(chunks.len(), 1);
    let payload = chunks.first().map(|entry| &entry.payload);
    assert_eq!(payload, Some(&ChunkPayloadDocument::Uniform(2)));
}

#[rstest]
fn an_all_air_chunk_is_omitted_entirely() {
    let size = SMALL_CHUNK;
    let extent = Extent::new(4, 4, 4, size).expect("aligned");
    let mut grid = VoxelGrid::empty(extent, size);
    grid.insert_uniform(ChunkCoord::new(0, 0, 0), VoxelIndex::AIR)
        .expect("the origin chunk is within a 4x4x4 extent");
    assert!(
        grid.to_chunks().is_empty(),
        "an absent chunk already means air"
    );
}

#[rstest]
fn a_short_dense_chunk_is_refused() {
    let size = SMALL_CHUNK;
    let extent = Extent::new(4, 4, 4, size).expect("aligned");
    let mut grid = VoxelGrid::empty(extent, size);
    let too_short = vec![VoxelIndex::new(1); 3];
    assert!(
        grid.insert_dense(ChunkCoord::new(0, 0, 0), too_short)
            .is_err(),
        "a short chunk would read as air past its end"
    );
}

#[rstest]
fn a_position_outwith_the_extent_is_none_and_an_empty_chunk_is_air() {
    let size = SMALL_CHUNK;
    let extent = Extent::new(4, 4, 4, size).expect("aligned");
    let grid = VoxelGrid::empty(extent, size);
    assert_eq!(grid.get(VoxelPos::new(0, 0, 0)), Some(VoxelIndex::AIR));
    assert_eq!(
        grid.get(VoxelPos::new(4, 0, 0)),
        None,
        "outwith the extent is not air"
    );
}

/// A dense chunk of `volume` voxels drawn from a small alphabet, so runs form.
fn dense_chunk(volume: usize) -> impl Strategy<Value = Vec<VoxelIndex>> {
    proptest::collection::vec((0u16..4).prop_map(VoxelIndex::new), volume..=volume)
}

proptest! {
    #![proptest_config(ProptestConfig {
        failure_persistence: Some(Box::new(FileFailurePersistence::Direct(
            "tests/proptest-regressions/grid_runs.txt",
        ))),
        ..ProptestConfig::default()
    })]

    /// Collapse then expand returns the original chunk.
    #[test]
    fn the_run_codec_is_a_fixpoint(voxels in dense_chunk(64)) {
        let runs = collapse(&voxels);
        let expanded = expand(&runs, 64).expect("canonical runs must expand");
        prop_assert_eq!(expanded, voxels);
    }

    /// Collapse produces the canonical form, which is stronger than a fixpoint.
    #[test]
    fn collapse_produces_canonical_runs(voxels in dense_chunk(64)) {
        let runs = collapse(&voxels);
        prop_assert!(runs.iter().all(|run| run.length > 0), "no zero-length run");
        prop_assert!(
            runs.windows(2).all(|pair| match pair {
                [first, second] => first.index != second.index,
                _ => true,
            }),
            "no two adjacent runs may share an index"
        );
        let total: u64 = runs.iter().map(|run| u64::from(run.length)).sum();
        prop_assert_eq!(total, 64, "runs must cover the chunk exactly");
    }

    /// Re-collapsing an expanded stream reproduces it byte for byte.
    #[test]
    fn expanding_and_re_collapsing_is_stable(voxels in dense_chunk(64)) {
        let once = collapse(&voxels);
        let twice = collapse(&expand(&once, 64).expect("expand"));
        prop_assert_eq!(once, twice);
    }
}

#[rstest]
fn a_chunk_outwith_the_extent_is_refused_by_both_mutators() {
    // Without this the grid would serialize, through `to_chunks`, a document
    // its own validator refuses — an invalid state reachable through a safe
    // public API, which is what the document/domain split exists to prevent.
    let size = SMALL_CHUNK;
    let extent = Extent::new(4, 4, 4, size).expect("aligned");
    let mut grid = VoxelGrid::empty(extent, size);
    let volume = usize::try_from(size.volume()).expect("small");
    let outwith = ChunkCoord::new(99, 0, 0);

    assert!(
        grid.insert_uniform(outwith, VoxelIndex::new(1)).is_err(),
        "a uniform chunk outwith the extent must be refused"
    );
    assert!(
        grid.insert_dense(outwith, vec![VoxelIndex::new(1); volume])
            .is_err(),
        "a dense chunk outwith the extent must be refused"
    );
    assert!(
        grid.to_chunks().is_empty(),
        "nothing refused may reach the serialized form"
    );
}
