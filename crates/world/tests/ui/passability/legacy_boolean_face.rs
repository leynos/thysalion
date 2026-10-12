//! A face field no longer accepts the former untyped boolean assignment.

use thysalion_world::scene::palette::Passability;

fn main() {
    let mut passability = Passability::closed();
    passability.pos_x = true;
}
