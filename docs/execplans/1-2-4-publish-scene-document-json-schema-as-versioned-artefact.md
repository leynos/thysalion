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
   uvx --exclude-newer 2026-09-22 --from check-jsonschema==0.38.0 \
     check-jsonschema --schemafile schemas/scene-document-1.0.schema.json \
     assets/scenes/keep-interior.scene.json
   ```

   The validator prints `ok -- validation done`. Pointed at
   `crates/world/tests/fixtures/corrupt/zero-length-run.scene.json` it fails,
   naming the offending location.
3. Rely on a clear contract about what the schema catches. It rejects every
   *structural* fault: a malformed document, a document outside the version
   range, or a breach of one of eight *local rules*, each of which constrains a
   single value against a compile-time constant. It accepts, by design,
   documents whose faults are *semantic*, meaning cross-references and rules
   that depend on injected tables. The loader remains the only authority on
   whether a scene loads. The schema is a strictly weaker, early-warning mirror
   of it: anything the loader accepts, the schema accepts.
4. Trust that the artefact is current. The test suite regenerates the schema
   and compares it byte for byte with the committed file on every run,
   including in continuous integration (CI). CI also validates every fixture
   scene against the committed file with an independent, non-Rust validator.

Observable success is the roadmap's own criterion. CI regenerates the artefact
and compares it byte for byte. Every fixture scene validates against it. Every
corrupt fixture whose fault is structural is rejected by it.

The schema describes the **JSON encoding only**. The MessagePack encoding
carries the same structure, but JSON Schema cannot validate it. Tooling that
emits MessagePack should validate the JSON form first.

## Signposts: documents and skills to load

Read these before starting, in this order.

- [roadmap.md](../roadmap.md) §1.2, task 1.2.4. This is the task and its
  success criterion. Also read §10.2 (tasks 10.2.1 and 10.2.2), which are the
  consumers of this task.
- [thysalion-design.md](../thysalion-design.md) §7.3 (scene format) and §7.4
  (authoring pipeline).
- [adr-006-scene-document-model.md](../adr-006-scene-document-model.md), which
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
- Testing and code-quality guides:
  - [rust-testing-with-rstest-fixtures.md](../rust-testing-with-rstest-fixtures.md).
  - [rstest-bdd-users-guide.md](../rstest-bdd-users-guide.md), in particular
    the "Scenario Outline" and placeholder sections.
  - [reliable-testing-in-rust-via-dependency-injection.md](../reliable-testing-in-rust-via-dependency-injection.md).
  - [rust-doctest-dry-guide.md](../rust-doctest-dry-guide.md).
  - [complexity-antipatterns-and-refactoring-strategies.md](../complexity-antipatterns-and-refactoring-strategies.md).
- [documentation-style-guide.md](../documentation-style-guide.md) for the ADR
  template and prose rules, and
  [scripting-standards.md](../scripting-standards.md) for any `uv` tooling.

Load these agent skills when their topic arises:

- `execplans`, to maintain this document.
- `leta`, for navigation (`leta show SceneDocument`,
  `leta refs SUPPORTED_VERSION`, `leta refs ExtentDocument`).
- `rust-router`, then `rust-types-and-apis` for the public schema API,
  `rust-errors` for the emitter's error type, `rust-unit-testing` for `rstest`
  and `googletest` shape, and `proptest` for the properties.
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
  `generated_fixtures`, `loading`) must pass. The only changes they may see are
  purely additive: new rows appended to `CLASSES` in
  `crates/world/tests/support/corrupt_classes.rs` with its existing two-column
  shape, new fixture files, and new snapshots. No existing row, destructuring
  site, or snapshot may change.
- **No wire-format change.** Annotations added to document types are
  `#[schemars(...)]` attributes only. No `#[serde(...)]` attribute, field,
  type, or variant changes. `SUPPORTED_VERSION` stays `1.0`, and the golden
  bytes in `crates/world/tests/fixtures/golden/` must not change.
- **The schema is strictly weaker than the loader (soundness).** For every JSON
  document `v`, if `SceneLoader` accepts `v`, under any injected `Bounds` or
  namespace table, then the schema accepts `v`. The schema may never reject a
  loadable scene.
- **The artefact is data.** The committed file must validate against the JSON
  Schema 2020-12 metaschema. It may use only keywords from the 2020-12 core,
  applicator, validation, and meta-data vocabularies, and it may contain no
  `format` keyword at all. Ajv's default strict mode fails compilation on
  unknown keywords and unknown formats, and schemars' `uint8`, `uint16`,
  `uint32`, and `int32` formats are unknown to Ajv.
- **The bytes are canonical.** The committed bytes are a pure function of the
  document types, `SUPPORTED_VERSION`, and the pinned `schemars` version. They
  must not depend on Cargo feature unification or on map iteration order.
  Object keys are sorted explicitly at render time.
- **Frozen history.** Once a newer document version exists, the schema file of
  every older version is immutable, and a test enforces it by hash.
- **Crate layering.** The emitter lives in `thysalion-world`, where the derives
  are. It adds no runtime dependency. Per ADR 005, no plane crate may become a
  dependency of `thysalion-world`.
- **Workspace lints apply unchanged.** There is no new `#[allow]` or
  `#[expect]`. The `clippy.toml` thresholds are cognitive complexity 9, 70
  lines per function, four arguments, and nesting depth four. No file may
  exceed 400 lines.
- **Filesystem access** in the example binary and tests goes through `cap_std`
  (`fs_utf8`) and `camino`, per AGENTS.md. Ambient authority is taken once, at
  the entry point.
- **Pinned generators and validators.** `schemars` stays pinned exactly
  (`=1.2.2`), and `jsonschema` is pinned exactly too. Bumping either is a
  deliberate act that follows the procedure in the developers' guide.
- **No committed red tests.** Red-phase failures are recorded as transcripts
  in "Artefacts and notes". Each test is committed together with the code that
  makes it pass, so the gate is never kept green by `#[ignore]`.

## Tolerances (exception triggers)

- Scope: stop and escalate if the change touches more than 45 files, excluding
  new corrupt fixtures and their snapshots, or adds more than 1,600 net lines
  of Rust, excluding fixtures and snapshots. The planned count is about 38: 6
  source files in the schema module, 3 annotated document files, 1 example,
  about 11 test and support files, `Cargo.toml` ×2 and `Cargo.lock`, `Makefile`,
  `ci.yml`, the artefact, and about 9 documents.
- Loader behaviour: stop if any existing loader test or snapshot changes
  outcome, which would threaten the "loader is the authority" constraint.
- Wire format: stop if the golden bytes, the fixture scenes under
  `assets/scenes/`, or `SUPPORTED_VERSION` would change.
- Structural rule set: the local rules the schema mirrors are exactly the
  eight in Decision log entry D3. If implementation finds a further loader rule
  that meets the definition, or finds that one of the eight does not, record it
  and escalate before changing the set.
- Dependencies: the plan adds exactly three dev-dependencies (`googletest`,
  `pretty_assertions`, and `jsonschema` with default features off) and one
  pinned tool invocation (`check-jsonschema==0.38.0` through `uvx`). Stop if
  anything else is needed, including any runtime dependency. Also stop if EP-M0
  finds that `jsonschema` adds more than 60 crates not already in `Cargo.lock`,
  or if its tree fails `make audit`. In that case, present `boon` as the
  alternative.
- Public API: the plan adds `thysalion_world::scene::schema` and one function
  beside `DocumentVersion` (see "Interfaces and dependencies"). Stop if an
  existing public signature must change.
- Iterations: if the differential sweep (obligation O5) still finds divergence
  after three fix attempts, stop and record the counterexample.
- Ambiguity: if schemars cannot express one of the eight local rules through an
  attribute that reads the loader's constant, stop and present the options
  rather than hand-editing JSON. The options are a field-level `schema_with`
  function, a post-generation transform, or reclassifying the rule as semantic.

## Risks

