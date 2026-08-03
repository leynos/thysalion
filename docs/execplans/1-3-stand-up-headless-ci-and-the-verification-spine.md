# Stand up headless CI and the verification spine (roadmap 1.3)

This ExecPlan (execution plan) is a living document. The sections
`Constraints`, `Tolerances`, `Risks`, `Progress`, `Surprises & discoveries`,
`Decision log`, and `Outcomes & retrospective` must be kept up to date as work
proceeds.

Status: DRAFT

## Purpose / big picture

Thysalion is an isometric voxel role-playing game built on four architectural
planes — state, logic, knowledge, and presentation — described in
[thysalion-design.md](../thysalion-design.md) §6.1. Roadmap step 1.3
([roadmap.md](../roadmap.md) §1.3) is the last step of phase 1, the shared
core: it answers whether the design's invariant-based verification strategy
(design §14) can run without a graphics processing unit or a window from day
one.

The surprising discovery of reconnaissance is that the headless tests already
exist. `crates/world/tests/loading/main.rs` already loads every shipped
fixture scene headlessly through a behavioural suite, and
`crates/harness/tests/headless/main.rs` already drives a `MinimalPlugins` app
with `HarnessCorePlugin` through four behavioural scenarios. What is missing
is everything around them:

1. The two `rstest-bdd` harness adapters — `BevyHarness` in
   `crates/harness/tests/headless/support.rs` and `LoaderHarness` in
   `crates/world/tests/loading/support.rs` — are duplicated, and both carry
   doc comments naming roadmap step 1.3.1 as the point where they are
   promoted into one shared test-support crate. That crate does not exist.
2. No behavioural test yet combines the two halves that task 1.3.1 names in
   one breath: a `MinimalPlugins` harness app *and* fixture-scene loading
   *and* diagnostics counters exposed for assertions. The harness suite never
   loads a scene; the loading suite never builds an app.
3. No workflow runs on every push. `.github/workflows/ci.yml` and
   `act-validation.yml` trigger on `pull_request` and `workflow_dispatch`
   only; `coverage-main.yml` pushes are restricted to `main`. The roadmap's
   success criterion — "a trivial headless behavioural test loads a fixture
   scene in CI on every push" — is therefore not met, and no workflow-shape
   test guards trigger configuration at all.
4. The deterministic replay harness of design §14 (invariant I1) has no
   skeleton. Its storage format must be versioned from the first byte,
   because §14 commits to a replay *corpus* that accumulates as the
   combinatorial-coverage strategy — recorded sessions are long-lived
   artefacts, not throwaway scaffolding.

Concretely, after this change:

1. A crate `thysalion-test-support` at `crates/test-support` hosts both
   harness adapters, and both existing behavioural suites consume them from
   there. The duplicated adapter definitions are gone (task 1.3.1).
2. A new behavioural scenario builds a headless harness app, loads the
   `bare-cell` fixture scene inside it, and asserts against the harness's
   registered diagnostics — the roadmap task's three clauses in one test
   (task 1.3.1).
3. Every push to any branch runs the full test suite in GitHub Actions, and
   `tests/workflow_shape.rs` fails if that trigger is ever removed
   (task 1.3.1).
4. Recording an empty replay session and replaying it is byte-identical,
   asserted by a test that runs in CI, over a storage format whose version
   field, probe, and golden bytes follow the pattern ADR 006 established for
   scene documents. A new ADR 007 records the format decisions (task 1.3.2).

The stability promise of this step is *one adapter set, one verification
spine*: every later phase's property tests, replay assertions, and
end-to-end suites (roadmap 4.1.1, 4.1.2, 6.3.1, 6.3.2, 9.1.2) build on the
scaffolding this step lands, and none of them should ever re-invent a harness
adapter or a record format.

## Documentation and skills to consult

This plan assumes the implementor has only this repository and this file.
Orient with these before each stage:

- [Developer guide](../developers-guide.md) — the commit gates, the headless
  testing and coverage boundary, and the workflow-pin policy that constrains
  Stage C3.
- [ADR 005](../adr-005-workspace-crate-layout.md) — layering rules the new
  crate must respect; [ADR 006](../adr-006-scene-document-model.md) — the
  versioning pattern Stage C4's format copies.
- [Roadmap](../roadmap.md) §1.3 — the two tasks and their success criteria;
  design §14 for the invariant catalogue behind them.
