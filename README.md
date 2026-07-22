# 🕯️ Thysalion

*An isometric voxel RPG in the lineage of Ultima VII: a chunky little world
that simulates first and asks questions later.*

Thysalion renders dense voxel dioramas — market towns at late afternoon,
harbours at blue hour, swamps at twilight — and fills them with NPCs who know
things, fires that spread, rain that wets the cobbles, and light that behaves.
Under the hood it is a Rust engine with an unusual spine: game rules run as
incrementally maintained dataflow views, and world knowledge lives in a
queryable graph.

______________________________________________________________________

## Why Thysalion?

- **Simulation-first.** Perception, fire spread, rumour, and light level
  are derived facts, not scripted events. Rules are expressed once, as
  incremental views over the world, and stay correct as the world changes.
- **Deterministic by construction.** The [DBSP](https://docs.rs/dbsp)
  circuit that derives all game state is reproducible tick for tick — replays
  and honest saves come with the architecture.
- **NPCs with beliefs, not flags.** Each character holds a named RDF graph
  of what they think is true. It can be wrong. Gossip, lies, and stale news are
  first-class citizens of an [oxigraph](https://docs.rs/oxigraph) store.
- **The good kind of lighting.** Three tiers — clustered forward direct
  light, a flood-fill voxel light field, and diffuse global illumination probes
  ray-marched through the voxel grid itself — with no ray-tracing hardware
  required.
- **Bounded dioramas, not infinite noise.** Scenes are authored, loaded
  whole, and meshed at load on [Bevy](https://bevy.org/) 0.19.

______________________________________________________________________

## Quick start

Thysalion is at the design stage; the engine is under construction. The build
and test scaffolding works today:

### Build and test

```bash
git clone git@github.com:leynos/thysalion.git
cd thysalion
make test    # cargo test --workspace
make lint    # clippy, warnings denied
```

### Read the design

The fastest way into the project is the design document:

```bash
$EDITOR docs/thysalion-design.md
```

It defines the four-plane architecture — state (Bevy ECS), logic (DBSP
circuit), knowledge (oxigraph), and presentation — and the contracts between
them.

______________________________________________________________________

## Features (designed, in build order)

- Bounded voxel scenes with palette-driven materials, per-face
  passability, and prototype-inherited entities
- A single DBSP circuit as the authority for all derived game state,
  with seven named verification invariants
- Per-NPC belief graphs, storylet dialogue gated by SPARQL preconditions,
  and TriG as the designer-facing authoring format
- Tiered lighting: day/night, weather, wet-surface response, and
  leak-resistant probe GI on mid-range GPUs
- Material simulation as stencil-updated scalar fields (heat, moisture,
  fuel) co-located with the voxel grid

______________________________________________________________________

## Learn more

- [Technical design](docs/thysalion-design.md) — the architecture and its
  rationale
- [Decision records](docs/contents.md) — ADRs 001–004 for the load-bearing
  choices
- [Roadmap](docs/roadmap.md) — capability-demo spikes and integration
  milestones
- [Users' guide](docs/users-guide.md) — build and test commands
- [Developers' guide](docs/developers-guide.md) — contributing workflow

______________________________________________________________________

## Licence

ISC — see [LICENSE](LICENSE) for details.

______________________________________________________________________

## Contributing

Contributions welcome! Please read [AGENTS.md](AGENTS.md) and the
[developers' guide](docs/developers-guide.md) before diving in.
