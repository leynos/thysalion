# Thysalion roadmap

This roadmap translates [thysalion-design.md](thysalion-design.md) and ADRs
001–004 into an outcome-oriented delivery sequence. It promises no dates. Each
phase carries one testable idea at the GIST level; the steps under a phase work
toward validating or falsifying that idea; tasks are review-sized execution
units.

The shape is a sea urchin, not a layer cake. Phase 1 builds a small shared core
— the urchin's body — and every subsequent spike phase (2–5, 7, 8) radiates
independently of it, each culminating in an **executable capability
demonstration**: a runnable binary that trials one cluster of concepts
(presentation, lighting, simulation, knowledge, saves) without waiting on a
full game. Spike phases 2–5 depend only on phase 1 and may proceed in any order
or in parallel. Phases 6 and 9 are **integration milestones**: once enough
spikes exist, they deliver cohesive, playable concept demonstrations that prove
the planes compose. Phase 10 collects deferred scope.

Every demonstration binary is kept working from the moment it lands: demos are
the roadmap's regression surface as well as its deliverables.

## 1. The core: demo harness, scene format, and verification spine

Idea: if a minimal shared core — one workspace layout, one demo harness, one
scene format, and headless test scaffolding — exists before any capability work
starts, then every capability can be built and judged as an independent
runnable demo instead of as a layer of an unfinished game.

This phase is the urchin's body. It contains no gameplay; its deliverable is
the ability to add a spike cheaply.

### 1.1. Establish the workspace and the demo harness contract

This step answers what a capability demonstration is, concretely, so every
later spike ships one without reinventing scaffolding. Its outcome fixes the
crate layout and the harness API that all demos share. See thysalion-design.md
§6.1 and §4.

- [x] 1.1.1. Create the workspace crate layout for the four planes.
  - Crates for world/scene data, simulation, knowledge, presentation, and
    a `demos` crate hosting one binary per capability demonstration.
  - Success: `make test` and `make lint` pass on the empty skeleton, and
    the layout is recorded in repository-layout.md.
- [x] 1.1.2. Implement the shared demo harness.
  - Isometric camera with four yaw quadrants and zoom, input mapping, a
    diagnostics overlay (frame time, tick time, counters), and a
    screenshot key for visual review.
  - Success: a `demo-empty` binary opens a window, renders a ground
    plane, and reports diagnostics; the harness API is documented in the
    developers' guide.

### 1.2. Deliver the scene format and fixture scenes

This step answers whether the authored-scene contract of the design can be
loaded, validated, and shared by every demo. Its outcome supplies the fixture
scenes all later spikes and CI suites consume. See thysalion-design.md §7.

- [ ] 1.2.1. Implement the voxel type registry and scene document model.
  - Palette entries with material class, per-face passability, slope,
    emission, simulation coefficients, and optional concept IRI.
  - See thysalion-design.md §7.2.
  - Success: a hand-written JSON scene round-trips through the model and
    the MessagePack encoding without loss.
- [ ] 1.2.2. Implement scene loading with load-time validation.
  - Reject unknown palette references, out-of-bounds spawns, and dangling
    knowledge IRIs with diagnostics; never partially load.
  - See thysalion-design.md §7.3.
  - Success: corrupt fixture variants each produce a distinct diagnostic
    and leave no scene state behind.
- [ ] 1.2.3. Author the three fixture scenes used by all later phases.
  - Requires 1.2.2.
  - A keep interior, a market-town block, and a swamp fragment, sized per
    the scene classes and matching the reference art's palette bands.
  - See thysalion-design.md §7.1 and §1.
  - Success: all three load through the validator; each is referenced by
    at least one demo or CI suite by the end of phase 6.
- [ ] 1.2.4. Publish the scene document's JSON Schema as a versioned
  artefact.
  - Requires 1.2.1.
  - Emit the schema from the document types' existing `schemars` derives,
    stamp it with the document version, and commit it so external content
    tooling can target the format as data rather than by depending on
    `thysalion-world`.
  - See thysalion-design.md §7.3 and §7.4, and
    adr-006-scene-document-model.md.
  - Success: CI regenerates the artefact and compares it byte for byte;
    every fixture scene validates against it, and every corrupt fixture
    whose fault is structural is rejected by it.

