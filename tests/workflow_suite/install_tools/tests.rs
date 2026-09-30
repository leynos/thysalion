//! Exercise build-tool installation across suite routes and runner shapes.

use rstest::rstest;

use super::{INSTALL_ROUTING_FIXTURE, MINIMAL_INSTALL_ROUTING, inspect, inspect_routing};
use crate::reading::workflows;

#[test]
fn workflows_provision_build_tools_before_each_linux_suite_path() {
    let found = workflows().expect("failed to read workflows");
    let report = inspect(&found).expect("workflow suite routes must be resolvable");
    assert!(
        report.missing_installers.is_empty(),
        "{:?}",
        report.missing_installers
    );
}

#[test]
fn route_reader_covers_runner_shapes_and_every_suite_entrypoint() {
    let report = inspect_routing(INSTALL_ROUTING_FIXTURE).expect("fixture routes must resolve");
    assert_eq!(
        report.linux_suite_entries, 8,
        "all runner shapes must be counted"
    );
    assert!(
        report.missing_installers.is_empty(),
        "fixture routes must install build tools"
    );
}

#[rstest]
#[case::removed(MINIMAL_INSTALL_ROUTING.replace("      - name: Install build tools\n        run: make install-build-tools\n", ""))]
#[case::moved_after_suite(MINIMAL_INSTALL_ROUTING.replace("      - name: Install build tools\n        run: make install-build-tools\n      - name: Run suite\n        run: make test\n", "      - name: Run suite\n        run: make test\n      - name: Install build tools\n        run: make install-build-tools\n"))]
#[case::conditional(MINIMAL_INSTALL_ROUTING.replace("        run: make install-build-tools\n", "        if: runner.os == 'Linux'\n        run: make install-build-tools\n"))]
#[case::conditional_job(MINIMAL_INSTALL_ROUTING.replace("  test:\n    runs-on:", "  test:\n    if: runner.os == 'Linux'\n    runs-on:"))]
#[case::soft_failure(MINIMAL_INSTALL_ROUTING.replace("        run: make install-build-tools\n", "        continue-on-error: true\n        run: make install-build-tools\n"))]
fn installer_mutations_are_reported(#[case] source: String) {
    let report = inspect_routing(&source).expect("mutated workflow must remain inspectable");
    assert_eq!(
        report.missing_installers.len(),
        1,
        "one missing installer expected"
    );
    assert!(
        report
            .missing_installers
            .iter()
            .all(|path| path.contains(":test:step-")),
        "every installer finding must identify the affected suite step"
    );
}

#[rstest]
#[case::shell_wrapper("bash -c 'cargo test'")]
#[case::local_script("./ci-tests.sh")]
#[case::env_shell_wrapper("env bash -c 'cargo test'")]
#[case::env_local_script("env ./ci-tests.sh")]
fn unprovable_suite_routes_fail_closed(#[case] route: &str) {
    let source = MINIMAL_INSTALL_ROUTING.replace(
        "        run: make test\n",
        &format!("        run: {route}\n"),
    );
    assert_ne!(
        source, MINIMAL_INSTALL_ROUTING,
        "route mutation must change the fixture"
    );
    let error = inspect_routing(&source).expect_err("unprovable suite route must fail");
    assert!(error.contains("suite command is unresolved"), "{error}");
}