- [Rust testing with rstest fixtures](../rust-testing-with-rstest-fixtures.md)
  and the two adapter modules named above for the harness idiom.
- Agent skills, where available: `execplans` (this document's format),
  `arch-crate-design` (Stage C1's crate boundary), `rust-unit-testing`
  (Stage B), `commit-message` and `pr-creation` (every commit and the PR),
  and `leta` for code navigation throughout.

## Constraints

Hard invariants that must hold throughout implementation. Violation requires
escalation, not workarounds.

- The layering rules in [ADR 005](../adr-005-workspace-crate-layout.md)
  govern crate edges. `thysalion-world` remains the dependency sink among
  plane crates; `thysalion-presentation` must never depend on
  `thysalion-harness`. The new `thysalion-test-support` crate is demo/test
  tooling in ADR 005's sense: no plane crate may take it as a *normal*
  dependency — dev-dependency edges only. The release build stays scoped to
  `-p thysalion` and must not acquire the new crate in its graph
  (`tests/workflow_shape.rs` pins this scoping already).
- The dev-dependency edge `thysalion-world` → `thysalion-test-support`
  alongside the normal edge `thysalion-test-support` → `thysalion-world` is
  the `serde`/`serde_test` shape and is legal to Cargo; a *normal*-edge cycle
  is not, and must never be introduced.
- `bevy` must not enter `thysalion-world`'s dependency graph through the
  test-support crate. The `BevyHarness` half of the crate sits behind a
  non-default `bevy` cargo feature; `thysalion-world` consumes the crate with
  default features only, and `cargo tree -p thysalion-world -e normal,dev`
  must stay `bevy`-free on the default-feature path.
- The design document is the specification. The replay format serves design
  §14 (I1) and §6.2's input-phase semantics; any departure is justified in
  the decision log and written back into the design document in the same
  change.
- The replay storage format follows ADR 006's canonical-encoding discipline
  wholesale: named-struct maps encoded via `rmp_serde::to_vec_named`,
  `#[serde(deny_unknown_fields)]` on every wire type, no `#[serde(flatten)]`,
  no `#[serde(untagged)]`, no tuple structs on the wire, no hand-written
  `serde` implementations, `BTreeMap` never `HashMap`, and a
  `{major, minor}` version with a permissive probe decoded before the full
  document. Golden bytes are checked in and refreshed only deliberately.
- The byte-identity promise of task 1.3.2 is *same-build round-trip*
  determinism: record, replay, re-record within one binary yields identical
  bytes. It is not cross-version wire byte-determinism, which ADR 006
  explicitly declines to promise for scenes; ADR 007 must state the same
  distinction for replay records.
- Workflow contract tests follow the developers' guide's Dependabot policy:
  assert the *shape* of workflow callers (pinned 40-hex SHAs, expected
  triggers, least-privilege permissions), never a literal SHA value.
- The workspace lint table, `clippy.toml` thresholds
  (cognitive complexity 9, 70 lines per function, 4 arguments, nesting 4),
  Whitaker, and the AGENTS.md rules apply unchanged: every module begins
  `//!`, public items carry Rustdoc, no file exceeds 400 lines, filesystem
  access goes through `cap_std::fs_utf8` and `camino`, errors are semantic
  `thiserror` enums, and no exported type is named exactly `Error`.
- `make all` (check-fmt, lint, test, spelling, scripts-test, scenes-check) is
  the commit gate and must pass at every commit. Documentation changes must
  additionally pass `make markdownlint` and `make nixie`. Red test states
  live only in the working tree, never in a commit.
- Dependencies use caret requirements declared once in
  `[workspace.dependencies]`; member manifests use `{ workspace = true }`.
- All documentation follows the
  [documentation style guide](../documentation-style-guide.md):
  en-GB-oxendict spelling, sentence-case headings, 80-column prose wrap, and
  a language identifier on every fenced block. The new ADR follows the ADR
  template and numbering (`adr-007-…`).

## Tolerances (exception triggers)

Thresholds that trigger escalation. They bound autonomous action; they are
not quality criteria.

- Scope: if the change grows beyond roughly 30 files or 2,500 net lines,
  stop and escalate. Excluded from the count: `Cargo.lock`, checked-in golden
  bytes, and mechanical import-path churn in the two existing behavioural
  suites (the promotion renames a module path in every step file; that churn
  is the point of the stage, not a warning sign).
- Dependencies: the approved dependency set for `thysalion-test-support` is
  exactly the crates already pinned in `[workspace.dependencies]`: `serde`,
  `rmp-serde`, `thiserror`, `smol_str`, `tracing`, `cap-std`, `camino`,
  `rstest-bdd-harness`, and (behind the `bevy` feature) `bevy` and
  `thysalion-harness`, plus dev-dependencies from the existing test set
  (`rstest`, `rstest-bdd`, `rstest-bdd-macros`, `proptest`, `insta`, `rmpv`).
  Any dependency outwith that list — including `arrow`, `parquet`,
  `postcard`, or `leafwing_input_playback` — requires escalation before
  adoption.
- Interface: if promoting the adapters forces a change to `thysalion-world`
  or `thysalion-harness` *library* surface (not test code), stop and
  escalate; the promotion is meant to move test code, not to reshape crates.
- CI: if the push trigger cannot be added without either double-running the
  suite on pull-request synchronize events or skipping fork PRs, stop and
  present the trade-offs rather than shipping either silently.
- Iterations: if the same test still fails after five attempts, stop and
  escalate with the failure output.
- Ambiguity: if design §14's wording and this plan's reading of "input
  record" diverge once real simulation input exists to record, stop and
  present the readings; do not extend the format speculatively.

## Risks

- Risk: the promotion breaks both behavioural suites at once, because the
  two `main.rs` files, their `#[scenario]` macros, and the reserved
  `rstest_bdd_harness_context` fixture wiring all reference the adapters by
  path. Severity: medium. Likelihood: medium. Mitigation: promote in two
  movements inside one stage — land the new crate re-exporting the adapters
  while the old modules still exist, re-point one suite, then the other,
  then delete the old modules; `make test` runs between movements.
- Risk: the dev-dependency loop (`world` dev-depends on `test-support`,
  which normally depends on `world`) confuses tooling even though Cargo
  accepts it — `cargo llvm-cov`, `cargo nextest`, or Whitaker may behave
  unexpectedly. Severity: medium. Likelihood: low. Mitigation: Stage A
  spikes the skeleton crate with the loop in place and runs `make test`,
  `make lint`, and `make coverage` before any code moves; if a tool
  misbehaves, escalate before investing in the promotion.
- Risk: adding `push` alongside the retained `pull_request` trigger
  double-runs the suite for every push to an open PR branch, roughly
  doubling Actions minutes; removing `pull_request` instead silently stops
  testing fork PRs, which have no push events in this repository.
  Severity: medium. Likelihood: high if unaddressed. Mitigation: keep both
  triggers; add a `concurrency` group keyed on workflow and ref with
  `cancel-in-progress: true`, and gate the `pull_request` job with an `if`
  that skips same-repository PRs (their pushes already ran the push
  trigger). The decision log records the choice; the workflow-shape test
  asserts both triggers and the guard's presence. Note for reviewers of
  the guard: required status checks match on job name regardless of the
  triggering event, so a same-repository PR whose `pull_request` run is
  skipped is satisfied by the push-triggered run on the same head commit —
  do not "fix" the skip.
- Risk: the shared `generate-coverage` action (invoked with
  `with-ratchet: 'true'`) may assume pull-request context — a base ref to
  diff against, a ratchet baseline to fetch — and misbehave on plain push
  events, turning every branch push red. The likely human response would
  be reverting the push trigger, silently unmeeting the roadmap
  criterion. Severity: high. Likelihood: medium. Mitigation: Stage A reads
  the pinned action's source (it is a `leynos/shared-actions` action,
  locally auditable) and, if push-event behaviour is unsupported or
  unclear, Stage C3 falls back to a separate lightweight job in `ci.yml`
  that runs `make test` on push events only, leaving the coverage job
  PR-scoped. Either shape satisfies "the behavioural test runs on every
  push"; the decision log records which was taken and why.
- Risk: the coverage ratchet (the shared `generate-coverage` action with
  `with-ratchet: 'true'`) shifts when test code moves between crates or new
  measured code lands in `thysalion-test-support`, failing CI for reasons
  unrelated to correctness. Severity: low. Likelihood: medium. Mitigation:
  the test-support crate is itself exercised by the suites that consume it;
  watch the first PR run and, if the ratchet objects, treat the exclusion
  question in the decision log rather than silencing it ad hoc.
- Risk: the replay format is designed against no simulation — `thysalion-sim`
  is an empty skeleton until phase 4 — so the record vocabulary invented now
  is speculation, and speculation hardens into wire compatibility burden.
  Severity: high. Likelihood: high if the format ships a rich payload.
  Mitigation: the skeleton defines the *envelope only* — a versioned session
  header plus a stream of tick-stamped records whose payload enum is
  *uninhabited*: `TickRecord.inputs` can only ever be empty until roadmap
  4.1.2 adds the first real variants with a minor version bump, so nothing
  meaningless ever reaches the wire and no placeholder variant becomes
  permanent compatibility baggage. ADR 007 states this staging explicitly,
  and the tolerance on ambiguity guards the boundary.
- Risk: byte-identity fails intermittently because something nondeterministic
  reaches the wire — a `HashMap` iteration order, a timestamp, a platform
  integer width. Severity: medium. Likelihood: low given the constraints.
  Mitigation: the ADR 006 discipline (BTreeMap, all-integer fields, named
  structs) is applied from the first type; the round-trip test re-records
  twice in one process and also asserts against checked-in golden bytes, so
  drift across builds is caught by review rather than flake.
- Risk: recording raw operating-system input events — the
  `leafwing_input_playback` approach — is the obvious prior art and the
  wrong seam: practitioner reports mark it flaky in CI, the crate is dormant
  (last release targets Bevy 0.15, December 2024), and design §14's I1 is
  stated over *input records at the circuit boundary*, not key codes.
  Severity: medium. Likelihood: low now that it is named. Mitigation: the
  format records domain-level input records (design §6.2's input-phase
  writes: commands, intents, waypoints), never window events; ADR 007
  records the rejection and its grounds.
- Risk: `act-validation.yml` invokes `make test WITH_ACT=1`, but no Makefile
  target consumes `WITH_ACT` — the variable is a silent no-op, so the Act
  workflow duplicates the plain suite rather than adding container-backed
  checks, and the developers' guide describes an intent the build does not
  implement. Severity: low. Likelihood: certain (observed). Mitigation:
  out of scope to fix here; recorded in `Surprises & discoveries`, flagged
  in the PR description, and the new trigger-shape test covers
  `act-validation.yml`'s pinning so the file stops being the untested one.

## Progress

- [ ] Stage A: spike the crate skeleton and the dev-dependency loop; confirm
  tooling accepts it.
- [ ] Stage B: red tests — workflow-shape assertions, the combined headless
  scenario, and the replay round-trip.
- [ ] Stage C1: create `thysalion-test-support`; promote both adapters;
  re-point both suites; delete the duplicates.
- [ ] Stage C2: the combined fixture-in-harness behavioural scenario goes
  green.
- [ ] Stage C3: CI triggers, concurrency, fork guard; workflow-shape tests
  green; `act-validation.yml` joins the pinning test.
- [ ] Stage C4: replay envelope, recorder/replayer, golden bytes, CI test;
  ADR 007.
- [ ] Stage D: documentation, roadmap checkboxes, refactor pass,
  retrospective.

## Surprises & discoveries

- Observation: the "trivial headless behavioural test loads a fixture scene"
  of task 1.3.1 already exists —
  `every_shipped_fixture_scene_loads_clean` in
  `crates/world/tests/loading/main.rs` — and already runs under `make test`.
  Evidence: reconnaissance of the loading suite and the Makefile.
  Impact: 1.3.1's substance shifts to the promotion, the combined scenario,
  and the CI trigger; the plan is written accordingly.
- Observation: no workflow triggers on every push. `ci.yml` and
  `act-validation.yml` are `pull_request` + `workflow_dispatch` only;
  `coverage-main.yml`'s push trigger is `main`-scoped. Evidence: the three
  workflow files. Impact: Stage C3 exists.
- Observation: both adapter doc comments claim "the developers' guide
  already names" the 1.3.1 promotion point, but
  [developers-guide.md](../developers-guide.md)'s headless-testing section
  does not mention a test-support crate or the promotion. Evidence: grep of
  the guide. Impact: Stage D updates the guide so the claim becomes true
  rather than editing the comments to weaken it.
- Observation: `WITH_ACT=1` is passed by `act-validation.yml` and consumed
  by nothing in the Makefile. Evidence: grep of the Makefile. Impact:
  recorded here and in the PR description; not fixed in this step.
- Observation: `make nixie` (Mermaid validation) is wired into no workflow.
  Evidence: grep of `.github/workflows/`. Impact: noted for the PR
  description; adding it to CI is a one-line follow-up outwith this step's
  scope.
- Observation (pre-work verification of the Stage A question): the pinned
  `generate-coverage` action's ratchet stores and restores its baseline
  through the GitHub Actions cache (`ratchet-baseline-…` restore keys) and
  references no pull-request event context anywhere in `action.yml`, so it
  functions on plain `push` events. Evidence: the action source in the
  local `shared-actions` checkout. Impact: Stage C3's single-job form is
  viable; the split-job fallback stays in the plan only as the contingency
  if the live run contradicts this reading.

## Decision log

- Decision: the shared crate is `thysalion-test-support` at
  `crates/test-support`, joining the workspace through the existing
  `crates/*` members glob, with `publish = false`. Rationale: both adapter
  doc comments and the 1.2 ExecPlan name "one shared test-support crate" as
  the 1.3.1 promotion target; ADR 005's taxonomy places it beside
  `thysalion-harness` as tooling, not a plane; `publish = false` because it
  exists for this workspace's suites, not for release (the release build is
  already scoped `-p thysalion`). Date/Author: 2026-08-04, plan author.
