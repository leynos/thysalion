# Establish the workspace and the demo harness contract (roadmap 1.1)

This ExecPlan (execution plan) is a living document. The sections `Constraints`,
`Tolerances`, `Risks`, `Progress`, `Surprises & discoveries`, `Decision log`,
and `Outcomes & retrospective` must be kept up to date as work proceeds.

Status: COMPLETE (implementation and gates; one post-delivery manual check —
zoom bounds in the windowed demo — remains open in Progress)

## Purpose / big picture

Thysalion is a voxel game built on four architectural planes — state, logic,
knowledge, and presentation — as described in
[thysalion-design.md](../thysalion-design.md) §6.1. Today the repository is a
single root crate containing only a stub library and a "hello world" binary.
Roadmap step 1.1 ([roadmap.md](../roadmap.md) §1.1) turns that stub into the
shared core every later capability spike builds on:

1. Task 1.1.1 converts the repository into a Cargo workspace with one crate
   per plane plus a `demos` crate, so that `make test` and `make lint` pass on
   the empty skeleton and the layout is recorded in
   [repository-layout.md](../repository-layout.md).
2. Task 1.1.2 implements the shared demo harness — an orthographic-isometric
   camera with four yaw quadrants and bounded zoom, input mapping, a
   diagnostics overlay (frame time, tick time, counters), and a screenshot key
   — proven by a `demo-empty` binary that opens a window, renders a ground
   plane, and reports diagnostics, with the harness API documented in the
   [developers' guide](../developers-guide.md).

After this change, a contributor can run `make demo`
(`cargo run -p thysalion-demos --features demo-empty --bin demo-empty`), see a
window showing a ground plane from the isometric camera, rotate the view
through the four yaw quadrants, zoom within bounds, read live frame diagnostics
in an overlay, and press a key to save a screenshot to disk. A maintainer can
add the next capability demonstration by adding one binary to the `demos` crate
and registering the harness plugins — no scaffolding rework. "No scaffolding
rework" is this step's real stability promise: adding a harness capability must
never require editing existing demos, enforced by all demos compiling under
`make all`.

This plan was revised after a six-lens design review (see the decision log and
the revision note at the end); the review's confirmed findings are folded into
the stages below rather than tracked separately.

## Constraints

Hard invariants that must hold throughout implementation. Violation requires
escalation, not workarounds.

- The four-plane authority table in thysalion-design.md §6.1 governs crate
  layering: presentation reads state; logic and knowledge are reached only via
  the state boundary; knowledge never participates in the frame loop. Crate
  dependency edges must not contradict it. The presentation plane must never
  depend on demo scaffolding.
- The camera contract in thysalion-design.md §8.2 is fixed: orthographic-
  isometric projection, exactly four allowed yaw quadrants, bounded zoom.
  `Quadrant` is therefore a closed (exhaustive) enum.
- Engine pin: Bevy 0.19 on wgpu 29 (thysalion-design.md §5.2, verified
  current on 2026-07-24). One Bevy version per development phase; never upgrade
  mid-feature. The single machine-readable source of truth for the version
  literal is `[workspace.dependencies]`; ADR-005 owns the pin rationale and
  other documents reference it. Dependencies use caret requirements (`"0.19"`),
  never wildcards or `>=`, per AGENTS.md.
- The Clippy policy in the root `Cargo.toml` (pedantic warn plus the
  explicit deny table) must survive the workspace conversion with equivalent
  coverage on every member crate. Lint suppressions must be tightly scoped with
  a stated reason; the graphics-crate allowance set is enumerated in ADR-005,
  not improvised per file.
- `make all` (check-fmt, lint, test, spelling) is the commit gate and must
  pass at every commit. Docs changes must additionally pass `make markdownlint`
  and `make nixie`. Red test states live only in the working tree, never in a
  commit (AGENTS.md quality gates).
- All documentation follows the
  [documentation style guide](../documentation-style-guide.md): en-GB-oxendict
  spelling, sentence-case headings, 80-column prose wrap, language identifiers
  on fenced blocks.
- Workflow contract-test rules: workflow-shape tests may assert pinned-SHA
  *patterns* and required flags, but never literal SHA values
  (developers-guide.md §"Workflow pins and Dependabot").
- Module hygiene per AGENTS.md: every module starts with a `//!` comment;
  public APIs carry Rustdoc; no file exceeds 400 lines. Filesystem access uses
  `cap_std`/`camino`, not `std::fs`/`std::path`.

## Tolerances (exception triggers)

- Scope: if the change grows beyond roughly 30 files or 2,500 net lines
  (excluding `Cargo.lock` and generated snapshots), stop and escalate.
- Dependencies: the approved new dependency set is `bevy` (0.19, with a
  curated feature list — see Stage A), and dev-dependencies `rstest`,
  `rstest-bdd` 0.6.0-beta3 (with `rstest-bdd-macros` and `rstest-bdd-harness`
  at the same version), and `cap-std`/`camino` where tests touch the
  filesystem. Any further external dependency (for example
  `leafwing-input-manager`, `bevy_panorbit_camera`, `insta`) requires
  escalation before adoption.
- Interfaces: renaming the root package `thysalion`, or changing the
  release artefact name consumed by `.github/workflows/release.yml` and the
  `[package.metadata.binstall]` template, requires escalation.
- Iterations: if `make lint` or `make test` still fails after three fix
  attempts on the same failure class, stop and escalate. A lint-policy clash
  that the ADR-recorded allowance set does not already cover counts as one
  failure class, not many.
- CI wall-clock: the first (cold-cache) CI run after Bevy lands is expected
  to exceed 30 minutes and is not an escalation trigger; escalate if a
  *warm-cache* run exceeds 30 minutes.
- Ambiguity: if the harness API design forces a choice that visibly
  constrains later spikes (phases 2–5) in a way this plan does not already
  record, stop and present options.

## Risks

- Risk: Bevy's default features drag in system build dependencies
  (`bevy_audio` → alsa headers, `bevy_gilrs` → libudev headers) that the four
  CI workflows do not install, so the first Bevy-dependent member breaks
  `make lint`, the coverage action, `coverage-main.yml`, and
  `act-validation.yml` at compile time. Local spikes hide this because
  developer machines already carry the headers. Severity: high. Likelihood:
  near-certain if unmitigated. Mitigation:
  `bevy = { default-features = false, features = [...] }` with a curated list
  that drops audio, gamepad, and glTF support the harness does not use;
  finalize the list in Stage A by building in a clean container without dev
  headers, and add any residual system packages (for example winit's windowing
  headers) to *all four* workflows in the same commit that adds the dependency.
  Record the list and its maintenance cost in ADR-005.
- Risk: the coverage ratchet (`with-ratchet: true` in `ci.yml`) fails this
  PR and then penalizes every later PR, because the windowed harness half
  (overlay, screenshot) and the demo binaries cannot execute in CI and land as
  permanently uncovered lines. Severity: high. Likelihood: high. Mitigation:
  exclude the windowed modules and the `demos` crate from coverage measurement
  (llvm-cov ignore patterns in the Makefile coverage target; verify the shared
  coverage action honours the same boundary) and document the headless coverage
  boundary in the developers' guide. If the shared action cannot express the
  exclusion, escalate before Commit 2.
- Risk: the strict lint table clashes with Bevy idioms well beyond the
  numeric group — `needless_pass_by_value` fires on system parameters (`Res<T>`,
  `Query`), `must_use_candidate` and `missing_const_for_fn` on pure camera
  functions — and the meta-lints (`allow_attributes*` are themselves denied)
  constrain how allowances can be written. Severity: medium. Likelihood: high.
  Mitigation: Stage A runs the full `make lint` against a throwaway
  harness-shaped crate to enumerate the real allowance set empirically; ADR-005
  records the full set and the attribute form the meta-lints accept. Simulation
  and world crates keep the full policy — float discipline there protects
  determinism (design §10.4).
- Risk: Whitaker (the dylint-based custom linter in `make lint`) has never
  been run against a Bevy-dependent workspace member; it recompiles the tree
  under its own driver and its lints are separate from Clippy's, so Clippy
  allowances do not cover them. Severity: medium. Likelihood: medium.
  Mitigation: include Whitaker in the Stage A lint spike; if it fails or adds
  an unacceptable per-run compile floor, escalate with measurements.
- Risk: `cross`-compiled release builds (six targets including FreeBSD)
  would break if the Bevy-dependent demo crates ever enter the release graph.
  Severity: high (release pipeline breaks at tag time, with no earlier signal).
  Likelihood: medium over the project's life. Mitigation: demos live in a
  separate crate precisely so the release build can be scoped; add
  `-p thysalion` to the release build line, a comment in `release.yml` stating
  demos must never enter the release graph, and a workflow-shape test asserting
  the `-p thysalion` scope (pattern assertion, compatible with the
  no-literal-SHA rule). Note the Stage A `cargo build -p thysalion` spike
  proves manifest scoping only, not the cross toolchain path.
- Risk: Bevy 0.19 compile times inflate CI. Cranelift and mold accelerate
  *local* dev builds only; the CI-dominating passes (clippy, rustdoc, Whitaker,
  LLVM coverage instrumentation) are untouched by them. Severity: medium.
  Likelihood: medium. Mitigation: the curated feature list is the main lever;
  only `harness` and `demos` depend on Bevy; measure the first warm run against
  the tolerance; watch the Actions cache for eviction as demos multiply.
- Risk: Bevy screenshots capture the previous frame's camera position
  (bevyengine/bevy issue 18230), making screenshot review misleading after a
  camera move. Severity: low. Likelihood: medium. Mitigation: trigger capture
  on key release and document the one-frame settling behaviour in the users'
  guide.

## Progress

- [x] (2026-07-24 19:00Z) Stage A: all five spikes complete. Lint reality
  including Whitaker (after the toolchain upgrade); Message and screenshot
  idioms compile; `-p thysalion` scoping proven (Bevy never compiled);
  rstest-bdd Bevy harness adapter green; clean-container `cargo check` *and*
  `cargo test` pass with the curated feature list and **zero additional system
  packages** — the feature trim eliminates the system-dependency risk entirely,
  so no workflow package installs are needed.
- [x] (2026-07-24 19:40Z) Stages B–D file authoring complete in the
  working tree (workspace manifests, six member crates, camera contract,
  harness two-plugin implementation, demo-empty, BDD suite and adapter,
  Makefile and workflow updates, ADR-005, repository-layout rewrite, guide
  sections, design §6.1 back-reference). The host then refused to spawn
  processes (`posix_spawn` EIO) until a session restart, so validation followed
  later.
- [x] (2026-07-24 20:40Z) Stages B–C validated after host recovery:
  `tests/stub.rs` deleted; fix cycle recorded in `Surprises & discoveries`
  (coverage attribute gating, `DirectionalLight` field rename, six lint
  findings, headless input expiry); `make check-fmt`, `make lint` (rustdoc,
  Clippy, Whitaker), `make test` (38/38), and `make spelling` all green. Logs:
  `/tmp/lint-thysalion-1-1.out`, `/tmp/test-thysalion-1-1.out`.
- [x] (2026-07-24 22:30Z) Stage C1/C2 delivery complete: code committed
  (`c7888ab` workspace + harness, `f6a4f0c` demo launch scoping) and pushed;
  windowed smoke run under WSLg confirmed the window opens and renders via
  software Vulkan without panicking; CodeRabbit agent review of the full branch
  (39 files) returned **zero findings**.
- [x] (2026-07-24 22:45Z) Stage D delivery: docs committed (ADR-005,
  repository-layout rewrite, developers'/users' guide sections, contents,
  design back-reference, roadmap 1.1.1/1.1.2 ticked); retrospective recorded
  below.
- [x] (2026-07-24 23:25Z) Post-delivery CI evidence: both pull-request
  CI runs green (13 m 28 s on the code push, 4 m 24 s warm cache on the docs
  push — see Artefacts); the docs-milestone CodeRabbit review also returned
  zero findings.
- [x] (2026-07-24 23:35Z) Manual verification, rotation: the
  maintainer ran the windowed demo and confirmed quadrant rotation looks good.
- [x] (2026-07-24 23:45Z) Manual verification, overlay and screenshot:
  the maintainer pressed F12 in the windowed demo and
  `screenshots/demo-empty-1784931927.png` landed on disk. The capture shows the
  isometric ground-plane diamond with the F3 overlay reading "14 fps, 70.91
  ms/frame, tick: n/a" — 14 fps is the expected llvmpipe software-rendering
  rate, and "tick: n/a" is correct because `TICK_TIME` is registered but
  nothing measures into it until the simulation plane exists.
- [x] (2026-07-25) CodeRabbit PR review round 1 (11 comments,
  CHANGES_REQUESTED) actioned: initial zoom now clamps into the configured
  bounds in `RigState::from_config`; screenshot filenames gained a
  process-local sequence counter so same-second captures cannot collide; the
  presentation crate's `float_arithmetic` expectation narrowed from module
  level to the two arithmetic functions plus a shared `assert_close` test
  helper; the zoom tests share an rstest `bounds` fixture; the workflow-shape
  test now rejects `shared-actions` references lacking `@<sha>`; five prose and
  grammar fixes across the docs; the plan status line now names the open
  zoom-bounds check explicitly. One suggestion declined with a reply: "an
  rstest-bdd" is correct because the letter r is pronounced with a leading
  vowel sound.
- [x] (2026-07-25) CodeRabbit PR review round 2 (7 comments) actioned:
  the windowed camera module's `float_arithmetic` expectation narrowed to the
  three arithmetic functions; the screenshot system now spawns one capture per
  queued action instead of coalescing; the per-demo feature convention is now
  real (`demo-empty = []` feature, `required-features` on the binary, and
  `make demo` passes `--features demo-$(DEMO)`); demo run commands in the
  ExecPlan and binary docs are workspace-qualified; the retrospective's
  open-items list trimmed to the zoom-bounds check; the users' guide names
  `SCREENSHOT_KEY` alongside `KEY_BINDINGS`; screenshot filename docs include
  the new sequence component; one ADR collocation fix. CodeRabbit withdrew the
  round-1 "an rstest-bdd" finding after the pronunciation-rule reply.
- [x] (2026-07-25) CodeRabbit PR review round 3 (2 comments; all
  round-1/2 threads confirmed resolved) actioned: screenshot capture-path
  generation gained unit and behavioural tests (distinct same-second paths; a
  mixed action batch spawns one capture per screenshot action), and slugs are
  now sanitized to `[A-Za-z0-9-]` in capture paths so a slug containing path
  separators cannot escape the screenshots directory. `make demo` input
  validation is intentionally delegated to Cargo: an unknown `DEMO` value fails
  with Cargo's own missing-feature/missing-binary error, which names the
  problem precisely.
- [x] (2026-07-25) CodeRabbit PR review round 4 (1 comment) actioned: the
  `make demo` target now validates `DEMO` before use — the value reaches the
  shell via an exported environment variable (never via make interpolation, so
  it cannot inject shell syntax) and a `case` guard whitelists `[a-z0-9-]+`
  slugs with a clear error. Verified: an injection attempt and an empty value
  fail with the guard's message; an unknown-but-well-formed slug still reaches
  Cargo's precise missing-feature error.
- [ ] Post-delivery: remaining manual verification (zoom bounds). This is
  the only open item; all review-bot findings raised on the pull request are
  actioned or explicitly declined with rationale.

## Surprises & discoveries

- Observation: Bevy 0.19 reworked its feature model. `default` is now the
  umbrella set `["2d", "3d", "ui", "audio"]`, and the `2d`/`3d`/`ui` umbrellas
  all pull `default_platform`, which hard-includes `bevy_gilrs` (libudev) and
  `wayland`. Evidence: `cargo metadata` feature tables for bevy 0.19.0. Impact:
  avoiding the gamepad/audio system dependencies requires composing
  fine-grained features (`default_app`, `common_api`, `bevy_render`,
  `bevy_pbr`, …) rather than subtracting from an umbrella; the curated list in
  `Interfaces and dependencies` reflects this.
- Observation: the development host (WSL2) itself lacks
  `wayland-client` headers, so the `wayland` feature fails to build even
  locally. Evidence: `wayland-sys` build-script failure in the Stage A spike.
  Impact: the harness ships X11-only windowing (`x11` feature; WSLg serves it
  via XWayland, and CI never runs windowed); `wayland` can be revisited when a
  target platform needs it.
- Observation: Whitaker's installed lint suite was built for
  `nightly-2025-09-18` (rustc 1.92), which cannot compile Bevy 0.19 (requires
  rustc ≥ 1.95), so `make lint` would fail on any Bevy-dependent member
  regardless of code quality. Evidence: Whitaker run failed with "bevy@0.19.0
  requires rustc 1.95.0". Impact: upgraded locally to whitaker-installer 0.2.7
  plus the rolling lint suite built for `nightly-2026-05-28` (the repository's
  pinned toolchain) and cargo-dylint 6.0.1 (sha256-verified from the Whitaker
  rolling release; cargo-dylint 5.0.0's driver does not compile against the new
  nightly). `ci.yml` must bump `WHITAKER_INSTALLER_VERSION` to 0.2.7, drop the
  removed `--cranelift` flag, and roll the Whitaker cache key.
- Observation: `MinimalPlugins` provides no input resources, so a
  headless app running the input-reading system fails Bevy's system-param
  validation (`ButtonInput<KeyCode>` and `AccumulatedMouseScroll` missing).
  Evidence: first BDD spike run panicked with "Resource does not exist". Impact:
  `HarnessCorePlugin` initializes those resources itself; this also lets
  headless tests inject synthetic key presses.
- Observation: `#[coverage(off)]` is still feature-gated on the pinned
  nightly. Evidence: E0658 on the harness windowed modules. Impact: the
  attribute is applied as `#[cfg_attr(coverage_nightly, coverage(off))]` with
  `#![cfg_attr(coverage_nightly, feature(coverage_attribute))]` — the canonical
  cargo-llvm-cov pattern — plus an `unexpected_cfgs` check-cfg entry in the
  workspace lints; the Makefile's `--ignore-filename-regex` remains as belt and
  braces for coverage runs that do not set the cfg.
- Observation: Bevy 0.19 renamed `DirectionalLight::shadows_enabled` to
  `shadow_maps_enabled` (with a separate `contact_shadows_enabled`). Evidence:
  E0560 in `demo-empty`; field list in bevy_light 0.19 source. Impact: one-line
  fix; migration guides lag the release.
- Observation: initializing input resources is necessary but not
  sufficient for headless input: without `InputPlugin`, nothing expires
  `just_pressed`/`just_released` between frames, so one synthetic press fired
  on every subsequent update (the rig rotated twice). Evidence: the
  `synthetic_key_press_drives_the_rig` scenario failed with
  `left: SouthWest, right: SouthEast`. Impact: `HarnessCorePlugin` adds a
  `Last`-schedule `clear_synthetic_input` system when `InputPlugin` is absent,
  giving synthetic input one-shot semantics; roadmap 1.3's CI scaffolding
  inherits this behaviour.
- Observation: the empirical lint-allowance set is smaller than feared:
  `float_arithmetic` (crate-level `#![expect]` in graphics crates) and
  `needless_pass_by_value` (per-system `#[expect]`) suffice; the other feared
  lints are satisfiable directly (`#[must_use]`, `const fn`, documentation).
  Whitaker additionally requires `//!` docs on *every* module, including
  `#[cfg(test)]` modules. Evidence: Stage A lint spike passes clippy and
  Whitaker cleanly with only those two expects.

## Decision log

- Decision: keep the root package `thysalion` as the workspace root package
  (a non-virtual workspace) and give it an explicit role: it is the phase-9
  integrated game binary and the future *composition root* — the one place that
  may depend on all four plane crates and own cross-plane wiring. It hosts no
  plane logic itself. Rationale: `.github/workflows/release.yml` hard-codes
  `BIN_NAME: thysalion`, `[package.metadata.binstall]` embeds the same name,
  and the roadmap's phase 9 deliverable is the `thysalion` binary.
  Structurally, a composition root is also how the design's cyclic data flow
  (ECS → circuit → ECS, circuit → store → ECS) stays acyclic at the crate
  level: cycles are resolved by the root wiring planes together, not by planes
  depending on each other. Date/Author: 2026-07-24, planning session;
  strengthened per design review (structural lens).
- Decision: `crates/world` is the designated dependency sink for shared
  state types. Plane-to-crate mapping: state plane → `thysalion-world`, logic
  plane → `thysalion-sim`, knowledge plane → `thysalion-knowledge`,
  presentation plane → `thysalion-presentation`. ADR-005 records this table and
  each crate's `//!` header repeats its own row, because two of the four crate
  names differ from the design's plane names. Rationale: the roadmap names the
  crates ("world/scene data, simulation, knowledge, presentation"); renaming to
  match the design's plane vocabulary would diverge from the roadmap instead.
  The explicit mapping removes the silent translation. Dependency-freedom of
  the plane crates is *staging*, not an invariant: world will need bevy (ECS
  types), sim needs dbsp, knowledge needs oxigraph, presentation needs
  bevy_voxel_world; ADR-005 names these eventual edges. Layering will be
  enforced by review against the ADR until edges exist to lint (cargo-deny bans
  are a candidate once phase 2 starts). Date/Author: 2026-07-24, per design
  review (structural lens).
- Decision: create all four plane crates now, empty, rather than the
  minimal three-crate workspace (root + harness + demos) that defers them.
  Rationale: the roadmap's thesis is spikes 2–5 radiating independently and
  possibly in parallel; pre-creating the plane crates removes the coordination
  race of two concurrent spikes inventing workspace structure. That — not
  "encoding the authority table," which empty crates cannot enforce — is the
  justification. The minimal-workspace alternative was reviewed and rejected on
  exactly this trade. Date/Author: 2026-07-24, per design review (alternatives
  checkpoint).
- Decision: the camera contract types (`Quadrant`, `ZoomBounds`, quadrant
  yaw mathematics) live in `thysalion-presentation`, not in the harness.
  Rationale: §8.2 makes the active quadrant a presentation input (octant
  culling swaps face sets by quadrant in phase 2). If the harness owned
  `Quadrant`, the shipping presentation plane would depend on demo scaffolding
  or duplicate the type. The harness consumes the contract:
  `harness → presentation` is a legal edge (demo scaffolding reads the
  presentation contract); `presentation → harness` never is. The contract
  module is pure mathematics, so `thysalion-presentation` stays dependency-free
  this step. Date/Author: 2026-07-24, per design review (structural lens).
- Decision: the demo harness is its own crate (`crates/harness`), exposing
  *two public plugins*: `HarnessCorePlugin` (headless-safe: camera state, input
  mapping, diagnostics registration) and `DemoHarnessPlugin` (core plus overlay
  UI and screenshot capture, for windowed demos). Rationale: separation keeps
  demo-only affordances out of the shipping presentation plane, and the
  two-plugin split makes the headless subset a public, documented contract that
  this step's tests and roadmap 1.3's CI scaffolding consume without reaching
  into internals. The split is also a compile-visible boundary: the headless
  modules import no render types, and the `MinimalPlugins` test is the
  enforcing guard. Date/Author: 2026-07-24, per design review (structural and
  contract lenses).
- Decision: `HarnessConfig` is `#[non_exhaustive]` and built via
  `HarnessConfig::new(slug)` plus chainable `with_*` methods; it carries a
  stable `slug` (used for screenshot filenames) separate from the window title.
  `HarnessAction` is `#[non_exhaustive]` and is a Bevy 0.19 *message*
  (`Messages<HarnessAction>` with `MessageReader`), not an observer event,
  because it is a buffered per-frame stream with multiple readers. `Quadrant`
  stays exhaustive (a domain invariant of §8.2) with documented `next()`/
  `prev()`/`yaw()` rather than ordinal arithmetic. Rationale: every later demo
  constructs `HarnessConfig` cross-crate; a bare struct literal would break all
  demos the first time a field is added, contradicting the
  no-scaffolding-rework promise. Date/Author: 2026-07-24, per design review
  (contract lens).
- Decision: harness diagnostics use Bevy's `DiagnosticsStore` with
  published `DiagnosticPath` constants — no bespoke stringly-named counter
  registry. The tick-time seam is defined now: the harness publishes a
  `TICK_TIME` diagnostic path that the overlay displays when present; the
  simulation (phase 4) writes to it via the composition root. This is a debug
  HUD, deliberately not the `metrics` crate (no exporter, no low-cardinality
  label discipline needed for an on-screen overlay). Rationale: typed paths
  avoid collision and typo divergence, reuse the engine's diagnostic smoothing,
  and give design §10.6's later per-operator counters a registration mechanism
  that does not change the overlay. Date/Author: 2026-07-24, per design review
  (contract lens).
- Decision: use plain Bevy input (`ButtonInput<KeyCode>`,
  `AccumulatedMouseScroll`) for harness input mapping, not
  `leafwing-input-manager`; build the quadrant camera rig by hand, not with
  `bevy_panorbit_camera`. Rationale: the harness needs a handful of fixed
  bindings; an input-mapping dependency adds upgrade surface for no present
  benefit (leafwing-input-manager 0.19 is confirmed Bevy-0.19-compatible should
  that change). bevy_panorbit_camera 0.35 provides *continuous* orbit; §8.2
  demands a discrete four-quadrant, orthographic, bounded-zoom rig —
  constraining a general tool down to a narrow contract is more friction than
  the pure mathematics. The harness owns chrome bindings
  (rotate/zoom/overlay/screenshot); demos own their own gameplay input — that
  boundary is documented in the developers' guide. Date/Author: 2026-07-24,
  planning session; panorbit rejection recorded per design review.
- Decision: demo binaries live in `crates/demos`, never in the root
  package's `src/bin`, and each demo's heavy dependencies are declared
  `optional = true` behind a per-demo feature with `required-features` on the
  `[[bin]]` target. Rationale: this and the release scoping are the same
  decision viewed twice — demos in root `src/bin` would put Bevy into the
  `cross build -p thysalion` graph and break six-target release builds. The
  per-demo feature convention costs nothing now (demo-empty needs only the
  harness) and prevents the demos crate becoming a union of every spike's
  dependencies (dbsp, oxigraph, bevy_voxel_world) that every `make demo` must
  compile. If the union still hurts by phase 6, the recorded migration path is
  crate-per-demo. Date/Author: 2026-07-24, per design review (alternatives and
  scaling lenses).
- Decision: plane crates ship as documented, dependency-free stubs with a
  `//!` header only — no placeholder public API, no stub doctests. Rationale:
  `missing_docs` fires on public items only, so an empty `lib.rs` with the
  plane/authority `//!` doc passes every gate; invented placeholder APIs are
  churn that phases 2–5 delete. The `todo!()` stub types needed to make Stage B
  tests compile live in `presentation` and `harness` (which Stage C2 fills),
  not in the plane stubs. Date/Author: 2026-07-24, per design review (viability
  lens).
- Decision: behavioural tests use `rstest-bdd` 0.6.0-beta3 with its
  extensible harness support: a small in-repo Bevy harness adapter (implementing
  `rstest_bdd_harness::HarnessAdapter` with `type Context = bevy::app::App`)
  builds the `MinimalPlugins` app with `HarnessCorePlugin` before steps run,
  and step functions borrow the app via the reserved
  `#[from(rstest_bdd_harness_context)]` fixture. The adapter lives in the
  harness crate's test support (`crates/harness/tests/support/`) for now;
  roadmap step 1.3.1 (headless Bevy CI scaffolding) is the natural point to
  promote it to a shared test-support crate. Unit-level mathematics (quadrant,
  zoom, input mapping) stays plain `rstest`. Rationale: user direction
  supersedes the earlier deferral; the adapter pattern follows the "third-party
  harness adapter cookbook" in the rstest-bdd users' guide (which sketches
  exactly this Bevy case), and it gives the headless behavioural test a Gherkin
  specification that 1.3's CI scaffolding inherits. Date/Author: 2026-07-24,
  user direction.
