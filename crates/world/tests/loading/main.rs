//! Behavioural specification for scene loading (roadmap task 1.2.2).
//!
//! Scenarios live in `tests/features/scene_loading.feature`. Steps receive a
//! [`support::LoaderSession`] through the reserved `rstest_bdd_harness_context`
//! fixture; the adapter builds one per scenario, so a failed load in one
//! scenario cannot reach another.
//!
//! Corrupt documents are derived here by mutating the minimal document rather
//! than being read from disk. That guarantees the corruption is the *only*
//! difference from a valid scene, which is exactly what each scenario claims
//! to be testing. The hand-written corrupt fixtures under
//! `tests/fixtures/corrupt/` serve a different purpose: they pin the rendered
//! diagnostic report, one file per corruption class.

mod support;

#[path = "../support/mod.rs"]
mod fixtures;

use fixtures::minimal_document;
use rstest_bdd_macros::{given, scenario, then, when};
use smol_str::SmolStr;
use support::{FIXTURE_NAMES, LoaderHarness, LoaderSession};
use thysalion_world::{
    codec::Encoding,
    scene::{
        document::{
            ChunkPayloadDocument,
            SceneDocument,
            SpawnDocument,
            VoxelPosDocument,
            VoxelRunDocument,
        },
        validation::{DiagnosticCode, SceneDiagnostic},
    },
};

/// A position well outwith the minimal scene's 64 x 32 x 32 extent.
const OUTWITH_GRID: VoxelPosDocument = VoxelPosDocument {
    x: 9_999,
    y: 0,
    z: 0,
};

/// Inside the uniform stone chunk, which spans `x` 32 to 63 and is solid.
const INSIDE_STONE: VoxelPosDocument = VoxelPosDocument { x: 40, y: 4, z: 4 };

/// The first run of the first chunk, which every mutator below reaches for.
///
/// # Panics
///
/// Panics when the minimal fixture no longer has a run-encoded first chunk.
/// Returning `None` and quietly skipping the mutation would leave a scenario
/// asserting that a *valid* document is rejected, and the failure would name
/// the assertion rather than the fixture drift that caused it.
fn first_run(document: &mut SceneDocument) -> &mut VoxelRunDocument {
    let Some(entry) = document.voxels.first_mut() else {
        panic!("the minimal fixture must declare at least one chunk");
    };
    let ChunkPayloadDocument::Runs(runs) = &mut entry.payload else {
        panic!("the minimal fixture's first chunk must be run-encoded");
    };
    let Some(first) = runs.first_mut() else {
        panic!("the minimal fixture's first chunk must declare at least one run");
    };
    first
}

/// The first spawn, for the same reason and with the same contract.
///
/// # Panics
///
/// Panics when the minimal fixture declares no spawn.
fn first_spawn(document: &mut SceneDocument) -> &mut SpawnDocument {
    let Some(spawn) = document.entities.spawns.first_mut() else {
        panic!("the minimal fixture must declare at least one spawn");
    };
    spawn
}

/// Replaces the first run's palette index with one the palette cannot resolve.
fn with_unknown_palette_index() -> SceneDocument {
    let mut document = minimal_document();
    first_run(&mut document).index = 99;
    document
}

/// Moves the spawn outwith the declared extent.
fn with_out_of_bounds_spawn() -> SceneDocument {
    let mut document = minimal_document();
    first_spawn(&mut document).at = OUTWITH_GRID;
    document
}

/// Puts the spawn inside the uniform stone chunk, which is solid.
fn with_obstructed_spawn() -> SceneDocument {
    let mut document = minimal_document();
    first_spawn(&mut document).at = INSIDE_STONE;
    document
}

/// Three faults in one document, all in the semantic phase so they accumulate.
///
/// One unresolvable palette index, one spawn outwith the grid, and one concept
/// identifier in a namespace the project does not publish. All three are phase
/// three, which is the point: the scenario asserts that a phase reports every
/// problem it finds rather than the first.
fn with_three_faults() -> SceneDocument {
    let mut document = with_unknown_palette_index();
    let spawn = first_spawn(&mut document);
    spawn.at = OUTWITH_GRID;
    spawn.concept = Some(SmolStr::new("not-a-project-namespace:Thing"));
    document
}

#[given("the minimal hand-written scene document")]
fn given_minimal(#[from(rstest_bdd_harness_context)] session: &mut LoaderSession) {
    session.document = Some(minimal_document());
}

#[given("the scene document with an out-of-range palette index")]
fn given_unknown_index(#[from(rstest_bdd_harness_context)] session: &mut LoaderSession) {
    session.document = Some(with_unknown_palette_index());
}