- Decision: `LoaderHarness` and `LoaderSession` are the crate's
  default-feature surface; `BevyHarness` sits behind a non-default `bevy`
  feature that pulls `bevy` and `thysalion-harness`. Rationale: keeps
  `thysalion-world`'s dev-dependency path `bevy`-free, preserving ADR 005's
  staging (the state plane acquires `bevy` at roadmap 2.1.1, not through a
  test convenience); consumers that want the app harness opt in exactly as
  rstest-bdd's own ADR 005 (harness adapter crates) intends. Date/Author:
  2026-08-04, plan author.
- Decision: the combined 1.3.1 scenario lives in the test-support crate's
  own behavioural suite (`crates/test-support/tests/`), exercised with the
  `bevy` feature on. Rationale: it tests the promoted product — that the
  Bevy adapter and the loader session compose — so it belongs to the crate
  that owns both; placing it in `crates/harness` would re-create the
  cross-crate reach the promotion removes. Date/Author: 2026-08-04, plan
  author.
- Decision: `ci.yml` gains `push` on all branches while retaining
  `pull_request`, with a workflow-level `concurrency` group
  (`${{ github.workflow }}-${{ github.ref }}`, `cancel-in-progress: true`)
  and an `if` guard on the job skipping `pull_request` events whose head
  repository is this repository. Rationale: push-only would drop fork PRs,
  which never generate push events here; both-unguarded doubles every PR
  push. The guard keeps exactly one run per event that carries new
  information. `act-validation.yml` keeps its `pull_request`-only triggers:
  it is a slower secondary check, and the roadmap criterion names the test
  suite, not Act. Date/Author: 2026-08-04, plan author.
