//! Phase three for the entity section: spawn placement, name uniqueness,
//! prototype resolution, and concept syntax.
//!
//! Prototype chains are walked iteratively against an explicit worklist and a
//! depth bound, never recursively. Cycle detection alone does not save a
//! recursive resolver from an *acyclic* chain ten thousand deep, and the
//! resulting stack overflow is a signal rather than a `Result` — uncatchable,
//! and a direct contradiction of the never-panics contract in precisely the
//! place a hand-edited file reaches.

use std::collections::{BTreeMap, BTreeSet};

use smol_str::SmolStr;

use crate::{
    grid::{Extent, VoxelGrid, VoxelPos},
    scene::{
        concept::{ConceptIri, NamespaceTable},
        document::{EntitiesDocument, PrototypeDocument, SpawnDocument},
        entities::{Entities, SpawnPoint},
        palette::Palette,
        validation::{
            bounds::Bounds,
            diagnostics::{
                DiagnosticCode,
                DocumentLocation,
                DocumentSection,
                SceneDiagnostic,
                describe_concept_problem,
            },
            rules::header::Geometry,
        },
    },
};

/// Checks the entity section and resolves it into its domain form.
///
/// # Errors
///
/// Returns every problem found across all spawns, in document order. Only the
/// fatal classes appear here; the two advisory classes need the decoded grid
/// and are produced by [`warnings`] once the scene exists.
pub fn check(
    document: &EntitiesDocument,
    geometry: Geometry,
    namespaces: &NamespaceTable,
    bounds: &Bounds,
) -> Result<Entities, Vec<SceneDiagnostic>> {
    let resolver = Resolver {
        prototypes: &document.prototypes,
        namespaces,
        extent: geometry.extent,
        max_depth: bounds.max_prototype_depth,
    };

    let mut problems = Vec::new();
    resolver.check_prototype_concepts(&mut problems);

    let mut seen: BTreeSet<&str> = BTreeSet::new();
    let mut spawns = Vec::with_capacity(document.spawns.len());
    for (ordinal, spawn) in document.spawns.iter().enumerate() {
        let at = location(ordinal);
        if !seen.insert(spawn.name.as_str()) {
            problems.push(SceneDiagnostic::structural(
                DiagnosticCode::DuplicateSpawnName,
                at,
                format!("two spawns are named {:?}", spawn.name),
            ));
        }
        match resolver.resolve(spawn, at) {
            Ok(point) => spawns.push(point),
            Err(found) => problems.extend(found),
        }
    }

    if problems.is_empty() {
        Ok(Entities::from_validated(spawns))
    } else {
        Err(problems)
    }
}

/// The document location of entity-section item `ordinal`.
fn location(ordinal: usize) -> DocumentLocation {
    DocumentLocation::new(
        DocumentSection::Entities,
        u32::try_from(ordinal).unwrap_or(u32::MAX),
    )
}

/// The prototype table and the policy a spawn is resolved against.
struct Resolver<'a> {
    prototypes: &'a BTreeMap<SmolStr, PrototypeDocument>,
    namespaces: &'a NamespaceTable,
    extent: Extent,
    max_depth: usize,
}

impl Resolver<'_> {
    /// Checks each prototype's own concept identifier.
    ///
    /// Done separately from spawn resolution so that a malformed concept on an
    /// unreferenced prototype is still reported: an author who has just
    /// mistyped one is about to reference it.
    fn check_prototype_concepts(&self, problems: &mut Vec<SceneDiagnostic>) {
        let whole = DocumentLocation::section(DocumentSection::Entities);
        for prototype in self.prototypes.values() {
            let Some(raw) = prototype.concept.as_ref() else {
                continue;
            };
            if let Err(problem) = ConceptIri::parse(raw, self.namespaces) {
                problems.push(SceneDiagnostic::structural(
                    DiagnosticCode::ConceptIriInvalid,
                    whole,
                    describe_concept_problem(raw, &problem),
                ));
            }
        }
    }

    /// Validates one spawn into its domain form.
    fn resolve(
        &self,
        spawn: &SpawnDocument,
        at: DocumentLocation,
    ) -> Result<SpawnPoint, Vec<SceneDiagnostic>> {
        let mut problems = Vec::new();

        let position = VoxelPos::new(spawn.at.x, spawn.at.y, spawn.at.z);
        if !self.contains(position) {
            problems.push(SceneDiagnostic::structural(
                DiagnosticCode::SpawnOutwithGrid,
                at,
                format!(
                    "spawn {:?} stands at ({}, {}, {}), outwith the {} x {} x {} extent",
                    spawn.name,
                    position.x,
                    position.y,
                    position.z,
                    self.extent.x(),
                    self.extent.y(),
                    self.extent.z()
                ),
            ));
        }

        let inherited = match self.inherited_concept(spawn.prototype.as_ref(), at) {
            Ok(inherited) => inherited,
            Err(problem) => {
                problems.push(problem);
                None
            }
        };
        let concept = self.parse_concept(spawn.concept.as_ref().or(inherited), at, &mut problems);

        if problems.is_empty() {
            Ok(SpawnPoint {
                name: spawn.name.clone(),
                at: position,
                facing: spawn.facing,
                airborne: spawn.airborne,
                concept,
            })
        } else {
            Err(problems)
        }
    }

    /// Whether `position` lies inside the declared extent.
    const fn contains(&self, position: VoxelPos) -> bool {
        position.x < self.extent.x() && position.y < self.extent.y() && position.z < self.extent.z()
    }

    /// Parses a concept identifier, recording the problem rather than raising.
    fn parse_concept(
        &self,
        raw: Option<&SmolStr>,
        at: DocumentLocation,
        problems: &mut Vec<SceneDiagnostic>,
    ) -> Option<ConceptIri> {
        let candidate = raw?;
        match ConceptIri::parse(candidate, self.namespaces) {
            Ok(iri) => Some(iri),
            Err(problem) => {
                problems.push(SceneDiagnostic::structural(
                    DiagnosticCode::ConceptIriInvalid,
                    at,
                    describe_concept_problem(candidate, &problem),
                ));
                None
            }
        }
    }

    /// Walks a prototype chain, returning the nearest concept it supplies.
    ///
    /// Iterative, depth-bounded, and cycle-detecting. The nearest definition
    /// wins, so a chain overrides its ancestors rather than the reverse.
    fn inherited_concept<'a>(
        &'a self,
        start: Option<&'a SmolStr>,
        at: DocumentLocation,
    ) -> Result<Option<&'a SmolStr>, SceneDiagnostic> {
        let mut visited: BTreeSet<&str> = BTreeSet::new();
        let mut current = start;
        let mut inherited: Option<&SmolStr> = None;
        let mut depth: usize = 0;

        while let Some(name) = current {
            if !visited.insert(name.as_str()) {
                return Err(chain_fault(DiagnosticCode::PrototypeCycle, at, name, self));
            }
            depth = depth.saturating_add(1);
            if depth > self.max_depth {
                return Err(chain_fault(
                    DiagnosticCode::PrototypeTooDeep,
                    at,
                    name,
                    self,
                ));
            }
            let Some(prototype) = self.prototypes.get(name) else {
                return Err(chain_fault(
                    DiagnosticCode::UnknownPrototype,
                    at,
                    name,
                    self,
                ));
            };
            inherited = inherited.or(prototype.concept.as_ref());
            current = prototype.extends.as_ref();
        }
        Ok(inherited)
    }
}