- Decision: windowing is X11-only for this step (`x11` feature, no
  `wayland`). Rationale: the `wayland` feature needs `libwayland-dev` at build
  time on every machine and CI runner, the development host (WSL2) lacks it,
  WSLg serves X11 clients via XWayland, and CI never runs the windowed path.
  Revisit when a deployment target requires native Wayland. Date/Author:
  2026-07-24, Stage A finding.
- Decision: upgrade Whitaker to installer 0.2.7 with the rolling lint
  suite for `nightly-2026-05-28` and cargo-dylint 6.0.1, locally and in
  `ci.yml` (version bump, drop the removed `--cranelift` flag, roll the cache
  key). Rationale: the previous suite's `nightly-2025-09-18` toolchain cannot
  compile Bevy 0.19 (rustc ≥ 1.95 required), which would fail `make lint`
  unconditionally; the rolling suite matches the repository's pinned toolchain
  exactly. Date/Author: 2026-07-24, Stage A finding.
- Decision: red states are assertion failures, not compile failures. Stage
  C1 creates the contract types with `todo!()` bodies so Stage B tests compile
  and fail meaningfully; the red state exists only in the working tree between
  Commits 1 and 2, and every commit passes `make all`. Rationale: a
  non-compiling test is indistinguishable from broken code and cannot satisfy
  AGENTS.md's commit gate. Date/Author: 2026-07-24, per design review
  (operations lens).

