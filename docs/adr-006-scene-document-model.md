# ADR 006: The scene document model

## Status

Accepted

## Date

2026-07-30

## Context and problem statement

Roadmap step 1.2 fills `crates/world` (the crate `thysalion-world`, the *state*
plane) with the scene format: the authored representation of a bounded voxel
world, its palette of voxel types, its entity spawns, its lighting, and its
knowledge references. Every later phase consumes scenes through this one API —
rendering at phase 2, lighting at phase 3, simulation at phase 4, knowledge at
phase 5, and save archives at phase 8 — so the shape fixed here is expensive to
change afterwards.

[thysalion-design.md](thysalion-design.md) §7.2 and §7.3 specify the format at
the level of a field list. They leave open the encoding's canonical byte form,
the module boundary, the validation contract, the compatibility policy, and
what happens when a hand-edited document is hostile rather than merely wrong.
This record fixes those, and resolves the open question
[ADR 005](adr-005-workspace-crate-layout.md) forwarded to this step.

## Decision drivers

- Design §7.3 requires two encodings from one structure: JSON for authoring
  and diffing, MessagePack for shipping.
- Design §7.3 requires all-or-nothing loading: "unknown palette references,
  out-of-bounds spawns, or dangling knowledge IRIs fail the load with a
  diagnostic, never a partially loaded scene."
- Design §12.3 requires save archives to carry content hashes over scene
  assets and to refuse a load on mismatch; invariant I3 depends on it. A stable
  canonical byte form is therefore a requirement of this step even though the
  hash is consumed at phase 8.
- Design §13, Table 6 commits to "load rejected with diagnostic; previous
  scene remains active" as a *player-facing* degradation path, which puts the
  loader on the untrusted side of a trust boundary.
- ADR 005 makes `thysalion-world` the dependency sink: no plane crate may
  become a dependency of it.
- Design §7.1, Table 1 sizes the wilderness scene class at 1024 x 1024 x 128,
  which is 134 million voxels — a dense encoding is 256 MiB.

## Decision outcome

### The format stays in `thysalion-world`, behind an optional `bevy` feature

ADR 005 forwarded the question of whether the scene format should become a leaf
`thysalion-scene` crate, given that palette entries carry optional
knowledge-plane concept Internationalized Resource Identifiers (IRIs).

It stays in `thysalion-world`. The concern that motivated the question does not
materialize: an identifier is validated for *syntax and namespace membership
only*, against a namespace table the caller injects, so nothing in the state
plane knows what a concept means and no dependency on `oxigraph` or
`thysalion-knowledge` arises. Resolving an identifier against the ontology is
the knowledge plane's work at roadmap step 5.1, and the dependency edge runs
`knowledge -> world`, never the reverse. A separate crate would buy a boundary
that is already enforced by the injected namespace table and by review.

The heavy dependency ADR 005 anticipated — `bevy`, for Entity Component System
types — is staged in behind an optional feature rather than being taken now, so
the scene format stays consumable without an engine.

**Reversal trigger.** Split out `thysalion-scene` when a consumer outwith this
workspace needs to read scene documents without taking the runtime voxel grid,
or when the `bevy` feature ceases to be optional. Neither is true at phase 2.

### Two stages: a permissive document, a validated domain

A scene exists in two forms. `scene::document::SceneDocument` and its
components are *wire* types: they derive `Serialize`, `Deserialize`, and
`JsonSchema`, they carry `#[serde(deny_unknown_fields)]`, and they are
deliberately permissive about values. `Scene`, `Palette`, `VoxelType`,
`Entities`, `SpawnPoint`, `SceneKnowledge`, `ConceptIri`, and `LightEmission`
are *domain* types: they derive no `serde` traits at all, their fields are
private or their constructors crate-private, and the only route to one is
`scene::validation::validate`.

This is the mechanism that makes all-or-nothing loading structural rather than
conventional. A derived `Deserialize` on a domain type is a public constructor
that reaches every private field and skips every invariant, so a caller could
materialize a `Palette` whose entry zero is not air. `#[non_exhaustive]` does
not close that hole either; it is documented as not restricting deserialization.

Not every type needs two forms. A closed enum such as `MaterialClass` has no
private field to protect and no invariant to enforce, so it is *shared*
vocabulary and carries no `Document` suffix. A second form would be two names
for one set of values and a conversion function that can only be the identity.
The suffix therefore marks a real distinction — `VoxelTypeDocument` against
`VoxelType`, `EmissionDocument` against `LightEmission` — rather than
decorating every type in the module.

