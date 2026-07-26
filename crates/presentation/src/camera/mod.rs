//! Pure camera contract for the orthographic-isometric rig of
//! thysalion-design.md §8.2: exactly four yaw quadrants and a bounded zoom
//! range. No Bevy types appear here; the demo harness (and, from phase 2,
//! the octant-culling mesher) consume these values.

use core::{
    f32::consts::{FRAC_PI_2, FRAC_PI_4},
    fmt,
};

/// The four allowed camera yaw quadrants (thysalion-design.md §8.2).
///
/// The enum is deliberately exhaustive: four quadrants is a domain
/// invariant, and downstream `match` expressions are expected to cover all
/// four. The cyclic order is `NorthEast → SouthEast → SouthWest →
/// NorthWest` under [`Quadrant::next`]; rely on [`Quadrant::next`] and
/// [`Quadrant::prev`], never on the declaration order.
///
/// # Examples
///
/// ```
/// use thysalion_presentation::Quadrant;
///
/// let start = Quadrant::NorthEast;
/// assert_eq!(start.next().prev(), start);
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum Quadrant {
    /// Camera looks into the scene from the north-east.
    #[default]
    NorthEast,
    /// Camera looks into the scene from the south-east.
    SouthEast,
    /// Camera looks into the scene from the south-west.
    SouthWest,
    /// Camera looks into the scene from the north-west.
    NorthWest,
}

impl Quadrant {
    /// All four quadrants in cyclic [`Quadrant::next`] order.
    pub const ALL: [Self; 4] = [
        Self::NorthEast,
        Self::SouthEast,
        Self::SouthWest,
        Self::NorthWest,
    ];

    /// Returns the next quadrant in cyclic order (a clockwise quarter-turn
    /// of the viewpoint when seen from above).
    ///
    /// # Examples
    ///
    /// ```
    /// use thysalion_presentation::Quadrant;
    ///
    /// assert_eq!(Quadrant::NorthEast.next(), Quadrant::SouthEast);
    /// assert_eq!(Quadrant::NorthWest.next(), Quadrant::NorthEast);
    /// ```
    #[must_use]
    pub const fn next(self) -> Self {
        match self {
            Self::NorthEast => Self::SouthEast,
            Self::SouthEast => Self::SouthWest,
            Self::SouthWest => Self::NorthWest,
            Self::NorthWest => Self::NorthEast,
        }
    }

    /// Returns the previous quadrant in cyclic order (the inverse of
    /// [`Quadrant::next`]).
    ///
    /// # Examples
    ///
    /// ```
    /// use thysalion_presentation::Quadrant;
    ///
    /// assert_eq!(Quadrant::NorthEast.prev(), Quadrant::NorthWest);
    /// ```
    #[must_use]
    pub const fn prev(self) -> Self {
        match self {
            Self::NorthEast => Self::NorthWest,
            Self::SouthEast => Self::NorthEast,
            Self::SouthWest => Self::SouthEast,
            Self::NorthWest => Self::SouthWest,
        }
    }

    /// Returns the camera yaw for this quadrant, in radians.
    ///
    /// Quadrant yaws sit on the diagonals — `π/4 + k·π/2` — so the camera
    /// always looks along a scene diagonal, matching the reference art's
    /// framing.
    ///
    /// # Examples
    ///
    /// ```
    /// use core::f32::consts::FRAC_PI_4;
    ///
    /// use thysalion_presentation::Quadrant;
    ///
    /// assert!((Quadrant::NorthEast.yaw_radians() - FRAC_PI_4).abs() < f32::EPSILON);
    /// ```
    #[must_use]
    #[expect(
        clippy::float_arithmetic,
        reason = "camera geometry is inherently floating point (design §8.2)"
    )]
    pub const fn yaw_radians(self) -> f32 {
        match self {
            Self::NorthEast => FRAC_PI_4,
            Self::SouthEast => FRAC_PI_4 + FRAC_PI_2,
            Self::SouthWest => FRAC_PI_4 + 2.0 * FRAC_PI_2,
            Self::NorthWest => FRAC_PI_4 + 3.0 * FRAC_PI_2,
        }
    }
}

/// Validated zoom range: construction enforces `0 < min < max`.
///
/// Zoom is a magnification factor: `1.0` is the baseline framing, larger
/// values are closer. The orthographic viewport height for a zoom level is
/// produced by [`ZoomBounds::viewport_height`], which is strictly
/// decreasing in zoom (zooming in shows less of the scene).
///
/// # Examples
///
/// ```
/// use thysalion_presentation::ZoomBounds;
///
/// let bounds = ZoomBounds::new(0.5, 4.0).expect("valid bounds");
/// assert!((bounds.clamp(9.0) - 4.0).abs() < f32::EPSILON);
/// ```
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ZoomBounds {
    min: f32,
    max: f32,
}

