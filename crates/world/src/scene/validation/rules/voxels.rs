//! Phases two and three for the voxel payload: bounded decode, then the
//! semantic checks over what was decoded.
//!
//! The checks that must walk a chunk's runs share one traversal, but as an
//! iterator feeding single-purpose accumulators rather than as one function
//! doing four jobs — `clippy.toml` caps cognitive complexity at nine, and a
//! function checking volume, canonicality, zero lengths, and palette bounds at
//! once breaches it by construction.
//!
//! The per-chunk state every check needs — the palette to resolve against, the
//! chunk being walked, the geometry to size it, and the document location to
//! blame — travels as one private `ChunkContext` rather than as four parameters
//! threaded through six signatures. The workspace caps functions at four
//! arguments, and a run of five-argument helpers differing only in their first
//! parameter is exactly the shape that cap exists to catch.

use std::collections::BTreeSet;

use smol_str::SmolStr;

use crate::{
    grid::{
        ChunkCoord,
        ChunkInsertError,
        VoxelGrid,
        VoxelIndex,
        VoxelPos,
        runs::{RunDecodeError, expand},
    },
    scene::{
        document::{ChunkEntryDocument, ChunkPayloadDocument},
        palette::Palette,
        validation::{
            diagnostics::{
                DiagnosticCode,
                DocumentLocation,
                DocumentSection,
                SceneDiagnostic,
                VoxelSite,
            },
            rules::header::Geometry,
        },
    },
};

/// Decodes the payload into a grid, checking every voxel rule as it goes.
///
/// # Errors
///
/// Returns every problem found across all chunks, in document order.
pub fn decode(
    entries: &[ChunkEntryDocument],
    geometry: Geometry,
    palette: &Palette,
) -> Result<VoxelGrid, Vec<SceneDiagnostic>> {
    let mut problems = Vec::new();
    let mut grid = VoxelGrid::empty(geometry.extent, geometry.chunk_size);
    let mut seen: BTreeSet<ChunkCoord> = BTreeSet::new();
    let mut previous: Option<ChunkCoord> = None;

    for (ordinal, entry) in entries.iter().enumerate() {
        let context = ChunkContext {
            palette,
            geometry,
            coord: ChunkCoord::new(entry.at.x, entry.at.y, entry.at.z),
            at: location(ordinal),
        };

        context.check_placement(&seen, previous, &mut problems);
        seen.insert(context.coord);
        previous = Some(context.coord);

        match context.decode_one(&entry.payload) {
            Ok(chunk) => context.install(&mut grid, chunk, &mut problems),
            Err(found) => problems.extend(found),
        }
    }

    if problems.is_empty() {
        Ok(grid)
    } else {
        Err(problems)
    }
}

/// The document location of chunk entry `ordinal`.
fn location(ordinal: usize) -> DocumentLocation {
    DocumentLocation::new(
        DocumentSection::Voxels,
        u32::try_from(ordinal).unwrap_or(u32::MAX),
    )
}

/// One decoded chunk, ready to install.
enum Decoded {
    /// Every voxel is this index.
    Uniform(VoxelIndex),
    /// A dense chunk-local array.
    Dense(Vec<VoxelIndex>),
}

/// Everything the checks over one chunk entry share.
struct ChunkContext<'a> {
    /// The palette every index in this chunk must resolve against.
    palette: &'a Palette,
    /// The scene geometry phase one already checked.
    geometry: Geometry,
    /// Which chunk this entry places.
    coord: ChunkCoord,
    /// Where in the document to blame.
    at: DocumentLocation,
}

