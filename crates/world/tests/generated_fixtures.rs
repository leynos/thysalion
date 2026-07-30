//! The committed fixture scenes, and the agreement between the two
//! implementations of one schema.
//!
//! There are two readers of the scene format now: this crate, and
//! `scripts/build_fixture_scenes.py`. Two implementations with no sync
//! mechanism drift — one grows a field, the other keeps writing the old shape,
//! and nothing notices until a scene silently loses its lighting. The mechanism
//! is `deny_unknown_fields` plus this file: Rust decodes what Python wrote and
//! re-encodes it, and the re-encoding must be byte-identical to what a Rust
//! author would have produced. A field Python omits fails to deserialize; a
//! field Python invents is rejected; a value Python encodes differently changes
//! the bytes.
//!
//! Deliberately *not* a check that the generator is current — that is
//! `make scenes-check`, which lives outwith `cargo test` so a contributor with
//! no Python toolchain can still run the Rust suite. This file reads the
//! committed artefacts, which are tracked files.

use std::sync::Arc;

use camino::Utf8PathBuf;
use serde::Deserialize;
use thysalion_world::{
    codec::{Encoding, decode_document, encode_document},
    loader::SceneLoader,
    scene::{Scene, validation::Strictness},
    source::DirSceneSource,
};

#[path = "support/scenes.rs"]
mod scenes_support;

use scenes_support::{FIXTURE_NAMES as FIXTURES, SCENES, scene_dir as scenes};

/// Loads a fixture through the real loader and the real filesystem adapter.
///
/// # Panics
///
/// Panics with the diagnostic report when the fixture does not load, because a
/// committed fixture that fails validation is the single most useful failure
/// this suite can produce and `assert!(result.is_ok())` is not it.
fn load(name: &str) -> Scene {
    let loader = SceneLoader::new(Arc::new(DirSceneSource::new(scenes(), SCENES)));
    let path = Utf8PathBuf::from(format!("{name}.scene.json"));
    match loader.load(&path) {
        Ok(found) => found.scene,
        Err(error) => panic!("{name} must load: {error}\n{:#?}", error.diagnostics()),
    }
}

/// Reads a fixture's bytes.
///
/// # Panics
///
/// Panics when the file is missing, which is a broken checkout or a fixture
/// nobody ran `make scenes` for.
fn bytes(name: &str) -> Vec<u8> {
    match scenes().read(format!("{name}.scene.json")) {
        Ok(found) => found,
        Err(error) => panic!("{name}.scene.json must exist; run `make scenes`: {error}"),
    }
}

#[test]
fn every_fixture_loads_clean() {
    for name in FIXTURES {
        let scene = load(name);
        assert!(
            scene.non_air_count() > 0,
            "{name}: a fixture with no content proves nothing"
        );
    }
}

#[test]
fn every_fixture_is_strict_clean() {
    // Continuous integration runs strict, so a fixture with a spawn inside a
    // wall must not reach the repository. These are the scenes every later
    // phase renders, lights, simulates, and tests against; a warning in one is
    // a warning every phase inherits.
    let loader = SceneLoader::new(Arc::new(DirSceneSource::new(scenes(), SCENES)));
    for name in FIXTURES {
        let path = Utf8PathBuf::from(format!("{name}.scene.json"));
        let Ok(checked) = loader.load(&path) else {
            panic!("{name} must load");
        };
        let report = thysalion_world::scene::validation::Report::new(&path, SCENES)
            .with_warnings(&checked.warnings);
        assert!(
            report.is_acceptable(Strictness::Strict),
            "{name}: a committed fixture must carry no warnings\n{}",
            report.to_text()
        );
    }
}

