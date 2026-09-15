# ADR 007: The replay record format

## Status

Accepted

## Date

2026-09-14

## Context and problem statement

[thysalion-design.md](thysalion-design.md) §14 defines correctness by named
invariants. The first of them, I1 (simulation determinism), is verified two
ways: property-based tests over randomized input scenarios, and "a replay
harness [that] re-executes recorded play sessions in CI". The same section
commits to a replay *corpus* as the project's combinatorial-coverage strategy —
rule classes crossed with scene classes are "exercised through the replay
corpus as it accumulates rather than exhaustively enumerated".

That commitment is what makes this a format decision rather than a scaffolding
one. A corpus accumulates: a session recorded during phase 4 is meant to still
run in phase 9, and a recording that cannot be read by a later build is a hole
in the coverage argument rather than a stale file. Roadmap step 1.3.2 therefore
requires the storage format to be "versioned from the start", and sets the
success criterion that "recording and replaying an empty session is
byte-identical".

The awkward part is the timing. Roadmap step 1.3 is the last step of phase 1;
`thysalion-sim` is an empty skeleton and stays one until phase 4. There is no
simulation to record, no circuit boundary to record at, and no input vocabulary
to record in. The question this record answers is how to version a format from
the first byte when the payload it will eventually carry does not exist.

## Decision drivers

- Design §14 makes recorded sessions long-lived artefacts, so wire
  compatibility is a requirement from the first recording rather than from the
  first release.
- Design §14's I1 is stated over *input record sequences* at the circuit
  boundary, not over window or device events.
- Roadmap 1.3.2 requires byte-identical record/replay and a harness invoked
  from continuous integration.
- [ADR 006](adr-006-scene-document-model.md) already solved canonical
  encoding, versioning, and wire-shape enforcement for this workspace, and its
  discipline is enforced by tests that exist.
- `thysalion-sim` is empty until phase 4, so any record vocabulary invented now
  would be speculation.
- ADR 006's alternatives section explicitly forwards the Arrow question to this
  step: "Where Arrow *would* fit is the replay corpus of roadmap step 1.3.2 and
  invariant I1".

## Decision outcome

### The format is an envelope, and the payload is uninhabited

A recorded session is a `SessionHeader` — a `{major, minor}` version, the tick
rate the session ran at, and an optional reference to the scene it ran against
— followed by a sequence of tick-stamped `TickRecord`s. A `TickRecord` carries
a tick number and a list of `InputRecord`s.

`InputRecord` is an enum with no variants:

```rust
/// One domain-level input, as design §6.2's input phase writes it.
pub enum InputRecord {}
```

An uninhabited payload is the whole of this record's answer to the timing
problem. It makes `TickRecord.inputs` provably empty until the first real
variants land, so the envelope, the version rule, the recorder, the replayer,
and the byte-identity test all exist and are all exercised, while nothing
meaningless reaches the wire.

The alternative that suggests itself — a `Placeholder` variant, or a raw byte
blob, to "have something to record" — is worse than it looks in a corpus
format. A variant that ships becomes permanent compatibility baggage: every
later reader must still decode it, and no version can quietly drop it without a
major bump that invalidates the corpus. An uninhabited enum has no such cost,
because no recording can contain one.

The first real variants arrive at roadmap step 4.1.2, when the circuit boundary
exists and there is something to record at it, and they arrive with a minor
version bump.

### The wire discipline is ADR 006's, wholesale

Not a new set of rules, and deliberately not a relaxed one. Every rule below
already has a mechanism behind it in `thysalion-world`, and copying the rules
copies the mechanisms:

- MessagePack via `rmp_serde::to_vec_named`, never `to_vec`. `rmp_serde` writes
  structs as positional arrays by default and its decoder accepts either shape
  silently, so `deny_unknown_fields` would have no field names to act on. An
  accidental `to_vec` ships a corpus that is positional, permanently
  unevolvable, and rejects nothing, while passing every Rust-level round-trip
  test. Only an explicit wire-shape assertion catches it, and
  `crates/test-support/tests/replay_round_trip.rs` carries one.
- `#[serde(deny_unknown_fields)]` on every wire type. No `#[serde(flatten)]`,
  which is incompatible with it at runtime. No `#[serde(untagged)]`, which
  buffers through serde's `Content` and destroys error locality. No tuple
  structs, because `serialize_tuple_struct` ignores the struct-map
  configuration entirely. No hand-written `serde` implementations.
- Every quantity is an integer, so the tree keeps `Eq`, `Ord`, and `Hash`, and
  no float representation can differ between builds.
