//! Thysalion state plane (crate `thysalion-world`; the design documents call
//! this the *state* plane — see the plane-to-crate table in ADR-005).
//!
//! Authority row (thysalion-design.md §6.1): authoritative for current
//! component values, the voxel grid, and material fields; never derives rule
//! consequences.
//!
//! This crate is the designated dependency sink for shared state types:
//! other plane crates may depend on `thysalion-world`, never the reverse.
//! It is an empty skeleton until roadmap step 1.2 delivers the voxel type
//! registry and scene document model. Eventual heavy dependency: `bevy`
//! (ECS types).