- Risk: a schemars validation attribute silently emits nothing.
  `insert_validation_property` writes a keyword only when the target schema
  already has the matching `type`, so on a field whose schema is a `$ref` the
  attribute is a no-op. Severity: high. Likelihood: medium. Mitigation: EP-M0
  confirms each attribute. The rule-agreement test (O6) asserts that every one
  of the eight constraint keywords is present at its expected location in the
  emitted schema.
- Risk: validation errors inside `oneOf` (the externally tagged
  `ChunkPayloadDocument`) or `anyOf` branches are reported at the combinator,
  not at the leaf. An error in a run's `length` could therefore be reported at
  `/voxels/0/payload`, keyword `oneOf`. Severity: medium. Likelihood: high.
  Mitigation: EP-M0 establishes the actual behaviour of `jsonschema`. Tests and
  BDD steps assert that the error lies *at or below* a path prefix, rather than
  asserting an exact leaf. The snapshot pins the de-duplicated set of leaf
  errors from the validator's detailed output. ADR 007 records that external
  tools may report combinator-level errors.
- Risk: JSON Schema's data model treats `32.0` as an integer, but `serde_json`
  refuses it for a `u32`. JSON Schema also cannot see duplicate object keys,
  which `serde` refuses. Severity: low. These cases preserve soundness, because
  the schema is weaker, not stricter. Likelihood: certain. Mitigation: record
  both as named residual gaps in ADR 007 and in the format reference. The O5
  mutation catalogue produces neither, by construction.
- Risk: `jsonschema` is heavy. In the version inspected (0.37.4),
  `default-features = false` drops only `reqwest` and `tokio`. `idna` (the
  ICU4X stack), `fancy-regex`, `regex`, `email_address`, `uuid-simd`,
  `fraction`, `ahash`, and `referencing` remain unconditional, and several of
  them have build scripts. Its minimum supported Rust version (1.83) is
  satisfied by the pinned nightly. Severity: medium. Likelihood: high.
  Mitigation: EP-M0 measures the 0.57 tree with `cargo tree -e normal` and
  records the new-crate count. The tolerance above decides between it and
  `boon`. `fancy-regex` is not an attack surface here, because the schema
  contains no `pattern` keyword.
- Risk: the three ways a version number is recorded drift apart:
  `SUPPORTED_VERSION` in Rust, `DOCUMENT_VERSION` in
  `scripts/build_fixture_scenes.py`, and `SCENE_SCHEMA_VERSION` in the
  `Makefile`. Severity: medium. Likelihood: medium, at the first version bump.
  Mitigation: `--expect-version` makes `make schema` and `make schema-check`
  exit with code 64 on a Makefile mismatch. The existing `generated_fixtures`
  tests guard the Python copy. The developers' guide gains a version-bump
  checklist.
- Risk: a `schemars` or `jsonschema` bump churns the artefact, the snapshot,
  and the differential all at once, so a reviewer cannot separate intended from
  unintended change. Severity: medium. Likelihood: medium. Mitigation: O5
  prints its verdict tallies in a stable form. The bump procedure in the
  developers' guide requires the tallies to be unchanged before any snapshot is
  accepted.
- Risk: CI network flakiness or a floating Python dependency when `uvx`
  fetches `check-jsonschema`. Severity: low. Likelihood: low. Mitigation: pin
  with `--exclude-newer 2026-09-22` as well as the package version. The
  byte-for-byte comparison needs no network, because it runs inside
  `cargo test`. The network-dependent validation is a separate CI step, so a
  failure names its cause.
