# World plane architecture

The crate `thysalion-world` is the *state* plane
([thysalion-design.md](thysalion-design.md) §6.1): authoritative for current
component values, the voxel grid, and material fields, and never deriving rule
consequences. It is also the workspace's dependency sink — other plane crates
may depend on it, never the reverse
([ADR 005](adr-005-workspace-crate-layout.md)).

This document describes its internally facing interfaces. The decisions behind
them, and the alternatives rejected, are recorded in
[ADR 006](adr-006-scene-document-model.md).

## Module tree

| Module                                                                    | What it holds                                                                                                 |
| ------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------- |
| `grid`                                                                    | The runtime voxel structure and the coordinate types. What phase 2's mesher and phase 4's pathfinder consume. |
| `scene::document`                                                         | The wire types. `serde` derives, `deny_unknown_fields`, deliberately permissive.                              |
| `scene::palette`, `scene::entities`, `scene::knowledge`, `scene::concept` | The validated domain types. No `serde` derives at all.                                                        |
| `scene::validation`                                                       | The rules, the phase orchestration, the diagnostic vocabulary, and the reporter.                              |
| `codec`                                                                   | JSON and MessagePack, from one structure.                                                                     |
| `source`                                                                  | The driven port through which a scene reaches its resources, and its two adapters.                            |
| `loader`                                                                  | The application service that composes the three.                                                              |
| `check`                                                                   | The `scene-check` operator tool, as a library.                                                                |

*Table 1: the world crate's modules and what each owns.*

`scene` may depend on `grid`; `grid` never depends on `scene` beyond the wire
types it converts to and from. The direction matters because the grid outlives
the document that produced it.

## The port and its adapters

Validation is a pure function of the document with exactly one exception:
whether the knowledge resources a scene names are actually reachable. That rule
set lives in `scene::validation::rules::resources` and reaches the outside
world only through the `SceneSource` port. Keeping the infrastructure surface
to one named file is what lets the rest of the validator be exercised without a
filesystem, and what makes an accidental widening visible in review.

```rust
pub trait SceneSource: Send + Sync {
    fn read(&self, path: &Utf8Path) -> Result<Vec<u8>, SceneSourceError>;
}
```

There is deliberately no `exists` method. A boolean cannot distinguish an
absent file from an unreadable one, so an adapter would be obliged to discard
that distinction — and an author told their scene "names a file that is not
there" about a file sitting right in front of them, merely unreadable, has been
actively misled. `SceneSourceError` carries the difference instead, and it
costs nothing: phase 5 reads these files anyway, and phase 8 needs their bytes
for the content hashes.

Two adapters ship. `DirSceneSource` wraps a `cap_std` directory capability, so
ambient authority is taken once by whoever constructs it and never inside the
domain. `MemorySceneSource` is a map from path to bytes, for tests and for
callers that already hold the data. Both enforce the same path rules — no
absolute paths, no parent-directory components — because the trust boundary
covers both and only one of them has a sandbox to fall back on.

`Send + Sync` is a supertrait now rather than later: a loader becomes a Bevy
resource at roadmap step 2.1.1, and adding a supertrait afterwards is a
breaking change.

## Loading

```rust
let loader = SceneLoader::new(Arc::new(source));
let loaded = loader.load(Utf8Path::new("keep-interior.scene.json"))?;
```

`SceneLoader` is non-generic on purpose: the port is object-safe, and a type
parameter would be a stability surface of its own — every caller naming the
loader would also have to name its source type. It holds no mutable state, no
cache, and no interior mutability, which is what makes design §13, Table 6's
"previous scene remains active" free rather than a matter of defensive copying.

`LoadedScene` carries the scene *and* a warning list. The warning channel
exists from the outset because retrofitting it would break every caller:
`Result<Scene, _>` has nowhere to put "this loads, but a spawn is inside a
wall".

`SceneLoadError` has four cases, and the distinction between them is what the
tool's exit codes rest on. `Malformed` and `Invalid` both mean the document is
wrong; `Source` and `UnknownEncoding` mean the tool never got to look at it.

## Reading a diagnostic report

```text
document: keep-interior.scene.json
source root: assets/scenes
1 error(s), 0 warning(s)

error scene.voxels.unknown-palette-index at voxels[0]
  chunk (1, 1, 0) at local (0, 0, 0)
  palette index 99 does not resolve; the palette has 6 entries
```

Four things to read, in order.

- **The source root.** Running from the wrong working directory produces
  `scene.knowledge.resource-unreadable`, not `resource-absent`:
  `rules::resources::classify` maps an unavailable root to the unreadable class
  precisely so a wrong directory is distinguishable from a scene naming a file
  that is genuinely missing. Check this line first either way — the root is what
  tells the two apart.