### 1.3. Stand up headless CI and the verification spine

This step answers whether the invariant-based verification strategy can run
without a GPU or a window from day one, which decides how much of the design's
verification programme is continuously enforced. See thysalion-design.md §14.

- [ ] 1.3.1. Wire headless Bevy test scaffolding into CI.
  - MinimalPlugins app construction, fixture-scene loading, and the
    diagnostics counters exposed for assertions.
  - Success: a trivial headless behavioural test loads a fixture scene in
    CI on every push.
- [ ] 1.3.2. Build the deterministic replay harness skeleton.
  - Record and replay input-record sequences against whatever simulation
    exists; storage format versioned from the start.
  - See thysalion-design.md §14 (I1).
  - Success: recording and replaying an empty session is byte-identical;
    the harness is invoked from CI.

## 2. Spike: the diorama looks right

Idea: if a greedy-meshed, fixed-camera voxel pipeline can render the fixture
scenes with cut-away interiors at the reference art's framing, then the
meshed-pipeline bet (ADR 001) holds and no raymarched fallback is needed for
presentation work.

Deliverable: `demo-diorama` — load any fixture scene, orbit the four quadrants,
zoom, walk a proxy party marker indoors, and watch roofs cut away.

### 2.1. Mesh and draw bounded scenes

This step answers whether load-time binary greedy meshing over
`bevy_voxel_world` delivers the diorama workload on Bevy 0.19's batched path.
See thysalion-design.md §8.1 and adr-001-meshed-voxel-rendering-pipeline.md.

- [ ] 2.1.1. Integrate `bevy_voxel_world` with authored-scene delivery.
  - Requires steps 1.1–1.2.
  - Replace procedural terrain delegates with fixture-scene content;
    bounded chunk set, no spawn distance.
  - Success: the market-town fixture renders in `demo-diorama` with all
    chunks meshed at load.
- [ ] 2.1.2. Swap in the binary greedy mesher via the custom-meshing
  delegate.
  - Requires 2.1.1.
  - Carry ambient occlusion and tint in vertex attributes; verify Bevy
    0.19/glam compatibility.
  - Success: triangle counts drop measurably versus the default mesher on
    all three fixtures, with no visual regression in screenshots.
- [ ] 2.1.3. Implement incremental chunk re-meshing on voxel edit.
  - Requires 2.1.2.
  - Async re-mesh of the edited chunk and face-adjacent neighbours; stale
    mesh drawn until replacement.
  - Success: an edit burst in `demo-diorama` never blocks the frame, and
    the diagnostics overlay reports re-mesh latency.

### 2.2. Exploit the fixed camera

This step answers whether octant culling and cut-away interiors — the Ultima
VII signature — work as material and scheduling features under the 0.19 render
model. See thysalion-design.md §8.2–8.3.

- [ ] 2.2.1. Implement quadrant-aware face sets and octant culling.
  - Requires 2.1.2.
  - Success: rotating quadrants in `demo-diorama` swaps precomputed face
    sets without re-meshing, confirmed by the overlay's mesh counters.
- [ ] 2.2.2. Implement cut-away interiors via clip-height material.
  - Requires 2.1.1.
  - Dithered edge band; per-structure clip driven by a placeholder
    party-position input until 6.1 supplies the derived relation.
  - Success: moving the proxy marker into the keep fixture hides the
    roof and upper storeys exactly as in the reference interiors.

## 3. Spike: light behaves

Idea: if the flood-fill light field and the atmosphere stack reproduce the
concept art's time-of-day and weather moods on tiers 0–1 alone, then the game's
baseline look is secured on low-spec hardware before any probe GI exists.

Deliverable: `demo-light-field` — edit voxels and watch light propagate; scrub
time of day; toggle rain and snow; watch surfaces wet and dry.

### 3.1. Deliver tier 0 direct lighting and the sun path