- Decision: the replay skeleton lives in `thysalion-test-support` behind the
  default feature set (`replay` module, no `bevy` requirement), and its
  wire types copy ADR 006's discipline with a `{major, minor}` version and
  a permissive probe. Rationale: the harness that re-executes sessions is
  verification tooling (design §14 names it beside the property tests);
  `thysalion-sim` is empty until phase 4, so parking format types there
  would put wire code in a plane crate for no consumer; ADR 006's pattern
  is battle-tested locally and the style guide's ADR template gives it a
  home as ADR 007. Roadmap 4.1.2 may re-home the *driver* when the circuit
  boundary exists; the format types stay put. Date/Author: 2026-08-04, plan
  author.
- Decision: the record payload enum starts *uninhabited* (`enum
  InputRecord {}`), and Arrow/Parquet are deferred exactly as ADR 006's
  forward note anticipated. Rationale: inventing a record vocabulary
  before any simulation exists is speculation that hardens into wire
  burden (see `Risks`), and even a `Placeholder` variant would be
  permanent compatibility baggage in a corpus format — an uninhabited
  enum lets the envelope, the version rule, and the byte-identity test
  all exist while making a non-empty payload unrepresentable until
  roadmap 4.1.2 defines the first real variants with a minor bump.
  MessagePack via the existing discipline costs no new dependency, and
  the Arrow question re-opens when the corpus acquires analytical
  consumers. ADR 007 records both deferrals with their re-opening
  criteria. Date/Author: 2026-08-04, plan author; revised after the
  design-review panel, same day.

