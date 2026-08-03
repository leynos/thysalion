//! Resource attacks, and the different assertion they carry.
//!
//! A corrupt fixture asks "does this produce the right diagnostic?". A hostile
//! input asks something weaker and more important: **does the loader terminate,
//! without panicking, within a bound?** Design §13, Table 6 already commits to
//! "load rejected with diagnostic; previous scene remains active" as a
//! *player-facing* degradation path, which puts the loader on the untrusted side
//! of a trust boundary. Scene documents are hand-editable, and from phase 8 they
//! will sit beside save archives that users exchange.
//!
//! These are constructed in memory rather than checked in. Each would run to
//! megabytes as a file — a palette of 65,537 entries, a payload of 16.8 million
//! chunk entries — and what matters about them is not their bytes but that a
//! header check refuses them before anything sizes an allocation from a declared
//! quantity. A checked-in file would test the same property at a hundred times
//! the repository cost.
//!
//! Every test here has a wall-clock bound. A bound that a correct
//! implementation clears by three orders of magnitude is not a benchmark; it is
//! the difference between a failing test and a continuous-integration job that
//! hangs until someone kills it.

use std::{
    sync::Arc,
    time::{Duration, Instant},
};

use cap_std::{ambient_authority, fs_utf8::Dir};
use thysalion_world::{
    codec::{Encoding, encode_document},
    loader::{SceneLoadError, SceneLoader},
    scene::{
        document::{
            ChunkCoordDocument,
            ChunkEntryDocument,
            ChunkPayloadDocument,
            PrototypeDocument,
            SceneDocument,
            VoxelRunDocument,
        },
        validation::{Bounds, DiagnosticCode},
    },
    source::{
        DirSceneSource,
        MAX_RESOURCE_BYTES,
        MemorySceneSource,
        SceneSource as _,
        SceneSourceError,
    },
};

mod support;

use support::minimal_document;

/// The wall-clock bound every hostile input must clear.
///
/// Generous by design. A header check that refuses a declared quantity before
/// allocating finishes in microseconds; a loader that materializes what the
/// document claims does not finish at all. Anything between the two is a
/// regression worth a failing test either way.
const BOUND: Duration = Duration::from_secs(5);

/// Runs `load` and asserts it returned within [`BOUND`].
///
/// # Panics
///
/// Panics when the load overruns, naming the input so a failure says which
/// attack got through.
fn within_bound<T>(what: &str, load: impl FnOnce() -> T) -> T {
    let started = Instant::now();
    let outcome = load();
    let elapsed = started.elapsed();
    assert!(
        elapsed < BOUND,
        "{what}: took {elapsed:?}, which is over the {BOUND:?} bound — the loader is \
         materializing what the document declares rather than refusing it"
    );
    outcome
}

/// Loads `document` through a loader with no resources at all.
///
/// The knowledge resource is deliberately absent: every input here fails long
/// before the knowledge rules run, and supplying one would only obscure which
/// phase did the refusing.
fn load(document: &SceneDocument) -> Result<(), SceneLoadError> {
    load_with(document, Bounds::DEFAULT)
}

/// Loads `document` against the given resource bounds.
///
/// An encoding failure is reported as a load failure rather than a panic: this
/// function returns a `Result`, and the workspace forbids asserting inside one.
/// No test here distinguishes the two, because every one of them asserts only
/// that the load *failed*, and a document these tests cannot even encode has
/// failed at least as hard.
fn load_with(document: &SceneDocument, bounds: Bounds) -> Result<(), SceneLoadError> {
    let bytes =
        encode_document(document, Encoding::Json).map_err(|error| SceneLoadError::Malformed {
            path: "<hostile>".into(),
            encoding: Encoding::Json,
            pointer: String::new(),
            message: error.to_string().into(),
        })?;
    let loader = SceneLoader::new(Arc::new(MemorySceneSource::new())).with_bounds(bounds);
    loader.load_bytes(&bytes, Encoding::Json).map(|_| ())
}

/// The codes a failed load reported.
fn codes(outcome: &Result<(), SceneLoadError>) -> Vec<&'static str> {
    match outcome {
        Ok(()) => Vec::new(),
        Err(error) => error
            .diagnostics()
            .iter()
            .map(|diagnostic| diagnostic.code().as_str())
            .collect(),
    }
}