This step answers whether emissive voxel registration and the authored sun path
produce the warm-key/cool-fill structure of the reference art. See
thysalion-design.md §9.1 and §9.4.

- [ ] 3.1.1. Register emissive voxel types as clustered point lights and
  animate the scene sun path.
  - Requires steps 2.1.
  - Shadow-casting budgets per scene class from the manifest.
  - Success: the keep-interior fixture at evening matches the reference
    torchlit framing in a side-by-side screenshot review.

### 3.2. Deliver the flood-fill light field

This step answers whether the two-channel 0–15 field converges at load and
stays convergent under incremental edits — the design's invariant I4 — and
whether its sky-visibility channel is fit to drive gameplay and wetness. See
thysalion-design.md §9.2 and §14 (I4).

- [ ] 3.2.1. Implement the dual-channel flood-fill field in compute with a
  CPU reference implementation.
  - Requires steps 1.2–1.3.
  - Success: property tests show incremental state equals from-scratch
    recomputation over random edit sequences (I4), and GPU/CPU parity
    holds on fixtures in CI.
- [ ] 3.2.2. Composite the field into the voxel material and expose
  per-voxel light level to the CPU.
  - Requires 3.2.1 and 3.1.1.
  - Day/night scaling of the sky channel; `max(sky × day, block)`
    composition.
  - Success: dawn-to-night scrubbing in `demo-light-field` reproduces the
    style guide's time-of-day strip across all three fixtures.

### 3.3. Deliver atmosphere and wetness

This step answers whether fog, precipitation, and material wetness compose with
tiers 0–1 into the harbour-in-rain and swamp-twilight moods. See
thysalion-design.md §9.4–9.5.

- [ ] 3.3.1. Integrate froxel volumetric fog and the radial-blur fallback.
  - Requires 3.1.1.
  - Success: the swamp fixture at twilight shows layered fog and light
    shafts; the fallback path renders on the low-spec preset.
- [ ] 3.3.2. Implement precipitation particles and the wetness response.
  - Requires 3.2.2.
  - Rain clipped by the sky-visibility mask; wetness raising specular and
    darkening albedo, decaying over game time.
  - Success: in `demo-light-field`, covered ground stays dry during rain
    and cobbles read wet in the blue-hour harbour framing.

## 4. Spike: the circuit thinks

Idea: if one DBSP circuit derives motion, perception, and spread
deterministically within the tick budget under demo workloads, then the
design's principal novelty risk (ADR 002) is retired at demo scale before any
game depends on it.

Deliverable: `demo-sim` — spawn dozens of agents in a fixture scene, watch them
move, see perception relations visualized, ignite a voxel and watch fire
spread; flip a switch to replay the session and observe identical outcomes.

### 4.1. Build the circuit scaffold and the ECS bridge

This step answers whether the extract → step → apply cycle, retraction
discipline, and identity map — lille's mechanics — hold under Bevy 0.19 and
current dbsp. Everything later in the phase rests on it. See
thysalion-design.md §6.2, §10.1, and adr-002-dbsp-as-logic-authority.md.

- [ ] 4.1.1. Implement `DbspCircuit`, the `SimId` identity map, and the
  chained extract/apply systems.
  - Requires steps 1.1–1.3.
  - Non-send resource; in-circuit tick source; input clearing every tick;
    failed steps write nothing.
  - Success: determinism (I1) and retraction-soundness (I2) property
    tests pass in CI over random spawn/act/despawn interleavings.
- [ ] 4.1.2. Wire the replay harness to the circuit boundary.
  - Requires 4.1.1 and 1.3.2.
  - Success: a recorded `demo-sim` session replays to identical
    consolidated outputs, asserted in CI.

### 4.2. Derive support, motion, and pathfinding hand-off

This step answers whether the circuit resolves movement over the voxel
passability model while A* stays outside as designed. See thysalion-design.md
§10.2 (support and motion) and §10.3.

- [ ] 4.2.1. Implement the support/motion rule class over fixture
  passability data.
  - Requires 4.1.1 and 1.2.1.
  - Floor-height aggregation, standing/unsupported branches, slope
    handling.
  - Success: agents in `demo-sim` walk ramps and ledges of the keep
    fixture without clipping or floating.
