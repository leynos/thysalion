//! Fallible readers and predicates shared by the Make routing contract tests.

use std::{error::Error, process::Command};

use cap_std::{ambient_authority, fs_utf8::Dir};

/// The parallel-frontend flag required for development builds.
pub(super) const THREADS_FLAG: &str = "-Zthreads=8";

/// The Linux linker flag, normalized to one token.
pub(super) const MOLD_FLAG: &str = "-Clink-arg=-fuse-ld=mold";

/// The linker flag used by LLVM coverage builds.
const LLD_FLAG: &str = "fuse-ld=lld";

/// Linux target table keys, where mold is supported.
pub(super) const LINUX_TABLES: [&str; 2] =
    ["x86_64-unknown-linux-gnu", "cfg(target_os = \"linux\")"];

/// Built-in profile selectors that may override a route's codegen backend.
pub(super) const PROFILE_BACKEND_SELECTORS: [&str; 8] = [
    "CARGO_PROFILE_DEV_CODEGEN_BACKEND",
    "CARGO_PROFILE_DEV_BUILD_OVERRIDE_CODEGEN_BACKEND",
    "CARGO_PROFILE_TEST_CODEGEN_BACKEND",
    "CARGO_PROFILE_TEST_BUILD_OVERRIDE_CODEGEN_BACKEND",
    "CARGO_PROFILE_RELEASE_CODEGEN_BACKEND",
    "CARGO_PROFILE_RELEASE_BUILD_OVERRIDE_CODEGEN_BACKEND",
    "CARGO_PROFILE_BENCH_CODEGEN_BACKEND",
    "CARGO_PROFILE_BENCH_BUILD_OVERRIDE_CODEGEN_BACKEND",
];

/// The result of a fallible contract reader.
pub(super) type Read<T> = Result<T, Box<dyn Error>>;

/// Joins `-C value` pairs into `-Cvalue`, so both spellings compare equal.
fn normalized(flags: &[String]) -> Vec<String> {
    let mut joined: Vec<String> = Vec::new();
    for flag in flags {
        match joined.last_mut() {
            Some(last) if last == "-C" => *last = format!("-C{flag}"),
            _ => joined.push(flag.clone()),
        }
    }
    joined
}

/// Returns whether a flag list names one flag.
pub(super) fn names(flags: &[String], flag: &str) -> bool {
    flags.iter().any(|candidate| candidate == flag)
}

/// Reads the Cargo configuration as TOML.
pub(super) fn cargo_config() -> Read<toml::Value> {
    let root = Dir::open_ambient_dir(env!("CARGO_MANIFEST_DIR"), ambient_authority())?;
    let text = root.read_to_string(".cargo/config.toml")?;
    Ok(toml::from_str(&text)?)
}

/// Reads one table's `rustflags` as a list of strings, if it has one.
fn table_flags(table: &toml::Value) -> Option<Vec<String>> {
    let flags = table.get("rustflags")?.as_array()?;
    Some(
        flags
            .iter()
            .filter_map(|flag| flag.as_str().map(str::to_owned))
            .collect(),
    )
}

/// Returns every configured rustflag source, by table name.
pub(super) fn sources() -> Read<Vec<(String, Vec<String>)>> {
    let config = cargo_config()?;
    let mut found = Vec::new();
    if let Some(flags) = config.get("build").and_then(table_flags) {
        found.push(("build".to_owned(), normalized(&flags)));
    }
    if let Some(targets) = config.get("target").and_then(toml::Value::as_table) {
        for (key, table) in targets {
            if let Some(flags) = table_flags(table) {
                found.push((key.clone(), normalized(&flags)));
            }
        }
    }
    Ok(found)
}

