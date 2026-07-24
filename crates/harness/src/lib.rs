//! Thysalion demo harness (crate `thysalion-harness`).
//!
//! Shared scaffolding consumed by every capability demonstration binary in
//! `thysalion-demos`: window setup, the isometric camera rig, input
//! mapping, the diagnostics overlay, and screenshot capture. This crate is
//! demo tooling, not a plane: the shipping presentation plane must never
//! depend on it, while it consumes the presentation plane's camera
//! contract (`thysalion-presentation`).
//!
//! The harness is a **two-plugin contract**:
//!
//! - [`HarnessCorePlugin`] is headless-safe: rig state, input mapping, and diagnostics
//!   registration. `MinimalPlugins` apps (tests, and roadmap step 1.3's continuous-integration
//!   scaffolding) run it without a window or graphics processing unit.
//! - [`DemoHarnessPlugin`] adds the windowed half — camera entity, diagnostics overlay, screenshot
//!   capture — on top of the core.
//!
//! Stability promise: adding a harness capability must never require
//! editing existing demos. [`HarnessConfig`] is `#[non_exhaustive]` with
//! builder construction, and [`HarnessAction`] is `#[non_exhaustive]`, so
//! both grow additively.
#![cfg_attr(coverage_nightly, feature(coverage_attribute))]

use bevy::{
    app::{App, Plugin, Startup, Update},
    diagnostic::{
        Diagnostic,
        DiagnosticsPlugin,
        FrameTimeDiagnosticsPlugin,
        RegisterDiagnostic as _,
    },
    ecs::{
        change_detection::DetectChangesMut as _,
        schedule::{IntoScheduleConfigs as _, SystemSet},
        system::ResMut,
    },
    input::{ButtonInput, keyboard::KeyCode, mouse::AccumulatedMouseScroll},
};

pub mod config;
pub mod diagnostics;
pub mod input;
pub mod rig;

#[cfg_attr(coverage_nightly, coverage(off))]
mod camera;
#[cfg_attr(coverage_nightly, coverage(off))]
mod overlay;
#[cfg_attr(coverage_nightly, coverage(off))]
mod screenshot;

pub use camera::HarnessCamera;
pub use config::HarnessConfig;
pub use input::HarnessAction;
pub use rig::RigState;

/// Scheduling contract for harness systems in the `Update` schedule.
///
/// Demos and later phases order their own systems relative to these sets
/// instead of naming harness system functions.
#[derive(SystemSet, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum HarnessSet {
    /// Headless-safe core: input mapping, then rig-state application.
    Core,
    /// Windowed chrome: camera sync, overlay, screenshots.
    Windowed,
}

/// Headless-safe harness scaffolding: camera-rig state, input mapping,
/// and diagnostics registration.
///
/// # Examples
///
/// ```no_run
/// use bevy::prelude::*;
/// use thysalion_harness::{HarnessConfig, HarnessCorePlugin};
///
/// App::new()
///     .add_plugins(MinimalPlugins)
///     .add_plugins(HarnessCorePlugin::new(HarnessConfig::new("demo-empty")))
///     .run();
/// ```
#[derive(Debug, Clone, Default)]
pub struct HarnessCorePlugin {
    config: HarnessConfig,
}

impl HarnessCorePlugin {
    /// Creates the core plugin for a demo's configuration.
    #[must_use]
    pub const fn new(config: HarnessConfig) -> Self { Self { config } }
}

impl Plugin for HarnessCorePlugin {
    fn build(&self, app: &mut App) {
        if !app.is_plugin_added::<DiagnosticsPlugin>() {
            app.add_plugins(DiagnosticsPlugin);
        }
        if !app.is_plugin_added::<FrameTimeDiagnosticsPlugin>() {
            app.add_plugins(FrameTimeDiagnosticsPlugin::default());
        }
        app.register_diagnostic(Diagnostic::new(diagnostics::TICK_TIME).with_suffix("ms"))
            .insert_resource(self.config.clone())
            .insert_resource(rig::RigState::from_config(&self.config))
            .add_message::<HarnessAction>()
            // MinimalPlugins provides no input plugin; initializing the
            // input resources keeps the core headless-safe and lets tests
            // inject synthetic key presses.
            .init_resource::<ButtonInput<KeyCode>>()
            .init_resource::<AccumulatedMouseScroll>()
            .configure_sets(Update, HarnessSet::Windowed.after(HarnessSet::Core))
            .add_systems(
                Update,
                (input::read_input, rig::apply_actions)
                    .chain()
                    .in_set(HarnessSet::Core),
            );
        if !app.is_plugin_added::<bevy::input::InputPlugin>() {
            // Without `InputPlugin`, nothing expires `just_pressed` and
            // `just_released` between frames, so a synthetic key press
            // injected by a headless test would repeat every update.
            // Clearing at end of frame gives synthetic input the same
            // one-shot semantics as real input.
            app.add_systems(bevy::app::Last, clear_synthetic_input);
        }
    }
}

/// Expires synthetic input state at end of frame in headless apps.
fn clear_synthetic_input(
    mut keys: ResMut<ButtonInput<KeyCode>>,
    mut scroll: ResMut<AccumulatedMouseScroll>,
) {
    keys.bypass_change_detection().clear();
    scroll.bypass_change_detection().delta = bevy::math::Vec2::ZERO;
}

/// Windowed harness scaffolding: [`HarnessCorePlugin`] plus the camera
/// entity, the diagnostics overlay, and screenshot capture.
///
/// # Examples
///
/// ```no_run
/// use bevy::prelude::*;
/// use thysalion_harness::{DemoHarnessPlugin, HarnessConfig};
///
/// App::new()
///     .add_plugins(DefaultPlugins)
///     .add_plugins(DemoHarnessPlugin::new(HarnessConfig::new("demo-empty")))
///     .run();
/// ```
#[derive(Debug, Clone, Default)]
pub struct DemoHarnessPlugin {
    config: HarnessConfig,
}

impl DemoHarnessPlugin {
    /// Creates the windowed plugin for a demo's configuration.
    #[must_use]
    pub const fn new(config: HarnessConfig) -> Self { Self { config } }
}

impl Plugin for DemoHarnessPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(HarnessCorePlugin::new(self.config.clone()))
            .init_resource::<overlay::OverlayTimer>()
            .add_systems(Startup, (camera::spawn_camera, overlay::setup_overlay))
            .add_systems(
                Update,
                (
                    camera::sync_camera,
                    overlay::toggle_overlay,
                    overlay::refresh_overlay,
                    screenshot::trigger_screenshots,
                )
                    .in_set(HarnessSet::Windowed),
            );
    }
}
