//! Compile-time contract: enabling `bevy` exposes the public `BevyHarness`.

use thysalion_test_support::BevyHarness;

fn main() {
    let _harness = BevyHarness;
}
