//! Locating the committed fixture scenes on disk.
//!
//! Promoted here from `crates/world/tests/support/scenes.rs` at roadmap step
//! 1.3.1, because [`crate::LoaderSession`] reaches for the fixture directory
//! and a promoted adapter cannot depend on a module in the suite that consumes
//! it. Every suite that reads a committed fixture goes through this module, so
//! there is one statement of where the fixtures live and one place that takes
//! ambient filesystem authority.

use camino::{Utf8Path, Utf8PathBuf};
use cap_std::fs_utf8::Dir;

/// Where the compiled fixture scenes live, relative to the repository root.
pub const SCENES: &str = "assets/scenes";

/// Every fixture the repository ships.
///
/// The three named scenes are sized per design §7.1, Table 1. `bare-cell` is
/// the deliberately ugly fourth: the other three all derive from one table, so
/// whatever they happen to share would otherwise become an unstated engine
/// assumption that surfaces at phase 6 or 9.
pub const FIXTURE_NAMES: &[&str] = &[
    "bare-cell",
    "keep-interior",
    "market-town-block",
    "swamp-fragment",
];

/// The repository root, two levels above this crate.
///
/// Tests run with their crate's directory as the working directory, so a
/// fixture path relative to the repository root has to be built rather than
/// assumed. Every member crate sits at `crates/<name>`, so two parents of this
/// crate's manifest directory is the root for consumers of this crate too.
#[must_use]
pub fn repository_root() -> Utf8PathBuf {
    let crate_root = Utf8PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    // Owned before the fallback rather than cloned before the walk, so the
    // copy happens only on the branch that needs one. The borrow of
    // `crate_root` has to end before it can be moved into `unwrap_or`, which
    // is why the `map` is separate — and why `map_or` cannot be used here.
    crate_root
        .parent()
        .and_then(Utf8Path::parent)
        .map(Utf8Path::to_owned)
        .unwrap_or(crate_root)
}

/// Opens the compiled fixture directory as a capability.
///
/// Ambient authority is taken here, once, rather than by every call site: the
/// point of `cap_std` is that a reader can see the whole filesystem surface a
/// module touches by reading one function (AGENTS.md).
///
/// # Panics
///
/// Panics when `assets/scenes` is missing, which is a broken checkout or a
/// tree nobody has run `make scenes` in.
#[must_use]
pub fn scene_dir() -> Dir {
    let root = repository_root().join(SCENES);
    match Dir::open_ambient_dir(&root, cap_std::ambient_authority()) {
        Ok(directory) => directory,
        Err(error) => panic!("the fixture scenes must exist at {root}: {error}"),
    }
}
