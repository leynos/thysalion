//! The authored scene: its wire document today, its validated domain form
//! from roadmap task 1.2.2.
//!
//! The split between the two is the mechanism that makes loading
//! all-or-nothing. Document types derive `serde` traits and are deliberately
//! permissive; domain types derive none and are constructible only through
//! validation. A derived `Deserialize` on a domain type would be a public
//! constructor reaching every private field, which is a hole straight through
//! the validation gate — and `#[non_exhaustive]` does not close it, being
//! documented as not restricting deserialization.

pub mod document;

pub use crate::grid::VoxelIndex;