## Outcomes & retrospective

Delivered (2026-07-24): roadmap tasks 1.1.1 and 1.1.2 are complete. The
repository is now a six-member Cargo workspace with the root package as
composition root; the four plane crates are documented stubs with their
authority rows; `thysalion-presentation` owns the pure camera contract
(`Quadrant`, `ZoomBounds`); `thysalion-harness` ships the two-plugin contract
(`HarnessCorePlugin` headless-safe, `DemoHarnessPlugin` windowed); and
`demo-empty` opens a window, renders a lit ground plane, and reports
diagnostics. Thirty-eight tests pass, including four rstest-bdd scenarios
driving the harness through the Bevy adapter, and all deterministic gates plus
a zero-finding CodeRabbit review cleared both milestones.

What worked well:

- Stage A spikes earned their keep: every one of them changed the plan
  (curated Bevy features removed the system-package risk outright; the Whitaker
  toolchain incompatibility was found before, not after, the workspace
  conversion; release `-p` scoping was proven without ever compiling Bevy for a
  foreign target).
- The red test phase caught a real defect that review had not predicted:
  without `InputPlugin`, synthetic key presses never expire, so one press
  rotated the rig twice. The fix (`clear_synthetic_input` in `Last`) is now
  part of the headless contract.
- The six-expert design review moved structural decisions (Quadrant's
  home, the dependency sink, the composition root) out of the implementation
  loop entirely; implementation surfaced no structural rework.