/// Renders a prototype-chain fault, naming the prototype it stopped at.
fn chain_fault(
    code: DiagnosticCode,
    at: DocumentLocation,
    name: &str,
    resolver: &Resolver<'_>,
) -> SceneDiagnostic {
    let detail = match code {
        DiagnosticCode::PrototypeCycle => {
            format!("prototype {name:?} extends itself, directly or through a chain")
        }
        DiagnosticCode::PrototypeTooDeep => format!(
            "the prototype chain reaching {name:?} is deeper than the bound of {}",
            resolver.max_depth
        ),
        _ => format!("prototype {name:?} is not declared in this scene"),
    };
    SceneDiagnostic::structural(code, at, detail)
}

/// The two advisory classes, which need the decoded grid.
///
/// They exist because they are the two cheapest checks that catch the
/// "loads clean, is nonsense" class: a spawn embedded in masonry, and a spawn
/// hanging in mid-air that nobody meant to float. Both are warnings rather
/// than errors because a scene can legitimately want either, and
/// `scene-check --strict` promotes them.
#[must_use]
pub fn warnings(entities: &Entities, grid: &VoxelGrid, palette: &Palette) -> Vec<SceneDiagnostic> {
    entities
        .iter()
        .enumerate()
        .flat_map(|(ordinal, spawn)| spawn_warnings(spawn, location(ordinal), grid, palette))
        .collect()
}

/// The advisory findings for one spawn.
fn spawn_warnings(
    spawn: &SpawnPoint,
    at: DocumentLocation,
    grid: &VoxelGrid,
    palette: &Palette,
) -> Vec<SceneDiagnostic> {
    let mut found = Vec::new();
    if is_obstructed(spawn.at, grid, palette) {
        found.push(SceneDiagnostic::structural(
            DiagnosticCode::SpawnObstructed,
            at,
            format!(
                "spawn {:?} stands inside a voxel no face admits passage through",
                spawn.name
            ),
        ));
    }
    if !spawn.airborne && !is_supported(spawn.at, grid, palette) {
        found.push(SceneDiagnostic::structural(
            DiagnosticCode::SpawnUnsupported,
            at,
            format!(
                "spawn {:?} has nothing beneath it and is not marked airborne",
                spawn.name
            ),
        ));
    }
    found
}

/// Whether the voxel at `position` refuses passage on every face.
fn is_obstructed(position: VoxelPos, grid: &VoxelGrid, palette: &Palette) -> bool {
    voxel_type_at(position, grid, palette).is_some_and(|kind| !kind.passable.is_any_passable())
}

/// Whether something beneath `position` would stop an entity falling.
///
/// The floor of the scene supports by definition: an entity standing at
/// `z == 0` has nowhere to fall to.
fn is_supported(position: VoxelPos, grid: &VoxelGrid, palette: &Palette) -> bool {
    let Some(below) = position.z.checked_sub(1) else {
        return true;
    };
    let beneath = VoxelPos::new(position.x, position.y, below);
    voxel_type_at(beneath, grid, palette).is_some_and(|kind| !kind.passable.pos_z)
}

/// The voxel type at `position`, when the position is in the scene and its
/// index resolves.
fn voxel_type_at<'a>(
    position: VoxelPos,
    grid: &VoxelGrid,
    palette: &'a Palette,
) -> Option<&'a crate::scene::palette::VoxelType> {
    palette.get(grid.get(position)?)
}
