//! Contract test that each pull request runs the test suite once.
//!
//! `make test` runs the suite with `--all-targets --all-features` and then
//! the doctests. The coverage step in `ci.yml`'s `build-test` job runs the
//! same tests, but not the doctests, which `build-test` runs in a step of its
//! own. The repository used to carry an `act-validation.yml` workflow running
//! `make test WITH_ACT=1`; nothing reads `WITH_ACT` and no test is gated on
//! Act, so it ran the whole suite a second time and was removed.
//!
//! These tests read the workflows textually and the manifest as TOML, and
//! hold the split:
//!
//! - no workflow line runs the suite, in any spelling of `make test`, `make all`, a bare `make`,
//!   `cargo test`, `cargo nextest` or `cargo llvm-cov`, whatever the options or separators around
//!   it, except the one doctest step;
//! - that doctest step is in `build-test`, and neither the job nor the step carries an `if:`;
//! - `build-test` runs the coverage action in one unguarded step, and no workflow turns on its
//!   doctests;
//! - the coverage steps in `ci.yml` and `coverage-main.yml` pass every declared feature, so `make
//!   test`'s `--all-features` and the coverage run select the same tests.
//!
//! The readers live in `workflow_suite/reading.rs`.

use rstest::rstest;

#[path = "workflow_suite/reading.rs"]
mod reading;

use reading::{Command, Job, Manifest, Workflow, manifest_dir, workflows};

/// The one suite command a workflow may run outside the coverage step.
const DOCTEST_COMMAND: &str = "cargo test --doc --workspace --all-features";

/// The coverage action every pull request's suite run goes through.
const COVERAGE_ACTION: &str = "leynos/shared-actions/.github/actions/generate-coverage@";

/// The job that must run both the coverage step and the doctest step.
const SUITE_JOB: &str = "build-test";

/// Returns the `build-test` job of `ci.yml`, if it exists.
fn suite_job(found: &[(String, String)]) -> Option<Job<'_>> {
    found
        .iter()
        .filter(|(name, _)| name == "ci.yml")
        .flat_map(|(_, text)| Workflow(text).jobs())
        .find(|job| job.name == SUITE_JOB)
}

/// Reads the manifest at `path` under the crate directory.
fn manifest(path: &str) -> std::io::Result<Manifest> {
    let text = manifest_dir()?.read_to_string(path)?;
    Manifest::parse(&text)
        .map_err(|error| std::io::Error::new(std::io::ErrorKind::InvalidData, error))
}

/// Returns every workflow line that runs the suite, as
/// `(workflow, job, command)`.
fn suite_runs(found: &[(String, String)]) -> Vec<(&str, &str, &str)> {
    found
        .iter()
        .flat_map(|(name, text)| {
            Workflow(text).jobs().into_iter().flat_map(move |job| {
                job.commands()
                    .filter(|command| command.runs_suite())
                    .map(|command| (name.as_str(), job.name, command.text()))
                    .collect::<Vec<_>>()
            })
        })
        .collect()
}

#[rstest]
#[case::plain("make test", true)]
#[case::act_flag("make test WITH_ACT=1", true)]
#[case::make_option("make -j2 test", true)]
#[case::make_directory("make -C . test", true)]
#[case::make_all("make all", true)]
#[case::make_default_goal("make", true)]
#[case::quoted_target("make \"test\"", true)]
#[case::cargo("cargo test --all-features", true)]
#[case::cargo_config("cargo --config tools/dev-fast/config.toml test", true)]
#[case::cargo_toolchain("cargo +nightly test", true)]
#[case::nextest("cargo nextest run --all-targets", true)]
#[case::llvm_cov("cargo llvm-cov nextest --lcov", true)]
#[case::chained("set -eu && make test", true)]
#[case::unspaced("make lint&&make test", true)]
#[case::assignment("RUSTFLAGS=-Dwarnings cargo test", true)]
#[case::named_target("make test-workflow-contracts", false)]
#[case::lint("make lint", false)]
#[case::build("cargo build --all-targets", false)]
#[case::run_argument("cargo run -- test", false)]
#[case::echo("echo cargo test", false)]
#[case::step_item("- run: cargo test --all-features", true)]
fn suite_spellings_are_recognized(#[case] line: &str, #[case] expected: bool) {
    assert_eq!(
        Command::from_line(line).runs_suite(),
        expected,
        "misread {line:?}"
    );
}

#[test]
fn only_the_doctest_step_runs_the_suite_outside_coverage() {
    let found = workflows().expect("failed to read the workflows");
    let (doctests, repeated): (Vec<_>, Vec<_>) =
        suite_runs(&found)
            .into_iter()
            .partition(|(name, job, command)| {
                *command == DOCTEST_COMMAND && *name == "ci.yml" && *job == SUITE_JOB
            });
    assert!(
        repeated.is_empty(),
        "the suite runs outside coverage in {repeated:?}"
    );
    assert_eq!(
        doctests.len(),
        1,
        "`{DOCTEST_COMMAND}` must run once, in {SUITE_JOB}"
    );
}

