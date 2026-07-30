//! Rendering a diagnostic list for humans and for machines.
//!
//! This crate's own renderer rather than a library's, deliberately. The text
//! form is pinned by `insta` snapshots, so it is a contract about wording,
//! ordering, and location; an upstream release that improved its formatting
//! would churn every one of those snapshots for no gain.
//!
//! The two forms have different audiences and different stability promises.
//! The text is for a contributor reading a terminal, and the snapshots exist to
//! make a change to it deliberate. The JSON is for a program — the fixture
//! generator's test suite consumes it — and its field names are the contract.
//! Having a program parse the human text instead is how a wording tweak
//! silently breaks a downstream suite.
//!
//! Every report names the resolved source root. Running from the wrong working
//! directory otherwise produces "knowledge resource absent", which is the same
//! diagnostic a genuinely broken scene produces, and a reader with no root to
//! check spends the afternoon editing a correct document.

mod stats;

use camino::{Utf8Path, Utf8PathBuf};
use serde::Serialize;
use smol_str::SmolStr;
pub use stats::SceneStats;

use crate::scene::validation::diagnostics::{SceneDiagnostic, VoxelSite};

/// The outcome of checking one document, ready to render.
#[derive(Debug, Clone)]
#[non_exhaustive]
pub struct Report {
    /// The document that was checked.
    pub document: Utf8PathBuf,
    /// The resolved source root the document's resources were sought under.
    pub root: SmolStr,
    /// Fatal problems, in document order. Empty when the scene loaded.
    pub errors: Vec<SceneDiagnostic>,
    /// Advisory findings, in document order.
    pub warnings: Vec<SceneDiagnostic>,
    /// Size and count measurements, when they were asked for.
    ///
    /// Part of the report rather than something a caller concatenates
    /// afterwards. Two JSON documents in one stream is not JSON, and splicing
    /// one object into another's rendered text is a parser nobody wants to own.
    pub stats: Option<SceneStats>,
}

impl Report {
    /// A report over the given document and source root.
    #[must_use]
    pub fn new(document: &Utf8Path, root: impl Into<SmolStr>) -> Self {
        Self {
            document: document.to_owned(),
            root: root.into(),
            errors: Vec::new(),
            warnings: Vec::new(),
            stats: None,
        }
    }

    /// Adds the fatal problems from a failed load.
    #[must_use]
    pub fn with_errors(mut self, errors: &[SceneDiagnostic]) -> Self {
        self.errors = errors.to_vec();
        self
    }

    /// Adds the advisory findings from a successful load.
    #[must_use]
    pub fn with_warnings(mut self, warnings: &[SceneDiagnostic]) -> Self {
        self.warnings = warnings.to_vec();
        self
    }

    /// Adds the size and count measurements.
    #[must_use]
    pub const fn with_stats(mut self, stats: SceneStats) -> Self {
        self.stats = Some(stats);
        self
    }

    /// Whether the document is acceptable, given how warnings are treated.
    ///
    /// Under [`Strictness::Strict`] a warning is a failure. Continuous
    /// integration runs strict, so a scene that loads with a spawn inside a
    /// wall does not reach the repository unnoticed; a contributor iterating
    /// locally is not blocked by one.
    #[must_use]
    pub const fn is_acceptable(&self, strictness: Strictness) -> bool {
        match strictness {
            Strictness::Lenient => self.errors.is_empty(),
            Strictness::Strict => self.errors.is_empty() && self.warnings.is_empty(),
        }
    }

    /// The report as text for a human reader.
    #[must_use]
    pub fn to_text(&self) -> String { self.to_string() }

    /// The report as structured data for a program.
    ///
    /// # Errors
    ///
    /// Returns the `serde_json` failure. It cannot arise for this type — every
    /// field is a string, an integer, or a `Vec` of them — but the encoder's
    /// signature is fallible and this crate does not use `expect`.
    pub fn to_json(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string_pretty(&ReportJson::from(self))
    }
}

impl core::fmt::Display for Report {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        writeln!(f, "document: {}", self.document)?;
        writeln!(f, "source root: {}", self.root)?;
        writeln!(
            f,
            "{} error(s), {} warning(s)",
            self.errors.len(),
            self.warnings.len()
        )?;
        for diagnostic in &self.errors {
            write_entry(f, "error", diagnostic)?;
        }
        for diagnostic in &self.warnings {
            write_entry(f, "warning", diagnostic)?;
        }
        if let Some(stats) = self.stats.as_ref() {
            writeln!(f)?;
            write!(f, "{stats}")?;
        }
        Ok(())
    }
}