- **The code.** Stable and machine-readable; the thing to search for and the
  thing tests assert on. Renaming one is a contract change; adding one is not.
- **The document location** (`voxels[0]`). Which section, and which entry
  within it. The report is sorted by this, so it reads in the order a person
  walks the file.
- **The world position**, when the fault has one. A run ordinal is useless in a
  134-million-voxel scene. Join the chunk coordinate against the fixture's
  `*.provenance.json` to reach the authoring layer file and line.

`scene-check --json` emits the same information as structured data. Consume
that, never the text: the text is simultaneously pinned by snapshots as a
wording contract, so scraping it makes a message tweak break two things at once.

### Exit codes

| Code | Meaning                                                              |
| ---- | -------------------------------------------------------------------- |
| 0    | Valid, and acceptable under the requested strictness.                |
| 1    | The document is wrong — it failed validation, or it failed to parse. |
| 2    | The document or one of its resources could not be read.              |
| 64   | The command line was wrong (`EX_USAGE`).                             |

*Table 2: `scene-check` exit codes. Four rather than two, so a broken tool
cannot be read as a bad scene.*

"Non-zero" cannot distinguish a bad scene from a broken tool, and a
continuous-integration job that cannot tell them apart reports a mistyped
fixture path as a validation failure.

## The scene format reference

Every field is mandatory. The document has no defaults of its own: an omitted
field in a document is ambiguous between "the author meant the default" and "a
tool dropped it". Authoring defaults live in the generator, where the only
reader is the generator itself.

### Top level

| Field        | Type             | Meaning                                                             |
| ------------ | ---------------- | ------------------------------------------------------------------- |
| `version`    | `{major, minor}` | Schema version. Probed before the rest is read.                     |
| `name`       | string           | The scene's stable machine name.                                    |
| `dimensions` | `{x, y, z}`      | Grid bounds in voxels. A multiple of `chunk_size` on every axis.    |
| `chunk_size` | integer          | Chunk edge length. Design §7.1 fixes this at 32.                    |
| `palette`    | array            | Ordered voxel types. Index zero is always air.                      |
| `voxels`     | array            | Populated chunks, sorted by coordinate. An absent chunk is all air. |
| `entities`   | object           | Prototypes and spawns.                                              |
| `lighting`   | object           | Sun path, ambient bands, and probe spacing.                         |
| `knowledge`  | object           | The scene's named graph and its TriG sources.                       |

*Table 3: the scene document's top-level fields.*

### A palette entry

| Field      | Type           | Meaning                                                                                                               |
| ---------- | -------------- | --------------------------------------------------------------------------------------------------------------------- |
| `name`     | string         | Unique within the palette.                                                                                            |
| `material` | enum           | `air`, `stone`, `timber`, `roofing`, `cloth`, `ground`, `natural`, `water`.                                           |
| `passable` | object         | Six named booleans: `pos_x`, `neg_x`, `pos_y`, `neg_y`, `pos_z`, `neg_z`.                                             |
| `slope`    | enum           | `flat`, `pos_x`, `neg_x`, `pos_y`, `neg_y`.                                                                           |
| `emission` | object         | `intensity` 0–15 on design §9.2's scale, and `colour` as three 8-bit channels.                                        |
| `sim`      | object         | `fuel`, `ignition_point`, `moisture_capacity`, all Q8.8 fixed point. `ignition_point` of 65535 means "never ignites". |
| `concept`  | string or null | An ontology concept as `prefix:local`. Checked for syntax and namespace only.                                         |

*Table 4: one palette entry — everything the engine knows about a voxel
kind.*

Six named booleans rather than design §7.2's `[bool; 6]`. An array requires
every reader to agree on the index-to-face mapping and offers no way to notice
when one does not.

### A chunk entry

| Field     | Type        | Meaning                                                    |
| --------- | ----------- | ---------------------------------------------------------- |
| `at`      | `{x, y, z}` | Chunk coordinate, counted in chunks rather than in voxels. |
| `payload` | object      | Either `{"uniform": index}` or `{"runs": [...]}`.          |

*Table 5: one chunk entry in the voxel payload.*

A run is `{"length": n, "index": i}`. Runs are chunk-local **Z-major**: within
a chunk of side `s`, chunk-local `(x, y, z)` sits at `z * s * s + y * s + x`.
They sum to exactly the chunk volume, none has length zero, and no two adjacent
runs share an index. Every voxel format that left this implicit produced
incompatible third-party readers.

