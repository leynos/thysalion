//! Contract that pull requests run the suite once through coverage, with
//! doctests in a separate unconditional step. The workflow reader also checks
//! build-tool provisioning before every Linux suite route.

use rstest::rstest;

#[path = "workflow_suite/install_tools.rs"]
mod install_tools;
#[path = "workflow_suite/reading.rs"]
mod reading;
#[path = "workflow_suite/whitaker.rs"]
mod whitaker;

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
#[case::make_coverage("make coverage", true)]
#[case::make_dev_test("make dev-test", true)]
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
    assert_eq!(
        parsed.features(),
        expected,
        "manifest feature parsing changed"
    );
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
        let steps: Vec<_> = found
            .iter()
            .filter(|(name, _)| name == workflow)
            .flat_map(|(_, text)| Workflow(text).jobs())
            .flat_map(|job| job.steps())
            .filter(|step| step.uses(COVERAGE_ACTION))
            .collect();
        assert!(
            !steps.is_empty() && steps.iter().all(|step| step.has_line(&expected)),
            "{workflow}'s coverage step must pass `{expected}`"
        );
    }
}

#[test]
fn a_new_linux_suite_job_without_installation_is_not_hidden() {
    let source = format!(
        "{minimal_routing}  new-linux-test:\n    runs-on: ubuntu-latest\n    steps:\n      - run: \
         cargo nextest run\n",
        minimal_routing = install_tools::MINIMAL_INSTALL_ROUTING,
    );
    let report = install_tools::inspect_routing(&source).expect("new job route must be inspected");
    assert_eq!(
        report.missing_installers,
        vec!["fixture.yml:new-linux-test:step-0".to_owned()],
        "a new Linux suite job must report its missing installer"
    );
}

#[test]
fn empty_or_unresolvable_workflow_inventory_fails_closed() {
    assert!(
        install_tools::inspect(&[]).is_err(),
        "empty workflow inventory must fail"
    );
    assert!(
        install_tools::inspect_routing("name: empty\njobs: {}\n").is_err(),
        "workflow with no jobs must fail"
    );
    let missing_runner = "name: unknown\njobs:\n  test:\n    steps:\n      - run: cargo test\n";
    assert!(
        install_tools::inspect_routing(missing_runner).is_err(),
        "suite job without a runner must fail"
    );
    let no_suite = "jobs:\n  check:\n    steps:\n      - run: echo ready\n";
    assert!(
        install_tools::inspect_routing(no_suite).is_err(),
        "workflow inventory without a suite route must fail"
    );
}

/// Reads the recipe lines beneath one Make target declaration.
fn make_recipe<'a>(makefile: &'a str, target: &str) -> Vec<&'a str> {
    let prefix = format!("{target}:");
    makefile
        .lines()
        .skip_while(|line| !line.starts_with(&prefix))
        .skip(1)
        .take_while(|line| line.starts_with('\t'))
        .collect()
}

#[test]
fn indirect_make_suite_targets_reach_test_and_coverage_commands() {
    let makefile = manifest_dir()
        .expect("failed to open the repository")
        .read_to_string("Makefile")
        .expect("failed to read Makefile");
    assert!(
        makefile
            .lines()
            .any(|line| { line.starts_with("test:") && line.contains("check-build-tools") }),
        "test must depend on the build-tool preflight"
    );
    assert!(
        makefile
            .lines()
            .any(|line| { line.starts_with("coverage:") && line.contains("check-build-tools") }),
        "coverage must depend on the build-tool preflight"
    );
    assert!(
        make_recipe(&makefile, "all")
            .join("\n")
            .contains("$(MAKE) test"),
        "all must reach the test target"
    );
    assert!(
        make_recipe(&makefile, "test")
            .join("\n")
            .contains("$(CARGO) $(TEST_CMD)"),
        "test must invoke the injectable Cargo command"
    );
    assert!(
        make_recipe(&makefile, "coverage")
            .join("\n")
            .contains("$(CARGO) llvm-cov"),
        "coverage must invoke cargo llvm-cov"
    );
    assert!(
        Command::from_line("make all").runs_suite(),
        "all must reach the suite"
    );
    assert!(
        Command::from_line("make coverage").runs_suite(),
        "coverage must reach the suite"
    );
}
