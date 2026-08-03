//! One corrupt fixture per corruption class, and the report each produces.
//!
//! Two assertions per fixture, with a deliberate division of labour. [`CLASSES`]
//! is the *contract*: it says which diagnostic code each fault must produce, and
//! it lives in this file so a reader sees the whole mapping without opening
//! twenty-nine documents. The `insta` snapshots pin the rendered text — wording,
//! ordering, and located position — so changing a message is a reviewed act
//! rather than a silent one.
//!
//! The fixtures are literal documents on disk, read by the real loader through a
//! real capability-scoped directory. Nothing generates them at test time. They
//! must never be routed through the Stage C3 fixture compiler: that compiler is
//! what they exist to validate, and a fault expressed through it would be an
//! artefact of the compiler rather than of a document.
//!
//! [`CLASSES`] doubles as a completeness check. A class added to
//! `DiagnosticCode` with no fixture here is invisible to review, so the last
//! test asserts the two agree.

use std::sync::Arc;

use camino::{Utf8Path, Utf8PathBuf};
use cap_std::{ambient_authority, fs_utf8::Dir};
use thysalion_world::{
    check::{self, ExitCode, Options, Outcome, OutputFormat},
    loader::SceneLoader,
    scene::validation::Strictness,
    source::DirSceneSource,
};

/// Where the corrupt fixtures live, relative to the crate root.
const FIXTURES: &str = "tests/fixtures/corrupt";

// The tables are inline data, not test logic, and this file is over the
// 400-line cap without them. Declared by path rather than through
// `support/mod.rs` for the reason that file gives: each integration test is its
// own crate, so a helper reachable but unused is dead code under `-D warnings`.
#[path = "support/corrupt_classes.rs"]
mod corrupt_classes;

use corrupt_classes::{CLASSES, WARNINGS};

/// Opens the fixture directory as a capability, per AGENTS.md's filesystem
/// policy. Ambient authority is taken once, here.
///
/// # Panics
///
/// Panics when the fixture directory is missing, which is a broken checkout.
fn fixtures() -> Dir {
    let root = Utf8PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(FIXTURES);
    match Dir::open_ambient_dir(&root, ambient_authority()) {
        Ok(directory) => directory,
        Err(error) => panic!("the corrupt fixture directory must exist at {root}: {error}"),
    }
}

/// Checks one fixture by name, in the requested format and strictness.
fn check_fixture(name: &str, options: Options) -> Outcome {
    let document = Utf8PathBuf::from(format!("{name}.scene.json"));
    let source = DirSceneSource::new(fixtures(), FIXTURES);
    let loader = SceneLoader::new(Arc::new(source));
    check::run(&loader, &document, FIXTURES, options)
}

/// The codes a fixture's report carries, read back from the `--json` form.
///
/// Read from the structured output rather than scraped from the text, because
/// that is what a program is supposed to consume — and because the text is
/// simultaneously pinned as a wording contract, so scraping it here would make a
/// message tweak break two things at once.
fn codes_of(name: &str, member: &str) -> Vec<String> {
    let outcome = check_fixture(name, json_options());
    let parsed: serde_json::Value = match serde_json::from_str(&outcome.output) {
        Ok(parsed) => parsed,
        Err(error) => panic!(
            "{name}: the --json form must parse: {error}\n{}",
            outcome.output
        ),
    };
    parsed
        .get(member)
        .and_then(serde_json::Value::as_array)
        .map(|entries| {
            entries
                .iter()
                .filter_map(|entry| entry.get("code"))
                .filter_map(serde_json::Value::as_str)
                .map(ToOwned::to_owned)
                .collect()
        })
        .unwrap_or_default()
}

/// Structured output, lenient, no measurements.
fn json_options() -> Options {
    let mut options = Options::default();
    options.format = OutputFormat::Json;
    options
}

#[test]
fn every_corrupt_fixture_reports_its_own_class() {
    for (name, class) in CLASSES {
        let Some(expected) = *class else {
            continue;
        };
        let codes = codes_of(name, "errors");
        assert!(
            codes.iter().any(|code| code == expected),
            "{name}: expected {expected:?} among the reported codes, got {codes:?}"
        );
    }
}

