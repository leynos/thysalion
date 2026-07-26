//! Behavioural tests for the windowed harness half, run without a real
//! operating-system window or graphics device: `DemoHarnessPlugin` is
//! exercised under `MinimalPlugins`, where the camera and UI text are
//! plain entity data that no renderer consumes.
//!
//! Isolation seams: no screenshot actions are queued here, so the
//! screenshot system returns before touching the filesystem; the overlay
//! refresh throttle is driven through `OverlayTimer::advance_to_brink`
//! instead of waiting out the real interval; and smoothing assertions are
//! monotonicity invariants of the exponential settle, so they hold for
//! any real frame delta.

use std::time::Duration;

use bevy::{
    app::App,
    camera::ScalingMode,
    diagnostic::{DiagnosticMeasurement, DiagnosticsStore},
    ecs::message::Messages,
    prelude::*,
};
use rstest::rstest;

use crate::{
    DemoHarnessPlugin,
    HarnessConfig,
    camera::{CameraYaw, HarnessCamera, shortest_angle_delta},
    diagnostics::TICK_TIME,
    input::HarnessAction,
    overlay::{OverlayText, OverlayTimer},
    rig::RigState,
};

mod custom_configuration;

/// Builds a headless app running the full windowed plugin from the
/// supplied configuration, and runs startup.
///
/// Taking the configuration as an argument (rather than hard-coding the
/// default) lets the custom-configuration tests below exercise the same
/// startup path a demo with non-default settings would get.
fn windowed_app_with(config: HarnessConfig) -> App {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins)
        .add_plugins(DemoHarnessPlugin::new(config));
    app.update();
    app
}

/// Builds a headless windowed app with the default configuration.
fn windowed_app() -> App { windowed_app_with(HarnessConfig::default()) }

fn send(app: &mut App, action: HarnessAction) {
    app.world_mut()
        .resource_mut::<Messages<HarnessAction>>()
        .write(action);
}

fn camera_yaw(app: &mut App) -> f32 {
    let mut query = app.world_mut().query::<&CameraYaw>();
    match query.single(app.world()) {
        Ok(yaw) => yaw.0,
        Err(error) => panic!("exactly one harness camera expected: {error}"),
    }
}

fn overlay_visibility(app: &mut App) -> Visibility {
    let mut query = app
        .world_mut()
        .query_filtered::<&Visibility, With<OverlayText>>();
    match query.single(app.world()) {
        Ok(visibility) => *visibility,
        Err(error) => panic!("exactly one overlay entity expected: {error}"),
    }
}

fn overlay_text(app: &mut App) -> String {
    let mut query = app.world_mut().query_filtered::<&Text, With<OverlayText>>();
    match query.single(app.world()) {
        Ok(text) => text.0.clone(),
        Err(error) => panic!("exactly one overlay entity expected: {error}"),
    }
}

/// Absolute angular gap between a yaw and its target.
fn gap_to(app: &mut App, target: f32) -> f32 { shortest_angle_delta(camera_yaw(app), target).abs() }

#[expect(
    clippy::float_arithmetic,
    reason = "epsilon comparison of viewport heights"
)]
fn close(actual: f32, expected: f32) -> bool { (actual - expected).abs() < f32::EPSILON * 8.0 }

#[rstest]
fn startup_spawns_one_camera_and_one_overlay() {
    let mut app = windowed_app();
    let cameras = app
        .world_mut()
        .query_filtered::<(), With<HarnessCamera>>()
        .iter(app.world())
        .count();
    let overlays = app
        .world_mut()
        .query_filtered::<(), With<OverlayText>>()
        .iter(app.world())
        .count();
    assert_eq!(cameras, 1, "startup must spawn exactly one harness camera");
    assert_eq!(overlays, 1, "startup must spawn exactly one overlay entity");
}

#[rstest]
fn projection_tracks_the_rig_zoom() {
    let mut app = windowed_app();
    send(&mut app, HarnessAction::ZoomIn);
    app.update();
    let (zoom, expected_height) = {
        let rig = app.world().resource::<RigState>();
        let config = app.world().resource::<HarnessConfig>();
        (rig.zoom(), config.zoom_bounds.viewport_height(rig.zoom()))
    };
    assert!(zoom > 1.0, "a zoom-in action must raise the rig zoom");
    let mut query = app
        .world_mut()
        .query_filtered::<&Projection, With<HarnessCamera>>();
    let projection = query.single(app.world()).expect("one harness camera");
    let Projection::Orthographic(orthographic) = projection else {
        panic!("the harness camera must stay orthographic");
    };
    let ScalingMode::FixedVertical { viewport_height } = orthographic.scaling_mode else {
        panic!("the harness camera must use fixed-vertical scaling");
    };
    assert!(
        close(viewport_height, expected_height),
        "viewport height {viewport_height} must track the rig zoom (expected {expected_height})"
    );
}

