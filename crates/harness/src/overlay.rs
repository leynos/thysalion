//! Windowed diagnostics overlay: frame time, frames per second, and the
//! simulation tick time when a measurement exists. Refreshes at a
//! throttled cadence reading Bevy's already-smoothed `DiagnosticsStore`
//! values, rather than re-formatting text every frame.

use core::time::Duration;

use bevy::{diagnostic::DiagnosticsStore, ecs::message::MessageReader, prelude::*};

use crate::{
    diagnostics::{FPS, FRAME_TIME, TICK_TIME},
    input::HarnessAction,
};

/// Seconds between overlay refreshes (~5 Hz reads steadily).
const REFRESH_SECONDS: f32 = 0.2;

/// Marker for the overlay text entity.
#[derive(Component, Debug)]
pub(crate) struct OverlayText;

/// Repeating timer throttling overlay refreshes.
#[derive(Resource, Debug)]
pub(crate) struct OverlayTimer(Timer);

impl Default for OverlayTimer {
    fn default() -> Self {
        Self(Timer::new(
            Duration::from_secs_f32(REFRESH_SECONDS),
            TimerMode::Repeating,
        ))
    }
}

impl OverlayTimer {
    /// Test seam: advances the timer to just before expiry, so the next
    /// system tick (any positive delta) triggers a refresh without the
    /// test having to wait out the real throttle interval.
    #[cfg(test)]
    pub(crate) fn advance_to_brink(&mut self) {
        let brink = self.0.duration().saturating_sub(Duration::from_nanos(1));
        self.0.tick(brink);
    }
}

/// Spawns the overlay text node, initially visible.
pub(crate) fn setup_overlay(mut commands: Commands) {
    commands.spawn((
        OverlayText,
        Text::new("diagnostics…"),
        TextFont::default(),
        TextColor(Color::WHITE),
        Node {
            position_type: PositionType::Absolute,
            top: Val::Px(8.0),
            left: Val::Px(8.0),
            ..Node::default()
        },
    ));
}

/// Shows or hides the overlay on [`HarnessAction::ToggleOverlay`].
pub(crate) fn toggle_overlay(
    mut reader: MessageReader<HarnessAction>,
    mut overlays: Query<&mut Visibility, With<OverlayText>>,
) {
    let toggles = reader
        .read()
        .filter(|action| matches!(action, HarnessAction::ToggleOverlay))
        .count();
    // An even number of toggles in one frame cancels out; `& 1` avoids the
    // denied remainder operator.
    if toggles & 1 == 1 {
        for mut visibility in &mut overlays {
            *visibility = match *visibility {
                Visibility::Hidden => Visibility::Inherited,
                _ => Visibility::Hidden,
            };
        }
    }
}

/// Formats the overlay readout from the smoothed diagnostic values.
fn format_readout(fps: Option<f64>, frame_time: Option<f64>, tick_time: Option<f64>) -> String {
    let frame = match (fps, frame_time) {
        (Some(fps_value), Some(frame_ms)) => {
            format!("{fps_value:.0} fps  {frame_ms:.2} ms/frame")
        }
        _ => String::from("collecting…"),
    };
    let tick = tick_time.map_or_else(
        || String::from("  tick: n/a"),
        |tick_ms| format!("  {tick_ms:.2} ms/tick"),
    );
    format!("{frame}{tick}")
}

/// Rewrites the overlay text at the throttled cadence.
#[expect(
    clippy::needless_pass_by_value,
    reason = "Bevy system parameters are taken by value"
)]
pub(crate) fn refresh_overlay(
    time: Res<Time>,
    mut timer: ResMut<OverlayTimer>,
    diagnostics: Res<DiagnosticsStore>,
    mut overlays: Query<&mut Text, With<OverlayText>>,
) {
    if !timer.0.tick(time.delta()).just_finished() {
        return;
    }
    let smoothed = |path| {
        diagnostics
            .get(path)
            .and_then(bevy::diagnostic::Diagnostic::smoothed)
    };
    let readout = format_readout(smoothed(&FPS), smoothed(&FRAME_TIME), smoothed(&TICK_TIME));
    for mut text in &mut overlays {
        text.0.clone_from(&readout);
    }
}

#[cfg(test)]
mod tests {
    //! Semantic assertions for the overlay readout formatter: exact
    //! strings for populated, partially populated, and missing
    //! diagnostics.

    use rstest::rstest;

    use super::*;

    #[rstest]
    fn populated_diagnostics_render_all_three_values() {
        let readout = format_readout(Some(60.0), Some(16.6666), Some(2.5));
        assert_eq!(readout, "60 fps  16.67 ms/frame  2.50 ms/tick");
    }

    #[rstest]
    fn missing_tick_measurement_renders_not_available() {
        let readout = format_readout(Some(14.0), Some(70.9111), None);
        assert_eq!(readout, "14 fps  70.91 ms/frame  tick: n/a");
    }

    #[rstest]
    fn missing_frame_diagnostics_render_the_collecting_placeholder() {
        assert_eq!(format_readout(None, None, None), "collecting…  tick: n/a");
    }

    #[rstest]
    fn partial_frame_diagnostics_still_render_collecting() {
        assert_eq!(
            format_readout(Some(60.0), None, Some(1.0)),
            "collecting…  1.00 ms/tick"
        );
    }
}
