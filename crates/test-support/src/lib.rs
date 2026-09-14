//! Shared test scaffolding for the Thysalion workspace.
//!
//! This crate is the single home of the `rstest-bdd` harness adapters the
//! workspace's behavioural suites run on, and of the deterministic replay
//! envelope of design §14 (invariant I1). It exists because the adapters were
//! previously duplicated — one copy per consuming crate's `tests/` tree — and
//! roadmap step 1.3.1 names their promotion into one crate as its deliverable.
//!
//! # Scope and re-use policy
//!
//! This crate is *test tooling* in
//! ADR 005's sense (`docs/adr-005-workspace-crate-layout.md`), beside
//! `thysalion-harness` rather than beside a plane crate. Three rules follow,
//! and each has a mechanism behind it:
//!
//! - **Dev-dependency edges only.** No plane crate may take this crate as a normal dependency.
//!   `thysalion-world` dev-depends on it while it normally depends on `thysalion-world`; that is
//!   the `serde`/`serde_test` shape and is legal to Cargo, but a *normal*-edge cycle is not and
//!   must never be introduced. `publish = false` and the release build's `-p thysalion` scoping
//!   (pinned by `tests/workflow_shape.rs`) keep it out of the shipped graph.
//! - **The `bevy` half is opt-in.** `BevyHarness` and everything it needs sit behind the
//!   non-default `bevy` feature, which pulls `bevy` and `thysalion-harness`. `thysalion-world`
//!   consumes this crate with default features only, so the state plane does not acquire `bevy`
//!   through a test convenience before ADR 005 stages it in at roadmap 2.1.1.
//! - **Nothing here is production API.** A helper belongs in this crate when two or more test
//!   targets need it and it drives public crate surface. A helper that a single suite uses stays in
//!   that suite's `tests/` tree.
//!
//! # Composition
//!
//! The two adapters are deliberately the same shape — a unit struct
//! implementing [`rstest_bdd_harness::HarnessAdapter`] whose `Context` is
//! built fresh per scenario — so a suite can host either without restructuring
//! its steps. They compose rather than nest: a suite that needs both an app
//! and a loaded scene builds the app through `BevyHarness` and reaches for
//! [`scenes`] to locate the fixtures, as
//! `tests/verification_spine.rs` does.

#[cfg(feature = "bevy")]
mod bevy_harness;
mod loader;
pub mod replay;
pub mod scenes;

#[cfg(feature = "bevy")]
pub use bevy_harness::BevyHarness;
pub use loader::{LoaderHarness, LoaderSession};
