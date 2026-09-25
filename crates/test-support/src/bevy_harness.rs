//! rstest-bdd harness adapter for headless Bevy apps, following the
//! third-party harness adapter cookbook in the rstest-bdd users' guide.
//!
//! The adapter's context is a full `bevy::app::App` built with
//! `MinimalPlugins` and `HarnessCorePlugin`, so step functions drive the
//! real schedule with `app.update()` rather than poking systems directly.
//!
//! The module sits behind the crate's non-default `bevy` feature. That is
//! what keeps `thysalion-world` — which dev-depends on this crate — free of
//! `bevy` until ADR 005 stages the dependency in at roadmap 2.1.1; a
//! default-on feature here would stage it in early through a test
//! convenience.

use bevy::{MinimalPlugins, app::App};
use rstest_bdd_harness::{HarnessAdapter, HarnessResult, ScenarioRunRequest};
use thysalion_harness::{HarnessConfig, HarnessCorePlugin};

/// Runs each scenario against a fresh headless harness app.
///
/// A fresh app per scenario, not a shared one: Bevy resources are mutable
/// global state, so a scenario that leaves the rig rotated would silently
/// become the next scenario's precondition.
#[derive(Default)]
pub struct BevyHarness;

impl HarnessAdapter for BevyHarness {
    type Context = App;

    fn run<T>(&self, request: ScenarioRunRequest<'_, Self::Context, T>) -> HarnessResult<T> {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins);
        app.add_plugins(HarnessCorePlugin::new(HarnessConfig::default()));
        Ok(request.run(app))
    }
}
