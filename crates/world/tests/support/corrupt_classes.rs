//! The corruption-class contract: which diagnostic each corrupt fixture must
//! produce.
//!
//! Extracted from `corrupt_fixtures.rs` because that file exceeded the 400-line
//! cap AGENTS.md sets, and because these two tables are inline data rather than
//! test logic — exactly what the same rule says to move out. The tables stay
//! together, and stay readable as a single list, so a reviewer can still see
//! the whole mapping without opening twenty-nine documents.

/// Every fixture, and the diagnostic code it must produce.
///
/// `None` means the document does not parse at all, so there is no document to
/// locate a fault within and the report carries a `serde` path instead of a
/// code. Both such fixtures still fail as *invalid* rather than as unreadable:
/// the bytes were read perfectly well.
pub const CLASSES: &[(&str, Option<&str>)] = &[
    // Header phase: refused before anything sizes an allocation.
    ("unsupported-version", Some("scene.version.unsupported")),
    ("chunk-size-not-design", Some("scene.chunk-size.not-design")),
    ("zero-dimension", Some("scene.dimensions.zero")),
    ("unaligned-dimensions", Some("scene.dimensions.unaligned")),
    ("dimensions-overflow", Some("scene.dimensions.overflow")),
    // Palette coherence.
    ("empty-palette", Some("scene.palette.empty")),
    ("palette-zero-not-air", Some("scene.palette.zero-not-air")),
    (
        "duplicate-voxel-type-name",
        Some("scene.palette.duplicate-name"),
    ),
    (
        "emission-out-of-range",
        Some("scene.palette.emission-out-of-range"),
    ),
    ("concept-iri-invalid", Some("scene.concept.invalid")),
    // The voxel payload.
    (
        "chunk-outwith-extent",
        Some("scene.voxels.chunk-outwith-extent"),
    ),
    ("duplicate-chunk", Some("scene.voxels.duplicate-chunk")),
    (
        "chunks-out-of-order",
        Some("scene.voxels.chunks-out-of-order"),
    ),
    (
        "unknown-palette-index",
        Some("scene.voxels.unknown-palette-index"),
    ),
    ("zero-length-run", Some("scene.voxels.zero-length-run")),
    (
        "adjacent-duplicate-runs",
        Some("scene.voxels.adjacent-duplicate-runs"),
    ),
    (
        "run-length-mismatch",
        Some("scene.voxels.run-length-mismatch"),
    ),
    // Entities and their prototype chains.
    (
        "spawn-outwith-grid",
        Some("scene.entities.spawn-outwith-grid"),
    ),
    (
        "duplicate-spawn-name",
        Some("scene.entities.duplicate-spawn-name"),
    ),
    (
        "unknown-prototype",
        Some("scene.entities.unknown-prototype"),
    ),
    ("prototype-cycle", Some("scene.entities.prototype-cycle")),
    (
        "prototype-too-deep",
        Some("scene.entities.prototype-too-deep"),
    ),
    // Knowledge resources.
    ("graph-iri-invalid", Some("scene.knowledge.graph-invalid")),
    ("resource-absent", Some("scene.knowledge.resource-absent")),
    (
        "resource-unsafe-path",
        Some("scene.knowledge.resource-unsafe-path"),
    ),
    // Documents that do not parse.
    ("unknown-document-field", None),
    ("truncated-json", None),
];

/// The two advisory classes, which load and are refused only by `--strict`.
pub const WARNINGS: &[(&str, &str)] = &[
    ("spawn-obstructed", "scene.entities.spawn-obstructed"),
    ("spawn-unsupported", "scene.entities.spawn-unsupported"),
];