## Outcomes & retrospective

To be completed as stages land.

## Context and orientation

The workspace is a non-virtual Cargo workspace; the root package
`thysalion` is the composition root, and six member crates live under
`crates/` (`world`, `sim`, `knowledge`, `presentation`, `harness`, `demos`
— packages `thysalion-<name>`). Behavioural tests use `rstest-bdd`
0.6.0-beta3: a `#[scenario]` macro binds a Gherkin feature file to a test
function, and a *harness adapter* — a type implementing
`rstest_bdd_harness::HarnessAdapter` with an associated `Context` — owns
scenario execution, handing its context to step functions through the
reserved fixture name `rstest_bdd_harness_context`.

Two adapters exist today, deliberately identical in shape:

- `crates/harness/tests/headless/support.rs` — `BevyHarness`,
  `Context = bevy::app::App`, building `MinimalPlugins` +
  `HarnessCorePlugin::new(HarnessConfig::default())` per scenario. Consumed
  by `crates/harness/tests/headless/main.rs` against
  `crates/harness/tests/features/harness.feature`.
- `crates/world/tests/loading/support.rs` — `LoaderHarness`,
  `Context = LoaderSession`, a plain struct wrapping `MemorySceneSource`,
  a decoded `SceneDocument`, and load outcomes. Consumed by
  `crates/world/tests/loading/main.rs` (declared as an explicit `[[test]]`
  target named `loading`) against
  `crates/world/tests/features/scene_loading.feature`.

