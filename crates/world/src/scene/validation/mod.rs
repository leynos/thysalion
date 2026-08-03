//! Load-time validation: the only route from a document to a [`Scene`].
//!
//! Validation runs in three ordered phases, and the ordering is a *correctness*
//! property rather than an optimization.
//!
//! 1. **Header.** Version, dimensions, chunk-size alignment, and the declared collection sizes, all
//!    against [`Bounds`]. Nothing in this phase allocates in proportion to a declared quantity.
//! 2. **Bounded decode.** Only once the header is sound is the voxel payload decoded, into an
//!    allocation the header has already bounded.
//! 3. **Semantic rules.** Palette coherence, run canonicality, spawn placement, prototype
//!    resolution, identifier syntax, and — in the one impure rule set — knowledge-resource
//!    presence.
//!
//! Diagnostics accumulate within a phase and a failing phase stops the next, so
//! the promise is *every problem in the earliest failing phase*. An unqualified
//! "every problem" is not implementable: it would oblige the loader to fully
//! decode a document declaring a four-billion-entry palette so that the later
//! rules could also run, which is the resource exhaustion the bounds exist to
//! refuse.
//!
//! Phases two and three are reported together, because the rules in them share
//! one bound: the header has already capped what they can allocate. Within
//! that, a failure in one section does not silence another — a scene with a bad
//! palette index *and* a stray spawn reports both.

pub mod bounds;
pub mod diagnostics;
pub mod report;
pub mod rules;

pub use bounds::Bounds;
pub use diagnostics::{
    DiagnosticCode,
    DocumentLocation,
    DocumentSection,
    SceneDiagnostic,
    VoxelSite,
};
pub use report::{Report, ReportFailure, SceneStats, Strictness};

use crate::{
    grid::VoxelGrid,
    scene::{
        aggregate::{Scene, SceneParts},
        concept::NamespaceTable,
        document::SceneDocument,
        entities::Entities,
        knowledge::SceneKnowledge,
        palette::Palette,
        validation::rules::header::Geometry,
    },
    source::SceneSource,
};

/// A scene that passed validation, together with any advisory findings.
#[derive(Debug, Clone)]
#[non_exhaustive]
pub struct Validated {
    /// The scene.
    pub scene: Scene,
    /// Non-fatal findings, in reporting order.
    pub warnings: Vec<SceneDiagnostic>,
}

/// The policy a document is validated against.
///
/// A struct rather than three parameters because it is threaded through every
/// phase, and because the namespace table is knowledge-plane policy that this
/// crate must not bake in: phase 5 adding a namespace must not mean editing the
/// state plane.
#[derive(Debug, Clone)]
#[non_exhaustive]
pub struct Policy<'a> {
    /// The prefixes a scene's identifiers may use.
    pub namespaces: &'a NamespaceTable,
    /// The runtime resource bounds.
    pub bounds: &'a Bounds,
}

impl<'a> Policy<'a> {
    /// A policy with the given namespace table and bounds.
    #[must_use]
    pub const fn new(namespaces: &'a NamespaceTable, bounds: &'a Bounds) -> Self {
        Self { namespaces, bounds }
    }
}

/// Validates `document`, resolving its resources through `source`.
///
/// # Errors
///
/// Returns every problem found in the earliest failing phase, sorted into
/// document order. No partially constructed scene is ever returned.
pub fn validate(
    document: &SceneDocument,
    source: &dyn SceneSource,
    policy: &Policy<'_>,
) -> Result<Validated, Vec<SceneDiagnostic>> {
    let geometry = tracing::info_span!("scene.validate.header")
        .in_scope(|| rules::header::check(document, policy.bounds))
        .map_err(sorted)?;

    let sections = tracing::info_span!("scene.validate.sections")
        .in_scope(|| check_sections(document, source, policy, geometry))?;

    let scene = Scene::from_validated(SceneParts {
        name: document.name.clone(),
        palette: sections.palette,
        voxels: sections.voxels,
        entities: sections.entities,
        lighting: document.lighting.clone(),
        knowledge: sections.knowledge,
    });
    let warnings = sorted(rules::entities::warnings(
        scene.entities(),
        scene.voxels(),
        scene.palette(),
    ));
    Ok(Validated { scene, warnings })
}

/// Everything phases two and three produce when they all pass.
struct Sections {
    palette: Palette,
    voxels: VoxelGrid,
    entities: Entities,
    knowledge: SceneKnowledge,
}

/// Runs the decode and the semantic rules, accumulating across sections.
///
/// Each section is attempted independently so that one fault does not mask
/// another in a different section. The two dependent edges are honoured: voxel
/// indices cannot be resolved without a palette, and resource presence cannot
/// be checked without the paths the knowledge rules validate, so each is
/// skipped when its input is absent rather than checked against a guess.
fn check_sections(
    document: &SceneDocument,
    source: &dyn SceneSource,
    policy: &Policy<'_>,
    geometry: Geometry,
) -> Result<Sections, Vec<SceneDiagnostic>> {
    let mut problems = Vec::new();

    let checked_palette = collect(
        Palette::from_document(&document.palette, policy.namespaces),
        &mut problems,
    );
    let decoded_voxels = checked_palette.as_ref().and_then(|resolved| {
        collect(
            rules::voxels::decode(&document.voxels, geometry, resolved),
            &mut problems,
        )
    });
    let resolved_entities = collect(
        rules::entities::check(
            &document.entities,
            geometry,
            policy.namespaces,
            policy.bounds,
        ),
        &mut problems,
    );
    let checked_knowledge = collect(
        rules::knowledge::check(&document.knowledge, policy.namespaces),
        &mut problems,
    );
    if let Some(reachable) = checked_knowledge.as_ref() {
        problems.extend(rules::resources::check(reachable, source));
    }

    match (
        checked_palette,
        decoded_voxels,
        resolved_entities,
        checked_knowledge,
    ) {
        (Some(palette), Some(voxels), Some(entities), Some(knowledge)) if problems.is_empty() => {
            Ok(Sections {
                palette,
                voxels,
                entities,
                knowledge,
            })
        }
        _ => Err(sorted(problems)),
    }
}

/// Keeps the value on success and the diagnostics on failure.
fn collect<T>(
    outcome: Result<T, Vec<SceneDiagnostic>>,
    problems: &mut Vec<SceneDiagnostic>,
) -> Option<T> {
    match outcome {
        Ok(value) => Some(value),
        Err(found) => {
            problems.extend(found);
            None
        }
    }
}

/// Sorts diagnostics into document order.
///
/// Without an explicit order the list arrives in rule-registration order, the
/// snapshots pin that accident, and adding a rule churns unrelated snapshots.
/// The sort is stable, so two diagnostics at one location keep the order the
/// rule emitted them in.
fn sorted(mut problems: Vec<SceneDiagnostic>) -> Vec<SceneDiagnostic> {
    problems.sort_by_key(SceneDiagnostic::sort_key);
    problems
}
