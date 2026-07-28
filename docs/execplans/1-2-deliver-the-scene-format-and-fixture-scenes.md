# Deliver the scene format and fixture scenes (roadmap 1.2)

This ExecPlan (execution plan) is a living document. The sections `Constraints`,
`Tolerances`, `Risks`, `Progress`, `Surprises & discoveries`, `Decision log`,
and `Outcomes & retrospective` must be kept up to date as work proceeds.

Status: DRAFT

## Purpose / big picture

Thysalion is an isometric voxel role-playing game built on four architectural
planes — state, logic, knowledge, and presentation — described in
[thysalion-design.md](../thysalion-design.md) §6.1. Roadmap step 1.1 delivered
the workspace and the demo harness; `crates/world` (the crate `thysalion-world`,
the *state* plane) is still an empty skeleton whose module comment says so.

Roadmap step 1.2 ([roadmap.md](../roadmap.md) §1.2) fills it. After this change
a contributor can take a scene written as a human-readable JSON document, load
it through one validating loader, and get back either a fully-formed in-memory
scene or a list of precise diagnostics and nothing else. Three fixture scenes —
a keep interior, a market-town block, and a swamp fragment — ship with the
repository and become the shared substrate every later spike renders, lights,
simulates, and tests against.

Concretely, after this change:

1. A JSON scene document round-trips losslessly through the Rust model and the
   MessagePack encoding: `document -> JSON -> document -> MessagePack ->
   document` yields a value equal to the original (task 1.2.1).
2. Loading a corrupt scene produces a distinct, named diagnostic for each
   corruption class, reports *every* problem found rather than the first, and
   never yields a partially constructed scene (task 1.2.2).
3. Running `cargo run -p thysalion-world --bin scene-check --
   assets/scenes/keep-interior.scene.json` prints `keep-interior: ok` and the
   same command against `crates/world/tests/fixtures/corrupt/unknown-palette-index.scene.json`
   prints a numbered diagnostic list and exits non-zero (task 1.2.2).
4. All three fixture scenes under `assets/scenes/` load clean through the
   validator, exercised by a behavioural suite that runs headless in continuous
   integration (task 1.2.3).

The stability promise of this step is *one document, one loader, one set of
diagnostics*. Every later phase — rendering (phase 2), lighting (phase 3),
simulation (phase 4), knowledge (phase 5), saves (phase 8) — consumes scenes
through this API. Adding a field to the scene format must never require editing
an existing consumer, which is enforced by `#[non_exhaustive]` on every public
document type and by every fixture continuing to load under `make all`.

This step also resolves the open question
[ADR 005](../adr-005-workspace-crate-layout.md) forwarded to it: whether the
scene format becomes a leaf `thysalion-scene` crate or stays inside
`thysalion-world`. The answer, and its reversal trigger, are recorded in a new
ADR 006.

## Constraints

Hard invariants that must hold throughout implementation. Violation requires
escalation, not workarounds.

- The plane authority table in [thysalion-design.md](../thysalion-design.md)
  §6.1 and the layering rules in
  [ADR 005](../adr-005-workspace-crate-layout.md) govern crate edges.
  `thysalion-world` is the dependency sink: no plane crate may become a
  dependency of it. In particular the scene model must not depend on
  `thysalion-knowledge`, `thysalion-sim`, or `thysalion-presentation`, and must
  not depend on `oxigraph` to handle concept Internationalized Resource
  Identifiers (IRIs).
- The scene format is specified by
  [thysalion-design.md](../thysalion-design.md) §7.2 and §7.3. Any departure
  from the field set or semantics recorded there must be justified in the
  decision log and written back into the design document in the same change.
  The design document is the specification; this plan may amend it, never
  silently diverge from it.
- Validation happens entirely at load and is all-or-nothing (design §7.3):
  "unknown palette references, out-of-bounds spawns, or dangling knowledge IRIs
  fail the load with a diagnostic, never a partially loaded scene." No public
  API may return a `Scene` value that has not passed validation.
- The workspace lint table in the root `Cargo.toml` applies unchanged. The
  denials that bite hardest here are `clippy::indexing_slicing`,
  `clippy::cast_possible_truncation`, `clippy::cast_possible_wrap`,
  `clippy::float_arithmetic`, `clippy::unwrap_used`, `clippy::expect_used`,
  `clippy::integer_division`, `clippy::result_large_err`,
  `clippy::missing_const_for_fn`, and `rust::missing_docs`. ADR 005 states that
  "simulation and world crates take no numeric allowances", so this step
  introduces no `#[expect]` for `clippy::float_arithmetic` or the cast lints in
  `crates/world`. Voxel indexing therefore uses `slice::get`, and every
  narrowing conversion uses `TryFrom` with a named error.
- `make all` (check-fmt, lint, test, spelling) is the commit gate and must pass
  at every commit. Documentation changes must additionally pass
  `make markdownlint` and `make nixie`. Red test states live only in the working
  tree, never in a commit (AGENTS.md quality gates).
- Filesystem access uses `cap_std::fs_utf8` and `camino`, never `std::fs` or
  `std::path` (AGENTS.md). The scene loader itself performs no ambient-authority
  filesystem access: it reads through an injected port.
- Every module begins with a `//!` comment; every public item carries Rustdoc;
  no source file exceeds 400 lines (AGENTS.md).
- Dependencies use caret requirements and are declared once in
  `[workspace.dependencies]` (ADR 005); member manifests use
  `{ workspace = true }`.
- All documentation follows the
  [documentation style guide](../documentation-style-guide.md): en-GB-oxendict
  spelling, sentence-case headings, 80-column prose wrap, 120-column code, and a
  language identifier on every fenced block. Python helper scripts follow
  [scripting standards](../scripting-standards.md): Python 3.13, an inline `uv`
  metadata block, Cyclopts for the command-line interface, and pytest coverage.
- Binary assets are Git Large File Storage (LFS) attachments
  (developers-guide.md). This step ships no new binary asset type; fixture
  scenes are text JSON and are tracked normally.

## Tolerances (exception triggers)

Thresholds that trigger escalation. They bound autonomous action; they are not
quality criteria.

- Scope: if the change grows beyond roughly 40 files or 3,500 net lines
  (excluding `Cargo.lock`, generated fixture JSON, and `insta` snapshots), stop
  and escalate.
- Dependencies: the approved new dependency set is exactly `serde` (with
  `derive`), `serde_json`, `rmp-serde`, `thiserror`, `miette`, `smol_str` (with
  `serde`), `serde_path_to_error`, and `schemars` as normal dependencies of
  `thysalion-world`, plus `insta` and `rmpv` as workspace dev-dependencies.
  `cap-std`, `camino`, `rstest`, `proptest`, `trybuild`, and the `rstest-bdd`
  trio are already pinned. Any dependency outwith that list — including `glam`,
  `ariadne`, `bevy`, `bevy_voxel_world`, `dot_vox`, or `block-mesh` — requires
  escalation before adoption.
- Interface: if a public item added by this step must change shape after the
  fixture scenes are authored against it, stop and escalate rather than editing
  fixtures to match a churning API.
- Fixture size: if any fixture scene's JSON document exceeds 1 MiB on disk, or
  the decoded in-memory grid for any fixture exceeds 16 MiB, stop and escalate —
  the storage strategy is wrong. The measured budgets these bound are in
  `Risks`; the tripwires sit roughly two-and-a-half times above the expected
  values, so tripping one means a real regression, not tuning noise.
- Load time: if any fixture takes more than 250 ms to load and validate in a
  release build on the development machine, stop and escalate. Design §6.3 makes
  the Loading-to-Active transition a performance contract, and every later phase
  pays this cost on every scene entry and every test.
- Iterations: if the same test still fails after five attempts, stop and
  escalate with the failure output.
- Ambiguity: if the design document and this plan disagree on the meaning of a
  scene-format field, stop and present the readings rather than choosing one.

## Risks

- Risk: JSON and MessagePack diverge silently because a type's `serde`
  implementation branches on `Serializer::is_human_readable`, and the failure is
  invisible until a shipped scene decodes differently from the authored one.
  Severity: high. Likelihood: low, given the mitigation.
  Mitigation: type discipline, not configuration. The hazard list is narrower
  than folklore suggests — in the standard library only the `std::net` address
  types branch, plus `uuid::Uuid` and the `chrono` types outwith it; `glam`
  vectors do not branch, and derived enums encode their variant name as a string
  in both formats. ADR 006 therefore states a closed rule: a document type may
  contain only integers, booleans, `String`, `SmolStr`, `Option`, `Vec`,
  `BTreeMap`, plain derived enums, and other document types. No manual `serde`
  implementation, no `Uuid`, no `chrono`, no network types, no
  `serde_json::Value`. The enforcing guard is a `proptest` round-trip property
  over generated documents, because the failure is runtime and no lint or
  `trybuild` case can catch it.
