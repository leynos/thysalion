//! `demo-empty`: the degenerate first capability demonstration (roadmap
//! task 1.1.2). Opens a window, renders a ground plane under a
//! directional light, and hands everything else — camera, input,
//! overlay, screenshots — to the shared demo harness.
//!
//! Run with `make demo` (or `cargo run -p thysalion-demos --features
//! demo-empty --bin demo-empty`). Key bindings
//! are defined in `thysalion_harness::input::KEY_BINDINGS`.
#![cfg_attr(coverage_nightly, feature(coverage_attribute))]
#![cfg_attr(coverage_nightly, coverage(off))]

use bevy::prelude::*;
use thysalion_harness::{DemoHarnessPlugin, HarnessConfig};

/// Ground plane side length, in world units.
const GROUND_SIZE: f32 = 24.0;

/// Ground albedo: a muted grass-green, chosen to read clearly under the
/// key light without competing with future scene content.
const GROUND_COLOR: Color = Color::srgb(0.35, 0.42, 0.3);

/// Ground roughness: near-matte, so the plane shows lighting direction
/// rather than specular highlights.
const GROUND_ROUGHNESS: f32 = 0.9;

/// Key-light intensity, in lux.
const LIGHT_ILLUMINANCE: f32 = 12_000.0;

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
            base_color: GROUND_COLOR,
            perceptual_roughness: GROUND_ROUGHNESS,
            ..StandardMaterial::default()
        })),
    ));
    commands.spawn((
        DirectionalLight {
            illuminance: LIGHT_ILLUMINANCE,
            shadow_maps_enabled: true,
            ..DirectionalLight::default()
        },
        Transform::from_xyz(12.0, 18.0, 8.0).looking_at(Vec3::ZERO, Vec3::Y),
    ));
}

#[cfg(test)]
mod tests {
    //! Startup-scene smoke test for `demo-empty`.
    //!
    //! `spawn_ground` is a plain startup system, so it runs in a bare app
    //! holding only the asset resources it writes to: no window, renderer,
    //! or graphics device is involved, and `main` is untouched.

    use bevy::{asset::AssetPlugin, camera::primitives::MeshAabb as _};

    use super::*;

    /// Runs the scene setup once in a headless app and returns it.
    fn scene_app() -> App {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins)
            .add_plugins(AssetPlugin::default())
            .init_asset::<Mesh>()
            .init_asset::<StandardMaterial>()
            .add_systems(Startup, spawn_ground);
        app.update();
        app
    }

    #[test]
    fn startup_spawns_one_lit_ground_plane() {
        let mut app = scene_app();

        let grounds: Vec<(Mesh3d, MeshMaterial3d<StandardMaterial>)> = app
            .world_mut()
            .query::<(&Mesh3d, &MeshMaterial3d<StandardMaterial>)>()
            .iter(app.world())
            .map(|(mesh, material)| (mesh.clone(), material.clone()))
            .collect();
        assert_eq!(
            grounds.len(),
            1,
            "startup must spawn exactly one ground entity"
        );

        let lights: Vec<DirectionalLight> = app
            .world_mut()
            .query::<&DirectionalLight>()
            .iter(app.world())
            .copied()
            .collect();
        assert_eq!(
            lights.len(),
            1,
            "startup must spawn exactly one directional light"
        );

        let Some(light) = lights.first() else {
            panic!("the directional light was asserted present above");
        };
        assert!(
            (light.illuminance - LIGHT_ILLUMINANCE).abs() < f32::EPSILON,
            "key-light illuminance must stay {LIGHT_ILLUMINANCE}, got {}",
            light.illuminance
        );
        assert!(
            light.shadow_maps_enabled,
            "the key light must cast shadows, or the plane reads flat"
        );

        let Some((mesh_handle, material_handle)) = grounds.first() else {
            panic!("the ground entity was asserted present above");
        };
        let meshes = app.world().resource::<Assets<Mesh>>();
        let mesh = meshes
            .get(&mesh_handle.0)
            .expect("the ground mesh asset must exist");
        // Assert the geometry, not merely the handle: a handle check still
        // passes if the plane is rebuilt at the wrong dimensions.
        let bounds = mesh
            .compute_aabb()
            .expect("the ground mesh must have positions to bound");
        let half = GROUND_SIZE / 2.0;
        assert!(
            (bounds.half_extents.x - half).abs() < f32::EPSILON
                && (bounds.half_extents.z - half).abs() < f32::EPSILON,
            "the ground must span {GROUND_SIZE} world units in x and z, got half-extents {:?}",
            bounds.half_extents
        );
        assert!(
            bounds.half_extents.y.abs() < f32::EPSILON,
            "the ground must be a flat plane, got y half-extent {}",
            bounds.half_extents.y
        );
        let materials = app.world().resource::<Assets<StandardMaterial>>();
        let material = materials
            .get(&material_handle.0)
            .expect("the ground material asset must exist");
        assert_eq!(
            material.base_color, GROUND_COLOR,
            "the ground must keep its intended albedo"
        );
        assert!(
            (material.perceptual_roughness - GROUND_ROUGHNESS).abs() < f32::EPSILON,
            "the ground must stay near-matte, got {}",
            material.perceptual_roughness
        );
    }
}