What cost time:

- The strict lint set required several fix cycles (`shadow_reuse`,
  `format_push_string`, `missing_const_for_fn`, Whitaker's
  `no-unwrap-or-else-panic` interacting badly with `panic_in_result_fn` in
  tests). The lint-reality spike covered the production idioms but not the test
  idioms; future plans should lint a representative *test* file in the spike
  too.
- A mid-session host failure (`posix_spawn` EIO) split authoring from
  validation; the plan's file-by-file progress notes made resumption cheap,
  which vindicates the "update the living sections frequently" discipline.

Left open (tracked in Progress): the manual zoom-bounds check in the windowed
demo. Rotation, the overlay, the F12 screenshot, and the warm-cache CI duration
are all verified and recorded above. The open check does not block roadmap 1.2,
which builds on the scene contract rather than the harness chrome.

## Context and orientation

The repository is currently a single Cargo package named `thysalion` (edition
2024, nightly toolchain pinned in `rust-toolchain.toml`), with:

- `src/lib.rs` — a stub `greet()` function with a doctest.
- `src/main.rs` — a stub `main` printing a greeting.
- `tests/stub.rs` — a placeholder test explicitly marked for deletion.
- `Cargo.toml` — package metadata, cargo-binstall metadata, and a large
  `[lints.clippy]`/`[lints.rust]`/`[lints.rustdoc]` deny table.
