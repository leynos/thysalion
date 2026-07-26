//! Configuration-propagation tests for [`HarnessCorePlugin`].
//!
//! These go through the real plugin (`HarnessCorePlugin::new(config)`)
//! rather than inserting resources directly, so they exercise the wiring a
//! demo actually gets: the config resource the plugin installs, the
//! `RigState` it derives from that config, and the `Update` systems it
//! schedules. A non-default configuration is used throughout, so a
//! regression that silently fell back to `HarnessConfig::default()` or
//! `ZoomBounds::default()` fails here rather than passing by coincidence.

use bevy::{app::App, ecs::message::Messages, prelude::*};
use rstest::{fixture, rstest};
use thysalion_presentation::{Quadrant, ZoomBounds};

use crate::{HarnessConfig, HarnessCorePlugin, input::HarnessAction, rig::RigState};

/// Custom zoom range, chosen to overlap the default range nowhere at its
/// limits: the default is 0.5–4.0, so a rig clamping at 2.0/3.0 cannot be
/// mistaken for one using the defaults. It also excludes the baseline
/// zoom of 1.0, which pins the clamping in `RigState::from_config`.
const CUSTOM_MIN_ZOOM: f32 = 2.0;
const CUSTOM_MAX_ZOOM: f32 = 3.0;

/// Non-default slug, distinct from `HarnessConfig::default()`'s.
const CUSTOM_SLUG: &str = "demo-custom";

/// Non-default starting quadrant (the default is `NorthEast`).
const CUSTOM_QUADRANT: Quadrant = Quadrant::SouthWest;

/// Enough zoom steps to reach either bound from anywhere inside the
/// custom range: each step multiplies by 1.25, and 1.25^4 exceeds the
/// range's 1.5 ratio comfortably. Bounded, so a rig that never clamps
/// fails the assertion instead of looping forever.
const ZOOM_STEPS_TO_SATURATE: usize = 8;

#[fixture]
fn custom_bounds() -> ZoomBounds {
    match ZoomBounds::new(CUSTOM_MIN_ZOOM, CUSTOM_MAX_ZOOM) {
        Ok(bounds) => bounds,
        Err(error) => panic!("the custom zoom range must be valid: {error}"),
    }
}

#[fixture]
fn custom_config(custom_bounds: ZoomBounds) -> HarnessConfig {
    HarnessConfig::new(CUSTOM_SLUG)
        .with_zoom_bounds(custom_bounds)
        .with_initial_quadrant(CUSTOM_QUADRANT)
}

/// Builds a headless app from the supplied configuration through the real
/// core plugin, and runs one update so startup wiring settles.
fn core_app(config: HarnessConfig) -> App {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins)
        .add_plugins(HarnessCorePlugin::new(config));
    app.update();
    app
}

/// Sends one action and advances a frame, so the action flows through the
/// plugin's own scheduled systems rather than being applied directly.
fn send_and_update(app: &mut App, action: HarnessAction) {
    app.world_mut()
        .resource_mut::<Messages<HarnessAction>>()
        .write(action);
    app.update();
}

fn rig_zoom(app: &App) -> f32 { app.world().resource::<RigState>().zoom() }

/// Asserts a zoom level equals its expected bound within one epsilon.
///
/// The comparison lives here rather than inline because `rstest` moves a
/// `#[case]`-parameterized body into generated case functions, where a
/// function-level `#[expect]` on the test no longer covers it.
#[expect(
    clippy::float_arithmetic,
    reason = "epsilon comparison of the clamped zoom level"
)]
fn assert_zoom_at(zoom: f32, expected: f32, bound: &str, default_bound: f32) {
    assert!(
        (zoom - expected).abs() < f32::EPSILON,
        "zoom must clamp at the configured {bound} {expected}, got {zoom} (the default {bound} is \
         {default_bound})"
    );
}

#[rstest]
fn the_plugin_installs_the_supplied_configuration(custom_config: HarnessConfig) {
    let app = core_app(custom_config);
    let installed = app.world().resource::<HarnessConfig>();
    assert_eq!(
        installed.slug, CUSTOM_SLUG,
        "the plugin must install the supplied slug"
    );
    assert_eq!(
        installed.window_title,
        format!("Thysalion — {CUSTOM_SLUG}"),
        "the window title must stay derived from the supplied slug"
    );
    assert_eq!(
        installed.initial_quadrant, CUSTOM_QUADRANT,
        "the plugin must install the supplied initial quadrant"
    );
    assert_eq!(
        installed.zoom_bounds,
        custom_bounds(),
        "the plugin must install the supplied zoom bounds"
    );
}

#[rstest]
fn the_rig_starts_in_the_configured_quadrant(custom_config: HarnessConfig) {
    let app = core_app(custom_config);
    assert_eq!(
        app.world().resource::<RigState>().quadrant(),
        CUSTOM_QUADRANT,
        "the rig must start in the configured quadrant, not the default"
    );
}

#[rstest]
#[expect(
    clippy::float_arithmetic,
    reason = "epsilon comparison of the clamped zoom level"
)]
fn the_initial_zoom_is_clamped_into_the_configured_range(custom_config: HarnessConfig) {
    let app = core_app(custom_config);
    // The baseline zoom is 1.0, which this range excludes, so a rig that
    // ignored the configured bounds would report 1.0 here.
    assert!(
        (rig_zoom(&app) - CUSTOM_MIN_ZOOM).abs() < f32::EPSILON,
        "the initial zoom must clamp up to the configured minimum, got {}",
        rig_zoom(&app)
    );
}

/// Saturating the zoom in either direction must settle on the
/// *configured* limit. The default limit for the same direction is passed
/// in only so the diagnostic can name it: this range shares neither limit
/// with the default, so a fallback to `ZoomBounds::default()` shows up as
/// the default value rather than as a near miss.
#[rstest]
#[case(HarnessAction::ZoomIn, CUSTOM_MAX_ZOOM, "maximum", 4.0)]
#[case(HarnessAction::ZoomOut, CUSTOM_MIN_ZOOM, "minimum", 0.5)]
fn zooming_clamps_at_the_configured_bound(
    #[case] action: HarnessAction,
    #[case] expected: f32,
    #[case] bound: &str,
    #[case] default_bound: f32,
) {
    // The configuration is built directly rather than injected as a
    // fixture: with four `#[case]` parameters, a fifth argument would
    // exceed the workspace's `too_many_arguments` limit.
    let mut app = core_app(custom_config(custom_bounds()));
    for _ in 0..ZOOM_STEPS_TO_SATURATE {
        send_and_update(&mut app, action);
    }
    assert_zoom_at(rig_zoom(&app), expected, bound, default_bound);
}

#[rstest]
#[expect(
    clippy::float_arithmetic,
    reason = "epsilon comparison of the clamped zoom level"
)]
fn zoom_moves_within_the_configured_range_before_clamping(custom_config: HarnessConfig) {
    let mut app = core_app(custom_config);
    let start = rig_zoom(&app);
    send_and_update(&mut app, HarnessAction::ZoomIn);
    let stepped = rig_zoom(&app);
    assert!(
        stepped > start,
        "one zoom-in step must raise the zoom from {start}, got {stepped}"
    );
    assert!(
        stepped <= CUSTOM_MAX_ZOOM + f32::EPSILON,
        "zoom must never exceed the configured maximum, got {stepped}"
    );
}
