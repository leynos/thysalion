# Publish the scene document's JSON Schema as a versioned artefact (roadmap 1.2.4)

This ExecPlan (execution plan) is a living document. The sections `Constraints`,
`Tolerances`, `Risks`, `Progress`, `Surprises & discoveries`, `Decision log`,
`Outcomes & retrospective`, `Conformance basis`, and `Verification plan` must
be kept up to date as work proceeds.

Status: DRAFT

## Purpose / big picture

Thysalion's scenes are JSON documents, loaded and validated by the
`thysalion-world` crate (`crates/world`). Every document type there already
derives `schemars::JsonSchema`, but nothing emits the schema. The result is
that the only way a tool can check a scene today is to link against
`thysalion-world`. The authoring pipeline of
[thysalion-design.md](../thysalion-design.md) §7.4 needs something different.
Its Tiled exporter (roadmap 10.2.1), its `.vox` importer (roadmap 10.2.2), and
any editor a contributor points at a scene should target the format *as data*.

After this change a contributor or an external tool can do all of the following:

1. Open `schemas/scene-document-1.0.schema.json`, a committed JSON Schema
   (draft 2020-12) generated from the document types. It is stamped with the
   scene document version it describes, and it does not depend on Rust or on
   this workspace.
2. Validate any scene with an off-the-shelf validator, for example:

   ```sh
   uvx --from check-jsonschema==0.38.0 check-jsonschema \
     --schemafile schemas/scene-document-1.0.schema.json \
     assets/scenes/keep-interior.scene.json
   ```

   The validator prints `ok -- validation done`. Pointed at
   `crates/world/tests/fixtures/corrupt/unknown-document-field.scene.json` it
   fails, naming the offending property.
3. Rely on a clear contract about what the schema catches. It rejects every
   *structural* fault, meaning one that is decidable from a single value
   against a constant. It accepts, by design, documents whose faults are
   *semantic*, meaning cross-references and rules that depend on injected
   tables. The loader remains the only authority on whether a scene loads. The
   schema is a faithful, strictly weaker, early-warning mirror of it.
4. Trust that the artefact is current. `make schema-check` regenerates the
   schema, compares it byte for byte with the committed file, and validates
   every fixture scene against it with an independent validator. Continuous
   integration (CI) runs it on every pull request.

Observable success is the roadmap's own criterion. CI regenerates the artefact
and compares it byte for byte. Every fixture scene validates against it. Every
corrupt fixture whose fault is structural is rejected by it.

## Signposts: documents and skills to load

Read these before starting, in this order.

- [roadmap.md](../roadmap.md) §1.2, task 1.2.4. This is the task and its
  success criterion.
- [thysalion-design.md](../thysalion-design.md) §7.3 (scene format) and §7.4
  (authoring pipeline).
- [adr-006-scene-document-model.md](../adr-006-scene-document-model.md). It
  covers the document/domain split, the canonical byte form, and the
  compatibility (version) policy.
  [adr-005-workspace-crate-layout.md](../adr-005-workspace-crate-layout.md)
  covers crate layering.
- [world-plane-architecture.md](../world-plane-architecture.md): the scene
  format reference, the version-history table (Table 6), and "What this plane
  does not validate".
- [execplans/1-2-deliver-the-scene-format-and-fixture-scenes.md](1-2-deliver-the-scene-format-and-fixture-scenes.md),
  Decision log entries dated 2026-07-28 and 2026-08-03. These are the original
  schema decision and the shortfall that created this task.
- [rust-testing-with-rstest-fixtures.md](../rust-testing-with-rstest-fixtures.md),
  [rstest-bdd-users-guide.md](../rstest-bdd-users-guide.md),
  [reliable-testing-in-rust-via-dependency-injection.md](../reliable-testing-in-rust-via-dependency-injection.md),
  [rust-doctest-dry-guide.md](../rust-doctest-dry-guide.md), and
  [complexity-antipatterns-and-refactoring-strategies.md](../complexity-antipatterns-and-refactoring-strategies.md).
- [documentation-style-guide.md](../documentation-style-guide.md) for the ADR
  template and prose rules, and
  [scripting-standards.md](../scripting-standards.md) for any Python or `uv`
  tooling.

Load these agent skills when their topic arises:

- `execplans`, to maintain this document.
- `leta`, for navigation (`leta show SceneDocument`,
  `leta refs SUPPORTED_VERSION`).
- `rust-router`, then `rust-types-and-apis` for the public schema API,
  `rust-errors` for the emitter's error type, `rust-unit-testing` for `rstest`
  and `googletest` shape, and `proptest` for the differential properties.
- `arch-decision-records` for ADR 007, and `arch-supply-chain` before adding
  the `jsonschema` dev-dependency.
- `en-gb-oxendict-style` for prose, `commit-message` for commits, and
  `pr-creation` for the pull request.

## Constraints

Hard invariants. Violation requires escalation, not a workaround.

- **The loader is the authority.** The schema is derived, never normative
  (original decision, 2026-07-28). Nothing in this change alters what
  `SceneLoader` accepts or rejects. The existing loader suites
  (`corrupt_fixtures`, `hostile_*`, `golden_bytes`, `document_round_trip`,
  `generated_fixtures`, `loading`) must pass unchanged. The only exception is
  that they gain *new* corrupt fixtures and snapshots.
- **No wire-format change.** Annotations added to document types are
  `#[schemars(...)]` attributes only. No `#[serde(...)]` attribute, field,
  type, or variant changes. `SUPPORTED_VERSION` stays `1.0`, and the golden
  bytes in `crates/world/tests/fixtures/golden/` must not change.
- **The schema is strictly weaker than the loader.** For every JSON document
  `v`, if `SceneLoader` accepts `v` then the schema accepts `v`. The schema may
  never reject a loadable scene. This is the soundness half of the verification
  plan.
- **The artefact is data.** The committed file must validate against the JSON
  Schema 2020-12 metaschema. It must use only keywords from the 2020-12 core,
  applicator, validation, meta-data, and unevaluated vocabularies, and it must
  carry no `format` values outside the specification's defined set. Ajv's
  default strict mode fails compilation on unknown keywords and unknown
  formats. schemars' `uint16`, `uint32`, and `int32` formats are unknown to
  Ajv, so they are stripped.
- **The bytes are canonical.** The committed bytes are a pure function of the
  document types, `SUPPORTED_VERSION`, and the pinned `schemars` version. They
  must not depend on Cargo feature unification, such as another crate enabling
  `serde_json/preserve_order`, or on hash iteration order. Object keys are
  sorted explicitly at render time.
- **Crate layering.** The emitter lives in `thysalion-world`, where the derives
  are. It adds no runtime dependency. Per ADR 005, no plane crate may become a
  dependency of `thysalion-world`.
- **Workspace lints apply unchanged.** No new `#[allow]` or `#[expect]`. The
  `clippy.toml` thresholds are cognitive complexity 9, 70 lines per function,
  four arguments, and nesting depth four. No file may exceed 400 lines.
- **Filesystem access** in the example binary and tests goes through `cap_std`
  (`fs_utf8`) and `camino`, per AGENTS.md. Ambient authority is taken once, at
  the entry point.