#[test]
fn build_test_runs_the_doctests_unconditionally() {
    let found = workflows().expect("failed to read the workflows");
    let job = suite_job(&found).expect("ci.yml must define build-test");
    assert!(
        !job.is_conditional(),
        "{SUITE_JOB} must run on every pull request"
    );
    let doctest = Command::from_line(DOCTEST_COMMAND);
    let steps: Vec<_> = job
        .steps()
        .into_iter()
        .filter(|step| step.runs(doctest))
        .collect();
    assert_eq!(
        steps.len(),
        1,
        "{SUITE_JOB} must run the doctests in one step"
    );
    assert!(
        !steps.iter().any(reading::Step::is_conditional),
        "the doctest step must always run"
    );
}

#[test]
fn build_test_runs_coverage_unconditionally() {
    let found = workflows().expect("failed to read the workflows");
    let job = suite_job(&found).expect("ci.yml must define build-test");
    let steps: Vec<_> = job
        .steps()
        .into_iter()
        .filter(|step| step.uses(COVERAGE_ACTION))
        .collect();
    assert_eq!(
        steps.len(),
        1,
        "{SUITE_JOB} must run the coverage action once"
    );
    assert!(
        !steps.iter().any(reading::Step::is_conditional),
        "coverage must always run"
    );
}

#[test]
fn coverage_leaves_the_doctests_off() {
    let found = workflows().expect("failed to read the workflows");
    let enabling: Vec<&str> = found
        .iter()
        .filter(|(_, text)| {
            text.lines().any(|raw| {
                let line = raw.trim();
                line.starts_with("doctests:") && line.contains("true")
            })
        })
        .map(|(name, _)| name.as_str())
        .collect();
    assert!(
        enabling.is_empty(),
        "coverage would repeat the doctests in {enabling:?}"
    );
}

#[rstest]
#[case::none("[package]\nname = \"x\"\n", &[])]
#[case::commented_table("[features] # flags\nextra = []\n", &["extra"])]
#[case::tabbed_optional("[dependencies]\nserde = { version = \"1\", optional\t=\ttrue }\n", &["serde"])]
#[case::target_optional("[target.'cfg(unix)'.dependencies]\nlibc = { version = \"1\", optional = true }\n", &["libc"])]
#[case::dep_syntax("[features]\nextra = [\"dep:serde\"]\n\n[dependencies]\nserde = { version = \"1\", optional = true }\n", &["extra"])]
#[case::required("[dependencies]\nserde = { version = \"1\" }\n", &[])]
fn manifest_features_are_read(#[case] text: &str, #[case] expected: &[&str]) {
    let parsed = Manifest::parse(text).expect("the fixture must parse");
    assert_eq!(parsed.features(), expected);
}

/// The features the coverage steps pass, which must be every declared one.
const COVERAGE_FEATURES: &str = "thysalion-demos/demo-empty";

/// The workflows whose coverage step must pass the features.
const COVERAGE_WORKFLOWS: [&str; 2] = ["ci.yml", "coverage-main.yml"];

/// Returns every workspace feature as `package/feature`, sorted.
fn workspace_features() -> std::io::Result<Vec<String>> {
    let crates = manifest_dir()?.open_dir("crates")?;
    let mut paths = vec!["Cargo.toml".to_owned()];
    for entry in crates.entries()? {
        paths.push(format!("crates/{}/Cargo.toml", entry?.file_name()?));
    }
    let mut found = Vec::new();
    for path in &paths {
        let parsed = manifest(path)?;
        let package = parsed.package();
        found.extend(
            parsed
                .features()
                .into_iter()
                .filter(|feature| feature != "default")
                .map(|feature| format!("{package}/{feature}")),
        );
    }
    found.sort();
    Ok(found)
}

#[test]
fn coverage_enables_every_declared_feature() {
    let mut passed: Vec<String> = COVERAGE_FEATURES
        .split_whitespace()
        .map(str::to_owned)
        .collect();
    passed.sort();
    assert_eq!(
        workspace_features().expect("failed to read the workspace manifests"),
        passed,
        "`make test` runs --all-features, so coverage must enable every declared feature"
    );
}

#[test]
fn both_coverage_steps_pass_the_features() {
    let found = workflows().expect("failed to read the workflows");
    let expected = format!("features: {COVERAGE_FEATURES}");
    for workflow in COVERAGE_WORKFLOWS {
        let text = found
            .iter()
            .find(|(name, _)| name == workflow)
            .map_or("", |(_, text)| text.as_str());
        assert!(
            text.lines().any(|line| line.trim() == expected),
            "{workflow}'s coverage step must pass `{expected}`"
        );
    }
}