#[rstest]
#[expect(
    clippy::float_arithmetic,
    reason = "settling-progress arithmetic over angular gaps"
)]
fn quadrant_turns_settle_monotonically_toward_the_target() {
    let mut app = windowed_app();
    let start = camera_yaw(&mut app);
    send(&mut app, HarnessAction::RotateRight);
    app.update();
    let target = app.world().resource::<RigState>().quadrant().yaw_radians();
    let initial_gap = shortest_angle_delta(start, target).abs();
    assert!(
        initial_gap > 0.1,
        "a quarter turn leaves a real gap to close"
    );
    let mut previous_gap = gap_to(&mut app, target);
    assert!(
        previous_gap <= initial_gap + f32::EPSILON * 4.0,
        "the yaw must never move away from the target"
    );
    for _ in 0..300 {
        std::thread::sleep(Duration::from_millis(2));
        app.update();
        let gap = gap_to(&mut app, target);
        assert!(
            gap <= previous_gap + f32::EPSILON * 4.0,
            "the yaw must approach the target monotonically ({gap} after {previous_gap})"
        );
        previous_gap = gap;
        if gap < initial_gap * 0.25 {
            break;
        }
    }
    assert!(
        previous_gap < initial_gap * 0.5,
        "the yaw must settle toward the target rather than stall (still {previous_gap} of \
         {initial_gap} away)"
    );
}

#[rstest]
fn overlay_toggles_on_odd_batches_and_ignores_even_batches() {
    let mut app = windowed_app();
    assert_eq!(
        overlay_visibility(&mut app),
        Visibility::default(),
        "the overlay starts visible"
    );
    send(&mut app, HarnessAction::ToggleOverlay);
    app.update();
    assert_eq!(
        overlay_visibility(&mut app),
        Visibility::Hidden,
        "one toggle hides the overlay"
    );
    send(&mut app, HarnessAction::ToggleOverlay);
    send(&mut app, HarnessAction::ToggleOverlay);
    app.update();
    assert_eq!(
        overlay_visibility(&mut app),
        Visibility::Hidden,
        "an even batch of toggles in one frame cancels out"
    );
    send(&mut app, HarnessAction::ToggleOverlay);
    app.update();
    assert_eq!(
        overlay_visibility(&mut app),
        Visibility::Inherited,
        "the next single toggle shows the overlay again"
    );
}

/// Forces the throttle to the brink and updates until the overlay text
/// changes from its value at entry; bounded so a broken refresh still
/// fails loudly (via the caller's assertion) rather than hanging.
fn refresh_overlay_now(app: &mut App) {
    let before = overlay_text(app);
    for _ in 0..100 {
        app.world_mut()
            .resource_mut::<OverlayTimer>()
            .advance_to_brink();
        std::thread::sleep(Duration::from_millis(1));
        app.update();
        if overlay_text(app) != before {
            return;
        }
    }
}

#[rstest]
fn overlay_refresh_is_throttled_until_the_interval_elapses() {
    let mut app = windowed_app();
    // Two immediate updates take microseconds — far below the 0.2 s
    // throttle — so the placeholder text must survive them.
    app.update();
    assert_eq!(
        overlay_text(&mut app),
        "diagnostics…",
        "the overlay must not refresh before the throttle interval"
    );
    refresh_overlay_now(&mut app);
    assert_ne!(
        overlay_text(&mut app),
        "diagnostics…",
        "the overlay must refresh once the interval elapses"
    );
}

#[rstest]
fn overlay_renders_missing_and_populated_tick_diagnostics() {
    let mut app = windowed_app();
    refresh_overlay_now(&mut app);
    assert!(
        overlay_text(&mut app).ends_with("tick: n/a"),
        "without a tick measurement the readout must say so, got {:?}",
        overlay_text(&mut app)
    );
    app.world_mut()
        .resource_mut::<DiagnosticsStore>()
        .get_mut(&TICK_TIME)
        .expect("the harness registers the tick-time diagnostic")
        .add_measurement(DiagnosticMeasurement {
            time: bevy::platform::time::Instant::now(),
            value: 2.5,
        });
    refresh_overlay_now(&mut app);
    assert!(
        overlay_text(&mut app).ends_with("2.50 ms/tick"),
        "a tick measurement must appear in the readout, got {:?}",
        overlay_text(&mut app)
    );
}