- `Makefile` — the command surface (`make all`, `check-fmt`, `lint`,
  `test`, `typecheck`, `coverage`, `markdownlint`, `spelling`, `nixie`,
  `audit`). `TARGET ?= thysalion` drives `build`/`release` paths. AGENTS.md
  already documents `make test` as workspace-wide although the current Makefile
  line lacks `--workspace` — pre-existing drift this conversion aligns, not a
  regression.
- `.github/workflows/` — `ci.yml` (gates via Makefile targets plus a
  shared coverage action with a coverage ratchet), `release.yml` (cross-builds
  `BIN_NAME: thysalion` for six targets, currently with no package scoping),
  `coverage-main.yml`, `act-validation.yml` (runs `make test WITH_ACT=1`; the
  `WITH_ACT` flag is dead — it appears nowhere in the Makefile. Inherited
  drift, noted here so it is not mistaken for a regression; fixing it is out of
  scope).
- `docs/` — the design document, ADRs 001–004, the roadmap, and the guides
  named throughout this plan.

Terms used below:

- A *plane* is one of the four architectural layers of
  thysalion-design.md §6.1: state (Bevy ECS and voxel data), logic (the DBSP
  incremental circuit), knowledge (the oxigraph RDF store), and presentation
  (rendering, lighting, UI).
- The *demo harness* is the shared scaffolding every capability
  demonstration binary uses: window setup, isometric camera rig, input mapping,
  diagnostics overlay, and screenshot capture.
- A *capability demonstration* (roadmap terminology) is a runnable binary
  in the `demos` crate that trials one cluster of concepts, for example
  `demo-diorama` (phase 2) or `demo-sim` (phase 4). `demo-empty` is the
  degenerate first one: harness plus ground plane, nothing else.
- A Bevy *message* (0.17+) is the buffered, multiple-reader event stream
  (`Messages<T>`, `MessageReader<T>`); the term *event* is reserved for
  observer-targeted events. The harness action stream is a message.

Relevant skills for the implementer: `rust-router` (entry point for Rust
questions), `arch-crate-design` (workspace and feature-flag structure),
`arch-decision-records` (ADR-005 is written to its Y-statement form),
`rust-unit-testing` (rstest fixture shape), `rust-errors` (fallible screenshot
and filesystem paths), `hexagonal-architecture` (port and adapter discipline
for the plane boundaries), `juice-it-or-lose-it` (the camera settle animation
and overlay feel are graded by acceptance criterion 3), `leta` (semantic code
navigation; a workspace is already registered), and `commit-message` /
`pr-creation` for delivery mechanics. Relevant documents:
[AGENTS.md](../../AGENTS.md) (coding standards, testing policy, error handling,
observability), [developers-guide.md](../developers-guide.md) (local workflow,
tooling prerequisites),
[documentation-style-guide.md](../documentation-style-guide.md), and
[reliable-testing-in-rust-via-dependency-injection.md](../reliable-testing-in-rust-via-dependency-injection.md).
For the behavioural-test harness, the authoritative reference is the
rstest-bdd users' guide, §"Third-party harness adapter cookbook" (local
checkout: `../rstest-bdd/docs/users-guide.md` relative to the repository's
parent directory; also published with the crate), together with its ADR-005
(harness adapter crates) and ADR-007 (harness context injection).

## Plan of work

### Stage A: verification spikes and dependency pinning (no tracked changes)

Confirm, in throwaway directories under the session scratchpad, the assumptions
this plan depends on. Spikes 1–3 must run in a clean container (for example
`podman run rust:*`) *without* desktop dev headers, because a developer
machine's installed headers are exactly what hid the CI risk.

1. Feature list: build a minimal harness-shaped crate against
   `bevy = { version = "0.19", default-features = false, features = [...] }`
   with a candidate list covering windowing (winit, x11/wayland), core
   pipeline, pbr, ui, text with the default font, png (screenshot encoding),
   asset handling, diagnostics, and multi-threading — and *excluding* audio,
   gamepad, and glTF. Record the final list and any system packages it still
   needs; those packages must be added to all four workflows in Commit 2.
2. Lint reality: run the full `make lint` (Clippy with the workspace deny
   table *and* Whitaker) against the spike crate containing one Bevy system,
   one plugin, and the camera mathematics. Enumerate every lint that fires on
   idiomatic Bevy code; that empirical set becomes ADR-005's graphics-crate
   allowance list, including the attribute form the meta-lints
   (`allow_attributes*`) accept.
3. API idioms: confirm the 0.19 message API (`Messages<HarnessAction>`,
   `MessageReader`) and the screenshot idiom
   (`commands.spawn(Screenshot::primary_window()).observe(save_to_disk(path))`)
   compile.
4. Release scoping: `cargo build -p thysalion` in a mock workspace with a
   Bevy-dependent member confirms `-p` scoping keeps Bevy out of the build
   graph. This proves manifest resolution only — the cross toolchain path is
   exercised for real at the next tag; the workflow-shape test in Stage C1 is
   the standing guard.
5. BDD harness adapter: confirm `rstest-bdd` 0.6.0-beta3's harness contract
   compiles against Bevy 0.19 — a `HarnessAdapter` with
   `type Context = bevy::app::App` whose `run` builds a `MinimalPlugins` app
   and calls `request.run(app)`, plus one step borrowing
   `#[from(rstest_bdd_harness_context)] app: &mut App`. (The cookbook's example
   targets an older Bevy; this spike proves the pairing.)

