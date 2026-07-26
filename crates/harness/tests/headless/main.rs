//! Headless behavioural specification for the harness core, driven by
//! rstest-bdd 0.6.0-beta3 through the Bevy harness adapter in
//! [`support`]. Scenarios live in `tests/features/harness.feature`.
//!
//! Steps borrow the running `bevy::app::App` via the reserved
//! `rstest_bdd_harness_context` fixture; the adapter builds a
//! `MinimalPlugins` app with `HarnessCorePlugin` before steps run.

mod support;

use bevy::{
    app::App,
    ecs::message::Messages,
    input::{ButtonInput, keyboard::KeyCode},
};
use rstest_bdd_macros::{given, scenario, then, when};
use support::BevyHarness;
use thysalion_harness::{HarnessAction, HarnessConfig, RigState};
use thysalion_presentation::Quadrant;

fn send_action(app: &mut App, action: HarnessAction, count: usize) {
    for _ in 0..count {
        app.world_mut()
            .resource_mut::<Messages<HarnessAction>>()
            .write(action);
        app.update();
    }
}

#[given("a headless harness app in the north-east quadrant")]
fn app_in_north_east(#[from(rstest_bdd_harness_context)] app: &mut App) {
    app.update();
    let rig = app.world().resource::<RigState>();
    assert_eq!(
        rig.quadrant(),
        Quadrant::NorthEast,
        "unexpected start quadrant"
    );
}

#[when("a rotate-left action is sent and the app updates")]
fn send_rotate_left(#[from(rstest_bdd_harness_context)] app: &mut App) {
    send_action(app, HarnessAction::RotateLeft, 1);
}

#[when("the app updates")]
fn app_updates(#[from(rstest_bdd_harness_context)] app: &mut App) { app.update(); }

#[when("the rotate-right key is pressed and the app updates")]
fn press_rotate_right_key(#[from(rstest_bdd_harness_context)] app: &mut App) {
    app.world_mut()
        .resource_mut::<ButtonInput<KeyCode>>()
        .press(KeyCode::KeyE);
    app.update();
    app.world_mut()
        .resource_mut::<ButtonInput<KeyCode>>()
        .release(KeyCode::KeyE);
    app.update();
}

#[when("twenty zoom-in actions are sent and the app updates")]
fn send_twenty_zoom_ins(#[from(rstest_bdd_harness_context)] app: &mut App) {
    send_action(app, HarnessAction::ZoomIn, 20);
}

#[then("the camera rig is in the north-west quadrant")]
fn rig_in_north_west(#[from(rstest_bdd_harness_context)] app: &mut App) {
    assert_eq!(
        app.world().resource::<RigState>().quadrant(),
        Quadrant::NorthWest
    );
}

#[then("the camera rig is in the south-east quadrant")]
fn rig_in_south_east(#[from(rstest_bdd_harness_context)] app: &mut App) {
    assert_eq!(
        app.world().resource::<RigState>().quadrant(),
        Quadrant::SouthEast
    );
}

#[then("the harness diagnostic paths are registered")]
fn diagnostic_paths_registered(#[from(rstest_bdd_harness_context)] app: &mut App) {
    let store = app.world().resource::<bevy::diagnostic::DiagnosticsStore>();
    for path in [
        thysalion_harness::diagnostics::FRAME_TIME,
        thysalion_harness::diagnostics::FPS,
        thysalion_harness::diagnostics::TICK_TIME,
    ] {
        assert!(
            store.get(&path).is_some(),
            "diagnostic path {path:?} is not registered"
        );
    }
}

#[then("the camera rig zoom sits at the configured maximum")]
#[expect(
    clippy::float_arithmetic,
    reason = "epsilon comparison of the clamped zoom level"
)]
fn zoom_at_configured_maximum(#[from(rstest_bdd_harness_context)] app: &mut App) {
    let max = HarnessConfig::default().zoom_bounds.max();
    let zoom = app.world().resource::<RigState>().zoom();
    assert!(
        (zoom - max).abs() < f32::EPSILON,
        "zoom {zoom} should be clamped to the configured maximum {max}"
    );
}

#[scenario(path = "tests/features/harness.feature", index = 0, harness = BevyHarness)]
fn rotating_advances_one_quadrant() {}

#[scenario(path = "tests/features/harness.feature", index = 1, harness = BevyHarness)]
fn diagnostics_are_registered() {}

#[scenario(path = "tests/features/harness.feature", index = 2, harness = BevyHarness)]
fn synthetic_key_press_drives_the_rig() {}

#[scenario(path = "tests/features/harness.feature", index = 3, harness = BevyHarness)]
fn zoom_clamps_to_bounds() {}