/// Returns evaluated dry-run output with hostile ambient backend overrides.
pub(super) fn make_output(target: &str, overrides: &[&str]) -> Read<String> {
    let mut command = Command::new("make");
    command
        .args(["--dry-run", "--always-make"])
        .current_dir(env!("CARGO_MANIFEST_DIR"));
    command.args(overrides).arg(target).envs([
        ("CARGO_ENCODED_RUSTFLAGS", "-Copt-level=0"),
        ("CARGO_UNSTABLE_CODEGEN_BACKEND", "true"),
    ]);
    for selector in PROFILE_BACKEND_SELECTORS {
        command.env(selector, "cranelift");
    }
    if target == "coverage" {
        command.envs([
            ("CARGO_PROFILE_DEV_CODEGEN_BACKEND", "cranelift"),
            ("CARGO_PROFILE_TEST_CODEGEN_BACKEND", "cranelift"),
            (
                "CARGO_PROFILE_DEV_BUILD_OVERRIDE_CODEGEN_BACKEND",
                "cranelift",
            ),
            (
                "CARGO_PROFILE_TEST_BUILD_OVERRIDE_CODEGEN_BACKEND",
                "cranelift",
            ),
        ]);
    }
    let output = command.output()?;
    if !output.status.success() {
        return Err(String::from_utf8_lossy(&output.stderr).to_string().into());
    }
    Ok(String::from_utf8_lossy(&output.stdout).replace("\\\n", " "))
}

/// Returns logical command lines that invoke the injected Cargo executable.
pub(super) fn cargo_lines(output: &str) -> Vec<&str> {
    output
        .lines()
        .filter(|line| line.contains("probe-cargo"))
        .collect()
}

/// Returns the rustflags assigned to one evaluated Cargo command.
fn assigned_flags(command: &str) -> Read<Vec<String>> {
    let (_, rest) = command
        .split_once("RUSTFLAGS=\"")
        .ok_or_else(|| format!("Cargo command does not assign RUSTFLAGS: {command}"))?;
    let (value, _) = rest
        .split_once('"')
        .ok_or_else(|| format!("unterminated RUSTFLAGS in `{command}`"))?;
    let words: Vec<String> = value.split_whitespace().map(str::to_owned).collect();
    Ok(normalized(&words))
}

/// Requires one unset of a caller's override before the route assigns flags.
fn unset_before_flags(command: &str, selector: &str) -> bool {
    let Some((environment, _)) = command.split_once("RUSTFLAGS=\"") else {
        return false;
    };
    environment
        .split_whitespace()
        .collect::<Vec<_>>()
        .windows(2)
        .filter(|pair| pair.first() == Some(&"-u") && pair.get(1) == Some(&selector))
        .count()
        == 1
}

/// Checks that all Cargo profile backend selectors are isolated from callers.
fn backend_selectors_unset(command: &str) -> bool {
    PROFILE_BACKEND_SELECTORS
        .iter()
        .all(|selector| unset_before_flags(command, selector))
}

/// Checks that a development command clears encoded and backend overrides.
pub(super) fn development_route_matches(command: &str, host: &str) -> Read<bool> {
    let flags = assigned_flags(command)?;
    Ok(unset_before_flags(command, "CARGO_ENCODED_RUSTFLAGS")
        && unset_before_flags(command, "CARGO_UNSTABLE_CODEGEN_BACKEND")
        && backend_selectors_unset(command)
        && !PROFILE_BACKEND_SELECTORS
            .iter()
            .any(|selector| command.contains(format!("{selector}=").as_str()))
        && flags.join(" ").contains("-D warnings")
        && names(&flags, THREADS_FLAG)
        && names(&flags, MOLD_FLAG) == (host == "Linux"))
}