- Risk: `rmp-serde` serializes structs as positional arrays by default, so
  adding or reordering a field silently reinterprets previously encoded bytes.
  Severity: high. Likelihood: high if unaddressed.
  Mitigation: encode through `rmp_serde::to_vec_named` (equivalently
  `Serializer::new(w).with_struct_map()`), never `to_vec` or a bare
  `Serializer::new`. This is writer-side only: `rmp-serde`'s deserializer
  forwards `deserialize_struct` to its any-value path and accepts either shape,
  which makes the migration one-way and painless but also means an accidentally
  tuple-encoded payload will *not* surface as a decode error. The choice is
  therefore pinned by a test that decodes the encoder's output into an
  `rmpv::Value` and asserts it is a map with the expected keys, not merely that
  it round-trips.
- Risk: the design's scene size classes (design §7.1, Table 1) reach
  1024 x 1024 x 128 voxels for the wilderness class. Decoded densely at two
  bytes per voxel that is 256 MiB, and a naive global run stream over it is
  similarly unaffordable to encode, hash, or diff.
  Severity: high. Likelihood: high if stored as one flat stream.
  Mitigation: the document's voxel payload is keyed by chunk, and the decoded
  grid is sparse by chunk, so both scale with populated volume rather than with
  declared extent. The arithmetic, at the design's 32-cubed chunk size:

  | Class      | Extent            | Voxels  | Chunks | Dense   | Populated | Sparse |
  | ---------- | ----------------- | ------- | ------ | ------- | --------- | ------ |
  | Interior   | 128 x 128 x 64    | 1.0 M   | 32     | 2 MiB   | 32        | 2 MiB  |
  | District   | 512 x 512 x 96    | 25.2 M  | 768    | 48 MiB  | 32        | 2 MiB  |
  | Wilderness | 1024 x 1024 x 128 | 134.2 M | 4 096  | 256 MiB | 64        | 4 MiB  |

  Each populated chunk is 32,768 voxels, or 64 KiB decoded. The fixture-size
  tolerance sits at 16 MiB decoded, comfortably above the 4 MiB worst case here
  and comfortably below anything that would trouble continuous integration.
- Risk: run-length encoding a spatially localized fixture against a *global*
  Z-major linear order fragments badly: every raster row of a populated region
  is interrupted by the air either side of it, so a 256 x 256 x 32 populated
  fragment produces on the order of 8,000 interrupted rows and, at a plausible
  twenty runs per row, roughly 165,000 runs — about 2 MB of JSON, over the
  fixture tolerance, and a payload in which a one-building edit rewrites the
  whole stream and defeats review.
  Severity: high. Likelihood: high under the design's literal wording.
  Mitigation: runs are chunk-local, and a chunk that is entirely one voxel type
  is elided to a single uniform token (the trick Minecraft's Anvil format uses
  for single-state sections). Locality collapses the run count by roughly an
  order of magnitude, an edit touches one chunk's payload so the diff is
  readable, and re-encoding costs time proportional to populated chunks rather
  than to declared extent. This departs from design §7.3's literal "dense
  Z-major layers, run-length encoded" and is written back into the design
  document in Stage D.
- Risk: design §12.3 requires save archives to carry content hashes over the
  scene assets and to refuse a load on mismatch, and invariant I3 depends on it.
  A format without a canonical byte form cannot support that, and the failure
  would appear in phase 8 as saves that refuse themselves.
  Severity: medium. Likelihood: medium.
  Mitigation: canonical ordering is designed in now, not retrofitted. Every map
  in the document is a `BTreeMap` or a sequence sorted by an explicit key, chunk
  entries are sorted by chunk coordinate, `serde` derive fixes field order, and
  `to_vec_named` makes that order observable. A test asserts that encoding the
  same document twice yields identical bytes, and that a document round-tripped
  through JSON re-encodes to the same MessagePack bytes.
- Risk: hand-authoring even a 128 x 128 x 64 interior as literal JSON is not
  humanly possible, so "author the three fixture scenes" degenerates into
  checking in machine noise that no reviewer can diff.
  Severity: medium. Likelihood: high.
  Mitigation: fixtures are authored as layered text layouts with a character
  legend — the text analogue of the Tiled layered-isometric workflow in design
  §7.4 — and compiled to JSON by a checked-in, tested `uv` script. Both the
  layouts and the generated JSON are tracked, so review reads the layouts and
  continuous integration proves the JSON matches.
- Risk: the concept IRI field (design §7.2) tempts a dependency on the
  knowledge plane, inverting the layering rule.
  Severity: medium. Likelihood: medium.
  Mitigation: `ConceptIri` is a validated string newtype in `thysalion-world`
  that checks syntax and project-namespace membership only. Resolution against
  the ontology is the knowledge plane's job at roadmap step 5.1, and the
  dependency edge runs `knowledge -> world`. The project namespaces are not yet
  written down anywhere, so this step must define them — a prefix table mapping
  `thy:` and the scene-graph prefix to their full IRI bases — in ADR 006 and the
  world-plane architecture document. Without that table the namespace check has
  nothing to check against and the "dangling knowledge IRI" diagnostic is
  vacuous.
- Risk: phase 2 needs to mutate voxels (roadmap step 2.1.3, incremental
  re-meshing on edit), but the `Scene` this step produces is an immutable
  load-time value with no edit interface. The likely accident is a second,
  mutable voxel representation appearing beside it, after which the two diverge
  and nobody can say which is authoritative.
  Severity: medium. Likelihood: medium.
  Mitigation: state the answer now rather than discovering it in phase 2. Design
  §7.1 already settles it — `bevy_voxel_world` supplies "the chunk map and edit
  overlay", so the loaded `Scene` is the authored snapshot and runtime edits
  live in the voxel world's overlay, with the ownership matrix in design §12.1
  naming the Entity Component System as authoritative for the voxel grid.
  `VoxelGrid` therefore ships read-only on purpose, and the world-plane
  architecture document says so explicitly so that phase 2 wires the overlay
  instead of inventing a parallel grid.
- Risk: the behavioural suite cannot reuse the existing Bevy `rstest-bdd`
  adapter, because `thysalion-world` has no Bevy dependency at this phase, and
  adding one to satisfy the harness would contradict ADR 005's staging.
  Severity: low. Likelihood: high.
  Mitigation: a second, structurally identical `HarnessAdapter` whose context is
  a plain loader session. Both adapters are promoted into a shared test-support
  crate at roadmap step 1.3.1, which the developers' guide already names as the
  promotion point. See the decision log.
- Risk: run-length decoding is the one hot loop in this step and sits under
  `clippy::indexing_slicing` and the cast denials, tempting a suppression.
  Severity: low. Likelihood: medium.
  Mitigation: the decoder is written against iterators and `slice::get_mut`,
  with `TryFrom` at every narrowing boundary, and a `proptest` covering
  encode/decode fixpoint over random grids. If a suppression looks unavoidable,
  that is a tolerance breach, not a judgement call.

## Progress

Four commits are expected, one per stage from C1 onwards, each passing
`make all`. Stage A leaves no tracked changes beyond the dependency pins, which
travel with the C1 commit.

- [ ] Stage A: verification spikes A1–A3 and dependency pinning.
- [ ] Stage B: the behavioural specification and the task 1.2.1 red tests.
- [ ] Stage C1: the voxel type registry and scene document model (task 1.2.1) —
  first commit.
- [ ] Stage C2: scene loading with load-time validation (task 1.2.2) — opens
  with its own red step; second commit.
- [ ] Stage C3: the three fixture scenes and their generator (task 1.2.3) —
  opens with its own red step; third commit.
- [ ] Stage D: documentation, ADR 006, design-document amendments, refactor,
  and cleanup — fourth commit.

## Surprises & discoveries

- Observation: `insta` is not yet a workspace dependency, despite AGENTS.md
  requiring snapshot tests where multivariant output format consistency matters.
  Evidence: no `insta::` reference anywhere in the workspace; absent from
  `[workspace.dependencies]`.
  Impact: this step introduces it, and the diagnostic report is its first
  subject. The pin belongs in `[workspace.dependencies]` alongside the other
  test tooling.
