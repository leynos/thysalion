//! Concept Internationalized Resource Identifiers, and the project namespace
//! table they are checked against.
//!
//! A voxel type or spawn may name the ontology concept it instantiates, for
//! example `thy:OakDoor` (design §7.2). This module validates *syntax and
//! namespace membership only*. Resolving an identifier against the ontology is
//! the knowledge plane's work at roadmap step 5.1, and the dependency edge
//! runs `knowledge -> world`, never the reverse — which is why nothing here
//! knows what a concept means.
//!
//! The namespace list is knowledge-plane policy, so it is *injected* rather
//! than hard-coded into a check. Baking it in would mean phase 5 editing the
//! state plane to add a namespace, with no Cargo edge for review to catch —
//! exactly the coupling ADR 005's review-enforced layering misses.

use std::collections::BTreeMap;

use smol_str::SmolStr;

/// The prefix under which Thysalion's own ontology terms live.
pub const THYSALION_PREFIX: &str = "thy";

/// The base the [`THYSALION_PREFIX`] expands to.
pub const THYSALION_BASE: &str = "https://thysalion.df12.net/ontology#";

/// The prefix under which per-scene named graphs live.
pub const SCENE_PREFIX: &str = "scene";

/// The base the [`SCENE_PREFIX`] expands to.
pub const SCENE_BASE: &str = "https://thysalion.df12.net/scene/";

/// Why an identifier was rejected.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[non_exhaustive]
pub enum ConceptIriProblem {
    /// The identifier has no `prefix:local` shape.
    #[error("expected `prefix:local`")]
    Malformed,
    /// The local part is empty, so the identifier names nothing.
    #[error("the local part is empty")]
    EmptyLocal,
    /// The local part holds a character an identifier may not carry.
    #[error("the local part contains {0:?}, which is not permitted")]
    IllegalCharacter(char),
    /// The prefix is not one this project publishes.
    #[error("prefix {found:?} is not a project namespace")]
    UnknownPrefix {
        /// The prefix the document used.
        found: SmolStr,
    },
}

/// The prefixes a scene may use, and what they expand to.
///
/// Supplied by the caller so that adding a namespace is a knowledge-plane
/// change rather than a state-plane one.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NamespaceTable {
    bases: BTreeMap<SmolStr, SmolStr>,
}

impl NamespaceTable {
    /// An empty table, which rejects every prefixed identifier.
    #[must_use]
    pub const fn empty() -> Self {
        Self {
            bases: BTreeMap::new(),
        }
    }

    /// Adds a prefix and the base it expands to.
    #[must_use]
    pub fn with(mut self, prefix: impl Into<SmolStr>, base: impl Into<SmolStr>) -> Self {
        self.bases.insert(prefix.into(), base.into());
        self
    }

    /// Whether the table publishes `prefix`.
    #[must_use]
    pub fn contains(&self, prefix: &str) -> bool { self.bases.contains_key(prefix) }

    /// The base `prefix` expands to.
    #[must_use]
    pub fn base(&self, prefix: &str) -> Option<&str> { self.bases.get(prefix).map(SmolStr::as_str) }

    /// The prefixes this table publishes, in sorted order.
    pub fn prefixes(&self) -> impl Iterator<Item = &str> { self.bases.keys().map(SmolStr::as_str) }
}

impl Default for NamespaceTable {
    /// The project's own namespaces: `thy:` for ontology terms and `scene:`
    /// for per-scene named graphs.
    fn default() -> Self {
        Self::empty()
            .with(THYSALION_PREFIX, THYSALION_BASE)
            .with(SCENE_PREFIX, SCENE_BASE)
    }
}

/// A syntactically valid, project-namespaced concept identifier.
///
/// Validated on construction and immutable thereafter, so a value of this type
/// is evidence the check ran. It derives no `serde` traits: a derived
/// `Deserialize` would be a public constructor that skipped validation.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[non_exhaustive]
pub struct ConceptIri {
    raw: SmolStr,
    colon: usize,
}

impl ConceptIri {
    /// Parses `raw` as `prefix:local` and checks the prefix against `table`.
    ///
    /// # Errors
    ///
    /// Returns [`ConceptIriProblem`] describing the first fault found.
    pub fn parse(raw: &str, table: &NamespaceTable) -> Result<Self, ConceptIriProblem> {
        let colon = raw.find(':').ok_or(ConceptIriProblem::Malformed)?;
        let (prefix, rest) = raw.split_at(colon);
        let local = rest.get(1..).unwrap_or_default();
        if prefix.is_empty() {
            return Err(ConceptIriProblem::Malformed);
        }
        if local.is_empty() {
            return Err(ConceptIriProblem::EmptyLocal);
        }
        if let Some(bad) = local.chars().find(|c| !is_local_char(*c)) {
            return Err(ConceptIriProblem::IllegalCharacter(bad));
        }
        if !table.contains(prefix) {
            return Err(ConceptIriProblem::UnknownPrefix {
                found: SmolStr::new(prefix),
            });
        }
        Ok(Self {
            raw: SmolStr::new(raw),
            colon,
        })
    }

    /// The identifier as written, such as `thy:OakDoor`.
    #[must_use]
    pub fn as_str(&self) -> &str { &self.raw }

    /// The prefix part, such as `thy`.
    #[must_use]
    pub fn prefix(&self) -> &str { self.raw.get(..self.colon).unwrap_or_default() }

    /// The local part, such as `OakDoor`.
    #[must_use]
    pub fn local(&self) -> &str {
        self.raw
            .get(self.colon.saturating_add(1)..)
            .unwrap_or_default()
    }
}

impl core::fmt::Display for ConceptIri {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result { f.write_str(&self.raw) }
}

/// Whether `c` may appear in the local part of an identifier.
///
/// Deliberately narrow. The Resource Description Framework permits far more,
/// but a scene document is authored by hand and a permissive rule turns a typo
/// into a silently distinct concept. Widening this later is compatible;
/// narrowing it would not be.
const fn is_local_char(c: char) -> bool {
    c.is_ascii_alphanumeric() || matches!(c, '_' | '-' | '.' | '/')
}
