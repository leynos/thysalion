# User Guide

This guide explains how to use the generated Thysalion project after rendering
it from the template.

## Generated Tooling

Generated projects use Rust 2024, a pinned nightly toolchain, strict lint
settings, and documented starter code. Library projects render `src/lib.rs`.
Application projects render `src/main.rs`, `src/lib.rs`, release automation, and
`[package.metadata.binstall]` metadata for binary installation.

Development builds use Cranelift for debug code generation. On Linux targets,
`.cargo/config.toml` configures clang to link with `mold` so local debug builds
link quickly. Coverage generation uses `lld` instead because LLVM coverage
tools expect LLVM-compatible linker behaviour.

## Makefile Targets

The generated `Makefile` exposes these public targets:

- `make all` runs formatting checks, linting, tests, and spelling checks.
- `make check-fmt` verifies Rust formatting.
- `make lint` runs rustdoc, Clippy, and Whitaker with warnings denied.
- `make test` runs `cargo nextest run` when cargo-nextest is installed and
  falls back to `cargo test` otherwise. All projects also run doctests.
- `make build` builds the debug target.
- `make release` builds the release target.
- `make coverage` writes `lcov.info` using `cargo llvm-cov` and `lld`.
- `make audit` derives the Rust workspace root with `cargo metadata` and runs
  `cargo audit` once from that root.
- `make markdownlint` checks Markdown files and enforces en-GB-oxendict
  spelling through the pinned `typos` release.
- `make spelling` refreshes the shared Oxford dictionary when its published
  source is newer than the ignored local cache, generates `typos.toml`, and
  checks Markdown prose.
- `make nixie` validates Mermaid diagrams.

Install `clang`, `lld`, `mold`, `python3`, and `cargo-audit` before running the
full generated workflow locally on Linux.

- `make demo` runs a capability demonstration binary (`DEMO=empty` by
  default, so `make demo DEMO=empty` and `make demo` are equivalent).

## Running the demos

Thysalion ships one runnable demonstration binary per capability spike. The
first is `demo-empty`: it opens a window, renders a ground plane from the
isometric camera, and reports live diagnostics.

```sh
make demo            # runs demo-empty
make demo DEMO=empty # explicit form; later demos use their own names
```

Inside any demo:

- `Q` and `E` rotate the view anticlockwise and clockwise through the
  four camera quadrants; the camera settles smoothly rather than snapping.
- `+`/`-` or the mouse wheel zoom in and out within the demo's bounds.
- `F3` shows or hides the diagnostics overlay (frames per second, frame
  time, and simulation tick time once a simulation exists — until then it reads
  `tick: n/a`).
- `F12` (on release) saves a screenshot to
  `screenshots/<demo>-<timestamp>.png` and logs the absolute path. Screenshots
  capture the settled view; immediately after a camera move the image can trail
  the screen by one frame, so pause briefly before capturing.

The binding table is defined once in the harness source
(`thysalion_harness::input::KEY_BINDINGS`); if this list ever disagrees with
the code, the code wins and this guide needs updating.
