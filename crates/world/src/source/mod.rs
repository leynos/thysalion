//! The driven port through which a scene reaches its resources.
//!
//! The domain never touches the filesystem. Validation is a pure function of
//! the document except for one rule set — knowledge-resource presence — and
//! that rule set reaches the outside world only through this trait.
//!
//! There is deliberately no `exists` method. A boolean cannot distinguish an
//! absent file from an unreadable one, so an adapter would be obliged to
//! discard that distinction and the reader would be told "your scene names a
//! file that is not there" about a file sitting right there, unreadable.
//! [`SceneSourceError`] carries the difference instead, and it costs nothing:
//! phase 5 reads these files anyway, and phase 8 needs their bytes for the
//! content hashes.

mod cap_fs;
mod memory;

use camino::{Utf8Path, Utf8PathBuf};
pub use cap_fs::DirSceneSource;
pub use memory::MemorySceneSource;
use smol_str::SmolStr;

/// Why a source could not supply bytes.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[non_exhaustive]
pub enum SceneSourceError {
    /// No resource exists at that path.
    #[error("no resource at {0}")]
    NotFound(Utf8PathBuf),
    /// The resource exists but could not be read.
    #[error("cannot read {path}: {message}")]
    Unreadable {
        /// The resource that could not be read.
        path: Utf8PathBuf,
        /// The underlying failure.
        message: SmolStr,
    },
    /// The source root itself is unavailable — usually a wrong working
    /// directory, which must never be reported as a missing resource.
    #[error("source root unavailable: {0}")]
    RootUnavailable(SmolStr),
    /// The resource is larger than a source will read into memory.
    ///
    /// A bound rather than a best effort: `rules::resources::check` reads every
    /// declared resource only to classify whether it is reachable, and then
    /// discards the bytes. Without a ceiling, a scene naming one enormous file
    /// inside its own root causes an allocation proportional to that file for
    /// no result anybody uses.
    #[error("{path} is {size} bytes, above the {limit}-byte resource bound")]
    TooLarge {
        /// The resource that was refused.
        path: Utf8PathBuf,
        /// Its size on disk, in bytes.
        size: u64,
        /// The bound it exceeded.
        limit: u64,
    },
    /// The path escapes the scene's directory, or is absolute.
    #[error("unsafe resource path {path}: {reason}")]
    UnsafePath {
        /// The rejected path.
        path: Utf8PathBuf,
        /// Why it was rejected.
        reason: SmolStr,
    },
}

/// The largest knowledge resource any adapter will read into memory.
///
/// Eight mebibytes: comfortably above every TriG file the fixtures ship and far
/// below anything that would trouble a machine running the engine. The bound
/// exists because validation reads each declared resource only to decide
/// whether it resolves, so an unbounded read is an allocation nothing consumes.
pub const MAX_RESOURCE_BYTES: u64 = 8 * 1024 * 1024;

/// Resolves scene-relative resource paths to bytes.
///
/// `Send + Sync` is required now rather than added later: a loader becomes a
/// Bevy resource at roadmap step 2.1.1, and adding a supertrait afterwards is
/// a breaking change.
pub trait SceneSource: Send + Sync {
    /// Reads the resource at `path`, relative to the scene's own directory.
    ///
    /// # Errors
    ///
    /// Returns [`SceneSourceError`] describing why the bytes are unavailable.
    fn read(&self, path: &Utf8Path) -> Result<Vec<u8>, SceneSourceError>;
}

/// Rejects absolute paths and any path that could escape the scene directory.
///
/// This is the port's contract rather than an adapter's business. `cap_std`
/// already stops [`DirSceneSource`] escaping its root, but
/// [`MemorySceneSource`] has no such protection and the trust boundary covers
/// both — a scene document is hand-editable and, from phase 8, will sit beside
/// save archives that users exchange.
///
/// # Errors
///
/// Returns [`SceneSourceError::UnsafePath`] for an absolute path, a path with
/// a parent-directory component, or a path with a Windows prefix.
pub fn check_resource_path(path: &Utf8Path) -> Result<(), SceneSourceError> {
    use camino::Utf8Component;

    let reason = path.components().find_map(|component| match component {
        Utf8Component::ParentDir => Some("contains a parent-directory component"),
        Utf8Component::RootDir | Utf8Component::Prefix(_) => Some("is absolute"),
        Utf8Component::CurDir | Utf8Component::Normal(_) => None,
    });
    if let Some(why) = reason {
        return Err(SceneSourceError::UnsafePath {
            path: path.to_owned(),
            reason: SmolStr::new(why),
        });
    }
    Ok(())
}
