//! The one impure rule set: whether the knowledge resources a scene names are
//! actually reachable.
//!
//! Every other rule is a pure function of the document. This one needs the
//! outside world, and it reaches it only through the [`SceneSource`] port, so
//! the whole infrastructure surface of validation is this file. That is what
//! makes the rest of the validator testable without a filesystem.
//!
//! Presence is confirmed by *reading*, because the port deliberately offers no
//! `exists`: a boolean cannot distinguish an absent file from an unreadable
//! one, and an author sent hunting for a file that is sitting right there —
//! merely unreadable — has been actively misled. The bytes are not wasted
//! either: roadmap step 5.1 parses them and phase 8 hashes them.

use crate::{
    scene::{
        knowledge::SceneKnowledge,
        validation::{
            diagnostics::{DiagnosticCode, SceneDiagnostic},
            rules::knowledge::location,
        },
    },
    source::{SceneSource, SceneSourceError},
};

/// Confirms each named resource resolves through `source`.
///
/// Returns one diagnostic per unreachable resource, distinguishing absent from
/// unreadable. An empty list means every resource is there.
#[must_use]
pub fn check(knowledge: &SceneKnowledge, source: &dyn SceneSource) -> Vec<SceneDiagnostic> {
    knowledge
        .sources()
        .enumerate()
        .filter_map(|(ordinal, path)| {
            let error = source.read(path).err()?;
            Some(SceneDiagnostic::resource(
                classify(&error),
                location(ordinal),
                path.to_owned(),
                error.to_string(),
            ))
        })
        .collect()
}

/// Maps a source failure onto its corruption class.
///
/// `RootUnavailable` is reported as unreadable rather than absent, and the
/// distinction matters: running from the wrong working directory otherwise
/// produces the same diagnostic as a genuinely broken scene, and the reader
/// spends the afternoon editing a correct document.
const fn classify(error: &SceneSourceError) -> DiagnosticCode {
    match error {
        SceneSourceError::NotFound(_) => DiagnosticCode::KnowledgeResourceAbsent,
        SceneSourceError::UnsafePath { .. } => DiagnosticCode::KnowledgeResourceUnsafePath,
        SceneSourceError::Unreadable { .. } | SceneSourceError::RootUnavailable(_) => {
            DiagnosticCode::KnowledgeResourceUnreadable
        }
    }
}
