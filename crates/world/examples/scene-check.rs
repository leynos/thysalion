//! Check a scene document and report what is wrong with it.
//!
//! ```text
//! cargo run -p thysalion-world --example scene-check -- \
//!     assets/scenes/keep-interior.scene.json
//! ```
//!
//! Deliberately thin. Everything worth testing — the exit codes, the two output
//! formats, the strictness rule, the measurements — lives in
//! [`thysalion_world::check`], because a binary cannot be called from a test.
//! What is left here is the part that genuinely cannot be: taking ambient
//! authority, writing to a stream, and setting a process exit status.
//!
//! Ambient authority is taken once, in one place, and immediately narrowed to
//! the scene document's own directory. That directory is the source root, so a
//! scene naming `knowledge/town.trig` reaches `<root>/knowledge/town.trig` and
//! cannot reach anything above it.

use std::{
    io::{self, Write as _},
    process,
    sync::Arc,
};

use camino::{Utf8Path, Utf8PathBuf};
use cap_std::{ambient_authority, fs_utf8::Dir};
use thysalion_world::{
    check::{self, ExitCode, Invocation, Options},
    loader::SceneLoader,
    source::DirSceneSource,
};

fn main() -> process::ExitCode {
    let code = dispatch(check::parse(std::env::args().skip(1)));
    // `process::exit` would skip every destructor; returning the status lets the
    // streams flush. The conversion is lossless for the four codes this tool
    // produces, all of which fit a `u8`.
    process::ExitCode::from(u8::try_from(code.get()).unwrap_or(1))
}

/// Runs whatever the command line asked for.
fn dispatch(invocation: Invocation) -> ExitCode {
    match invocation {
        Invocation::Help => {
            emit(check::USAGE, Stream::Out);
            ExitCode::Valid
        }
        Invocation::Usage { problem } => {
            emit(
                &format!("scene-check: {problem}\n\n{}", check::USAGE),
                Stream::Err,
            );
            ExitCode::Usage
        }
        Invocation::Check { path, options } => check_document(&path, options),
        // `Invocation` is `#[non_exhaustive]`, so a variant added later reaches
        // here. Refusing is the safe default: a request this build does not
        // understand must not be treated as a successful check.
        _ => {
            emit("scene-check: unsupported invocation\n", Stream::Err);
            ExitCode::Usage
        }
    }
}

/// Opens the document's directory as a capability and checks the document.
fn check_document(path: &Utf8Path, options: Options) -> ExitCode {
    let (root, document) = split(path);
    let Ok(directory) = Dir::open_ambient_dir(&root, ambient_authority()) else {
        emit(
            &format!("scene-check: cannot open source root {root}\n"),
            Stream::Err,
        );
        return ExitCode::SourceFailure;
    };

    let source = DirSceneSource::new(directory, root.as_str());
    let loader = SceneLoader::new(Arc::new(source));
    let outcome = check::run(&loader, &document, root.as_str(), options);
    emit(&outcome.output, stream_for(outcome.code));
    outcome.code
}

/// Splits a document path into its source root and its file name.
///
/// A bare file name roots at the current directory. Naming it `.` rather than
/// leaving the root empty matters: the root appears in every report, and an
/// empty one tells a reader nothing about where the tool looked.
fn split(path: &Utf8Path) -> (Utf8PathBuf, Utf8PathBuf) {
    let root = path
        .parent()
        .filter(|parent| !parent.as_str().is_empty())
        .map_or_else(|| Utf8PathBuf::from("."), Utf8Path::to_owned);
    let document = path
        .file_name()
        .map_or_else(|| path.to_owned(), Utf8PathBuf::from);
    (root, document)
}

/// Which stream a message belongs on.
#[derive(Clone, Copy)]
enum Stream {
    /// Standard output, for a report the caller asked for.
    Out,
    /// Standard error, for a failure the caller did not.
    Err,
}

/// Where a report goes, given how the check turned out.
///
/// A valid or invalid scene is a *result*, and results go to standard output so
/// a caller can pipe `--json` into a program. A tool or usage failure is not a
/// result and must not contaminate that pipe.
const fn stream_for(code: ExitCode) -> Stream {
    match code {
        ExitCode::Valid | ExitCode::Invalid => Stream::Out,
        // Including any code added later: standard error is the safe default,
        // because a message that is not a result must never contaminate a pipe
        // a caller is reading `--json` from.
        _ => Stream::Err,
    }
}

/// Writes a message, reporting whether it landed.
///
/// `io::Write` rather than `println!`, which the workspace denies. Callers
/// ignore the result deliberately: a failed write means the reader has gone away
/// — the canonical case is `scene-check | head` — and there is nowhere left to
/// report that to, so the only honest response is to carry on quietly and let
/// the exit code speak.
fn emit(message: &str, stream: Stream) -> bool {
    let bytes = message.as_bytes();
    let written = match stream {
        Stream::Out => io::stdout().write_all(bytes),
        Stream::Err => io::stderr().write_all(bytes),
    };
    written.is_ok()
}
