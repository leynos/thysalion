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
    //! Unit tests for the key-binding table and lookup, plus behavioural
    //! coverage of [`read_input`]'s scroll and screenshot-release paths.
    //!
    //! The behavioural tests build a bare app holding exactly the
    //! resources `HarnessCorePlugin` initializes for the same system, so
    //! they exercise the real scheduling path without a window, renderer,
    //! operating-system input, or timing dependency.

    use bevy::{
        app::{App, Update},
        ecs::message::Messages,
        math::Vec2,
    };
    use rstest::rstest;

    use super::*;

    /// Builds a headless app running only [`read_input`], with the input
    /// resources `HarnessCorePlugin` provides.
    fn input_app() -> App {
        let mut app = App::new();
        app.add_message::<HarnessAction>()
            .init_resource::<ButtonInput<KeyCode>>()
            .init_resource::<AccumulatedMouseScroll>()
            .add_systems(Update, read_input);
        app
    }

    /// Sets the accumulated scroll delta for the next update.
    fn set_scroll(app: &mut App, delta: Vec2) {
        app.world_mut()
            .resource_mut::<AccumulatedMouseScroll>()
            .delta = delta;
    }

    /// Drains the actions emitted so far, in order.
    fn drain_actions(app: &mut App) -> Vec<HarnessAction> {
        let mut messages = app.world_mut().resource_mut::<Messages<HarnessAction>>();
        messages.drain().collect()
    }

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

    #[rstest]
    #[case::scroll_up(Vec2::new(0.0, 1.0), Some(HarnessAction::ZoomIn))]
    #[case::scroll_down(Vec2::new(0.0, -1.0), Some(HarnessAction::ZoomOut))]
    #[case::small_scroll_up(Vec2::new(0.0, 0.01), Some(HarnessAction::ZoomIn))]
    #[case::no_scroll(Vec2::ZERO, None)]
    #[case::horizontal_only(Vec2::new(3.0, 0.0), None)]
    #[case::horizontal_negative(Vec2::new(-3.0, 0.0), None)]
    fn vertical_scroll_drives_zoom_actions(
        #[case] delta: Vec2,
        #[case] expected: Option<HarnessAction>,
    ) {
        let mut app = input_app();
        set_scroll(&mut app, delta);
        app.update();
        let actions = drain_actions(&mut app);
        match expected {
            Some(action) => assert_eq!(
                actions,
                vec![action],
                "scroll delta {delta:?} must emit exactly one {action:?}"
            ),
            None => assert!(
                actions.is_empty(),
                "scroll delta {delta:?} must emit no action, got {actions:?}"
            ),
        }
    }

    #[rstest]
    fn diagonal_scroll_emits_one_zoom_action_from_its_vertical_component() {
        let mut app = input_app();
        set_scroll(&mut app, Vec2::new(-5.0, 2.0));
        app.update();
        assert_eq!(
            drain_actions(&mut app),
            vec![HarnessAction::ZoomIn],
            "the horizontal component must not add or suppress an action"
        );
    }

    #[rstest]
    fn the_screenshot_key_fires_once_on_release_and_never_on_press() {
        let mut app = input_app();

        app.world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .press(SCREENSHOT_KEY);
        app.update();
        assert!(
            drain_actions(&mut app).is_empty(),
            "pressing the screenshot key must not capture; capture is on release"
        );

        app.world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .release(SCREENSHOT_KEY);
        app.update();
        assert_eq!(
            drain_actions(&mut app),
            vec![HarnessAction::Screenshot],
            "releasing the screenshot key must capture exactly once"
        );

        // `just_released` is a one-frame edge. This bare app has no
        // `clear_synthetic_input` system, so the assertion also proves the
        // edge is consumed by `ButtonInput` itself rather than by the
        // harness's synthetic-input clearing.
        app.world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .clear();
        app.update();
        assert!(
            drain_actions(&mut app).is_empty(),
            "the screenshot action must not repeat after its release frame"
        );
    }
}
