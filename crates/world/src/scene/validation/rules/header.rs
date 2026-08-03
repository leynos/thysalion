//! Phase one: cheap header invariants.
//!
//! Nothing here allocates in proportion to a declared quantity. That is the
//! whole point of the phase: a document claiming a four-billion-entry palette
//! or a 2^32-voxel extent is refused before anything sizes a buffer from it.

use crate::{
    grid::{ChunkSize, DESIGN_CHUNK_SIZE, Extent, ExtentError},
    scene::{
        document::{SUPPORTED_VERSION, SceneDocument},
        validation::{
            bounds::Bounds,
            diagnostics::{DiagnosticCode, DocumentLocation, DocumentSection, SceneDiagnostic},
        },
    },
};

/// What phase one produces when it passes: the checked geometry later phases
/// size their work from.
#[derive(Debug, Clone, Copy)]
pub struct Geometry {
    /// The declared bounds, non-zero and chunk-aligned.
    pub extent: Extent,
    /// The chunk edge length.
    pub chunk_size: ChunkSize,
}

/// Checks dimensions, chunk size, and the declared collection sizes.
///
/// # Errors
///
/// Returns every header problem found. Geometry is returned only when the
/// document is sound enough for phase two to size a decode from it.
pub fn check(document: &SceneDocument, bounds: &Bounds) -> Result<Geometry, Vec<SceneDiagnostic>> {
    let mut problems = Vec::new();
    if let Err(unsupported) = check_version(document) {
        return Err(vec![unsupported]);
    }

    let at = DocumentLocation::section(DocumentSection::Dimensions);

    if document.chunk_size != DESIGN_CHUNK_SIZE {
        problems.push(SceneDiagnostic::structural(
            DiagnosticCode::ChunkSizeNotDesign,
            at,
            format!(
                "chunk size {} is not the {DESIGN_CHUNK_SIZE} design section 7.1 fixes",
                document.chunk_size
            ),
        ));
    }

    let chunk_size = match ChunkSize::new(document.chunk_size) {
        Ok(chunk_size) => chunk_size,
        Err(error) => {
            // Rendered through the shared mapping rather than assumed to be
            // the zero case: an edge whose cube overflows fails here too, and
            // reporting that as "chunk size is zero" would be a lie.
            problems.push(extent_diagnostic(&error, at));
            return Err(problems);
        }
    };

    let extent = match Extent::new(
        document.dimensions.x,
        document.dimensions.y,
        document.dimensions.z,
        chunk_size,
    ) {
        Ok(extent) => extent,
        Err(error) => {
            problems.push(extent_diagnostic(&error, at));
            return Err(problems);
        }
    };

    if let Err(error) = extent.volume() {
        problems.push(extent_diagnostic(&error, at));
        return Err(problems);
    }

    check_collection_sizes(document, bounds, &mut problems);

    if problems.is_empty() {
        Ok(Geometry { extent, chunk_size })
    } else {
        Err(problems)
    }
}

/// Refuses a document version this build cannot read.
///
/// Checked here as well as by the codec's version probe, because a caller may
/// hand [`check`] a document it built in memory rather than decoded. The probe
/// exists to produce a *better* message when decoding — an unsupported version
/// rather than a confusing complaint about an unknown field — and is not the
/// gate.
///
/// # Errors
///
/// Returns the unsupported-version diagnostic. Nothing accumulates with it: a
/// document from a future build may legitimately break every other rule here,
/// and reporting those would send the reader chasing consequences.
fn check_version(document: &SceneDocument) -> Result<(), SceneDiagnostic> {
    if SUPPORTED_VERSION.accepts(document.version) {
        return Ok(());
    }
    Err(SceneDiagnostic::structural(
        DiagnosticCode::UnsupportedVersion,
        DocumentLocation::section(DocumentSection::Version),
        format!(
            "document version {} is not readable by this build, which supports {SUPPORTED_VERSION}",
            document.version
        ),
    ))
}

/// Renders an extent failure as its matching diagnostic class.
fn extent_diagnostic(error: &ExtentError, at: DocumentLocation) -> SceneDiagnostic {
    let code = match error {
        ExtentError::ZeroAxis { .. } | ExtentError::ZeroChunkSize => DiagnosticCode::ZeroDimension,
        ExtentError::VolumeOverflow | ExtentError::ChunkVolumeOverflow { .. } => {
            DiagnosticCode::DimensionsOverflow
        }
        ExtentError::Unaligned { .. } => DiagnosticCode::DimensionsUnaligned,
    };
    SceneDiagnostic::structural(code, at, error.to_string())
}

/// How many per-chunk run faults are listed before the rest are counted.
///
/// One diagnostic per offending chunk means a document declaring a hundred
/// thousand over-limit chunks allocates a hundred thousand formatted strings,
/// which `SceneLoadError::Invalid` then carries and `scene-check` renders —
/// allocation in proportion to a declared quantity, which this phase exists to
/// refuse. Eight is enough to show whether the fault is one chunk or the whole
/// payload. The remainder are counted rather than dropped: a silent cap reads
/// as "that was all of them".
const REPORTED_RUN_FAULTS: usize = 8;

/// Refuses declared collection sizes above the runtime bounds.
fn check_collection_sizes(
    document: &SceneDocument,
    bounds: &Bounds,
    problems: &mut Vec<SceneDiagnostic>,
) {
    if document.voxels.len() > bounds.max_chunks {
        problems.push(SceneDiagnostic::structural(
            DiagnosticCode::TooManyChunks,
            DocumentLocation::section(DocumentSection::Voxels),
            format!(
                "{} chunk entries exceeds the bound of {}",
                document.voxels.len(),
                bounds.max_chunks
            ),
        ));
        // Refused already. Describing faults inside a chunk list that has just
        // been rejected for its length is work proportional to the very
        // quantity the bound exists to refuse, and it tells the reader nothing
        // they can act on before shrinking the list.
        return;
    }

    // Past the bound above, the list is no longer than `max_chunks`, so this
    // walk is bounded by policy rather than by the document.
    let mut reported = 0_usize;
    let mut unreported = 0_usize;
    for (ordinal, entry) in document.voxels.iter().enumerate() {
        let run_count = entry.payload.run_count();
        if run_count <= bounds.max_runs_per_chunk {
            continue;
        }
        if reported == REPORTED_RUN_FAULTS {
            unreported = unreported.saturating_add(1);
            continue;
        }
        problems.push(SceneDiagnostic::structural(
            DiagnosticCode::TooManyRuns,
            DocumentLocation::new(
                DocumentSection::Voxels,
                u32::try_from(ordinal).unwrap_or(u32::MAX),
            ),
            format!(
                "{run_count} runs exceeds the per-chunk bound of {}",
                bounds.max_runs_per_chunk
            ),
        ));
        reported = reported.saturating_add(1);
    }
    if unreported > 0 {
        problems.push(SceneDiagnostic::structural(
            DiagnosticCode::TooManyRuns,
            DocumentLocation::section(DocumentSection::Voxels),
            format!(
                "{unreported} further chunk entries exceed the per-chunk bound of {}",
                bounds.max_runs_per_chunk
            ),
        ));
    }
}
