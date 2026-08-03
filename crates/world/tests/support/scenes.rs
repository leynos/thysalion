//! Locating the compiled fixture scenes on disk.
//!
//! A sibling of `mod.rs` rather than part of it, and *not* re-exported from it.
//! An integration test compiles each `tests/*.rs` as its own crate, so a helper
//! reachable from a module a test declares but never uses is dead code in that
//! crate — and `make test` runs with warnings denied. Each test therefore
//! declares only the helpers it uses, which is the same reason `strategy.rs`
//! sits beside this file.

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
/// Integration tests run with the crate directory as their working directory,
/// so a fixture path relative to the repository root has to be built rather
/// than assumed.
#[must_use]
pub fn repository_root() -> camino::Utf8PathBuf {
    let crate_root = camino::Utf8PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    // Owned before the fallback rather than cloned before the walk, so the
    // copy happens only on the branch that needs one. The borrow of
    // `crate_root` has to end before it can be moved into `unwrap_or`, which
    // is why the `map` is separate — and why `map_or` cannot be used here.
    crate_root
        .parent()
        .and_then(camino::Utf8Path::parent)
        .map(camino::Utf8Path::to_owned)
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
pub fn scene_dir() -> cap_std::fs_utf8::Dir {
    let root = repository_root().join(SCENES);
    match cap_std::fs_utf8::Dir::open_ambient_dir(&root, cap_std::ambient_authority()) {
        Ok(directory) => directory,
        Err(error) => panic!("the fixture scenes must exist at {root}: {error}"),
    }
}