/// Whether warnings are treated as failures.
///
/// A named enum rather than a `bool` parameter: `check(&report, true)` at a
/// call site says nothing about what is true.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Strictness {
    /// Warnings are reported and the document is still accepted.
    Lenient,
    /// Warnings fail the check.
    Strict,
}

/// Writes one diagnostic as three lines: severity and code, place, specifics.
///
/// Three lines rather than one long one because a diagnostic carries three
/// independent things — what class of fault, where in the document, and where
/// in the world — and a reader scanning for the second should not have to parse
/// past the first.
fn write_entry(
    f: &mut core::fmt::Formatter<'_>,
    severity: &str,
    diagnostic: &SceneDiagnostic,
) -> core::fmt::Result {
    writeln!(f)?;
    writeln!(
        f,
        "{severity} {} at {}",
        diagnostic.code(),
        diagnostic.location()
    )?;
    if let Some(site) = diagnostic.voxel_position() {
        writeln!(f, "  {site}")?;
    }
    if let Some(path) = diagnostic.resource_path() {
        writeln!(f, "  resource: {path}")?;
    }
    writeln!(f, "  {}", diagnostic.detail())
}

/// The JSON shape of a report. Field names are the contract.
///
/// A separate type from [`Report`] rather than `#[derive(Serialize)]` on the
/// diagnostics themselves. The diagnostics are domain values, and giving them
/// `serde` derives would make the wire shape an accident of their field layout;
/// here it is written down, and renaming a Rust field cannot silently change
/// it.
#[derive(Debug, Serialize)]
struct ReportJson<'a> {
    /// The document that was checked.
    document: &'a str,
    /// The resolved source root.
    source_root: &'a str,
    /// Whether the document produced no fatal problems.
    ok: bool,
    /// Fatal problems, in document order.
    errors: Vec<DiagnosticJson<'a>>,
    /// Advisory findings, in document order.
    warnings: Vec<DiagnosticJson<'a>>,
    /// Size and count measurements, or `null` when they were not asked for.
    ///
    /// Always present, never omitted. A consumer that must ask whether a key
    /// exists before reading it is one refactor away from reading `undefined`
    /// as zero.
    stats: Option<&'a SceneStats>,
}

impl<'a> From<&'a Report> for ReportJson<'a> {
    fn from(report: &'a Report) -> Self {
        Self {
            document: report.document.as_str(),
            source_root: &report.root,
            ok: report.errors.is_empty(),
            errors: report.errors.iter().map(DiagnosticJson::from).collect(),
            warnings: report.warnings.iter().map(DiagnosticJson::from).collect(),
            stats: report.stats.as_ref(),
        }
    }
}

/// The JSON shape of one diagnostic.
#[derive(Debug, Serialize)]
struct DiagnosticJson<'a> {
    /// The stable dotted corruption-class code.
    code: &'static str,
    /// The document section the fault is in.
    section: &'static str,
    /// Position within that section.
    index: u32,
    /// Where in the world, when the fault has a place.
    site: Option<SiteJson>,
    /// The resource path, when the fault names one.
    resource: Option<&'a str>,
    /// The human-readable specifics.
    detail: &'a str,
}

impl<'a> From<&'a SceneDiagnostic> for DiagnosticJson<'a> {
    fn from(diagnostic: &'a SceneDiagnostic) -> Self {
        let at = diagnostic.location();
        Self {
            code: diagnostic.code().as_str(),
            section: at.section.as_str(),
            index: at.index,
            site: diagnostic.voxel_position().map(SiteJson::from),
            resource: diagnostic.resource_path().map(Utf8Path::as_str),
            detail: diagnostic.detail(),
        }
    }
}

/// The JSON shape of a position in the world.
#[derive(Debug, Serialize)]
struct SiteJson {
    /// Chunk coordinate, as `x`, `y`, `z`.
    chunk: [u32; 3],
    /// Chunk-local voxel position, as `x`, `y`, `z`.
    local: [u32; 3],
}

impl From<VoxelSite> for SiteJson {
    fn from(site: VoxelSite) -> Self {
        Self {
            chunk: [site.chunk.x, site.chunk.y, site.chunk.z],
            local: [site.local.x, site.local.y, site.local.z],
        }
    }
}
