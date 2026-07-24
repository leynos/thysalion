//! Thysalion capability demonstrations (crate `thysalion-demos`).
//!
//! Hosts one binary per capability demonstration (`src/bin/<name>.rs`),
//! starting with `demo-empty` (roadmap task 1.1.2). Demos depend on
//! `thysalion-harness` for shared scaffolding; each demo's heavy
//! dependencies must be declared `optional = true` behind a per-demo
//! feature with `required-features` on the `[[bin]]` target, so no demo
//! pays for another demo's dependency graph (see ADR-005).
//!
//! Demo binaries must never enter the release build graph: the release
//! workflow builds `-p thysalion` only.
