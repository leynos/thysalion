//! Headless camera-rig state: the active quadrant and zoom level, and the
//! system that applies [`HarnessAction`] messages to them. Rendering-side
//! systems (`camera` module) read this state; nothing here touches render
//! types, so `MinimalPlugins` apps exercise it in CI.

use bevy::{
    ecs::message::MessageReader,
    prelude::{Res, ResMut, Resource},
};
use thysalion_presentation::Quadrant;

use crate::{config::HarnessConfig, input::HarnessAction};

/// Multiplicative zoom step per `ZoomIn`/`ZoomOut` action.
const ZOOM_STEP: f32 = 1.25;

/// The camera rig's logical state: active quadrant and zoom level.
///
/// Fields are private so every mutation flows through the message-driven
/// action-application system and respects the configured bounds.
#[derive(Resource, Debug, Clone, Copy)]
pub struct RigState {
    quadrant: Quadrant,
    zoom: f32,
}

impl RigState {
    /// Creates the rig state a demo starts with.
    #[must_use]
    pub const fn from_config(config: &HarnessConfig) -> Self {
        Self {
            quadrant: config.initial_quadrant,
            zoom: 1.0,
        }
    }

    /// Returns the active yaw quadrant.
    #[must_use]
    pub const fn quadrant(&self) -> Quadrant { self.quadrant }

    /// Returns the current zoom level.
    #[must_use]
    pub const fn zoom(&self) -> f32 { self.zoom }
}

/// Applies buffered [`HarnessAction`] messages to the rig state.
///
/// Rotation follows the camera contract's cyclic order: `RotateLeft` is
/// [`Quadrant::prev`], `RotateRight` is [`Quadrant::next`]. Zoom steps
/// multiply by [`ZOOM_STEP`] and clamp to the configured bounds.
#[expect(
    clippy::needless_pass_by_value,
    reason = "Bevy system parameters are taken by value"
)]
#[expect(
    clippy::float_arithmetic,
    reason = "zoom stepping is inherently floating point"
)]
pub(crate) fn apply_actions(
    mut reader: MessageReader<HarnessAction>,
    mut rig: ResMut<RigState>,
    config: Res<HarnessConfig>,
) {
    for action in reader.read() {
        match action {
            HarnessAction::RotateLeft => rig.quadrant = rig.quadrant.prev(),
            HarnessAction::RotateRight => rig.quadrant = rig.quadrant.next(),
            HarnessAction::ZoomIn => {
                rig.zoom = config.zoom_bounds.clamp(rig.zoom * ZOOM_STEP);
            }
            HarnessAction::ZoomOut => {
                rig.zoom = config.zoom_bounds.clamp(rig.zoom / ZOOM_STEP);
            }
            // Overlay and screenshot actions are handled by the windowed
            // systems; the rig ignores them.
            _ => {}
        }
    }
}
