//! The validated knowledge section: the scene's named graph and the TriG
//! resources that populate it.
//!
//! Validation here confirms *shape and reachability only*. The graph
//! identifier is parsed and checked against the injected namespace table, and
//! each source is confirmed to be a safe relative path that the scene source
//! can supply. Nothing parses a TriG file: that is the knowledge plane's work
//! at roadmap step 5.1, and the dependency edge runs `knowledge -> world`.
//!
//! A domain type, deriving no `serde` traits.

use camino::{Utf8Path, Utf8PathBuf};

use crate::scene::concept::ConceptIri;

/// The scene's named graph and the resources that populate it.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct SceneKnowledge {
    graph: ConceptIri,
    sources: Vec<Utf8PathBuf>,
}

impl SceneKnowledge {
    /// Wraps an already-validated graph identifier and source list.
    ///
    /// Crate-private: the only caller is
    /// [`crate::scene::validation::rules::knowledge`].
    pub(crate) const fn from_validated(graph: ConceptIri, sources: Vec<Utf8PathBuf>) -> Self {
        Self { graph, sources }
    }

    /// The identifier of the scene's named graph.
    #[must_use]
    pub const fn graph(&self) -> &ConceptIri { &self.graph }

    /// The TriG resources, relative to the scene document's own directory.
    pub fn sources(&self) -> impl Iterator<Item = &Utf8Path> {
        self.sources.iter().map(Utf8PathBuf::as_path)
    }

    /// How many resources the scene names.
    #[must_use]
    pub const fn source_count(&self) -> usize { self.sources.len() }
}