- Risk: `make scenes-check` is not currently run in CI (see "Surprises &
  discoveries"). Wiring it in may surface fixtures that are already stale.
  Severity: medium. Likelihood: low. Mitigation: run `make scenes-check`
  locally in EP-M4 before wiring it. If it fails, stop and escalate, because
  fixture staleness is outwith this task.
- Risk: `rstest-bdd` supports `Scenario Outline` (users' guide, lines 83–85),
  but this repository has never bound an outline through
  `#[scenario(index = N)]`. Severity: low. Likelihood: low. Mitigation:
  smoke-test an outline in EP-M0. The fallback is the `scenarios!` macro, or
  four explicit scenarios.

## Progress

- [x] (2026-09-22) Reconnaissance: document types, fixtures, CI, docs, and
  external tooling surveyed.
- [x] (2026-09-22) Draft ExecPlan written.
- [x] (2026-09-22) Expert design review completed (six-lens panel), and its
  findings folded in (revision 2).
- [ ] Plan approved by the user.
- [ ] EP-M0: prototype the schemars, `jsonschema`, and `rstest-bdd` mechanics
  (throwaway).
- [ ] EP-M1: the emitter with stamp, integer fidelity, and canonical
  rendering, plus the committed artefact.
- [ ] EP-M2: local-rule annotations, new structural fixtures, and
  classification and differential verification.
- [ ] EP-M3: behavioural suite, end-to-end check, frozen-history guard, and
  Makefile and CI wiring.
- [ ] EP-M4: documentation, ADR 007, and the roadmap marked done.

## Surprises & discoveries

- Observation: schemars 1.2.2 emits `minimum`/`maximum` for `u8`, `u16`,
  `i8`, and `i16`, but only `minimum: 0` for `u32` and no bounds at all for
  `i32`. Evidence: `schemars-1.2.2/src/json_schema_impls/primitives.rs`, where
  `ranged_impl!` covers the narrow types and `unsigned_impl!`/`simple_impl!`
  cover the rest. Impact: without a transform, `"length": 4294967296` in a run
  passes the schema and fails the loader. That fault is structural and must be
  rejected, so the integer-bounds transform (D4) is required, not optional.
- Observation: the non-standard `format` values schemars emits make Ajv's
  default strict mode throw at schema compile time. Evidence: Ajv strict-mode
  documentation: "By default unknown formats throw exception during schema
  compilation", and "By default Ajv fails schema compilation when unknown
  keywords are used." Impact: the transform removes `format`, and no custom
  `x-` keyword is used.
- Observation: `make scenes-check` is part of `make all` but is not a step in
  `.github/workflows/ci.yml`, although design §7.4 says that "a
  continuous-integration check regenerates and compares them byte for byte".
  Evidence: `grep -rn scenes-check .github/` returns nothing. Impact: EP-M3
  wires it in (D8).
- Observation: of the 27 file-based corrupt error fixtures, only
  `unknown-document-field` and `truncated-json` are refused by the *decoder*.
  Every other one decodes and is then refused by a loader rule. Evidence:
  `crates/world/tests/support/corrupt_classes.rs`, where only these two map to
  `None`. Impact: a schema that merely mirrored the decoder would reject only
  two fixtures, which is too weak to be useful. Hence D3's local rules and the
  new decoder-level fixtures.
- Observation (panel): `scene.palette.too-large` is checked against the
  compile-time `MAX_PALETTE_ENTRIES = u16::MAX + 1`
  (`crates/world/src/scene/palette.rs:38`), not against `Bounds`. It is
  therefore a local rule, which the first draft missed. Impact: D3 now lists
  it, and it is covered in memory, because a file fixture of 65,537 entries is
  too large to commit.
- Observation (panel): no crate enables `serde_json/preserve_order`
  (`cargo tree -e features -i serde_json` lists only `alloc`, `std`, `default`,
  and `unbounded_depth`). `serde_json::Map` is therefore already a `BTreeMap`.
  Impact: the planned red phase for canonical rendering, which relied on
  `preserve_order`, could never fail. O2 now inspects the rendered text
  directly. The explicit sort stays as protection against a future feature
  change.
- Observation (panel): the loader reports only the earliest failing phase, and
  some codes are shared by more than one rule. For example,
  `scene.dimensions.zero` is also emitted for `chunk_size == 0`
  (`validation/rules/header.rs:121`), and voxel rules are skipped when the
  palette fails (`validation/mod.rs:149`). Impact: O5's equivalence holds only
  for single-mutation documents. Soundness is stated separately as the general
  theorem.

## Decision log

- D1 — Decision: the artefact lives at
  `schemas/scene-document-<major>.<minor>.schema.json`, at the repository root.
  One file is kept per document version. Only the file for `SUPPORTED_VERSION`
  is regenerated. Older files are frozen, and a test pins each by its `blake3`
  hash in a `FROZEN_SCHEMAS` table. The table is empty until the first version
  bump. Rationale: external tooling pins to a version and must keep finding
  that version after a bump. A top-level `schemas/` directory signals
  "consumable data", distinct from game content in `assets/`. Pinning by hash
  turns "don't touch the old file" from a convention into a test. `blake3` is
  already a dependency. Date/Author: 2026-09-22, plan author; frozen-hash guard
  added after panel review.
- D2 — Decision: the stamp has four parts:
  - `$id` =
    `https://thysalion.df12.net/schemas/scene-document-<major>.<minor>.schema.json`;
  - `title` = `Thysalion scene document <major>.<minor>`;
  - a `$comment` naming the generator and `make schema`;
  - the `version` property's subschema, which is the only part with teeth:
    `major: {"type":"integer","const":M}` and
    `minor: {"type":"integer","minimum":0,"maximum":m}`, with both required and
    `additionalProperties: false`.

  The subschema is built by `DocumentVersion::acceptance_schema(self)`, which
  lives in `scene/document/mod.rs` beside `DocumentVersion::accepts`. It is
  attached to `SceneDocument::version` through a field-level
  `#[schemars(schema_with = "...")]` whose target applies it to
  `SUPPORTED_VERSION`. Rationale: a stamp that only decorates cannot fail.
  Constraining `version` makes the schema accept exactly the range
  `DocumentVersion::accepts` implements. Keeping the mirror next to the rule
  means a change to either is seen together, and the document types do not
  point back into the emitter. A version-parameterized function is what makes
  O3's property test possible. `$id` reuses the project's existing IRI base
  (`THYSALION_BASE`). It is an identifier, and nothing requires it to resolve.
  Date/Author: 2026-09-22, plan author; placement revised after panel review.
- D3 — Decision: a fault is **structural** when one of the following detects
  it:
  - the JSON parser;
  - the decoder: type, required field, unknown field, enum tag, array arity, or
    integer width;
  - the document version range;
  - a **local rule**. A local rule is a loader rule that, in every document
    satisfying the other local rules, constrains one value, or the length of
    one array, against compile-time constants, independently of that value's
    position and of any injected table or `Bounds`.

  All other faults are **semantic**. Exactly eight loader rules are local:

  - `scene.version.unsupported`: the version is outside the accepted range
    (enforced by D2).
  - `scene.chunk-size.not-design`: `chunk_size` equals `DESIGN_CHUNK_SIZE`.
  - `scene.dimensions.zero`: each axis of `dimensions` is at least 1.
  - `scene.dimensions.unaligned`: each axis of `dimensions` is a multiple of
    `DESIGN_CHUNK_SIZE`. This is local because the `chunk_size` rule pins the
    chunk size to that constant.
  - `scene.palette.empty`: `palette` has at least one entry.
  - `scene.palette.too-large`: `palette` has at most `MAX_PALETTE_ENTRIES`
    entries.
  - `scene.palette.emission-out-of-range`: emission `intensity` is at most
    `LightEmission::MAX_INTENSITY`.
  - `scene.voxels.zero-length-run`: every run's `length` is at least 1.

  Some rules stay semantic:

  - The string-syntax rules (concept IRIs, graph IRIs, and resource paths)
    depend on the injected `NamespaceTable` or on the capability-based path
    policy. Regular-expression dialects also differ across validators; that is
    why `check-jsonschema --regex-variant` exists.
  - `Bounds` limits are injectable policy, not format.
  - `palette-zero-not-air` depends on position.
  - `dimensions-overflow` depends on the product of the three axes.

  Rationale: the definition is decidable and testable. It gives tooling
  immediate feedback on the commonest authoring slips, alignment first among
  them, without duplicating cross-reference logic that would drift. Under it,
  nine existing corrupt fixtures are structural: seven local-rule fixtures and
  two parse failures. `too-large` has no file fixture. Date/Author: 2026-09-22,
  plan author; `too-large` and `unaligned` added after panel review.
- D4 — Decision: a generator-level schemars `Transform` visits every
  subschema whose `type` is `integer` and which carries a schemars integer
  `format`. It intersects any existing bounds with the Rust type's range,
  writes the result as `minimum`/`maximum`, and removes `format`. A tighter
  bound from a local rule therefore survives. Any integer format outside the
  table (`uint64`, `int64`, `uint`, `int`, and anything else) is an emitter
  *error*, not a pass-through. Rationale: this closes the `u32`/`i32` gap and
  makes the artefact portable to Ajv's strict mode. It also forces a deliberate
  decision if a 64-bit field ever appears, because JSON numbers beyond 2^53 do
  not round-trip exactly in most implementations. Date/Author: 2026-09-22, plan
  author.
- D5 — Decision: the renderer sorts object keys recursively and explicitly,
  pretty-prints with two-space indentation, and ends with exactly one trailing
  newline. Rationale: the file is reviewed as a diff, which is half its value
  according to the 2026-07-28 decision. The explicit sort protects the bytes
  against a future `preserve_order` feature being unified in by some
  dependency. Date/Author: 2026-09-22, plan author.
- D6 — Decision: the emitter is library code in
  `thysalion_world::scene::schema`. A thin example,
  `crates/world/examples/scene-schema.rs`, writes the artefact (`--write`) or
  compares it (`--check`), in the same way as `scene-check`. An in-process test
  (O1) compares the committed bytes on every `cargo test`. The CI test step
  therefore regenerates and compares the artefact byte for byte, which
  discharges SC1 without a second Cargo build. Rationale: the pattern already
  exists, so a contributor sees nothing new. Staleness becomes a local,
  pure-Cargo failure. Date/Author: 2026-09-22, plan author; CI discharge
  revised after panel review.
- D7 — Decision: two independent validators are used:
  - The Rust `jsonschema` crate is an exact-pinned dev-dependency with
    `default-features = false`. It drives the unit, property, and behavioural
    tests.
  - `check-jsonschema==0.38.0`, which is Python, is run through `uvx` with
    `--exclude-newer 2026-09-22`. It runs in a Cargo-free `make schema-validate`
    target. That target checks every file in `schemas/` against the metaschema
    and validates every `assets/scenes/*.scene.json` against the current
    schema.
  Rationale: the claim that "external content tooling can target the format as
  data" is only demonstrated by a consumer that shares no code with the
  producer. `check-jsonschema` is exactly what a contributor would run by hand.
  Date/Author: 2026-09-22, plan author.
- D8 — Decision: CI gains two steps, `make schema-validate` and
  `make scenes-check`, both placed after `setup-uv` and before any Cargo build.
  `make all` gains `schema-check`, which is `scene-schema --check` followed by
  `schema-validate`. Rationale: see "Surprises & discoveries". The first step
  adds no Cargo cold build to CI. The second makes design §7.4's existing claim
  true at a cost of one line. Date/Author: 2026-09-22, plan author; revised
  after panel review.
- D9 — Decision: the local-rule annotations read the loader's own constants
  (`DESIGN_CHUNK_SIZE`, `MAX_PALETTE_ENTRIES`, `LightEmission::MAX_INTENSITY`)
  rather than literals. The attribute forms are `range(min = 1)`,
  `range(equal = DESIGN_CHUNK_SIZE)`, `range(max = …)`,
  `length(min = 1, max = MAX_PALETTE_ENTRIES)`, and
  `extend("multipleOf" = DESIGN_CHUNK_SIZE)`. `extend` passes its value through
  `json!`, so a path is accepted. The set of local rules is recorded once, in
  the test-support table `LOCAL_RULES`. Each row holds a diagnostic code, a
  schema location, and a keyword. The structural/semantic classification of
  every corrupt fixture is *computed* from that table (a code of `None` or a
  code in `LOCAL_RULES` is structural), so `CLASSES` keeps its existing shape.
  A rule-agreement test (O6) asserts that the constraint keywords present in
  the emitted schema are exactly those `LOCAL_RULES` names. Rationale: this
  keeps one source of truth for each constant and a single list of rules, and
  nothing can silently drift. Deriving the classification avoids editing the
  existing loader suites. Date/Author: 2026-09-22, plan author; revised after
  panel review.
- D10 — Decision: there is no Kani harness and no Verus proof. The rationale
  is recorded under "Verification plan". Date/Author: 2026-09-22, plan author;
  the panel concurred.
- D11 — Decision: `googletest` is used for matcher-style assertions in the
  table tests, and `pretty_assertions` for byte and string diffs. Rationale:
  the task brief for this change mandates both. The panel noted that neither is
  yet used in the workspace. They are introduced here with a sentence in the
  developers' guide saying when to use each alongside `insta`. Date/Author:
  2026-09-22, plan author.
- D12 — Decision: this task does not adopt the panel's strongest alternative,
  a loader-side rule registry declaring each rule's locality from which the
  schema transform, differential oracle, and fixture classes all read. ADR 007
  records it as the path to take if a ninth local rule appears, or if the
  attributes and `LOCAL_RULES` ever drift. Rationale: it would touch loader
  code, which the "loader is the authority" constraint rules out for this task.
  Its JSON pointers into `$defs` would also depend on schemars' naming. The
  rule-agreement test gives most of its safety at a fraction of its cost.
  Date/Author: 2026-09-22, after panel review.

## Outcomes & retrospective

Not yet started.

## Context and orientation

The workspace is a Rust (edition 2024) Cargo workspace. The crate that matters
here is `thysalion-world`, in `crates/world`.

- `crates/world/src/scene/document/` holds the scene document. It has four
  files:
  - `mod.rs`: `SceneDocument`, `DocumentVersion` with `accepts`,
    `SUPPORTED_VERSION = 1.0`, and `VersionProbe`.
  - `sections.rs`: entities, lighting, and knowledge.
  - `voxel_type.rs`: palette entries, `MaterialClass`, `SlopeDirection`,
    `Face`, `Passability`, `EmissionDocument`, and `SimProperties`.
  - `voxels.rs`: chunk entries, the `ChunkPayloadDocument` enum with its
    `uniform`/`runs` variants, `VoxelRunDocument`, and `ExtentDocument`.

  Every type derives `Serialize`, `Deserialize`, and `JsonSchema`, and carries
  `#[serde(deny_unknown_fields)]`. There are no custom serde implementations, no
  `flatten`, and no `untagged`. The only map is a string-keyed `BTreeMap` of
  prototypes. The integer widths used are `u8`, `u16`, `u32`, and `i32`. Every
  annotated type (`ExtentDocument`, `VoxelRunDocument`, and `EmissionDocument`)
  has exactly one use site. `emission` is not optional, and `concept` is.
- `crates/world/src/loader.rs` and `crates/world/src/scene/validation/` hold
  the loader. It validates in three ordered phases (header, bounded decode,
  then semantic rules) and reports only the earliest failing phase. It emits
  dotted diagnostic codes such as `scene.voxels.zero-length-run`. The eight
  local rules of D3 are implemented in several places:
  - `validation/rules/header.rs` and `grid/extent.rs`: version, chunk size,
    and dimensions.
  - `scene/palette.rs`: empty palette, too-large palette, and emission
    intensity.
  - `grid/runs.rs`: zero-length run.
- `crates/world/tests/fixtures/corrupt/*.scene.json` holds 29 handwritten
  corrupt documents: 27 errors and 2 warnings.
  `crates/world/tests/support/corrupt_classes.rs` maps each one to its
  diagnostic code, in the `CLASSES` and `WARNINGS` tables.
  `crates/world/tests/corrupt_fixtures.rs` pins each rendered report as an
  insta snapshot under `crates/world/tests/snapshots/corrupt/`. It also
  documents codes deliberately left without a file fixture, such as
  `palette.too-large`.
- `crates/world/tests/support/strategy.rs` provides `proptest` generators of
  decodable, though usually not valid, `SceneDocument` values.
- `assets/scenes/*.scene.json` holds four compiled fixture scenes
  (`bare-cell`, `keep-interior`, `market-town-block`, and `swamp-fragment`).
  They are generated by `scripts/build_fixture_scenes.py` (`make scenes`) and
  checked by `make scenes-check`.
- `crates/world/tests/loading/` and
  `crates/world/tests/features/scene_loading.feature` form the existing
  `rstest-bdd` suite. Its `HarnessAdapter`, wrapped around a plain session
  context and binding scenarios with `#[scenario(index = N)]`, is the pattern
  to copy.
- `crates/world/examples/scene-check.rs` is the thin-example pattern. Ambient
  authority is taken once, the logic lives in the library, and each outcome has
  its own exit code.
- `Makefile` targets include `check-fmt`, `lint`, `test`, `all`, `scenes`,
  `scenes-check`, `scripts-test`, `markdownlint`, `nixie`, and `spelling`.
  `.github/workflows/ci.yml` has a single `build-test` job. The Rust toolchain
  and `uv` are set up early in it, and the tests run inside the
  `generate-coverage` action.

Terms used below:

- **JSON Schema 2020-12** is the current draft of the JSON Schema
  specification. A *metaschema* is the schema that schemas themselves must
  satisfy.
- **Decoder** means `serde_json` deserializing into `SceneDocument`, which is
  what `codec::decode_document` does.
- **Schema location** means a JSON Pointer into a document with every array
  index and map key replaced by `*`, such as `/voxels/*/payload/runs/*/length`.
- **Differential test** means running two implementations on the same input
  and asserting that their verdicts agree according to a stated relation.
- **Rejected by the schema** means that the document either fails to parse as
  JSON or fails validation. A validator cannot apply a schema to bytes that are
  not JSON, so `truncated-json` counts as rejected.

## Conformance basis

Upstream artefacts, at the revisions current on `main` at `de78972`:

- ROADMAP-1.2.4: [roadmap.md](../roadmap.md) task 1.2.4 and its success
  criteria:
  - SC1: CI regenerates the artefact and compares it byte for byte.
  - SC2: every fixture scene validates against it.
  - SC3: every structurally corrupt fixture is rejected by it.
- DESIGN-7.3: [thysalion-design.md](../thysalion-design.md) §7.3, covering the
  scene format, its version rule, and load-time validation.
- DESIGN-7.4: design §7.4, covering the authoring pipeline that emits JSON and
  the claim that CI regenerates the fixtures.
- ADR-006:
  [adr-006-scene-document-model.md](../adr-006-scene-document-model.md),
  covering the document/domain split, the canonical form, and the compatibility
  policy.
- ADR-005: crate layering, under which `thysalion-world` is the dependency
  sink.
- PRIOR-DECISION: plan 1.2's Decision log entries. The 2026-07-28 entry covers
  the committed schema, the stale-schema test, the exact pin, and the principle
  "derived and never normative". The 2026-08-03 entry records the shortfall and
  defers the name, stamp, and gate to 1.2.4.
- No Terms of Reference document exists for this project. The design document
  and the roadmap are the upstream contract.

Trace:

```plaintext
ROADMAP-1.2.4/SC1 -> PRIOR-DECISION -> D5, D6 -> EP-M1
    -> schema_artefact::committed_schema_is_current (runs in the CI test step)
ROADMAP-1.2.4/SC2 -> DESIGN-7.3 -> D3, D7, D8 -> EP-M2, EP-M3
    -> schema_fixtures::shipped_scene_validates; make schema-validate (CI)
ROADMAP-1.2.4/SC3 -> DESIGN-7.3 -> D3, D4, D9 -> EP-M2
    -> schema_fixtures::corrupt_fixture_verdict_matches_classification
ROADMAP-1.2.4 "stamp" -> ADR-006 version policy -> D1, D2 -> EP-M1
    -> document::tests::acceptance_schema_agrees_with_accepts
ROADMAP-1.2.4 "as data" -> DESIGN-7.4 -> D2, D4, D7 -> EP-M1, EP-M3
    -> schema_artefact::uses_only_portable_keywords; check-jsonschema --check-metaschema
DESIGN-7.3 "loader is authority" -> PRIOR-DECISION -> Constraints -> EP-M2
    -> schema_differential (O5 soundness and equivalence)
```

## Verification plan

The change introduces a *derived artefact* and a *relation* between two
acceptors, the schema and the loader. The obligations below cover that
relation, the artefact's determinism, and its version stamp.

Axioms relied on, which this plan does not verify:

- A1: the `jsonschema` crate and `check-jsonschema` implement JSON Schema
  2020-12 validation correctly for the keywords used: `type`, `properties`,
  `required`, `additionalProperties`, `enum`, `const`, `oneOf`, `anyOf`, `$ref`/
  `$defs`, `items`, `minItems`/`maxItems`, `minimum`/`maximum`, and
  `multipleOf`. Both run the official JSON Schema test suite.
- A2: serde derive semantics. `deny_unknown_fields` refuses unknown keys. A
  missing or `null` `Option` field decodes as `None`. Externally tagged enums
  take the form `{"variant": value}`. Integer deserialization refuses
  out-of-range and non-integral numbers.
- A3: schemars 1.2.2 under `Contract::Deserialize` renders the serde
  attributes as the panel verified from source:
  - `additionalProperties: false`;
  - `oneOf` of single-key objects for the payload enum;
  - `minItems`/`maxItems: 3` for `[u8; 3]`;
  - `type: [T, "null"]` for an `Option` scalar, not required.

  A3 informs only the design sketch. O5 and O6 check the emitted result
  regardless.
- A4: `serde_json::Value` parses integers up to `u64::MAX` exactly, and larger
  magnitudes as `f64`.

Known, accepted divergences form the residual gap. All of them preserve
soundness, because in each case the schema accepts and the loader rejects:

- integral floats such as `32.0`;
- duplicate object keys;
- `Bounds` limits;
- every semantic rule.

The O5 catalogue produces neither of the first two, and ADR 007 lists all four.

The **coverage document** is built in `crates/world/tests/support/schema.rs`.
It is a small, hand-built `SceneDocument` of about 100 JSON nodes that loads
cleanly, which a test asserts. It contains:

- one `uniform` chunk and one `runs` chunk with two runs;
- a palette of three entries, one with a non-zero emission and a `concept`,
  one with `concept: null`;
- one prototype and one spawn that `extends` it, with every optional field of
  each set;
- one ambient band;
- one knowledge source, supplied through the in-memory `SceneSource`.

A completeness test asserts two things. Its set of schema locations equals the
set of leaf locations the O4 walk finds, which covers every type, variant, and
optional field. The coverage document also loads without diagnostics.

- O1 — Artefact freshness. The committed file's bytes equal
  `render_scene_document_schema()`.
  - Method: named unit test. `make schema-check` repeats it for local use.
  - Artefact: `crates/world/tests/schema_artefact.rs`
    (`committed_schema_is_current`, compared with `pretty_assertions`), and
    `compare_artefact` in the library.
  - Evidence: red while the file is absent, with "read committed schema: not
    found"; green after `make schema`.
  - Non-vacuity: a companion unit test renders the schema, alters one byte,
    and asserts that `compare_artefact` returns `ArtefactStatus::Stale`.
- O2 — Canonical rendering. In the rendered text, keys are strictly ascending
  at every object depth. Rendering is idempotent: render, then parse, then
  render gives identical bytes. The output ends in exactly one newline.
  - Method: `rstest` cases, over the emitted schema and three hand-built
    values with keys inserted out of order, plus one small `proptest` over
    generated nested objects (depth up to 3, up to 6 keys each).
  - Artefact: `crates/world/tests/schema_canonical.rs`. A text scanner reads
    keys per depth from the rendered string and does not re-parse into a map.
  - Evidence: the scanner is shown to reject a handwritten string with
    descending keys, which is the negative control. `preserve_order` is not
    enabled anywhere in the workspace, so no red phase against the renderer
    itself is possible. The obligation guards against a *future* change of
    feature set, and the plan says so honestly.
  - Non-vacuity: the negative control above, and the assertion that the
    emitted schema has more than 20 objects.
- O3 — Stamp coherence and version acceptance.
  - Claim: for any version `v`, `schema_id(v)`, `schema_title(v)`, and
    `schema_file_name(v)` all name `v`. `v.acceptance_schema()` accepts a pair
    `(major, minor)` exactly when `v.accepts(pair)` holds, and it rejects
    wrong-typed values and extra keys.
  - Method: an `rstest` table over `v ∈ {0.0, 1.0, 1.3, 2.0}` for the strings,
    plus an in-crate `#[cfg(test)]` `proptest` for the acceptance
    equivalence.
  - Domain: `v` and the pair are drawn from `u16 × u16`. Half of the pairs
    share `v.major` and have a `minor` within ±2 of `v.minor`.
  - Artefact: `crates/world/src/scene/document/mod.rs`, in its test module
    (`acceptance_schema_agrees_with_accepts`), and `schema_artefact.rs`
    (`stamp_strings_follow_version`, and `committed_schema_carries_the_stamp`,
    which checks `$id`, `title`, and the `version` subschema in the committed
    file).
  - Evidence: before D2 is implemented, the derived `DocumentVersion` schema
    accepts `(2, 0)` for `v = 1.0`. The property's first failing case is
    recorded.
  - Non-vacuity: after the run, assert that both the accepted count and the
    rejected count are greater than zero.
- O4 — Integer fidelity and portability.
  - Claim: a walk of every subschema reachable from the root, following
    `$ref`, finds that every `type: integer` subschema has both `minimum` and
    `maximum`. It also finds no `format` keyword anywhere, and every keyword
    lies in the 2020-12 allowlist. The artefact validates against the 2020-12
    metaschema.
  - Method: an exhaustive structural walk, plus metaschema validation by both
    validators.
  - Artefact: `schema_artefact.rs` (`every_integer_is_bounded`,
    `uses_only_portable_keywords`, `is_valid_against_metaschema`), and
    `check-jsonschema --check-metaschema` in `make schema-validate`.
  - Evidence: against the raw schemars output, the walk reports
    `/voxels/*/payload/runs/*/length` (no maximum) and the `format` keywords.
  - Non-vacuity: assert that the walk visited more than 50 keywords, and that
    the swept widths include all four of `u8`, `u16`, `u32`, and `i32`. A unit
    test feeds the walker a schema containing `"format":"uint16"` and `"x-foo"`
    and asserts that it flags both.
- O5 — Structural agreement, the central differential. It has two relations.
  - Soundness (general): for every document `d` that
    `strategy::scene_document()`
    generates, if the schema rejects `d`, the loader rejects `d`. This is the
    contrapositive of "loader accepts implies schema accepts". The generator's
    documents are decodable but usually semantically invalid, so the property
    exercises the schema-rejects side heavily.
    - Method: `proptest`, 256 cases, under the default bounds of
      `strategy.rs`.
    - Non-vacuity: assert that at least 10% of the cases are schema-rejected.
      The generator produces zero-length runs and out-of-range emission, so
      rejections are expected. If fewer occur, bias the strategy.
  - Equivalence (single mutation): for every document `v` obtained from the
    coverage document by exactly one mutation from the catalogue,

    ```plaintext
    schema_accepts(v) ⇔ parses(v) ∧ decodes(v) ∧ version_accepted(v)
                         ∧ no diagnostic of v has a code in LOCAL_RULES
    ```

    The mutation catalogue is:

    - replace a value with each other JSON type;
    - delete a required key;
    - add an unknown key to an object;
    - set an integer to each of `min−1`, `min`, `max`, and `max+1` of its Rust
      type, and to each local bound ±1;
    - replace an enum string with an unknown string;
    - empty a variable-length array;
    - change the arity of a fixed array (`colour`) by ±1;
    - replace a payload tag with an unknown tag, or add a second tag;
    - set `null` where the field is not an `Option`.

    The sweep takes one representative per schema location. It uses
    `Validator::is_valid`, and the loader runs with the in-memory source.
    `palette.too-large` has an extra in-memory case: a palette of
    `MAX_PALETTE_ENTRIES + 1` entries built programmatically, run once.
    - Method: an exhaustive deterministic sweep. It covers about 100 locations
      by about 8 applicable mutations, around 1,000 cases in total.
    - Evidence: red before EP-M2's annotations, with counterexamples such as
      `chunk_size = 16` (schema accepts, local rule rejects) and a dimension
      axis of 33.
    - Non-vacuity: the sweep tallies four verdict classes: accepted by both,
      rejected by the parser or decoder, rejected by a local rule, and
      semantic-only (schema accepts, loader rejects). It asserts that each
      tally is non-zero and prints them in a stable form. As a negative
      control, the same sweep runs against the raw schemars output (draft
      2020-12 settings, no transform, no stamp, no annotations), and the test
      asserts that it reports at least one divergence.
  - Artefact: `crates/world/tests/schema_differential.rs`, with the catalogue
    in `crates/world/tests/support/mutations.rs`.
- O6 — Fixture classification and rule agreement.
  - Claim: every `assets/scenes/*.scene.json` validates. Every corrupt fixture
    is rejected by the schema exactly when its computed classification is
    structural. The eight `LOCAL_RULES` rows correspond exactly to the
    constraint keywords present in the emitted schema, beyond type-derived
    bounds and arity.
  - Method: parameterized `rstest` over the fixture manifest, a glob
    completeness check, and a table-agreement test.
  - Artefact: `crates/world/tests/schema_fixtures.rs`
    (`shipped_scene_validates`,
    `corrupt_fixture_verdict_matches_classification`,
    `every_corrupt_fixture_is_classified`, `local_rules_match_the_schema`).
  - Evidence: after EP-M2, 19 structural fixtures are rejected and 18 semantic
    and 2 warning fixtures are accepted. The structural 19 are the 9 existing
    ones and the 10 new ones.
  - Non-vacuity: both classes are asserted non-empty. The glob covers every
    file, and accepting the semantic fixtures is the check against the schema
    being too strict. The agreement test fails if any annotation silently
    emitted nothing (see Risks).
- O7 — Frozen history. Every file in `schemas/` is either the current file or
  a `FROZEN_SCHEMAS` entry whose `blake3` hash matches. The stamp of every
  frozen file is strictly older than `SUPPORTED_VERSION`.
  - Method: unit test (`schema_artefact::older_schemas_are_frozen`).
  - Non-vacuity: while the table is empty, a companion test runs the checker
    against an in-memory directory with a tampered 0.9 file and asserts the
    failure. It uses `cap_std`'s in-memory or temporary directory support.
- O8 — The loader is unchanged. All pre-existing tests and snapshots pass. The
  only changes are appended `CLASSES` rows and new snapshot files.
  - Method: `make test` before EP-M1 and after every milestone, plus
    `git diff --stat origin/main -- crates/world/tests/snapshots/` showing only
    added files.

Why there is no Kani and no Verus (D10). No `unsafe` code, arithmetic kernel,
or bounded state machine is introduced. The only arithmetic is the table of
constant bounds, which O4 and O5 exercise exhaustively at its boundaries. The
invariants are relations over JSON documents, and the implementation on one
side of each relation is a third-party validator, which is axiom A1. A Verus
proof would have to model both JSON Schema semantics and serde's derive, so it
would restate the axioms rather than discharge anything. The finite space of
schema locations and mutations is enumerated exhaustively (O5), which is the
bounded-exhaustive rung for this problem without the cost of a model checker.
The residual gap is values outside the catalogue. The O4 walk and the
coverage-completeness test bound its shape.

## Plan of work

### EP-M0 — Prototype: mechanics (throwaway)

Work in a scratch test file that is deleted at the end of the milestone. Settle
six questions:

1. Do `range(min = 1)`, `range(equal = DESIGN_CHUNK_SIZE)`,
   `range(max = LightEmission::MAX_INTENSITY)`,
   `length(min = 1, max = MAX_PALETTE_ENTRIES)`, and
   `extend("multipleOf" = DESIGN_CHUNK_SIZE)` compile on schemars 1.2.2, and
   does each emit its keyword at the intended location? Watch for the silent
   no-op on `$ref` targets.
2. Does a generator-level transform added through
   `SchemaSettings::draft2020_12().with_transform(...)` reach every subschema,
   including `$defs` entries and the branches of `anyOf` and `oneOf`? Is
   `transform::RecursiveTransform` needed?
3. Does `jsonschema = "=0.57.0"` with `default-features = false` build under
   the workspace lints? How many crates does `cargo tree -e normal` add, and
   does `make audit` stay clean? If more than 60 crates are added, repeat the
   question for `boon`.
4. For a fault inside `ChunkPayloadDocument`'s `oneOf`, and for one inside an
   `Option` field's `anyOf`, what instance paths and keywords does `jsonschema`
   report, both through `iter_errors` and through its detailed (`evaluate`)
   output?
5. Does an `rstest-bdd` `Scenario Outline` with an `Examples` table bind
   through `#[scenario(path = ..., index = N)]` in this workspace, with a quoted
   `{scene}` placeholder?
6. Is `schema_with` on a field compatible with `deny_unknown_fields` on the
   parent (`additionalProperties: false` must stay)?

Record every answer in "Surprises & discoveries" and each chosen fallback in
the Decision log. The milestone leaves no code behind.

### EP-M1 — Emitter, stamp, integer fidelity, canonical rendering, and the artefact

This milestone is developed test-first. Write each test, run it, and record its
red transcript in "Artefacts and notes". Implement until the test passes, then
commit the test and the code together.

1. Add the dev-dependencies to the root `Cargo.toml`
   `[workspace.dependencies]` and to the `[dev-dependencies]` of
   `crates/world/Cargo.toml`: `googletest = "0.14"`,
   `pretty_assertions = "1.4"`, and
   `jsonschema = { version = "=0.57.0", default-features = false }`. Each gets
   a comment explaining it, in the style of the existing entries, including why
   `jsonschema` is pinned exactly.
2. Add `DocumentVersion::acceptance_schema(self) -> schemars::Schema` in
   `crates/world/src/scene/document/mod.rs`, together with its in-crate test
   (O3). Attach it to `SceneDocument::version` through
   `#[schemars(schema_with = "supported_version_schema")]`. The target is a
   private function in the same file that returns
   `SUPPORTED_VERSION.acceptance_schema()`.
3. Create the module `crates/world/src/scene/schema/`, exported from
   `crates/world/src/scene/mod.rs`. Each file stays under 400 lines and opens
   with a `//!` comment.
   - `mod.rs`: `scene_document_schema()`, `render_scene_document_schema()`,
     `compare_artefact()`, `ArtefactStatus`, and `SchemaError`.
   - `stamp.rs`: `schema_id(v)`, `schema_title(v)`, `schema_file_name(v)`,
     and the `$comment` text.
   - `integers.rs`: the D4 transform. Its format-to-range table is a `match`
     returning `Option<(i64, i64)>`. `Transform::transform` cannot return an
     error, so the transform records unknown formats in a field that
     `scene_document_schema` checks after generation.
   - `canonical.rs`: `render_canonical(&serde_json::Value) -> String`. It
     sorts keys recursively into a fresh value tree, calls
     `to_string_pretty`, and appends `'\n'`.
   - `walk.rs`: a visitor over subschemas that follows `$ref`. It is shared by
     the transform's error reporting and by the O4 walk, which the tests reach
     through a public `schema::keywords` iterator. If the public surface
     should stay smaller, the walk can instead be duplicated in test support.
     Decide in EP-M0 and record the decision.
4. Write `crates/world/tests/schema_artefact.rs` (O1, O3 strings, O4, O7) and
   `crates/world/tests/schema_canonical.rs` (O2).
5. Create `crates/world/examples/scene-schema.rs` with the usage
   `scene-schema (--write | --check) <schemas-dir> --expect-version M.m`. It
   opens `<schemas-dir>` as a `cap_std::fs_utf8::Dir`, then writes or compares
   `schema_file_name(SUPPORTED_VERSION)`. The exit codes are:
   - 0: the file was written, or is current.
   - 1: the file is stale or missing.
   - 2: an I/O failure.
   - 64: a usage error, or an `--expect-version` mismatch.
6. Add the following to the `Makefile`, then run `mbake validate Makefile`:

   ```make
   SCENE_SCHEMA_DIR := schemas
   SCENE_SCHEMA_VERSION := 1.0
   SCENE_SCHEMA := $(SCENE_SCHEMA_DIR)/scene-document-$(SCENE_SCHEMA_VERSION).schema.json
   SCENE_SCHEMA_TOOL = $(CARGO) run --quiet -p thysalion-world --example scene-schema --
   CHECK_JSONSCHEMA ?= uvx --exclude-newer 2026-09-22 --from check-jsonschema==0.38.0 check-jsonschema

   schema: ## Regenerate the published scene document JSON Schema
   	$(SCENE_SCHEMA_TOOL) --write $(SCENE_SCHEMA_DIR) \
   		--expect-version $(SCENE_SCHEMA_VERSION)

   schema-validate: ## Validate fixture scenes against the schema without Cargo
   	$(CHECK_JSONSCHEMA) --check-metaschema $(SCENE_SCHEMA_DIR)/*.schema.json
   	$(CHECK_JSONSCHEMA) --schemafile $(SCENE_SCHEMA) assets/scenes/*.scene.json

   schema-check: ## Verify the published schema is current, then validate
   	$(SCENE_SCHEMA_TOOL) --check $(SCENE_SCHEMA_DIR) \
   		--expect-version $(SCENE_SCHEMA_VERSION)
   	+$(MAKE) schema-validate
   ```

7. Run `make schema` and commit `schemas/scene-document-1.0.schema.json`
   together with the emitter.

Plateau: the stamped, integer-faithful, canonical artefact is committed and
fresh. O1–O4 and O7 are green. The local rules are not yet annotated, so the
artefact is sound but incomplete. That is correct, only weaker than intended.

### EP-M2 — Local rules, new fixtures, classification, and the differential

1. Write `crates/world/tests/support/schema.rs`. It holds `LOCAL_RULES` (eight
   rows of code, location, and keyword), `is_structural(code: Option<&str>)`, a
   lazily compiled `jsonschema::Validator` over the committed file, and the
   coverage document. Write `crates/world/tests/support/mutations.rs`.
2. Write `schema_differential.rs` (O5) and `schema_fixtures.rs` (O6). Run them
   against the EP-M1 schema and record the red counterexamples. Expected
   failures include `chunk_size = 16`, a dimension axis of 33, and the O6
   mismatches for the seven local-rule fixtures.
3. Add the D9 annotations. Before each one, confirm with `leta refs` that the
   annotated type has a single use site. Put the palette `length` on the
   `SceneDocument::palette` field.
4. Add ten decoder-level structural fixtures under
   `crates/world/tests/fixtures/corrupt/`. Each is a copy of the `bare-cell`
   document with exactly one thing changed. Each gets an appended `CLASSES` row
   `(name, None)` and a reviewed insta snapshot of the loader report:
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
5. Update the comment in `corrupt_fixtures.rs` that calls `palette.too-large`
   a "resource bound", noting that O5 now covers it in memory. The comment
   change is the only edit to that file.
6. Run `make schema` to regenerate, commit, and confirm that O5 and O6 are
   green with the tallies recorded.

Plateau: the schema mirrors all eight local rules. Soundness and
single-mutation equivalence hold, and every fixture is classified.

### EP-M3 — Behavioural suite, end-to-end check, and CI

Add `crates/world/tests/features/scene_schema.feature` and a
`[[test]] name = "schema"` harness binary, made up of
`crates/world/tests/schema/main.rs` and `support.rs`, following the layout of
`tests/loading/`. The context type is a `SchemaSession`. It holds the compiled
validator, the current document bytes, the last verdict, and the last error
locations.

```gherkin
Feature: Published scene document schema

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

  Scenario: A malformed value is caught without the engine
    Given the corrupt fixture "run-length-overflow"
    When it is validated against the committed schema
    Then validation fails at or below "/voxels/0/payload"

  Scenario: A local rule is caught without the engine
    Given the corrupt fixture "unaligned-dimensions"
    When it is validated against the committed schema
    Then validation fails at or below "/dimensions"

  Scenario: A semantic fault is left to the loader
    Given the corrupt fixture "unknown-palette-index"
    When it is validated against the committed schema
    Then validation succeeds
    And loading the same document fails with "scene.voxels.unknown-palette-index"

  Scenario: Text that is not JSON is rejected
    Given the corrupt fixture "truncated-json"
    When it is validated against the committed schema
    Then the document is rejected as unparseable
```

The path prefixes above follow the EP-M0 finding on combinator-level errors.
Confirm them against the fixtures while writing the steps.

Add the snapshot test `schema_fixtures::structural_rejections_snapshot`. For
every structural fixture, it renders the sorted, de-duplicated set of *leaf*
instance paths from `jsonschema`'s detailed output. It omits keyword names
under combinators and all message text, since neither is a stable contract.
This pins what an external tool will point at.

Wire CI. In `.github/workflows/ci.yml`, directly after the `make scripts-test`
step, where `uv` is set up and no Cargo build has yet run, add two steps:
`Verify fixture scenes are current` (`make scenes-check`) and
`Validate fixture scenes against the published schema`
(`make schema-validate`). Add `+$(MAKE) schema-check` to `all`. Run
`make scenes-check` locally first, as the corresponding risk requires.

Plateau: every success criterion is met and observable in CI.

### EP-M4 — Documentation and close-out

- Write `docs/adr-007-published-scene-document-schema.md`, with status
  `Accepted`. It records:
  - D1–D5, D7, and D9, including the structural definition (D3) and its eight
    rules;
  - that the schema covers the JSON encoding only;
  - the residual divergences;
  - combinator-level error locations;
  - the frozen-history policy;
  - that the exact `schemars` pin reaches consumers through the public
    `Schema` return type;
  - the rule-registry alternative (D12);
  - the reversal triggers: a 64-bit field, which fails the emitter by design; a
    need for string-syntax rules, which would require settling a regular
    expression dialect; and a ninth local rule, which would trigger D12's
    registry.
- Amend [thysalion-design.md](../thysalion-design.md) §7.3 with one paragraph
  saying that the schema is published, derived, and strictly weaker than the
  loader. Amend §7.4 to say that tooling targets the schema, and that CI checks
  both the fixtures and the schema. Reference ADR 007 from both.
- Add a "Published JSON Schema" section to
  [world-plane-architecture.md](../world-plane-architecture.md). It covers the
  file, the stamp, a table of the eight local rules, the divergences, and the
  `thysalion_world::scene::schema` interface. Add a sentence under "Version
  history" saying that each version bump adds a new schema file and freezes the
  old one.
- Update [developers-guide.md](../developers-guide.md):
  - Add a "Regenerating the schema" subsection. It covers `make schema`,
    `make schema-check`, and `make schema-validate`, what a schema diff means
    in review, and adding a corrupt fixture.
  - Add a "Bumping `schemars` or `jsonschema`" procedure: regenerate, confirm
    that the O5 tallies are unchanged, and only then accept snapshots.
  - Add a "Bumping the document version" checklist:
    1. Add the field with `#[serde(default)]` for a minor bump.
    2. Bump `SUPPORTED_VERSION`.
    3. Update `DOCUMENT_VERSION` in `scripts/build_fixture_scenes.py`, then
       run `make scenes`.
    4. Update `SCENE_SCHEMA_VERSION` in the `Makefile`.
    5. Run `make schema`, which writes a new file.
    6. Add the old file's `blake3` hash to `FROZEN_SCHEMAS`.
    7. Add a row to Table 6.
    8. Refresh the golden bytes deliberately.
  - Add one sentence on when to use `googletest`, `pretty_assertions`, and
    `insta`.
- Update [users-guide.md](../users-guide.md). Add a "Validating a scene without
  the engine" section with the `check-jsonschema` command, a note that the
  schema covers JSON only, and an example of associating the schema with an
  editor. Add `schema`, `schema-check`, and `schema-validate` to its list of
  Makefile targets.
- Add the `schemas/` directory and its ownership to
  [repository-layout.md](../repository-layout.md). Add ADR 007 to
  [contents.md](../contents.md).
- Mark roadmap task 1.2.4 `[x]` in [roadmap.md](../roadmap.md).
- Run `make fmt`, `make markdownlint`, and `make nixie`. Complete "Outcomes &
  retrospective", and set the Status to `COMPLETE`.

## Milestones and plateaus

- EP-M0.
  - Outcome: the mechanics are known, and there are no code changes.
  - Requirements: de-risks D3, D4, D7, and D9, and the BDD mechanics.
  - Acceptance: Decision log and discoveries entries.
  - Recovery: discard the scratch file.
  - Compatibility decision: none.
- EP-M1.
  - Outcome: the committed, stamped, integer-faithful, canonical artefact, with
    freshness enforced in `cargo test`.
  - Requirements: SC1, D1, D2, D4, D5, and D6.
  - Acceptance: O1–O4 and O7 green; `make check-fmt`, `make lint`, and
    `make test` green; `make schema-check` green.
  - Conformance: the golden bytes are unchanged, and the artefact passes the
    metaschema.
  - Recovery: `make schema` is idempotent, and the commits can be reverted.
  - Gaps: the local rules, the differential, BDD, and CI.
  - Compatibility decision: none. `scene::schema` is new, pre-1.0 API.
- EP-M2.
  - Outcome: the schema mirrors the eight local rules, and the differential
    and classification hold.
  - Requirements: SC2, SC3, D3, and D9.
  - Acceptance: O5, O6, and O8 green, with the tallies recorded.
  - Conformance: soundness holds.
  - Recovery: the annotations affect only the schema, so reverting them
    restores EP-M1.
  - Gaps: BDD and CI.
  - Compatibility decision: none.
- EP-M3.
  - Outcome: the behavioural suite and the end-to-end check exist, and CI is
    gated.
  - Requirements: SC1 and SC2 in CI, D7, and D8.
  - Acceptance: the `schema` harness passes, and the draft PR's CI run shows
    both new steps green.
  - Conformance: `scenes-check` was green before it was wired in.
  - Recovery: revert the workflow edit.
  - Gaps: documentation.
  - Compatibility decision: none.
- EP-M4.
  - Outcome: the documentation and ADR are written, and the roadmap is marked
    done.
  - Requirements: all trace rows.
  - Acceptance: `make markdownlint`, `make nixie`, and `make all` green.
  - Conformance: the upstream documents are amended to match.
  - Recovery: documentation only.
  - Compatibility decision: none.

After every milestone, run `make check-fmt`, `make lint`, and `make test` one
after another, never in parallel, then commit.

## Concrete steps

All commands run from the repository root.

```sh
git branch --show   # expect: 1-2-4-publish-scene-document-json-schema-as-versioned-artefact
make test           # baseline; record the pass count before EP-M1
make schema         # EP-M1 onward: writes schemas/scene-document-1.0.schema.json
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

- Tests: `make test` passes, including the new `schema_artefact`,
  `schema_canonical`, `schema_fixtures`, and `schema_differential` tests, the
  in-crate O3 property, and the `schema` BDD harness. Every pre-existing test
  is unchanged.
- Verification: O1–O8 are discharged with the evidence named in the
  "Verification plan", and the O5 tallies are recorded under "Artefacts and
  notes".
- Lint, formatting, and documentation: `make check-fmt`, `make lint`,
  `make markdownlint`, `make nixie`, and `mbake validate Makefile` all pass.
- Security: `make audit` passes with the new dev-dependency.
- CI: the draft PR's `build-test` job shows green steps for "Verify fixture
  scenes are current" and "Validate fixture scenes against the published
  schema", and its test step runs O1.

Red-Green-Refactor evidence to record:

- Red, EP-M1: the O1 "not found" failure, O3's first failing pair, and O4's
  unbounded locations against the raw output.
- Red, EP-M2: O5's counterexamples (`chunk_size = 16`, a dimension axis of 33)
  and O6's seven local-rule mismatches.
- Green: the same commands passing.
- Refactor: `make check-fmt`, `make lint`, and `make test` after each tidy-up
  commit.

## Idempotence and recovery

`make schema` overwrites one file deterministically, so running it twice
changes nothing. `make schema-check` and `make schema-validate` are read-only.
New fixtures and snapshots are additive. To abandon a milestone, `git revert`
its commits. No step touches `assets/scenes/` or the golden bytes. If `uvx`
cannot reach the network locally, the byte comparison still runs in
`cargo test`, and `make schema-validate` can be retried later.

## Artefacts and notes

Record here, as work proceeds: the red and green transcripts, the EP-M0
answers, the `jsonschema` tree size, the O5 tallies, and the final size of the
schema.

## Interfaces and dependencies

The new public module is `thysalion_world::scene::schema`:

```rust,no_run
/// Builds the JSON Schema (draft 2020-12) for [`SceneDocument`] at
/// [`SUPPORTED_VERSION`], stamped, with integer bounds made explicit and no
/// `format` keywords.
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum ArtefactStatus { Current, Stale }

#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum SchemaError {
    #[error("integer format {format:?} at {pointer} has no JSON-safe range")]
    UnsupportedIntegerFormat { format: String, pointer: String },
    #[error("rendering the schema failed: {0}")]
    Render(#[from] serde_json::Error),
}
```

The following is added beside `DocumentVersion::accepts` in
`crates/world/src/scene/document/mod.rs`:

```rust,no_run
impl DocumentVersion {
    /// A JSON Schema accepting exactly the versions `self.accepts` accepts.
    pub fn acceptance_schema(self) -> schemars::Schema;
}
```

Every public item gets a rustdoc example (per
[rust-doctest-dry-guide.md](../rust-doctest-dry-guide.md)). Returning
`schemars::Schema` is acceptable, because the `JsonSchema` derives already make
`schemars` part of the public surface. ADR 007 notes that the exact pin
therefore reaches consumers.

There are no new runtime dependencies. `schemars`, `serde_json`, `thiserror`,
and `blake3` are already present. The new dev-dependencies are:

- `googletest` 0.14, for matcher-style assertions (`expect_that!`,
  `elements_are!`) in the table tests. It works with `rstest`, as its own
  documentation describes.
- `pretty_assertions` 1.4, for readable byte and string diffs in O1.
- `jsonschema =0.57.0` with `default-features = false`, the 2020-12 validator
  used on the Rust side. It is pinned exactly because a patch release may
  change the shape of its error output, which the snapshot pins.

The new external tool is `check-jsonschema==0.38.0`, run through `uvx` with
`--exclude-newer 2026-09-22`, and only in `make schema-validate`.

## Revision note

- 2026-09-22, revision 1: initial draft. It was written from reconnaissance
  of the document types, the fixtures, CI, and the documentation, and from
  research into schemars 1.2.2, the `jsonschema` crate, Ajv's strict mode, and
  `check-jsonschema`.
- 2026-09-22, revision 2: the six-lens expert panel's findings are folded in:
  - The local rules grow from six to eight, adding `palette.too-large` and
    `dimensions.unaligned`.
  - The fixture classification is computed, so `CLASSES` keeps its shape.
  - The O5 sweep now uses a small coverage document with one representative
    per schema location, adds a general soundness property over the existing
    generator, and adds an empty-array mutation.
  - O2 now inspects the rendered text, because `preserve_order` is not enabled
    anywhere.
  - O3 moves into the crate, beside `accepts`.
  - The redundant freshness BDD scenario is dropped, and O4's sweep is folded
    into O5.
  - The frozen-history hash guard (O7) is new.
  - No red tests are committed.
  - CI runs a Cargo-free `schema-validate` step, and SC1 is discharged by O1
    inside the CI test step.
  - `check-jsonschema` is pinned with `--exclude-newer`, and `jsonschema` is
    pinned exactly.
  - The public enums are marked `#[non_exhaustive]`.
  - EP-M0 gains questions on error locations under combinators and on BDD
    outlines.
  - The scope tolerance is raised to 45 files and 1,600 lines.
  - The developers' guide gains a version-bump checklist and a dependency-bump
    procedure.

  The milestones are renumbered: the former EP-M1 and EP-M2 are merged into
  EP-M1. The remaining work is unchanged in intent.
