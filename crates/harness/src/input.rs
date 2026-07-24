//! Input mapping: fixed key bindings translated into the buffered
//! [`HarnessAction`] message stream. The binding table below is the single
//! source of truth for harness chrome bindings; the developers' and users'
//! guides reference it rather than duplicating it.
//!
//! Boundary: the harness owns *chrome* bindings (rotate, zoom, overlay,
//! screenshot); demos own their gameplay input and must not rebind these.

use bevy::{
    ecs::message::{Message, MessageWriter},
    input::{ButtonInput, keyboard::KeyCode, mouse::AccumulatedMouseScroll},
    prelude::Res,
};

/// Buffered per-frame harness action stream (a Bevy message, not an
/// observer event: multiple systems read it each frame).
///
/// Non-exhaustive: later phases add chrome actions (time scrub, replay
/// pause, probe visualization) without breaking demo `match` expressions.
#[derive(Message, Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum HarnessAction {
    /// Rotate the camera one quadrant anticlockwise ([`Quadrant::prev`]).
    ///
    /// [`Quadrant::prev`]: thysalion_presentation::Quadrant::prev
    RotateLeft,
    /// Rotate the camera one quadrant clockwise ([`Quadrant::next`]).
    ///
    /// [`Quadrant::next`]: thysalion_presentation::Quadrant::next
    RotateRight,
    /// Zoom in one step (bounded by the configured [`ZoomBounds`]).
    ///
    /// [`ZoomBounds`]: thysalion_presentation::ZoomBounds
    ZoomIn,
    /// Zoom out one step (bounded by the configured [`ZoomBounds`]).
    ///
    /// [`ZoomBounds`]: thysalion_presentation::ZoomBounds
    ZoomOut,
    /// Show or hide the diagnostics overlay.
    ToggleOverlay,
    /// Capture a screenshot of the primary window.
    Screenshot,
}

/// Chrome key bindings: pressing the key emits the paired action.
///
/// The screenshot key is deliberately absent: it triggers on *release*
/// (see [`SCREENSHOT_KEY`]) so the captured frame does not show the key
/// still held during any capture flash.
pub const KEY_BINDINGS: &[(KeyCode, HarnessAction)] = &[
    (KeyCode::KeyQ, HarnessAction::RotateLeft),
    (KeyCode::KeyE, HarnessAction::RotateRight),
    (KeyCode::Equal, HarnessAction::ZoomIn),
    (KeyCode::Minus, HarnessAction::ZoomOut),
    (KeyCode::F3, HarnessAction::ToggleOverlay),
];

/// Screenshot trigger key; emits [`HarnessAction::Screenshot`] on release.
pub const SCREENSHOT_KEY: KeyCode = KeyCode::F12;

/// Returns the action bound to a pressed key, if any.
///
/// # Examples
///
/// ```
/// use bevy::input::keyboard::KeyCode;
/// use thysalion_harness::input::{HarnessAction, action_for_key};
///
/// assert_eq!(
///     action_for_key(KeyCode::KeyQ),
///     Some(HarnessAction::RotateLeft)
/// );
/// assert_eq!(action_for_key(KeyCode::KeyZ), None);
/// ```
#[must_use]
pub fn action_for_key(key: KeyCode) -> Option<HarnessAction> {
    KEY_BINDINGS
        .iter()
        .find(|(bound, _)| *bound == key)
        .map(|(_, action)| *action)
}

/// Reads raw key and scroll input and writes [`HarnessAction`] messages.
///
/// Headless-safe: `HarnessCorePlugin` initializes the input resources it
/// reads, so `MinimalPlugins` apps can inject synthetic presses.
#[expect(
    clippy::needless_pass_by_value,
    reason = "Bevy system parameters are taken by value"
)]
pub(crate) fn read_input(
    keys: Res<ButtonInput<KeyCode>>,
    scroll: Res<AccumulatedMouseScroll>,
    mut writer: MessageWriter<HarnessAction>,
) {
    for key in keys.get_just_pressed() {
        if let Some(action) = action_for_key(*key) {
            writer.write(action);
        }
    }
    if keys.just_released(SCREENSHOT_KEY) {
        writer.write(HarnessAction::Screenshot);
    }
    if scroll.delta.y > 0.0 {
        writer.write(HarnessAction::ZoomIn);
    } else if scroll.delta.y < 0.0 {
        writer.write(HarnessAction::ZoomOut);
    }
}

#[cfg(test)]
mod tests {
    //! Unit tests for the key-binding table and lookup.

    use rstest::rstest;

    use super::*;

    #[rstest]
    #[case(KeyCode::KeyQ, Some(HarnessAction::RotateLeft))]
    #[case(KeyCode::KeyE, Some(HarnessAction::RotateRight))]
    #[case(KeyCode::Equal, Some(HarnessAction::ZoomIn))]
    #[case(KeyCode::Minus, Some(HarnessAction::ZoomOut))]
    #[case(KeyCode::F3, Some(HarnessAction::ToggleOverlay))]
    #[case(KeyCode::KeyW, None)]
    #[case(KeyCode::Space, None)]
    fn bound_keys_map_to_their_actions(
        #[case] key: KeyCode,
        #[case] expected: Option<HarnessAction>,
    ) {
        assert_eq!(action_for_key(key), expected);
    }

    #[rstest]
    fn the_screenshot_key_is_not_a_press_binding() {
        assert_eq!(action_for_key(SCREENSHOT_KEY), None);
    }
}