- Observation: design §7.3 says the scene format follows "lille's proven
  format", but lille's JSON/MessagePack map format is an unimplemented
  specification. Lille ships a Tiled `.tmx` pipeline through `bevy_ecs_tiled`
  instead, and its runtime representation is a flat `Block`/`BlockSlope` pair
  with no palette, no per-face passability, and no prototype inheritance.
  Evidence: `docs/map-data-format.md` in `leynos/lille` describes the JSON
  document, but no corresponding module exists in `src/` and `Cargo.toml`
  carries no MessagePack dependency; `src/map/translate.rs` builds `Block`
  records from Tiled tile positions with `z` hardcoded to zero.
  Impact: material. The design document's appeal to prior art is weaker than it
  reads, so this step must justify its format choices on their own merits
  rather than by inheritance. Design §7.3 is corrected in Stage D to say the
  format follows lille's *specification*, and the specific inheritances are
  named. Three concrete lessons are taken from lille's document and are
  improved upon here rather than copied: its per-face passability is a
  stringly-keyed dictionary of unit-normal strings with no completeness
  guarantee (this plan uses a six-field struct); its `extends` mechanism
  specifies no cycle detection, depth limit, or resolution order (this plan
  specifies all three); and it carries no schema version field at all (this
  plan has `DocumentVersion` from the first commit). Its palettization is also
  signalled by the mere presence of a palette array, which this plan avoids by
  always palettizing.
- Observation: lille's slope story diverged between format and implementation —
  the documented format carries a horizontal direction of rise only
  (`slope_dir: [sx, sy]`), while the shipping `BlockSlope` component carries
  continuous `grad_x`/`grad_y` gradients, and the format never caught up.
  Evidence: `docs/map-data-format.md` versus `src/components.rs` in
  `leynos/lille`.
  Impact: a direction-only slope encoding is likely to prove insufficient once
  roadmap step 4.2.1 resolves motion over ramps. This plan keeps design §7.2's
  direction-only semantics (as `SlopeDirection`) but records the pressure, and
  the `#[non_exhaustive]` `VoxelType` leaves room for a rise magnitude to
  arrive without breaking authored scenes.
- Observation: `bevy_voxel_world` 0.17.0 (released 2026-07-05) declares
  `bevy ^0.19` and integrates authored content through a `voxel_lookup_delegate`
  closure on its `VoxelWorldConfig` trait, so the crate that *constructs* the
  configuration needs the scene, but the scene format needs nothing from
  `bevy_voxel_world`.
  Evidence: the crate's published dependency set and the `VoxelWorldConfig`
  documentation.
  Impact: confirms the layering this plan assumes. `thysalion-world` stays free
  of both `bevy` and `bevy_voxel_world`; roadmap step 2.1.1 wires the delegate
  in `thysalion-presentation`. It also warns against a direct dependency on
  `block-mesh` or `ndshape`, which `bevy_voxel_world` pulls in transitively and
  which have been dormant since 2024 and 2022 respectively.
- Observation: lille re-pushes every static `Block` record into its DBSP
  circuit on every tick, and records this as a known, unfixed cost; its block
  identifiers are also session-scoped and explicitly not stable across saves or
  reloads.
  Evidence: `docs/lille-isometric-tiled-maps-design.md` §5.3 and §7;
  `src/map/translate.rs` allocates identifiers from a `Local<i64>` counter.
  Impact: forward-facing, not blocking. Thysalion's design §12.2 requires a
  stable 64-bit `SimId` shared across all three stores, and §10.6 bounds trace
  growth, so scene-derived voxel and spawn identity must be a deterministic
  function of the document rather than of load order. This step therefore
  derives nothing from load order: palette indices, chunk coordinates, and
  spawn ordinals are all document-determined. Roadmap step 4.1.1 inherits the
  one-shot-versus-restream decision.

## Decision log

- Decision: the scene format stays inside `thysalion-world` as a `scene` module
  tree; no leaf `thysalion-scene` crate is created.
  Rationale: this resolves the open question ADR 005 forwarded to step 1.2. The
  concept IRI is a syntactic string newtype, so it creates no knowledge-plane
  edge and therefore no cycle to break. `thysalion-world` is already the
  designated dependency sink, and a separate crate would add a boundary without
  removing one. The split is cheap later: the recorded reversal trigger is the
  first content-tooling binary outwith this workspace needing the format, or
  `thysalion-world` acquiring `bevy` (phase 2 or 4) while the format must stay
  engine-free. Recorded in ADR 006.
  Date/Author: 2026-07-26, plan author.
- Decision: the scene document is all-integer. Angles are centi-degrees
  (`i32`), lengths are millimetres (`u32`), colours are 8-bit channels, light
  emission is the design's 0–15 scale, and simulation coefficients use the
  16-bit fixed-point representation design §10.5 already mandates for the
  material fields.
  Rationale: it makes the lossless round-trip claim of task 1.2.1 true by
  construction rather than by tolerance — no float formatting, no NaN class, no
  f32-versus-f64 asymmetry between JSON and MessagePack — and it keeps
  `crates/world` free of the `clippy::float_arithmetic` allowance ADR 005
  withholds from the world crate. Cost: authoring writes `1745` where it means
  17.45 degrees, mitigated by unit-suffixed field names.
  Date/Author: 2026-07-26, plan author.
- Decision: `slope_dir` is a closed `SlopeDirection` enum (`Flat`, `PlusX`,
  `MinusX`, `PlusY`, `MinusY`) rather than design §7.2's `IVec2`.
  Rationale: ramps and stairs rise in one of four compass directions; an
  `IVec2` admits meaningless values such as `(7, -3)` that validation would then
  have to reject anyway. It also removes a `glam` dependency, which matters
  because Bevy 0.19 pins `glam` 0.32.1 while the registry is at 0.33.2, so a
  direct pin would risk two `glam` versions in the graph. Note that avoiding
  `glam` is *not* about serialization: `glam`'s `serde` implementations do not
  branch on `is_human_readable` and would have round-tripped correctly. The
  design document is amended in Stage D to match.
  Date/Author: 2026-07-26, plan author.
- Decision: the voxel payload is keyed by chunk, and a chunk that is entirely
  one voxel type is elided to a uniform token. Runs are chunk-local, in
  chunk-local Z-major order.
  Rationale: it makes encoding, decoding, hashing, and diffing all scale with
  populated volume instead of declared extent, it aligns the wire format with
  the sparse in-memory grid so decoding is a direct transcription, and it keeps
  an edit's diff confined to the chunk it touched. A global Z-major run stream
  fragments on every raster row of a localized fixture — see `Risks` for the
  arithmetic. The uniform-chunk elision is Minecraft Anvil's single-block-state
  optimization; the ordered-palette-plus-explicit-index-formula shape is
  Sponge Schematic v3's. Departs from design §7.3's literal wording, which
  Stage D corrects.
  Date/Author: 2026-07-28, plan author, after the scaling analysis.
- Decision: runs are explicit `(count, index)` structure. There are no in-band
  escape values, and no palette index carries a reserved meaning beyond index
  zero being air.
  Rationale: Qubicle Binary encodes runs with escape sequences built from
  reserved colour values, which collide with real data and produce exactly the
  corruption class that cannot be diagnosed after the fact. Explicitness costs
  bytes and buys diagnosability.
  Date/Author: 2026-07-28, plan author.
- Decision: loading is two-phase. Phase one deserializes and fails fast on the
  first structural error, located by `serde_path_to_error`. Phase two validates
  the deserialized structure and accumulates *every* semantic violation before
  failing.
  Rationale: `serde` cannot continue past a type mismatch, so "report every
  problem at once" is only achievable after deserialization succeeds. Splitting
  the phases makes that honest rather than aspirational, and phase two is
  encoding-agnostic — the same validator serves JSON and MessagePack, degrading
  from source spans to structural paths for the binary encoding, which carries
  no line or column information.
  Date/Author: 2026-07-28, plan author.
- Decision: diagnostics are `thiserror` variants decorated with `miette`'s
  `Diagnostic` derive, accumulated through a `#[related]` collection on one
  top-level error.
  Rationale: `thiserror` gives one variant per corruption class, which is what
  makes the roadmap's "each produces a distinct diagnostic" mechanically
  checkable. `miette` supplies the diagnostic codes, help text, source spans,
  and the `#[related]` accumulation that renders many problems as one report.
  `ariadne` was the alternative and renders comparably, but its GitHub
  repository is archived with development moved elsewhere, and `miette`'s
  `Diagnostic` is a trait that becomes part of the public error surface rather
  than merely a renderer.
  Date/Author: 2026-07-28, plan author.
- Decision: the document carries a monotonic integer `version` as its first
  field, read by a minimal probe structure before the full deserialization is
  attempted, and every document type is `#[serde(deny_unknown_fields)]`.
  Rationale: without the probe, a future document fails against today's
  structure with a confusing field-level error instead of "unsupported version
  2; this build supports 1". `deny_unknown_fields` turns a typo in a
  hand-authored fixture into an error rather than a silently ignored field, and
  it works on both encodings precisely because MessagePack is written as maps.
  A plain integer rather than a semantic version, because a file format has one
  axis of change and semantic versioning only invites arguments about which
  component to bump.
  Date/Author: 2026-07-28, plan author.
