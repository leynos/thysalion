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
    Utf8PathBuf::from(SCREENSHOT_DIR).join(format!("{slug}-{seconds}-{sequence}.png"))
}

/// Renders the path as absolute where the working directory is readable,
/// so the log line is copy-pasteable.
fn absolute_display(path: &Utf8PathBuf) -> Utf8PathBuf {
    std::env::current_dir()
        .ok()
        .and_then(|cwd| Utf8PathBuf::from_path_buf(cwd).ok())
        .map_or_else(|| path.clone(), |cwd| cwd.join(path))
}