`LoaderSession` uses only public `thysalion-world` API: `SceneLoader`,
`LoadedScene`, `SceneLoadError`, `MemorySceneSource`, `DirSceneSource`,
`encode_document`, `Encoding`. `thysalion_harness` exposes
`HarnessCorePlugin`, `HarnessConfig`, `HarnessAction`, `RigState`, and
diagnostic path constants (`FRAME_TIME`, `FPS`, `TICK_TIME`).

Continuous integration lives in `.github/workflows/`: `ci.yml` (format,
markdownlint, spelling, audit, lint, then tests-with-coverage through the
pinned `leynos/shared-actions` `generate-coverage` action),
`act-validation.yml` (runs `make test` under `nektos/act`),
`coverage-main.yml` (push-to-main coverage upload), `release.yml` (tag
builds). Root-level `tests/workflow_shape.rs` asserts release-build scoping
and 40-hex SHA pinning for three of the four workflows. The Makefile's
`test` target runs `cargo nextest run --workspace --all-targets
--all-features` with warnings denied, then doctests.

"Input record" (design §14, invariant I1; roadmap 1.3.2) means a serialized
capture of what design §6.2's input systems would write ahead of a tick —
commands, intents, waypoints — such that replaying the sequence through the
extract phase reproduces identical circuit outputs. No schema exists yet
anywhere in the documentation set; this step defines only the envelope.

## Plan of work

### Stage A — spike the crate skeleton (go/no-go)

Create `crates/test-support` containing only a `lib.rs` module comment and
the manifest: `package.name = "thysalion-test-support"`,
`publish = false`, normal dependencies `thysalion-world` and
`rstest-bdd-harness`, dev-dependency additions to `crates/world` pointing
back at the new crate. Run `make test`, `make lint`, and `make coverage`.
This proves the dev-dependency loop is tolerated by Cargo, nextest,
Whitaker, and `cargo llvm-cov` before anything moves. If any tool rejects
it, stop: the fallback (keeping adapters per-crate and sharing by `#[path]`
include) is a materially different plan requiring escalation. Delete
nothing yet; the spike commit lands with Stage C1 or not at all.

Second verification, per `Risks`: read the pinned
`leynos/shared-actions` `generate-coverage` action's source and determine
whether it behaves correctly on `push` events (ratchet baseline lookup,
any pull-request-context assumptions). Record the finding in
`Surprises & discoveries`; it decides Stage C3's shape between the
single-job and split-job forms.

### Stage B — red tests

Three red surfaces, each failing for its intended reason before any
production change:

1. Extend `tests/workflow_shape.rs` with trigger-shape assertions:
   `ci.yml` must declare a `push` trigger covering all branches, a
   `concurrency` group, and the fork guard; `act-validation.yml` joins the
   SHA-pinning loop. These fail now because the triggers do not exist.