Go/no-go: if any spike fails, stop and revise this plan before touching the
tree.

### Stage B: red tests

Write the failing tests in their final locations. They compile against the
`todo!()`-bodied contract types Stage C1 creates, and fail assertions — that is
the red state, and it exists only in the working tree.

- `crates/presentation` unit tests (rstest): quadrant yaw angles are
  exactly the four quarter-turns; `next()`/`prev()` from each quadrant land on
  the adjacent quadrant and compose to identity; `ZoomBounds` construction
  rejects `min >= max` and non-positive bounds; zoom requests clamp to the
  bounds; the orthographic scale for a zoom level is monotonic.
- `crates/harness/src/input.rs` unit tests (rstest): each bound key maps
  to the intended `HarnessAction`; unbound keys map to nothing.
- `crates/harness/tests/headless.rs` (rstest-bdd 0.6.0-beta3): the headless
  behavioural specification, run through the in-repo Bevy harness adapter
  (`crates/harness/tests/support/bevy_harness.rs`, implementing
  `HarnessAdapter` with `type Context = bevy::app::App`; its `run` builds the
  `MinimalPlugins` app with `HarnessCorePlugin` and passes it to
  `request.run(app)`). Step functions borrow the app via
  `#[from(rstest_bdd_harness_context)]`. Scenarios bind with
  `#[scenario(path = "tests/features/harness.feature", harness = BevyHarness)]`.
  The feature file `crates/harness/tests/features/harness.feature`:

  ```gherkin
  Feature: Demo harness headless core

    Scenario: Rotating advances one quadrant
      Given a headless harness app in the north-east quadrant
      When a rotate-left action is sent and the app updates
      Then the camera rig is in the north-west quadrant

    Scenario: Diagnostics are registered
      Given a headless harness app in the north-east quadrant
      When the app updates
      Then the harness diagnostic paths are registered
  ```

  (Quadrant naming in steps follows `Quadrant::next()`'s documented cyclic
  order; adjust the expected quadrant if the documented order differs at
  implementation time, keeping the feature and the `next()` contract in
  lock-step.)

### Stage C1: workspace conversion (task 1.1.1)

Root `Cargo.toml`:

- Add a `[workspace]` table with `members = ["crates/*"]` and
  `resolver = "3"`.
- Move the `[lints.*]` tables to `[workspace.lints.*]`; the root package
  and every member adopt `[lints] workspace = true`.
- Add `[workspace.dependencies]` entries for `bevy` (0.19,
  `default-features = false`, the Stage A feature list), `rstest`, and the
  `rstest-bdd` 0.6.0-beta3 family (`rstest-bdd`, `rstest-bdd-macros`,
  `rstest-bdd-harness`), so member crates inherit one pinned version. Members
  must not declare passthrough features onto Bevy without weighing that `make`
  gates run `--all-features` (note recorded in ADR-005).
- Add `[workspace.package]` keys (edition, license, repository) inherited
  by members.
