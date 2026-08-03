//! Runtime resource bounds.
//!
//! These are correctness requirements, not hardening. A scene document is
//! hand-editable and, from phase 8, will sit beside save archives that users
//! exchange; design §13, Table 6 already commits to "load rejected with
//! diagnostic; previous scene remains active" as a *player-facing* path. The
//! loader is therefore on the untrusted side of the boundary and must
//! terminate, without panicking and within bounded memory, on any input.
//!
//! The bounds are checked in phase one, before anything sizes an allocation
//! from a declared quantity.

/// The limits a document must respect.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub struct Bounds {
    /// The most chunk entries a document may declare.
    pub max_chunks: usize,
    /// The most runs one chunk may declare.
    pub max_runs_per_chunk: usize,
    /// The deepest a prototype chain may go before it is refused.
    ///
    /// Cycle detection alone does not save a resolver from an *acyclic* chain
    /// thousands deep, so resolution is iterative against a worklist and this
    /// bound, never recursive: a stack overflow is a signal rather than a
    /// `Result`, and uncatchable.
    pub max_prototype_depth: usize,
    /// The most memory a decoded grid may occupy, in bytes.
    pub max_decoded_bytes: u64,
}

impl Bounds {
    /// The defaults this crate ships, matching the 1.2 execution plan.
    pub const DEFAULT: Self = Self {
        max_chunks: 16_777_216,
        max_runs_per_chunk: 65_536,
        max_prototype_depth: 32,
        max_decoded_bytes: 64 * 1024 * 1024,
    };
}

impl Default for Bounds {
    fn default() -> Self { Self::DEFAULT }
}
