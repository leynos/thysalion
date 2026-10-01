//! Contract test: the Cargo configuration stays readable by stable Cargo.
//!
//! `release.yml` builds release binaries with `cross +stable`, and Cargo reads
//! `.cargo/config.toml` inside that build. Stable Cargo refuses a configuration
//! naming any `codegen-backend` key ("feature `codegen-backend` is required"),
//! so while the release workflow builds on stable the configuration must name
//! none. That is why the repository carries a Cranelift exception to the build
//! standard; see "Cranelift exception" in `docs/developers-guide.md`.
//!
//! Two tests hold this. The first reads the configuration and the workflow, so
//! the failure names the key and the reason. The second hands the repository's
//! configuration to a real `cargo +stable check` in a throwaway crate, so a
//! refusal the first test does not anticipate still fails the suite.
//!
//! File access goes through `cap_std` directory handles: one rooted at the
//! crate manifest directory, one at the fixture crate under
//! `CARGO_TARGET_TMPDIR`.

use std::{error::Error, process::Command};

use cap_std::{ambient_authority, fs::Dir};

/// The result of a reader, which the test unwraps.
type Read<T> = Result<T, Box<dyn Error>>;

/// The configuration key stable Cargo refuses.
const UNSTABLE_KEY: &str = "codegen-backend";
/// Backend-related Cargo settings cleared by the release workflow.
const BACKEND_ENVIRONMENT_VARIABLES: [&str; 10] = [
    "CARGO_ENCODED_RUSTFLAGS",
    "CARGO_UNSTABLE_CODEGEN_BACKEND",
    "CARGO_PROFILE_DEV_CODEGEN_BACKEND",
    "CARGO_PROFILE_DEV_BUILD_OVERRIDE_CODEGEN_BACKEND",
    "CARGO_PROFILE_TEST_CODEGEN_BACKEND",
    "CARGO_PROFILE_TEST_BUILD_OVERRIDE_CODEGEN_BACKEND",
    "CARGO_PROFILE_RELEASE_CODEGEN_BACKEND",
    "CARGO_PROFILE_RELEASE_BUILD_OVERRIDE_CODEGEN_BACKEND",
    "CARGO_PROFILE_BENCH_CODEGEN_BACKEND",
    "CARGO_PROFILE_BENCH_BUILD_OVERRIDE_CODEGEN_BACKEND",
];

/// Reads a file relative to the crate manifest directory.
fn read(path: &str) -> Read<String> {
    let root = Dir::open_ambient_dir(env!("CARGO_MANIFEST_DIR"), ambient_authority())?;
    Ok(root.read_to_string(path)?)
}

/// Returns whether the workflow's release build runs on stable: it has at
/// least one `cross ... build` command, and every one selects `+stable`.
///
/// The check reads the release build commands alone, the lines
/// `tests/workflow_shape.rs` also reads, so a `+stable` elsewhere in the
/// workflow cannot keep it green.
fn release_builds_on_stable(workflow: &str) -> bool {
    let builds: Vec<&str> = workflow
        .lines()
        .filter(|line| !line.trim_start().starts_with('#'))
        .filter(|line| line.contains("cross ") && line.contains("build"))
        .collect();
    !builds.is_empty()
        && builds
            .iter()
            .all(|line| line.split_whitespace().any(|word| word == "+stable"))
}

/// Collects the dotted path of every `codegen-backend` key under `value`.
fn backend_keys(value: &toml::Value, path: &str, found: &mut Vec<String>) {
    let Some(table) = value.as_table() else {
        return;
    };
    for (key, child) in table {
        let child_path = if path.is_empty() {
            key.clone()
        } else {
            format!("{path}.{key}")
        };
        if key == UNSTABLE_KEY {
            found.push(child_path.clone());
        }
        backend_keys(child, &child_path, found);
    }
}

#[test]
fn configuration_names_no_codegen_backend_while_releases_build_on_stable() {
    let workflow = read(".github/workflows/release.yml").expect("read release.yml");
    assert!(
        release_builds_on_stable(&workflow),
        "release.yml's cross build no longer runs on +stable; revisit the Cranelift exception and \
         this test"
    );
    let text = read(".cargo/config.toml").expect("read .cargo/config.toml");
    let config: toml::Value = toml::from_str(&text).expect("parse .cargo/config.toml");
    let mut found = Vec::new();
    backend_keys(&config, "", &mut found);
    assert!(
        found.is_empty(),
        "stable Cargo refuses `{UNSTABLE_KEY}` keys, but .cargo/config.toml names {found:?}"
    );
}

/// The manifest of the throwaway crate the stable smoke test checks.
///
/// The empty `[workspace]` table detaches it from any enclosing workspace, so
/// Cargo reads the copied configuration and nothing else from the tree.
const FIXTURE_MANIFEST: &str = "[package]\nname = \"stable_fixture\"\nversion = \
                                \"0.0.0\"\nedition = \"2021\"\n\n[workspace]\n";

/// Writes the fixture crate, carrying a copy of the repository's Cargo
/// configuration, and returns the directory it was written under.
fn write_fixture() -> Read<std::path::PathBuf> {
    let config = read(".cargo/config.toml")?;
    let path = std::path::Path::new(env!("CARGO_TARGET_TMPDIR")).join("stable-config-fixture");
    // A clean directory keeps a stale `Cargo.lock` or target from an earlier run out.
    Dir::open_ambient_dir(env!("CARGO_TARGET_TMPDIR"), ambient_authority())?
        .remove_dir_all("stable-config-fixture")
        .or_else(|error| match error.kind() {
            std::io::ErrorKind::NotFound => Ok(()),
            _ => Err(error),
        })?;
    Dir::create_ambient_dir_all(path.join("src"), ambient_authority())?;
    Dir::create_ambient_dir_all(path.join(".cargo"), ambient_authority())?;
    let root = Dir::open_ambient_dir(&path, ambient_authority())?;
    root.write("Cargo.toml", FIXTURE_MANIFEST)?;
    root.write("src/lib.rs", "")?;
    root.write(".cargo/config.toml", config)?;
    Ok(path)
}

#[test]
fn stable_cargo_accepts_the_repository_configuration() {
    let fixture = write_fixture().expect("write the fixture crate");
    // `check` resolves profiles, which is where stable Cargo refuses an
    // unstable key; it needs no network for a crate without dependencies. The
    // The release step assigns empty `RUSTFLAGS` and clears the inherited
    // backend selectors before invoking stable Cargo. Mirror that boundary so
    // coverage's LLVM overrides cannot leak into this nested process. The
    // other wrappers and target directories are removed so the fixture builds
    // under the copied configuration alone.
    let mut command = Command::new("cargo");
    command
        .args(["+stable", "check", "--offline"])
        .current_dir(&fixture)
        .env("CARGO_TARGET_DIR", fixture.join("target"))
        .env_remove("CARGO_BUILD_BUILD_DIR")
        .env("RUSTFLAGS", "")
        .env_remove("RUSTC_WRAPPER")
        .env_remove("RUSTC_WORKSPACE_WRAPPER");
    for variable in BACKEND_ENVIRONMENT_VARIABLES {
        command.env_remove(variable);
    }
    let output = command
        .output()
        .expect("run `cargo +stable check`; is the stable toolchain installed?");
    assert!(
        output.status.success(),
        "stable Cargo rejects .cargo/config.toml, so `cross +stable build` would fail:\n{}",
        String::from_utf8_lossy(&output.stderr)
    );
}