### Every quantity is an integer

Angles are centi-degrees; lengths are millimetres; the simulation coefficients
design §10.5 mandates are Q8.8 fixed point, meaning the stored integer divided
by 256. Design §10.5 fixes the *width* at 16 bits but states no scale; this
record fixes the scale, and the design document is amended to match.

Three things follow. The whole document tree keeps `Eq`, `Ord`, and `Hash`,
which the canonical form and the content hash both need. NaN and the infinities
— which JSON cannot represent, and which `serde_json` writes as `null` and then
fails to read back — cannot arise. And the two encodings cannot disagree about
a float's representation.

The authoring cost is real and is paid in the generator rather than in the
format: `scene.toml` accepts `azimuth = "17.45deg"` and compiles it to `1745`.

### MessagePack is written with named fields, and that is a correctness rule

`rmp_serde::to_vec_named`, never `to_vec`. This is not a preference for
evolvability. `rmp_serde` writes structs as positional arrays by default, its
decoder accepts either shape silently, and `deny_unknown_fields` has no field
names to act on in the array form. An accidental `to_vec` would therefore ship
a scene that is positional, permanently unevolvable, and rejects nothing, while
looking healthy to any round-trip test. Only an explicit wire-shape assertion
catches it, and `crates/world/tests/document_round_trip.rs` carries one.

A consequence that is easy to miss: `serialize_tuple_struct` ignores the
struct-map configuration entirely. No document type may be a tuple struct.
`VoxelRunDocument` is a named struct with `length` and `index` fields for
exactly this reason — as a tuple struct it could never gain a field, and adding
one would misalign the MessagePack stream while JSON errored cleanly. The
divergent failure mode is the objection, not the size.

### Enum variants encode as their names

In both encodings, a variant is written as its `serde` name. Reordering
variants is therefore safe and renaming one is a wire break — the opposite of
the intuition a C programmer brings, and worth stating because the mistake is
silent.

`#[non_exhaustive]` on an enum does **not** make it tolerant of unknown
variants on the wire. It constrains downstream Rust code and nothing else. A
document naming a `material` this build does not know fails to deserialize, and
`#[serde(other)]` is unavailable for externally tagged enums, so there is no
catch-all to add. Widening a closed vocabulary is therefore a major version
bump, and the enums here are chosen to be genuinely closed.

### Palette index zero is always air

Reserved, and enforced by validation. An absent chunk and a long run of air
then agree without a lookup, which is what lets the sparse payload omit empty
chunks entirely rather than storing a palette-dependent fill value.

The palette is indexed by `u16`, so it holds at most 65,536 entries. Design
§7.2 fixes that width; narrowing it — Minecraft Anvil's bit-packed indices are
the obvious prior art — is a deferred optimization with a trigger rather than a
rejected idea. **Trigger.** Revisit when a measured fixture's palette is small
enough that packing would save more than ten per cent of the shipped bytes, and
treat it as a design amendment because §7.2 fixes the width.

### The voxel payload is chunk-keyed and sparse

Not design §7.3's "dense Z-major layers, run-length encoded". A scene stores a
sorted list of populated chunks; each carries either a single repeated index or
a canonical chunk-local Z-major run stream; an absent chunk is entirely air.

A global run stream fragments on every raster row of a spatially localized
region, and rewrites wholesale on a one-chunk edit. Chunk-keying makes
encoding, decoding, hashing, and diffing scale with *populated volume* rather
than with *declared extent*. The measured `swamp-fragment` fixture declares 134
million voxels and ships in 3.4 KiB.

The access cost is a `BTreeMap` lookup per chunk — O(log n) in populated
chunks, then O(1) within one — against O(1) for a dense array. That is the
trade, stated so a future swap has a criterion.

**Reversal trigger.** Reconsider when step 2.1.2's mesher or
`bevy_voxel_world`'s own chunk map makes this structure redundant, or when a
profile attributes more than five per cent of frame time to grid lookups.

### Validation runs in three ordered phases, and the order is correctness

1. **Header.** Version, dimensions, chunk-size alignment, and the declared
   collection sizes, against injected `Bounds`. Nothing in this phase allocates
   in proportion to a declared quantity.
2. **Bounded decode.** Only once the header is sound is the payload decoded,
   into an allocation the header has already bounded.
3. **Semantic rules.** Palette coherence, run canonicality, spawn placement,
   prototype resolution, identifier syntax, and knowledge-resource presence.

