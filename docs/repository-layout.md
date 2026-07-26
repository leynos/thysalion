# Repository layout

This document describes the Thysalion repository layout. It is the canonical
reference for where source code, tests, configuration, automation, and
long-lived documentation belong. The workspace shape is decided in
[ADR 005](adr-005-workspace-crate-layout.md); this document records the
resulting tree.

## Top-level tree

The tree below shows the repository structure. It is intentionally compact and
omits build output such as `target/`.

```plaintext
.
├── .cargo/
│   └── config.toml
├── .github/
│   ├── dependabot.yml
│   └── workflows/
│       ├── act-validation.yml
│       ├── ci.yml
│       ├── coverage-main.yml
│       └── release.yml
├── crates/
│   ├── world/          # state plane: thysalion-world
│   ├── sim/            # logic plane: thysalion-sim
│   ├── knowledge/      # knowledge plane: thysalion-knowledge
│   ├── presentation/   # presentation plane: thysalion-presentation
│   ├── harness/        # demo scaffolding: thysalion-harness
│   └── demos/          # demo binaries: thysalion-demos
├── docs/
│   ├── contents.md
│   ├── developers-guide.md
│   ├── repository-layout.md
│   ├── users-guide.md
│   └── ...
├── references/         # concept art (Git LFS)
├── scripts/            # uv-run helper scripts
├── src/
│   ├── lib.rs
│   └── main.rs
├── tests/
│   └── workflow_shape.rs
├── AGENTS.md
├── Cargo.toml
├── LICENSE
├── Makefile
├── README.md
├── clippy.toml
├── codecov.yml
└── rust-toolchain.toml
```

## Workspace shape

The repository is a non-virtual Cargo workspace. The root package `thysalion`
is the phase-9 integrated game binary and the composition root: the only crate
that may depend on all four plane crates and own cross-plane wiring; it hosts
no plane logic. Release artefacts build from the root package alone
(`-p thysalion`). Members live under `crates/`; each member inherits the shared
lint table (`[lints] workspace = true`) and dependency pins
(`[workspace.dependencies]`, the single source of truth for version literals)
from the root `Cargo.toml`.

## Path responsibilities

- `.cargo/config.toml`: Configures Cargo defaults for local development,
  including Linux linker and code-generation settings. Applies workspace-wide.
- `.github/dependabot.yml`: Configures automated dependency update checks.
- `.github/workflows/act-validation.yml`: Runs workflow validation through
  `act` separately from main CI.
- `.github/workflows/ci.yml`: Runs the continuous integration gates via the
  Makefile targets.
- `.github/workflows/coverage-main.yml`: Uploads coverage from pushes to
  the main branch.
- `.github/workflows/release.yml`: Cross-builds and publishes the
  `thysalion` release binary for six targets. The build is scoped
  `-p thysalion` so demo crates never enter the release graph; a workflow-shape
  test asserts the flag stays present.
- `crates/world/` (`thysalion-world`): State plane — voxel grid, scene
  documents, palettes (content from roadmap step 1.2). Designated dependency
  sink for shared state types: planes may depend on it, never the reverse.
- `crates/sim/` (`thysalion-sim`): Logic plane — the DBSP circuit (content
  from phase 4).
- `crates/knowledge/` (`thysalion-knowledge`): Knowledge plane — the
  oxigraph store wrapper (content from phase 5).
- `crates/presentation/` (`thysalion-presentation`): Presentation plane —
  currently the pure camera contract (`Quadrant`, `ZoomBounds`); meshing,
  lighting, and atmosphere passes arrive in phases 2–3. Must never depend on
  `thysalion-harness`.
- `crates/harness/` (`thysalion-harness`): Shared demo scaffolding — the
  two-plugin harness contract (`HarnessCorePlugin` headless,
  `DemoHarnessPlugin` windowed). Demo tooling, not a plane.
- `crates/demos/` (`thysalion-demos`): One binary per capability
  demonstration under `src/bin/`. Per-demo heavy dependencies are feature-gated
  with `required-features` on each `[[bin]]`.
- `docs/`: Holds long-lived reference documentation, guides, style rules,
  and design material.
- `docs/contents.md`: Indexes the documentation set and should be updated
  when documentation files are added, renamed, or removed.
- `docs/users-guide.md`: Explains how to use the project, its public build
  and test commands, and the demo binaries.
- `docs/developers-guide.md`: Explains the contributor workflow, local
  tooling, and the demo harness API.
- `docs/repository-layout.md`: Documents the repository tree and path
  responsibilities.
- `docs/execplans/`: Execution plans for roadmap steps.
- `references/`: Concept art stored via Git LFS (see the developers'
  guide).
- `scripts/`: Python helper scripts run through `uv` (see
  `docs/scripting-standards.md`).
- `src/lib.rs`: Composition-root library support and doctested examples.
- `src/main.rs`: The `thysalion` application entrypoint.
- `tests/`: Root-package integration tests, including the workflow-shape
  contract tests (`tests/workflow_shape.rs`).
- `AGENTS.md`: Provides repository-specific working instructions for agents
  and contributors.
- `Cargo.toml`: Defines the workspace, shared dependency pins, the shared
  lint tables, and the root package metadata.
- `LICENSE`: Records the project licence text.
- `Makefile`: Provides the public build, lint, test, coverage, demo, and
  documentation validation commands.
- `README.md`: Introduces the project and gives the shortest useful
  getting-started path.
- `clippy.toml`: Configures Clippy lint behaviour that is not expressed
  directly in `Cargo.toml`. Applies workspace-wide.
- `codecov.yml`: Configures coverage reporting behaviour.
- `rust-toolchain.toml`: Pins the Rust toolchain channel and required
  components for the whole workspace.

## Ownership boundaries

- Keep plane logic in its plane crate under `crates/`; the root package
  wires planes together and hosts no plane logic itself.
- Keep demo-only affordances (overlay, screenshot key, camera rig systems)
  in `crates/harness`; the presentation plane must never depend on it.
- Keep demo binaries in `crates/demos/src/bin/`, never in the root
  package's `src/bin/` — the release build's `-p thysalion` scoping depends on
  it.
- Keep black-box integration tests next to the crate they exercise; keep
  workflow-shape contract tests under the root `tests/`.
- Keep reusable documentation under `docs/`. Update `docs/contents.md`
  whenever a documentation file is added, renamed, or removed.
- Keep build and validation entrypoints in `Makefile`; prefer adding or
  extending a Make target over documenting an ad hoc command.
- Keep continuous integration workflow changes under `.github/workflows/`
  and dependency-update policy under `.github/dependabot.yml`.
- Do not commit generated build output such as `target/`, coverage
  artefacts, screenshot captures (`screenshots/`), or local editor state.

## Updating this document

Update this document when the repository gains a new top-level directory, a new
workspace member, a new long-lived documentation category, a new workflow file,
or a changed ownership boundary that would otherwise make the tree misleading.
