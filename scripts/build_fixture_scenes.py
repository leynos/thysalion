#!/usr/bin/env -S uv run --script
# /// script
# requires-python = ">=3.13"
# dependencies = ["cyclopts>=2.9"]
# ///
"""Compile layered-text scene sources into scene documents.

Fixture scenes are authored as text and compiled, never hand-written as JSON.
A chunk-keyed run-length payload is not a thing a person can edit: a
single-voxel change moves every subsequent run, and a reviewer reading the diff
sees numbers rather than a room. The authoring sources are what review looks
at; the emitted document is a build artefact that happens to be committed.

It is committed, and this is deliberately *not* a build script. A contributor
with no ``uv`` must still be able to build, test, and run the demos, so
``make scenes`` is a maintenance target rather than a build step and the
compiled fixtures are tracked files. ``make scenes-check`` regenerates into a
temporary directory and compares, which is what stops the sources and the
fixtures drifting apart until the sources become decoration.

Determinism is therefore load-bearing, and every input to it is pinned here:
sorted keys, compact separators, no indentation, one trailing newline, and a
single run-splitting rule. A generator that emits different bytes for the same
input makes the freshness check flaky, the first flaky comparison earns a skip
marker, and then the control is gone silently.

Authoring layout, under ``assets/scenes/src/<name>/``:

``scene.toml``
    Dimensions, chunk size, palette, lighting, entities, and knowledge, plus
    ``content_origin`` and ``content_extent`` — the sub-box the rasters cover.
``legend.toml``
    One character per palette name.
``layers/z###.txt``
    One text raster per populated layer. An absent layer is air.

See ``docs/execplans/1-2-deliver-the-scene-format-and-fixture-scenes.md`` for
the full authoring specification and the reasoning behind it.
"""

from __future__ import annotations

import json
import sys
import tempfile
import tomllib
import typing as typ
from dataclasses import dataclass, field
from pathlib import Path

import cyclopts

if typ.TYPE_CHECKING:
    from collections.abc import Iterable, Iterator, Mapping, Sequence

app = cyclopts.App(
    name="build-fixture-scenes",
    help="Compile assets/scenes/src/<name>/ into assets/scenes/<name>.scene.json.",
)

#: The document schema this generator writes. Must match ``SUPPORTED_VERSION``
#: in ``crates/world/src/scene/document/mod.rs``; the cross-language agreement
#: test is what keeps the two honest.
DOCUMENT_VERSION = {"major": 1, "minor": 0}

#: Palette index zero is always air, so an absent chunk and a long air run agree
#: without a lookup.
AIR_INDEX = 0

#: The chunk edge length design section 7.1 fixes.
DESIGN_CHUNK_SIZE = 32

#: How a document is serialized. Compact, because Spike A2 measured the busiest
#: plausible fixture at 2.33 MiB pretty-printed against a 1 MiB tolerance and
#: 0.74 MiB compact — and because the authoring sources, not the emitted JSON,
#: are the review surface.
#:
#: Keys are emitted in *declaration* order, not sorted. Sorting would be equally
#: deterministic and would give up the stronger guarantee: `serde` writes struct
#: fields in declaration order, so matching it lets the cross-language test
#: compare the Rust re-encoding to this output byte for byte rather than merely
#: comparing decoded values. Every dictionary built below is therefore in the
#: order its Rust counterpart declares, and `prototypes` is sorted explicitly
#: because its Rust counterpart is a `BTreeMap`.
JSON_ARGS: typ.Final = {
    "separators": (",", ":"),
    "ensure_ascii": False,
}


class SourceError(Exception):
    """An authoring source is malformed.

    Carries the offending file so a message names something the author can
    open. A traceback into this script tells them nothing they can act on.
    """


@dataclass(frozen=True)
class Box:
    """An axis-aligned box in voxels."""

    origin: tuple[int, int, int]
    extent: tuple[int, int, int]

    def contains(self, position: tuple[int, int, int]) -> bool:
        """Whether ``position`` lies inside this box."""
        return all(
            origin <= value < origin + extent
            for value, origin, extent in zip(position, self.origin, self.extent, strict=True)
        )


