# ADR 005: Workspace crate layout and the demo harness contract

## Status

Accepted

## Date

2026-07-24

## Context and problem statement

Thysalion's design ([thysalion-design.md](thysalion-design.md) §6.1) divides
the game into four planes — state, logic, knowledge, and presentation — and its
roadmap delivers capability spikes (phases 2–5) that radiate independently from
a shared core, each shipping a runnable demonstration binary. The repository
began as a single stub crate. Before any spike starts, the project must fix the
crate layout, the layering mechanism that keeps the planes' cyclic *data* flow
acyclic at the *crate* level, and the scaffolding contract every demonstration
shares. These decisions are hard to reverse once several spikes build on them.

## Decision drivers

- Spikes 2–5 may proceed in parallel; two concurrent spikes must not race
  to invent workspace structure.
- The design's data flow is cyclic (ECS → circuit → ECS; circuit → store
  → ECS), but Cargo crate dependencies must remain acyclic.
- The release pipeline cross-compiles a single `thysalion` binary for six
  targets, including FreeBSD, where the graphics stack does not build.
- The strict workspace lint policy must survive the conversion, while
  presentation-plane geometry is inherently floating point.
- Continuous integration has no display and no graphics processing unit;
  a coverage ratchet fails pull requests that add unexecuted lines.

## Decision outcome

### Workspace shape

The repository is a non-virtual Cargo workspace (`resolver = "3"`). The root
package `thysalion` remains: it is the phase-9 integrated game binary and the
*composition root* — the only place that may depend on all four plane crates
and own cross-plane wiring. It hosts no plane logic. Members live under
`crates/`:

| Crate                    | Plane          | Authoritative for                       | Eventual heavy dependency  |
| ------------------------ | -------------- | --------------------------------------- | -------------------------- |
| `thysalion-world`        | state          | components, voxel grid, material fields | `bevy` (ECS types)         |
| `thysalion-sim`          | logic          | all derived state                       | `dbsp`                     |
| `thysalion-knowledge`    | knowledge      | lore, beliefs, quests, dialogue facts   | `oxigraph`                 |
| `thysalion-presentation` | presentation   | meshes, lighting textures, UI           | `bevy`, `bevy_voxel_world` |
| `thysalion-harness`      | (demo tooling) | demo scaffolding                        | `bevy`                     |
| `thysalion-demos`        | (demo tooling) | one binary per capability demonstration | per-demo, feature-gated    |

Table 1: plane-to-crate mapping. Two crate names differ from the design's plane
names (state → `world`, logic → `sim`) because the roadmap names the crates;
each crate's `//!` header restates its row.

The plane crates are created now, empty, so parallel spikes inherit their homes
instead of coordinating structure later. Their dependency-freedom is staging,
not an invariant: the eventual heavy dependencies are listed above and adopted
by the phase that needs them.

### Layering

Cycles are broken by two rules: `thysalion-world` is the dependency sink for
shared state types (planes may depend on it, never the reverse), and the root
package is the only crate that may depend on every plane.
`presentation → world` (read-only) is the canonical legal edge. The
presentation plane must never depend on `thysalion-harness`: the camera contract
(`Quadrant`, `ZoomBounds`) therefore lives in `thysalion-presentation`, where
phase 2's octant culling will consume it, and the harness depends on
presentation. Until enough edges exist to lint mechanically (`cargo-deny` bans
are the candidate), layering is enforced by review against this record.

### Demo harness contract

`thysalion-harness` exposes two public plugins: `HarnessCorePlugin`
(headless-safe: rig state, input mapping, diagnostics registration — runnable
under `MinimalPlugins` with no window) and `DemoHarnessPlugin` (core plus
camera entity, overlay, and screenshot capture). The split is a compile-visible
boundary: headless modules import no render types, and the headless behavioural
suite is the enforcing guard. `HarnessConfig` and `HarnessAction` are
`#[non_exhaustive]` so the harness grows without editing existing demos.
Diagnostics flow through Bevy's `DiagnosticsStore` under published
`DiagnosticPath` constants; the `thysalion/tick_time` path is the seam the
simulation plane writes from phase 4.

### Demos and the release graph

Demo binaries live only in `thysalion-demos`. Each demo's heavy dependencies
must be `optional = true` behind a per-demo feature with `required-features` on
its `[[bin]]`, so no demo pays for another's graph; if the union still hurts by
phase 6, the migration path is crate-per-demo. The release build is scoped
`-p thysalion`, asserted by a workflow-shape test, so demos never enter the
six-target cross-build.

### Dependency and lint policy

`[workspace.dependencies]` is the single source of truth for version literals;
this record owns the rationale for the Bevy pin (one version per phase,
upgraded deliberately — design §5.2). Bevy is consumed with
`default-features = false` and a curated feature list because the 0.19 umbrella
features hard-include gamepad (libudev) and Wayland system dependencies; the
curated list builds in a bare container with no system development headers.
Windowing is X11-only until a target platform needs native Wayland. Member
crates must not declare passthrough features onto Bevy without costing
`--all-features` (the Make gates enable it).

The lint table lives in `[workspace.lints]` and every member inherits it
(`[lints] workspace = true`). The graphics-crate allowance set, enumerated
empirically against the full gate, is exactly:

- `clippy::float_arithmetic` — module-level `#![expect]` in geometry and
  overlay modules, and
- `clippy::needless_pass_by_value` — per-system `#[expect]`, because
  Bevy system parameters (`Res<T>`, `Query`) are taken by value.

Because `clippy::allow_attributes` is denied, allowances use `#[expect]` with
reasons. Simulation and world crates take no numeric allowances: float
discipline there protects determinism (design §10.4).

Windowed harness modules and demo binaries carry `#[coverage(off)]` (and the
Makefile mirrors the boundary with `--ignore-filename-regex`): they cannot
execute in continuous integration, and counting their lines would poison the
coverage ratchet for every later pull request.

## Options considered

- **Minimal workspace** (root, harness, demos; plane crates deferred):
  honest about empty crates, but pushes structure invention into the parallel
  spike phases — the coordination race this record exists to prevent.
- **Virtual workspace** (no root package): churns the release pipeline
  and binstall metadata for no benefit, and discards the natural composition
  root.
- **Harness inside the presentation crate**: leaks demo-only affordances
  (screenshot key, debug overlay) into the shipping plane and inverts the
  permitted dependency direction.
- **`bevy_panorbit_camera` for the rig**: provides continuous orbit;
  design §8.2 demands a discrete four-quadrant, orthographic, bounded-zoom rig
  — constraining the general tool exceeds the cost of the pure mathematics.
- **`leafwing-input-manager` for input**: six fixed chrome bindings do
  not justify the upgrade surface; the typed-action module keeps the swap cheap
  if rebinding ever matters (version 0.19 is confirmed Bevy-0.19-compatible).

## Consequences

- Adding a spike means adding one binary to `thysalion-demos` behind a
  feature, registering the harness plugins, and nothing else.
- The first phase to wire two planes together does so in the root
  package, not by adding a plane-to-plane edge; review enforces this until
  `cargo-deny` bans take over.
- The curated Bevy feature list is maintenance surface: revisit it at
  every Bevy upgrade, in this record.
- Open question forwarded to step 1.2: whether the scene format becomes
  a leaf `thysalion-scene` crate or stays inside `thysalion-world`, given
  palette entries carry optional knowledge-plane concept Internationalized
  Resource Identifiers (design §7.2).