- [ ] 4.2.2. Implement A* pathfinding as an imperative service feeding
  waypoint inputs.
  - Requires 4.2.1.
  - Success: click-to-move in `demo-sim` routes agents through doorways
    and around obstacles; waypoints appear in replays.

### 4.3. Derive perception, combat, and discrete spread

This step answers whether the remaining demo-scale rule classes — the ones
integration milestones need — fit the circuit within budget. See
thysalion-design.md §10.2 and §10.6.

- [ ] 4.3.1. Implement the perception rule class with a placeholder light
  level.
  - Requires 4.2.1.
  - Range, facing arc, and light threshold joins; visualized as cones and
    sight lines in `demo-sim`.
  - Success: the overlay shows who-sees-whom updating only for changed
    entities each tick.
- [ ] 4.3.2. Implement combat resolution and health accounting.
  - Requires 4.3.1.
  - Saturating bounds, death edges, per-source dedup as in the design.
  - Success: scripted melee in `demo-sim` produces identical health
    traces across replays.
- [ ] 4.3.3. Implement the discrete-spread fixpoint rule class for fire.
  - Requires 4.1.1 and 1.2.1.
  - Bounded per-tick spread over voxel adjacency using palette fuel
    coefficients; ignition and extinction events as circuit outputs.
  - Success: igniting the swamp fixture's boardwalk in `demo-sim` spreads
    fire tick-bounded and deterministically; trace sizes stay within the
    step's stated envelope on the diagnostics overlay.

## 5. Spike: the world means something

Idea: if storylet dialogue over per-NPC belief graphs works as an event-queried
oxigraph store — with no engine, no 3-D, and no circuit — then the knowledge
plane (ADR 003) can be authored and trialled by designers entirely outwith the
game build.

Deliverable: `demo-parley` — a windowed dialogue trial: pick an NPC, see the
storylets their beliefs unlock, converse, watch quest state and beliefs change,
and inspect any graph live.

### 5.1. Stand up the store, authoring load, and query discipline

This step answers whether TriG authoring, load-time validation, and the
event-only query rule are workable in practice. See thysalion-design.md
§11.1–11.2, §11.5–11.6, and adr-003-oxigraph-knowledge-plane.md.

- [ ] 5.1.1. Implement the knowledge store wrapper with the named-graph
  layout and TriG loading.
  - Requires 1.1.1.
  - Ontology, world, belief, and scene graphs; `rdf-12` enabled; load
    validation per the design.
  - Success: the fixture content pack loads, and invalid variants are
    rejected with per-file diagnostics.
- [ ] 5.1.2. Enforce the event-only query discipline and result caching.
  - Requires 5.1.1.
  - Debug assertion against tick-schedule queries; cached result
    components with event-driven invalidation.
  - Success: `demo-parley` shows cache hits on repeated opens and the
    assertion trips in a negative test.

### 5.2. Deliver storylets, salience, and belief divergence

This step answers whether quality-based narrative over SPARQL is
author-friendly and expressive enough for the MVP's dialogue. See
thysalion-design.md §11.3–11.4.

- [ ] 5.2.1. Implement storylet preconditions, salience ranking, and
  transactional effects.
  - Requires 5.1.2.
  - `ASK`/`SELECT` preconditions over belief ∪ world ∪ ontology;
    most-specific-match ordering with authored tiebreak; effects as
    SPARQL `UPDATE` in one transaction.
  - Success: the three-NPC fixture cast offers different options for the
    same situation, and choosing an option advances quest stage
    atomically.
- [ ] 5.2.2. Implement load-time derivation rules and RDF-star provenance.
  - Requires 5.1.1.
  - Bounded `INSERT … WHERE` closure to fixpoint; provenance-annotated
    belief statements ("heard from, on day").
  - Success: subclass-dependent storylets fire without runtime reasoning,
    and `demo-parley` displays each belief's provenance chain.

## 6. Integration milestone: a living street