@dataclass
class Provenance:
    """Where each populated chunk's voxels were authored.

    The highest-leverage operational feature in this step for roughly forty
    lines: a positional diagnostic names a chunk and a chunk-local position,
    and this carries it the last step to the layer file and line a person
    actually wrote.
    """

    chunks: dict[tuple[int, int, int], set[tuple[str, int]]] = field(default_factory=dict)

    def record(self, chunk: tuple[int, int, int], layer_file: str, line: int) -> None:
        """Notes that ``layer_file`` line ``line`` contributed to ``chunk``."""
        self.chunks.setdefault(chunk, set()).add((layer_file, line))

    def to_document(self, name: str) -> dict[str, object]:
        """The provenance sidecar, sorted so regeneration is byte-stable."""
        return {
            "scene": name,
            "chunks": [
                {
                    "at": {"x": chunk[0], "y": chunk[1], "z": chunk[2]},
                    "sources": [
                        {"file": source[0], "line": source[1]}
                        for source in sorted(self.chunks[chunk])
                    ],
                }
                for chunk in sorted(self.chunks, key=lambda at: (at[2], at[1], at[0]))
            ],
        }


def read_toml(path: Path) -> dict[str, typ.Any]:
    """Reads a TOML file, naming it if it will not parse."""
    try:
        return tomllib.loads(path.read_text(encoding="utf-8"))
    except FileNotFoundError as error:
        raise SourceError(f"{path}: no such file") from error
    except tomllib.TOMLDecodeError as error:
        raise SourceError(f"{path}: {error}") from error


def parse_quantity(raw: object, unit: str, scale: int, where: str) -> int:
    """Converts an authored human quantity to the document's integer.

    ``"17.45deg"`` becomes ``1745`` centi-degrees; ``"2m"`` becomes ``2000``
    millimetres. Every quantity in the document is an integer so the whole tree
    keeps equality, ordering, and hashing, and so the two encodings cannot
    disagree about a float. That is the right choice for the format and a
    miserable one for an author, and this is the promised mitigation.

    A bare integer passes through unchanged, which is what the tests and the
    smallest fixtures use.
    """
    if isinstance(raw, int) and not isinstance(raw, bool):
        return raw
    if not isinstance(raw, str):
        raise SourceError(f"{where}: expected an integer or a string like '1.5{unit}'")
    text = raw.strip()
    if not text.endswith(unit):
        raise SourceError(f"{where}: {raw!r} does not end in {unit!r}")
    try:
        value = float(text[: -len(unit)])
    except ValueError as error:
        raise SourceError(f"{where}: {raw!r} is not a number followed by {unit!r}") from error
    return round(value * scale)


def parse_legend(path: Path) -> dict[str, str]:
    """Reads the character-to-voxel-type map."""
    legend = read_toml(path)
    for character, name in legend.items():
        if len(character) != 1:
            raise SourceError(f"{path}: legend key {character!r} must be a single character")
        if not isinstance(name, str):
            raise SourceError(f"{path}: legend entry {character!r} must name a voxel type")
    return typ.cast("dict[str, str]", legend)


def layer_files(directory: Path) -> list[tuple[int, Path]]:
    """The layer rasters, as ``(offset, path)`` sorted by offset.

    Files need not be contiguous: an absent layer is a layer of air, which is
    what keeps a two-storey keep from needing sixty-two empty files.
    """
    if not directory.is_dir():
        return []
    found: list[tuple[int, Path]] = []
    for path in sorted(directory.iterdir()):
        stem = path.stem
        if path.suffix != ".txt" or not stem.startswith("z"):
            raise SourceError(f"{path}: layer files must be named z<nnn>.txt")
        try:
            found.append((int(stem[1:]), path))
        except ValueError as error:
            raise SourceError(f"{path}: layer files must be named z<nnn>.txt") from error
    return sorted(found)