### A worked minimal example

Two chunks in a 64 x 32 x 32 scene: one carrying two rows of stone on its
`z = 0` layer, one entirely stone. The spawn stands on the second row.

```json
{
  "version": {"major": 1, "minor": 0},
  "name": "minimal",
  "dimensions": {"x": 64, "y": 32, "z": 32},
  "chunk_size": 32,
  "palette": [
    {
      "name": "air",
      "material": "air",
      "passable": {"pos_x": true, "neg_x": true, "pos_y": true,
                   "neg_y": true, "pos_z": true, "neg_z": true},
      "slope": "flat",
      "emission": {"intensity": 0, "colour": [0, 0, 0]},
      "sim": {"fuel": 0, "ignition_point": 65535, "moisture_capacity": 0},
      "concept": null
    },
    {
      "name": "stone-block",
      "material": "stone",
      "passable": {"pos_x": false, "neg_x": false, "pos_y": false,
                   "neg_y": false, "pos_z": false, "neg_z": false},
      "slope": "flat",
      "emission": {"intensity": 0, "colour": [0, 0, 0]},
      "sim": {"fuel": 0, "ignition_point": 65535, "moisture_capacity": 3277},
      "concept": "thy:StoneBlock"
    }
  ],
  "voxels": [
    {
      "at": {"x": 0, "y": 0, "z": 0},
      "payload": {"runs": [
        {"length": 4, "index": 1}, {"length": 28, "index": 0},
        {"length": 4, "index": 1}, {"length": 32732, "index": 0}
      ]}
    },
    {"at": {"x": 1, "y": 0, "z": 0}, "payload": {"uniform": 1}}
  ],
  "entities": {
    "prototypes": {},
    "spawns": [{
      "name": "party-start", "prototype": null,
      "at": {"x": 2, "y": 1, "z": 1},
      "facing": "pos_y", "airborne": false, "concept": null
    }]
  },
  "lighting": {
    "sun_path": {"azimuth_centidegrees": 13500, "elevation_centidegrees": 3000},
    "ambient_bands": [],
    "probe_spacing_mm": 2000
  },
  "knowledge": {
    "graph": "thy:scene/minimal",
    "sources": ["knowledge/minimal.trig"]
  }
}
```

Shipped documents are compact rather than indented. Spike A2 measured the
busiest plausible fixture at 2.33 MiB pretty-printed against a 1 MiB tolerance
and 0.74 MiB compact, and the authoring sources — not the emitted JSON — are
the review surface.

## Version history

Minecraft's 1.15-to-1.16 change to palette index packing broke every
third-party tool, precisely because the change was invisible in the data and
unrecorded. This table is the countermeasure.

| Version | Change                             |
| ------- | ---------------------------------- |
| 1.0     | Initial format (roadmap step 1.2). |

*Table 6: document version history.*

What a future bump would mean:

- **Minor** — a field added. The field carries `#[serde(default)]`, so a reader
  of a *newer* minor still loads an older document of the same major. The
  reverse does not hold: every document type carries `deny_unknown_fields`, so a
  reader that knows only 1.0 refuses a 1.1 document rather than ignoring the
  field it does not recognize. Anticipated: phase 3's fog volume in the lighting
  section, and phase 4's component vocabulary on spawns.
- **Major** — a field removed, retyped, or re-meant; an enum variant renamed; a
  closed vocabulary widened. A reader of an older major refuses the document
  rather than misreading it.

Renaming an enum variant is a **major** bump even though it looks cosmetic:
variants encode as their names in both encodings. Reordering them is safe.

## What this plane does not validate

Semantic plausibility. A scene can load clean and still be nonsense — a
building over a void, a room with no door, a staircase to nowhere. Checking
that needs judgement about passable-but-solid materials such as water, and
about what "reachable" means, which is not defined until phase 4's pathfinding
exists.

That work belongs to a scene lint built then, and the boundary is deliberate:
this plane validates *document integrity*, not level design. The two warnings
the loader does emit — a spawn inside a wall, and a spawn with nothing beneath
it — are there because they are the two cheapest checks that catch the "loads
clean, is nonsense" class, not because the line has moved.

## Authoring fixture scenes

Fixtures are authored as layered text under `assets/scenes/src/<name>/` and
compiled by `scripts/build_fixture_scenes.py`. See the developers' guide for
the authoring format; the short version is that a chunk-keyed run-length
payload is not a thing a person can edit, so nobody should.

Run `make scenes` to regenerate and `make scenes-check` to verify the committed
documents match their sources. The latter runs in continuous integration and is
what stops the sources becoming decoration.