Idea: if fire, light, perception, weather, and knowledge compose in one scene
through the designed plane contracts — fire brightens the street, guards see
arson by its light, rain quenches it, and the town learns of it — then the
four-plane architecture composes without rework, which is the design's central
claim.

Deliverable: `demo-living-street` — the market-town fixture with a patrolling
guard cast, weather scrubbing, an arson tool, and an inspector for every
plane's view of the incident.

### 6.1. Bridge the planes

This step answers whether the three cross-plane contracts — light level into
perception, circuit exports into the world graph, and structure membership into
the cut-away — carry real data. See thysalion-design.md §10.2, §11.4,
§12.1–12.2.

- [ ] 6.1.1. Feed the flood-fill light level into the perception rule
  class.
  - Requires 3.2.2 and 4.3.1.
  - Success: in darkness a thief walks past a guard undetected in
    `demo-living-street`; torchlight reverses the outcome
    deterministically.
- [ ] 6.1.2. Export reactive social facts from the circuit into the world
  graph.
  - Requires 4.3.1 and 5.1.1.
  - Witness events to crime-awareness aggregates written as triples in
    the apply phase.
  - Success: after a witnessed arson, `demo-parley` dialogue against the
    same store reflects district-level awareness without any bespoke
    scripting (G1).
- [ ] 6.1.3. Derive structure membership in the circuit and drive the
  cut-away from it.
  - Requires 4.1.1 and 2.2.2.
  - Replace the placeholder party-position clip input with the fixpoint
    connected-components relation.
  - Success: entering any fixture building cuts away exactly that
    structure's upper voxels.

### 6.2. Couple material fields to the circuit

This step answers whether the GPU field / circuit threshold-event split holds:
continuous heat and moisture on the GPU, discrete ignition in the circuit, rain
measurably retarding fire. See thysalion-design.md §10.5 and §9.5.

- [ ] 6.2.1. Implement the heat and moisture fields as compute stencil
  passes with threshold-event readback.
  - Requires 3.3.2 and 4.3.3.
  - Success: threshold crossings arrive as circuit inputs within one
    tick, and field values stay clamped under fuzzed inputs.
- [ ] 6.2.2. Close the fire–weather loop.
  - Requires 6.2.1.
  - Circuit ignition decisions as field boundary conditions; moisture
    from wetness suppressing spread.
  - Success: the same arson attempt succeeds in drought and fails in
    rain in `demo-living-street`, deterministically across replays.

### 6.3. Prove the composition holds

This step answers whether the composed scene meets the design's coverage
commitments rather than merely demonstrating well. See thysalion-design.md
§9.6, §13, and §14.

- [ ] 6.3.1. Build the lighting-combination E2E suite.
  - Requires 6.1.1 and steps 3.1–3.3.
  - Presets (low, default) × weather (clear, rain, night) on fixture
    scenes, headless, asserting I4, NaN-freedom, and the §13 fallback
    ladder.
  - Success: the six-way matrix runs green in CI and failure output names
    the offending combination.
- [ ] 6.3.2. Build the state-growth soak against the living street.
  - Requires steps 6.1–6.2.
  - Scripted enter/exit cycles across all fixtures with the arson
    scenario; per-operator trace and quad counts checked against the
    envelope (I7).
  - Success: the soak runs in CI and fails on monotonic growth.

## 7. Spike: the light bounces

Idea: if DDGI probes updated by DDA marches through the resident voxel grid
deliver leak-free interior bounce within a 2 ms budget on mid-range hardware,
then the hero look (ADR 004) ships without ray-tracing hardware; if not, tier 1
remains the shipped baseline and the design's tiering absorbs the miss.

Deliverable: `demo-gi` — any fixture with a tier 1/tier 2 split-screen toggle,
probe visualization, and a live probe-budget readout.

### 7.1. Implement the probe volume and software ray march

This step answers whether the voxel grid suffices as the acceleration structure
and the Chebyshev test prevents leaks in the cut-away fixtures. See
thysalion-design.md §9.3 and adr-004-tiered-lighting-software-ddgi.md.

