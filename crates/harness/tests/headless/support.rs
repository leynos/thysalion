//! rstest-bdd harness adapter for headless Bevy apps, following the
//! third-party harness adapter cookbook in the rstest-bdd users' guide.
//!
//! The adapter's context is a full `bevy::app::App` built with
//! `MinimalPlugins` and `HarnessCorePlugin`, so step functions drive the
//! real schedule with `app.update()` rather than poking systems directly.
//! Roadmap step 1.3.1 (headless continuous-integration scaffolding) is
//! the intended point to promote this adapter into a shared test-support
//! crate.

use bevy::{MinimalPlugins, app::App};
use rstest_bdd_harness::{HarnessAdapter, HarnessResult, ScenarioRunRequest};
use thysalion_harness::{HarnessConfig, HarnessCorePlugin};

/// Runs each scenario against a fresh headless harness app.
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
