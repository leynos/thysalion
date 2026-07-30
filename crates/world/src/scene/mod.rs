//! The authored scene: its wire document, and the validated domain form a load
//! yields.
//!
//! The split between the two is the mechanism that makes loading
//! all-or-nothing. Document types derive `serde` traits and are deliberately
//! permissive; domain types derive none and are constructible only through
//! validation. A derived `Deserialize` on a domain type would be a public
//! constructor reaching every private field, which is a hole straight through
//! the validation gate — and `#[non_exhaustive]` does not close it, being
//! documented as not restricting deserialization.
//!
//! Not every type needs two forms. A closed enum such as
//! [`document::MaterialClass`] has no private field to protect and no invariant
//! to enforce, so it is *shared* vocabulary and carries no `Document` suffix. A
//! second form would be two names for one set of values, and a conversion
//! function that can only be the identity. The suffix therefore marks a real
//! distinction — [`document::VoxelTypeDocument`] against [`palette::VoxelType`],
//! [`document::EmissionDocument`] against [`palette::LightEmission`] — rather
//! than decorating every type in the module.
//!
//! # Layout
//!
//! - [`document`] holds the wire types.
//! - [`palette`], [`entities`], [`knowledge`], and [`concept`] hold the validated domain types.
//! - [`validation`] holds the rules and the phase orchestration, and is the only route to a
//!   [`Scene`].

mod aggregate;
pub mod concept;
pub mod document;
pub mod entities;
pub mod knowledge;
pub mod palette;
pub mod validation;

pub use aggregate::{Scene, SceneContentHash};

pub use crate::grid::VoxelIndex;