- [ ] 7.1.1. Implement probe volumes, octahedral irradiance and depth
  textures, and the DDA probe-ray kernel.
  - Requires steps 3.1–3.2.
  - Per-scene volumes from the manifest; rays sampling tier 0/1 lighting
    at hits per the design's kernel sketch.
  - Success: `demo-gi` shows colour bleed and interior bounce on the keep
    fixture; probe update cost appears in the overlay.
- [ ] 7.1.2. Implement visibility-weighted shading with the self-shadow
  bias.
  - Requires 7.1.1.
  - Eight-probe interpolation with trilinear, cosine, and Chebyshev
    weights.
  - Success: the probe-leak fixture suite (I5) passes with weights below
    epsilon behind full occluders.

### 7.2. Harden probes to production behaviour

This step answers whether hysteresis, probe states, and round-robin budgets
meet the published cost envelope on target hardware. See thysalion-design.md
§9.3 and §9.6.

- [ ] 7.2.1. Implement temporal hysteresis, the probe state machine, and
  the round-robin update budget.
  - Requires 7.1.2.
  - Published defaults (α ≈ 0.95); off/asleep/awake classification; edit-
    and light-biased scheduling.
  - Success: the district fixture stays within the 2 ms tier 2 budget
    under the design §9.3 reference baseline (RTX 3060, NVIDIA driver
    591.59, 1080p, default preset, 95th-percentile over a 10 s window
    after 5 s warm-up), measured in `demo-gi`.
- [ ] 7.2.2. Extend the lighting E2E matrix to three presets.
  - Requires 7.2.1 and 6.3.1.
  - Success: the nine-way preset × weather matrix of the design runs
    green headless in CI.

## 8. Spike: the party persists

Idea: if the Ultima VII interaction surface — party control, paperdoll
inventory, journal, look-at text — and honest save/load work against the
fixture scenes, then the game-shaped remainder of the MVP is UI content, not
architecture.

Deliverable: `demo-party` — a four-character party in any fixture with the
reference UI chrome, plus save, load, and replay-verified restore.

### 8.1. Deliver the party interaction surface

This step answers whether the concept art's UI reads at size and the
interaction loop (select, move, look, use) feels right over the existing
planes. See thysalion-design.md §1 and the reference images.

- [ ] 8.1.1. Implement party selection, movement orders, and look-at
  descriptions.
  - Requires steps 4.2 and 2.2.
  - Look-at text resolved through voxel concepts and entity identity.
  - Success: the market-town walkthrough of the concept art (walk, look,
    talk trigger) is reproducible in `demo-party`.
- [ ] 8.1.2. Implement the party UI chrome: portraits, paperdoll
  inventory, journal, and minimap.
  - Requires 8.1.1.
  - Success: side-by-side with `references/example-1.png`, the layout and
    hierarchy match; journal entries append from knowledge-plane events.

### 8.2. Deliver honest persistence

This step answers whether the three-store save design — ECS snapshot, TriG
serialization, circuit rebuild — meets the round-trip invariant. See
thysalion-design.md §12.3 and §14 (I3, I6).

- [ ] 8.2.1. Implement the save archive and the load-time circuit
  rebuild.
  - Requires 4.1.2 and 5.1.1.
  - Versioned archive with content hashes covering the scene assets, the
    static ontology, the storylet definitions, and the derivation rules;
    refusal on mismatch; field planes bit-exact in their fixed-point
    format (design §10.5).
  - Success: the I3 property holds on scripted sessions — save, load, and
    N ticks equal N ticks direct — including fixtures saved with field
    values one unit below gameplay thresholds, asserted in CI; a fixture
    mutating the ontology or a storylet between save and load is refused
    at load; and a fixture that saves mid-horizon — with a live horizon-
    limited event and a pending cooldown — verifies identical expiry ticks
    and derived outputs after load.
- [ ] 8.2.2. Implement the cross-store referential integrity sweep.
  - Requires 8.2.1.
  - Validation at load, scene transition, and save per I6.
  - Success: NPC-death and quest-completion edge fixtures pass the sweep;
    seeded corruption is detected and refused.