#[given("the scene document with a spawn outwith the grid")]
fn given_out_of_bounds_spawn(#[from(rstest_bdd_harness_context)] session: &mut LoaderSession) {
    session.document = Some(with_out_of_bounds_spawn());
}

#[given("the scene document naming a missing TriG file")]
fn given_missing_trig(#[from(rstest_bdd_harness_context)] session: &mut LoaderSession) {
    session.document = Some(minimal_document());
    session.forget("knowledge/minimal.trig");
}

#[given("the scene document with three independent faults")]
fn given_three_faults(#[from(rstest_bdd_harness_context)] session: &mut LoaderSession) {
    session.document = Some(with_three_faults());
}

#[given("the scene document with a spawn inside a solid voxel")]
fn given_obstructed_spawn(#[from(rstest_bdd_harness_context)] session: &mut LoaderSession) {
    session.document = Some(with_obstructed_spawn());
}

#[when("the scene is loaded")]
fn when_loaded(#[from(rstest_bdd_harness_context)] session: &mut LoaderSession) {
    session.load(Encoding::Json);
    if let Some(Ok(loaded)) = session.outcome.as_ref() {
        session.from_json = Some(loaded.scene.clone());
    }
}

#[when("the document is re-encoded as MessagePack and loaded")]
fn when_loaded_as_msgpack(#[from(rstest_bdd_harness_context)] session: &mut LoaderSession) {
    session.load(Encoding::Json);
    if let Some(Ok(loaded)) = session.outcome.as_ref() {
        session.from_json = Some(loaded.scene.clone());
    }
    session.load(Encoding::MessagePack);
}

#[when("the minimal hand-written scene document is loaded afterwards")]
fn when_minimal_loaded_after(#[from(rstest_bdd_harness_context)] session: &mut LoaderSession) {
    let document = minimal_document();
    session.load_other(&document);
}

#[then("loading succeeds")]
fn then_succeeds(#[from(rstest_bdd_harness_context)] session: &mut LoaderSession) {
    let _ = session.loaded();
}

#[then("loading fails")]
fn then_fails(#[from(rstest_bdd_harness_context)] session: &mut LoaderSession) {
    let _ = session.diagnostics();
}

#[then("the scene reports 3 palette entries")]
fn then_three_palette_entries(#[from(rstest_bdd_harness_context)] session: &mut LoaderSession) {
    assert_eq!(session.loaded().scene.palette().len(), 3);
}

#[then("the scene reports 32784 non-air voxels")]
fn then_non_air_count(#[from(rstest_bdd_harness_context)] session: &mut LoaderSession) {
    assert_eq!(session.loaded().scene.non_air_count(), 32_784);
}

#[then("the loaded scene equals the scene loaded from JSON")]
fn then_equals_json(#[from(rstest_bdd_harness_context)] session: &mut LoaderSession) {
    let Some(from_json) = session.from_json.as_ref() else {
        panic!("the JSON load must run before the comparison");
    };
    assert_eq!(&session.loaded().scene, from_json);
}

#[then("the diagnostics name the unknown palette index")]
fn then_names_unknown_index(#[from(rstest_bdd_harness_context)] session: &mut LoaderSession) {
    let codes: Vec<DiagnosticCode> = session
        .diagnostics()
        .iter()
        .map(SceneDiagnostic::code)
        .collect();
    assert!(
        codes.contains(&DiagnosticCode::UnknownPaletteIndex),
        "expected an unknown-palette-index diagnostic, got {codes:?}"
    );
}

#[then("the diagnostics locate it by chunk and chunk-local position")]
fn then_locates_by_position(#[from(rstest_bdd_harness_context)] session: &mut LoaderSession) {
    // A run ordinal is useless in a 134-million-voxel scene: the reader needs
    // a place to look, not an index into a stream no human wrote.
    let located = session
        .diagnostics()
        .iter()
        .find(|d| d.code() == DiagnosticCode::UnknownPaletteIndex)
        .and_then(SceneDiagnostic::voxel_position);
    assert!(
        located.is_some(),
        "the diagnostic must carry a chunk coordinate and a chunk-local position"
    );
}

#[then("the diagnostics name the out-of-bounds spawn")]
fn then_names_spawn(#[from(rstest_bdd_harness_context)] session: &mut LoaderSession) {
    let codes: Vec<DiagnosticCode> = session
        .diagnostics()
        .iter()
        .map(SceneDiagnostic::code)
        .collect();
    assert!(
        codes.contains(&DiagnosticCode::SpawnOutwithGrid),
        "expected an out-of-bounds spawn diagnostic, got {codes:?}"
    );
}

