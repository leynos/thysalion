//! Windowed screenshot capture: on [`HarnessAction::Screenshot`] (emitted
//! on key *release* — see the input module), capture the primary window to
//! `screenshots/<slug>-<timestamp>-<sequence>.png`.
//!
//! Bevy screenshots can lag the camera by one frame
//! (bevyengine/bevy issue 18230); triggering on key release and letting
//! the app run leaves the captured frame settled in practice. The users'
//! guide documents the behaviour.

use std::{
    sync::atomic::{AtomicU64, Ordering},
    time::{SystemTime, UNIX_EPOCH},
};

use bevy::{
    ecs::message::MessageReader,
    prelude::*,
    render::view::screenshot::{Screenshot, save_to_disk},
};
use camino::Utf8PathBuf;
use cap_std::{ambient_authority, fs_utf8::Dir};

use crate::{config::HarnessConfig, input::HarnessAction};

/// Directory (relative to the working directory) receiving captures.
const SCREENSHOT_DIR: &str = "screenshots";

/// Process-local capture counter, so two captures within the same second
/// cannot collide on one path.
static CAPTURE_SEQUENCE: AtomicU64 = AtomicU64::new(0);

/// Spawns a screenshot capture for each requested action.
#[expect(
    clippy::needless_pass_by_value,
    reason = "Bevy system parameters are taken by value"
)]
pub(crate) fn trigger_screenshots(
    mut reader: MessageReader<HarnessAction>,
    mut commands: Commands,
    config: Res<HarnessConfig>,
) {
    let requested = reader
        .read()
        .filter(|action| matches!(action, HarnessAction::Screenshot))
        .count();
    if requested == 0 {
        return;
    }
    if let Err(error) = ensure_screenshot_dir() {
        error!(%error, "could not create the screenshots directory; capture skipped");
        return;
    }
    for _ in 0..requested {
        let path = capture_path(&config.slug);
        info!(path = %absolute_display(&path), "capturing screenshot");
        commands
            .spawn(Screenshot::primary_window())
            .observe(save_to_disk(path.into_string()));
    }
}

/// Creates the screenshots directory when missing.
fn ensure_screenshot_dir() -> std::io::Result<()> {
    let cwd = Dir::open_ambient_dir(".", ambient_authority())?;
    cwd.create_dir_all(SCREENSHOT_DIR)
}

/// Builds the capture path `screenshots/<slug>-<unix-seconds>-<sequence>.png`.
fn capture_path(slug: &str) -> Utf8PathBuf {
    let seconds = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |elapsed| elapsed.as_secs());
    let sequence = CAPTURE_SEQUENCE.fetch_add(1, Ordering::Relaxed);
    let safe_slug = sanitize_slug(slug);
    Utf8PathBuf::from(SCREENSHOT_DIR).join(format!("{safe_slug}-{seconds}-{sequence}.png"))
}

/// Reduces a slug to filename-safe characters, replacing anything outside
/// ASCII alphanumerics and `-` with `-`, so a slug containing path
/// separators cannot escape the screenshots directory.
fn sanitize_slug(slug: &str) -> String {
    slug.chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '-' {
                c
            } else {
                '-'
            }
        })
        .collect()
}

/// Renders the path as absolute where the working directory is readable,
/// so the log line is copy-pasteable.
fn absolute_display(path: &Utf8PathBuf) -> Utf8PathBuf {
    std::env::current_dir()
        .ok()
        .and_then(|cwd| Utf8PathBuf::from_path_buf(cwd).ok())
        .map_or_else(|| path.clone(), |cwd| cwd.join(path))
}

#[cfg(test)]
mod tests {
    //! Coverage for screenshot scheduling: every request maps to a
    //! distinct, directory-confined destination, and mixed action batches
    //! spawn one capture per screenshot action.

    use bevy::ecs::message::Messages;

    use super::*;

    #[test]
    fn consecutive_capture_paths_are_distinct() {
        let first = capture_path("demo-test");
        let second = capture_path("demo-test");
        assert_ne!(first, second, "same-second captures must not collide");
    }

    #[test]
    fn hostile_slugs_stay_inside_the_screenshots_directory() {
        let path = capture_path("../escape/attempt");
        assert!(
            path.starts_with(SCREENSHOT_DIR) && !path.as_str().contains(".."),
            "sanitized path escaped the screenshots directory: {path}"
        );
    }

    #[test]
    fn mixed_batches_spawn_one_capture_per_screenshot_action() {
        let mut app = App::new();
        app.add_message::<HarnessAction>()
            .insert_resource(HarnessConfig::default())
            .add_systems(Update, trigger_screenshots);
        let mut messages = app.world_mut().resource_mut::<Messages<HarnessAction>>();
        messages.write(HarnessAction::Screenshot);
        messages.write(HarnessAction::RotateLeft);
        messages.write(HarnessAction::Screenshot);
        app.update();
        let captures = app
            .world_mut()
            .query::<&Screenshot>()
            .iter(app.world())
            .count();
        assert_eq!(
            captures, 2,
            "two screenshot actions must spawn two captures"
        );
    }
}