#[test]
fn every_corrupt_fixture_reports_exactly_one_problem() {
    // "One fault each" is what makes a fixture a *contract about a class*
    // rather than a sample of a broken document. A second diagnostic means
    // either the fixture carries an accidental extra fault or a rule reports a
    // consequence of another rule's finding, and both are worth catching: a
    // report of forty consequences buries the four causes.
    for (name, expected) in CLASSES {
        if expected.is_none() {
            continue;
        }
        let codes = codes_of(name, "errors");
        assert_eq!(
            codes.len(),
            1,
            "{name}: a corrupt fixture must report exactly one problem, got {codes:?}"
        );
    }
}

#[test]
fn every_corrupt_fixture_fails_the_check() {
    // Exit 1 rather than 2 for all of them, including the two that do not
    // parse: the bytes were read perfectly well, and telling a contributor
    // their file is unreadable when one key is misspelled sends them looking in
    // the wrong place entirely.
    for (name, _) in CLASSES {
        let outcome = check_fixture(name, Options::default());
        assert_eq!(
            outcome.code,
            ExitCode::Invalid,
            "{name}: a corrupt fixture must fail as invalid, got {:?}\n{}",
            outcome.code,
            outcome.output
        );
    }
}

#[test]
fn the_warning_fixtures_load_and_name_their_finding() {
    for (name, expected) in WARNINGS {
        let outcome = check_fixture(name, Options::default());
        assert_eq!(
            outcome.code,
            ExitCode::Valid,
            "{name}: a warning must not fail a lenient check\n{}",
            outcome.output
        );
        let codes = codes_of(name, "warnings");
        assert!(
            codes.iter().any(|code| code == expected),
            "{name}: expected {expected:?} among the warnings, got {codes:?}"
        );
    }
}

#[test]
fn the_warning_fixtures_fail_under_strict() {
    let mut options = Options::default();
    options.strictness = Strictness::Strict;
    for (name, _) in WARNINGS {
        let outcome = check_fixture(name, options);
        assert_eq!(
            outcome.code,
            ExitCode::Invalid,
            "{name}: --strict must promote a warning to a failure\n{}",
            outcome.output
        );
    }
}

#[test]
fn the_corrupt_reports_match_their_snapshots() {
    // The snapshots are the wording, ordering, and location contract. Refresh
    // them with `cargo insta review`, deliberately: a changed message is a
    // change to what a contributor reads when a scene will not load.
    let mut settings = insta::Settings::clone_current();
    settings.set_snapshot_path("snapshots/corrupt");
    settings.set_prepend_module_to_snapshot(false);
    let _guard = settings.bind_to_scope();

    for (name, _) in CLASSES.iter().chain(warning_names().iter()) {
        let outcome = check_fixture(name, Options::default());
        insta::assert_snapshot!(*name, outcome.output);
    }
}

/// The warning fixtures in the same shape as [`CLASSES`], for the snapshot loop.
fn warning_names() -> Vec<(&'static str, Option<&'static str>)> {
    WARNINGS
        .iter()
        .map(|(name, code)| (*name, Some(*code)))
        .collect()
}

#[test]
fn every_diagnostic_class_has_a_fixture() {
    // The completeness check. A code added to `DiagnosticCode` with no fixture
    // here would be a corruption class nobody ever sees reported, which is how
    // a diagnostic ships with the wrong wording and nobody notices for a year.
    let covered: Vec<&str> = CLASSES
        .iter()
        .filter_map(|(_, code)| *code)
        .chain(WARNINGS.iter().map(|(_, code)| *code))
        .collect();
    let missing: Vec<&str> = uncovered_codes()
        .into_iter()
        .filter(|code| !covered.contains(code))
        .collect();
    assert!(
        missing.is_empty(),
        "these diagnostic codes have no corrupt fixture: {missing:?}"
    );
}

