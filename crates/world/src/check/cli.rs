//! Parsing the `scene-check` command line.
//!
//! Hand-rolled over three flags and one path. Reaching for an argument-parsing
//! crate would be a new external dependency, which the 1.2 execution plan's
//! tolerances make an escalation rather than a judgement call, and the surface
//! here does not come close to justifying one.
//!
//! Parsing is separated from [`super::run`] so that a wrong command line is
//! testable: [`ExitCode::Usage`](super::ExitCode::Usage) exists precisely so a
//! continuous-integration job can tell a mistyped flag from a broken scene, and
//! a contract nothing exercises is a contract nothing keeps.

use camino::Utf8PathBuf;

use super::{Options, OutputFormat};
use crate::scene::validation::Strictness;

/// The usage text, shown for `--help` and for a bad command line.
pub const USAGE: &str = "\
usage: scene-check [--json] [--strict] [--stats] <scene.json|scene.msgpack>

  --json    emit the report as structured data rather than text
  --strict  treat warnings as failures, as continuous integration does
  --stats   include voxel, run, and byte measurements
  --help    show this message

exit codes: 0 valid, 1 validation failed, 2 source failure, 64 usage error
";

/// What a command line asked for.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum Invocation {
    /// Check one document.
    Check {
        /// The document to check.
        path: Utf8PathBuf,
        /// How to check and render it.
        options: Options,
    },
    /// Show the usage text and succeed.
    Help,
    /// Reject the command line, naming what was wrong with it.
    Usage {
        /// What the caller got wrong, as one line.
        problem: String,
    },
}

/// Parses the arguments after the program name.
///
/// Flags may appear in any order and before or after the path. A second path,
/// an unrecognized flag, or no path at all is a usage error rather than a
/// guess: guessing which of two paths was meant is how a check silently
/// validates the wrong file and reports success.
#[must_use]
pub fn parse<I>(arguments: I) -> Invocation
where
    I: IntoIterator<Item = String>,
{
    let mut options = Options::default();
    let mut document: Option<Utf8PathBuf> = None;

    for argument in arguments {
        match argument.as_str() {
            "--help" | "-h" => return Invocation::Help,
            "--json" => options.format = OutputFormat::Json,
            "--strict" => options.strictness = Strictness::Strict,
            "--stats" => options.stats = true,
            other if other.starts_with('-') => {
                return usage(format!("unrecognized option {other:?}"));
            }
            other if document.is_some() => {
                return usage(format!("unexpected second document {other:?}"));
            }
            other => document = Some(Utf8PathBuf::from(other)),
        }
    }

    let Some(path) = document else {
        return usage("no scene document was named".to_owned());
    };
    Invocation::Check { path, options }
}

/// A rejected command line.
const fn usage(problem: String) -> Invocation { Invocation::Usage { problem } }