/// Why a [`ZoomBounds`] construction was rejected.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum ZoomBoundsError {
    /// The minimum zoom was zero, negative, or not finite.
    NonPositiveMinimum,
    /// The maximum zoom did not exceed the minimum, or was not finite.
    MaximumNotAboveMinimum,
}

impl fmt::Display for ZoomBoundsError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NonPositiveMinimum => {
                write!(f, "zoom minimum must be finite and greater than zero")
            }
            Self::MaximumNotAboveMinimum => {
                write!(
                    f,
                    "zoom maximum must be finite and greater than the minimum"
                )
            }
        }
    }
}

impl core::error::Error for ZoomBoundsError {}

impl ZoomBounds {
    /// Baseline orthographic viewport height at zoom `1.0`, in world units.
    pub const BASE_VIEWPORT_HEIGHT: f32 = 12.0;

    const DEFAULT_MIN: f32 = 0.5;
    const DEFAULT_MAX: f32 = 4.0;

    /// Creates a zoom range, rejecting non-finite, non-positive, or
    /// inverted bounds.
    ///
    /// # Errors
    ///
    /// Returns [`ZoomBoundsError::NonPositiveMinimum`] when `min` is not a
    /// finite positive number, and
    /// [`ZoomBoundsError::MaximumNotAboveMinimum`] when `max` is not
    /// finite or does not exceed `min`.
    ///
    /// # Examples
    ///
    /// ```
    /// use thysalion_presentation::{ZoomBounds, ZoomBoundsError};
    ///
    /// assert!(ZoomBounds::new(0.5, 4.0).is_ok());
    /// assert_eq!(
    ///     ZoomBounds::new(0.0, 4.0),
    ///     Err(ZoomBoundsError::NonPositiveMinimum)
    /// );
    /// assert_eq!(
    ///     ZoomBounds::new(2.0, 2.0),
    ///     Err(ZoomBoundsError::MaximumNotAboveMinimum)
    /// );
    /// ```
    pub fn new(min: f32, max: f32) -> Result<Self, ZoomBoundsError> {
        if !min.is_finite() || min <= 0.0 {
            return Err(ZoomBoundsError::NonPositiveMinimum);
        }
        if !max.is_finite() || max <= min {
            return Err(ZoomBoundsError::MaximumNotAboveMinimum);
        }
        Ok(Self { min, max })
    }

    /// Returns the smallest permitted zoom level.
    #[must_use]
    pub const fn min(&self) -> f32 { self.min }

    /// Returns the largest permitted zoom level.
    #[must_use]
    pub const fn max(&self) -> f32 { self.max }

    /// Clamps a requested zoom level into the permitted range.
    ///
    /// Non-finite requests clamp to the minimum, so a corrupted input can
    /// never wedge the camera at an unrepresentable zoom.
    ///
    /// # Examples
    ///
    /// ```
    /// use thysalion_presentation::ZoomBounds;
    ///
    /// let bounds = ZoomBounds::new(0.5, 4.0).expect("valid bounds");
    /// assert!((bounds.clamp(0.1) - 0.5).abs() < f32::EPSILON);
    /// assert!((bounds.clamp(1.0) - 1.0).abs() < f32::EPSILON);
    /// ```
    #[must_use]
    pub const fn clamp(&self, level: f32) -> f32 {
        if !level.is_finite() {
            return self.min;
        }
        level.clamp(self.min, self.max)
    }

    /// Returns the orthographic viewport height for a zoom level, in world
    /// units. Strictly decreasing in zoom: zooming in shows less.
    ///
    /// # Examples
    ///
    /// ```
    /// use thysalion_presentation::ZoomBounds;
    ///
    /// let bounds = ZoomBounds::new(0.5, 4.0).expect("valid bounds");
    /// assert!(bounds.viewport_height(2.0) < bounds.viewport_height(1.0));
    /// ```
    #[must_use]
    #[expect(
        clippy::float_arithmetic,
        reason = "camera geometry is inherently floating point (design §8.2)"
    )]
    pub fn viewport_height(&self, zoom: f32) -> f32 {
        Self::BASE_VIEWPORT_HEIGHT / self.clamp(zoom)
    }
}

impl Default for ZoomBounds {
    /// The default range spans half to four-times magnification, matching
    /// the bounded-zoom requirement of design §8.2.
    fn default() -> Self {
        Self {
            min: Self::DEFAULT_MIN,
            max: Self::DEFAULT_MAX,
        }
    }
}

#[cfg(test)]
mod tests;
