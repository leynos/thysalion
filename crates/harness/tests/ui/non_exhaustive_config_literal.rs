//! `HarnessConfig` is `#[non_exhaustive]`: external crates must construct
//! it through `HarnessConfig::new` and the `with_*` builders, never with a
//! struct literal, so the harness can add fields without breaking demos.

use thysalion_harness::HarnessConfig;

fn main() {
    let _config = HarnessConfig {
        slug: String::from("demo-empty"),
        window_title: String::from("Thysalion"),
        zoom_bounds: Default::default(),
        initial_quadrant: Default::default(),
    };
}