Diagnostics accumulate within a phase and a failing phase stops the next, so
the promise is *every distinct problem in the earliest failing phase*. An
unqualified "every problem" is not implementable: it would oblige the loader to
fully decode a document declaring a four-billion-entry palette so that the
later rules could also run, which is the resource exhaustion the bounds exist
to refuse. "Distinct" is also load-bearing — a single bad run expands to as
many identical faults as it is long, and a report of 32,768 consequences buries
every other diagnostic in the document.

Prototype chains resolve iteratively against a worklist and a depth bound,
never recursively. Cycle detection alone does not save a recursive resolver
from an *acyclic* chain ten thousand deep, and the resulting stack overflow is
a signal rather than a `Result` — uncatchable, and a direct contradiction of
the never-panics contract in precisely the place a hand-edited file reaches.

Two findings are **warnings** rather than errors, carried on `LoadedScene`: a
spawn inside a non-passable voxel, and a spawn with nothing beneath it and no
explicit airborne flag. A scene can legitimately want either.
`scene-check --strict` promotes them, and continuous integration runs strict.

### The compatibility policy

Two promises, and conflating them is the easiest mistake here.

**Source compatibility** — adding a field must not force an edit to a Rust
consumer. `#[non_exhaustive]` on the domain types delivers this, but only for
crates outwith `thysalion-world`.

**Wire compatibility** — a document written by one build must still load in
another. `#[non_exhaustive]` has no bearing on this whatsoever. It is governed
by the `serde` attribute policy and the version rule below, and it is enforced
by checked-in golden bytes rather than by regenerating fixtures. Fixtures are
regenerated by `make scenes`, so they are current-version by construction and
can never detect a wire break.

`DocumentVersion` is a `{major, minor}` pair with an accepted *range*, not a
single monotonic integer against a whitelist. Adding a field bumps the minor
and the field carries `#[serde(default)]`; removing, retyping, or re-meaning a
field bumps the major. A whitelist would make every additive change breaking,
and the collision is immediate — phase 3 adds a fog volume to the lighting
section, and the checked-in fixtures would fail to load until regenerated. It
is not a semantic version: a file format has one axis of change, and semantic
versioning only invites arguments about which component to bump.

Decoding probes the version through a deliberately permissive `VersionProbe`
*before* attempting the full structure, so a document from a future build is
reported as an unsupported version rather than as a confusing complaint about
an unknown field.

**The `serde` attribute policy.** Every document type carries
`deny_unknown_fields`. `#[serde(flatten)]` is forbidden, being incompatible
with it at runtime. `#[serde(untagged)]` is forbidden: it buffers through
serde's `Content`, collapses integer widths differently per format, and
destroys error locality. No document type has a hand-written `Serialize` or
`Deserialize`. No document type is a tuple struct.

### The canonical byte form

Design §12.3's content hashes require that equal scenes encode to equal bytes.
Four rules follow, and none may be relaxed.

- Every map in the document tree is a `BTreeMap`, never a `HashMap`.
- No field carries `#[serde(skip)]` or `#[serde(skip_serializing_if)]`. Under
  struct-as-map these make the encoded map *length* value-dependent, so two
  equal values encode to different bytes.
- The run stream is canonical: no zero-length run, and no two adjacent runs
  sharing an index.
- No serialized map has a non-string key. `serde_json` rejects struct, tuple,
  and sequence keys outright while MessagePack accepts them, so a
  coordinate-keyed collection would encode in one format and not the other.
  Coordinate-keyed collections reach the wire as a sequence of entry structs.

`Scene::content_hash` is a BLAKE3 digest over the **MessagePack** encoding of
the scene's document form, independent of the encoding it was loaded from. A
scene read as JSON and the same scene read as MessagePack therefore hash
identically, which is what stops a save taken in a development build refusing
itself in a shipped one.

### The loaded scene is an immutable baseline; edits live in an overlay

`VoxelGrid` is read-only. The loaded scene is the authored baseline, and it is
what design §12.3 content-hashes; runtime voxel edits belong to the state
plane's overlay, and the overlay is what a save archive records. Recording the
division here is what stops phase 2 inventing a parallel mutable grid.

### The namespace table is injected, and the knowledge plane owns it

The prefixes a scene's identifiers may use are supplied by the caller as a
`NamespaceTable`, defaulting to `thy:` and `scene:`. Baking the list into a
check would mean phase 5 editing the state plane to add a namespace, with no
Cargo edge for review to catch — exactly the coupling ADR 005's review-enforced
layering misses.

## Alternatives considered

### Sponge Schematic v3

