//! The `scene-check` operator tool, as a library.
//!
//! Not merely a printer. Continuous integration and the fixture generator's own
//! test suite both consume it, so it needs a contract, and the contract lives
//! here rather than in the thin `examples/scene-check.rs` wrapper — a binary is
//! not testable, and the exit codes are precisely what wants testing.
//!
//! The tool returns text and an [`ExitCode`]; it never writes to a stream
//! itself. That is what lets the whole contract be exercised in-process against
//! a [`MemorySceneSource`](crate::source::MemorySceneSource), and it is also why
//! nothing here needs `print!`, which the workspace denies outright.
//!
//! Four exit codes, because "non-zero" cannot distinguish a bad scene from a
//! broken tool, and a continuous-integration job that cannot tell them apart
//! reports a missing fixture as a validation failure.

mod cli;

use camino::Utf8Path;
pub use cli::{Invocation, USAGE, parse};

use crate::{
    loader::{SceneLoadError, SceneLoader},
    scene::validation::{Report, SceneStats, Strictness},
};

/// What the process should exit with.
///
/// Follows `sysexits.h` where it has an opinion: 64 is `EX_USAGE`. The rest are
/// this tool's own, and 1 against 2 is the distinction that earns the enum — an
/// invalid scene is the tool working, and an unreadable file is the tool being
/// unable to start.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum ExitCode {
    /// The scene is valid, and acceptable under the requested strictness.
    Valid,
    /// The scene parsed but failed validation.
    Invalid,
    /// The document or one of its resources could not be read.
    SourceFailure,
    /// The command line was wrong.
    Usage,
}

impl ExitCode {
    /// The process exit status.
    #[must_use]
    pub const fn get(self) -> i32 {
        match self {
            Self::Valid => 0,
            Self::Invalid => 1,
            Self::SourceFailure => 2,
            Self::Usage => 64,
        }
    }
}

/// How the report should be rendered.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OutputFormat {
    /// Text for a human reader, pinned by `insta` snapshots.
    Text,
    /// Structured data for a program. The fixture generator's tests read this.
    Json,
}

/// What to check, and how.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub struct Options {
    /// Whether to render text or structured data.
    pub format: OutputFormat,
    /// Whether warnings fail the check.
    pub strictness: Strictness,
    /// Whether to include the size and count measurements.
    pub stats: bool,
}

impl Default for Options {
    /// Text, lenient, no measurements: what a contributor running the tool by
    /// hand wants. Continuous integration passes `--strict` explicitly, so the
    /// stricter behaviour is a visible choice in the workflow file rather than a
    /// default a local run silently diverges from.
    fn default() -> Self {
        Self {
            format: OutputFormat::Text,
            strictness: Strictness::Lenient,
            stats: false,
        }
    }
}

/// What the tool produced.
#[derive(Debug, Clone)]
#[non_exhaustive]
pub struct Outcome {
    /// The rendered report, ready to write to a stream.
    pub output: String,
    /// What the process should exit with.
    pub code: ExitCode,
}

/// Checks the scene document at `path` through `loader`.
///
/// `root` names the resolved source root in the report. The loader already holds
/// the source, but the label cannot be recovered from a `dyn SceneSource`
/// without widening the port for the sake of one diagnostic line, so the caller
/// that opened the root supplies it.
#[must_use]
pub fn run(loader: &SceneLoader, path: &Utf8Path, root: &str, options: Options) -> Outcome {
    let checked = match loader.load(path) {
        Ok(checked) => checked,
        Err(error) => return failed(&error, path, root, options),
    };

    let mut report = Report::new(path, root).with_warnings(&checked.warnings);
    if options.stats {
        match SceneStats::of(&checked.scene) {
            Ok(stats) => report = report.with_stats(stats),
            Err(error) => return tool_failure(&error.to_string(), options),
        }
    }
    let code = if report.is_acceptable(options.strictness) {
        ExitCode::Valid
    } else {
        ExitCode::Invalid
    };
    render(&report, options, code)
}

/// Renders a failed load.
///
/// The split is between *the document is wrong* and *the tool never got to look
/// at it*, not between "has diagnostics" and "does not". A document that fails
/// to parse is wrong — the bytes were read perfectly well, and a mistyped field
/// name under `deny_unknown_fields` lands here rather than in a diagnostic
/// because a parse failure yields no document to locate a fault within. Its
/// `serde` path (`palette[2].emission.intensity`) is the locator instead, and
/// reporting it as a source failure would tell a contributor their *file* is
/// unreadable when in fact one key is misspelled.
fn failed(error: &SceneLoadError, path: &Utf8Path, root: &str, options: Options) -> Outcome {
    let diagnostics = error.diagnostics();
    if !diagnostics.is_empty() {
        let report = Report::new(path, root).with_errors(diagnostics);
        return render(&report, options, ExitCode::Invalid);
    }

    let code = match error {
        SceneLoadError::Malformed { .. } => ExitCode::Invalid,
        SceneLoadError::Source { .. } | SceneLoadError::UnknownEncoding { .. } => {
            ExitCode::SourceFailure
        }
        // `Invalid` is handled above by its diagnostics, and a later variant is
        // safest treated as a tool problem rather than as a verdict on the
        // document.
        _ => ExitCode::SourceFailure,
    };
    // Folded into the report rather than appended to its rendered output.
    // Neither is a diagnostic *about a place in the document*, so neither
    // belongs in the `errors` array — but appending text after a rendered
    // report puts a bare line after a closed JSON object under `--json`, and
    // that is not JSON any consumer can parse.
    render(
        &Report::new(path, root).with_failure(label(code), error.to_string()),
        options,
        code,
    )
}

/// The name of the kind of failure a report carries.
const fn label(code: ExitCode) -> &'static str {
    match code {
        ExitCode::Invalid => "malformed document",
        _ => "source failure",
    }
}

/// Renders a report in the requested format.
fn render(report: &Report, options: Options, code: ExitCode) -> Outcome {
    let rendered = match options.format {
        OutputFormat::Text => Ok(report.to_text()),
        OutputFormat::Json => report.to_json(),
    };
    match rendered {
        Ok(output) => Outcome { output, code },
        Err(error) => tool_failure(&error.to_string(), options),
    }
}

/// The tool-failure object, or a last-resort literal if even that will not
/// encode.
///
/// The fallback cannot arise — the value is one string — but this crate does
/// not use `expect`, and a panic here would replace a legible failure with a
/// backtrace.
fn encode_tool_error(message: &str) -> String {
    serde_json::to_string_pretty(&serde_json::json!({ "tool_error": message })).map_or_else(
        |_| String::from("{\n  \"tool_error\": \"unrenderable\"\n}\n"),
        |mut json| {
            json.push('\n');
            json
        },
    )
}

/// A failure in this crate rather than a fault in the document.
///
/// Reports [`ExitCode::SourceFailure`] rather than `Invalid`, because a report
/// that will not render says nothing about whether the scene is valid, and
/// telling a contributor their scene is broken when the tool is would send them
/// editing a correct document.
fn tool_failure(message: &str, options: Options) -> Outcome {
    let output = match options.format {
        OutputFormat::Text => format!("scene-check failed: {message}\n"),
        // Encoded rather than formatted with `{:?}`. Rust's `Debug` escaping
        // for `str` and JSON's string escaping agree on the common cases and
        // diverge on some control characters, so a message carrying one would
        // render this published contract unparseable.
        OutputFormat::Json => encode_tool_error(message),
    };
    Outcome {
        output,
        code: ExitCode::SourceFailure,
    }
}
