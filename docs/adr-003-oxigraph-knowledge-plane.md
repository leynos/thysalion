# Architectural decision record (ADR) 003: oxigraph as the knowledge plane

## Status

Accepted, 2026-07-22. An in-memory oxigraph store holds lore, beliefs, quests,
and dialogue facts, queried on events only; reactive derived facts are computed
in the DBSP circuit, not by the store.

## Date

2026-07-22.

## Context and problem statement

Ultima-VII-style NPCs know different, possibly wrong, things, and narrative
content is gated on world state ([design](thysalion-design.md) §11). This
"meaning" layer changes slowly, is authored by designers, and must be queryable
with expressive patterns. The question is where it lives: an RDF quad store
with SPARQL (oxigraph), bespoke fact components in the ECS, or a Datalog fact
base.

## Decision drivers

- Named graphs model per-NPC belief sets natively, including divergence
  from ground truth; RDF-star annotates individual statements with provenance
  for gossip mechanics.
- TriG/Turtle are diffable, designer-editable authoring formats (design
  G4) with standard parsers and serializers — saves become plain text.
- SPARQL `ASK`/`SELECT` map directly onto storylet preconditions in the
  quality-based narrative model.
- Oxigraph's documented weaknesses (pre-1.0, single maintainer,
  unoptimized SPARQL joins, no inference) are real and must shape the
  integration rather than be wished away.

## Options considered

### Option A: oxigraph, event-driven, in-memory

Store + query only; no per-frame access; derivation via load-time SPARQL rules
and circuit-computed reactive facts; TriG for authoring and saves.

### Option B: ECS fact components

Facts as components/resources; queries as hand-written system logic.

### Option C: Datalog fact base as primary store

`ascent`/`crepe`-style relations with rules.

| Topic                    | A: oxigraph                    | B: ECS facts           | C: Datalog          |
| ------------------------ | ------------------------------ | ---------------------- | ------------------- |
| Belief sets / provenance | Named graphs, RDF-star         | Hand-rolled            | Hand-rolled         |
| Authoring format         | TriG (standard, diffable)      | None standard          | None standard       |
| Query expressiveness     | SPARQL 1.1                     | Bespoke code per query | Datalog rules       |
| Hot-path safety          | Enforced event-only discipline | Always hot             | Always hot          |
| Maturity risk            | Pre-1.0, one maintainer        | None                   | Low-maturity crates |

_Table 1: comparison of knowledge-store options._

## Decision outcome

Option A. The named-graph belief model and the standard authoring format are
the decisive capabilities; neither alternative offers them without reinventing
them. The store's weaknesses are neutralized structurally: no frame-loop
queries (a debug assertion enforces this, design §11.6), single-pattern lookups
bypass SPARQL, reactive joins live in the circuit (ADR 002), and the RocksDB
backend is unused, so persistence is portable TriG.

## Known risks and limitations

- Single-maintainer dependency: mitigated by using only standard RDF and
  SPARQL 1.1 surfaces, so migration to another store is a parser swap.
- No inference: subclass closure and similar derivations are explicit
  load-time rules, versioned with the ontology (design §11.4).
- SPARQL cost on dialogue open is uncharacterized at content scale; the
  candidate-storylet query set is benchmarked as content grows, with
  per-conversation caching already designed in.