#[test]
fn a_palette_larger_than_a_sixteen_bit_index_is_refused() {
    let mut document = minimal_document();
    let Some(entry) = document.palette.first().cloned() else {
        panic!("the minimal fixture must have a palette");
    };
    // One past what a `u16` can address. The entry above index 65,535 is
    // unreachable by any voxel, so a loader that accepted it would hold memory
    // nothing could ever refer to.
    document.palette = vec![entry; 65_537];

    let outcome = within_bound("a 65,537-entry palette", || load(&document));
    assert!(
        codes(&outcome).contains(&DiagnosticCode::PaletteTooLarge.as_str()),
        "got {:?}",
        codes(&outcome)
    );
}

#[test]
fn a_chunk_count_past_the_bound_is_refused_before_decoding() {
    // The bound is exercised by *lowering* it rather than by constructing the
    // 16,777,216 chunk entries the shipped value admits, which would cost more
    // memory than the attack it models. `Bounds` is injected precisely so the
    // rule can be tested at a scale a test can afford; the shipped value is a
    // ceiling, not the thing under test.
    let mut document = minimal_document();
    document.dimensions.x = 32 * 16;
    document.voxels = (0..12)
        .map(|index| ChunkEntryDocument {
            at: ChunkCoordDocument {
                x: index,
                y: 0,
                z: 0,
            },
            payload: ChunkPayloadDocument::Uniform(1),
        })
        .collect();

    let mut bounds = Bounds::DEFAULT;
    bounds.max_chunks = 8;
    let outcome = within_bound("twelve chunks against a bound of eight", || {
        load_with(&document, bounds)
    });
    assert!(
        codes(&outcome).contains(&DiagnosticCode::TooManyChunks.as_str()),
        "got {:?}",
        codes(&outcome)
    );
}

#[test]
fn a_run_stream_claiming_more_than_the_chunk_volume_is_refused() {
    let mut document = minimal_document();
    // Each run claims almost the whole `u32` range. Summed unchecked these wrap
    // to a plausible small volume, which is the worst possible failure for a
    // bound that exists to refuse oversized input — so the run codec sums on
    // `u64` and refuses.
    document.voxels = vec![ChunkEntryDocument {
        at: ChunkCoordDocument { x: 0, y: 0, z: 0 },
        payload: ChunkPayloadDocument::Runs(vec![
            VoxelRunDocument {
                length: u32::MAX,
                index: 1,
            },
            VoxelRunDocument {
                length: u32::MAX,
                index: 0,
            },
        ]),
    }];

    let outcome = within_bound("a run stream claiming 8.6 billion voxels", || {
        load(&document)
    });
    assert!(
        codes(&outcome).contains(&DiagnosticCode::RunLengthMismatch.as_str()),
        "got {:?}",
        codes(&outcome)
    );
}

#[test]
fn a_run_count_past_the_per_chunk_bound_is_refused() {
    let mut document = minimal_document();
    // 65,537 runs in a chunk that holds 32,768 voxels: more runs than there are
    // positions for them, which the header phase refuses on the declared count
    // before the decoder walks it.
    document.voxels = vec![ChunkEntryDocument {
        at: ChunkCoordDocument { x: 0, y: 0, z: 0 },
        payload: ChunkPayloadDocument::Runs(
            (0_u32..65_537)
                .map(|index| VoxelRunDocument {
                    length: 1,
                    index: u16::from(index.is_multiple_of(2)),
                })
                .collect(),
        ),
    }];

    let outcome = within_bound("65,537 runs in one chunk", || load(&document));
    assert!(
        codes(&outcome).contains(&DiagnosticCode::TooManyRuns.as_str()),
        "got {:?}",
        codes(&outcome)
    );
}

#[test]
fn a_prototype_chain_past_the_depth_bound_terminates() {
    let mut document = minimal_document();
    // Ten thousand deep and entirely acyclic, which is the case cycle detection
    // alone does not save a recursive resolver from. The resulting stack
    // overflow would be a signal rather than a `Result` — uncatchable, and a
    // direct contradiction of the never-panics contract in precisely the place a
    // hand-edited file reaches.
    let depth = 10_000_u32;
    document.entities.prototypes = (0..depth)
        .map(|index| {
            let next = (index + 1 < depth).then(|| format!("link-{}", index + 1).into());
            (
                format!("link-{index}").into(),
                PrototypeDocument {
                    extends: next,
                    concept: None,
                },
            )
        })
        .collect();
    if let Some(spawn) = document.entities.spawns.first_mut() {
        spawn.prototype = Some("link-0".into());
    }

    let outcome = within_bound("a 10,000-deep prototype chain", || load(&document));
    assert!(
        codes(&outcome).contains(&DiagnosticCode::PrototypeTooDeep.as_str()),
        "got {:?}",
        codes(&outcome)
    );
}

