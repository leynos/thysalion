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

    let Ok(chunk_size) = ChunkSize::new(document.chunk_size) else {
        problems.push(SceneDiagnostic::structural(
            DiagnosticCode::ZeroDimension,
            at,
            "chunk size is zero",
        ));
        return Err(problems);
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
        ExtentError::VolumeOverflow => DiagnosticCode::DimensionsOverflow,
        ExtentError::Unaligned { .. } => DiagnosticCode::DimensionsUnaligned,
    };
    SceneDiagnostic::structural(code, at, error.to_string())
}

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
    }
    for (ordinal, entry) in document.voxels.iter().enumerate() {
        let run_count = entry.payload.run_count();
        if run_count > bounds.max_runs_per_chunk {
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
        }
    }
}