def raster_rows(path: Path, content: Box) -> Iterator[tuple[int, str]]:
    """The rows of one raster, checked for width, as ``(line number, row)``.

    A short row is an error rather than being padded. Padding hides a truncated
    edit, and a truncated edit is exactly the mistake this format invites: the
    author deletes to the end of a line and the missing voxels silently become
    air.
    """
    width, height, _ = content.extent
    rows = path.read_text(encoding="utf-8").splitlines()
    stripped = [row.rstrip() for row in rows]
    while stripped and not stripped[-1]:
        stripped.pop()
    if len(stripped) != height:
        raise SourceError(
            f"{path}: expected {height} rows for a content extent of {height}, got {len(stripped)}"
        )
    for offset, row in enumerate(stripped):
        if len(row) != width:
            raise SourceError(
                f"{path}:{offset + 1}: expected {width} columns, got {len(row)}"
            )
        yield offset + 1, row


@dataclass
class Grid:
    """The voxels a scene's rasters produced, sparse and keyed by position."""

    voxels: dict[tuple[int, int, int], int] = field(default_factory=dict)

    def set(self, position: tuple[int, int, int], index: int) -> None:
        """Places a voxel, dropping air so the map holds only content."""
        if index == AIR_INDEX:
            return
        self.voxels[position] = index

    def get(self, position: tuple[int, int, int]) -> int:
        """The voxel at ``position``; air where nothing was placed."""
        return self.voxels.get(position, AIR_INDEX)

    def chunks(self, chunk_size: int) -> list[tuple[int, int, int]]:
        """The populated chunk coordinates, in the document's sort order.

        Z-major, matching ``ChunkCoord``'s field order in the Rust model. The
        two must agree or the emitted document is not canonical and the Rust
        re-encode produces different bytes.
        """
        found = {
            (
                position[0] // chunk_size,
                position[1] // chunk_size,
                position[2] // chunk_size,
            )
            for position in self.voxels
        }
        return sorted(found, key=lambda at: (at[2], at[1], at[0]))


def read_layers(
    source: Path,
    legend: Mapping[str, str],
    palette: Sequence[str],
    content: Box,
) -> tuple[Grid, Provenance]:
    """Compiles the layer rasters into a grid and its provenance.

    Returns the grid and the provenance together because they are produced by
    one traversal: a second pass to rediscover which line placed which voxel
    would be a second chance to disagree with the first.
    """
    placer = Placer(
        content=content,
        legend=legend,
        indices={name: number for number, name in enumerate(palette)},
    )

    for offset, path in layer_files(source / "layers"):
        refuse_layer_outwith_extent(offset, path, content)
        cursor_z = content.origin[2] + offset
        relative = f"layers/{path.name}"
        for line, row in raster_rows(path, content):
            placer.place_row(row, path, LayerCursor(cursor_z, line, relative))

    return placer.grid, placer.provenance


def refuse_layer_outwith_extent(offset: int, path: Path, content: Box) -> None:
    """Refuses a layer raster the content box has no room for."""
    if offset >= content.extent[2]:
        raise SourceError(
            f"{path}: layer {offset} lies outwith the content extent of {content.extent[2]}"
        )


@dataclass(frozen=True, slots=True)
class LayerCursor:
    """Where one raster row lands, and which authored line put it there."""

    z: int
    line: int
    relative: str