/// Every code this build can produce, less the four no checked-in file can
/// express.
///
/// Three are resource bounds: `scene.palette.too-large` needs 65,537 palette
/// entries, `scene.voxels.too-many-chunks` needs 16.8 million chunk entries, and
/// `scene.voxels.too-many-runs` needs 65,537 runs in one chunk. All three are
/// resource attacks rather than authoring mistakes, they run to megabytes as
/// files, and what matters about them is that the loader terminates within a
/// bound rather than what the message says. They are constructed in memory as
/// hostile inputs instead.
///
/// The fourth, `scene.knowledge.resource-unreadable`, needs a source that fails
/// on read rather than a file that is missing. Git cannot portably check in a
/// file that exists and refuses to be read, so
/// [`an_unreadable_resource_is_distinguished_from_an_absent_one`] covers it with
/// a stub source — which is the distinction the port exists to preserve.
fn uncovered_codes() -> Vec<&'static str> {
    use thysalion_world::scene::validation::DiagnosticCode as Code;

    const EXCLUDED: &[&str] = &[
        "scene.palette.too-large",
        "scene.voxels.too-many-chunks",
        "scene.voxels.too-many-runs",
        "scene.knowledge.resource-unreadable",
    ];
    const ALL: &[Code] = &[
        Code::UnsupportedVersion,
        Code::ZeroDimension,
        Code::DimensionsOverflow,
        Code::DimensionsUnaligned,
        Code::ChunkSizeNotDesign,
        Code::PaletteEmpty,
        Code::PaletteTooLarge,
        Code::PaletteZeroNotAir,
        Code::DuplicateVoxelTypeName,
        Code::EmissionOutOfRange,
        Code::ConceptIriInvalid,
        Code::TooManyChunks,
        Code::ChunkOutwithExtent,
        Code::DuplicateChunk,
        Code::ChunksOutOfOrder,
        Code::TooManyRuns,
        Code::UnknownPaletteIndex,
        Code::ZeroLengthRun,
        Code::AdjacentDuplicateRuns,
        Code::RunLengthMismatch,
        Code::SpawnOutwithGrid,
        Code::DuplicateSpawnName,
        Code::UnknownPrototype,
        Code::PrototypeCycle,
        Code::PrototypeTooDeep,
        Code::SceneGraphIriInvalid,
        Code::KnowledgeResourceAbsent,
        Code::KnowledgeResourceUnreadable,
        Code::KnowledgeResourceUnsafePath,
        Code::SpawnObstructed,
        Code::SpawnUnsupported,
    ];

    ALL.iter()
        .map(|code| code.as_str())
        .filter(|code| !EXCLUDED.contains(code))
        .collect()
}

#[test]
fn the_fixture_directory_holds_nothing_the_contract_omits() {
    // The other direction: a fixture on disk that no test names is dead weight
    // a reader will trust as covered.
    let named: Vec<String> = CLASSES
        .iter()
        .map(|(name, _)| (*name).to_owned())
        .chain(WARNINGS.iter().map(|(name, _)| (*name).to_owned()))
        .collect();
    for entry in fixture_names() {
        assert!(
            named.contains(&entry),
            "{entry:?} is in {FIXTURES} but no test names it"
        );
    }
}

/// The fixture stems on disk, without their `.scene.json` suffix.
///
/// # Panics
///
/// Panics when the directory cannot be read, which is a broken checkout.
fn fixture_names() -> Vec<String> {
    let Ok(entries) = fixtures().entries() else {
        panic!("the corrupt fixture directory must be readable");
    };
    entries
        .filter_map(Result::ok)
        .filter_map(|entry| entry.file_name().ok())
        .filter_map(|name| {
            Utf8Path::new(&name)
                .file_name()
                .and_then(|file| file.strip_suffix(".scene.json"))
                .map(ToOwned::to_owned)
        })
        .collect()
}

/// A source whose every read fails as unreadable rather than as absent.
///
/// The one corruption class that needs infrastructure to misbehave rather than a
/// document to be wrong.
struct UnreadableSource;

impl thysalion_world::source::SceneSource for UnreadableSource {
    fn read(&self, path: &Utf8Path) -> Result<Vec<u8>, thysalion_world::source::SceneSourceError> {
        Err(thysalion_world::source::SceneSourceError::Unreadable {
            path: path.to_owned(),
            message: smol_str::SmolStr::new("permission denied"),
        })
    }
}

#[test]
fn an_unreadable_resource_is_distinguished_from_an_absent_one() {
    // The whole reason the port has no `exists` method. A boolean would collapse
    // these two, and an author told their scene "names a file that is not there"
    // about a file sitting right in front of them has been actively misled.
    let loader = SceneLoader::new(Arc::new(UnreadableSource));
    let bytes = match fixtures().read("spawn-obstructed.scene.json") {
        Ok(bytes) => bytes,
        Err(error) => panic!("the fixture must be readable: {error}"),
    };
    let outcome = loader
        .load_bytes(&bytes, thysalion_world::codec::Encoding::Json)
        .err();
    let Some(failure) = outcome else {
        panic!("a source that cannot supply the knowledge resource must fail the load");
    };
    let codes: Vec<&str> = failure
        .diagnostics()
        .iter()
        .map(|diagnostic| diagnostic.code().as_str())
        .collect();
    assert!(
        codes.contains(&"scene.knowledge.resource-unreadable"),
        "expected an unreadable-resource diagnostic, got {codes:?}"
    );
}
