//! `demo-empty`: the degenerate first capability demonstration (roadmap
//! task 1.1.2). Opens a window, renders a ground plane under a
//! directional light, and hands everything else — camera, input,
//! overlay, screenshots — to the shared demo harness.
//!
//! Run with `make demo` (or `cargo run --bin demo-empty`). Key bindings
//! are defined in `thysalion_harness::input::KEY_BINDINGS`.
#![cfg_attr(coverage_nightly, feature(coverage_attribute))]
#![cfg_attr(coverage_nightly, coverage(off))]

use bevy::prelude::*;
use thysalion_harness::{DemoHarnessPlugin, HarnessConfig};

/// Ground plane side length, in world units.
const GROUND_SIZE: f32 = 24.0;

fn main() -> AppExit {
    let config = HarnessConfig::new("demo-empty");
    App::new()
        .add_plugins(DefaultPlugins.set(WindowPlugin {
            primary_window: Some(Window {
                title: config.window_title.clone(),
                ..Window::default()
            }),
            ..WindowPlugin::default()
        }))
        .add_plugins(DemoHarnessPlugin::new(config))
        .add_systems(Startup, spawn_ground)
        .run()
}

/// Spawns the ground plane and a warm key light.
fn spawn_ground(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    commands.spawn((
        Mesh3d(meshes.add(Plane3d::default().mesh().size(GROUND_SIZE, GROUND_SIZE))),
        MeshMaterial3d(materials.add(StandardMaterial {
            base_color: Color::srgb(0.35, 0.42, 0.3),
            perceptual_roughness: 0.9,
            ..StandardMaterial::default()
        })),
    ));
    commands.spawn((
        DirectionalLight {
            illuminance: 12_000.0,
            shadow_maps_enabled: true,
            ..DirectionalLight::default()
        },
        Transform::from_xyz(12.0, 18.0, 8.0).looking_at(Vec3::ZERO, Vec3::Y),
    ));
}
