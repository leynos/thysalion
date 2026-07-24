//! Thysalion logic plane (crate `thysalion-sim`; the design documents call
//! this the *logic* plane — see the plane-to-crate table in ADR-005).
//!
//! Authority row (thysalion-design.md §6.1): authoritative for all derived
//! state — motion resolution, damage, spread, visibility, aggregates; never
//! holds render or asset state, and never performs search.
//!
//! This crate is an empty skeleton until roadmap phase 4 delivers the DBSP
//! circuit scaffold. Eventual heavy dependency: `dbsp`.
