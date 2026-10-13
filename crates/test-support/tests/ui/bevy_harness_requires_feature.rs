//! Compile-time contract: without `bevy`, the public `BevyHarness` is absent.

use thysalion_test_support::BevyHarness;

fn main() {
    let _harness = BevyHarness;
}
