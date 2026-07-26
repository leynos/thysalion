//! Unit tests for the camera contract: quadrant cycle laws, yaw
//! placement, zoom validation, clamping, and viewport monotonicity.

use rstest::{fixture, rstest};

use super::*;

/// Shared zoom range exercised by the clamping and viewport tests.
/// [`ZoomBounds::default`] is the 0.5–4.0 range of design §8.2.
#[fixture]
fn bounds() -> ZoomBounds {
    // Kept multi-line: collapsing to one line trips `unused_braces`
    // through the `#[fixture]` macro expansion.
    ZoomBounds::default()
}

/// Asserts two floats are within [`f32::EPSILON`] of each other.
#[expect(
    clippy::float_arithmetic,
    reason = "epsilon comparison is inherently floating point"
)]
fn assert_close(actual: f32, expected: f32) {
    assert!(
        (actual - expected).abs() < f32::EPSILON,
        "{actual} should equal {expected}"
    );
}

#[rstest]
#[case(Quadrant::NorthEast, Quadrant::SouthEast)]
#[case(Quadrant::SouthEast, Quadrant::SouthWest)]
#[case(Quadrant::SouthWest, Quadrant::NorthWest)]
#[case(Quadrant::NorthWest, Quadrant::NorthEast)]
fn next_moves_to_the_adjacent_quadrant(#[case] from: Quadrant, #[case] expected: Quadrant) {
    assert_eq!(from.next(), expected);
}

/// Exhaustive proof of the quadrant cycle laws.
///
/// `Quadrant` is a closed four-value enum, so iterating
/// [`Quadrant::ALL`] enumerates the entire input space: this test *is*
/// the proof of cyclic identity (`prev` inverts `next`) and four-step
/// closure, with no generated inputs required.
#[rstest]
fn quadrant_cycle_laws_hold_for_every_quadrant() {
    for quadrant in Quadrant::ALL {
        assert_eq!(quadrant.next().prev(), quadrant, "prev must invert next");
        assert_eq!(quadrant.prev().next(), quadrant, "next must invert prev");
        assert_eq!(
            quadrant.next().next().next().next(),
            quadrant,
            "four next steps must return home"
        );
        assert_eq!(
            quadrant.prev().prev().prev().prev(),
            quadrant,
            "four prev steps must return home"
        );
    }
}

#[rstest]
#[expect(
    clippy::float_arithmetic,
    reason = "computing expected yaws and epsilon comparisons"
)]
fn yaws_are_the_four_quarter_turn_diagonals() {
    for (index, quadrant) in Quadrant::ALL.iter().enumerate() {
        #[expect(
            clippy::cast_precision_loss,
            reason = "index is at most three; exact in f32"
        )]
        let expected = FRAC_PI_4 + (index as f32) * FRAC_PI_2;
        assert!(
            (quadrant.yaw_radians() - expected).abs() < f32::EPSILON,
            "{quadrant:?} yaw should be {expected}"
        );
    }
}

#[rstest]
#[case(0.0, 4.0, ZoomBoundsError::NonPositiveMinimum)]
#[case(-1.0, 4.0, ZoomBoundsError::NonPositiveMinimum)]
#[case(f32::NAN, 4.0, ZoomBoundsError::NonPositiveMinimum)]
#[case(2.0, 2.0, ZoomBoundsError::MaximumNotAboveMinimum)]
#[case(2.0, 1.0, ZoomBoundsError::MaximumNotAboveMinimum)]
#[case(2.0, f32::INFINITY, ZoomBoundsError::MaximumNotAboveMinimum)]
fn invalid_bounds_are_rejected(
    #[case] min: f32,
    #[case] max: f32,
    #[case] expected: ZoomBoundsError,
) {
    assert_eq!(ZoomBounds::new(min, max), Err(expected));
}

#[rstest]
#[case(0.1, 0.5)]
#[case(1.0, 1.0)]
#[case(9.0, 4.0)]
#[case(f32::NAN, 0.5)]
fn zoom_requests_clamp_to_bounds(bounds: ZoomBounds, #[case] request: f32, #[case] expected: f32) {
    assert_close(bounds.clamp(request), expected);
}

#[rstest]
fn viewport_height_is_strictly_decreasing_in_zoom(bounds: ZoomBounds) {
    let zooms = [0.5_f32, 1.0, 2.0, 4.0];
    for (lower, higher) in zooms.iter().zip(zooms.iter().skip(1)) {
        assert!(
            bounds.viewport_height(*higher) < bounds.viewport_height(*lower),
            "viewport height must shrink as zoom grows"
        );
    }
}

mod properties {
    //! Generated-input properties for the zoom contract. The quadrant
    //! half of the camera contract needs no generated inputs — the
    //! exhaustive enumeration above covers the whole space.

    use proptest::prelude::*;

    use super::*;

    /// Builds bounds whose maximum is a multiple of the minimum, so
    /// the monotonicity property always has room for two distinct
    /// in-bounds zoom levels.
    #[expect(
        clippy::float_arithmetic,
        reason = "constructing generated zoom ranges"
    )]
    fn bounds_from(min: f32, ratio: f32) -> ZoomBounds {
        match ZoomBounds::new(min, min * ratio) {
            Ok(bounds) => bounds,
            Err(error) => panic!("generated bounds must be valid: {error}"),
        }
    }

    fn wide_bounds() -> impl Strategy<Value = ZoomBounds> {
        (1.0e-3_f32..1.0e2, 2.0_f32..100.0).prop_map(|(min, ratio)| bounds_from(min, ratio))
    }

    /// Picks an ordered in-bounds pair `(low, high)` with at least a
    /// one-percent relative gap, so strict `f32` comparison is
    /// meaningful.
    #[expect(
        clippy::float_arithmetic,
        reason = "deriving generated zoom levels from the bounds"
    )]
    fn ordered_zooms(bounds: ZoomBounds, frac: f32, factor: f32) -> (f32, f32) {
        let half = bounds.max() / 2.0;
        let low = bounds.min() + (half - bounds.min()) * frac;
        (low, low * factor)
    }

    proptest! {
        /// `clamp` must return a finite, in-bounds value for every
        /// possible `f32` request, including NaN and the infinities.
        #[test]
        fn clamp_is_always_finite_and_in_bounds(
            bounds in wide_bounds(),
            request in proptest::num::f32::ANY,
        ) {
            let clamped = bounds.clamp(request);
            prop_assert!(clamped.is_finite(), "clamp({request}) = {clamped}");
            prop_assert!(
                clamped >= bounds.min() && clamped <= bounds.max(),
                "clamp({request}) = {clamped} escaped {bounds:?}"
            );
        }

        /// Viewport height must strictly decrease as zoom grows, for
        /// generated ordered finite zoom pairs inside valid bounds.
        #[test]
        fn viewport_height_decreases_for_generated_ordered_zooms(
            bounds in wide_bounds(),
            frac in 0.0_f32..1.0,
            factor in 1.01_f32..2.0,
        ) {
            let (low, high) = ordered_zooms(bounds, frac, factor);
            prop_assert!(
                bounds.viewport_height(high) < bounds.viewport_height(low),
                "viewport height must shrink from zoom {low} to {high}"
            );
        }
    }
}
