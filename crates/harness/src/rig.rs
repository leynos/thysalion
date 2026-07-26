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
    ///
    /// The baseline zoom of `1.0` is clamped into the configured bounds,
    /// so a range that excludes `1.0` still starts inside its bounds.
    #[must_use]
    pub const fn from_config(config: &HarnessConfig) -> Self {
        Self {
            quadrant: config.initial_quadrant,
            zoom: config.zoom_bounds.clamp(1.0),
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

#[cfg(test)]
mod tests {
    //! Model-based property coverage for the rig: a generated action
    //! sequence applied through the real message-driven system must match
    //! a simple reference model (a four-state quadrant counter and a
    //! clamped multiplicative zoom fold), and the zoom must stay inside
    //! the configured bounds after every single action.

    use bevy::{app::App, ecs::message::Messages, prelude::Update};
    use proptest::prelude::*;
    use thysalion_presentation::ZoomBounds;

    use super::*;

    /// Reference model for one zoom step; mirrors the contract, not the
    /// implementation's storage.
    #[expect(
        clippy::float_arithmetic,
        reason = "the zoom model is inherently floating point"
    )]
    fn model_zoom(zoom: f32, action: HarnessAction, bounds: ZoomBounds) -> f32 {
        match action {
            HarnessAction::ZoomIn => bounds.clamp(zoom * ZOOM_STEP),
            HarnessAction::ZoomOut => bounds.clamp(zoom / ZOOM_STEP),
            _ => zoom,
        }
    }

    /// Reference model for one quadrant step.
    fn model_quadrant(quadrant: Quadrant, action: HarnessAction) -> Quadrant {
        match action {
            HarnessAction::RotateLeft => quadrant.prev(),
            HarnessAction::RotateRight => quadrant.next(),
            _ => quadrant,
        }
    }

    /// Exact-equality seam: model and system perform the identical float
    /// operations in the identical order, so results must be bitwise
    /// equal; comparing with `to_bits` avoids a float-comparison lint
    /// exception.
    fn same_zoom(actual: f32, expected: f32) -> bool { actual.to_bits() == expected.to_bits() }

    fn action_strategy() -> impl Strategy<Value = HarnessAction> {
        prop_oneof![
            Just(HarnessAction::RotateLeft),
            Just(HarnessAction::RotateRight),
            Just(HarnessAction::ZoomIn),
            Just(HarnessAction::ZoomOut),
            Just(HarnessAction::ToggleOverlay),
            Just(HarnessAction::Screenshot),
        ]
    }

    /// Builds a bare app hosting only the rig system and its inputs.
    fn rig_app(config: HarnessConfig) -> App {
        let mut app = App::new();
        app.add_message::<HarnessAction>()
            .insert_resource(RigState::from_config(&config))
            .insert_resource(config)
            .add_systems(Update, apply_actions);
        app
    }

    proptest! {
        #[test]
        fn rig_state_matches_the_reference_model(
            actions in proptest::collection::vec(action_strategy(), 0..32),
        ) {
            let config = HarnessConfig::default();
            let bounds = config.zoom_bounds;
            let mut app = rig_app(config);
            let mut expected_quadrant = Quadrant::default();
            let mut expected_zoom = bounds.clamp(1.0);
            for action in actions {
                app.world_mut()
                    .resource_mut::<Messages<HarnessAction>>()
                    .write(action);
                app.update();
                expected_quadrant = model_quadrant(expected_quadrant, action);
                expected_zoom = model_zoom(expected_zoom, action, bounds);
                let rig = app.world().resource::<RigState>();
                prop_assert_eq!(
                    rig.quadrant(),
                    expected_quadrant,
                    "quadrant diverged from the model after {:?}",
                    action
                );
                prop_assert!(
                    same_zoom(rig.zoom(), expected_zoom),
                    "zoom {} diverged from the model {} after {:?}",
                    rig.zoom(),
                    expected_zoom,
                    action
                );
                prop_assert!(
                    rig.zoom() >= bounds.min() && rig.zoom() <= bounds.max(),
                    "zoom {} escaped the configured bounds",
                    rig.zoom()
                );
            }
        }
    }
}
