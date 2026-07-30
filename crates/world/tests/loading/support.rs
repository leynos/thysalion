//! `rstest-bdd` harness adapter for scene loading.
//!
//! The context is a plain [`LoaderSession`] rather than a Bevy `App`.
//! `thysalion-world` has no Bevy dependency at this phase and ADR 005 stages
//! one in later, so hosting these scenarios in an app would put the whole
//! render feature set into the state plane's graph to satisfy a test harness.
//! The adapter is deliberately the same *shape* as the Bevy one in
//! `crates/harness/tests/headless/support.rs`, so roadmap step 1.3.1 can
//! promote both into one shared test-support crate — the promotion point the
//! developers' guide already names.

use std::sync::Arc;

use camino::Utf8Path;
use rstest_bdd_harness::{HarnessAdapter, HarnessResult, ScenarioRunRequest};
use thysalion_world::{
    codec::{Encoding, encode_document},
    loader::{LoadedScene, SceneLoadError, SceneLoader},
    scene::{Scene, document::SceneDocument},
    source::MemorySceneSource,
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
        }
    }

    /// The document a `Given` step selected.
    ///
    /// # Panics
    ///
    /// Panics when no `Given` step ran, which is a malformed scenario rather
    /// than a runtime condition.
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