#[test]
fn a_self_referential_prototype_terminates() {
    let mut document = minimal_document();
    document.entities.prototypes = [(
        "torch".into(),
        PrototypeDocument {
            extends: Some("torch".into()),
            concept: None,
        },
    )]
    .into_iter()
    .collect();
    if let Some(spawn) = document.entities.spawns.first_mut() {
        spawn.prototype = Some("torch".into());
    }

    let outcome = within_bound("a self-extending prototype", || load(&document));
    assert!(
        codes(&outcome).contains(&DiagnosticCode::PrototypeCycle.as_str()),
        "got {:?}",
        codes(&outcome)
    );
}

#[test]
fn deeply_nested_json_does_not_overflow_the_stack() {
    // `serde_json` bounds recursion depth itself, and the assertion is only that
    // this returns an error rather than aborting the process — a stack overflow
    // is not a `Result` and cannot be reported as one.
    let depth = 100_000;
    let mut bytes = Vec::with_capacity(depth * 2);
    bytes.extend(std::iter::repeat_n(b'[', depth));
    bytes.extend(std::iter::repeat_n(b']', depth));

    let loader = SceneLoader::new(Arc::new(MemorySceneSource::new()));
    let outcome = within_bound("100,000 levels of nested JSON", || {
        loader.load_bytes(&bytes, Encoding::Json)
    });
    assert!(outcome.is_err(), "nested JSON must not load as a scene");
}

#[test]
fn a_truncated_messagepack_payload_is_refused() {
    let document = minimal_document();
    let bytes = match encode_document(&document, Encoding::MessagePack) {
        Ok(bytes) => bytes,
        Err(error) => panic!("the fixture must encode: {error}"),
    };
    let Some(truncated) = bytes.get(..bytes.len().div_euclid(2)) else {
        panic!("the encoding must be long enough to halve");
    };

    let loader = SceneLoader::new(Arc::new(MemorySceneSource::new()));
    let outcome = within_bound("a half-length MessagePack payload", || {
        loader.load_bytes(truncated, Encoding::MessagePack)
    });
    assert!(outcome.is_err(), "a truncated payload must not load");
}

#[test]
fn a_messagepack_payload_declaring_a_gigantic_array_is_refused() {
    // `0xdd` is `array32`, followed by a big-endian length. This claims four
    // billion elements and then supplies none. A decoder that pre-allocates from
    // the declared length exhausts memory before reading a byte of content,
    // which is why the length is a claim to be checked rather than a size to
    // trust.
    let bytes = vec![0xdd, 0xff, 0xff, 0xff, 0xff];

    let loader = SceneLoader::new(Arc::new(MemorySceneSource::new()));
    let outcome = within_bound("a MessagePack array32 claiming 4.29 billion items", || {
        loader.load_bytes(&bytes, Encoding::MessagePack)
    });
    assert!(outcome.is_err(), "a gigantic declared array must not load");
}

#[test]
fn an_empty_input_is_refused_rather_than_treated_as_an_empty_scene() {
    let loader = SceneLoader::new(Arc::new(MemorySceneSource::new()));
    for encoding in [Encoding::Json, Encoding::MessagePack] {
        let outcome = loader.load_bytes(&[], encoding);
        assert!(outcome.is_err(), "{encoding}: empty input must not load");
    }
}

#[test]
fn an_over_long_chunk_list_is_refused_without_describing_its_contents() {
    // The chunk bound is the one that makes every later walk safe, so it must
    // stop the scan rather than annotate it. Every entry here also breaches the
    // per-chunk run bound; if the scan continued, each would be described.
    let mut document = minimal_document();
    let mut bounds = Bounds::DEFAULT;
    bounds.max_chunks = 4;
    bounds.max_runs_per_chunk = 2;
    document.voxels = (0_u32..64)
        .map(|index| ChunkEntryDocument {
            at: ChunkCoordDocument {
                x: index,
                y: 0,
                z: 0,
            },
            payload: ChunkPayloadDocument::Runs(
                (0_u32..8)
                    .map(|run| VoxelRunDocument {
                        length: 4096,
                        index: u16::from(run.is_multiple_of(2)),
                    })
                    .collect(),
            ),
        })
        .collect();

    let outcome = within_bound("64 chunks against a bound of 4", || {
        load_with(&document, bounds)
    });
    // Exactly one: the list is too long, and that is the whole finding.
    assert_eq!(
        codes(&outcome),
        vec![DiagnosticCode::TooManyChunks.as_str()],
        "an over-long chunk list must not be described entry by entry"
    );
}

