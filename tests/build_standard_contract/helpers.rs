//! Fallible readers and predicates shared by the Make routing contract tests.

use std::{error::Error, fmt, process::Command};

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

/// A shell excerpt emitted by an evaluated Make route.
#[derive(Clone, Copy)]
pub(crate) struct RouteText<'a>(pub(super) &'a str);

impl std::ops::Deref for RouteText<'_> {
    type Target = str;

    fn deref(&self) -> &Self::Target { self.0 }
}

impl fmt::Display for RouteText<'_> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(self.0, formatter)
    }
}

/// A built-in Cargo environment variable selecting a profile backend.
#[derive(Clone, Copy)]
pub(super) struct EnvVariable(pub(super) &'static str);

/// Built-in profile selectors that may override a route's codegen backend.
pub(super) const PROFILE_BACKEND_SELECTORS: [EnvVariable; 8] = [
    EnvVariable("CARGO_PROFILE_DEV_CODEGEN_BACKEND"),
    EnvVariable("CARGO_PROFILE_DEV_BUILD_OVERRIDE_CODEGEN_BACKEND"),
    EnvVariable("CARGO_PROFILE_TEST_CODEGEN_BACKEND"),
    EnvVariable("CARGO_PROFILE_TEST_BUILD_OVERRIDE_CODEGEN_BACKEND"),
    EnvVariable("CARGO_PROFILE_RELEASE_CODEGEN_BACKEND"),
    EnvVariable("CARGO_PROFILE_RELEASE_BUILD_OVERRIDE_CODEGEN_BACKEND"),
    EnvVariable("CARGO_PROFILE_BENCH_CODEGEN_BACKEND"),
    EnvVariable("CARGO_PROFILE_BENCH_BUILD_OVERRIDE_CODEGEN_BACKEND"),
];

/// Host categories that affect the development linker flags.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum BuildHost {
    Linux,
    Other,
}

/// Make entrypoints exercised by the build-routing contract.
#[derive(Clone, Copy)]
pub(super) enum MakeTarget {
    Build,
    Test,
    Typecheck,
    Clippy,
    Demo,
    Coverage,
    Release,
    Fmt,
    CheckFmt,
    Lint,
    LintWhitaker,
}

impl MakeTarget {
    /// Returns the Make target selected by this case.
    pub(super) const fn as_str(self) -> &'static str {
        match self {
            Self::Build => "build",
            Self::Test => "test",
            Self::Typecheck => "typecheck",
            Self::Clippy => "lint-clippy",
            Self::Demo => "demo",
            Self::Coverage => "coverage",
            Self::Release => "release",
            Self::Fmt => "fmt",
            Self::CheckFmt => "check-fmt",
            Self::Lint => "lint",
            Self::LintWhitaker => "lint-whitaker",
        }
    }
}

/// One expected shell environment assignment used by a route contract.
#[derive(Clone, Copy)]
struct EnvAssignment {
    name: &'static str,
    value: &'static str,
}

const COVERAGE_ASSIGNMENTS: [EnvAssignment; 5] = [
    EnvAssignment {
        name: "CARGO_PROFILE_DEV_CODEGEN_BACKEND",
        value: "llvm",
    },
    EnvAssignment {
        name: "CARGO_PROFILE_TEST_CODEGEN_BACKEND",
        value: "llvm",
    },
    EnvAssignment {
        name: "CARGO_PROFILE_DEV_BUILD_OVERRIDE_CODEGEN_BACKEND",
        value: "llvm",
    },
    EnvAssignment {
        name: "CARGO_PROFILE_TEST_BUILD_OVERRIDE_CODEGEN_BACKEND",
        value: "llvm",
    },
    EnvAssignment {
        name: "CARGO_UNSTABLE_CODEGEN_BACKEND",
        value: "true",
    },
];

const WHITAKER_ASSIGNMENTS: [EnvAssignment; 5] = [
    EnvAssignment {
        name: "CARGO_UNSTABLE_CODEGEN_BACKEND",
        value: "true",
    },
    EnvAssignment {
        name: "CARGO_PROFILE_DEV_CODEGEN_BACKEND",
        value: "llvm",
    },
    EnvAssignment {
        name: "CARGO_PROFILE_TEST_CODEGEN_BACKEND",
        value: "llvm",
    },
    EnvAssignment {
        name: "CARGO_PROFILE_DEV_BUILD_OVERRIDE_CODEGEN_BACKEND",
        value: "llvm",
    },
    EnvAssignment {
        name: "CARGO_PROFILE_TEST_BUILD_OVERRIDE_CODEGEN_BACKEND",
        value: "llvm",
    },
];

const WHITAKER_UNSET_BACKEND_SELECTORS: [EnvVariable; 4] = [
    EnvVariable("CARGO_PROFILE_RELEASE_CODEGEN_BACKEND"),
    EnvVariable("CARGO_PROFILE_RELEASE_BUILD_OVERRIDE_CODEGEN_BACKEND"),
    EnvVariable("CARGO_PROFILE_BENCH_CODEGEN_BACKEND"),
    EnvVariable("CARGO_PROFILE_BENCH_BUILD_OVERRIDE_CODEGEN_BACKEND"),
];

const ENCODED_FLAGS: EnvVariable = EnvVariable("CARGO_ENCODED_RUSTFLAGS");
const UNSTABLE_BACKEND: EnvVariable = EnvVariable("CARGO_UNSTABLE_CODEGEN_BACKEND");

/// The result of a fallible contract reader.
pub(crate) type Read<T> = Result<T, Box<dyn Error>>;

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
pub(super) fn make_output(target: MakeTarget, overrides: &[&str]) -> Read<String> {
    let mut command = Command::new("make");
    command
        .args(["--dry-run", "--always-make"])
        .current_dir(env!("CARGO_MANIFEST_DIR"));
    command.args(overrides).arg(target.as_str()).envs([
        ("CARGO_ENCODED_RUSTFLAGS", "-Copt-level=0"),
        ("CARGO_UNSTABLE_CODEGEN_BACKEND", "true"),
    ]);
    for selector in PROFILE_BACKEND_SELECTORS {
        command.env(selector.0, "cranelift");
    }
    if matches!(target, MakeTarget::Coverage) {
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

#[path = "route_matchers.rs"]
mod route_matchers;
pub(super) use route_matchers::{
    build_override_mutations,
    cargo_lines,
    coverage_mutations,
    coverage_route_matches,
    development_route_matches,
    preflight_precedes_cargo,
    release_route_matches,
    whitaker_route_matches,
};