@dataclass(slots=True)
class Placer:
    """Writes a raster row's voxels into the grid, recording who wrote them.

    A type rather than another parameter list: placing one voxel needs the
    content box, the legend, the palette indices, the grid, and the provenance,
    and threading five of those through two more functions is exactly the shape
    the argument cap exists to refuse. It also flattens ``read_layers``, whose
    loop-inside-loop-inside-conditional was its own readability problem.
    """

    content: Box
    legend: Mapping[str, str]
    indices: Mapping[str, int]
    grid: Grid = field(default_factory=Grid)
    provenance: Provenance = field(default_factory=Provenance)

    def place_row(self, row: str, path: Path, cursor: LayerCursor) -> None:
        """Places every non-air character of ``row``."""
        for column, character in enumerate(row):
            index = resolve(character, self.legend, self.indices, path, cursor.line)
            if index != AIR_INDEX:
                self.place(column, cursor, index)

    def place(self, column: int, cursor: LayerCursor, index: int) -> None:
        """Places one voxel and records the line that authored it."""
        x = self.content.origin[0] + column
        y = self.content.origin[1] + cursor.line - 1
        self.grid.set((x, y, cursor.z), index)
        self.provenance.record(
            (
                x // DESIGN_CHUNK_SIZE,
                y // DESIGN_CHUNK_SIZE,
                cursor.z // DESIGN_CHUNK_SIZE,
            ),
            cursor.relative,
            cursor.line,
        )


def resolve(
    character: str,
    legend: Mapping[str, str],
    indices: Mapping[str, int],
    path: Path,
    line: int,
) -> int:
    """The palette index a raster character stands for.

    An unlisted character is an error, never a silent air. Silently treating an
    unknown character as empty space is how a mistyped legend produces a scene
    with holes in it that loads perfectly well.
    """
    name = legend.get(character)
    if name is None:
        raise SourceError(f"{path}:{line}: {character!r} is not in legend.toml")
    index = indices.get(name)
    if index is None:
        raise SourceError(f"{path}:{line}: legend names {name!r}, which is not in the palette")
    return index


def chunk_payload(grid: Grid, chunk: tuple[int, int, int], chunk_size: int) -> dict[str, object]:
    """One chunk's payload: a uniform index, or a canonical run stream.

    Chunk-local Z-major, runs maximal, none of length zero, no two adjacent
    runs sharing an index. The rule is stated once, here and in
    ``crates/world/src/grid/runs.rs``, and the golden-bytes test is what
    notices when the two stop agreeing.
    """
    origin = tuple(value * chunk_size for value in chunk)
    voxels = [
        grid.get((origin[0] + x, origin[1] + y, origin[2] + z))
        for z in range(chunk_size)
        for y in range(chunk_size)
        for x in range(chunk_size)
    ]
    first = voxels[0]
    if all(index == first for index in voxels):
        return {"uniform": first}
    return {"runs": collapse(voxels)}


def collapse(voxels: Sequence[int]) -> list[dict[str, int]]:
    """Run-length encodes a chunk's voxels."""
    runs: list[dict[str, int]] = []
    for index in voxels:
        if runs and runs[-1]["index"] == index:
            runs[-1]["length"] += 1
        else:
            runs.append({"length": 1, "index": index})
    return runs


def build_voxels(grid: Grid, chunk_size: int) -> list[dict[str, object]]:
    """The document's chunk entries: sorted, all-air chunks omitted."""
    entries: list[dict[str, object]] = []
    for chunk in grid.chunks(chunk_size):
        payload = chunk_payload(grid, chunk, chunk_size)
        if payload == {"uniform": AIR_INDEX}:
            continue
        entries.append({"at": {"x": chunk[0], "y": chunk[1], "z": chunk[2]}, "payload": payload})
    return entries


def build_palette(entries: Iterable[Mapping[str, typ.Any]], where: str) -> list[dict[str, object]]:
    """Expands the authored palette into full document entries.

    Authoring supplies what differs; everything a voxel type must declare but
    rarely varies — full impassability, no slope, no emission, an inert
    material — comes from here. The document has no defaults of its own, and
    deliberately so: an omitted field in a *document* is ambiguous between "the
    author meant the default" and "a tool dropped it".
    """
    palette: list[dict[str, object]] = []
    for ordinal, entry in enumerate(entries):
        name = entry.get("name")
        if not isinstance(name, str):
            raise SourceError(f"{where}: palette entry {ordinal} has no name")
        passable = bool(entry.get("passable", False))
        emission = entry.get("emission", {})
        palette.append(
            {
                "name": name,
                "material": entry.get("material", "stone"),
                "passable": dict.fromkeys(FACES, passable),
                "slope": entry.get("slope", "flat"),
                "emission": {
                    "intensity": int(emission.get("intensity", 0)),
                    "colour": list(emission.get("colour", [0, 0, 0])),
                },
                "sim": {
                    "fuel": int(entry.get("fuel", 0)),
                    "ignition_point": int(entry.get("ignition_point", 65535)),
                    "moisture_capacity": int(entry.get("moisture_capacity", 0)),
                },
                "concept": entry.get("concept"),
            }
        )
    return palette


