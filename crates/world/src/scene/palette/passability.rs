//! Validated passability values used by palette and pathfinding code.
//!
//! These types intentionally have no serde or schema implementations; scene
//! documents convert to and from them at the validation and encoding boundary.

use crate::scene::document::{PassabilityDocument, PassabilityFaceDocument};

/// Whether an agent may cross one named voxel face.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FacePassability(bool);

impl FacePassability {
    /// Creates a face passability value from its wire boolean.
    ///
    /// # Examples
    ///
    /// ```
    /// use thysalion_world::scene::palette::FacePassability;
    ///
    /// let face = FacePassability::new(true);
    /// assert!(face.is_passable());
    /// ```
    #[must_use]
    pub const fn new(is_passable: bool) -> Self { Self(is_passable) }

    /// Whether an agent may cross this face.
    ///
    /// # Examples
    ///
    /// ```
    /// use thysalion_world::scene::palette::FacePassability;
    ///
    /// let face = FacePassability::new(false);
    /// assert!(!face.is_passable());
    /// ```
    #[must_use]
    pub const fn is_passable(self) -> bool { self.0 }
}

/// Converts a boolean into its face-specific domain value.
///
/// # Examples
///
/// ```
/// use thysalion_world::scene::palette::FacePassability;
///
/// let face = FacePassability::from(true);
/// assert!(face.is_passable());
/// ```
impl From<bool> for FacePassability {
    fn from(is_passable: bool) -> Self { Self::new(is_passable) }
}

/// Per-face passability for validated pathfinding data.
///
/// Six named fields make the face-to-value mapping explicit. An array would
/// require each reader to agree on an index mapping; the document
/// representation is `scene::document::PassabilityDocument`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Passability {
    /// Whether an agent may cross the `+x` face.
    pub pos_x: FacePassability,
    /// Whether an agent may cross the `-x` face.
    pub neg_x: FacePassability,
    /// Whether an agent may cross the `+y` face.
    pub pos_y: FacePassability,
    /// Whether an agent may cross the `-y` face.
    pub neg_y: FacePassability,
    /// Whether an agent may cross the `+z` face.
    pub pos_z: FacePassability,
    /// Whether an agent may cross the `-z` face.
    pub neg_z: FacePassability,
}

impl Passability {
    /// Creates a value with the same passability on every face.
    ///
    /// # Examples
    ///
    /// ```
    /// use thysalion_world::scene::palette::Passability;
    ///
    /// let solid = Passability::closed();
    /// assert!(!solid.pos_x.is_passable());
    /// assert!(!solid.neg_z.is_passable());
    /// ```
    #[must_use]
    const fn uniform(is_passable: bool) -> Self {
        let face = FacePassability::new(is_passable);
        Self {
            pos_x: face,
            neg_x: face,
            pos_y: face,
            neg_y: face,
            pos_z: face,
            neg_z: face,
        }
    }

    /// Every face passable, as air is.
    #[must_use]
    pub const fn open() -> Self { Self::uniform(true) }

    /// No face passable, as a solid block is.
    #[must_use]
    pub const fn closed() -> Self { Self::uniform(false) }

    /// Whether any face admits passage.
    #[must_use]
    pub const fn is_any_passable(&self) -> bool {
        self.pos_x.is_passable()
            || self.neg_x.is_passable()
            || self.pos_y.is_passable()
            || self.neg_y.is_passable()
            || self.pos_z.is_passable()
            || self.neg_z.is_passable()
    }
}

impl From<PassabilityDocument> for Passability {
    fn from(document: PassabilityDocument) -> Self {
        Self {
            pos_x: document.pos_x.is_passable().into(),
            neg_x: document.neg_x.is_passable().into(),
            pos_y: document.pos_y.is_passable().into(),
            neg_y: document.neg_y.is_passable().into(),
            pos_z: document.pos_z.is_passable().into(),
            neg_z: document.neg_z.is_passable().into(),
        }
    }
}

impl From<Passability> for PassabilityDocument {
    fn from(passability: Passability) -> Self {
        Self {
            pos_x: PassabilityFaceDocument::from(passability.pos_x.is_passable()),
            neg_x: PassabilityFaceDocument::from(passability.neg_x.is_passable()),
            pos_y: PassabilityFaceDocument::from(passability.pos_y.is_passable()),
            neg_y: PassabilityFaceDocument::from(passability.neg_y.is_passable()),
            pos_z: PassabilityFaceDocument::from(passability.pos_z.is_passable()),
            neg_z: PassabilityFaceDocument::from(passability.neg_z.is_passable()),
        }
    }
}
