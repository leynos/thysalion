//! Match evaluated Make route text against the build-routing contract.

use super::{
    BuildHost,
    COVERAGE_ASSIGNMENTS,
    ENCODED_FLAGS,
    EnvAssignment,
    EnvVariable,
    LLD_FLAG,
    MOLD_FLAG,
    PROFILE_BACKEND_SELECTORS,
    Read,
    RouteText,
    THREADS_FLAG,
    UNSTABLE_BACKEND,
    WHITAKER_ASSIGNMENTS,
    WHITAKER_UNSET_BACKEND_SELECTORS,
    names,
    normalized,
};

/// Returns logical command lines that invoke the injected Cargo executable.
pub(crate) fn cargo_lines(output: RouteText<'_>) -> Vec<&str> {
    output
        .0
        .lines()
        .filter(|line| line.contains("probe-cargo"))
        .collect()
}

/// Returns the rustflags assigned to one evaluated Cargo command.
fn assigned_flags(command: RouteText<'_>) -> Read<Vec<String>> {
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
fn unset_before_flags(command: RouteText<'_>, selector: EnvVariable) -> bool {
    let Some((environment, _)) = command.split_once("RUSTFLAGS=\"") else {
        return false;
    };
    environment
        .split_whitespace()
        .collect::<Vec<_>>()
        .windows(2)
        .filter(|pair| pair.first() == Some(&"-u") && pair.get(1) == Some(&selector.0))
        .count()
        == 1
}

/// Checks that all Cargo profile backend selectors are isolated from callers.
fn backend_selectors_unset(command: RouteText<'_>) -> bool {
    PROFILE_BACKEND_SELECTORS
        .iter()
        .all(|selector| unset_before_flags(command, *selector))
}

/// Checks that a development command clears encoded and backend overrides.
pub(crate) fn development_route_matches(command: RouteText<'_>, host: BuildHost) -> Read<bool> {
    let flags = assigned_flags(command)?;
    Ok(unset_before_flags(command, ENCODED_FLAGS)
        && unset_before_flags(command, UNSTABLE_BACKEND)
        && backend_selectors_unset(command)
        && !PROFILE_BACKEND_SELECTORS
            .iter()
            .any(|selector| command.contains(format!("{}=", selector.0).as_str()))
        && flags.join(" ").contains("-D warnings")
        && names(&flags, THREADS_FLAG)
        && names(&flags, MOLD_FLAG) == (host == BuildHost::Linux))
}

/// Checks the explicit LLVM coverage backend and linker exclusion.
pub(crate) fn coverage_route_matches(command: RouteText<'_>) -> bool {
    let Some((environment_text, arguments)) = command.split_once("probe-cargo llvm-cov") else {
        return false;
    };
    let environment = RouteText(environment_text);
    [
        coverage_backend_is_llvm(environment),
        !arguments
            .split_whitespace()
            .any(|argument| argument == "--config" || argument.starts_with("--config=")),
        environment.starts_with("env -u CARGO_ENCODED_RUSTFLAGS ")
            && !environment.contains("CARGO_ENCODED_RUSTFLAGS="),
        coverage_rustflags_match(environment),
    ]
    .into_iter()
    .all(|condition| condition)
}

fn coverage_backend_is_llvm(environment: RouteText<'_>) -> bool {
    COVERAGE_ASSIGNMENTS
        .iter()
        .all(|assignment| single_env_assignment(environment, *assignment))
}

fn coverage_rustflags_match(environment: RouteText<'_>) -> bool {
    unique_rustflags(environment).is_some_and(|flags| {
        [
            flags.contains("-D warnings"),
            flags.contains(LLD_FLAG),
            !flags.contains(THREADS_FLAG),
            !flags.contains(MOLD_FLAG),
        ]
        .into_iter()
        .all(|condition| condition)
    })
}

fn unique_rustflags(environment: RouteText<'_>) -> Option<&str> {
    if environment.matches("RUSTFLAGS=\"").count() != 1 {
        return None;
    }
    environment
        .0
        .split_once("RUSTFLAGS=\"")
        .and_then(|(_, rest)| rest.split_once('"').map(|(flags, _)| flags))
}

/// Requires one effective pre-Cargo assignment, such as `BACKEND=llvm`.
fn single_env_assignment(environment: RouteText<'_>, assignment: EnvAssignment) -> bool {
    let prefix = format!("{}=", assignment.name);
    let expected = format!("{prefix}{}", assignment.value);
    let mut assignments = environment
        .split_whitespace()
        .filter(|word| word.starts_with(&prefix));
    assignments.next() == Some(expected.as_str()) && assignments.next().is_none()
}

/// Mutates each profile's build-script backend and its command-line precedence.
pub(crate) fn build_override_mutations(command: RouteText<'_>) -> Vec<(&'static str, String)> {
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
pub(crate) fn coverage_mutations(command: RouteText<'_>) -> [(&'static str, String); 11] {
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
pub(crate) fn preflight_precedes_cargo(output: RouteText<'_>) -> bool {
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
pub(crate) fn release_route_matches(command: RouteText<'_>, manifest: &str) -> bool {
    let isolated_environment = command.starts_with("(cd / && env -u CARGO_ENCODED_RUSTFLAGS ")
        && unset_before_flags(command, ENCODED_FLAGS)
        && unset_before_flags(command, UNSTABLE_BACKEND)
        && backend_selectors_unset(command)
        && PROFILE_BACKEND_SELECTORS
            .iter()
            .all(|selector| !command.contains(format!("{}=", selector.0).as_str()));
    isolated_environment && release_invocation_matches(command, manifest)
}

fn release_invocation_matches(command: RouteText<'_>, manifest: &str) -> bool {
    [
        command.contains("RUSTFLAGS=\"\" probe-cargo +stable build"),
        command.contains("--manifest-path"),
        command.contains(manifest),
        command.contains("--release"),
        command.contains("--bin thysalion"),
        !command.contains(THREADS_FLAG),
        !command.contains(MOLD_FLAG),
    ]
    .into_iter()
    .all(|condition| condition)
}

/// Checks the isolated LLVM route used by the installer-managed Whitaker suite.
pub(crate) fn whitaker_route_matches(command: RouteText<'_>) -> bool {
    let Some((environment_text, _)) = command.split_once("probe-whitaker --all") else {
        return false;
    };
    let environment = RouteText(environment_text);
    [
        unset_before_flags(environment, ENCODED_FLAGS),
        backend_selectors_unset(environment),
        environment.contains("RUSTFLAGS=\"\""),
        WHITAKER_ASSIGNMENTS
            .iter()
            .all(|assignment| single_env_assignment(environment, *assignment)),
        WHITAKER_UNSET_BACKEND_SELECTORS
            .iter()
            .all(|selector| !environment.contains(format!("{}=", selector.0).as_str())),
        !command.contains(THREADS_FLAG),
        !command.contains(MOLD_FLAG),
    ]
    .into_iter()
    .all(|condition| condition)
}