- **`schemars` stays pinned exactly** (`=1.2.2`). A `schemars` bump is a
  deliberate act that regenerates the schema in the same commit.

## Tolerances (exception triggers)

- Scope: stop and escalate if the change needs more than 30 files changed,
  excluding new corrupt fixtures and their snapshots, or more than 1,200 net
  lines of Rust, excluding fixtures and snapshots.
- Loader behaviour: stop if any existing loader test or snapshot changes
  outcome. That means the "loader is the authority" constraint is threatened.
- Wire format: stop if golden bytes, fixture scenes under `assets/scenes/`, or
  `SUPPORTED_VERSION` would change.
- Structural rule set: the local rules the schema mirrors are exactly the six
  in "Decision log", entry D3. If implementation finds another loader rule that
  meets the definition, record it and escalate before adding it. Also escalate
  if one of the six turns out not to meet the definition, for example because
  it depends on `Bounds`.
- Dependencies: the plan adds exactly three dev-dependencies (`googletest`,
  `pretty_assertions`, `jsonschema` with default features off) and one pinned
  tool invocation (`check-jsonschema==0.38.0` through `uvx`). Stop if anything
  else is needed, including any runtime dependency, or if `jsonschema`'s tree
  fails `make audit`.
- Public API: the plan adds `thysalion_world::scene::schema` (see "Interfaces
  and dependencies"). Stop if an existing public signature must change.
- Iterations: if the differential sweep (obligation O5) still finds divergence
  after three fix attempts, stop and record the counterexample.
- Ambiguity: if schemars cannot express one of the six local rules through an
  attribute with a constant path, stop and present options rather than
  hand-editing JSON. The fallback options are a post-generation transform, or
  reclassifying the rule as semantic.

## Risks

- Risk: schemars attributes (`range`, `length`, `extend`) may not accept a
  constant path such as `DESIGN_CHUNK_SIZE`, only literals. Severity: medium.
  Likelihood: medium. Mitigation: prototyping milestone EP-M0 settles this
  first. The fallback is field-level `#[schemars(schema_with = "...")]`
  functions that read the constant, which keeps a single source of truth.
- Risk: feature unification changes key order or number rendering. For
  example, `jsonschema` or `insta` enabling `serde_json/preserve_order` in test
  builds but not in the `cargo run --example` build would make the in-test
  comparison and `make schema-check` disagree. Severity: high. Likelihood:
  medium. Mitigation: the renderer sorts keys itself (obligation O2). The same
  render function backs both the test and the example.
- Risk: JSON Schema's data model treats `32.0` as an integer, but `serde_json`
  refuses it for a `u32`. JSON Schema also cannot see duplicate object keys,
  which `serde` refuses. The schema therefore accepts a few documents that the
  loader refuses. Severity: low (this is soundness-preserving, since the schema
  is weaker, not stricter). Likelihood: certain. Mitigation: record both as
  named residual gaps in ADR 007 and in the format reference. Exclude them from
  the differential oracle's generator by construction, not by filtering.
- Risk: the `jsonschema` crate's dependency tree is large or trips
  `cargo audit`. Severity: medium. Likelihood: low. Mitigation: use
  `default-features = false`, which drops `reqwest`, `rustls`, and `idna`. Run
  `make audit` in EP-M1. The fallback is the `boon` crate, which is also
  2020-12 compliant. Record the choice.
- Risk: new structural corrupt fixtures change the loader's `corrupt_fixtures`
  contract table, for example a test that asserts a fixed fixture count.
  Severity: low. Likelihood: medium. Mitigation: add each fixture with its
  `CLASSES` entry and its insta snapshot in the same commit, and review each
  snapshot by hand.
- Risk: CI network flakiness when `uvx` fetches `check-jsonschema`.
  Severity: low. Likelihood: low. Mitigation: `setup-uv` caches, and the
  version is pinned. The byte-for-byte comparison, the primary gate, needs no
  network.
- Risk: `make scenes-check` is not currently run in CI (a discovery during
  planning; see "Surprises & discoveries"). Adding it may surface already-stale
  fixtures. Severity: medium. Likelihood: low. Mitigation: run
  `make scenes-check` locally in EP-M4 before wiring it. If it fails, stop,
  since fixture staleness is outwith this task's scope, and escalate.

## Progress

- [x] (2026-09-22) Reconnaissance: document types, fixtures, CI, docs, and
  external tooling surveyed.
- [x] (2026-09-22) Draft ExecPlan written.
- [ ] Expert design review completed and findings folded in.
- [ ] Plan approved by the user.
- [ ] EP-M0: prototype schemars attribute and transform mechanics
  (throwaway).
- [ ] EP-M1: dev-dependencies, emitter scaffold, red tests.
- [ ] EP-M2: stamping, integer fidelity, canonical rendering, and the
  committed artefact (green).
- [ ] EP-M3: local-rule annotations, structural classification, new
  structural fixtures, and differential verification.
- [ ] EP-M4: behavioural suite, end-to-end check, Makefile and CI wiring.
- [ ] EP-M5: documentation, ADR 007, roadmap marked done.

## Surprises & discoveries

- Observation: schemars 1.2.2 emits `minimum`/`maximum` for `u8`, `u16`,
  `i8`, and `i16`, but only `minimum: 0` for `u32` and no bounds at all for
  `i32`. Evidence: `schemars-1.2.2/src/json_schema_impls/primitives.rs`, where
  `ranged_impl!` covers the narrow types and `unsigned_impl!`/`simple_impl!`
  cover the rest. Impact: without a transform, `"length": 4294967296` in a run
  passes the schema and fails the loader. That is structural and must be
  rejected, so the integer-bounds transform (D4) is required, not optional.
- Observation: the non-standard `format` values schemars emits (`uint8`,
  `uint16`, `uint32`, `int32`) make Ajv's default strict mode throw at schema
  compile time. Evidence: Ajv strict-mode documentation: "By default unknown
  formats throw exception during schema compilation", and "By default Ajv fails
  schema compilation when unknown keywords are used." Impact: the transform
  removes `format` from integer schemas after encoding their range as bounds.
  No custom `x-` keywords may be used for the version stamp.
- Observation: `make scenes-check` is part of `make all` but is not a step in
  `.github/workflows/ci.yml`. Design §7.4 claims that "a continuous-integration
  check regenerates and compares them byte for byte". Evidence:
  `grep -rn scenes-check .github/` finds nothing. Impact: this task's success
  criterion also concerns "CI regenerates … and compares byte for byte". EP-M4
  wires both checks into CI so that the design claim becomes true (D8).
- Observation: of the 27 file-based corrupt fixtures, only
  `unknown-document-field` and `truncated-json` are refused by the *decoder*.
  Every other one decodes and is refused by a loader rule. Evidence:
  `crates/world/tests/support/corrupt_classes.rs`, where only these two map to
  `None`. Impact: a schema that merely mirrors the decoder would reject two
  fixtures. That is too weak to be useful to tooling, and too thin to count as
  evidence. Hence D3's definition of "structural" and the new decoder-level
  fixtures in EP-M3.

## Decision log

- D1 — Decision: the artefact lives at
  `schemas/scene-document-<major>.<minor>.schema.json` at the repository root.
  One file is kept per released document version. Only the file for
  `SUPPORTED_VERSION` is regenerated. Older files are frozen history, and a
  test asserts their stamps are all older than the current one. Rationale:
  external tooling pins to a version and must keep finding it after a minor
  bump. A top-level `schemas/` directory signals "consumable data", distinct
  from `assets/`, which holds game content. It also keeps the path short for
  editor associations. Date/Author: 2026-09-22, plan author.
- D2 — Decision: the stamp consists of four parts. The first is `$id` =
  `https://thysalion.df12.net/schemas/scene-document-<major>.<minor>.schema.json`.
  The second is `title` = `Thysalion scene document <major>.<minor>`. The
  third is a `$comment` naming the generator and `make schema`. The fourth, and
  the only part with teeth, is the `version` property's subschema:
  `major: {"type":"integer","const":M}` and
  `minor: {"type":"integer","minimum":0,"maximum":m}`, with both required and
  no additional properties. It is produced by a field-level
  `#[schemars(schema_with = "...")]` on `SceneDocument::version` that reads
  `SUPPORTED_VERSION`. Rationale: a stamp that only decorates cannot fail.
  Constraining `version` makes the schema accept exactly the version range that
  `DocumentVersion::accepts` implements, so a 2.0 or 1.1 document is refused by
  the 1.0 schema, as the loader refuses it. `$id` reuses the project's existing
  IRI (Internationalized Resource Identifier) base (`THYSALION_BASE` in
  `crates/world/src/scene/concept.rs`). It is an identifier, and nothing
  requires it to resolve. No custom keywords are used (Ajv strict mode).
  Date/Author: 2026-09-22, plan author.
- D3 — Decision: a fault is **structural** when its detection is determined by
  one of three things. The first is the decoder: JSON syntax, type, required
  field, unknown field, enum tag, array arity, or integer width. The second is
  the document version range. The third is a **local rule**: a numeric bound,
  array-length bound, or constant on a single value, checked by the loader
  against a compile-time constant, independent of position, siblings, and
  injected tables. All other faults are **semantic**. Exactly six loader rules
  are local:

  - `scene.version.unsupported`: `version` outside the accepted range.
  - `scene.chunk-size.not-design`: `chunk_size` equals `DESIGN_CHUNK_SIZE`.
  - `scene.dimensions.zero`: each dimension axis is at least 1.
  - `scene.palette.empty`: `palette` has at least one entry.
  - `scene.palette.emission-out-of-range`: emission `intensity` is at most
    `LightEmission::MAX_INTENSITY`.
  - `scene.voxels.zero-length-run`: a run's `length` is at least 1.

  String-syntax rules (concept IRIs, graph IRIs, and resource paths) stay
  semantic. They depend on the injected `NamespaceTable` or on capability-based
  path policy, and regular-expression dialects differ across validators
  (`check-jsonschema --regex-variant` exists for exactly this reason). `Bounds`
  limits stay semantic because they are injectable policy, not format.
  `palette-zero-not-air` stays semantic because it depends on position.
  Rationale: the definition is decidable, testable, and useful. It gives
  tooling immediate feedback on the commonest authoring slips, without
  duplicating cross-reference logic that would drift. Under it, eight existing
  corrupt fixtures are structural: the six above plus the two parse failures.
  Date/Author: 2026-09-22, plan author (subject to review).
- D4 — Decision: a generator-level schemars `Transform` visits every subschema
  whose `type` is `integer` and has a schemars integer `format`. It sets
  `minimum`/`maximum` to that Rust type's range, intersected with any bound
  already present (so a local rule's tighter bound survives), and then removes
  `format`. An integer format outside the known table (`uint64`, `int64`,
  `uint`, `int`, and anything else) is an emitter *error*, not a pass-through.
  Rationale: this closes the `u32`/`i32` gap. It makes the artefact
  Ajv-strict-portable. It forces a deliberate decision if a 64-bit field ever
  appears, since such a field cannot round-trip exactly through JSON numbers
  beyond 2^53 in most implementations. Date/Author: 2026-09-22, plan author.
- D5 — Decision: rendering sorts object keys recursively and explicitly. It
  pretty-prints with two-space indentation and ends with exactly one trailing
  newline. Rationale: the file is reviewed as a diff, which is half its value
  per the 2026-07-28 decision, so it is pretty-printed. The sorting is explicit
  so that the bytes survive feature unification. Date/Author: 2026-09-22, plan
  author.
- D6 — Decision: the emitter is library code in
  `thysalion_world::scene::schema`. A thin example,
  `crates/world/examples/scene-schema.rs`, writes the artefact (`--write`) or
  compares it (`--check`), mirroring `scene-check`. `make schema` and
  `make schema-check` wrap it. An in-process test also compares the committed
  bytes, so `make test` alone catches staleness. Rationale: the pattern already
  exists, so a contributor sees nothing new. The test makes staleness a local,
  pure-Cargo failure. The Make target gives CI the explicit "regenerate and
  compare" step the roadmap names. Date/Author: 2026-09-22, plan author.
- D7 — Decision: two independent validators are used. The Rust `jsonschema`
  crate (dev-dependency, `default-features = false`) drives the unit, property,
  and behavioural tests. `check-jsonschema==0.38.0`, which is Python and runs
  through `uvx`, runs in `make schema-check` as the end-to-end proof. It
  validates the schema against the metaschema, and validates every
  `assets/scenes/*.scene.json` against the schema. Rationale: "external content
  tooling can target the format as data" is only demonstrated by a consumer
  that shares no code with the producer. `check-jsonschema` is exactly what a
  contributor would run by hand. The Rust validator keeps the fine-grained
  tests inside `cargo test`. Date/Author: 2026-09-22, plan author.
- D8 — Decision: CI gains two steps, `make schema-check` (new) and
  `make scenes-check` (existing but unwired). `make all` gains `schema-check`.
  Rationale: see "Surprises & discoveries". The second step makes design §7.4's
  existing claim true, at a cost of one line. Date/Author: 2026-09-22, plan
  author.
- D9 — Decision: the local-rule annotations reference the loader's own
  constants (`DESIGN_CHUNK_SIZE`, `LightEmission::MAX_INTENSITY`) rather than
  literals. The six local rules are listed once, as `LOCAL_RULE_CODES`, in the
  test support module. The differential oracle uses that list. Rationale: this
  gives each rule a single source of truth. If a constant changes, schema and
  loader move together, and the byte-for-byte check shows the change in review.
  Date/Author: 2026-09-22, plan author.
- D10 — Decision: no Kani harness and no Verus proof. The rationale is
  recorded under "Verification plan". Date/Author: 2026-09-22, plan author.

## Outcomes & retrospective

Not yet started.

## Context and orientation

The workspace is a Rust (edition 2024) Cargo workspace. The crate that matters
here is `thysalion-world`, in `crates/world`.

- `crates/world/src/scene/document/` holds the scene document: `mod.rs`
  (`SceneDocument`, `DocumentVersion`, `SUPPORTED_VERSION = 1.0`,
  `VersionProbe`), `sections.rs` (entities, lighting, knowledge),
  `voxel_type.rs` (palette entries, `MaterialClass`, `SlopeDirection`, `Face`,
  `Passability`, `EmissionDocument`, `SimProperties`), and `voxels.rs` (chunk
  entries, the `ChunkPayloadDocument` enum with `uniform`/`runs` variants,
  `VoxelRunDocument`, and `ExtentDocument`). Every type derives `Serialize`,
  `Deserialize`, and `JsonSchema`, and carries `#[serde(deny_unknown_fields)]`.
  There are no custom serde implementations, no `flatten`, and no `untagged`.
  The only map is a string-keyed `BTreeMap` of prototypes. Integer widths used
  are `u8`, `u16`, `u32`, and `i32`.
- `crates/world/src/loader.rs` and `crates/world/src/scene/validation/` hold
  the loader. It validates in three ordered phases, header then bounded decode
  then semantic rules, and emits dotted diagnostic codes such as
  `scene.voxels.zero-length-run`. The six local rules of D3 are implemented in
  `validation/rules/header.rs` (version, chunk size, dimensions),
  `scene/palette.rs` (empty palette, emission intensity), and `grid/runs.rs` or
  `validation/rules/voxels.rs` (zero-length run).
- `crates/world/tests/fixtures/corrupt/*.scene.json` holds 29 handwritten
  corrupt documents: 27 errors and 2 warnings.
  `crates/world/tests/support/corrupt_classes.rs` maps each to its diagnostic
  code in `CLASSES` and `WARNINGS`. `crates/world/tests/corrupt_fixtures.rs`
  pins each rendered report as an insta snapshot under
  `crates/world/tests/snapshots/corrupt/`.
- `assets/scenes/*.scene.json` holds four compiled fixture scenes
  (`bare-cell`, `keep-interior`, `market-town-block`, `swamp-fragment`),
  generated by `scripts/build_fixture_scenes.py` (`make scenes`) and checked by
  `make scenes-check`.
- `crates/world/tests/loading/` plus
  `crates/world/tests/features/scene_loading.feature` form the existing
  `rstest-bdd` suite. Its `HarnessAdapter` over a plain session context is the
  pattern to copy.
- `crates/world/examples/scene-check.rs` is the thin-example pattern: ambient
  authority is taken once, the logic lives in the library, and exit codes are
  distinct.
- `Makefile` targets: `check-fmt`, `lint`, `test`, `all`, `scenes`,
  `scenes-check`, `scripts-test`, `markdownlint`, `nixie`, and `spelling`.
  `.github/workflows/ci.yml` has a single `build-test` job, and tests run
  inside the `generate-coverage` action.

Terms used below:

- **JSON Schema 2020-12** is the current draft of the JSON Schema
  specification. A *metaschema* is the schema that schemas themselves must
  satisfy.
- **Decoder** means `serde_json` deserializing into `SceneDocument`, which is
  what `codec::decode_document` does.
- **Differential test** means running two implementations on the same input
  and asserting that their verdicts agree according to a stated relation.

## Conformance basis

Upstream artefacts, at the revisions current on `main` at `de78972`:

- ROADMAP-1.2.4: [roadmap.md](../roadmap.md) task 1.2.4 and its success
  criterion, split here into three:
  - SC1: CI regenerates the artefact and compares it byte for byte.
  - SC2: every fixture scene validates against it.
  - SC3: every structurally corrupt fixture is rejected by it.
- DESIGN-7.3: [thysalion-design.md](../thysalion-design.md) §7.3, the scene
  format, its version rule, and load-time validation.
- DESIGN-7.4: design §7.4, the authoring pipeline that emits JSON, and the
  claim that CI regenerates fixtures.
- ADR-006:
  [adr-006-scene-document-model.md](../adr-006-scene-document-model.md),
  covering the document/domain split, the canonical form, and the compatibility
  policy.
- ADR-005: crate layering. `thysalion-world` is the dependency sink.
- PRIOR-DECISION: plan 1.2, Decision log entries of 2026-07-28 (committed
  schema, stale-schema test, exact pin, "derived and never normative") and
  2026-08-03 (shortfall and name, stamp, and gate deferred to 1.2.4).
- No Terms of Reference document exists for this project. The design document
  and roadmap are the upstream contract.

Trace:

```plaintext
ROADMAP-1.2.4/SC1 -> PRIOR-DECISION -> D5, D6, D8 -> EP-M2, EP-M4
    -> schema_artefact::committed_schema_is_current; make schema-check (CI)
ROADMAP-1.2.4/SC2 -> DESIGN-7.3 -> D3, D7 -> EP-M3, EP-M4
    -> schema_fixtures::shipped_scene_validates; make schema-check (check-jsonschema)
ROADMAP-1.2.4/SC3 -> DESIGN-7.3 -> D3, D4, D9 -> EP-M3
    -> schema_fixtures::corrupt_fixture_verdict_matches_classification
ROADMAP-1.2.4 "stamp" -> ADR-006 version policy -> D1, D2 -> EP-M2
    -> schema_artefact::version_subschema_agrees_with_accepts
ROADMAP-1.2.4 "as data" -> DESIGN-7.4 -> D2, D4, D7 -> EP-M2, EP-M4
    -> schema_artefact::uses_only_portable_keywords; check-jsonschema --check-metaschema
DESIGN-7.3 "loader is authority" -> PRIOR-DECISION -> Constraints -> EP-M3
    -> schema_differential (O5 soundness)
```

## Verification plan

The change introduces a *derived artefact* and a *relation* between two
acceptors, the schema and the loader. The obligations below are about that
relation, about the artefact's determinism, and about its version stamp.

Axioms relied on, which are not verified here:

- A1: the `jsonschema` crate and `check-jsonschema` implement JSON Schema
  2020-12 validation correctly for the keywords used (`type`, `properties`,
  `required`, `additionalProperties`, `enum`, `const`, `oneOf`, `anyOf`, `$ref`/
  `$defs`, `items`, `minItems`/`maxItems`, and `minimum`/`maximum`). Both run
  the official JSON Schema test suite.
- A2: serde derive semantics. `deny_unknown_fields` refuses unknown keys. A
  missing `Option` field decodes as `None`. Externally tagged enums take the
  form `{"variant": value}`. Integer deserialization refuses out-of-range
  values and non-integral numbers.
- A3: schemars 1.2.2 under `Contract::Deserialize` emits a schema that reflects
  the serde attributes above. The byte-for-byte check and O5 detect any
  departure, so this axiom is only *relied on* for the sketch of the
  implementation, not for correctness.
- A4: `serde_json::Value` parses integers up to `u64::MAX` exactly, and larger
  magnitudes as `f64`.

Known, accepted divergences, which are the residual gap. All of them are
soundness-preserving, meaning the schema accepts and the loader rejects:
integral floats (`32.0`); duplicate object keys; `Bounds` limits; and every
semantic rule. The O5 generator does not produce integral floats or duplicate
keys, by construction, and ADR 007 lists these divergences.

- O1 — Artefact freshness. The committed file's bytes equal
  `render_scene_document_schema()` for `SUPPORTED_VERSION`.
  - Method: named unit test, plus the `make schema-check` CI step.
  - Rationale: equality of bytes is the claim, and a direct comparison is the
    whole proof.
  - Artefact: `crates/world/tests/schema_artefact.rs`
    (`committed_schema_is_current`), and the example's `--check` mode.
  - Evidence: before EP-M2 the file is absent and the test fails with "read
    committed schema: not found". After EP-M2 it passes.
  - Non-vacuity: the test compares full byte vectors with `pretty_assertions`.
    A negative control in the same file renders, flips one key's value, and
    asserts that the check function reports `Stale`.
- O2 — Canonical rendering. For any JSON value, the rendering is independent of
  the insertion order of object keys, and rendering is idempotent (render,
  parse, render gives identical bytes).
  - Method: property test.
  - Rationale: the property is about all orderings, and cheap generated inputs
    cover it.
  - Domain: `proptest` generates nested JSON objects, depth up to 4 and up to 8
    keys per object, with random insertion orders. It builds the same logical
    object twice through `serde_json::Map` insertion in two permutations, and
    runs under both the default and the `preserve_order` map behaviour by
    construction, because the renderer never iterates a `Map` without sorting.
  - Artefact: `crates/world/tests/schema_canonical.rs`.
  - Evidence: before EP-M2 the renderer delegates to `to_string_pretty`. Under
    `preserve_order` unification the permutation property fails. Run it with
    `cargo test -p thysalion-world --test schema_canonical`.
  - Non-vacuity: assert that at least one generated pair has differing
    insertion orders, by counting in the test. A seeded fault, rendering
    without the sort, must be rejected, which is demonstrated once during the
    red phase and recorded in "Artefacts and notes".
- O3 — Stamp coherence. For a given version `v`, `$id`, `title`, the file name,
  and the `version` subschema are all functions of `v`. The `version` subschema
  accepts a `{major, minor}` pair exactly when `v.accepts(pair)` holds.
  - Method: a parameterized `rstest` table over
    `v ∈ {1.0, 1.3, 2.0, 0.0}` for the strings, plus a property test for the
    acceptance equivalence.
  - Domain: the property ranges over the full `u16 × u16` space of pairs, plus
    wrong-typed and extra-key version objects (always rejected), with `v`
    drawn from `u16 × u16`.
  - Artefact: `crates/world/tests/schema_artefact.rs`
    (`version_subschema_agrees_with_accepts`, `stamp_strings_follow_version`).
  - Evidence: before EP-M2 the version subschema is schemars' unconstrained
    `DocumentVersion`, and the property fails on `(2, 0)` against `v = 1.0`.
  - Non-vacuity: the generator is biased so that 50% of pairs share `v.major`
    and have `minor` within ±2 of `v.minor`, so that both verdicts are common.
    Assert both `accepted > 0` and `rejected > 0` after the run.
- O4 — Integer fidelity. For every integer-typed location in the schema, the
  schema accepts an integer `n` exactly when `n` fits the Rust field type and
  satisfies that field's local rule. No integer subschema lacks either bound,
  and no `format` keyword survives.
  - Method: a structural walk (exhaustive over the emitted schema), plus an
    exhaustive boundary sweep.
  - Domain: the walk covers every subschema reachable from the root, following
    `$ref`. The sweep covers every integer leaf path in a *coverage document*.
    That is the richest fixture (`keep-interior`), augmented in test support so
    that every optional field and every enum variant (`uniform` and `runs`,
    emission present, `concept` present, prototypes) occurs at least once. Each
    leaf is set to `min−1`, `min`, `max`, and `max+1` of its Rust type, and to
    the local bound ±1 where one exists.
  - Artefact: `crates/world/tests/schema_integers.rs`.
  - Evidence: before EP-M2 the sweep reports acceptance of `u32::MAX + 1` at
    `/voxels/*/payload/runs/*/length` and of `i32::MAX + 1` at
    `/lighting/sun_path/azimuth_centidegrees`.
  - Non-vacuity: the test asserts that the set of integer leaf paths visited
    equals the set of integer locations the walk found. A missed location
    fails the test. It also asserts that the swept widths include all four of
    `u8`, `u16`, `u32`, and `i32`.
- O5 — Structural agreement, the central differential. For every document `v`
  obtained from a valid coverage document by exactly one mutation from the
  mutation catalogue:

  ```plaintext
  schema_accepts(v)  ⇔  decodes(v) ∧ version_accepted(v) ∧ no diagnostic of v ∈ LOCAL_RULE_CODES
  ```

  Soundness is the right-to-left direction ("loader-loadable implies
  schema-accepts"), and it follows because semantic codes are outside the
  right-hand side. The mutation catalogue is:

  - replace a value with each other JSON type;
  - delete a required key;
  - add an unknown key to an object;
  - set an integer to a boundary value (as in O4);
  - replace an enum string with an unknown string;
  - change a fixed array's arity (colour `[u8; 3]`) by ±1;
  - replace a `ChunkPayloadDocument` tag with an unknown tag, or give it two
    tags;
  - set `null` where the field is not an `Option`.

  - Method: an exhaustive deterministic sweep, covering every catalogue entry
    applicable at every path of the coverage document, plus a `proptest`
    property for random integer values and random strings at random paths.
  - Rationale: the path × mutation space of one document is finite, so it is
    enumerated rather than sampled. `proptest` adds breadth over value space.
  - Artefact: `crates/world/tests/schema_differential.rs`, with the mutation
    catalogue in `crates/world/tests/support/mutations.rs`.
  - Evidence: before EP-M3 the sweep reports counterexamples such as
    `chunk_size = 16` (schema accepts, local rule rejects). Before EP-M2 it
    reports `u32` overflow. After EP-M3 it reports zero divergences, and the
    count of cases is recorded.
  - Non-vacuity: the sweep tallies the four verdict classes (accepted by both;
    decoder-rejected; local-rule-rejected; semantic-only rejected, where the
    schema accepts and the loader rejects) and asserts that each is non-zero.
    Negative control: the same sweep run against the *raw* schemars schema
    (`schema_for!(SceneDocument)` with draft 2020-12 settings and no transform
    or stamp) must report at least one divergence. The test asserts this, so a
    sweep that cannot see faults fails.
- O6 — Fixture classification. Every `assets/scenes/*.scene.json` validates.
  Every corrupt fixture is rejected by the schema exactly when its manifest row
  is marked `Fault::Structural`.
  - Method: a parameterized `rstest` table over the manifest, plus a glob
    completeness check.
  - Artefact: `crates/world/tests/schema_fixtures.rs`, with the `Fault` column
    in `crates/world/tests/support/corrupt_classes.rs`.
  - Evidence: after EP-M3, 8 existing plus 10 new structural fixtures are
    rejected, and 19 semantic fixtures plus 2 warning fixtures are accepted.
  - Non-vacuity: assert that both classes are non-empty, and that every file
    in the directory appears in the manifest (glob completeness). Accepting
    the semantic fixtures is the check against over-strictness.
- O7 — Portability. The artefact validates against the 2020-12 metaschema.
  Every keyword in it is in the allowlist of 2020-12 vocabulary keywords, and
  it has no `format`.
  - Method: a structural walk, plus the metaschema check in both validators.
  - Artefact: `schema_artefact.rs` (`uses_only_portable_keywords`,
    `is_valid_against_metaschema`), and
    `check-jsonschema --check-metaschema` in `make schema-check`.
  - Non-vacuity: the allowlist test asserts that it visited more than 50
    keywords, and a unit test feeds it a schema containing `"format":"uint16"`
    and `"x-foo"`, which it must flag.
- O8 — Loader unchanged. All pre-existing tests and snapshots pass with no
  edits except additive new rows and new snapshots.
  - Method: `make test` before EP-M1 and after every milestone, plus
    `git diff --stat crates/world/tests/snapshots/` showing only additions.

Why there is no Kani and no Verus (D10). No `unsafe`, arithmetic kernel, or
bounded state machine is introduced. The only arithmetic is the constant bounds
table, which is exercised exhaustively at its boundaries by O4. The invariants
are relations over JSON documents, and their "implementation" on one side is a
third-party validator, which is axiom A1. A Verus proof would have to model
JSON Schema semantics and serde's derive, which would restate the axioms rather
than discharge anything. The finite path × mutation space is enumerated
exhaustively (O5), which is the bounded-exhaustive rung for this problem
without the cost of a model checker. The residual gap is values outside the
catalogue at paths the coverage document does not contain. The glob and walk
completeness checks bound that gap.

## Plan of work

### EP-M0 — Prototype: schemars mechanics (throwaway)

Work on a scratch branch or in a scratch test file that is deleted at the end
of the milestone. Settle three questions:

1. Does `#[schemars(range(min = 1))]`,
   `#[schemars(range(max = LightEmission::MAX_INTENSITY))]`,
   `#[schemars(length(min = 1))]`, and
   `#[schemars(extend("const" = DESIGN_CHUNK_SIZE))]` compile and emit the
   expected keywords on schemars 1.2.2? Is `extend` spelled that way?
2. Does a generator-level `schemars::transform::Transform` added through
   `SchemaSettings::draft2020_12().with_transform(...)` reach every subschema,
   including `$defs` entries and inline `anyOf` branches for `Option`?
   Recursion may need to be explicit, for example with
   `transform::RecursiveTransform`.
3. Does `jsonschema = { version = "0.57", default-features = false }` build
   under the workspace lint table, and does `make audit` stay clean?

Keep the findings if all three are yes. Otherwise, record the fallback in the
Decision log (`schema_with` functions, or `boon`) before EP-M1. The milestone
leaves no code behind, only Decision log and "Surprises & discoveries" entries.

### EP-M1 — Scaffold and red tests

Add the dev-dependencies to the root `Cargo.toml` `[workspace.dependencies]`
and to `crates/world/Cargo.toml` `[dev-dependencies]`: `googletest = "0.14"`,
`pretty_assertions = "1.4"`, and
`jsonschema = { version = "0.57", default-features = false }`. Each gets a
comment giving its reason, as the existing entries have.

Create `crates/world/src/scene/schema/mod.rs`, exported from
`crates/world/src/scene/mod.rs`, with the public surface in "Interfaces and
dependencies". The EP-M1 implementation is deliberately naive. It returns
schemars' raw draft 2020-12 schema and renders it with
`serde_json::to_string_pretty`. That lets the red tests compile and fail on
their assertions rather than on missing symbols.

Write the red tests: `schema_artefact.rs` (O1, O3, O7), `schema_canonical.rs`
(O2), and `schema_integers.rs` (O4). Run each and confirm it fails for the
reason stated in the Verification plan. Record the failure lines under
"Artefacts and notes". Commit, with the failing tests temporarily marked
`#[ignore = "red: EP-M1, see execplan"]` so that the gate stays green. The
marker is removed in EP-M2, and a `grep` in EP-M2's validation confirms that no
`red:` ignore remains. (Rust has no strict-xfail. `#[should_panic]` would pass
for the wrong reason, so the red evidence is the recorded transcript.)

### EP-M2 — Stamp, integer fidelity, canonical rendering, and the artefact

Implement the modules below, each at most 400 lines and each with a `//!`
comment:

- `scene/schema/stamp.rs`: `schema_id(v)`, `schema_title(v)`,
  `schema_file_name(v)`, and `version_schema(generator)`. The last is the
  `schema_with` target that reads `SUPPORTED_VERSION`. Its argument is a
  `&mut SchemaGenerator`, which is schemars' required signature. Attach it with
  `#[schemars(schema_with = "crate::scene::schema::version_schema")]` on
  `SceneDocument::version`.
- `scene/schema/integers.rs`: `IntegerBounds`, the D4 transform. Its
  format-to-range table is a `match` returning `Option<(i64, i64)>`. An unknown
  integer format is recorded as an error that `scene_document_schema` returns.
  Because `Transform::transform` cannot return an error, the transform collects
  errors into a field that is checked after generation.
- `scene/schema/canonical.rs`: `render_canonical(&serde_json::Value) -> String`.
  It performs a recursive key sort into a fresh value tree built from
  `BTreeMap`-ordered insertion, then calls `to_string_pretty` and appends
  `'\n'`.
- `scene/schema/mod.rs`: `scene_document_schema()`, which runs the settings,
  the transform, and the stamp keywords (`$id`, `title`, `$comment`), and
  `render_scene_document_schema()`.

Create `crates/world/examples/scene-schema.rs` with the usage
`scene-schema (--write | --check) <schemas-dir> [--expect-version M.m]`. It
opens `<schemas-dir>` as a `cap_std::fs_utf8::Dir` and writes or compares
`schema_file_name(SUPPORTED_VERSION)`. `--expect-version` fails with exit code
64 if it differs from `SUPPORTED_VERSION`, which guards the Makefile's copy of
the version. The exit codes are 0 for current or written, 1 for stale or
missing, 2 for an I/O failure, and 64 for usage errors. The compare logic lives
in the library as
`schema::compare_artefact(committed: &[u8]) -> ArtefactStatus`, so that it can
be tested without a process.

Add to the `Makefile` (with `mbake validate Makefile` afterwards):

```make
SCENE_SCHEMA_DIR := schemas
SCENE_SCHEMA_VERSION := 1.0
SCENE_SCHEMA := $(SCENE_SCHEMA_DIR)/scene-document-$(SCENE_SCHEMA_VERSION).schema.json
SCENE_SCHEMA_TOOL = $(CARGO) run --quiet -p thysalion-world --example scene-schema --
CHECK_JSONSCHEMA ?= uvx --from check-jsonschema==0.38.0 check-jsonschema

schema: ## Regenerate the published scene document JSON Schema
	$(SCENE_SCHEMA_TOOL) --write $(SCENE_SCHEMA_DIR) \
		--expect-version $(SCENE_SCHEMA_VERSION)

schema-check: ## Verify the published schema is current and fixtures validate
	$(SCENE_SCHEMA_TOOL) --check $(SCENE_SCHEMA_DIR) \
		--expect-version $(SCENE_SCHEMA_VERSION)
	$(CHECK_JSONSCHEMA) --check-metaschema $(SCENE_SCHEMA_DIR)/*.schema.json
	$(CHECK_JSONSCHEMA) --schemafile $(SCENE_SCHEMA) assets/scenes/*.scene.json
```

Run `make schema`, commit `schemas/scene-document-1.0.schema.json`, and remove
the `red:` ignores from O1–O4. At this plateau, O5's local-rule cases are still
expected to diverge, and the differential test does not exist yet.

### EP-M3 — Local rules, classification, new fixtures, and the differential

Add the six local-rule annotations of D3 using the mechanism EP-M0 selected.
They go on `ExtentDocument` axes (`min 1`), `SceneDocument::chunk_size`
(`const DESIGN_CHUNK_SIZE`), `SceneDocument::palette` (`minItems 1`),
`EmissionDocument::intensity` (`max MAX_INTENSITY`), and
`VoxelRunDocument::length` (`min 1`); the version rule is already covered by
D2. Before adding each annotation, confirm with `leta refs` that every use of
the annotated type is subject to the loader rule. `ExtentDocument` is expected
to be used only for `dimensions`. If it is used anywhere else, move the
annotation to a field-level `schema_with`.

Add a `Fault` enum (`Structural`, `Semantic`) column to `CLASSES` and
`WARNINGS` in `crates/world/tests/support/corrupt_classes.rs`, classifying per
D3. Add ten decoder-level structural fixtures under
`crates/world/tests/fixtures/corrupt/`. Each one is `bare-cell`-sized, changes
exactly one thing from a valid document, and has a `CLASSES` row of
`(name, None, Fault::Structural)` and a reviewed insta snapshot:

- `wrong-type-field`: `"chunk_size": "32"`.
- `missing-required-field`: the `lighting` key is absent.
- `unknown-enum-variant`: `"material": "plasma"`.
- `palette-index-overflow`: `{"uniform": 65536}`.
- `run-length-overflow`: `"length": 4294967296`.
- `negative-dimension`: `"x": -32`.
- `angle-overflow`: `"azimuth_centidegrees": 2147483648`.
- `colour-arity`: `"colour": [255, 0]`.
- `payload-unknown-tag`: `{"sparse": 1}`.
- `null-required`: `"name": null`.

Write `crates/world/tests/support/mutations.rs` (the O5 catalogue) and
`crates/world/tests/support/schema.rs`. The second holds `LOCAL_RULE_CODES`, a
lazily compiled `jsonschema::Validator` over the committed file, and the
coverage-document builder. Then write `schema_fixtures.rs` (O6) and
`schema_differential.rs` (O5). Run the differential against the EP-M2 schema
(expected red on the local rules), add the annotations, regenerate with
`make schema`, and run it again (green).

### EP-M4 — Behavioural suite, end-to-end check, and CI

Add `crates/world/tests/features/scene_schema.feature` and a
`[[test]] name = "schema"` harness binary, `crates/world/tests/schema/main.rs`
plus `support.rs`, following `tests/loading/`. The context type is a
`SchemaSession` that holds the compiled validator, the current document, and
the last verdict.

```gherkin
Feature: Published scene document schema

  Scenario: The committed schema is current
    Given the committed scene document schema
    When the schema is regenerated from the document types
    Then the regenerated bytes equal the committed bytes

  Scenario: The schema names the document version it describes
    Given the committed scene document schema
    Then its identifier names document version 1.0
    And it accepts a document declaring version 1.0
    And it rejects a document declaring version 1.1
    And it rejects a document declaring version 2.0

  Scenario Outline: Every shipped fixture scene validates
    Given the shipped fixture scene "<scene>"
    When it is validated against the committed schema
    Then validation succeeds

    Examples:
      | scene             |
      | bare-cell         |
      | keep-interior     |
      | market-town-block |
      | swamp-fragment    |

  Scenario: A structural fault is caught without the engine
    Given the corrupt fixture "run-length-overflow"
    When it is validated against the committed schema
    Then validation fails at "/voxels/0/payload/runs/0/length"

  Scenario: A local rule is caught without the engine
    Given the corrupt fixture "zero-length-run"
    When it is validated against the committed schema
    Then validation fails at "/voxels/0/payload/runs/0/length"

  Scenario: A semantic fault is left to the loader
    Given the corrupt fixture "unknown-palette-index"
    When it is validated against the committed schema
    Then validation succeeds
    And loading the same document fails with "scene.voxels.unknown-palette-index"
```

The JSON Pointer paths above are illustrative. Confirm them against the
fixtures when writing the steps, and correct the feature file if a fixture's
fault sits at a different index. Add an insta snapshot test,
`schema_fixtures::structural_rejections_snapshot`. It renders, for every
structural fixture, the sorted list of `(instance_path, keyword)` pairs from
the Rust validator's errors, with no error message text, since that is not a
stable contract. This pins what an external tool will point at.

Wire CI. In `.github/workflows/ci.yml`, after `make scripts-test` (where `uv`
is already set up), add a `Verify fixture scenes are current` step
(`make scenes-check`) and a `Verify the published scene schema` step
(`make schema-check`). Add `+$(MAKE) schema-check` to `all`. Run
`make scenes-check` locally *first*, per the risk entry.

### EP-M5 — Documentation and close-out

- Write `docs/adr-007-published-scene-document-schema.md` (status
  `Accepted`). It records D1–D5, D7, the structural definition (D3), the
  residual divergences, the "one file per version, older files frozen" policy,
  and the reversal triggers. The first trigger is a 64-bit field, which fails
  the emitter by design. The second is a need for string-syntax rules in the
  schema, which would require settling a regex dialect.
- Amend [thysalion-design.md](../thysalion-design.md) §7.3 (one paragraph: the
  schema is published, derived, and strictly weaker than the loader) and §7.4
  (tooling targets the schema; CI regenerates fixtures *and* schema). Reference
  ADR 007.
- Add a "Published JSON Schema" section to
  [world-plane-architecture.md](../world-plane-architecture.md). It covers the
  file, the stamp, the table of the six local rules, the divergences, and the
  `thysalion_world::scene::schema` interface. Add a sentence under "Version
  history" saying that a version bump adds a new schema file.
- [developers-guide.md](../developers-guide.md): a "Regenerating the schema"
  subsection covering `make schema` and `make schema-check`, what a schema diff
  in review means, what to do after a `schemars` bump, and how to add a new
  corrupt fixture with its `Fault` classification.
- [users-guide.md](../users-guide.md): a "Validating a scene without the
  engine" section, with the `check-jsonschema` command and an editor
  association example. Add `schema` and `schema-check` to its Makefile target
  list.
- [repository-layout.md](../repository-layout.md): the `schemas/` directory and
  its ownership. [contents.md](../contents.md): ADR 007.
- Mark roadmap task 1.2.4 `[x]` in [roadmap.md](../roadmap.md).
- Run `make fmt`, `make markdownlint`, and `make nixie`. Complete "Outcomes &
  retrospective" and set Status to `COMPLETE`.

## Milestones and plateaus

- EP-M0. Outcome: the mechanics are known, and no code changes.
  Requirements: de-risks D3, D4, D7. Acceptance: Decision log entries.
  Conformance: none altered. Recovery: discard scratch. Gaps: all
  implementation. Compatibility decision: none.
- EP-M1. Outcome: the naive emitter plus red tests, marked ignored, and a
  green gate. Requirements: advances SC1. Acceptance: recorded red transcripts,
  and `make check-fmt lint test` green. Conformance: no loader change and no
  wire change. Recovery: revert the commit. Gaps: all behaviour. Compatibility
  decision: none. `scene::schema` is new, pre-1.0 API.
- EP-M2. Outcome: the committed, stamped, integer-faithful, canonical artefact,
  and `make schema-check`. Requirements: SC1 locally, D1, D2, D4, D5, D6.
  Acceptance: O1–O4 and O7 green, and `make schema-check` green. Conformance:
  golden bytes unchanged, and the artefact passes the metaschema. Recovery:
  `make schema` is idempotent, and the commit can be reverted. Gaps: local
  rules, the differential, BDD, CI. Compatibility decision: none.
- EP-M3. Outcome: the schema mirrors the six local rules, and the differential
  and classification hold. Requirements: SC2, SC3, D3, D9. Acceptance: O5 and
  O6 green, and O8 (existing snapshots unchanged, new ones added). Conformance:
  the loader authority constraint holds (soundness, O5). Recovery: annotations
  are schema-only, and reverting them restores EP-M2. Gaps: BDD, CI.
  Compatibility decision: none.
- EP-M4. Outcome: the behavioural suite, the end-to-end check, and CI gated.
  Requirements: SC1 in CI, D7, D8. Acceptance: the `schema` harness passes, and
  the CI run on the draft PR shows both new steps green. Conformance:
  `scenes-check` is green before wiring. Recovery: revert the workflow edit.
  Gaps: docs. Compatibility decision: none.
- EP-M5. Outcome: docs and ADR, with the roadmap marked done.
  Requirements: all trace rows. Acceptance: `make markdownlint`, `make nixie`,
  and `make all` green. Conformance: the upstream docs are amended to match.
  Recovery: documentation-only. Gaps: none. Compatibility decision: none.

After every milestone, run `make check-fmt`, `make lint`, and `make test`
sequentially, never in parallel, then commit.

## Concrete steps

All commands run from the repository root.

```sh
git branch --show   # expect: 1-2-4-publish-scene-document-json-schema-as-versioned-artefact
make test           # baseline, record the pass count before EP-M1
make schema         # EP-M2 onward: writes schemas/scene-document-1.0.schema.json
make schema-check   # expect exit 0 and the transcript below
cargo test -p thysalion-world --test schema_differential -- --nocapture   # prints the verdict tallies
cargo test -p thysalion-world --test schema                                 # BDD suite
make check-fmt && make lint && make test                                    # after every milestone
```

Expected `make schema-check` transcript once the work is complete:

```plaintext
scene-schema: schemas/scene-document-1.0.schema.json is current
ok -- validation done
ok -- validation done
```

## Validation and acceptance

Quality criteria:

- Tests: `make test` passes. It includes the new `schema_artefact`,
  `schema_canonical`, `schema_integers`, `schema_fixtures`, and
  `schema_differential` tests and the `schema` BDD harness, and every
  pre-existing test is unchanged.
- Verification: O1–O8 discharged with the evidence named in "Verification
  plan", and O5's four verdict tallies recorded under "Artefacts and notes".
- Lint, format, and docs: `make check-fmt`, `make lint`, `make markdownlint`,
  `make nixie`, and `mbake validate Makefile` all pass.
- Security: `make audit` passes with the new dev-dependency.
- CI: the draft PR's `build-test` job shows green "Verify fixture scenes are
  current" and "Verify the published scene schema" steps.

Red-Green-Refactor evidence to record:

- Red (EP-M1): the O1, O3, and O4 failure lines, and the O2 permutation
  failure under the seeded fault.
- Red (EP-M3): the O5 counterexample `chunk_size = 16` and the O6 mismatches
  for the six local-rule fixtures.
- Green: the same commands passing.
- Refactor: `make check-fmt lint test` after each tidy-up commit.

## Idempotence and recovery

`make schema` overwrites one file deterministically, so running it twice
changes nothing. `make schema-check` is read-only. New fixtures and snapshots
are additive. If a milestone must be abandoned, `git revert` its commits. No
step touches `assets/scenes/` or golden bytes. If `uvx` cannot reach the
network locally, the byte comparison still runs, since it is the first line of
`schema-check`, and the `check-jsonschema` lines can be retried later.

## Artefacts and notes

Record red and green transcripts, the O5 tallies, and the final schema size
here as work proceeds.

## Interfaces and dependencies

New public module `thysalion_world::scene::schema`:

```rust,no_run
/// Builds the JSON Schema (draft 2020-12) for [`SceneDocument`] at
/// [`SUPPORTED_VERSION`], stamped and with integer bounds made explicit.
pub fn scene_document_schema() -> Result<schemars::Schema, SchemaError>;

/// Renders [`scene_document_schema`] canonically: keys sorted, two-space
/// indentation, one trailing newline. These are the committed bytes.
pub fn render_scene_document_schema() -> Result<String, SchemaError>;

/// Compares committed bytes with a fresh rendering.
pub fn compare_artefact(committed: &[u8]) -> Result<ArtefactStatus, SchemaError>;

/// `scene-document-1.0.schema.json` for version 1.0.
pub fn schema_file_name(version: DocumentVersion) -> String;
pub fn schema_id(version: DocumentVersion) -> String;
pub fn schema_title(version: DocumentVersion) -> String;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ArtefactStatus { Current, Stale }

#[derive(Debug, thiserror::Error)]
pub enum SchemaError {
    #[error("integer format {format:?} at {pointer} has no JSON-safe range")]
    UnsupportedIntegerFormat { format: String, pointer: String },
    #[error("rendering the schema failed: {0}")]
    Render(#[from] serde_json::Error),
}
```

Every public item gets a rustdoc example (per
[rust-doctest-dry-guide.md](../rust-doctest-dry-guide.md)). The `schema_with`
target `version_schema` is `pub(crate)`.

Dependencies. There are no new runtime dependencies (`schemars`, `serde_json`,
and `thiserror` are already present). The new dev-dependencies are:

- `googletest` 0.14, for matcher-style assertions (`expect_that!`,
  `elements_are!`) in the table tests. It is used with `rstest` per its
  documentation.
- `pretty_assertions` 1.4, for readable byte and string diffs in O1.
- `jsonschema` 0.57 with `default-features = false`. It is the Rust-side 2020-12
  validator, and the default features are dropped to exclude `reqwest`,
  `rustls`, and `idna`.

The new external tool is `check-jsonschema==0.38.0`, run through `uvx` in
`make schema-check` only.

## Revision note

- 2026-09-22: initial draft, written from reconnaissance of the document
  types, fixtures, CI, and docs, and from research into schemars 1.2.2, the
  `jsonschema` crate, Ajv strict mode, and `check-jsonschema`. It awaits the
  expert design review.
