//! Declarative harness settings a demo supplies when registering the
//! harness plugins. `HarnessConfig` is `#[non_exhaustive]` and built with
//! chainable `with_*` methods so adding a field never breaks existing
//! demos ("no scaffolding rework" — see the 1.1 execplan).

use bevy::prelude::Resource;
use thysalion_presentation::{Quadrant, ZoomBounds};

/// Declarative settings a demo supplies when registering the harness
/// plugins. Inserted as a Bevy [`Resource`] by `HarnessCorePlugin`.
///
/// The `slug` is the demo's stable machine name: it names screenshot
/// files (`screenshots/<slug>-<timestamp>-<sequence>.png`). The screenshot
/// module sanitizes the slug at the filesystem boundary (ASCII
/// alphanumerics, `-`, and `_` are kept; anything else becomes `-`), so a
/// hostile value cannot escape the screenshots directory — but keeping the
/// slug filename-safe at source is still the convention. The window title
/// is presentation-only and defaults to a value derived from the slug.
///
/// # Examples
///
/// ```
/// use thysalion_harness::HarnessConfig;
/// use thysalion_presentation::Quadrant;
///
/// let config = HarnessConfig::new("demo-empty")
///     .with_window_title("Thysalion — empty scene")
///     .with_initial_quadrant(Quadrant::SouthWest);
/// assert_eq!(config.slug, "demo-empty");
/// assert_eq!(config.initial_quadrant, Quadrant::SouthWest);
/// ```
#[derive(Resource, Debug, Clone)]
#[non_exhaustive]
pub struct HarnessConfig {
    /// Stable machine name for the demo (used in screenshot filenames).
    pub slug: String,
    /// Window title shown by windowed demos.
    pub window_title: String,
    /// Permitted zoom range for the camera rig.
    pub zoom_bounds: ZoomBounds,
    /// Quadrant the camera starts in.
    pub initial_quadrant: Quadrant,
}

impl HarnessConfig {
    /// Creates a configuration for the named demo with default camera
    /// settings and a window title derived from the slug.
    #[must_use]
    pub fn new(slug: impl Into<String>) -> Self {
        let slug_text = slug.into();
        Self {
            window_title: format!("Thysalion — {slug_text}"),
            slug: slug_text,
            zoom_bounds: ZoomBounds::default(),
            initial_quadrant: Quadrant::default(),
        }
    }

    /// Overrides the window title.
    #[must_use]
    pub fn with_window_title(mut self, title: impl Into<String>) -> Self {
        self.window_title = title.into();
        self
    }

    /// Overrides the permitted zoom range.
    #[must_use]
    pub const fn with_zoom_bounds(mut self, bounds: ZoomBounds) -> Self {
        self.zoom_bounds = bounds;
        self
    }

    /// Overrides the starting quadrant.
    #[must_use]
    pub const fn with_initial_quadrant(mut self, quadrant: Quadrant) -> Self {
        self.initial_quadrant = quadrant;
        self
    }
}

impl Default for HarnessConfig {
    /// A generic configuration for tests and tools that do not represent a
    /// specific demo.
    fn default() -> Self { Self::new("harness") }
}

#[cfg(test)]
mod tests {
    //! Unit tests for configuration construction and builder overrides.

    use rstest::rstest;
    use thysalion_presentation::Quadrant;

    use super::*;

    #[rstest]
    fn new_derives_the_window_title_from_the_slug() {
        let config = HarnessConfig::new("demo-empty");
        assert_eq!(config.window_title, "Thysalion — demo-empty");
    }

    #[rstest]
    fn builders_override_defaults() {
        let config = HarnessConfig::new("demo-empty")
            .with_window_title("custom")
            .with_initial_quadrant(Quadrant::SouthEast);
        assert_eq!(config.window_title, "custom");
        assert_eq!(config.initial_quadrant, Quadrant::SouthEast);
    }
}