- Every map would be a `BTreeMap`, never a `HashMap`, and no field carries
  `#[serde(skip_serializing_if)]` — under struct-as-map that would make the
  encoded map length value-dependent, so two equal sessions would encode to
  different bytes.

Only one encoding, not ADR 006's two. A scene document is authored and diffed
by a human, which is what earns JSON its place there; a replay recording is
written by a machine, read by a machine, and never hand-edited. A second
encoding would buy a review surface nobody reviews and a second writer to drift
against.

### The version is a range, probed before the full decode

`FormatVersion` is a `{major, minor}` pair with an accepted range, exactly as
ADR 006's `DocumentVersion` is, and for exactly the same reason: a whitelist
would make every additive change breaking, and the collision is immediate — the
first `InputRecord` variant would otherwise invalidate every recording made
before it.

Adding a field, or adding an `InputRecord` variant, bumps the minor; removing,
retyping, or re-meaning a field bumps the major. Decoding probes the version
through a deliberately permissive `SessionProbe` *before* attempting the full
structure, so a recording from a future build reports itself as an unsupported
version rather than as a confusing complaint about an unknown field.

### Byte-identity is a same-build promise, and the distinction matters

Roadmap 1.3.2's criterion is that recording and replaying an empty session is
byte-identical. That is a *same-build round-trip* promise: record, replay, and
re-record within one binary yields identical bytes. It is what
`an_empty_session_round_trips_byte_identically` asserts.

It is explicitly **not** a promise of cross-version wire byte-determinism — the
same promise ADR 006 declines to make for scene documents. A later build may
legitimately encode the same logical session differently, provided it does so
with a version bump. What holds that line is not a promise but a test: the
golden bytes in `crates/test-support/tests/fixtures/golden/empty.session.msgpack`
are committed, compared on every run, and refreshed only by deliberately
running an `#[ignore]`d regeneration test. A change to them is a wire-format
change and must arrive with a `FormatVersion` bump and a version-history entry.

### Ticks strictly increase, and both boundaries check it

A session's tick numbers must strictly increase. They need not be contiguous: a
tick at which nothing was input need not be recorded, and omitting it is what
keeps a long idle session small.

The rule is checked when encoding and again when decoding. The duplication is
deliberate rather than redundant. The encode-side check catches a recorder bug
before it writes a corpus entry nobody can interpret; the decode-side check is
about untrusted bytes, and the replay corpus is precisely where a build meets
bytes an older build wrote. Both failures name the offending pair of ticks,
because "some tick is out of order" sends a reader searching a session that may
be millions of ticks long.

### The format lives in `thysalion-test-support`

The harness that re-executes sessions is verification tooling; design §14 names
it beside the property tests. `thysalion-sim` is empty until phase 4, so
parking the format types there would put wire code in a plane crate with no
consumer, and `thysalion-world` is the state plane rather than a home for
simulation input.

Roadmap step 4.1.2 may re-home the *driver* — the part that steps a circuit
from a recording — once the circuit boundary exists. The format types stay put:
they are what the corpus is written in, and moving them would be a churn with
no reader-visible benefit.

## Alternatives considered

### Recording raw input events (the `leafwing_input_playback` approach)

The obvious prior art, and the wrong seam. Three grounds, in increasing order
of importance.

The crate is dormant: its last release targets Bevy 0.15 (December 2024)
against this workspace's Bevy 0.19, so adopting it would mean owning it.
Practitioner reports mark event-level playback flaky in continuous integration,
because a recording keyed to device events is sensitive to frame timing and
window focus — precisely the conditions a headless runner does not reproduce.

The decisive objection is neither of those. Design §14's I1 is stated over
input records *at the circuit boundary*: "for any input record sequence, two
circuit instances stepped identically produce identical output Z-sets".
Recording key codes would pin the invariant to a seam it is not about, and
would make every replay depend on the input-mapping layer staying unchanged —
so a rebinding change would invalidate the corpus without any determinism
property having been violated. The format therefore records design §6.2's
input-phase writes — commands, intents, waypoints — and never window events.

### Apache Arrow or Parquet

ADR 006 rejected these for the scene format and forwarded the question here,
noting that the replay corpus is where columnar storage would actually fit:
many homogeneous rows, analytical queries, and no authoring surface. The
observation stands, and the answer is *not yet* rather than *no*.

Three reasons to defer. There is no corpus to analyse: the payload is
uninhabited, so every recording today is a header. There is no analytical
consumer: nothing yet queries recordings across sessions, which is the workload
that would earn a columnar format. And Parquet does not promise byte
determinism across writer versions — files embed writer metadata, optional
statistics, row-group layout, and codec choices — so adopting it now would
forfeit the one criterion roadmap 1.3.2 actually sets.