/// Checks the explicit LLVM coverage backend and linker exclusion.
pub(super) fn coverage_route_matches(command: &str) -> bool {
    let Some((environment, arguments)) = command.split_once("probe-cargo llvm-cov") else {
        return false;
    };
    let has_llvm_backend =
        single_env_assignment(environment, "CARGO_PROFILE_DEV_CODEGEN_BACKEND", "llvm")
            && single_env_assignment(environment, "CARGO_PROFILE_TEST_CODEGEN_BACKEND", "llvm")
            && single_env_assignment(
                environment,
                "CARGO_PROFILE_DEV_BUILD_OVERRIDE_CODEGEN_BACKEND",
                "llvm",
            )
            && single_env_assignment(
                environment,
                "CARGO_PROFILE_TEST_BUILD_OVERRIDE_CODEGEN_BACKEND",
                "llvm",
            )
            && single_env_assignment(environment, "CARGO_UNSTABLE_CODEGEN_BACKEND", "true");
    let has_no_config_override = !arguments
        .split_whitespace()
        .any(|argument| argument == "--config" || argument.starts_with("--config="));
    let clears_encoded = environment.starts_with("env -u CARGO_ENCODED_RUSTFLAGS ")
        && !environment.contains("CARGO_ENCODED_RUSTFLAGS=");
    let has_coverage_rustflags = environment.matches("RUSTFLAGS=\"").count() == 1
        && environment
            .split_once("RUSTFLAGS=\"")
            .and_then(|(_, rest)| rest.split_once('"').map(|(flags, _)| flags))
            .is_some_and(|flags| {
                flags.contains("-D warnings")
                    && flags.contains(LLD_FLAG)
                    && !flags.contains(THREADS_FLAG)
                    && !flags.contains(MOLD_FLAG)
            });
    has_llvm_backend && has_no_config_override && clears_encoded && has_coverage_rustflags
}

/// Requires one effective pre-Cargo assignment, such as `BACKEND=llvm`.
fn single_env_assignment(environment: &str, name: &str, value: &str) -> bool {
    let prefix = format!("{name}=");
    let expected = format!("{prefix}{value}");
    let mut assignments = environment
        .split_whitespace()
        .filter(|word| word.starts_with(&prefix));
    assignments.next() == Some(expected.as_str()) && assignments.next().is_none()
}

/// Mutates each profile's build-script backend and its command-line precedence.
pub(super) fn build_override_mutations(command: &str) -> Vec<(&'static str, String)> {
    let mut mutations = Vec::new();
    for (profile, missing, cranelift, duplicate, config) in [
        (
            "DEV",
            "missing dev build override",
            "Cranelift dev build override",
            "later dev build override",
            "dev build-override config",
        ),
        (
            "TEST",
            "missing test build override",
            "Cranelift test build override",
            "later test build override",
            "test build-override config",
        ),
    ] {
        let variable = format!("CARGO_PROFILE_{profile}_BUILD_OVERRIDE_CODEGEN_BACKEND");
        let llvm = format!("{variable}=llvm");
        let hostile = format!("{variable}=cranelift");
        mutations.push((missing, command.replace(&format!("{llvm} "), "")));
        mutations.push((cranelift, command.replace(&llvm, &hostile)));
        mutations.push((
            duplicate,
            command.replace("RUSTFLAGS=\"", &format!("{hostile} RUSTFLAGS=\"")),
        ));
        let profile_name = profile.to_ascii_lowercase();
        let override_arg = format!(
            "probe-cargo llvm-cov --config \
             profile.{profile_name}.build-override.codegen-backend=cranelift"
        );
        mutations.push((
            config,
            command.replace("probe-cargo llvm-cov", &override_arg),
        ));
    }
    mutations
}

