//! `Thysalion` composition root.
//!
//! This package is the phase-9 integrated game binary and the workspace's
//! composition root: the one place that may depend on all four plane
//! crates (`thysalion-world`, `thysalion-sim`, `thysalion-knowledge`,
//! `thysalion-presentation`) and own cross-plane wiring. It hosts no plane
//! logic itself, and release artefacts are built from this package alone
//! (`-p thysalion`); demo binaries live in `thysalion-demos` and never
//! enter the release graph. See ADR-005 for the workspace layout.

// TODO: Replace this stub when application logic moves behind the executable.
/// Returns the generated application greeting.
///
/// # Examples
///
/// ```
/// use thysalion::greet;
///
/// assert_eq!(greet(), "Hello from Thysalion!");
/// ```
#[must_use]
pub const fn greet() -> &'static str { "Hello from Thysalion!" }