#[then("the diagnostics name the missing knowledge resource")]
fn then_names_missing_resource(#[from(rstest_bdd_harness_context)] session: &mut LoaderSession) {
    let codes: Vec<DiagnosticCode> = session
        .diagnostics()
        .iter()
        .map(SceneDiagnostic::code)
        .collect();
    assert!(
        codes.contains(&DiagnosticCode::KnowledgeResourceAbsent),
        "expected a missing-resource diagnostic, got {codes:?}"
    );
}

#[then("the diagnostics list 3 problems")]
fn then_three_problems(#[from(rstest_bdd_harness_context)] session: &mut LoaderSession) {
    let codes: Vec<DiagnosticCode> = session
        .diagnostics()
        .iter()
        .map(SceneDiagnostic::code)
        .collect();
    assert_eq!(
        codes.len(),
        3,
        "expected three accumulated problems, got {codes:?}"
    );
}

#[then("the warnings name the obstructed spawn")]
fn then_warns_obstructed(#[from(rstest_bdd_harness_context)] session: &mut LoaderSession) {
    let codes: Vec<DiagnosticCode> = session
        .loaded()
        .warnings
        .iter()
        .map(SceneDiagnostic::code)
        .collect();
    assert!(
        codes.contains(&DiagnosticCode::SpawnObstructed),
        "expected an obstructed-spawn warning, got {codes:?}"
    );
}

#[scenario(path = "tests/features/scene_loading.feature", index = 0, harness = LoaderHarness)]
fn a_well_formed_scene_loads() {}

#[scenario(path = "tests/features/scene_loading.feature", index = 1, harness = LoaderHarness)]
fn the_scene_survives_a_messagepack_round_trip() {}

#[scenario(path = "tests/features/scene_loading.feature", index = 2, harness = LoaderHarness)]
fn an_unknown_palette_index_is_rejected() {}

#[scenario(path = "tests/features/scene_loading.feature", index = 3, harness = LoaderHarness)]
fn an_out_of_bounds_spawn_is_rejected() {}

#[scenario(path = "tests/features/scene_loading.feature", index = 4, harness = LoaderHarness)]
fn a_dangling_knowledge_iri_is_rejected() {}

#[scenario(path = "tests/features/scene_loading.feature", index = 5, harness = LoaderHarness)]
fn every_problem_in_a_phase_is_reported_at_once() {}

#[scenario(path = "tests/features/scene_loading.feature", index = 6, harness = LoaderHarness)]
fn a_failed_load_leaves_the_loader_usable() {}

#[scenario(path = "tests/features/scene_loading.feature", index = 7, harness = LoaderHarness)]
fn a_spawn_inside_a_wall_warns_without_failing() {}

#[scenario(path = "tests/features/scene_loading.feature", index = 8, harness = LoaderHarness)]
fn every_shipped_fixture_scene_loads_clean() {}

#[scenario(path = "tests/features/scene_loading.feature", index = 9, harness = LoaderHarness)]
fn a_fixture_scene_survives_a_messagepack_round_trip() {}

#[given("the shipped fixture scenes")]
fn given_shipped_fixtures(#[from(rstest_bdd_harness_context)] session: &mut LoaderSession) {
    session.fixtures.clear();
}

#[given("the shipped fixture scene \"keep-interior\"")]
fn given_named_fixture(#[from(rstest_bdd_harness_context)] session: &mut LoaderSession) {
    session.fixtures.clear();
}

#[when("each fixture scene is loaded from disk")]
fn when_each_fixture_loaded(#[from(rstest_bdd_harness_context)] session: &mut LoaderSession) {
    for name in FIXTURE_NAMES {
        session.load_fixture(name);
    }
}

#[when("the fixture is loaded from disk")]
fn when_fixture_loaded(#[from(rstest_bdd_harness_context)] session: &mut LoaderSession) {
    session.load_fixture("keep-interior");
}

#[then("every fixture scene loads")]
fn then_every_fixture_loads(#[from(rstest_bdd_harness_context)] session: &mut LoaderSession) {
    assert_eq!(
        session.fixtures.len(),
        FIXTURE_NAMES.len(),
        "every shipped fixture must have been attempted"
    );
    for (name, outcome) in &session.fixtures {
        if let Err(error) = outcome {
            panic!("{name} must load: {error}");
        }
    }
}

#[then("no fixture scene reports a warning")]
fn then_no_fixture_warns(#[from(rstest_bdd_harness_context)] session: &mut LoaderSession) {
    // Continuous integration runs strict, so a warning in a shipped fixture is
    // a warning every later phase inherits: these are the scenes phase 2
    // renders, phase 3 lights, and phase 4 paths across.
    for (name, outcome) in &session.fixtures {
        let Ok(loaded) = outcome else {
            continue;
        };
        assert!(
            !loaded.has_warnings(),
            "{name} must carry no warnings, got {:?}",
            loaded.warnings
        );
    }
}
