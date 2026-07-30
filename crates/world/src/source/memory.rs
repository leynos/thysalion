//! An in-memory scene source, for tests and for callers that already hold the
//! bytes.

use std::collections::BTreeMap;

use camino::{Utf8Path, Utf8PathBuf};

use super::{SceneSource, SceneSourceError, check_resource_path};

/// A source backed by a map from path to bytes.
///
/// Enforces the same path rules as the filesystem adapter. It has no `cap_std`
/// sandbox to fall back on, and the trust boundary applies to both.
#[derive(Debug, Clone, Default)]
pub struct MemorySceneSource {
    files: BTreeMap<Utf8PathBuf, Vec<u8>>,
}

impl MemorySceneSource {
    /// An empty source.
    #[must_use]
    pub fn new() -> Self { Self::default() }

    /// Adds or replaces a resource.
    pub fn insert(&mut self, path: impl AsRef<Utf8Path>, bytes: Vec<u8>) {
        self.files.insert(path.as_ref().to_owned(), bytes);
    }

    /// Removes a resource, so a scene naming it becomes dangling.
    pub fn remove(&mut self, path: &Utf8Path) { self.files.remove(path); }

    /// The paths this source can supply, in sorted order.
    pub fn paths(&self) -> impl Iterator<Item = &Utf8Path> {
        self.files.keys().map(Utf8PathBuf::as_path)
    }
}

impl SceneSource for MemorySceneSource {
    fn read(&self, path: &Utf8Path) -> Result<Vec<u8>, SceneSourceError> {
        check_resource_path(path)?;
        self.files
            .get(path)
            .cloned()
            .ok_or_else(|| SceneSourceError::NotFound(path.to_owned()))
    }
}