/// Changes one coverage route requirement at a time.
pub(super) fn coverage_mutations(command: &str) -> [(&'static str, String); 11] {
    [
        (
            "Cranelift backend",
            command.replace(
                "CARGO_PROFILE_DEV_CODEGEN_BACKEND=llvm",
                "CARGO_PROFILE_DEV_CODEGEN_BACKEND=cranelift",
            ),
        ),
        (
            "later backend override",
            command.replace(
                "RUSTFLAGS=\"",
                "CARGO_PROFILE_DEV_CODEGEN_BACKEND=cranelift RUSTFLAGS=\"",
            ),
        ),
        (
            "missing test backend",
            command.replace("CARGO_PROFILE_TEST_CODEGEN_BACKEND=llvm ", ""),
        ),
        (
            "Cranelift test backend",
            command.replace(
                "CARGO_PROFILE_TEST_CODEGEN_BACKEND=llvm",
                "CARGO_PROFILE_TEST_CODEGEN_BACKEND=cranelift",
            ),
        ),
        (
            "later test backend override",
            command.replace(
                "RUSTFLAGS=\"",
                "CARGO_PROFILE_TEST_CODEGEN_BACKEND=cranelift RUSTFLAGS=\"",
            ),
        ),
        (
            "disabled backend feature",
            command.replace(
                "CARGO_UNSTABLE_CODEGEN_BACKEND=true",
                "CARGO_UNSTABLE_CODEGEN_BACKEND=false",
            ),
        ),
        (
            "Cargo config override",
            command.replace(
                "probe-cargo llvm-cov",
                "probe-cargo --config profile.dev.codegen-backend=cranelift llvm-cov",
            ),
        ),
        (
            "test config override",
            command.replace(
                "probe-cargo llvm-cov",
                "probe-cargo llvm-cov --config profile.test.codegen-backend=cranelift",
            ),
        ),
        (
            "mold linker",
            command.replace("fuse-ld=lld", "fuse-ld=mold"),
        ),
        (
            "missing encoded-flag guard",
            command.replace("env -u CARGO_ENCODED_RUSTFLAGS ", ""),
        ),
        (
            "reassigned encoded flags",
            command.replace("PATH=", "CARGO_ENCODED_RUSTFLAGS=-Zthreads=8 PATH="),
        ),
    ]
}

/// Returns whether the build-tool preflight precedes all Cargo commands.
pub(super) fn preflight_precedes_cargo(output: &str) -> bool {
    let Some(preflight) = output.find("scripts/check-build-tools.sh") else {
        return false;
    };
    cargo_lines(output).iter().all(|line| {
        output
            .find(line)
            .is_some_and(|position| preflight < position)
    })
}

/// Returns whether a release route selects stable from outside the repository.
pub(super) fn release_route_matches(command: &str, manifest: &str) -> bool {
    command.starts_with("(cd / && env -u CARGO_ENCODED_RUSTFLAGS ")
        && unset_before_flags(command, "CARGO_ENCODED_RUSTFLAGS")
        && unset_before_flags(command, "CARGO_UNSTABLE_CODEGEN_BACKEND")
        && backend_selectors_unset(command)
        && !PROFILE_BACKEND_SELECTORS
            .iter()
            .any(|selector| command.contains(format!("{selector}=").as_str()))
        && command.contains("RUSTFLAGS=\"\" probe-cargo +stable build")
        && command.contains("--manifest-path")
        && command.contains(manifest)
        && command.contains("--release")
        && command.contains("--bin thysalion")
        && !command.contains(THREADS_FLAG)
        && !command.contains(MOLD_FLAG)
}

/// Checks the isolated LLVM route used by the installer-managed Whitaker suite.
pub(super) fn whitaker_route_matches(command: &str) -> bool {
    let Some((environment, _)) = command.split_once("probe-whitaker --all") else {
        return false;
    };
    unset_before_flags(environment, "CARGO_ENCODED_RUSTFLAGS")
        && backend_selectors_unset(environment)
        && environment.contains("RUSTFLAGS=\"\"")
        && single_env_assignment(environment, "CARGO_UNSTABLE_CODEGEN_BACKEND", "true")
        && [
            "CARGO_PROFILE_DEV_CODEGEN_BACKEND",
            "CARGO_PROFILE_TEST_CODEGEN_BACKEND",
            "CARGO_PROFILE_DEV_BUILD_OVERRIDE_CODEGEN_BACKEND",
            "CARGO_PROFILE_TEST_BUILD_OVERRIDE_CODEGEN_BACKEND",
        ]
        .iter()
        .all(|selector| single_env_assignment(environment, selector, "llvm"))
        && PROFILE_BACKEND_SELECTORS
            .iter()
            .skip(4)
            .all(|selector| !environment.contains(format!("{selector}=").as_str()))
        && !command.contains(THREADS_FLAG)
        && !command.contains(MOLD_FLAG)
}