2. The combined scenario's feature file,
   `crates/test-support/tests/features/verification_spine.feature`:

   ```gherkin
   Feature: Headless verification spine

     Scenario: a fixture scene loads inside a headless harness app
       Given a headless harness app
       When the bare-cell fixture scene is loaded into the app
       Then the scene holds 2 palette entries
       And the frame time diagnostic is registered
   ```

   The binding test compiles against the not-yet-existing promoted
   adapters, so the red state is a compile failure in the new crate's test
   target only (kept out of `make test`'s way by landing the feature file
   and binding in the same working tree as Stage C1, per the no-red-commits
   constraint).
3. The replay round-trip test in
   `crates/test-support/tests/replay_round_trip.rs`: record an empty
   session, replay it, re-record, assert byte equality, and compare against
   `crates/test-support/tests/fixtures/golden/empty.session.msgpack`. Red
   because the `replay` module does not exist.

### Stage C1 — promote the adapters

Move `LoaderSession` + `LoaderHarness` into
`crates/test-support/src/loader.rs` and `BevyHarness` into
`crates/test-support/src/bevy_harness.rs` (feature-gated; not `bevy.rs`,
which would shadow the `bevy` crate name inside the module tree), both
re-exported from
`lib.rs` with module comments explaining scope and re-use policy (the
abstraction-policy sweep in AGENTS.md). Re-point
`crates/world/tests/loading/main.rs`, then
`crates/harness/tests/headless/main.rs`, deleting the two `support.rs`
modules once green. `make all` passes; commit.

### Stage C2 — the combined scenario

Implement the step definitions: `Given` builds the app exactly as
`BevyHarness` does; `When` loads `assets/scenes/bare-cell.scene.json`
through a `DirSceneSource` rooted at the workspace `assets/` directory and
inserts the `LoadedScene` as a Bevy resource (a thin newtype defined in the
test, not in any library crate); `Then` asserts palette arity and that
`thysalion_harness::diagnostics::FRAME_TIME` resolves in the app's
`DiagnosticsStore` after one `app.update()`. Green; `make all`; commit.

### Stage C3 — CI on every push

Edit `ci.yml`: add the `push` trigger, the `concurrency` block, and the
fork guard from the decision log. The Stage B workflow-shape assertions go
green. Verify no other workflow needs the trigger (`act-validation.yml`
deliberately stays PR-scoped; the decision log says why). `make all`;
commit; after push, confirm the Actions run appears for the branch push
itself, not only for the PR event.

### Stage C4 — the replay envelope and ADR 007

