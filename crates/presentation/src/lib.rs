//! Thysalion presentation plane (crate `thysalion-presentation`).
//!
//! Authority row (thysalion-design.md §6.1): authoritative for meshes,
//! lighting textures, and UI; never mutates game state.
//!
//! This step the crate hosts only the pure camera contract of design §8.2
//! (an orthographic-isometric camera with exactly four yaw quadrants and
//! bounded zoom). Meshing, lighting, and atmosphere passes arrive in
//! roadmap phases 2–3. Eventual heavy dependencies: `bevy`,
//! `bevy_voxel_world`.
//!
//! Layering: `presentation → world` (read-only) is the canonical legal
//! edge; presentation must never depend on demo scaffolding
//! (`thysalion-harness`).

pub mod camera;

pub use camera::{Quadrant, ZoomBounds, ZoomBoundsError};
