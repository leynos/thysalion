//! The scene loader: bytes in, validated scene or diagnostics out.
//!
//! The loader holds no mutable state. It is a pure function of its source, its
//! policy, and the bytes it is given — no caching, no interior mutability, and
//! nothing a failed load can leave behind. That is what makes design §13,
//! Table 6's "load rejected with diagnostic; previous scene remains active"
//! satisfiable for free rather than by defensive copying: the previous scene is
//! a value the caller still owns, and a failed load never touched it.
//!
//! Non-generic on purpose. [`SceneSource`] is object-safe, and a type parameter
//! on the loader would be a stability surface of its own — every caller naming
//! the loader would also have to name its source type, and `SceneLoader`
//! becomes a Bevy resource at roadmap step 2.1.1.

use std::sync::Arc;

use camino::{Utf8Path, Utf8PathBuf};
use smol_str::SmolStr;

use crate::{
    codec::{CodecError, Encoding, decode_document},
    scene::{
        Scene,
        concept::NamespaceTable,
        validation::{
            Bounds,
            DiagnosticCode,
            DocumentLocation,
            DocumentSection,
            Policy,
            SceneDiagnostic,
            validate,
        },
    },
    source::{SceneSource, SceneSourceError},
};

/// The path reported for a load that came from bytes rather than a file.
const IN_MEMORY: &str = "<in-memory>";

/// A validated scene together with any non-fatal findings.
///
/// The warning channel exists from the outset because retrofitting it would
/// break every caller: `Result<Scene, _>` has nowhere to put "this loads, but a
/// spawn is inside a wall".
#[derive(Debug, Clone)]
#[non_exhaustive]
pub struct LoadedScene {
    /// The validated scene.
    pub scene: Scene,
    /// Non-fatal findings, in document order. `scene-check --strict` promotes
    /// these to failures, and continuous integration runs strict.
    pub warnings: Arc<[SceneDiagnostic]>,
}

impl LoadedScene {
    /// Whether the load produced any advisory findings.
    #[must_use]
    pub fn has_warnings(&self) -> bool { !self.warnings.is_empty() }
}

/// Everything that can stop a scene from loading.
///
/// Three cases, not one. A malformed document yields exactly one problem and
/// *no* document, so nothing can accumulate with it, and only the invalid case
/// can honestly carry a list. `Arc<[_]>` keeps the enum small enough for
/// `clippy::result_large_err` and keeps `Clone` shallow.
#[derive(Debug, Clone, thiserror::Error)]
#[non_exhaustive]
pub enum SceneLoadError {
    /// The bytes are not a well-formed document in the stated encoding.
    #[error("{path}: malformed {encoding} document at {pointer}: {message}")]
    Malformed {
        /// The document that could not be parsed.
        path: Utf8PathBuf,
        /// The encoding the bytes were read as.
        encoding: Encoding,
        /// Structural path to the offending value.
        pointer: String,
        /// The underlying parser's message.
        message: SmolStr,
    },
    /// The document parsed but failed validation.
    ///
    /// Carries every problem found in the earliest failing phase, in document
    /// order.
    #[error("{path}: {} problem(s)", diagnostics.len())]
    Invalid {
        /// The document that failed validation.
        path: Utf8PathBuf,
        /// Every problem found, in document order.
        diagnostics: Arc<[SceneDiagnostic]>,
    },
    /// The source could not supply the bytes.
    #[error("{path}: {source}")]
    Source {
        /// The document that could not be read.
        path: Utf8PathBuf,
        /// Why the source could not supply it.
        #[source]
        source: SceneSourceError,
    },
    /// The path carries no extension this build recognizes.
    ///
    /// Reported rather than guessed: guessing wrong produces a parse error that
    /// blames the document for the caller's mistake.
    #[error("{path}: unrecognized scene extension; expected .json or .msgpack")]
    UnknownEncoding {
        /// The path whose extension could not be recognized.
        path: Utf8PathBuf,
    },
}

impl SceneLoadError {
    /// The diagnostics from a validation failure, or an empty slice.
    #[must_use]
    pub fn diagnostics(&self) -> &[SceneDiagnostic] {
        match self {
            Self::Invalid { diagnostics, .. } => diagnostics,
            Self::Malformed { .. } | Self::Source { .. } | Self::UnknownEncoding { .. } => &[],
        }
    }
}

/// Loads and validates scenes through an injected [`SceneSource`].
pub struct SceneLoader {
    source: Arc<dyn SceneSource>,
    namespaces: NamespaceTable,
    bounds: Bounds,
}

impl core::fmt::Debug for SceneLoader {
    /// Hand-written because [`SceneSource`] is a trait object and cannot derive
    /// `Debug` without making it a supertrait, which would constrain every
    /// adapter for the sake of a diagnostic line.
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("SceneLoader")
            .field("namespaces", &self.namespaces)
            .field("bounds", &self.bounds)
            .finish_non_exhaustive()
    }
}

