//! The validated entity section: spawn points with their prototype chains
//! already resolved.
//!
//! Prototypes do not survive validation. They are an authoring convenience —
//! a way to say "these forty spawns all instantiate `thy:Torch`" once — and
//! resolving them at load leaves the runtime one flat list with no indirection
//! to chase. Nothing downstream has to know a prototype existed, which is why
//! [`Entities`] has no prototype field.
//!
//! These are *domain* types: they derive no `serde` traits, so the only way to
//! obtain one is through validation.

use smol_str::SmolStr;

use crate::{
    grid::VoxelPos,
    scene::{concept::ConceptIri, document::Face},
};

/// One entity placed in the scene, with its prototype chain resolved.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct SpawnPoint {
    /// Name, unique within the scene.
    pub name: SmolStr,
    /// Where the entity stands, checked to lie within the scene extent.
    pub at: VoxelPos,
    /// Which way the entity faces.
    pub facing: Face,
    /// Whether the entity is expected to have no support beneath it.
    pub airborne: bool,
    /// The ontology concept this entity instantiates, parsed and namespaced.
    ///
    /// Inherited from the prototype chain when the spawn does not name one.
    pub concept: Option<ConceptIri>,
}

/// The scene's spawn points, in document order.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
#[non_exhaustive]
pub struct Entities {
    spawns: Vec<SpawnPoint>,
}

impl Entities {
    /// Wraps an already-validated spawn list.
    ///
    /// Crate-private: the only caller is
    /// [`crate::scene::validation::rules::entities`], which is where the
    /// invariants are established.
    pub(crate) const fn from_validated(spawns: Vec<SpawnPoint>) -> Self { Self { spawns } }

    /// The spawn points, in document order.
    pub fn iter(&self) -> impl Iterator<Item = &SpawnPoint> { self.spawns.iter() }

    /// How many spawn points the scene declares.
    #[must_use]
    pub const fn len(&self) -> usize { self.spawns.len() }

    /// Whether the scene declares no spawn points, which is permitted.
    #[must_use]
    pub const fn is_empty(&self) -> bool { self.spawns.is_empty() }

    /// The spawn point named `name`, when there is one.
    #[must_use]
    pub fn get(&self, name: &str) -> Option<&SpawnPoint> {
        self.spawns.iter().find(|spawn| spawn.name == name)
    }
}

impl<'a> IntoIterator for &'a Entities {
    type Item = &'a SpawnPoint;
    type IntoIter = core::slice::Iter<'a, SpawnPoint>;

    fn into_iter(self) -> Self::IntoIter { self.spawns.iter() }
}
