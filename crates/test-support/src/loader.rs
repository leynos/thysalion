//! `rstest-bdd` harness adapter for scene loading.
//!
//! The context is a plain [`LoaderSession`] rather than a Bevy `App`.
//! `thysalion-world` has no Bevy dependency at this phase and ADR 005 stages
//! one in later, so hosting these scenarios in an app would put the whole
//! render feature set into the state plane's graph to satisfy a test harness.
//! This module is therefore on the crate's *default-feature* path: consumers
//! that want an app opt into the `bevy` feature and
//! `BevyHarness` instead, which is deliberately the same shape.

use std::sync::Arc;

use camino::{Utf8Path, Utf8PathBuf};
use cap_std::fs_utf8::Dir;
use rstest_bdd_harness::{HarnessAdapter, HarnessResult, ScenarioRunRequest};
use thysalion_world::{
    codec::{Encoding, encode_document},
    loader::{LoadedScene, SceneLoadError, SceneLoader},
    scene::{Scene, document::SceneDocument},
    source::{DirSceneSource, MemorySceneSource},
};

/// The document under test, its source, and the outcome of the last load.
pub struct LoaderSession {
    /// Documents and resources the loader can reach.
    pub source: MemorySceneSource,
    /// The document a `Given` step selected, before encoding.
    pub document: Option<SceneDocument>,
    /// The outcome of the most recent `When` step.
    pub outcome: Option<Result<LoadedScene, SceneLoadError>>,
    /// A scene loaded from JSON earlier in the scenario, for comparison.
    pub from_json: Option<Scene>,
    /// The outcome of loading each shipped fixture, by name.
    pub fixtures: Vec<(String, Result<LoadedScene, SceneLoadError>)>,
}

impl LoaderSession {
    /// A session whose source holds the one knowledge resource the fixtures
    /// name, so a scene is missing a resource only when a step removes it.
    fn new() -> Self {
        let mut source = MemorySceneSource::new();
        source.insert("knowledge/minimal.trig", b"# empty for now\n".to_vec());
        Self {
            source,
            document: None,
            outcome: None,
            from_json: None,
            fixtures: Vec::new(),
        }
    }

    /// Loads a shipped fixture through the real filesystem adapter.
    ///
    /// Deliberately the real adapter and the real directory rather than the
    /// in-memory source the other scenarios use. These scenarios exist to prove
    /// the *committed artefacts* load, which an in-memory copy of them could
    /// not: it would prove the constructor agrees with itself.
    ///
    /// # Panics
    ///
    /// Panics when `assets/scenes` is missing, which is a broken checkout.
    pub fn load_fixture(&mut self, name: &str) {
        let directory = crate::scenes::scene_dir();
        let Ok(replica) = directory.try_clone() else {
            panic!("the fixture directory must be cloneable");
        };
        let loader = SceneLoader::new(Arc::new(DirSceneSource::new(
            directory,
            crate::scenes::SCENES,
        )));
        let path = Utf8PathBuf::from(format!("{name}.scene.json"));
        let outcome = loader.load(&path);
        // Cleared before the attempt, not left alone on failure. A scenario
        // that loads two fixtures and sees the second fail would otherwise
        // find the *first* one's document still selected, and every later step
        // would silently assert against the wrong scene rather than reporting
        // that a Given step selected nothing.
        self.document = None;
        if let Ok(loaded) = outcome.as_ref() {
            self.document = Some(loaded.scene.to_document());
            self.adopt_resources(loaded, &replica);
        }
        self.fixtures.push((name.to_owned(), outcome));
    }

    /// Copies a loaded fixture's knowledge resources into the in-memory source.
    ///
    /// A scenario that loads a fixture from disk and then re-encodes it needs
    /// the same resources reachable through the in-memory source, or the
    /// re-encoded document fails the resource check and the scenario reports a
    /// missing TriG file when what it was testing was the encoding. Copying
    /// them keeps the round-trip scenario about the round trip.
    fn adopt_resources(&mut self, loaded: &LoadedScene, directory: &Dir) {
        for source in loaded.scene.knowledge().sources() {
            let Ok(bytes) = directory.read(source) else {
                panic!("a loaded fixture's resources must still be readable: {source}");
            };
            self.source.insert(source, bytes);
        }
    }