#[test]
fn many_over_long_chunks_are_summarized_rather_than_listed_or_dropped() {
    // Within the chunk bound, so the scan runs — but the diagnostics it
    // produces must stay bounded, because `SceneLoadError::Invalid` carries
    // them and `scene-check` renders every one.
    let mut document = minimal_document();
    let mut bounds = Bounds::DEFAULT;
    bounds.max_chunks = 64;
    bounds.max_runs_per_chunk = 2;
    document.voxels = (0_u32..40)
        .map(|index| ChunkEntryDocument {
            at: ChunkCoordDocument {
                x: index,
                y: 0,
                z: 0,
            },
            payload: ChunkPayloadDocument::Runs(
                (0_u32..8)
                    .map(|run| VoxelRunDocument {
                        length: 4096,
                        index: u16::from(run.is_multiple_of(2)),
                    })
                    .collect(),
            ),
        })
        .collect();

    let outcome = within_bound("40 over-long chunks", || load_with(&document, bounds));
    let reported = codes(&outcome)
        .iter()
        .filter(|code| **code == DiagnosticCode::TooManyRuns.as_str())
        .count();
    // Eight listed plus one summary. Not forty, and not silently eight.
    assert_eq!(reported, 9, "got {:?}", codes(&outcome));
    let Err(error) = outcome else {
        panic!("40 over-long chunks must not load");
    };
    let rendered: Vec<String> = error
        .diagnostics()
        .iter()
        .map(ToString::to_string)
        .collect();
    assert!(
        rendered.iter().any(|line| line.contains("32 further")),
        "the unlisted remainder must be counted, got {rendered:?}"
    );
}

#[test]
fn a_second_document_appended_to_the_first_is_refused() {
    // A document is one value, not the first value of a stream. Accepting a
    // prefix means a file holding two scenes — or one scene and the tail of an
    // interrupted write — loads as whichever came first, silently, and the
    // content hash then describes bytes nobody can reproduce from the file.
    let document = minimal_document();
    let Ok(json) = encode_document(&document, Encoding::Json) else {
        panic!("the minimal document must encode as JSON");
    };
    let Ok(msgpack) = encode_document(&document, Encoding::MessagePack) else {
        panic!("the minimal document must encode as MessagePack");
    };

    for (encoding, one) in [(Encoding::Json, json), (Encoding::MessagePack, msgpack)] {
        let mut bytes = one.clone();
        bytes.extend_from_slice(&one);
        let loader = SceneLoader::new(Arc::new(MemorySceneSource::new()));
        let outcome = loader.load_bytes(&bytes, encoding).map(|_| ());
        let Err(SceneLoadError::Malformed { message, .. }) = outcome else {
            panic!("{encoding}: a doubled document must be refused as malformed");
        };
        assert!(
            message.contains("trailing input"),
            "{encoding}: got {message}"
        );
    }
}

#[test]
fn a_knowledge_resource_above_the_bound_is_refused_before_it_is_read() {
    // Validation reads each declared resource only to decide whether it
    // resolves, and discards the bytes. Without a bound, a scene naming one
    // enormous file inside its own root buys an allocation proportional to that
    // file for a result nobody consumes.
    //
    // Written through `cap_std` rather than `std::fs`, which the workspace
    // lints forbid: the point of the capability policy is that a reader sees
    // every filesystem surface a module touches, and a test is not exempt.
    // `CARGO_TARGET_TMPDIR` rather than a `tempfile` dependency, because the
    // plan treats a new dependency as an escalation and Cargo already hands
    // every integration test a scratch directory.
    let scratch = camino::Utf8PathBuf::from(env!("CARGO_TARGET_TMPDIR"));
    let Ok(parent) = Dir::open_ambient_dir(&scratch, ambient_authority()) else {
        panic!("the cargo scratch directory must be openable");
    };
    if parent.open_dir("resource-bound").is_err() {
        parent
            .create_dir("resource-bound")
            .expect("the scratch subdirectory must be creatable");
    }
    let Ok(root) = parent.open_dir("resource-bound") else {
        panic!("the scratch subdirectory must be openable");
    };
    let bytes = vec![b'#'; usize::try_from(MAX_RESOURCE_BYTES).unwrap_or(usize::MAX) + 1];
    root.write("big.trig", &bytes)
        .expect("the oversized resource must be writable");

    let source = DirSceneSource::new(root, "resource-bound");
    let outcome = source.read(camino::Utf8Path::new("big.trig"));
    assert!(
        matches!(outcome, Err(SceneSourceError::TooLarge { .. })),
        "an oversized resource must be refused by size, got {outcome:?}"
    );
}