impl ChunkContext<'_> {
    /// Checks this chunk's coordinate for range, duplication, and sort order.
    fn check_placement(
        &self,
        seen: &BTreeSet<ChunkCoord>,
        previous: Option<ChunkCoord>,
        problems: &mut Vec<SceneDiagnostic>,
    ) {
        let (cx, cy, cz) = self.geometry.extent.in_chunks(self.geometry.chunk_size);
        if self.is_outwith((cx, cy, cz)) {
            problems.push(self.structural(
                DiagnosticCode::ChunkOutwithExtent,
                format!(
                    "chunk ({}, {}, {}) lies outwith the {cx} x {cy} x {cz} chunk grid",
                    self.coord.x, self.coord.y, self.coord.z
                ),
            ));
        }
        if seen.contains(&self.coord) {
            problems.push(self.structural(
                DiagnosticCode::DuplicateChunk,
                format!(
                    "chunk ({}, {}, {}) is declared more than once",
                    self.coord.x, self.coord.y, self.coord.z
                ),
            ));
        }
        // Strictly greater, not `>=`. Equality is precisely the duplicate case,
        // already reported above, and saying "and also not in ascending order"
        // about it adds a second diagnostic for one authoring mistake — which
        // is how a report of forty consequences buries the four causes.
        if previous.is_some_and(|last| last > self.coord) {
            problems.push(self.structural(
                DiagnosticCode::ChunksOutOfOrder,
                "chunk entries must be sorted by coordinate, or the encoding is not canonical and \
                 the content hash is unstable",
            ));
        }
    }

    /// Whether this chunk lies outwith a chunk grid of the given size.
    ///
    /// Written as an iteration over the three axes rather than three
    /// disjunctions: the axes are interchangeable, and pairing each coordinate
    /// with its own limit makes the mismatched-axis mistake unwritable.
    fn is_outwith(&self, limits: (u32, u32, u32)) -> bool {
        let (cx, cy, cz) = limits;
        [(self.coord.x, cx), (self.coord.y, cy), (self.coord.z, cz)]
            .into_iter()
            .any(|(coordinate, limit)| coordinate >= limit)
    }

    /// Decodes this chunk's payload and checks its palette references.
    fn decode_one(&self, payload: &ChunkPayloadDocument) -> Result<Decoded, Vec<SceneDiagnostic>> {
        match payload {
            ChunkPayloadDocument::Uniform(raw) => {
                let index = VoxelIndex::new(*raw);
                if self.palette.get(index).is_none() {
                    return Err(vec![self.unresolved(index, VoxelPos::new(0, 0, 0))]);
                }
                Ok(Decoded::Uniform(index))
            }
            ChunkPayloadDocument::Runs(runs) => {
                let voxels = expand(runs, self.geometry.chunk_size.volume())
                    .map_err(|error| vec![self.run_diagnostic(&error)])?;
                self.check_indices(&voxels)?;
                Ok(Decoded::Dense(voxels))
            }
        }
    }

    /// Checks every decoded voxel resolves in the palette.
    ///
    /// Reports the first unresolved voxel in the chunk and stops. A single bad
    /// run expands to as many identical faults as it is long — up to 32,768 —
    /// and a report listing every one buries every other diagnostic in the
    /// document. The one that is reported carries its exact position, which is
    /// what an author needs to find the run that produced it.
    fn check_indices(&self, voxels: &[VoxelIndex]) -> Result<(), Vec<SceneDiagnostic>> {
        let first = voxels
            .iter()
            .enumerate()
            .find(|(_, index)| self.palette.get(**index).is_none());
        let Some((offset, index)) = first else {
            return Ok(());
        };
        let local = VoxelPos::from_local_index(offset as u64, self.geometry.chunk_size)
            .unwrap_or(VoxelPos::new(0, 0, 0));
        Err(vec![self.unresolved(*index, local)])
    }

    /// Installs a decoded chunk, reporting a length the grid refuses.
    fn install(&self, grid: &mut VoxelGrid, chunk: Decoded, problems: &mut Vec<SceneDiagnostic>) {
        let outcome = match chunk {
            Decoded::Uniform(index) => grid.insert_uniform(self.coord, index),
            Decoded::Dense(voxels) => grid.insert_dense(self.coord, voxels),
        };
        let Err(refusal) = outcome else {
            return;
        };
        match refusal {
            ChunkInsertError::WrongLength { expected, found } => {
                problems.push(self.structural(
                    DiagnosticCode::RunLengthMismatch,
                    format!("decoded {found} voxels, but a chunk holds {expected}"),
                ));
            }
            // Deliberately unreported. `check_placement` has already refused
            // this chunk and owns the class; reporting the grid's refusal too
            // gives one authoring mistake two diagnostics, which is exactly
            // what the exactly-one-problem assertion over the corrupt fixtures
            // exists to catch — and did catch, when this arm first reported.
            // The grid still refuses the chunk, so the invalid state cannot
            // reach `to_chunks` whether or not a rule speaks up.
            ChunkInsertError::OutwithExtent { .. } => {}
        }
    }

    /// Renders a run-codec failure as its matching diagnostic class.
    ///
    /// Positioned at the chunk origin: a stream that failed to expand has no
    /// meaningful position past the point it stopped, and the chunk coordinate
    /// is the part an author can act on.
    fn run_diagnostic(&self, error: &RunDecodeError) -> SceneDiagnostic {
        let code = match error {
            RunDecodeError::ZeroLength { .. } => DiagnosticCode::ZeroLengthRun,
            RunDecodeError::AdjacentDuplicate { .. } => DiagnosticCode::AdjacentDuplicateRuns,
            RunDecodeError::LengthMismatch { .. } => DiagnosticCode::RunLengthMismatch,
        };
        self.positioned(code, VoxelPos::new(0, 0, 0), error.to_string())
    }

    /// The unknown-palette-index diagnostic, located in the world.
    fn unresolved(&self, index: VoxelIndex, local: VoxelPos) -> SceneDiagnostic {
        self.positioned(
            DiagnosticCode::UnknownPaletteIndex,
            local,
            format!(
                "palette index {} does not resolve; the palette has {} entries",
                index.get(),
                self.palette.len()
            ),
        )
    }

    /// A structural diagnostic at this chunk entry.
    fn structural(&self, code: DiagnosticCode, detail: impl Into<SmolStr>) -> SceneDiagnostic {
        SceneDiagnostic::structural(code, self.at, detail)
    }

    /// A diagnostic at a chunk-local position within this chunk.
    fn positioned(
        &self,
        code: DiagnosticCode,
        local: VoxelPos,
        detail: impl Into<SmolStr>,
    ) -> SceneDiagnostic {
        SceneDiagnostic::positioned(
            code,
            self.at,
            VoxelSite {
                chunk: self.coord,
                local,
            },
            detail,
        )
    }
}
