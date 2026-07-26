//! Windowed screenshot capture: on [`HarnessAction::Screenshot`] (emitted
//! on key *release* — see the input module), capture the primary window to
//! `screenshots/<slug>-<unix-seconds>-<sequence>.png`.
//!
//! Bevy screenshots can lag the camera by one frame
//! (bevyengine/bevy issue 18230); triggering on key release and letting
//! the app run leaves the captured frame settled in practice. The users'
//! guide documents the behaviour.
//!
//! The slug is treated as untrusted at this filesystem boundary
//! ([`HarnessConfig::slug`] is public and mutable): [`sanitize_slug`]
//! reduces it to ASCII alphanumerics, `-`, and `_` (anything else becomes
//! `-`, and an empty result falls back to `capture`), so every generated
//! filename is a single file directly beneath the screenshots directory.
//!
//! Test seam: the capture counter is the [`CaptureSequence`] resource,
//! owned by `DemoHarnessPlugin` rather than process-global state, so every
//! test app starts from a deterministic sequence of zero. Behavioural
//! tests inspect the spawned [`Screenshot`] entities; the `save_to_disk`
//! observer only fires when a real renderer produces a capture event, so
//! headless tests never write image files.

use std::time::{SystemTime, UNIX_EPOCH};

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

/// Fallback filename stem when sanitisation leaves nothing of the slug.
const FALLBACK_SLUG: &str = "capture";

/// Monotonic capture counter, so two captures within the same second
/// cannot collide on one path. Owned by the plugin (not process-global)
/// so tests start from a deterministic sequence.
#[derive(Resource, Debug, Default)]
pub(crate) struct CaptureSequence(u64);

impl CaptureSequence {
    /// Returns the current sequence number and advances the counter.
    const fn advance(&mut self) -> u64 {
        let current = self.0;
        self.0 = self.0.wrapping_add(1);
        current
    }

    /// Returns how many captures have been sequenced so far.
    #[cfg(test)]
    pub(crate) const fn count(&self) -> u64 { self.0 }
}

/// Spawns a screenshot capture, with a distinct destination path, for
/// each requested action in the frame's batch.
#[expect(
    clippy::needless_pass_by_value,
    reason = "Bevy system parameters are taken by value"
)]
pub(crate) fn trigger_screenshots(
    mut reader: MessageReader<HarnessAction>,
    mut commands: Commands,
    config: Res<HarnessConfig>,
    mut sequence: ResMut<CaptureSequence>,
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
        let path = capture_path(&config.slug, sequence.advance());
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

/// Builds the capture path `screenshots/<slug>-<unix-seconds>-<sequence>.png`
/// from the sanitized slug and the supplied sequence number.
fn capture_path(slug: &str, sequence: u64) -> Utf8PathBuf {
    let seconds = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |elapsed| elapsed.as_secs());
    let safe_slug = sanitize_slug(slug);
    Utf8PathBuf::from(SCREENSHOT_DIR).join(format!("{safe_slug}-{seconds}-{sequence}.png"))
}

/// Reduces a slug to filename-safe characters.
///
/// Policy: ASCII alphanumerics, `-`, and `_` are retained; every other
/// character (path separators, `..` components, absolute-path prefixes,
/// control characters, Unicode separators, and so on) becomes `-`; a slug
/// that sanitizes to nothing falls back to [`FALLBACK_SLUG`]. The result
/// therefore never contains a path separator and never forms a traversal
/// component.
fn sanitize_slug(slug: &str) -> String {
    let safe: String = slug
        .chars()
        .map(|c| if is_slug_safe(c) { c } else { '-' })
        .collect();
    if safe.is_empty() {
        String::from(FALLBACK_SLUG)
    } else {
        safe
    }
}