## 9. Integration milestone: the vertical slice

Idea: if one continuous play session — town, interior, quest, combat, fire,
rain, dialogue, save — runs on the generic machinery alone, then the MVP
boundary of the design is real and everything beyond it is content and polish.

Deliverable: `thysalion` itself, at MVP scope: the design §15 slice as one
cohesive, replayable session.

### 9.1. Assemble the slice

This step answers what breaks when everything runs together that never broke in
spikes — the integration milestone's whole purpose. See thysalion-design.md §15.

- [ ] 9.1.1. Compose the MVP session from the demonstrated capabilities.
  - Requires phases 6 and 8; requires steps 5.2 and 7.2 for the default
    preset.
  - Town and interior scenes, the three-NPC quest line, one combat
    encounter, the fire/rain interaction, and save points.
  - Success: a scripted end-to-end playthrough passes headless in CI, and
    a human playthrough completes without touching a debug tool.
- [ ] 9.1.2. Build the MVP combination E2E suite.
  - Requires 9.1.1.
  - The slice's crossing surfaces: preset × weather × save/load-at-phase,
    exercised through the replay corpus.
  - Success: the suite runs green in CI and each failure names its
    combination and tick.
- [ ] 9.1.3. Hold the visual-target review against the reference art.
  - Requires 9.1.1.
  - Screenshot set across scenes, times of day, and weather versus
    `references/`; explicit go/no-go on the meshed pipeline per ADR 001's
    fallback criterion.
  - Success: the review is recorded in the design document with any
    divergences and follow-up decisions.

## 10. Deferred extensions after the vertical slice

Idea: if the vertical slice is trustworthy and boring to operate, the project
can evaluate and deliver the design's deferred bets on product value instead of
letting them destabilize the MVP.

### 10.1. Re-open the deferred decisions on their recorded criteria

See thysalion-design.md §15 (Table 7) for the re-opening criteria.

- [ ] 10.1.1. Evaluate `bevy_solari` as the ultra lighting tier.
  - Requires phase 9.
  - See thysalion-design.md §9.7.
- [ ] 10.1.2. Evaluate GPU meshing if load-time meshing exceeds budget.
  - Requires phase 9.
- [ ] 10.1.3. Evaluate the raymarched rendering tier if 9.1.3 fails the
  visual target.
  - See adr-001-meshed-voxel-rendering-pipeline.md.
- [ ] 10.1.4. Revisit multiplayer lockstep, WASM builds, and formal proof
  of circuit rules as their criteria trigger.
  - See thysalion-design.md §15.

### 10.2. Deliver the authoring pipeline beyond layered text

Layered text and `scripts/build_fixture_scenes.py` carry the fixture scenes
through the vertical slice. This step delivers the pipeline that replaces them
once scenes outgrow what a contributor will hand-author. See
thysalion-design.md §7.4.

- [ ] 10.2.1. Build the Tiled layered-isometric authoring workflow.
  - Requires phase 9 and 1.2.4.
  - Layered isometric editing as built for lille, emitting the JSON
    encoding against the published schema rather than against engine code.
  - Success: a scene authored in Tiled loads through the validator, and the
    layered-text fixtures re-author in it without loss.
- [ ] 10.2.2. Deliver MagicaVoxel `.vox` import as palette-mapped voxel
  stamps.
  - Requires phase 9 and 1.2.4.
  - Props and set-dressing only: `.vox` caps a model at 255 palette colours
    and 256 voxels per axis, against a 1024-extent scene class and a 16-bit
    palette, so it is interchange and never canonical authoring input.
  - Success: an imported stamp maps onto the scene palette by name, and a
    model breaching either cap is rejected with a diagnostic naming the cap.
- [ ] 10.2.3. Deliver in-engine voxel editing for detail passes.
  - Requires phase 9 and 2.1.3, whose re-mesh path this extends rather
    than reinvents.
  - See thysalion-design.md §8.1.
  - Success: an edit re-meshes only its own chunk and the face-adjacent
    chunks it touches, and round-trips through the scene format.
