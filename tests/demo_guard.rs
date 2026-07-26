//! Guards for the `make demo` launcher.
//!
//! The recipe must derive its whitelist of supported demo names from the
//! demo binaries on disk and reject an unsupported (even well-formed)
//! `DEMO` value *before* Cargo is invoked. These tests pin both halves so
//! the guard cannot be removed silently: one asserts the Makefile's
//! derivation shape, the other actually runs `make demo` with an
//! unsupported name and checks it fails at the guard, not in Cargo.

use std::process::Command;

use cap_std::{ambient_authority, fs_utf8::Dir};

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
    let output = Command::new("make")
        .arg("demo")
        .arg("DEMO=not-a-real-demo")
        .current_dir(env!("CARGO_MANIFEST_DIR"))
        .env_remove("MAKEFLAGS")
        .env_remove("MFLAGS")
        .output()
        .expect("run make demo");
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