    /// The document a `Given` step selected.
    ///
    /// # Panics
    ///
    /// Panics when no `Given` step ran, which is a malformed scenario rather
    /// than a runtime condition.
    #[must_use]
    pub fn document(&self) -> &SceneDocument {
        // `expect` rather than a `let ... else` would be shorter, but the
        // workspace allows it only inside `#[test]` functions, and a
        // step-definition helper is neither.
        let Some(document) = self.document.as_ref() else {
            panic!("a Given step must select a document");
        };
        document
    }

    /// Encodes the selected document and loads it.
    pub fn load(&mut self, encoding: Encoding) {
        let document = self.document().clone();
        self.load_other_as(&document, encoding);
    }

    /// Loads a document other than the selected one, leaving the selection.
    pub fn load_other(&mut self, document: &SceneDocument) {
        self.load_other_as(document, Encoding::Json);
    }

    /// Encodes `document` and loads it through a fresh loader.
    ///
    /// A fresh loader each time, deliberately: the loader holds no mutable
    /// state, so reusing one would prove nothing that this does not, and
    /// constructing one per load is what a caller actually does.
    ///
    /// # Panics
    ///
    /// Panics when the document will not encode, which is a broken fixture
    /// rather than a runtime condition.
    fn load_other_as(&mut self, document: &SceneDocument, encoding: Encoding) {
        let bytes = match encode_document(document, encoding) {
            Ok(bytes) => bytes,
            Err(error) => panic!("the fixture document must encode: {error}"),
        };
        let loader = SceneLoader::new(Arc::new(self.source.clone()));
        self.outcome = Some(loader.load_bytes(&bytes, encoding));
    }

    /// The loaded scene, or a panic naming what actually happened.
    ///
    /// # Panics
    ///
    /// Panics when the last load failed or no load ran.
    #[must_use]
    pub fn loaded(&self) -> &LoadedScene {
        match self.outcome.as_ref() {
            Some(Ok(loaded)) => loaded,
            Some(Err(error)) => panic!("expected the scene to load, but: {error}"),
            None => panic!("a When step must load a scene"),
        }
    }

    /// The diagnostics from a failed load.
    ///
    /// # Panics
    ///
    /// Panics when the last load succeeded or no load ran.
    #[must_use]
    pub fn diagnostics(&self) -> &[thysalion_world::scene::validation::SceneDiagnostic] {
        match self.outcome.as_ref() {
            Some(Err(SceneLoadError::Invalid { diagnostics, .. })) => diagnostics,
            Some(Err(other)) => panic!("expected validation to fail, but: {other}"),
            Some(Ok(_)) => panic!("expected the scene to fail loading, but it succeeded"),
            None => panic!("a When step must load a scene"),
        }
    }

    /// Removes a resource, so a scene that names it becomes dangling.
    pub fn forget(&mut self, path: &str) { self.source.remove(Utf8Path::new(path)); }
}

/// Runs each scenario against a fresh loader session.
#[derive(Default)]
pub struct LoaderHarness;

impl HarnessAdapter for LoaderHarness {
    type Context = LoaderSession;

    fn run<T>(&self, request: ScenarioRunRequest<'_, Self::Context, T>) -> HarnessResult<T> {
        Ok(request.run(LoaderSession::new()))
    }
}

#[cfg(test)]
mod tests {
    //! Regression coverage for the session's own state transitions, as
    //! distinct from the loading behaviour the behavioural suite in
    //! `crates/world/tests/loading/` covers. What is tested here is what a
    //! *scenario* would otherwise observe only indirectly: that a failed
    //! fixture load leaves no stale selection behind for the next step to
    //! assert against.

    use super::LoaderSession;

    #[test]
    fn a_failed_fixture_load_clears_the_previously_selected_document() {
        // The scenario this guards: two fixture loads in one session, the
        // second failing. Without the clear, `document()` would hand back the
        // first fixture's document and every later assertion would quietly
        // test the wrong scene.
        let mut session = LoaderSession::new();
        session.load_fixture("bare-cell");
        assert!(
            session.document.is_some(),
            "a shipped fixture must select a document"
        );

        session.load_fixture("no-such-fixture");
        assert!(
            session.document.is_none(),
            "a failed load must leave no document selected"
        );
        assert_eq!(
            session.fixtures.len(),
            2,
            "both attempts must be recorded, successful or not"
        );
    }
}