The incumbent in the Minecraft-adjacent tooling ecosystem, and the closest
thing to a standard sparse voxel interchange. Rejected on two counts. It is
a gzipped, binary Named Binary Tag format, which forfeits the JSON authoring
encoding design §7.3 requires. And its palette entries are blockstate strings, carrying
none of passability, slope, emission, simulation coefficients, or concept IRI —
so the palette would have to be carried out of band, which is the whole of what
makes this format worth having.

### MagicaVoxel `.vox`

Design §7.4 promises `.vox` import, and `dot_vox` 5.2.0 is maintained and would
make it cheap. It is not the scene format, and cannot be: `.vox` caps a model
at 255 palette colours and 256 voxels per axis, against a 1024-extent scene
class and a 16-bit palette. It carries geometry and colour and nothing else.
That is precisely the prop-and-stamp importer design §7.4 actually describes,
and the format decided here is a viable target for one. Deferred to a later
content-tooling step rather than rejected.

### OpenVDB

Built for sparse *float* fields — level sets, density, velocity — where this
format carries small integer indices. Its Rust reader is partial and dormant. A
future fluid or fog field is where it would earn its place, not here.

### Apache Arrow or Parquet

This deserves its own answer because lille's map-format document — this
project's own lineage — floats exactly it: §9 proposes "an optional Apache
Arrow encoding (dictionary-encoded block IDs + sparse tensor)". The idea has
provenance and will be raised again if it is not answered.

They are columnar analytics formats, and the mismatch is in four places.

- A scene is one small, deeply nested, heterogeneous record *plus* one large
  homogeneous array. Parquet is excellent at the second and clumsy at the
  first, and a one-row table of nested structs is not what its tooling is for.
- Parquet is binary only, so design §7.3's JSON authoring encoding would need a
  second schema and a conversion between them — the two-writer drift hazard
  this step guards against elsewhere.
- Parquet files embed writer metadata, optional statistics, row-group layout,
  and codec choices. Byte-determinism across writer versions is not a property
  the format promises, and design §12.3's content hashes require it.
- The win Parquet offers — predicate pushdown and column pruning over large
  datasets — is one this project cannot spend. Scenes load whole, and design
  §6.3 makes the whole-scene load a performance contract rather than a query.

The narrower observation is the more useful one: Arrow's dictionary encoding is
what the palette already is, and its run-end encoding is what the chunk payload
already is. Adopting the framework would buy, at the cost of a large dependency
and a second schema, two things this format has in a few dozen lines.

Where Arrow *would* fit is the replay corpus of roadmap step 1.3.2 and
invariant I1: many homogeneous rows, analytical queries, no authoring surface.
Recorded there as worth revisiting rather than dismissed.

## Consequences

### Positive

- Loading is all-or-nothing by construction, not by discipline. There is no
  route to a `Scene` that has not passed every rule.
- A hostile document terminates within a bound without panicking, which is what
  design §13, Table 6's player-facing degradation path requires.
- Equal scenes encode to equal bytes, so phase 8's content hashes are well
  defined before phase 8 starts.
- The sparse payload makes the wilderness scene class affordable: 3.4 KiB
  against 256 MiB dense.
- Two independent implementations of one schema — this crate and
  `scripts/build_fixture_scenes.py` — are held in step by a byte-comparison
  test rather than by convention.

### Negative

- Two forms of several types, and a conversion between them. The cost is real
  and is the price of the validation gate being structural.
- Every quantity is an integer, which is hostile to authoring. Mitigated by the
  generator's human-unit parsing, not by the format.
- Grid access costs a `BTreeMap` lookup per chunk rather than an array index.
- Widening a closed enum is a major version bump, because there is no
  wire-level catch-all to add.

### Neutral

- The fixture documents are committed build artefacts. `make scenes-check`
  keeps them current; a contributor with no Python toolchain can still build,
  test, and run demos.
- Semantic plausibility — a room with no door, a building over a void — is out
  of scope. It needs phase 4's pathfinding to define reachability, and belongs
  to a scene lint built then. This step validates document integrity, not level
  design.

## References

- [thysalion-design.md](thysalion-design.md) §7.1–§7.4, §9.2–§9.4, §10.5,
  §12.3, §13
- [ADR 005](adr-005-workspace-crate-layout.md), whose forwarded open question
  this record resolves
- [world-plane-architecture.md](world-plane-architecture.md) for the module
  tree, the format reference, and the version-history table
- [The 1.2 execution plan](execplans/1-2-deliver-the-scene-format-and-fixture-scenes.md)
  for the measurements and the decision log behind these choices
