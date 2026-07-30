//! Thysalion state plane (crate `thysalion-world`; the design documents call
//! this the *state* plane — see the plane-to-crate table in ADR-005).
//!
//! Authority row (thysalion-design.md §6.1): authoritative for current
//! component values, the voxel grid, and material fields; never derives rule
//! consequences.
//!
//! This crate is the designated dependency sink for shared state types: other
//! plane crates may depend on `thysalion-world`, never the reverse. Eventual
//! heavy dependency: `bevy` (ECS types), behind a feature so the scene format
//! stays consumable without an engine.
//!
//! # Layout
//!
//! - [`grid`] holds the runtime voxel structure and the coordinate types. It is what phase 2's
//!   mesher and phase 4's pathfinder consume.
//! - [`scene`] holds the authored scene: its wire document, its validated domain form, and the
//!   validation rules that separate the two.
//! - [`codec`] converts a document to and from its two encodings.
//! - [`source`] is the driven port through which a scene reaches its resources, and [`loader`] is
//!   the application service that composes the codec, the port, and validation into one call.
//!
//! `scene` may depend on `grid`; `grid` never depends on `scene` beyond the
//! wire types it converts to and from. The direction matters because the grid
//! outlives the document that produced it.
//!
//! # The scene format in one paragraph
//!
//! A scene is a bounded voxel grid with a palette of voxel types, authored as
//! JSON and shipped as MessagePack from one Rust structure. Voxels are stored
//! as a chunk-keyed payload: each populated chunk carries either a uniform
//! index or a canonical run-length stream in chunk-local Z-major order, and an
//! absent chunk is entirely air. Palette index zero is always air. See
//! [`scene::document`] for the wire types and design §7 for the specification.

pub mod codec;
pub mod grid;
pub mod loader;
pub mod scene;
pub mod source;
