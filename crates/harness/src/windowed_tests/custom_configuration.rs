//! The windowed half must honour a demo's non-default configuration,
//! not silently fall back to `HarnessConfig::default()`.
//!
//! These tests reuse the module's headless seams: no window, graphics
//! device, filesystem write, or wall-clock dependency is involved.
//! Screenshot configuration is covered through the installed
//! configuration resource rather than by triggering a capture, because
//! capturing would touch the filesystem; the isolated scheduling tests
//! in `screenshot.rs` cover the capture path itself.

use thysalion_presentation::{Quadrant, ZoomBounds};

use super::*;

/// A zoom range sharing no limit with the 0.5–4.0 default, and
/// excluding the 1.0 baseline, so any fallback to the defaults is
/// visible in both the clamped zoom and the derived viewport height.
const CUSTOM_MIN_ZOOM: f32 = 2.0;
const CUSTOM_MAX_ZOOM: f32 = 3.0;
const CUSTOM_SLUG: &str = "demo-windowed-custom";
const CUSTOM_QUADRANT: Quadrant = Quadrant::SouthEast;

/// Bounded step count: enough 1.25× steps to saturate a 1.5-ratio
/// range from either end, so a rig that never clamps fails rather
/// than looping.
const ZOOM_STEPS_TO_SATURATE: usize = 8;

fn custom_bounds() -> ZoomBounds {
    match ZoomBounds::new(CUSTOM_MIN_ZOOM, CUSTOM_MAX_ZOOM) {
        Ok(bounds) => bounds,
        Err(error) => panic!("the custom zoom range must be valid: {error}"),
    }
}

fn custom_config() -> HarnessConfig {
    HarnessConfig::new(CUSTOM_SLUG)
        .with_zoom_bounds(custom_bounds())
        .with_initial_quadrant(CUSTOM_QUADRANT)
}

/// Reads the camera's fixed-vertical viewport height.
fn viewport_height(app: &mut App) -> f32 {
    let mut query = app
        .world_mut()
        .query_filtered::<&Projection, With<HarnessCamera>>();
    let projection = match query.single(app.world()) {
        Ok(projection) => projection,
        Err(error) => panic!("exactly one harness camera expected: {error}"),
    };
    let Projection::Orthographic(orthographic) = projection else {
        panic!("the harness camera must stay orthographic");
    };
    let ScalingMode::FixedVertical { viewport_height } = orthographic.scaling_mode else {
        panic!("the harness camera must use fixed-vertical scaling");
    };
    viewport_height
}

#[rstest]
fn the_plugin_installs_the_supplied_configuration() {
    let app = windowed_app_with(custom_config());
    let installed = app.world().resource::<HarnessConfig>();
    assert_eq!(
        installed.slug, CUSTOM_SLUG,
        "screenshot filenames derive from this slug, so it must survive the plugin"
    );
    assert_eq!(
        installed.zoom_bounds,
        custom_bounds(),
        "the windowed plugin must install the supplied zoom bounds"
    );
    assert_eq!(
        installed.initial_quadrant, CUSTOM_QUADRANT,
        "the windowed plugin must install the supplied initial quadrant"
    );
}

#[rstest]
#[expect(clippy::float_arithmetic, reason = "epsilon comparison of camera yaw")]
fn the_camera_starts_aimed_at_the_configured_quadrant() {
    let mut app = windowed_app_with(custom_config());
    let expected = CUSTOM_QUADRANT.yaw_radians();
    let yaw = camera_yaw(&mut app);
    assert!(
        (yaw - expected).abs() < f32::EPSILON,
        "the camera must spawn aimed at the configured quadrant ({CUSTOM_QUADRANT:?}, yaw \
         {expected}), got {yaw}"
    );
    assert_eq!(
        app.world().resource::<RigState>().quadrant(),
        CUSTOM_QUADRANT,
        "the rig must agree with the camera's starting quadrant"
    );
}

#[rstest]
#[expect(
    clippy::float_arithmetic,
    reason = "epsilon comparison of viewport heights"
)]
fn the_viewport_height_derives_from_the_configured_bounds() {
    let mut app = windowed_app_with(custom_config());
    let bounds = custom_bounds();

    // At startup the zoom is clamped up to the custom minimum, so the
    // viewport already differs from a default-bounds camera.
    let initial = viewport_height(&mut app);
    let expected_initial = bounds.viewport_height(bounds.clamp(1.0));
    assert!(
        (initial - expected_initial).abs() < f32::EPSILON,
        "the initial viewport height must derive from the configured bounds (expected \
         {expected_initial}), got {initial}"
    );
    let default_initial = ZoomBounds::default().viewport_height(1.0);
    assert!(
        (initial - default_initial).abs() > f32::EPSILON,
        "this range must produce a viewport height distinct from the default bounds, or the test \
         cannot detect a fallback"
    );

    // Saturating the zoom must land on the configured maximum's
    // viewport height, not the default maximum's.
    for _ in 0..ZOOM_STEPS_TO_SATURATE {
        send(&mut app, HarnessAction::ZoomIn);
        app.update();
    }
    let saturated = viewport_height(&mut app);
    let expected_saturated = bounds.viewport_height(CUSTOM_MAX_ZOOM);
    assert!(
        (saturated - expected_saturated).abs() < f32::EPSILON,
        "the saturated viewport height must derive from the configured maximum (expected \
         {expected_saturated}), got {saturated}"
    );
}