impl SceneLoader {
    /// A loader over `source`, with the project's namespaces and the shipped
    /// bounds.
    #[must_use]
    pub fn new(source: Arc<dyn SceneSource>) -> Self {
        Self {
            source,
            namespaces: NamespaceTable::default(),
            bounds: Bounds::DEFAULT,
        }
    }

    /// Replaces the namespace table identifiers are checked against.
    #[must_use]
    pub fn with_namespaces(mut self, namespaces: NamespaceTable) -> Self {
        self.namespaces = namespaces;
        self
    }

    /// Replaces the runtime resource bounds.
    #[must_use]
    pub const fn with_bounds(mut self, bounds: Bounds) -> Self {
        self.bounds = bounds;
        self
    }

    /// Loads the document at `path`, inferring the encoding from its extension.
    ///
    /// # Errors
    ///
    /// Returns [`SceneLoadError::UnknownEncoding`] when the extension is not
    /// recognized, [`SceneLoadError::Source`] when the bytes are unavailable,
    /// and otherwise whatever [`SceneLoader::load_bytes`] returns.
    pub fn load(&self, path: &Utf8Path) -> Result<LoadedScene, SceneLoadError> {
        let encoding =
            Encoding::from_path(path).ok_or_else(|| SceneLoadError::UnknownEncoding {
                path: path.to_owned(),
            })?;
        let bytes = self
            .source
            .read(path)
            .map_err(|source| SceneLoadError::Source {
                path: path.to_owned(),
                source,
            })?;
        self.load_named(&bytes, encoding, path)
    }

    /// Loads from bytes in a known encoding.
    ///
    /// # Errors
    ///
    /// Returns [`SceneLoadError::Malformed`] when the bytes do not parse and
    /// [`SceneLoadError::Invalid`] when the document parses but fails
    /// validation.
    pub fn load_bytes(
        &self,
        bytes: &[u8],
        encoding: Encoding,
    ) -> Result<LoadedScene, SceneLoadError> {
        self.load_named(bytes, encoding, Utf8Path::new(IN_MEMORY))
    }

    /// Loads from bytes, reporting failures against `path`.
    fn load_named(
        &self,
        bytes: &[u8],
        encoding: Encoding,
        path: &Utf8Path,
    ) -> Result<LoadedScene, SceneLoadError> {
        let span = tracing::info_span!("scene.load", %path, %encoding);
        let _entered = span.enter();

        let document = decode_document(bytes, encoding).map_err(|error| decoded(error, path))?;
        let policy = Policy::new(&self.namespaces, &self.bounds);
        let validated =
            validate(&document, self.source.as_ref(), &policy).map_err(|diagnostics| {
                SceneLoadError::Invalid {
                    path: path.to_owned(),
                    diagnostics: diagnostics.into(),
                }
            })?;
        Ok(LoadedScene {
            scene: validated.scene,
            warnings: validated.warnings.into(),
        })
    }
}

/// Renders a codec failure as a load failure.
///
/// An unsupported version becomes a *validation* diagnostic rather than a parse
/// error, because that is the class it belongs to: the bytes are well formed and
/// the reader is the one out of date. Reporting it as malformed would send a
/// contributor looking for a syntax error that is not there.
fn decoded(error: CodecError, path: &Utf8Path) -> SceneLoadError {
    match error {
        CodecError::Malformed {
            encoding,
            pointer,
            message,
        } => SceneLoadError::Malformed {
            path: path.to_owned(),
            encoding,
            pointer,
            message,
        },
        CodecError::UnsupportedVersion { found, supported } => SceneLoadError::Invalid {
            path: path.to_owned(),
            diagnostics: Arc::from(vec![SceneDiagnostic::structural(
                DiagnosticCode::UnsupportedVersion,
                DocumentLocation::section(DocumentSection::Version),
                format!(
                    "document version {found} is not readable by this build, which supports \
                     {supported}"
                ),
            )]),
        },
        // An encoder failure on a *decode* path is this crate malfunctioning,
        // not a fault in the bytes. The variant is reused because the enum is
        // the loader's published contract, but the message must not claim the
        // document is malformed: that is precisely the misclassification this
        // function's doc comment warns about, and it would send a contributor
        // hunting a syntax error that does not exist.
        CodecError::Encode { encoding, message } => SceneLoadError::Malformed {
            path: path.to_owned(),
            encoding,
            pointer: String::from("/"),
            message: format!(
                "the {encoding} encoder failed, which is a fault in this build rather than in the \
                 document: {message}"
            )
            .into(),
        },
    }
}
