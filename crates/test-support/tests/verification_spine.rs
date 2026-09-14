//! The headless verification spine: roadmap task 1.3.1's three clauses in one
//! behavioural scenario.
//!
//! The task names three things in one breath — `MinimalPlugins` app
//! construction, fixture-scene loading, and diagnostics counters exposed for
//! assertions — and before this suite existed no test combined them. The
//! harness suite built an app and never loaded a scene; the loading suite
//! loaded scenes and never built an app.
//!
//! The scenario lives here rather than in `crates/harness` because it tests
//! the *promoted product*: that `BevyHarness` and the loader compose. Hosting
//! it in the harness crate would re-create exactly the cross-crate reach that
//! promoting the adapters removed.
//!
//! The suite requires the crate's non-default `bevy` feature, so it compiles
//! to nothing on the default-feature path `thysalion-world` consumes. Run it
//! with `cargo nextest run -p thysalion-test-support --all-features`.

#![cfg(feature = "bevy")]

use std::sync::Arc;

use bevy::{app::App, diagnostic::DiagnosticsStore, ecs::resource::Resource};
use camino::Utf8PathBuf;
use rstest_bdd_macros::{given, scenario, then, when};
use thysalion_test_support::{BevyHarness, scenes};
use thysalion_world::{loader::SceneLoader, source::DirSceneSource};

/// The loaded fixture, parked in the app's world for the `Then` steps.
///
/// A newtype defined in this test rather than in any library crate,
/// deliberately: `thysalion-world` has no Bevy dependency until ADR 005 stages
/// one in at roadmap 2.1.1, and a `Resource` wrapper shipped from a library to
/// satisfy a test would be that dependency arriving through the back door.
#[derive(Resource)]
struct LoadedSceneResource(thysalion_world::loader::LoadedScene);

#[given("a headless harness app")]
fn given_headless_app(#[from(rstest_bdd_harness_context)] app: &mut App) {
    // One update before the scene arrives, so the `Then` steps assert against
    // a schedule that has actually run rather than against plugin build-time
    // state. `HarnessCorePlugin` registers its diagnostic paths at build time,
    // but a diagnostic nobody has ever measured is exactly the kind of thing a
    // later refactor can move into a startup system without noticing.
    app.update();
}

#[when("the bare-cell fixture scene is loaded into the app")]
fn when_fixture_loaded(#[from(rstest_bdd_harness_context)] app: &mut App) {
    let loader = SceneLoader::new(Arc::new(DirSceneSource::new(
        scenes::scene_dir(),
        scenes::SCENES,
    )));
    let path = Utf8PathBuf::from("bare-cell.scene.json");
    match loader.load(&path) {
        Ok(loaded) => {
            app.insert_resource(LoadedSceneResource(loaded));
        }
        Err(error) => panic!("the bare-cell fixture must load: {error}"),
    }
    app.update();
}

#[then("the scene holds 2 palette entries")]
fn then_two_palette_entries(#[from(rstest_bdd_harness_context)] app: &mut App) {
    let loaded = app.world().resource::<LoadedSceneResource>();
    assert_eq!(
        loaded.0.scene.palette().len(),
        2,
        "bare-cell declares air and stone-block"
    );
}

#[then("the frame time diagnostic is registered")]
fn then_frame_time_registered(#[from(rstest_bdd_harness_context)] app: &mut App) {
    let store = app.world().resource::<DiagnosticsStore>();
    let path = thysalion_harness::diagnostics::FRAME_TIME;
    assert!(
        store.get(&path).is_some(),
        "diagnostic path {path:?} is not registered"
    );
}

#[scenario(
    path = "tests/features/verification_spine.feature",
    index = 0,
    harness = BevyHarness
)]
fn a_fixture_scene_loads_inside_a_headless_harness_app() {}