#: The six faces, in the order the document declares them.
FACES: typ.Final = ("pos_x", "neg_x", "pos_y", "neg_y", "pos_z", "neg_z")


def build_entities(section: Mapping[str, typ.Any]) -> dict[str, object]:
    """The entity section, with prototype keys sorted.

    Sorted because the Rust model holds prototypes in a ``BTreeMap`` and the
    document's canonical form is sorted. Python dictionaries preserve insertion
    order, so authoring order would otherwise leak into the bytes and the
    content hash.
    """
    prototypes = section.get("prototypes", {})
    return {
        "prototypes": {
            name: {
                "extends": prototypes[name].get("extends"),
                "concept": prototypes[name].get("concept"),
            }
            for name in sorted(prototypes)
        },
        "spawns": [
            {
                "name": spawn["name"],
                "prototype": spawn.get("prototype"),
                "at": {"x": spawn["at"][0], "y": spawn["at"][1], "z": spawn["at"][2]},
                "facing": spawn.get("facing", "pos_y"),
                "airborne": bool(spawn.get("airborne", False)),
                "concept": spawn.get("concept"),
            }
            for spawn in section.get("spawns", [])
        ],
    }


def build_lighting(section: Mapping[str, typ.Any], where: str) -> dict[str, object]:
    """The lighting section, converting authored angles and lengths."""
    sun = section.get("sun_path", {})
    return {
        "sun_path": {
            "azimuth_centidegrees": parse_quantity(
                sun.get("azimuth", 0), "deg", 100, f"{where} sun_path.azimuth"
            ),
            "elevation_centidegrees": parse_quantity(
                sun.get("elevation", 0), "deg", 100, f"{where} sun_path.elevation"
            ),
        },
        "ambient_bands": [
            {
                "name": band["name"],
                "at_centidegrees": parse_quantity(
                    band.get("at", 0), "deg", 100, f"{where} ambient band {band['name']!r}"
                ),
                "colour": list(band.get("colour", [0, 0, 0])),
            }
            for band in section.get("ambient_bands", [])
        ],
        "probe_spacing_mm": parse_quantity(
            section.get("probe_spacing", 2000), "m", 1000, f"{where} probe_spacing"
        ),
    }


def compile_scene(source: Path) -> tuple[dict[str, object], dict[str, object]]:
    """Compiles one authoring directory into a document and its provenance."""
    manifest = read_toml(source / "scene.toml")
    where = str(source / "scene.toml")
    scene = manifest.get("scene", {})
    content = Box(
        origin=tuple(scene.get("content_origin", (0, 0, 0))),
        extent=tuple(scene.get("content_extent", scene["dimensions"])),
    )
    palette_entries = manifest.get("palette", [])
    palette = build_palette(palette_entries, where)
    legend = parse_legend(source / "legend.toml")
    chunk_size = int(scene.get("chunk_size", DESIGN_CHUNK_SIZE))

    grid, provenance = read_layers(source, legend, [entry["name"] for entry in palette], content)

    dimensions = scene["dimensions"]
    document = {
        "version": DOCUMENT_VERSION,
        "name": scene["name"],
        "dimensions": {"x": dimensions[0], "y": dimensions[1], "z": dimensions[2]},
        "chunk_size": chunk_size,
        "palette": palette,
        "voxels": build_voxels(grid, chunk_size),
        "entities": build_entities(manifest.get("entities", {})),
        "lighting": build_lighting(manifest.get("lighting", {}), where),
        "knowledge": {
            "graph": manifest.get("knowledge", {}).get("graph", f"thy:scene/{scene['name']}"),
            "sources": list(manifest.get("knowledge", {}).get("sources", [])),
        },
    }
    return document, provenance.to_document(scene["name"])