- Decision: `schemars` emits a JSON Schema for the document, committed to the
  repository and regenerated by the same target that builds the fixtures; a test
  fails if the committed schema is stale. `schemars` is pinned to an exact
  version.
  Rationale: it gives editors autocompletion and validation over hand-authored
  scenes, and — more valuably — it turns any accidental change to the wire
  format into a visible diff in review. `schemars` documents that its emitted
  structure may change between versions without that being a breaking change,
  hence the exact pin; a schema diff after a dependency bump is expected noise,
  not a format change. The schema is derived and never normative: it cannot
  express the semantic invariants phase two checks.
  Date/Author: 2026-07-28, plan author.
- Decision: no `SceneLibrary` registry type is introduced. The roadmap's "leave
  no scene state behind" is satisfied structurally.
  Rationale: an earlier draft invented a registry so that a behavioural step
  could assert it was empty after a failed load. That is a type existing only to
  make a sentence assertable. At this step the loader is a pure function of
  bytes and a `SceneSource`, so no state exists to leave behind; the guarantee
  is that `Scene` has no public constructor and validation is the only path to
  one, pinned by a `trybuild` case. The behavioural scenario asserts the honest
  observable instead: a failed load does not poison a subsequent good one. A
  registry becomes real in phase 2, in the Entity Component System layer.
  Date/Author: 2026-07-28, plan author.
- Decision: the operator tool is `crates/world/examples/scene-check.rs`, a Cargo
  example, not a `[[bin]]`.
  Rationale: examples are built and linted by the workspace gate
  (`--all-targets`), never enter the release graph (which is scoped
  `-p thysalion`), and make no claim on ADR 005's rule that demo binaries live
  only in `thysalion-demos` or on the `make demo` allow-list derived from
  `crates/demos/src/bin`. The alternative — a `[[bin]]` in `thysalion-world` —
  is more discoverable but muddies both rules for no gain.
  Date/Author: 2026-07-28, plan author.
- Decision: palette index `0` is reserved for air in every scene.
  Rationale: it gives run-length encoding and sparse chunk storage a shared
  meaning for "nothing here", so an absent chunk and a long air run agree
  without a lookup. Validation rejects a palette whose entry 0 is not the air
  material class.
  Date/Author: 2026-07-26, plan author.
- Decision: entity spawns carry a closed, typed field set rather than an
  untyped component bag.
  Rationale: `serde_json::Value` is precisely the type whose behaviour differs
  between the two encodings, and there is no Entity Component System component
  vocabulary to author against until phase 4. A `#[non_exhaustive]` typed
  struct defers the vocabulary to the phase that owns it without weakening the
  round-trip guarantee.
  Date/Author: 2026-07-26, plan author.
- Decision: the behavioural suite for scene loading uses a new
  `rstest-bdd` `HarnessAdapter` whose context is a plain loader session, not the
  existing `BevyHarness` in `crates/harness/tests/headless/support.rs`.
  Rationale: `thysalion-world` has no Bevy dependency and ADR 005 stages Bevy
  into it later; adding `bevy` now purely to host a test harness would invert
  the staging and put the render feature set into the world crate's graph. The
  new adapter is deliberately structurally identical to `BevyHarness` so that
  roadmap step 1.3.1 can promote both into one shared test-support crate, which
  the developers' guide already names as the promotion point. The alternative —
  hosting scene-loading scenarios in `crates/harness` against a `MinimalPlugins`
  app — is available and is the right move once step 2.1.1 gives the scene an
  Entity Component System representation to assert against.
  Date/Author: 2026-07-26, plan author.

## Outcomes & retrospective

To be completed at the end of Stage D.

## Context and orientation

Read this section if you have never seen this repository.

### What exists today

The repository is a non-virtual Cargo workspace. The root package `thysalion`
is the eventual game binary and the composition root. Members live under
`crates/`:

- `crates/world/` — `thysalion-world`, the state plane. **This is the crate
  this step fills.** It currently contains only `Cargo.toml` (no dependencies)
  and a `src/lib.rs` of module documentation and no code.
- `crates/sim/`, `crates/knowledge/` — the logic and knowledge planes, likewise
  empty skeletons awaiting phases 4 and 5.
- `crates/presentation/` — the presentation plane, currently the pure camera
  contract in `crates/presentation/src/camera/mod.rs` (`Quadrant`,
  `ZoomBounds`).
- `crates/harness/` — `thysalion-harness`, the demo scaffolding delivered by
  roadmap step 1.1: `HarnessCorePlugin` (headless-safe) and `DemoHarnessPlugin`
  (windowed).
- `crates/demos/` — one binary per capability demonstration, currently
  `demo-empty`.

`[workspace.dependencies]` in the root `Cargo.toml` is the single source of
truth for every version literal. Today it pins `bevy` 0.19 (with a curated
feature list), `cap-std` 3, `camino` 1, `rstest` 0.26, `proptest` 1, `trybuild`
1, and the `rstest-bdd` trio at 0.6.0-beta3.

`[workspace.lints]` holds a strict Clippy table. The denials that shape this
step's code are listed under `Constraints` above.

### Terms of art used in this plan

- **Voxel** — one cubic cell of the world grid, roughly 10 cm on a side
  (design §1).
- **Palette** — the ordered list of voxel types belonging to one scene. A voxel
  in the grid stores a 16-bit index into it (design §7.2).
- **Chunk** — a 32 x 32 x 32 block of voxels. Chunks exist for meshing
  granularity and edit locality, not streaming; all of a scene's chunks are
  resident while the scene is loaded (design §7.1).
- **Run-length encoding (RLE)** — storing a repeated value once with a count,
  instead of once per position. Large volumes of air cost almost nothing.
- **MessagePack** — a compact binary encoding with a data model close enough to
  JSON that one Rust structure can serialize to both. Used as the shipping
  encoding while JSON stays the authoring and diffing encoding (design §7.3).
- **Data Transfer Object (DTO)** — a type whose only job is to mirror a wire
  format. Here, `SceneDocument`. It is deliberately more permissive than the
  validated domain type it becomes.
- **Port and adapter** — a port is an interface the domain owns; an adapter is
  an implementation that connects it to the outside world. Here the domain owns
  `SceneSource` (give me the bytes at this scene-relative path) and adapters
  supply it from a capability-scoped directory or from memory.
- **Internationalized Resource Identifier (IRI)** — the Resource Description
  Framework's identifier form, a Uniform Resource Identifier generalized to
  Unicode. A voxel type may name the ontology concept it instantiates, for
  example `thy:OakDoor` (design §7.2).
- **TriG** — a text serialization for Resource Description Framework datasets
  with named graphs. Scenes reference TriG files for the knowledge plane to load
  (design §7.3, §11.5).
- **Behaviour-driven development (BDD)** — specifying behaviour as
  `Given`/`When`/`Then` scenarios in a `.feature` file, executed by step
  functions. This repository uses `rstest-bdd` 0.6.0-beta3.

### The design's specification of the format

