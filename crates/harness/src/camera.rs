//! Windowed camera systems: spawn the orthographic-isometric rig camera
//! and settle it smoothly toward the active quadrant's yaw. These systems
//! touch render-world types (`Camera3d`, `Projection`) and therefore live
//! in `DemoHarnessPlugin`, never in the headless core (they are also
//! excluded from the CI coverage boundary — see the developers' guide).

use bevy::{camera::ScalingMode, prelude::*};
use thysalion_presentation::ZoomBounds;

use crate::{config::HarnessConfig, rig::RigState};

/// Marker for the camera entity the harness owns.
#[derive(Component, Debug)]
pub struct HarnessCamera;

/// The camera's smoothed yaw, in radians (settles toward the rig's
/// quadrant yaw rather than snapping).
#[derive(Component, Debug)]
pub(crate) struct CameraYaw(f32);

/// Distance from the scene origin to the camera along its view ray.
const CAMERA_DISTANCE: f32 = 40.0;
/// Classic 2:1 dimetric pitch, in radians (`atan(1/sqrt(2))` ≈ 35.264°).
const CAMERA_PITCH: f32 = 0.615_479_7;
/// Exponential settle rate for quadrant turns (higher settles faster).
const YAW_SETTLE_RATE: f32 = 8.0;

/// Spawns the harness camera at the configured quadrant and zoom.
#[expect(
    clippy::needless_pass_by_value,
    reason = "Bevy system parameters are taken by value"
)]
pub(crate) fn spawn_camera(mut commands: Commands, config: Res<HarnessConfig>, rig: Res<RigState>) {
    let yaw = rig.quadrant().yaw_radians();
    commands.spawn((
        HarnessCamera,
        CameraYaw(yaw),
        Camera3d::default(),
        Projection::from(OrthographicProjection {
            scaling_mode: ScalingMode::FixedVertical {
                viewport_height: config.zoom_bounds.viewport_height(rig.zoom()),
            },
            ..OrthographicProjection::default_3d()
        }),
        camera_transform(yaw),
    ));
}

/// Settles the camera toward the rig's target yaw and applies zoom.
#[expect(
    clippy::needless_pass_by_value,
    reason = "Bevy system parameters are taken by value"
)]
#[expect(
    clippy::float_arithmetic,
    reason = "camera geometry is inherently floating point (design §8.2)"
)]
pub(crate) fn sync_camera(
    time: Res<Time>,
    rig: Res<RigState>,
    config: Res<HarnessConfig>,
    mut cameras: Query<(&mut Transform, &mut Projection, &mut CameraYaw), With<HarnessCamera>>,
) {
    let target = rig.quadrant().yaw_radians();
    let blend = 1.0 - (-YAW_SETTLE_RATE * time.delta_secs()).exp();
    for (mut transform, mut projection, mut yaw) in &mut cameras {
        yaw.0 += shortest_angle_delta(yaw.0, target) * blend;
        *transform = camera_transform(yaw.0);
        if let Projection::Orthographic(ref mut orthographic) = *projection {
            orthographic.scaling_mode = ScalingMode::FixedVertical {
                viewport_height: config.zoom_bounds.viewport_height(rig.zoom()),
            };
        }
    }
}

/// Builds the camera transform for a yaw angle at the fixed dimetric
/// pitch, looking at the scene origin.
#[expect(
    clippy::float_arithmetic,
    reason = "camera geometry is inherently floating point (design §8.2)"
)]
fn camera_transform(yaw: f32) -> Transform {
    let radius = CAMERA_DISTANCE * CAMERA_PITCH.cos();
    let height = CAMERA_DISTANCE * CAMERA_PITCH.sin();
    let translation = Vec3::new(radius * yaw.sin(), height, radius * yaw.cos());
    Transform::from_translation(translation).looking_at(Vec3::ZERO, Vec3::Y)
}

/// Returns the signed shortest rotation from `from` to `to`, in radians.
#[expect(
    clippy::float_arithmetic,
    reason = "angle wrapping is inherently floating point"
)]
fn shortest_angle_delta(from: f32, to: f32) -> f32 {
    use core::f32::consts::{PI, TAU};
    ((to - from + PI).rem_euclid(TAU)) - PI
}

/// Compile-time guard: the baseline viewport height must be positive so
/// the projection is never degenerate.
const _: () = assert!(ZoomBounds::BASE_VIEWPORT_HEIGHT > 0.0);
