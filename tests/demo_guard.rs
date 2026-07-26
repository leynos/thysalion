//! Guards for the `make demo` launcher.
//!
//! The recipe must derive its whitelist of supported demo names from the
//! demo binaries on disk, reject an unsupported (even well-formed) `DEMO`
//! value *before* Cargo is invoked, and launch the right target for a
//! supported one. These tests pin all three so the guard cannot be
//! removed silently.
//!
//! Neither positive test launches the windowed demo, which would open a
//! window and block. They inspect the launcher two complementary ways,
//! because no single invocation shows everything:
//!
//! - `make --dry-run demo` prints the recipe *before* the shell runs it, so it shows the quoting
//!   that protects the arguments — but the slug is a shell variable (`demo-$DEMO_SLUG`), still
//!   unexpanded.
//! - Running the recipe with `CARGO` overridden to an echo stub lets the shell expand everything,
//!   showing the fully resolved target — but the shell has consumed the quotes by then.
//!
//! Together they pin both the quoting and the resolved default target.

use std::process::Command;

use cap_std::{ambient_authority, fs_utf8::Dir};

/// Runs a `make` invocation in the repository root with inherited make
/// flags removed, so a parent `make` (or a test harness that exports
/// `MAKEFLAGS`) cannot alter the recipe under test.
fn run_make(args: &[&str]) -> std::process::Output {
    match Command::new("make")
        .args(args)
        .current_dir(env!("CARGO_MANIFEST_DIR"))
        .env_remove("MAKEFLAGS")
        .env_remove("MFLAGS")
        .output()
    {
        Ok(output) => output,
        Err(error) => panic!("running `make {}` failed: {error}", args.join(" ")),
    }
}

#[test]
fn makefile_derives_the_demo_whitelist_from_the_demo_binaries() {
    let dir = Dir::open_ambient_dir(env!("CARGO_MANIFEST_DIR"), ambient_authority())
        .expect("open repository root");
    let makefile = dir.read_to_string("Makefile").expect("read Makefile");
    assert!(
        makefile.contains("wildcard crates/demos/src/bin/demo-*.rs"),
        "the demo whitelist must be derived from the demo binaries on disk"
    );
    assert!(
        makefile.contains("DEMO_ALLOWED"),
        "the demo recipe must pass the whitelist to the shell via the environment, not via make \
         interpolation"
    );
}

#[test]
fn make_demo_rejects_an_unsupported_demo_before_invoking_cargo() {
    let output = run_make(&["demo", "DEMO=not-a-real-demo"]);
    let stderr = String::from_utf8_lossy(&output.stderr);
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        !output.status.success(),
        "make demo must fail for an unsupported demo name; stdout: {stdout}"
    );
    assert!(
        stderr.contains("DEMO must be one of"),
        "the whitelist guard must produce the failure, got stderr: {stderr}"
    );
    assert!(
        !stdout.contains("cargo run") && !stderr.contains("Compiling"),
        "Cargo must never be invoked for an unsupported demo; stdout: {stdout} stderr: {stderr}"
    );
}

#[test]
fn make_dry_run_demo_renders_a_quoted_cargo_invocation() {
    let output = run_make(&["--dry-run", "demo"]);
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        output.status.success(),
        "make --dry-run demo must succeed for the default demo; stdout: {stdout} stderr: {stderr}"
    );
    assert!(
        stdout.contains("cargo run -p thysalion-demos"),
        "the launcher must scope the run to the demos package, got: {stdout}"
    );
    // The slug is a shell variable at this stage; what the dry run proves
    // is that both arguments stay quoted, so a slug can never split into
    // extra arguments.
    assert!(
        stdout.contains(r#"--features "demo-$DEMO_SLUG""#),
        "the feature argument must be quoted, got: {stdout}"
    );
    assert!(
        stdout.contains(r#"--bin "demo-$DEMO_SLUG""#),
        "the binary argument must be quoted, got: {stdout}"
    );
}

#[test]
fn make_demo_resolves_the_default_launcher_to_demo_empty() {
    // `CARGO` is a Makefile variable, so overriding it replaces the whole
    // command: the recipe's guard and shell expansion still run, but the
    // windowed binary is never launched.
    let output = run_make(&["demo", "CARGO=echo RESOLVED:"]);
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        output.status.success(),
        "the default demo must pass the whitelist guard; stdout: {stdout} stderr: {stderr}"
    );
    let resolved = stdout
        .lines()
        .find(|line| line.starts_with("RESOLVED:"))
        .unwrap_or_else(|| panic!("the recipe must invoke CARGO; stdout: {stdout}"));
    for fragment in [
        "run -p thysalion-demos",
        "--features demo-empty",
        "--bin demo-empty",
    ] {
        assert!(
            resolved.contains(fragment),
            "the resolved command must contain {fragment:?}, got: {resolved}"
        );
    }
    // Guards against a whitelist that silently resolved to some other
    // demo: only `demo-empty` may appear.
    let other_slugs: Vec<&str> = resolved
        .split_whitespace()
        .filter(|token| token.starts_with("demo-") && *token != "demo-empty")
        .collect();
    assert!(
        other_slugs.is_empty(),
        "no demo other than demo-empty may appear, found {other_slugs:?} in: {resolved}"
    );
}
