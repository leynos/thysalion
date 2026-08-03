# Developer Guide

This guide explains the contributor workflow for the generated Thysalion
project.

## Local Workflow

Use `make all` as the public entrypoint for formatting, linting, and tests.
`make lint` runs rustdoc, Clippy, and Whitaker. `make test` prefers
`cargo nextest run` and falls back to `cargo test` when cargo-nextest is not
available. `make audit` derives the Rust workspace root with `cargo metadata`,
logs workspace member manifests, and runs `cargo audit` once from the workspace
root. `make coverage` uses `cargo llvm-cov` with `lld`.

GitHub Actions Act validation lives in `.github/workflows/act-validation.yml`.
The main `.github/workflows/ci.yml` workflow deliberately does not run
`make test WITH_ACT=1`; the separate Act workflow runs those slower
container-backed checks in parallel.

## Tooling

Development builds use Cranelift for debug code generation. On Linux targets,
`.cargo/config.toml` configures clang to link with `mold` so debug builds link
quickly. Coverage generation uses `lld` because LLVM coverage tooling expects
LLVM-compatible linker behaviour.

Both of those defaults are wrong for coverage, and `make coverage` displaces
each of them rather than expecting the developer to. `RUSTFLAGS` replaces the
config's target flags wholesale, which is what takes `mold` out of the picture;
`mold` cannot be used here because it does not carry the instrumentation
sections `llvm-cov` reads. Cranelift is displaced separately, through
`CARGO_PROFILE_DEV_CODEGEN_BACKEND`, because no rustflag can reach a profile
setting and rustc refuses `-C instrument-coverage` under Cranelift outright.
`CARGO_UNSTABLE_CODEGEN_BACKEND` accompanies it so that the throwaway project
`trybuild` generates — which sits under `CARGO_TARGET_DIR` and so may never see
this repository's `.cargo/config.toml` — accepts the same override.

A system `lld` is therefore convenient but not required: `COVERAGE_LLD_DIR`
falls back to the `ld.lld` every rustup toolchain ships. Set
`COVERAGE_CODEGEN_BACKEND` or `COVERAGE_LLD_DIR` to opt out on a host whose own
toolchain already works.

Install `clang`, `lld`, `mold`, `python3`, and `cargo-audit` before running the
full generated workflow locally on Linux.

## Demo harness

Every capability demonstration binary in `crates/demos` builds on the shared
harness in `crates/harness` (`thysalion-harness`). The harness is a two-plugin
contract, decided in [ADR 005](adr-005-workspace-crate-layout.md):

- `HarnessCorePlugin` is headless-safe: camera-rig state (`RigState`),
  input mapping, and diagnostics registration. It runs under `MinimalPlugins`
  with no window or graphics processing unit — this is what tests and the
  future continuous-integration scaffolding (roadmap step 1.3) consume. The
  core initializes the input resources it reads, so headless tests can inject
  synthetic key presses.
- `DemoHarnessPlugin` adds the windowed half: the orthographic-isometric
  camera entity (four yaw quadrants with a smooth settle, bounded zoom), the
  diagnostics overlay, and screenshot capture. It registers the core itself;
  demos add only `DemoHarnessPlugin`.

Configuration is supplied through `HarnessConfig::new(slug)` plus chainable
`with_*` methods. The struct is `#[non_exhaustive]`: new fields arrive with
defaults and never break existing demos. The `slug` names screenshot files; the
window title is presentation-only.

Harness systems run in the public `HarnessSet` system sets (`Core`, then
`Windowed`) in the `Update` schedule; order demo systems relative to those sets
rather than naming harness system functions.

Key bindings live in one place — `thysalion_harness::input::KEY_BINDINGS` (plus
`SCREENSHOT_KEY`, which fires on release) — and both guides reference that
table rather than duplicating it. `HarnessAction` is a buffered Bevy *message*
(multiple readers per frame) and is `#[non_exhaustive]`; match with a wildcard
arm.