/// Returns whether a character is retained by the slug policy.
const fn is_slug_safe(c: char) -> bool { c.is_ascii_alphanumeric() || matches!(c, '-' | '_') }

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
    //! distinct, directory-confined destination; hostile slugs cannot
    //! escape the screenshots directory; and mixed action batches spawn
    //! one capture per screenshot action. No test writes an image file —
    //! the `save_to_disk` observer never fires without a renderer.

    use bevy::ecs::message::Messages;
    use rstest::rstest;

    use super::*;

    #[rstest]
    fn consecutive_sequences_produce_distinct_paths() {
        let first = capture_path("demo-test", 0);
        let second = capture_path("demo-test", 1);
        assert_ne!(first, second, "same-second captures must not collide");
    }

    #[rstest]
    fn normal_slugs_retain_the_expected_structure() {
        let path = capture_path("demo-empty", 7);
        let name = path.file_name().expect("capture path has a file name");
        assert!(
            path.starts_with(SCREENSHOT_DIR),
            "capture must live under {SCREENSHOT_DIR}, got {path}"
        );
        assert!(
            name.starts_with("demo-empty-") && name.ends_with("-7.png"),
            "expected demo-empty-<seconds>-7.png, got {name}"
        );
    }

    #[rstest]
    #[case::traversal("../escape/attempt")]
    #[case::nested_traversal("a/../../b")]
    #[case::absolute_unix("/etc/passwd")]
    #[case::absolute_windows("C:\\Windows\\System32")]
    #[case::backslashes("..\\escape")]
    #[case::control_characters("demo\u{0}\u{1b}name")]
    #[case::unicode_separators("demo\u{2028}\u{2029}name")]
    #[case::spaces_and_dots("demo .. name")]
    fn hostile_slugs_stay_inside_the_screenshots_directory(#[case] slug: &str) {
        let path = capture_path(slug, 0);
        let name = path.file_name().expect("capture path has a file name");
        assert!(
            path.starts_with(SCREENSHOT_DIR),
            "sanitized path must stay under {SCREENSHOT_DIR}, got {path}"
        );
        assert_eq!(
            path.components().count(),
            2,
            "capture must be a single file directly beneath {SCREENSHOT_DIR}, got {path}"
        );
        assert!(
            !name.contains("..") && !name.contains('/') && !name.contains('\\'),
            "sanitized file name must not contain traversal material, got {name}"
        );
    }

    #[rstest]
    #[case::empty("")]
    #[case::only_unsafe("///")]
    fn degenerate_slugs_fall_back_to_a_non_empty_stem(#[case] slug: &str) {
        let sanitized = sanitize_slug(slug);
        assert!(
            !sanitized.is_empty(),
            "sanitized slug must never be empty (input {slug:?})"
        );
    }

    /// Builds a headless app hosting only the screenshot system and its
    /// dependencies; the capture sequence starts at zero.
    fn screenshot_app() -> App {
        let mut app = App::new();
        app.add_message::<HarnessAction>()
            .insert_resource(HarnessConfig::default())
            .init_resource::<CaptureSequence>()
            .add_systems(Update, trigger_screenshots);
        app
    }

    fn count_captures(app: &mut App) -> usize {
        app.world_mut()
            .query::<&Screenshot>()
            .iter(app.world())
            .count()
    }

    #[rstest]
    fn mixed_batches_spawn_one_capture_per_screenshot_action() {
        let mut app = screenshot_app();
        let mut messages = app.world_mut().resource_mut::<Messages<HarnessAction>>();
        messages.write(HarnessAction::Screenshot);
        messages.write(HarnessAction::RotateLeft);
        messages.write(HarnessAction::Screenshot);
        app.update();
        assert_eq!(
            count_captures(&mut app),
            2,
            "two screenshot actions must spawn two captures"
        );
        assert_eq!(
            app.world().resource::<CaptureSequence>().count(),
            2,
            "each capture must consume one sequence number, so the two destination paths differ"
        );
    }

    #[rstest]
    fn non_screenshot_actions_spawn_no_captures() {
        let mut app = screenshot_app();
        let mut messages = app.world_mut().resource_mut::<Messages<HarnessAction>>();
        messages.write(HarnessAction::RotateLeft);
        messages.write(HarnessAction::ZoomIn);
        messages.write(HarnessAction::ToggleOverlay);
        app.update();
        assert_eq!(
            count_captures(&mut app),
            0,
            "non-screenshot actions must not schedule captures"
        );
        assert_eq!(
            app.world().resource::<CaptureSequence>().count(),
            0,
            "the sequence must not advance without screenshot actions"
        );
    }
}
