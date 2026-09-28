//! Contract test: the Cargo configuration stays readable by stable Cargo.
//!
//! `release.yml` builds release binaries with `cross +stable`, and Cargo reads
//! `.cargo/config.toml` inside that build. Stable Cargo refuses a configuration
//! naming any `codegen-backend` key ("feature `codegen-backend` is required"),
//! so while the release workflow builds on stable the configuration must name
//! none. That is why the repository carries a Cranelift exception to the build
//! standard; see "Cranelift exception" in `docs/developers-guide.md`.
//!
//! File access goes through a `cap_std` directory handle rooted at the crate
//! manifest directory.

use std::error::Error;

use cap_std::{ambient_authority, fs::Dir};

/// The result of a reader, which the test unwraps.
type Read<T> = Result<T, Box<dyn Error>>;

/// The configuration key stable Cargo refuses.
const UNSTABLE_KEY: &str = "codegen-backend";

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
