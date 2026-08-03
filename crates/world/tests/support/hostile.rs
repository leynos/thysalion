//! Shared scaffolding for the hostile-input suites.
//!
//! Split out of `hostile_inputs.rs` when that file passed the 400-line cap
//! AGENTS.md sets, and shared with `hostile_encodings.rs`: both suites need the
//! same wall-clock bound and the same load helpers, and two copies of a timing
//! bound are two places for it to drift.

use std::time::{Duration, Instant};

/// The wall-clock bound every hostile input must clear.
///
/// Generous by design. A header check that refuses a declared quantity before
/// allocating finishes in microseconds; a loader that materializes what the
/// document claims does not finish at all. Anything between the two is a
/// regression worth a failing test either way.
pub const BOUND: Duration = Duration::from_secs(5);

/// Runs `load` and asserts it returned within [`BOUND`].
///
/// # Panics
///
/// Panics when the load overruns, naming the input so a failure says which
/// attack got through.
pub fn within_bound<T>(what: &str, load: impl FnOnce() -> T) -> T {
    let started = Instant::now();
    let outcome = load();
    let elapsed = started.elapsed();
    assert!(
        elapsed < BOUND,
        "{what}: took {elapsed:?}, which is over the {BOUND:?} bound — the loader is \
         materializing what the document declares rather than refusing it"
    );
    outcome
}