Design §7.2 fixes what a voxel type carries: a name, a material class, per-face
passability, a slope direction, light emission, simulation coefficients, and an
optional concept IRI. Design §7.3 fixes the document's sections: `dimensions`,
`chunk_size`, `palette`, `voxels` (palettized dense Z-major layers, run-length
encoded), `entities` (a spawn list with prototype inheritance via `extends` plus
overrides), `lighting` (sun path, ambient palette per time-of-day band, probe
volume bounds and spacing overrides), and `knowledge` (TriG files plus the
scene's own named-graph IRI). Design §7.1, Table 1 fixes the scene size classes.

The concept art in `references/style-guide.png` fixes the vocabulary the fixture
palettes must express: material families (stone, timber, roofing, cloth and
banners, ground, natural), ground materials (cobblestone, wet cobblestone, wood
planks, mud and dirt, grass and moss, forest path, swamp boardwalk, snow and
ice), light sources (lantern, torch, fireplace, candle, magic, moonlight), and
seven named colour palettes of which three are relevant here: Market Town,
Keep Interior, and Haunted Swamp.

## Plan of work

### Stage A: verification spikes and dependency pinning (no tracked changes)

Nothing in this stage is committed. Its purpose is to retire the two questions
whose answers change the design, before any production code exists.

Spike A1 — round-trip parity and wire shape. In a scratch directory outwith the
repository, build a throwaway crate with `serde`, `serde_json`, `rmp-serde`, and
`rmpv`, containing a struct that exercises every shape the scene document will
use: nested structs, a `#[non_exhaustive]` enum with unit and struct variants,
`Option`, `BTreeMap`, `Vec`, `SmolStr`, and both signed and unsigned integers of
every width the format uses. Confirm three things:

1. Encoding to JSON and to MessagePack and decoding back yields an equal value
   in both directions.
2. `rmp_serde::to_vec_named` produces maps and `rmp_serde::to_vec` produces
   arrays, by decoding each into an `rmpv::Value` and inspecting it. This is the
   assertion the production test will mirror.
3. `#[serde(deny_unknown_fields)]` rejects an extra field under both encodings,
   and a `#[serde(default)]` field is accepted when absent.

Go/no-go: if any shape fails to round-trip, remove it from the format before
Stage B rather than working around it later.

Spike A2 — fixture storage and load cost. Write a scratch program that builds
the chunk-keyed payload for a 1024 x 1024 x 128 volume that is air except for a
populated fragment of roughly sixty-four chunks, encodes it as JSON and as
MessagePack, and reports both sizes, the decoded sparse memory footprint, and
the wall-clock time to parse, decode, and traverse it. Measure with `hyperfine`.
Compare against the fixture-size and load-time tolerances.

Go/no-go: if the wilderness class cannot be expressed within tolerance, escalate
with the measured numbers and propose reduced fixture extents rather than
quietly shipping a slow fixture.

Spike A3 — `serde_path_to_error` under MessagePack. The crate documents that it
wraps any `Deserializer`, but every published example is JSON and binary formats
are not covered by its tests. Confirm on a deliberately malformed MessagePack
payload that `Error::path()` yields a usable structural path. If it does not,
the MessagePack path degrades to an unlocated deserialization error, which is
acceptable — MessagePack is the shipping encoding, not the authoring one — but
the plan must say so rather than promise a path it cannot deliver.

Pinning. Add to `[workspace.dependencies]` in the root `Cargo.toml` exactly the
approved set from `Tolerances`, at the versions verified during this stage, with
caret requirements — except `schemars`, which is pinned to an exact version for
the reason in the decision log. Record each resolved version in `Artefacts and
notes`.

### Stage B: the behavioural specification and the first red tests

Write tests before production code and observe each failing for the intended
reason. Nothing is committed while red.

Only the tests for task 1.2.1 are written in this stage. Stages C2 and C3 each
open with their own red step, so that the three roadmap tasks stay separately
committable and each commit passes `make all` — AGENTS.md forbids committing a
red state, and writing every test upfront would make the first two commits
impossible.

The behavioural specification is written in full now, because it is the
document a reviewer reads to understand the whole step, but only the scenarios
belonging to task 1.2.1 are wired to `#[scenario]` functions at this stage.
Create
`crates/world/tests/features/scene_loading.feature`:

```gherkin
Feature: Scene loading and validation

  Scenario: A well-formed scene loads
    Given the minimal hand-written scene document
    When the scene is loaded
    Then loading succeeds
    And the scene reports 4 palette entries
    And the scene reports 64 non-air voxels

  Scenario: The same scene survives a MessagePack round trip
    Given the minimal hand-written scene document
    When the document is re-encoded as MessagePack and loaded
    Then loading succeeds
    And the loaded scene equals the scene loaded from JSON

  Scenario: An unknown palette index is rejected
    Given the scene document with an out-of-range palette index
    When the scene is loaded
    Then loading fails
    And the diagnostics name the unknown palette index
    And the diagnostics locate it by chunk and chunk-local position

  Scenario: An out-of-bounds spawn is rejected
    Given the scene document with a spawn outwith the grid
    When the scene is loaded
    Then loading fails
    And the diagnostics name the out-of-bounds spawn

  Scenario: A dangling knowledge IRI is rejected
    Given the scene document naming a missing TriG file
    When the scene is loaded
    Then loading fails
    And the diagnostics name the missing knowledge resource

  Scenario: Every problem in a scene is reported at once
    Given the scene document with three independent faults
    When the scene is loaded
    Then loading fails
    And the diagnostics list 3 problems

  Scenario: A failed load leaves the loader usable
    Given the scene document with an out-of-range palette index
    When the scene is loaded
    And the minimal hand-written scene document is loaded afterwards
    Then loading succeeds

  Scenario: Every fixture scene loads clean
    Given the fixture scene "<name>"
    When the scene is loaded
    Then loading succeeds

    Examples:
      | name              |
      | keep-interior     |
      | market-town-block |
      | swamp-fragment    |
```

Create `crates/world/tests/loading/support.rs` holding the adapter, and
`crates/world/tests/loading/main.rs` holding the step functions, mirroring the
structure of `crates/harness/tests/headless/{support,main}.rs`. The adapter's
context is a `LoaderSession` — a `MemorySceneSource` pre-populated with the
fixture documents, plus the outcome of the most recent load.

A note on the last scenario, because it replaced an earlier and worse idea. The
roadmap phrases 1.2.2's criterion as "leave no scene state behind", which
invites inventing a registry type purely so a test can assert it is empty. That
would be a fiction: at this step the loader is a pure function of bytes and a
`SceneSource`, so there is no state to leave behind, and the all-or-nothing
guarantee is structural — `Scene` has no public constructor and validation is
the only path to one, which the `trybuild` case pins. The honest observable is
that a failed load does not poison a subsequent good one, which is what the
scenario asserts. When a registry does become real — phase 2, in the Entity
Component System layer, not here — the assertion can strengthen.

Alongside the behavioural suite, add the task 1.2.1 unit and property tests:

- `rstest` cases for the chunk-local index mapping, palette lookup, and the
  uniform-chunk elision round trip.
- `proptest` properties: chunk-local run encoding and decoding is a fixpoint
  over random chunks; the chunk-local linear index and the `(x, y, z)` triple
  are a bijection; and — the guard against the human-readability trap — a
  generated `SceneDocument` survives `JSON -> document -> MessagePack ->
  document` unchanged. Bound the generators: at most eight chunks per document,
  at most sixty-four runs per chunk, and at most sixteen palette entries.
  Without bounds the generator's cost is superlinear in extent and the property
  becomes the slowest thing in continuous integration.
- A wire-shape test that encodes a document with `to_vec_named`, decodes it into
  an `rmpv::Value`, and asserts the top level is a map whose keys are the
  document's field names. This is what actually pins the struct-as-map choice;
  a round-trip test alone would pass under either encoding.
- A canonical-bytes test asserting that encoding the same document twice yields
  identical bytes, and that a document round-tripped through JSON re-encodes to
  the same MessagePack bytes. This is the property design §12.3's save-archive
  content hashes will depend on.
- A `trybuild` compile-fail case under `crates/world/tests/ui/` proving that
  `Scene` cannot be constructed by struct literal, so validation cannot be
  bypassed.

Here is the minimal hand-written scene the first scenario loads. It is the
worked example a reader needs in order to understand every later section, and
it is checked in at `crates/world/tests/fixtures/minimal.scene.json`. It
declares one chunk of extent, a four-entry palette, a single populated chunk
whose lower 4 x 4 x 4 corner is stone, one spawn, and one knowledge resource.

```json
{
  "version": 1,
  "name": "minimal",
  "dimensions": { "x": 32, "y": 32, "z": 32 },
  "chunk_size": 32,
  "palette": [
    {
      "name": "air",
      "material": "air",
      "passable": {
        "pos_x": true, "neg_x": true, "pos_y": true,
        "neg_y": true, "pos_z": true, "neg_z": true
      }
    },
    { "name": "stone-block", "material": "stone", "concept": "thy:StoneBlock" },
    { "name": "oak-plank", "material": "timber", "concept": "thy:OakPlank" },
    {
      "name": "wall-sconce",
      "material": "stone",
      "emission": { "intensity": 12, "colour": [255, 180, 90] },
      "concept": "thy:WallSconce"
    }
  ],
  "voxels": [
    {
      "at": { "x": 0, "y": 0, "z": 0 },
      "runs": [
        [4, 1], [28, 0], [4, 1], [28, 0],
        [4, 1], [28, 0], [4, 1], [32668, 0]
      ]
    }
  ],
  "entities": {
    "prototypes": {},
    "spawns": [ { "name": "party-start", "at": { "x": 2, "y": 2, "z": 4 } } ]
  },
  "lighting": {
    "sun_path": {
      "azimuth_centidegrees": 13500,
      "elevation_centidegrees": 3000
    },
    "ambient_bands": [],
    "probe_spacing_mm": 2000
  },
  "knowledge": { "graph": "thy:scene/minimal", "sources": ["minimal.trig"] }
}
```

Every field absent from a palette entry takes its documented default: fully
impassable, flat, inert, and with no concept. The `runs` array is chunk-local
and Z-major: within a chunk of side `s`, the voxel at chunk-local `(x, y, z)`
sits at index `z * s * s + y * s + x`, which for `s = 32` is
`z * 1024 + y * 32 + x`. Read the eight runs as four rows of stone, each four
voxels of stone followed by twenty-eight of air, at `y = 0`, `1`, `2`, `3` on
the `z = 0` layer, and then air for the remaining 32,668 positions. The counts
sum to 32,768, the chunk volume, and validation rejects any chunk whose runs do
not. Runs are also canonical: two adjacent runs may not share a palette index,
because a non-unique encoding would make the content hashes design §12.3
depends on unstable. Validation rejects the non-canonical form rather than
silently normalizing it, so a generator bug is visible rather than absorbed.

Validation for this stage: `cargo test -p thysalion-world` fails, and every
failure names a missing item or an unmet assertion in the new code, not a
compilation error in unrelated crates.

### Stage C1: the voxel type registry and scene document model (task 1.2.1)

Create the module tree under `crates/world/src/`. Keep each file under 400
lines; the split below is chosen to make that true.

- `scene/mod.rs` — module documentation, re-exports, and the validated `Scene`
  aggregate.
- `scene/extent.rs` — `Extent`, `ChunkSize`, `VoxelPos`, `ChunkCoord`, and the
  chunk-local index mapping given in Stage B. All extent arithmetic is checked:
  the declared volume is computed on `u64` with `checked_mul`, because
  1024 x 1024 x 128 already consumes 134 million of `u32`'s range and a
  malformed document may declare far more.
- `scene/voxel_type.rs` — `VoxelType`, `MaterialClass`, `Passability`,
  `SlopeDirection`, `LightEmission`, `SimProperties`.
- `scene/palette.rs` — `VoxelIndex` and `Palette`, with `VoxelIndex::AIR` and
  fallible lookup.
- `scene/concept.rs` — `ConceptIri` with syntactic and namespace validation.
- `scene/voxels.rs` — `VoxelRun`, `ChunkPayload` (uniform or runs), and the
  chunk-local run codec.
- `scene/grid.rs` — `VoxelGrid`, the decoded sparse store: a `BTreeMap` from
  `ChunkCoord` to a boxed chunk array, where an absent chunk is entirely air.
  Sorted rather than hashed, because iteration order feeds re-encoding and must
  be deterministic.
- `scene/entities.rs` — `PrototypeName`, `EntityPrototype`, `EntitySpawn`, and
  prototype-chain flattening.
- `scene/lighting.rs` — `LightingSection`, `SunPath`, `AmbientBand`,
  `ProbeVolume`.
- `scene/knowledge.rs` — `KnowledgeSection`.
- `scene/document.rs` — `SceneDocument` and `DocumentVersion`.

Then the codec and port modules:

- `codec/mod.rs` — `Encoding`, the `VersionProbe` structure, and the
  encode/decode entry points. Decoding always probes the version first and
  rejects an unsupported one with its own diagnostic before attempting the full
  deserialization.
- `codec/json.rs` — decodes through `serde_path_to_error` over
  `serde_json::Deserializer`, converting `serde_json::Error`'s line and column
  into a byte offset so `miette` can label the source. Encodes with
  `serde_json::to_string_pretty` so authored documents stay diffable.
- `codec/msgpack.rs` — encodes with `rmp_serde::to_vec_named` and never with
  `to_vec`. Decodes through `serde_path_to_error` if Spike A3 confirmed that
  works, and plainly otherwise.
- `source/mod.rs` — the `SceneSource` port.
- `source/cap_fs.rs` — `DirSceneSource`, wrapping `cap_std::fs_utf8::Dir`.
- `source/memory.rs` — `MemorySceneSource`, available to tests.

Every document type carries `#[serde(deny_unknown_fields)]`,
`#[derive(schemars::JsonSchema)]`, and — on the public domain types that
consumers will match or construct — `#[non_exhaustive]`.

Validation for this stage: the round-trip property, the wire-shape assertion,
the canonical-bytes assertion, and the codec unit tests all go green. Commit
task 1.2.1 here, with `make all` passing.

### Stage C2: scene loading with load-time validation (task 1.2.2)

Open with the red step: wire the remaining behavioural scenarios to
`#[scenario]` functions, add one corrupt fixture per corruption class under
`crates/world/tests/fixtures/corrupt/`, and add the `insta` snapshot assertions
over the rendered diagnostic report — one snapshot per class. The snapshot
captures report text with paths normalized, so it is a contract about diagnostic
wording, ordering, and codes rather than a dump of the document. Observe them
failing, then implement:

- `scene/validation/diagnostics.rs` — `SceneDiagnostic`, a `thiserror` enum
  decorated with `miette`'s `Diagnostic` derive, one variant per corruption
  class, each with its own stable `#[diagnostic(code(...))]`; and
  `SceneLoadError`, which holds the accumulated diagnostics in a
  `#[related]` collection behind an `Arc` so the error stays small enough for
  `clippy::result_large_err`. Check the naming against
  `clippy::error_impl_error`, which this workspace denies.
- `scene/validation/rules.rs` — the semantic checks. Rules that must walk the
  voxel payload are fused into a single traversal per chunk rather than one
  traversal per rule; rules over the palette, entities, lighting, and knowledge
  sections are separate and cheap. Each returns the diagnostics it found rather
  than short-circuiting.
- `scene/validation/mod.rs` — the orchestration: run every rule, and construct a
  `Scene` only if the diagnostic collection is empty.
- `loader.rs` — `SceneLoader<S: SceneSource>` with `load` and `load_bytes`. It
  holds no mutable state: the all-or-nothing guarantee is that `Scene` has no
  public constructor and validation is the only path to one. No registry type
  is introduced here; that belongs to the Entity Component System layer in
  phase 2.

Every diagnostic must locate its subject in terms an author can act on. "Voxel
run 41,203 is invalid" is useless in a 134-million-voxel scene; the report must
name the chunk coordinate and the chunk-local position, and for JSON input also
the structural path and source span. This is a requirement on the diagnostic
variants' fields, not a formatting nicety, and the `insta` snapshots pin it.

The corruption classes, each with its own diagnostic variant, its own
`miette` code, and its own corrupt fixture:

unsupported document version; zero, overflowing, or non-chunk-aligned
dimensions; chunk size not the design's 32; palette entry zero not air; empty
palette; duplicate voxel-type name; palette longer than 65,536 entries; chunk
coordinate outwith the declared extent; duplicate chunk coordinate; voxel run
referencing an index outwith the palette; run counts not summing to the chunk
volume; adjacent runs sharing an index (non-canonical); a run of count zero;
light emission outwith 0–15; spawn position outwith the grid; unknown prototype
name; prototype inheritance cycle; prototype chain deeper than the stated
limit; concept IRI outwith the project namespaces; malformed IRI; knowledge
TriG resource not present in the source; scene named-graph IRI missing or
malformed.

Prototype resolution is iterative with an explicit depth bound, never
recursive, so a hostile or mistaken document cannot overflow the stack.
Likewise, a declared extent is checked against the chunk-size and volume bounds
*before* any allocation proportional to it, so a document declaring an absurd
extent is rejected rather than exhausting memory.

Also add `crates/world/examples/scene-check.rs`, which loads a scene through
`DirSceneSource` and prints either `<name>: ok` or the rendered diagnostic
report, exiting non-zero on failure. It is the human-observable surface of this
step and the thing the fixture generator's tests shell out to.

Validation for this stage: the whole behavioural suite goes green except the
fixture-scene scenarios; `insta` snapshots are reviewed and accepted. Commit
task 1.2.2 here, with `make all` passing.

### Stage C3: the fixture scenes and their generator (task 1.2.3)

Fixtures are authored as layered text and compiled to JSON.

Authoring sources live under `assets/scenes/src/<name>/`: a `scene.toml`
declaring dimensions, chunk size, palette, lighting, entities, and knowledge;
`legend.toml` mapping single characters to palette names; and `layers/z###.txt`,
one text raster per populated Z layer, with unlisted layers implicitly air.

`scripts/build_fixture_scenes.py` compiles those sources to
`assets/scenes/<name>.scene.json`, emitting chunk-keyed payloads with uniform
chunks elided and chunk entries sorted by coordinate. It follows the scripting
standards: an inline `uv` metadata block, Cyclopts for the interface, and pytest
tests under `scripts/tests/`.

A `make scenes` target runs the generator and regenerates the committed JSON
Schema. Two tests guard the result, and both are load-bearing:

1. A staleness test regenerates every fixture into a temporary directory and
   asserts the output matches the committed JSON byte for byte. A hand-edited
   fixture, or a generator change nobody re-ran, fails the gate. Without this
   the authoring sources and the fixtures drift apart silently and the sources
   quietly become decoration.
2. A schema-staleness test does the same for the committed JSON Schema.

The generator is deliberately not a build script: fixtures are committed
artefacts so that a contributor who has no `uv` can still build, test, and run
demos. `make scenes` is a maintenance target, not a build step.

The three fixtures, sized per design §7.1, Table 1 and dressed from the style
guide's palettes:

- `keep-interior` — Interior class, 128 x 128 x 64. Stone walls, timber floors
  and stairs, an arched doorway, a hearth and wall sconces as emissive voxel
  types, and an upper storey so phase 2's cut-away has something to remove.
  Palette band: Keep Interior (rich and regal); torchlit evening lighting.
- `market-town-block` — Town district class, 512 x 512 x 96, of which one
  populated block. Cobblestone ground, timber-framed façades, tiled roofs, a
  well, stalls, and banners. Palette band: Market Town (warm and inviting);
  late-afternoon sun path.
- `swamp-fragment` — Wilderness class, 1024 x 1024 x 128, of which one populated
  fragment. Mud, water, a plank boardwalk, moss, and bioluminescent emissive
  voxels. Palette band: Haunted Swamp (eerie and desaturated); twilight sun path
  with a fog volume declared for phase 3.

Each fixture also declares a `knowledge` section naming a small TriG file under
`assets/scenes/knowledge/` and the scene's own named-graph IRI, so the dangling
IRI check has something real to succeed against. The TriG files are not parsed
at this step — only their presence is checked — and phase 5 takes over their
content.

Two things are deliberately **not** in scope here, and are recorded so a later
reader does not mistake their absence for an oversight:

- **Semantic plausibility.** A scene can load clean and still be nonsense —
  buildings floating over voids, a spawn embedded in solid rock, a room with no
  door. Checking that needs judgement about passable-but-solid materials such
  as water and about what "reachable" means, and it belongs to a scene lint
  built after phase 4's pathfinding exists to define reachability. This step
  validates document integrity, not level design. The distinction is stated in
  the world-plane architecture document so the boundary is deliberate.
- **MagicaVoxel `.vox` import.** Design §7.4 promises it, and `dot_vox` 5.2.0 is
  maintained and would make it cheap. It is deferred because `.vox` caps a model
  at 255 palette colours and 256 voxels per axis, so it can never be the
  canonical authoring input for a 1024-extent scene with a 16-bit palette — it
  is a prop-and-stamp importer, which is what design §7.4 actually says. The
  natural home is a later content-tooling step, and the format this step ships
  is a viable target for it.

Validation for this stage: `make scenes` is idempotent, the fixture scenarios go
green, and `cargo run -p thysalion-world --example scene-check` reports `ok` for
all three. Record the measured JSON size, decoded footprint, and load time for
each fixture in `Artefacts and notes`, against the tolerances. Commit task 1.2.3
here, with `make all` passing.

### Stage D: documentation, ADR 006, refactor, and cleanup

- Write `docs/adr-006-scene-document-model.md` recording: the format staying in
  `thysalion-world` with its reversal trigger (resolving ADR 005's forwarded
  open question); the two-stage DTO-to-domain shape as the mechanism for
  all-or-nothing loading; the all-integer document; named-field MessagePack;
  reserved air index; sparse chunk storage; and accumulate-then-fail
  diagnostics. Reference it from ADR 005's open question and from the design
  document §7.
- Amend `docs/thysalion-design.md` §7.2 and §7.3 where this plan departs from
  them: `slope_dir` becomes `SlopeDirection`; units become integer; entity
  spawns are typed; palette index 0 is air; the voxel payload becomes
  chunk-keyed with uniform elision rather than "dense Z-major layers,
  run-length encoded"; a `version` field is added to the section list; and the
  appeal to lille's "proven format" is corrected to name it as a specification
  and to say which parts were taken and which were improved on (see
  `Surprises & discoveries`).
- Add `docs/world-plane-architecture.md` documenting the internally facing
  interfaces of `thysalion-world`: the scene module tree, the `SceneSource`
  port and its adapters, the loader and library, and the diagnostic
  vocabulary — with the field-by-field scene format reference and a worked
  minimal example. Index it in `docs/contents.md`.
- Update `docs/repository-layout.md` for `assets/`, `crates/world/src/bin/`, and
  the fixture generator script.
- Update `docs/developers-guide.md` with a "Scene fixtures" section: where
  fixtures live, how to author a layer, how to regenerate, and how to read a
  diagnostic report.
- Update `docs/users-guide.md` with `make scenes` and the `scene-check` example.
- Add a scene-format version-history table to the world-plane architecture
  document, starting at version 1, stating what each future bump would mean.
  Minecraft's 1.15-to-1.16 change to index packing broke every third-party tool
  precisely because the change was invisible in the data and unrecorded.
- Refactor pass against the AGENTS.md heuristics, then rerun the full gate.

## Concrete steps

Run everything from the repository root,
`/data/leynos/Projects/thysalion.worktrees/1-2-deliver-the-scene-format-and-fixture-scenes`.

```bash
git branch --show-current          # expect 1-2-deliver-the-scene-format-and-fixture-scenes
make all                           # expect a clean baseline before any edit
```

Per-stage focused commands:

```bash
cargo test -p thysalion-world                       # unit, property, and snapshot tests
cargo test -p thysalion-world --test loading        # the behavioural suite
cargo insta review                                  # accept diagnostic snapshots
make scenes                                         # regenerate fixture JSON and the schema
cargo run --release -p thysalion-world --example scene-check -- assets/scenes/keep-interior.scene.json
hyperfine 'cargo run --release -p thysalion-world --example scene-check -- assets/scenes/swamp-fragment.scene.json'
```

Expected transcript for the `scene-check` run:

```plaintext
keep-interior: ok
```

And for a corrupt fixture — note that the report names the chunk and the
chunk-local position, not a run ordinal:

```plaintext
crates/world/tests/fixtures/corrupt/unknown-palette-index.scene.json: 1 problem

  thysalion::scene::unknown_palette_index

  x chunk (0, 0, 0) at local (4, 0, 0) references palette index 9, but the
  | palette has 4 entries
   ,-[minimal.scene.json:14:48]
14 |     { "at": { "x": 0, "y": 0, "z": 0 }, "runs": [[4, 9], ...
   :                                                     ^
   `----
  help: palette indices run from 0 to 3; index 0 is always air
```

The commit gate, before every commit:

```bash
make all
make markdownlint      # documentation changes only
make nixie             # documentation changes only
```

## Validation and acceptance

Acceptance is behavioural, and maps one-to-one onto the roadmap's success
criteria.

Task 1.2.1 — "a hand-written JSON scene round-trips through the model and the
MessagePack encoding without loss". Red: the round-trip property test fails
because `SceneDocument` does not exist. Green: it passes for the checked-in
hand-written minimal scene and for every generated document `proptest` produces.
Observable: `cargo test -p thysalion-world round_trip` reports the property
passing over its full case budget.

Task 1.2.2 — "corrupt fixture variants each produce a distinct diagnostic and
leave no scene state behind". Red: each corrupt-fixture scenario fails because
the loader does not exist. Green: each corrupt fixture yields its own named
diagnostic, the accepted `insta` snapshots pin the report wording, the
multi-fault fixture reports all three problems in one load, a failed load leaves
the loader able to load a good scene, and the `trybuild` case proves `Scene` is
unconstructible except through validation. Observable: running the
`scene-check` example against each corrupt fixture prints its distinct report,
naming the chunk and chunk-local position of the fault, and exits non-zero.

Task 1.2.3 — "all three load through the validator". Green: the three fixture
scenarios pass in the behavioural suite and `scene-check` reports `ok` for each,
each within the load-time tolerance and the fixture-size tolerance, with the
measurements recorded.
The roadmap's second clause — each fixture referenced by a demo or continuous
integration suite by the end of phase 6 — is satisfied for now by the
behavioural suite, and is a phase-6 obligation, not this step's.

The test stack is large, so here is why each layer is present and where it is
required rather than chosen. `rstest` and `rstest-bdd` are AGENTS.md's mandated
unit and behavioural frameworks. `proptest` is mandated where a change
"introduces an invariant over a range of inputs" — round-trip parity and codec
fixpoint are exactly that, and they are the only two properties here. `insta` is
mandated where "multivariant output format consistency is relevant to the
requirements", which is precisely the roadmap's "each produces a distinct
diagnostic". `trybuild` is the established precedent in this workspace for
pinning a compile-time contract, and the contract being pinned — that validation
cannot be bypassed — is load-bearing. The Python `pytest` suite is mandated by
the scripting standards for any helper script. Nothing here is optional, and
nothing beyond this is added: there is no `criterion` benchmark suite, no `kani`
or `verus` proof, and no mutation testing, because no invariant in this step
warrants that rigour. Load time is measured with `hyperfine` once per fixture
and recorded, not tracked continuously.

Quality criteria for "done":

- Tests: `make test` green, including doctests. The behavioural suite, the
  property tests, the snapshot tests, and the `trybuild` case all pass.
- Lint and formatting: `make lint` and `make check-fmt` green, with no new
  lint suppression in `crates/world`.
- Documentation: `make markdownlint` and `make nixie` green; every new public
  item carries Rustdoc; ADR 006 and the world-plane architecture document exist
  and are indexed in `docs/contents.md`.
- Spelling: `make spelling` green.
- Coverage: no new entry in the Makefile's coverage-ignore pattern. Everything
  this step adds is headless and must be measured.

## Idempotence and recovery

Every step is re-runnable. `make scenes` regenerates fixture JSON
deterministically from the tracked authoring sources, so a corrupted fixture is
recovered by regenerating it. `cargo insta review` is the only interactive step;
a rejected snapshot leaves the previous one in place. Nothing in this step
deletes tracked files or writes outwith the repository, and the Stage A spikes
happen in a scratch directory that is discarded.

If a stage's validation fails and five attempts do not resolve it, stop and
escalate per `Tolerances` rather than loosening a test or adding a suppression.

## Artefacts and notes

To be filled during implementation with the resolved dependency versions from
Stage A, the Spike A1 and A2 measurements, and the first passing transcripts.

## Interfaces and dependencies

These signatures must exist at the end of the step. Names are the contract;
bodies are not.

In `crates/world/src/scene/palette.rs`:

```rust
/// Index into a scene palette. Index zero is always air.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct VoxelIndex(u16);

impl VoxelIndex {
    /// The reserved air index present in every scene palette.
    pub const AIR: Self = Self(0);
}

/// The ordered voxel types belonging to one scene.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Palette { /* private */ }

impl Palette {
    /// Returns the voxel type at `index`, or `None` when it is out of range.
    pub fn get(&self, index: VoxelIndex) -> Option<&VoxelType>;
    /// Returns the number of entries, which is also the exclusive index bound.
    pub fn len(&self) -> usize;
}
```

In `crates/world/src/scene/voxels.rs`:

```rust
/// One run of identical voxels in a chunk's Z-major stream, encoded as the
/// two-element array `[count, index]` in both encodings.
///
/// The pair is frozen at two elements: `serde` encodes a tuple struct
/// positionally, so a third element could never be added compatibly. Anything
/// the payload needs to grow belongs on [`ChunkPayload`], not here.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct VoxelRun(u32, VoxelIndex);

/// A chunk's voxels: either one repeated type, or an explicit run stream.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields, rename_all = "kebab-case")]
#[non_exhaustive]
pub enum ChunkPayload {
    /// Every voxel in the chunk is this type; no run stream is stored.
    Uniform { voxel: VoxelIndex },
    /// Chunk-local Z-major runs, summing to the chunk volume, no two adjacent
    /// runs sharing an index.
    Runs { runs: Vec<VoxelRun> },
}

/// A scene's voxels, stored sparsely: an absent chunk is entirely air.
///
/// Backed by a `BTreeMap` so iteration order is deterministic, which is what
/// makes re-encoding byte-stable and therefore hashable (design §12.3).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VoxelGrid { /* private */ }

impl VoxelGrid {
    /// Returns the voxel at `pos`, or `None` when `pos` is outwith the extent.
    /// Positions inside the extent but in an unpopulated chunk are air.
    pub fn get(&self, pos: VoxelPos) -> Option<VoxelIndex>;
    /// Rebuilds the document's chunk entries, in chunk-coordinate order.
    /// Costs time proportional to populated chunks, never to declared extent.
    pub fn to_chunks(&self) -> Vec<ChunkEntry>;
}
```

In `crates/world/src/source/mod.rs`:

```rust
/// Driven port: resolves scene-relative resource paths to bytes.
///
/// The domain never touches the filesystem; adapters supply this.
pub trait SceneSource {
    /// Reads the resource at `path`, relative to the scene's own directory.
    fn read(&self, path: &Utf8Path) -> Result<Vec<u8>, SceneSourceError>;
    /// Reports whether the resource at `path` exists, without reading it.
    ///
    /// Returns `Err` when existence could not be determined — a permissions
    /// failure is not the same fact as a missing file, and reporting it as
    /// "your scene names a file that is not there" would send an author
    /// hunting for the wrong problem.
    fn has(&self, path: &Utf8Path) -> Result<bool, SceneSourceError>;
}
```

In `crates/world/src/loader.rs`:

```rust
/// Loads and validates scenes through an injected [`SceneSource`].
pub struct SceneLoader<S: SceneSource> { /* private */ }

impl<S: SceneSource> SceneLoader<S> {
    /// Loads the scene document at `path`, inferring the encoding from its
    /// extension, and validates it. Returns every diagnostic on failure and
    /// no partially constructed scene.
    pub fn load(&self, path: &Utf8Path) -> Result<Scene, SceneLoadError>;
}
```

In `crates/world/src/scene/validation/diagnostics.rs`:

```rust
/// One named problem found while validating a scene document.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[non_exhaustive]
pub enum SceneDiagnostic { /* one variant per corruption class */ }

/// Every problem found in one load. Carries its diagnostics behind an `Arc`
/// so the error type stays small.
#[derive(Debug, Clone, thiserror::Error)]
pub struct SceneLoadError { /* private */ }

impl SceneLoadError {
    /// The diagnostics, in document order.
    pub fn diagnostics(&self) -> &[SceneDiagnostic];
}
```

## Signposts

Documentation to read before starting, in this order:

- [AGENTS.md](../../AGENTS.md) — code style, testing expectations, dependency
  and error-handling policy, and the commit gate.
- [thysalion-design.md](../thysalion-design.md) §6.1 (plane authority), §7
  (the voxel world model — the specification this step implements), §10.5
  (fixed-point material fields), §11.5 (knowledge authoring), §12.1 (ownership
  matrix), and §14 (invariants).
- [ADR 005](../adr-005-workspace-crate-layout.md) — crate layering, the lint
  and allowance policy, and the open question this step resolves.
- [roadmap.md](../roadmap.md) §1.2 — the tasks and their success criteria; and
  §2.1, §3.2, §4.2 for the consumers this format must serve.
- [developers-guide.md](../developers-guide.md) — the demo harness contract,
  the headless testing and coverage boundary, and the Git LFS policy.
- [repository-layout.md](../repository-layout.md) — where files belong.
- [documentation-style-guide.md](../documentation-style-guide.md) — required
  before writing ADR 006 or any prose.
- [scripting-standards.md](../scripting-standards.md) — required before writing
  the fixture generator.
- [rust-testing-with-rstest-fixtures.md](../rust-testing-with-rstest-fixtures.md)
  and
  [reliable-testing-in-rust-via-dependency-injection.md](../reliable-testing-in-rust-via-dependency-injection.md)
  — the fixture and injection idioms this step's tests follow.
- [rust-doctest-dry-guide.md](../rust-doctest-dry-guide.md) — before writing the
  Rustdoc examples AGENTS.md requires.
- [complexity-antipatterns-and-refactoring-strategies.md](../complexity-antipatterns-and-refactoring-strategies.md)
  — for the Stage D refactor pass.
- `references/style-guide.png` — the palette bands, material families, and light
  sources the fixture palettes must name.

External prior art, read for calibration rather than for copying (see
`Surprises & discoveries` for what actually transferred):

- `leynos/lille`, `docs/map-data-format.md` — the map document specification
  design §7.3 refers to. Read it for the shape of the `extends` mechanism and
  the six-normal passability dictionary, and note that neither was ever
  implemented.
- `leynos/lille`, `docs/lille-isometric-tiled-maps-design.md` §5.3, §6.2, §7 —
  the recorded operational lessons: errors delivered as events rather than
  panics, idempotence by consumed-marker, the per-tick restream cost of static
  geometry, and session-scoped block identifiers.

Agent skills to load, and when:

- `rust-router` first, then the skill it routes to. For this step that is
  `rust-types-and-apis` (the newtype and `#[non_exhaustive]` discipline of the
  document model), `rust-errors` (the diagnostic enum and the
  `result_large_err` constraint), and `arch-crate-design` (the crate-versus-
  module question ADR 006 settles).
- `hexagonal-architecture` — for the `SceneSource` port and its adapters, and
  for keeping validation free of input and output.
- `leta` — for navigation and refactoring throughout; `leta show` and
  `leta refs` in place of reading files and grepping.
- `rust-unit-testing` and `proptest` — before Stage B.
- `rust-verification` — to confirm `proptest` is the right rigour here and that
  neither `kani` nor `verus` is warranted for the index mapping.
- `execplans` — to keep this document current as work proceeds.
- `arch-decision-records` — before writing ADR 006.
- `en-gb-oxendict` — for all prose.
- `commit-message` and `pr-creation` — at commit and pull-request time.
