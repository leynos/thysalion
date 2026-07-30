//! The validation rules, one module per document subject.
//!
//! Every rule set is a pure function of the document except [`resources`],
//! which is the only place infrastructure is reachable from validation. Keeping
//! that surface to one named file is what lets the rest be tested without a
//! filesystem, and what makes an accidental widening visible in review.
//!
//! [`header`] is phase one; [`voxels`] spans phases two and three; the rest are
//! phase three. [`super`] orchestrates them.

pub mod entities;
pub mod header;
pub mod knowledge;
pub mod resources;
pub mod voxels;
