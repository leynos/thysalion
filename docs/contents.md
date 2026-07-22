# Documentation contents

[Documentation contents](contents.md) is the index for Thysalion's
documentation set.

## Design

- [Thysalion technical design](thysalion-design.md) explains the four-plane
  architecture (state, logic, knowledge, presentation), the voxel world model,
  rendering, tiered lighting, simulation, and verification commitments.
- [ADR 001: meshed voxel rendering pipeline](adr-001-meshed-voxel-rendering-pipeline.md)
  records the choice of greedy-meshed rasterization over raymarching.
- [ADR 002: DBSP as the logic authority](adr-002-dbsp-as-logic-authority.md)
  records the choice of an incremental dataflow circuit for all derived game
  state.
- [ADR 003: oxigraph as the knowledge plane](adr-003-oxigraph-knowledge-plane.md)
  records the choice of an event-queried RDF store for lore, beliefs, and
  quests.
- [ADR 004: tiered lighting with software-ray-marched DDGI](adr-004-tiered-lighting-software-ddgi.md)
  records the three-tier lighting architecture without hardware ray tracing.
- [Roadmap](roadmap.md) sequences delivery as independent capability-
  demonstration spikes off a shared core, with two integration milestones.

## Project guides

- [User guide](users-guide.md) explains how to use the generated project and
  its public build and test commands.
- [Developer guide](developers-guide.md) explains the local workflow and
  implementation tooling for contributors.
- [Repository layout](repository-layout.md) explains the generated project's
  top-level files, directories, and ownership boundaries.
- [Documentation style guide](documentation-style-guide.md) defines the
  spelling, structure, Markdown, Architecture Decision Record (ADR), Request
  for Comments (RFC), and roadmap conventions used by this documentation set.

## Rust reference material

- [Reliable testing in Rust via dependency injection](reliable-testing-in-rust-via-dependency-injection.md)
  explains how to keep tests deterministic by injecting environment, clock,
  filesystem, and other external dependencies.
- [Rust doctest Don't Repeat Yourself guide](rust-doctest-dry-guide.md)
  explains how to write maintainable, executable Rust documentation examples.
- [Rust testing with `rstest` fixtures](rust-testing-with-rstest-fixtures.md)
  explains fixture-based, parameterized, and asynchronous testing with `rstest`.

## Engineering practice

- [Complexity antipatterns and refactoring strategies](complexity-antipatterns-and-refactoring-strategies.md)
  explains cognitive complexity, the bumpy-road antipattern, and refactoring
  approaches for maintainable code.
- [Scripting standards](scripting-standards.md) explains the preferred Python
  scripting stack, command execution patterns, and test expectations for helper
  scripts.
