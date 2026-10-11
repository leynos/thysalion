//! A typed face can be constructed, assigned, and read through the public API.

use thysalion_world::scene::document::{FacePassability, Passability};

fn main() {
    let mut passability = Passability::closed();
    passability.pos_x = FacePassability::new(true);
    assert!(passability.pos_x.is_passable());

    let closed_face: FacePassability = false.into();
    assert!(!closed_face.is_passable());
}
