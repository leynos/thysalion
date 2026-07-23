# Architectural decision record (ADR) 002: DBSP as the logic authority

## Status

Accepted, 2026-07-22. All derived game state is computed by one DBSP circuit;
the ECS never derives rule consequences; bounded imperative escape hatches are
enumerated per rule class.

## Date

2026-07-22.

## Context and problem statement

A simulation-first RPG derives most of its interesting state: perception,
spread, structure membership, social aggregates ([design](thysalion-design.md)
§10.2). Deriving such state imperatively across ECS systems scatters
invalidation logic and breeds stale-cache bugs; the question is whether to
adopt an incremental view maintenance engine (DBSP) as the single derivation
authority, following the `lille` prototype, or to derive state in conventional
ECS systems.

## Decision drivers

- Determinism is a stated goal (design G2): DBSP steps are
  transaction-ordered and reproducible, whereas archetype-ECS scheduling
  semantics are informal enough that published work warns against resting
  determinism on them (Tasnim & Zhao 2026,
  <https://doi.org/10.1145/3748522.3779910>).
- Per-step cost proportional to change size fits a game loop where few
  entities act per tick in a large world (Budiu et al. 2023,
  <https://www.vldb.org/pvldb/vol16/p1601-budiu.pdf>).
- Fixpoint queries express spreading phenomena (fire, flood,
  reachability) declaratively.
- The `lille` prototype (dbsp 0.98, bevy 0.17.3) validated the
  integration mechanics and documented its sharp edges.
- A commissioned literature search found no published precedent for an
  IVM engine as a game rules engine — this is the design's principal novelty
  risk.

## Options considered

### Option A: DBSP circuit as sole logic authority

One circuit; ECS systems are stateless marshals; enumerated rule classes with
named imperative escape hatches (search, order statistics, continuous fields).

### Option B: conventional ECS systems

Rules as ordinary Bevy systems with hand-managed caches and change detection.

### Option C: embedded Datalog (ascent/crepe) per subsystem

Declarative rules, but re-evaluated from scratch per tick rather than
incrementally maintained, and without DBSP's formal incremental semantics.

| Topic                 | A: DBSP                       | B: ECS systems          | C: Datalog           |
| --------------------- | ----------------------------- | ----------------------- | -------------------- |
| Determinism authority | Circuit semantics             | System ordering         | Evaluation order     |
| Incremental cost      | Proportional to change        | Hand-built caches       | Full re-evaluation   |
| Fixpoint rules        | Native (nested circuits)      | Manual iteration        | Native               |
| Precedent             | lille only (novel)            | Universal               | Rare in games        |
| Failure surface       | Trace growth, retraction bugs | Stale caches everywhere | Per-tick cost cliffs |

_Table 1: comparison of logic-engine options._

## Decision outcome

The decision is Option A, with Option B's mechanisms retained exactly where
DBSP is known to fit poorly (design §10.3). The determinism and
incremental-cost arguments are structural, not incidental, and the lille
experience converts much of the novelty risk into known engineering discipline
(retraction hygiene, non-send scheduling, input clearing). The novelty that
remains is bounded by the rule-class table (design Table 3): every class has a
conventional fallback implementation path if it proves unworkable in the
circuit.

## Known risks and limitations

- Join traces retain both sides; state growth is bounded by explicit
  policy and verified by soak test (design §10.6, invariant I7).
- The ECS↔circuit bridge is bespoke; no off-the-shelf adapter exists.
- Recursive rules have data-dependent iteration counts; per-tick delta
  bounding (design §10.3) is the mitigation.
- dbsp is pre-1.0 with a rapid release cadence; the version is pinned
  per development phase, as for Bevy.