Diagnostics flow through Bevy's `DiagnosticsStore` under the typed paths in
`thysalion_harness::diagnostics`. The `TICK_TIME` path (`thysalion/tick_time`)
is the seam the simulation plane writes from phase 4 (via the composition
root); the overlay shows `tick: n/a` until a measurement exists. Later counters
(for example, design §10.6's per-operator trace sizes) register additional
paths the same way.

To add a new demo: create `crates/demos/src/bin/demo-<name>.rs`, declare any
heavy dependencies `optional = true` behind a `demo-<name>` feature with
`required-features` on the `[[bin]]`, build a `HarnessConfig`, add
`DemoHarnessPlugin`, and spawn the demo's own content. Run it with
`make demo DEMO=<name>`; the target whitelists demo names derived from the
binaries on disk and rejects anything else before Cargo runs
(`tests/demo_guard.rs` pins that guard).

### Headless testing and the coverage boundary

Behavioural tests use `rstest-bdd` (0.6.0-beta3) with a Bevy harness adapter
(`crates/harness/tests/headless/support.rs`) that builds a `MinimalPlugins` app
with `HarnessCorePlugin` and hands it to steps via the reserved
`rstest_bdd_harness_context` fixture. Feature files live in
`crates/harness/tests/features/`. Unit-level mathematics uses plain `rstest`;
generated-input properties (zoom clamping, the rig's action-sequence model) use
`proptest`; and compile-time contracts (for example, struct-literal
construction of `#[non_exhaustive]` harness types being rejected) are pinned
with `trybuild` cases under `crates/harness/tests/ui/`.

The windowed half is behaviourally tested too, without a window or graphics
device: `crates/harness/src/windowed_tests.rs` runs `DemoHarnessPlugin` under
`MinimalPlugins`, where camera and UI text components are plain entity data.
The module documents its isolation seams (the `OverlayTimer` brink seam and the
plugin-owned screenshot `CaptureSequence` resource).

The windowed modules (camera, overlay, screenshot) and demo binaries cannot
execute in continuous integration; they carry `#[coverage(off)]` and the
Makefile's coverage target mirrors the same boundary with an ignore pattern. Do
not count new windowed code against the coverage ratchet — keep logic testable
headless and leave only thin windowed shims excluded.

Screenshots (`F12`, on key release) save to
`screenshots/<slug>-<timestamp>-<sequence>.png` (git-ignored). Bevy screenshots
can lag the camera by one frame (bevyengine/bevy issue 18230); let the view
settle before capturing.

## Scene fixtures

Fixtures live in two places: layered-text sources under
`assets/scenes/src/<name>/`, and their compiled output in `assets/scenes/`
(`*.scene.json`, `*.provenance.json` sidecars, and `assets/scenes/knowledge/`
TriG sources). The compiled documents are committed build artefacts,
deliberately: a contributor with no `uv` must still be able to build, test, and
run the demos. The authoring sources are the review surface; the compiled JSON
is not. The format itself is decided in
[ADR 006](adr-006-scene-document-model.md) and referenced in full in
[World plane architecture](world-plane-architecture.md).

### The authoring format

`scene.toml` declares the scene's dimensions, chunk size, palette, lighting,
entities, and knowledge, plus `content_origin` and `content_extent` — the
sub-box the layer rasters cover inside the declared extent. `legend.toml` maps
one character to one palette name. `layers/z###.txt` supplies one text raster
per populated layer; an absent layer is air, and the files present need not be
contiguous.

### Raster rules

- A raster covers the content sub-box only, never the declared extent.
- Row 0 is `content_origin.y`, column 0 is `content_origin.x`, and `y`
  increases downward as the file reads — this is how a layer looks viewed from
  above.
- `z<nnn>.txt` supplies the layer at `content_origin.z + nnn`.
- Every character in a raster must appear in `legend.toml`; an unlisted
  character is an error, never a silent air.
- A short row is an error rather than being padded: padding would hide a
  truncated edit.
- Trailing whitespace is stripped before a row is measured.

### Human units

`scene.toml` accepts human units where the compiled document stores integers:
`azimuth = "17.45deg"` compiles to `1745` centi-degrees, and
`probe_spacing = "2m"` compiles to `2000` millimetres.

### A worked example: `bare-cell`

`assets/scenes/src/bare-cell/` is the smallest fixture: a 32 x 32 x 32 scene
with a 4 x 4 x 2 content box at the origin. Its `legend.toml` maps `.` to air
and `#` to `stone-block`:

```toml
"." = "air"
"#" = "stone-block"
```

`layers/z000.txt`, the layer at `content_origin.z + 0`, is a solid stone floor:

```text
####
####
####
####
```

`layers/z001.txt`, the layer at `content_origin.z + 1`, is a hollow ring one
layer up:

```text
#..#
....
....
#..#
```

### Regenerating fixtures

`make scenes` compiles the sources into `assets/scenes/`. `make scenes-check`
regenerates into a temporary directory and compares byte for byte, so a
hand-edited fixture or a stale generator run cannot go unnoticed.
`make scripts-test` runs the generator's own pytest suite under
`scripts/tests/`. All three are wired into `make all`.

### Checking a scene document

`crates/world/examples/scene-check.rs` is a deliberately thin wrapper over
`thysalion_world::check`:

```sh
cargo run -p thysalion-world --example scene-check -- \
    assets/scenes/keep-interior.scene.json
```

It exits `0` for a valid scene, `1` when the document is wrong (it failed
validation, or it failed to parse), `2` when the document or one of its
resources could not be read, and `64` for a command-line usage error — a
contributor scripting against the tool needs those codes to hand. See "Reading
a diagnostic report" in [World plane architecture](world-plane-architecture.md)
for how to read the tool's output.

## Binary assets and Git LFS

The repository stores binary reference assets — currently the concept art in
`references/` — as [Git LFS](https://git-lfs.com/) attachments, tracked by the
`references/*.png` rule in `.gitattributes`. Install `git-lfs` and run
`git lfs install` before cloning (or run `git lfs pull` after a clone made
without it); otherwise the tracked files check out as small text pointer stubs
rather than images.

The tracking pattern is case-sensitive: `references/*.png` does not match
uppercase `.PNG` files. Add a matching rule before accepting uppercase
reference assets, or rename them to the lowercase extension.

When adding a new binary asset type, add a matching `git lfs track` rule to
`.gitattributes` in the same commit as the first asset, and confirm the staged
file is a pointer with `git lfs ls-files` before pushing.

## Spelling policy

Markdown uses en-GB-oxendict spelling enforced by the pinned `typos` release.
The tracked `typos.toml` is generated from the estate-wide shared dictionary
and the narrow repository overlay in `typos.local.toml`. Run `make spelling` to
refresh the ignored local shared-base cache when the published source is newer,
regenerate the tracked configuration, and check maintained prose.

### Security audit ignores

Security audit jobs may set `CARGO_AUDIT_IGNORES` for narrowly scoped RustSec
advisories that affect unused or tooling-only dependency paths. Keep each
ignore tied to a documented runtime impact analysis, and remove it when the
affected dependency leaves the graph or the project starts using the advised
runtime path.

## Workflow pins and Dependabot

Dependabot owns the upgrade of GitHub Actions and reusable workflows, including
calls into `leynos/shared-actions`. Contract tests that assert a caller's exact
commit SHA create a lockstep dependency: every time Dependabot opens a bump PR,
the test fails until a human edits the pinned constant to match. That defeats
the purpose of automated dependency updates and turns a routine bump into a
manual chore.

Contract tests may still verify the *shape* of a reusable-workflow caller. They
must not verify the specific SHA value.

- Do assert the workflow references the correct reusable workflow path.
- Do assert the ref is pinned to a full 40-character commit SHA, not a
  mutable branch such as `main` or `rolling`.
- Do assert the expected `on:` triggers, least-privilege `permissions:`, and
  the inputs the caller relies on.
- Do not hard-code the current SHA value as an expected string. Match it with
  a pattern instead.
- Do not fail a test purely because Dependabot bumped the pinned SHA.

```python
import re

SHA_RE = re.compile(r"^[0-9a-f]{40}$")

def test_uses_pinned_full_sha(caller_step):
    ref = caller_step["uses"].split("@")[-1]
    assert SHA_RE.match(ref), f"expected a 40-hex commit SHA, got {ref!r}"
```

If a workflow's behaviour genuinely depends on a feature only present from a
particular commit onwards, express that as a comment or a changelog note, not
as a test assertion on the SHA string.