def render(document: Mapping[str, object]) -> str:
    """Serializes a document, with every determinism input pinned."""
    return json.dumps(document, **JSON_ARGS) + "\n"


def write_scene(source: Path, output_root: Path) -> list[Path]:
    """Compiles one scene and writes its document and provenance sidecar."""
    document, provenance = compile_scene(source)
    output_root.mkdir(parents=True, exist_ok=True)
    scene_path = output_root / f"{source.name}.scene.json"
    provenance_path = output_root / f"{source.name}.provenance.json"
    scene_path.write_text(render(document), encoding="utf-8")
    provenance_path.write_text(render(provenance), encoding="utf-8")
    return [scene_path, provenance_path]


def source_directories(source_root: Path, names: Sequence[str]) -> list[Path]:
    """The authoring directories to compile, named or discovered."""
    if names:
        chosen = [source_root / name for name in names]
        for path in chosen:
            if not path.is_dir():
                raise SourceError(f"{path}: no such scene source")
        return chosen
    return sorted(path for path in source_root.iterdir() if path.is_dir())


def compare(generated: Path, committed: Path) -> str | None:
    """Reports how a regenerated file differs from the committed one."""
    if not committed.exists():
        return f"{committed}: missing; run `make scenes`"
    if generated.read_bytes() != committed.read_bytes():
        return f"{committed}: stale; run `make scenes`"
    return None


@app.default
def main(
    *names: str,
    source_root: Path = Path("assets/scenes/src"),
    output_root: Path = Path("assets/scenes"),
    check: bool = False,
) -> int:
    """Compiles scene sources, or checks the committed output is current.

    Parameters
    ----------
    names
        Scene source directory names. All of them when none is given.
    source_root
        Where the authoring sources live.
    output_root
        Where the compiled documents go.
    check
        Regenerate into a temporary directory and compare rather than writing.
        A stale or hand-edited fixture fails here; without it the authoring
        sources and the fixtures drift apart silently and the sources become
        decoration.
    """
    try:
        sources = source_directories(source_root, names)
    except SourceError as error:
        sys.stderr.write(f"build-fixture-scenes: {error}\n")
        return 2

    if not check:
        return write_all(sources, output_root)

    with tempfile.TemporaryDirectory() as scratch:
        return check_all(sources, output_root, Path(scratch))


def write_all(sources: Sequence[Path], output_root: Path) -> int:
    """Compiles every source, reporting the first failure."""
    for source in sources:
        try:
            written = write_scene(source, output_root)
        except (SourceError, KeyError) as error:
            sys.stderr.write(f"build-fixture-scenes: {source.name}: {error}\n")
            return 1
        for path in written:
            sys.stdout.write(f"wrote {path}\n")
    return 0


def check_all(sources: Sequence[Path], output_root: Path, scratch: Path) -> int:
    """Regenerates every source and compares against the committed output."""
    stale: list[str] = []
    for source in sources:
        try:
            generated = write_scene(source, scratch)
        except (SourceError, KeyError) as error:
            sys.stderr.write(f"build-fixture-scenes: {source.name}: {error}\n")
            return 1
        stale.extend(
            problem
            for problem in (compare(path, output_root / path.name) for path in generated)
            if problem is not None
        )
    for problem in stale:
        sys.stderr.write(f"build-fixture-scenes: {problem}\n")
    if stale:
        return 1
    sys.stdout.write(f"{len(sources)} scene source(s) are current\n")
    return 0



if __name__ == "__main__":
    raise SystemExit(app())