**Re-opening trigger.** Revisit when the corpus acquires an analytical consumer
that queries across sessions — a coverage report over rule classes and scene
classes is the likely first one — or when a measured corpus is large enough
that whole-session scans dominate the verification suite's runtime. At that
point the natural shape is an Arrow or Parquet *projection* built from the
MessagePack recordings, not a replacement for them: the recordings would remain
the byte-identical artefacts and the columnar form would be a derived index.

### Validated newtypes over the envelope's fields

Review raised the envelope's raw primitives — `content_hash_hex` as a
`String`, the tick rate and tick number as bare integers — and proposed
validated domain newtypes wrapping the existing wire serialization.

Declined, on ADR 006's own reasoning rather than on cost. That record
introduced the document/domain split precisely where a domain type has "a
private field to protect and an invariant to enforce", and it is explicit that
"not every type needs two forms" — a second form for a type with no invariant
"would be two names for one set of values and a conversion function that can
only be the identity". The envelope is that case today: its one real rule is
the ordering of ticks *across* records, which no per-field newtype can express,
and which is therefore checked at the encode and decode boundaries instead.

The hex digest is the strongest candidate, and it is still premature. Nothing
reads the field yet — the replay-time check that a fixture still matches its
recorded hash arrives with roadmap 4.1.2 — so a validating constructor now
would encode a guess about what that check needs. The plan's own tolerance on
ambiguity applies: do not extend the format speculatively.

**Re-opening trigger.** Revisit when roadmap 4.1.2 adds the replay-time scene
check, which is the first consumer that must reject a malformed hash, or when
the first `InputRecord` variants give the payload an invariant of its own. A
domain form added then is additive and needs no version bump, because it
changes Rust API rather than wire shape.

### `postcard` or another compact binary format

Smaller bytes, and nothing else. MessagePack is already in this workspace's
dependency graph with a tested discipline around it, so the change would trade
a mechanism that exists for a size win over recordings that are currently a few
dozen bytes. Reconsider only if a measured corpus is dominated by envelope
overhead rather than payload, which cannot be true of any format whose payload
is the part that grows.

### JSON alongside MessagePack, as scenes have

Rejected above under wire discipline: recordings are machine-written and
machine-read, so the authoring encoding that justifies the second writer for
scenes has no counterpart here. A debugging need for human-readable recordings
is better met by a dumping tool than by a second writer that can drift.

## Consequences

### Positive

- The corpus is versioned from its first byte, so a recording made now is
  either readable by a later build or refused as an unsupported version —
  never silently misinterpreted.
- No speculative record vocabulary reaches the wire, so phase 4 defines the
  input record against a real circuit boundary with no inherited baggage.
- The byte-identity criterion of roadmap 1.3.2 is asserted by a test that runs
  in continuous integration, and the golden bytes turn a wire-format drift into
  a review conversation rather than a silent corpus break.
- ADR 006's discipline gains a second consumer, which is the cheapest evidence
  that the discipline generalizes.

### Negative

- The format cannot record anything yet. That is the point, and it does mean
  the recorder and replayer are exercised only against empty sessions until
  phase 4.
- The tick-ordering rule is checked in two places, which is a small duplication
  to keep in step.
- A session references its scene by name and content hash but nothing yet
  verifies that reference at replay time; the field exists so that the check
  has somewhere to read from when roadmap 4.1.2 adds it.

### Neutral

- One encoding rather than two. A debugging dump tool is the answer if
  human-readable recordings are ever wanted.
- The format types sit in test tooling rather than in a plane crate, which is
  where design §14 places the harness and where the consumers are.

## References

- [thysalion-design.md](thysalion-design.md) §6.2 (the input phase), §14
  (invariant I1 and the replay corpus)
- [ADR 006](adr-006-scene-document-model.md), whose canonical-encoding
  discipline this record adopts wholesale and whose Arrow deferral it answers
- [ADR 005](adr-005-workspace-crate-layout.md) for the crate taxonomy that
  places `thysalion-test-support` as tooling rather than as a plane
- [roadmap.md](roadmap.md) §1.3.2 for the task and its success criterion
- [The 1.3 execution plan](execplans/1-3-stand-up-headless-ci-and-the-verification-spine.md)
  for the decision log behind these choices

## Version history

| Version | Change                                       |
| ------- | -------------------------------------------- |
| 1.0     | Initial envelope: header, ticks, no payload. |

*Table 1: the replay format's version history.*
