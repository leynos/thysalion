//! Size and count measurements over a validated scene.
//!
//! Spike A2's measurement made permanent rather than discarded. The spike
//! established that a busy wilderness fixture encodes to 0.74 MiB compact and
//! 2.33 MiB pretty-printed, which is how the compact-JSON decision was made;
//! keeping the measurement in the tool means the next such decision starts from
//! evidence rather than from another one-off script. It is also where a future
//! size or load-time budget assertion hangs.

use serde::Serialize;

use crate::{
    codec::{CodecError, Encoding, encode_document},
    scene::Scene,
};

/// How many bytes one voxel index occupies in a decoded chunk.
const BYTES_PER_VOXEL: u64 = size_of::<u16>() as u64;

/// What a scene costs, in voxels, runs, and bytes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[non_exhaustive]
pub struct SceneStats {
    /// Palette entries.
    pub palette_entries: usize,
    /// Spawn points.
    pub spawns: usize,
    /// Chunks actually stored. An absent chunk is entirely air.
    pub populated_chunks: usize,
    /// Chunks stored as a single repeated index rather than a run stream.
    pub uniform_chunks: usize,
    /// Runs across every run-encoded chunk.
    pub runs: usize,
    /// Voxels that are not air.
    pub non_air_voxels: u64,
    /// Voxels the declared extent covers, air included.
    pub declared_voxels: u64,
    /// Bytes of the compact JSON encoding.
    pub json_bytes: usize,
    /// Bytes of the MessagePack encoding, which is what ships.
    pub msgpack_bytes: usize,
    /// Bytes the decoded grid occupies while the scene is loaded.
    ///
    /// Counts only the chunks that are stored densely: a uniform chunk holds
    /// one index rather than 32,768 copies of it, which is the elision that
    /// makes a mostly-empty wilderness extent affordable at all.
    pub decoded_bytes: u64,
}

impl SceneStats {
    /// Measures `scene`.
    ///
    /// # Errors
    ///
    /// Returns [`CodecError::Encode`] when either encoding fails. The sizes are
    /// measured by encoding rather than estimated, because an estimate that
    /// drifts from the bytes on disk is worse than no number at all.
    pub fn of(scene: &Scene) -> Result<Self, CodecError> {
        let document = scene.to_document();
        let json = encode_document(&document, Encoding::Json)?;
        let msgpack = encode_document(&document, Encoding::MessagePack)?;

        let payload = ChunkTally::of(scene);
        Ok(Self {
            palette_entries: scene.palette().len(),
            spawns: scene.entities().len(),
            populated_chunks: scene.voxels().populated_chunks(),
            uniform_chunks: payload.uniform,
            runs: payload.runs,
            non_air_voxels: scene.non_air_count(),
            declared_voxels: scene.extent().volume().unwrap_or(u64::MAX),
            json_bytes: json.len(),
            msgpack_bytes: msgpack.len(),
            decoded_bytes: payload.decoded_bytes(scene.chunk_size().volume()),
        })
    }

    /// The measurements as text, one per line.
    #[must_use]
    pub fn to_text(&self) -> String { self.to_string() }

    /// The measurements as structured data.
    ///
    /// # Errors
    ///
    /// Returns the `serde_json` failure, which cannot arise for a struct of
    /// integers but is not hidden behind an `expect`.
    pub fn to_json(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string_pretty(self)
    }
}

impl core::fmt::Display for SceneStats {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        writeln!(f, "palette entries: {}", self.palette_entries)?;
        writeln!(f, "spawns: {}", self.spawns)?;
        writeln!(
            f,
            "populated chunks: {} ({} uniform)",
            self.populated_chunks, self.uniform_chunks
        )?;
        writeln!(f, "runs: {}", self.runs)?;
        writeln!(
            f,
            "voxels: {} non-air of {} declared",
            self.non_air_voxels, self.declared_voxels
        )?;
        writeln!(f, "json bytes: {}", self.json_bytes)?;
        writeln!(f, "msgpack bytes: {}", self.msgpack_bytes)?;
        writeln!(f, "decoded bytes: {}", self.decoded_bytes)
    }
}

/// How the payload divides between uniform and run-encoded chunks.
struct ChunkTally {
    /// Chunks stored as one repeated index.
    uniform: usize,
    /// Chunks stored as a run stream.
    dense: usize,
    /// Runs across every run-encoded chunk.
    runs: usize,
}

impl ChunkTally {
    /// Counts the chunks of a scene by how they encode.
    ///
    /// Measured from the canonical wire form rather than from the grid's
    /// internal storage, so the numbers describe what a reader will find in the
    /// file. A densely stored chunk holding one distinct index is *reported* as
    /// uniform, because that is how it encodes.
    fn of(scene: &Scene) -> Self {
        use crate::scene::document::ChunkPayloadDocument;

        let mut tally = Self {
            uniform: 0,
            dense: 0,
            runs: 0,
        };
        for entry in scene.voxels().to_chunks() {
            match entry.payload {
                ChunkPayloadDocument::Uniform(_) => tally.uniform = tally.uniform.saturating_add(1),
                ChunkPayloadDocument::Runs(runs) => {
                    tally.dense = tally.dense.saturating_add(1);
                    tally.runs = tally.runs.saturating_add(runs.len());
                }
            }
        }
        tally
    }

    /// Bytes the decoded grid occupies, given the chunk volume.
    fn decoded_bytes(&self, chunk_volume: u64) -> u64 {
        let dense = u64::try_from(self.dense).unwrap_or(u64::MAX);
        dense
            .saturating_mul(chunk_volume)
            .saturating_mul(BYTES_PER_VOXEL)
    }
}
