//! A filesystem scene source, rooted at a capability rather than at the
//! ambient working directory.
//!
//! The root is the scene document's own directory, so a scene naming
//! `knowledge/town.trig` reaches `<root>/knowledge/town.trig` and cannot reach
//! anything above `<root>`. Ambient authority is taken once, by whoever
//! constructs the adapter, and never inside the domain.

use camino::Utf8Path;
use cap_std::fs_utf8::Dir;
use smol_str::SmolStr;

use super::{MAX_RESOURCE_BYTES, SceneSource, SceneSourceError, check_resource_path};

/// A source backed by a capability-scoped directory.
#[derive(Debug)]
pub struct DirSceneSource {
    root: Dir,
    label: SmolStr,
}

impl DirSceneSource {
    /// Wraps an already-opened directory capability.
    ///
    /// `label` names the root in diagnostics. Reporting it matters: running
    /// from the wrong working directory otherwise produces "knowledge resource
    /// absent", which is the same diagnostic a genuinely broken scene
    /// produces.
    #[must_use]
    pub fn new(root: Dir, label: impl Into<SmolStr>) -> Self {
        Self {
            root,
            label: label.into(),
        }
    }

    /// The human-readable name of this source's root.
    #[must_use]
    pub fn label(&self) -> &str { &self.label }
}

impl SceneSource for DirSceneSource {
    fn read(&self, path: &Utf8Path) -> Result<Vec<u8>, SceneSourceError> {
        check_resource_path(path)?;
        // Sized before it is read. `Dir::read` allocates the whole file, and
        // validation discards the bytes once it knows the resource resolves.
        // Zero when the metadata cannot be read: the `read` below then reports
        // the real failure — absent, or unreadable — rather than this check
        // guessing at a size it does not have.
        let size = self.root.metadata(path).map_or(0, |found| found.len());
        if size > MAX_RESOURCE_BYTES {
            return Err(SceneSourceError::TooLarge {
                path: path.to_owned(),
                size,
                limit: MAX_RESOURCE_BYTES,
            });
        }
        self.root.read(path).map_err(|error| {
            if error.kind() == std::io::ErrorKind::NotFound {
                SceneSourceError::NotFound(path.to_owned())
            } else {
                SceneSourceError::Unreadable {
                    path: path.to_owned(),
                    message: SmolStr::new(error.to_string()),
                }
            }
        })
    }
}
