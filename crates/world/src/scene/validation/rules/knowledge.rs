//! Phase three for the knowledge section: the named-graph identifier and the
//! shape of each resource path.
//!
//! Pure. Whether a resource is actually *there* needs the scene source, and
//! that check lives in [`super::resources`] — the one rule set in this crate
//! that reaches infrastructure.

use camino::{Utf8Path, Utf8PathBuf};

use crate::{
    scene::{
        concept::{ConceptIri, NamespaceTable},
        document::KnowledgeDocument,
        knowledge::SceneKnowledge,
        validation::diagnostics::{
            DiagnosticCode,
            DocumentLocation,
            DocumentSection,
            SceneDiagnostic,
            describe_concept_problem,
        },
    },
    source::check_resource_path,
};

/// Checks the knowledge section and resolves it into its domain form.
///
/// # Errors
///
/// Returns every problem found: a malformed graph identifier, and one entry per
/// unsafe source path.
pub fn check(
    document: &KnowledgeDocument,
    namespaces: &NamespaceTable,
) -> Result<SceneKnowledge, Vec<SceneDiagnostic>> {
    let mut problems = Vec::new();
    let whole = DocumentLocation::section(DocumentSection::Knowledge);

    let parsed_graph = match ConceptIri::parse(&document.graph, namespaces) {
        Ok(iri) => Some(iri),
        Err(problem) => {
            problems.push(SceneDiagnostic::structural(
                DiagnosticCode::SceneGraphIriInvalid,
                whole,
                describe_concept_problem(&document.graph, &problem),
            ));
            None
        }
    };

    let mut sources = Vec::with_capacity(document.sources.len());
    for (ordinal, raw) in document.sources.iter().enumerate() {
        let path = Utf8Path::new(raw.as_str());
        match check_resource_path(path) {
            Ok(()) => sources.push(path.to_owned()),
            Err(error) => problems.push(SceneDiagnostic::resource(
                DiagnosticCode::KnowledgeResourceUnsafePath,
                location(ordinal),
                Utf8PathBuf::from(raw.as_str()),
                error.to_string(),
            )),
        }
    }

    match (parsed_graph, problems.is_empty()) {
        (Some(graph), true) => Ok(SceneKnowledge::from_validated(graph, sources)),
        _ => Err(problems),
    }
}

/// The document location of knowledge source `ordinal`.
pub(super) fn location(ordinal: usize) -> DocumentLocation {
    DocumentLocation::new(
        DocumentSection::Knowledge,
        u32::try_from(ordinal).unwrap_or(u32::MAX),
    )
}