#[test]
fn rust_re_encodes_the_python_output_byte_identically() {
    // The cross-language agreement guard. Decoding proves Python wrote every
    // field this build requires and none it does not know; re-encoding to JSON
    // and comparing proves the two agree on every *value*, not merely on the
    // shape. A difference here is one implementation having drifted.
    for name in FIXTURES {
        let original = bytes(name);
        let Ok(document) = decode_document(&original, Encoding::Json) else {
            panic!("{name}: the Python generator's output must decode");
        };
        let Ok(re_encoded) = encode_document(&document, Encoding::Json) else {
            panic!("{name}: a decoded document must re-encode");
        };
        assert_eq!(
            String::from_utf8_lossy(&re_encoded).trim_end(),
            String::from_utf8_lossy(&original).trim_end(),
            "{name}: the two implementations of the schema have drifted"
        );
    }
}

#[test]
fn a_fixture_hashes_the_same_whichever_encoding_it_came_from() {
    // Design §12.3 hashes the canonical MessagePack form independent of the
    // encoding actually read, so a save taken in a development build does not
    // refuse itself in a shipped one.
    for name in FIXTURES {
        let scene = load(name);
        let Ok(document) = encode_document(&scene.to_document(), Encoding::MessagePack) else {
            panic!("{name}: a validated scene must encode");
        };
        let Ok(round_tripped) = decode_document(&document, Encoding::MessagePack) else {
            panic!("{name}: the MessagePack form must decode");
        };
        let Ok(from_json) = scene.content_hash() else {
            panic!("{name}: a validated scene must hash");
        };
        let Ok(again) = encode_document(&round_tripped, Encoding::MessagePack) else {
            panic!("{name}: the decoded document must re-encode");
        };
        assert_eq!(
            from_json.as_bytes(),
            blake3::hash(&again).as_bytes(),
            "{name}: the content hash must not depend on the route taken"
        );
    }
}

/// The provenance sidecar as a consumer sees it.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ProvenanceShape {
    scene: String,
    chunks: Vec<ProvenanceChunk>,
}

/// One chunk's authoring sources.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ProvenanceChunk {
    at: ChunkShape,
    sources: Vec<SourceShape>,
}

/// A chunk coordinate in the sidecar.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ChunkShape {
    x: u32,
    y: u32,
    z: u32,
}

/// One authoring site.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct SourceShape {
    file: String,
    line: u32,
}

/// Reads a fixture's provenance sidecar, failing loudly on a shape change.
///
/// # Panics
///
/// Panics when the sidecar is missing or no longer matches its published
/// shape, both of which are `make scenes` not having been run.
fn provenance(name: &str) -> ProvenanceShape {
    let raw = match scenes().read(format!("{name}.provenance.json")) {
        Ok(found) => found,
        Err(error) => panic!("{name}.provenance.json must exist; run `make scenes`: {error}"),
    };
    match serde_json::from_slice(&raw) {
        Ok(parsed) => parsed,
        Err(error) => panic!("{name}: the provenance sidecar must match its shape: {error}"),
    }
}

#[test]
fn every_populated_chunk_has_provenance() {
    // What carries a positional diagnostic the last step. A diagnostic naming
    // chunk (3, 1, 0) at local (7, 2, 4) is precise and useless on its own;
    // joined against this it names a layer file and a line somebody wrote.
    for name in FIXTURES {
        let sidecar = provenance(name);
        assert_eq!(sidecar.scene, *name);
        assert_eq!(
            sidecar.chunks.len(),
            load(name).voxels().populated_chunks(),
            "{name}: every populated chunk must have provenance"
        );
    }
}

#[test]
fn every_provenance_entry_names_a_layer_raster_and_a_line() {
    for name in FIXTURES {
        for chunk in &provenance(name).chunks {
            let at = &chunk.at;
            assert!(
                !chunk.sources.is_empty(),
                "{name}: chunk ({}, {}, {}) has no authoring source",
                at.x,
                at.y,
                at.z
            );
            assert!(
                chunk.sources.iter().all(|source| source.line > 0),
                "{name}: every line number must be one-based"
            );
            assert!(
                chunk
                    .sources
                    .iter()
                    .all(|source| source.file.starts_with("layers/")),
                "{name}: every source must name a layer raster"
            );
        }
    }
}