- The root package keeps its binstall metadata and stub binary; its
  `lib.rs` `//!` doc states the composition-root role ("the phase-9 integrated
  binary; hosts no plane logic").

New member crates, each with a `//!` header stating its plane, its row of the
§6.1 authority table, its plane-to-crate name mapping, and its eventual heavy
dependency (staged, not yet added):

- `crates/world` — package `thysalion-world`. State plane; designated
  dependency sink for shared state types. Eventually: bevy (ECS types).
- `crates/sim` — package `thysalion-sim`. Logic plane. Eventually: dbsp.
- `crates/knowledge` — package `thysalion-knowledge`. Knowledge plane.
  Eventually: oxigraph.
- `crates/presentation` — package `thysalion-presentation`. Presentation
  plane. At this step, it gains the pure camera-contract module (Stage C2 types
  with `todo!()` bodies land here at this stage, so Stage B tests compile).
  Eventually: bevy, bevy_voxel_world.
- `crates/harness` — package `thysalion-harness`. Demo scaffolding
  (Stage C2). Depends on bevy and thysalion-presentation.
- `crates/demos` — package `thysalion-demos`. One `src/bin/<name>.rs` per
  capability demonstration; depends on thysalion-harness; per-demo heavy
  dependencies are optional features with `required-features` on each `[[bin]]`.

Dependency edges this step: `demos → harness → {bevy, presentation}`. Plane
crates other than presentation have no edges; `presentation → world` (when it
appears in phase 2) is the worked-example layering edge ADR-005 uses to anchor
the rule.

Makefile:

- Add `--workspace` to the primary `test` invocation, `lint`'s clippy
  line, `typecheck`, and `coverage`, so the root package no longer masks
  members.
- Keep `TARGET ?= thysalion` for `build`/`release`; add a `demo` target
  (`cargo run --bin demo-$(DEMO)` with `DEMO ?= empty`) as the documented way
  to launch demonstrations.
- Add the coverage exclusion for windowed harness modules and the demos
  crate (llvm-cov ignore patterns) to the `coverage` target.
- Run `mbake validate Makefile` and `checkmake` after editing.

Workflows (all in the same commit that introduces the Bevy dependency):

- `release.yml`: scope the cross build with `-p thysalion` and add the
  "demos must never enter the release graph" comment.
- Add the Stage A system-package set to `ci.yml`, `coverage-main.yml`,
  `act-validation.yml`, and `release.yml` as needed by the feature list.
- Add a workflow-shape test (pattern assertion per the developers' guide
  rules) that the release build line carries `-p thysalion`.
- Verify the coverage action's ratchet tolerates the exclusion boundary;
  escalate if not.

Remove `tests/stub.rs` (its own comment mandates deletion once real tests exist
— the Stage B tests are those tests).

Go/no-go: `make all` passes on the skeleton. Stage B tests fail assertions
(red) in the working tree; Commits 1 and 2 land together as one gated delivery
only if the `todo!()` stubs would otherwise fail `make lint` (`todo!` is
compatible with the deny table; `unreachable` and `panic_in_result_fn` do not
fire on it in non-`Result` fns — verified in the Stage A lint spike).

### Stage C2: harness crates and `demo-empty` (task 1.1.2)

`crates/presentation` camera contract (pure mathematics, no Bevy types):

```rust
/// The four allowed camera yaw quadrants (design §8.2). Closed by design.
pub enum Quadrant { NorthEast, SouthEast, SouthWest, NorthWest }

impl Quadrant {
    pub fn next(self) -> Self { /* cyclic, documented order */ }
    pub fn prev(self) -> Self { /* inverse of next */ }
    pub fn yaw_radians(self) -> f32 { /* quarter-turn angles */ }
}

/// Validated zoom range newtype: construction enforces 0 < min < max.
pub struct ZoomBounds { /* private fields; TryFrom construction */ }
```

`crates/harness` public API (the *harness contract* the roadmap names):

```rust
/// Headless-safe scaffolding: camera rig state, input mapping,
/// diagnostics registration. Consumed by MinimalPlugins tests and by
/// roadmap 1.3's CI scaffolding.
pub struct HarnessCorePlugin;

/// Windowed scaffolding: HarnessCorePlugin plus the overlay UI and the
/// screenshot capture systems.
pub struct DemoHarnessPlugin;

/// Declarative settings a demo supplies. Non-exhaustive; construct with
/// new() and chainable with_* methods so adding fields never breaks
/// existing demos.
#[non_exhaustive]
pub struct HarnessConfig {
    /* slug (stable, filenames), window_title, zoom_bounds,
       initial_quadrant */
}

/// Buffered per-frame action stream (a Bevy message, not an observer
/// event). Non-exhaustive: later phases add chrome bindings without
/// breaking demo matches.
#[non_exhaustive]
pub enum HarnessAction {
    RotateLeft, RotateRight, ZoomIn, ZoomOut,
    ToggleOverlay, Screenshot,
}

/// Published diagnostic paths (bevy DiagnosticsStore). TICK_TIME is the
/// seam phase 4's simulation writes through the composition root; the
/// overlay displays it when present. Design §10.6's per-operator
/// counters register additional paths the same way.
pub mod diagnostics {
    pub const FRAME_TIME: DiagnosticPath;
    pub const TICK_TIME: DiagnosticPath;
}
```

Internal structure (each module ≤ 400 lines, `//!` docs, rstest tests alongside;
`self_named_module_files` is denied, so systems split as sibling modules, not
`mod.rs`):

- `camera.rs` — systems that slerp the camera transform toward the active
  quadrant's yaw at a fixed pitch, using `Projection::Orthographic` with
  `ScalingMode::FixedVertical { viewport_height }` scaled by the zoom level.
  Pure mathematics stays in `thysalion-presentation`; this module imports no
  render-world types beyond transform and projection components.
- `input.rs` — the binding table (`Q`/`E` rotate, wheel or `+`/`-` zoom,
  `F3` overlay, `F12` screenshot) translated into `Messages<HarnessAction>`;
  the pure mapping function is separate from the reading system for
  testability. This table is the single source the two guides reference.
- `overlay.rs` (windowed) — `FrameTimeDiagnosticsPlugin` wiring and a
  `Text`/`Node` overlay refreshed at ~5 Hz via a `Timer`, reading smoothed
  `DiagnosticsStore` values — not re-formatted every frame.
- `screenshot.rs` (windowed) — on `HarnessAction::Screenshot` (key
  release), ensure `screenshots/` exists (cap_std `create_dir_all`), spawn
  `Screenshot::primary_window()` with a `save_to_disk` observer writing
  `screenshots/<slug>-<timestamp>.png`, and log the absolute path at info level
  so a silent write failure is visible.
- `lib.rs` — the two plugin assemblies; the headless module set imports no
  render types (the `MinimalPlugins` test is the enforcing guard).

`crates/demos/src/bin/demo-empty.rs`: `DefaultPlugins` (window title from
config) with `DemoHarnessPlugin`, a `Plane3d` ground mesh with a
`StandardMaterial`, and a directional light. Nothing else.

Go/no-go: Stage B tests pass; the manual verification script in
`Validation and acceptance` succeeds on a developer machine.

### Stage D: documentation, ADR, refactor, and cleanup

- New ADR `docs/adr-005-workspace-crate-layout.md` (Y-statement per the
  `arch-decision-records` skill and the style guide's ADR template). It owns:
  the crate-per-plane layout and plane-to-crate name table; the non-virtual
  workspace and composition-root role of the root package; `world` as
  dependency sink and the staged eventual dependencies; the Bevy pin policy and
  the curated feature list with its maintenance trade-off; the lint-inheritance
  policy including the empirically derived graphics-crate allowance set; the
  harness two-plugin contract; the demos-crate/release-scoping linkage; and the
  note that member crates must not add Bevy passthrough features without costing
  `--all-features`. Reference ADR-005 from thysalion-design.md §6.1.
- Rewrite `docs/repository-layout.md` (a full rewrite, not an edit):
  workspace tree, per-crate purpose and ownership prose, workspace-level versus
  per-crate configuration, the root package's role statement.
- Extend `docs/developers-guide.md` with a "Demo harness" section: the
  two-plugin contract and the headless/windowed boundary (including the
  coverage boundary), `HarnessConfig` construction, the binding table
  reference, how to add a new demo binary (including the per-demo feature/
  `required-features` convention), the behavioural-test convention (the
  rstest-bdd Bevy harness adapter, where the feature files live, and how steps
  borrow the app context), the tick-time seam, and the screenshot one-frame-lag
  caveat.
- Extend `docs/users-guide.md` with a task-oriented "Running the demos"
  section: `make demo`, the key bindings (referencing the same table), the
  overlay, screenshots.
- Update `docs/contents.md` with the new ADR.
- Tick the 1.1.1 and 1.1.2 checkboxes in `docs/roadmap.md`.
- Forward the open question to step 1.2's planner (recorded here and in
  ADR-005's consequences): whether the scene format becomes a leaf
  `thysalion-scene` crate or stays inside `thysalion-world`, given §7.2's
  optional concept IRI links the palette to the knowledge plane.
- Refactor pass per AGENTS.md heuristics as a separate commit if needed.

## Concrete steps

All commands run from the repository root. Commit after each numbered delivery,
gating each commit with `make all` (and `make markdownlint`, `make nixie`,
`make fmt` when docs changed). Use the `commit-message` skill for messages.
Sequential execution only — no parallel gate runs.

1. Stage A spikes in the scratchpad directory (feature list and lint
   spikes inside a clean container); no commit. Record transcripts in
   `Artefacts and notes`.
2. Commit 1 — workspace conversion: workspace tables and lint moves, the
   six member crates (plane stubs with `//!` docs only; presentation's contract
   types and harness's contract types with `todo!()` bodies), Makefile updates,
   workflow updates (system packages, release scoping, shape test), delete
   `tests/stub.rs`, add the Stage B test files. If the `todo!()` stubs cannot
   pass `make lint`/`make test` in committed form, fold Commits 1 and 2 into
   one gated delivery and record the observed red transcripts here first.

   ```sh
   make all 2>&1 | tee /tmp/gate-all.out
   ```

   Expected: format, clippy (workspace-wide), Whitaker, tests, and spelling
   pass.
3. Commit 2 — harness implementation and `demo-empty`. First edit:
   add `screenshots/` to `.gitignore` (before any manual run can strand a PNG).
   Then:

   ```sh
   cargo test -p thysalion-harness -p thysalion-presentation
   # red (assertion failures) before implementation, green after
   make all
   make demo                         # manual check, developer machine
   ```

4. Commit 3 — documentation and ADR:

   ```sh
   make fmt && make markdownlint && make nixie
   ```

5. Update this plan's living sections at every stopping point.

## Validation and acceptance

Task 1.1.1 acceptance: `make test` and `make lint` pass on the workspace
skeleton (empty plane crates included, proven by clippy running with
`--workspace`), and `docs/repository-layout.md` describes the new tree.

Task 1.1.2 acceptance, automated:
`cargo test -p thysalion-harness -p thysalion-presentation` passes, including
the rstest-bdd scenarios of `crates/harness/tests/features/harness.feature` run
through the Bevy harness adapter: a `MinimalPlugins` app with
`HarnessCorePlugin` steps through a rotate message and the quadrant change and
registered diagnostic paths are observed.

Red-green-refactor evidence to record in `Artefacts and notes`: the Stage B
test command failing with assertion errors before Stage C2 (with the failure
text) — for the BDD scenarios, the runner command
`cargo test -p thysalion-harness --test headless` failing on the Then steps
before implementation — the same commands passing after, and the post-refactor
gate run.

Task 1.1.2 acceptance, manual (developer machine with a display):

1. `make demo` opens a window titled per
   `HarnessConfig`.
2. A ground plane is visible from an isometric viewpoint.
3. `Q`/`E` rotate the view through exactly four quadrants; the camera
   settles rather than snapping.
4. Zoom in and out stops at the configured bounds.
5. `F3` toggles an overlay showing frame time and counters; values
   update at the throttled cadence.
6. `F12` writes a PNG under `screenshots/` named with the demo slug; the
   image matches the view (allowing the documented one-frame settle); the
   absolute path is logged.

Record the manual run's screenshot as review evidence in the PR.

Quality criteria: `make all` green; `make markdownlint` and `make nixie` green
on docs; CI green including the coverage ratchet with the documented exclusion
boundary; no lint suppressions beyond the ADR-recorded allowance set; every new
module has `//!` docs; public API rustdoc examples that construct a Bevy `App`
are marked `no_run` (each compiled doc example links a full engine binary,
serially); compiled doc examples are reserved for the pure camera and zoom
mathematics.

## Idempotence and recovery

All steps are additive file creation or in-place edits under version control;
re-running a step overwrites to the same state. If a gate fails mid-delivery,
fix forward or `git restore` the affected paths; no step is destructive. The
only generated artefact is `Cargo.lock`, which Cargo regenerates
deterministically from the manifests. Screenshots land in `screenshots/`,
ignored before any manual run (Commit 2, first edit).

## Artefacts and notes

- Stage A: the spike crate compiled `cargo check --all-targets` and passed
  `cargo test` in a clean `rust:latest` container with the curated feature list
  and no extra system packages; `cargo build -p thysalion` in a mock workspace
  finished in two seconds without ever compiling Bevy.
- Red-green evidence: the honest red observed after host recovery was the
  `synthetic_key_press_drives_the_rig` scenario failing with
  `left: SouthWest, right: SouthEast` — a genuine defect (without `InputPlugin`,
  `just_pressed` never expires, so one synthetic press rotated twice). Green
  after adding the `clear_synthetic_input` end-of-frame system: 38/38 tests pass
  (`/tmp/test-thysalion-1-1.out`). The remaining Stage B tests were authored
  before any implementation ran but their individual red states were not
  captured, because the authoring session lost its host before `cargo` could
  execute; this is recorded rather than reconstructed.
- Lint fix cycle (one failure class per plan tolerance, six findings):
  rustdoc private-item link in `rig.rs`; `missing_const_for_fn` on
  `ZoomBounds::clamp`; `shadow_reuse` in `workflow_shape.rs` and `config.rs`;
  `format_push_string` and an unfulfilled `float_arithmetic` expectation in
  `overlay.rs`; Whitaker `no-unwrap-or-else-panic` in `workflow_shape.rs`
  (resolved with `.expect(...)` inside `#[test]` functions — combining `?` with
  `assert!` trips `panic_in_result_fn`).
- CodeRabbit agent reviews: code milestone (2026-07-24, 39 files) and
  docs milestone (2026-07-24, 41 files) both completed with zero findings.
- CI durations on the pull request: first Bevy-bearing run (code
  milestone push) 13 minutes 28 seconds; warm-cache run (docs milestone push) 4
  minutes 24 seconds — both well inside the 30-minute tolerance, and the
  Whitaker 0.2.7 installer path worked in CI unmodified.
- Manual verification screenshot:
  `screenshots/demo-empty-1784931927.png` (untracked; `screenshots/` is
  git-ignored), captured by the maintainer with F12 on 2026-07-24. Isometric
  ground-plane diamond with the F3 overlay showing live frame diagnostics under
  llvmpipe software rendering.

## Interfaces and dependencies

- `bevy` 0.19 (workspace dependency; used by `thysalion-harness` and
  `thysalion-demos` only), `default-features = false` with the Stage A curated
  feature list (windowing, core pipeline, pbr, ui, text, png, assets,
  diagnostics, multi-threading; excluding audio, gamepad, glTF). Key APIs:
  `bevy::diagnostic::{FrameTimeDiagnosticsPlugin, DiagnosticsStore, DiagnosticPath}`,
  `bevy::render::view::screenshot::{Screenshot, save_to_disk}`,
  `Projection::Orthographic` with `ScalingMode::FixedVertical`,
  `ButtonInput<KeyCode>`, `AccumulatedMouseScroll`, and `Messages<T>`/
  `MessageReader<T>`.
- `rstest` (workspace dev-dependency) per AGENTS.md testing policy;
  `rstest-bdd`, `rstest-bdd-macros`, and `rstest-bdd-harness` at
  `"0.6.0-beta3"` (workspace dev-dependencies) for the behavioural
  specification, using the extensible harness support (`HarnessAdapter`/
  `ScenarioRunRequest` and the reserved `rstest_bdd_harness_context` fixture);
  `cap-std`/`camino` for the screenshot directory handling per AGENTS.md
  filesystem policy.
- The types named in Stage C2 (`HarnessCorePlugin`, `DemoHarnessPlugin`,
  `HarnessConfig`, `HarnessAction`, `diagnostics::{FRAME_TIME, TICK_TIME}` in
  `thysalion_harness::`; `Quadrant`, `ZoomBounds` in
  `thysalion_presentation::`) must exist by the end of Stage C2 — they are the
  harness contract the roadmap requires and the developers' guide documents.
- dbsp, oxigraph, and bevy_voxel_world are deliberately *not*
  dependencies of this step; their pins (design §5) are re-verified when phases
  2, 4, and 5 adopt them, and their destination crates are named in ADR-005.

## Revision note (2026-07-24)

Revised after a six-lens Logisphere design review before any implementation.
What changed: camera contract types (`Quadrant`, `ZoomBounds`) moved from the
harness to `thysalion-presentation` (presentation must never depend on demo
scaffolding); the harness became a public two-plugin contract
(`HarnessCorePlugin` headless, `DemoHarnessPlugin` windowed); `HarnessConfig`
became non-exhaustive with builder construction and a stable slug;
`HarnessAction` corrected to a Bevy 0.19 message; the stringly counter registry
was replaced by `DiagnosticsStore` paths with a defined tick-time seam; Bevy
moved to `default-features = false` with a Stage A-derived feature list and
system packages added to all four workflows (the plan as first drafted would
have failed its own CI); the coverage ratchet gained a documented headless
exclusion boundary; the lint allowance is now derived empirically in Stage A
and recorded in ADR-005; release scoping gained a workflow-shape guard;
per-demo optional features with `required-features` became the demos-crate
convention; plane-crate stubs dropped placeholder APIs; rstest-bdd was
deferred; red states became assertion failures against `todo!()` stubs; and the
skill/document signposting was extended. Why: confirmed review findings (two
structural, three contract, two operational blockers among them). Effect on
remaining work: Stage A grew two spikes (clean-container feature list, lint
enumeration); Stages B–D are otherwise unchanged in sequence.

## Revision note (2026-07-24, second)

What changed: the earlier decision to defer `rstest-bdd` was reversed on user
direction. The headless behavioural test is now an rstest-bdd 0.6.0-beta3
specification (`crates/harness/tests/features/harness.feature`, embedded in
Stage B) run through an in-repo Bevy harness adapter that uses the crate's
extensible harness support (`HarnessAdapter` with
`type Context = bevy::app::App`, steps borrowing the app via the reserved
`rstest_bdd_harness_context` fixture), following the third-party harness
adapter cookbook in the rstest-bdd users' guide. The `rstest-bdd` family joined
the approved dependency set and `[workspace.dependencies]`; Stage A gained a
spike proving the adapter compiles against Bevy 0.19; the developers' guide
section now documents the behavioural-test convention. Why: user requirement to
standardize behavioural testing on rstest-bdd from the outset, seeding roadmap
1.3.1's headless CI scaffolding. Effect on remaining work: Stage B's headless
test is authored as feature file plus steps plus adapter; unit-level
mathematics remains plain rstest; no other stage changes.