In `crates/test-support/src/replay/`, define the wire types
(`format.rs`), the version probe (`probe.rs`), and the
recorder/replayer pair (`session.rs`) per `Interfaces and dependencies`
below. Write `docs/adr-007-replay-record-format.md` covering: the envelope
shape; the `{major, minor}` rule and probe copied from ADR 006; the
same-build byte-identity promise and its explicit non-promise of
cross-version byte-determinism; the placeholder payload staging toward
roadmap 4.1.2; and the recorded rejections (raw input capture à la
`leafwing_input_playback`; Arrow now, per ADR 006's deferral note). Golden
bytes land beside the test. Stage B's replay test goes green. `make all`
plus `make markdownlint` and `make nixie`; commit.

### Stage D — documentation and closure

Update [developers-guide.md](../developers-guide.md) (the headless-testing
section gains the test-support crate, making the adapters' doc-comment
claim true; the CI section gains the push trigger and concurrency policy),
[repository-layout.md](../repository-layout.md) (the new crate),
[contents.md](../contents.md) (ADR 007), and tick roadmap 1.3.1 and 1.3.2
with any delivered-shape notes, following the precedent of the 1.2
entries. Post-commit review pass per AGENTS.md; separate refactor commits
if the heuristics fire. Complete `Outcomes & retrospective`.

## Concrete steps

All commands run from the repository root.

```sh
git branch --show   # expect: 1-3-stand-up-headless-ci-and-the-verification-spine
make all            # the commit gate; expect every target green
make markdownlint   # for any commit touching Markdown
make nixie          # for any commit touching Mermaid-bearing Markdown
cargo tree -p thysalion-world -e normal --no-default-features | grep -c bevy
                    # expect: 0 — the state plane stays bevy-free
cargo nextest run -p thysalion-test-support --all-features
                    # focused run for the new crate's suites
```

Expected shape of the replay test's first green run:

```plaintext
    PASS [   0.xxx s] thysalion-test-support::replay_round_trip
                      empty_session_round_trips_byte_identically
```

## Validation and acceptance

Red-Green-Refactor evidence to record per stage:

- Red: Stage B's workflow-shape additions fail with assertion messages
  naming the missing trigger; the replay test fails to compile with
  `unresolved import` on the `replay` module. Quote both in this section
  when observed.
- Green: after C3 and C4, `cargo nextest run` reports the new tests
  passing; after C1/C2 the full `make test` count rises by the new
  scenarios with no losses.
- Refactor: `make all` green after any Stage D cleanup.

Acceptance, phrased as behaviour:

1. Pushing any branch to GitHub triggers the CI workflow; its test step
   runs the behavioural suites, including the fixture-loading scenario
   (roadmap 1.3.1's criterion, now literally true).
2. `rg "struct BevyHarness|struct LoaderHarness" crates/` matches only
   under `crates/test-support/src/`.
3. Running the replay round-trip test twice in a row passes both times,
   and `git status` stays clean — the golden bytes do not churn.
4. `docs/adr-007-replay-record-format.md` exists, is linked from
   `contents.md`, and `make markdownlint` passes.

## Idempotence and recovery

Every stage is an ordinary commit on this branch; re-running `make` targets
is safe. If a stage fails mid-way, `git status` plus the stage list above
locates the boundary; the only cross-file coupling to watch is C1's
two-movement promotion, which is recoverable by restoring the deleted
`support.rs` from `git show HEAD` until both suites compile again. Golden
bytes are regenerated only by deliberately deleting the fixture and
re-running the recorder's test helper, never by a build step.

## Artefacts and notes

Reconnaissance transcripts and measurements land here as stages execute.
The load-bearing pre-work facts: both adapter files carry promotion
comments naming 1.3.1; `ci.yml` has no push trigger; `WITH_ACT` is a
Makefile no-op; `leafwing_input_playback` last targeted Bevy 0.15
(December 2024) and records at the wrong seam for I1.

## Interfaces and dependencies

In `crates/test-support/src/lib.rs` (default features):

```rust
pub use loader::{LoaderHarness, LoaderSession};
pub mod replay;
#[cfg(feature = "bevy")]
pub use bevy_harness::BevyHarness;
```

In `crates/test-support/src/replay/format.rs` — wire types under ADR 006
discipline, all integers, `deny_unknown_fields`:

```rust
/// Wire version of the replay session format; ADR 007 owns the rules.
/// A reader accepts `major == SUPPORTED_MAJOR && minor <= SUPPORTED_MINOR`,
/// probed permissively before full decode, exactly as ADR 006 does for
/// scene documents.
pub struct FormatVersion { pub major: u16, pub minor: u16 }

/// Names the scene a session ran against, by fixture name and the
/// canonical content hash `thysalion_world` already computes.
pub struct SceneRef { pub name: String, pub content_hash_hex: String }

/// Everything a replayer needs before the first tick.
pub struct SessionHeader {
    pub version: FormatVersion,
    pub tick_rate_hz: u32,
    pub scene: Option<SceneRef>,
}

/// One tick's recorded inputs. `InputRecord` is deliberately
/// uninhabited: no input vocabulary exists before roadmap 4.1.2, so
/// `inputs` can only ever be empty; the first real variants arrive with
/// a minor version bump.
pub struct TickRecord { pub tick: u64, pub inputs: Vec<InputRecord> }

pub enum InputRecord {}
```

In `crates/test-support/src/replay/session.rs`:

```rust
pub struct SessionRecorder { /* header + accumulated ticks */ }
impl SessionRecorder {
    pub fn new(header: SessionHeader) -> Self;
    pub fn record_tick(&mut self, record: TickRecord);
    pub fn finish(self) -> Result<Vec<u8>, ReplayEncodeError>;
}

pub struct SessionReplayer;
impl SessionReplayer {
    pub fn open(bytes: &[u8]) -> Result<RecordedSession, ReplayDecodeError>;
}

pub struct RecordedSession { pub header: SessionHeader, /* ticks */ }
impl RecordedSession {
    pub fn ticks(&self) -> impl Iterator<Item = &TickRecord>;
    pub fn re_encode(&self) -> Result<Vec<u8>, ReplayEncodeError>;
}
```

The byte-identity test is `SessionRecorder::finish` output equals
`RecordedSession::re_encode` output equals the golden bytes, for the
empty session (header, zero ticks).

No new external dependency is required anywhere in this plan.
