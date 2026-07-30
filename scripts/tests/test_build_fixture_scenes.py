"""Tests for the fixture-scene generator.

Two things are worth testing here and one is not. The *authoring rules* are
worth testing, because each one is a decision an implementer would otherwise
guess at and an author would otherwise discover by being surprised. The
*determinism* is worth testing, because ``make scenes-check`` compares bytes and
a generator that is only usually deterministic makes that check flaky — and the
first flaky comparison earns a skip marker, after which the control is gone
silently.

What is not worth testing here is whether the emitted document is *valid*. That
is the Rust loader's job, and the cross-language agreement test in
``crates/world/tests/generated_fixtures.rs`` is what proves the two agree. A
Python-side reimplementation of the validation rules would be a second thing to
keep in step with the first.
"""

from __future__ import annotations

import importlib.util
import json
import sys
import typing as typ
from pathlib import Path

import pytest

if typ.TYPE_CHECKING:
    from collections.abc import Mapping

SCRIPT = Path(__file__).resolve().parents[1] / "build_fixture_scenes.py"


def _load_module():
    """Imports the generator by path.

    The script carries an inline ``uv`` metadata block and a hyphenated command
    name, so it is not importable as a package. Loading it by path is what lets
    the tests exercise the compiler directly rather than through a subprocess,
    where every assertion would be about stderr text.
    """
    specification = importlib.util.spec_from_file_location("build_fixture_scenes", SCRIPT)
    if specification is None or specification.loader is None:
        message = f"cannot load {SCRIPT}"
        raise RuntimeError(message)
    module = importlib.util.module_from_spec(specification)
    sys.modules[specification.name] = module
    specification.loader.exec_module(module)
    return module


generator = _load_module()


MINIMAL_SCENE = """
[scene]
name = "test"
dimensions = [32, 32, 32]
chunk_size = 32
content_origin = [0, 0, 0]
content_extent = [4, 2, 2]

[[palette]]
name = "air"
material = "air"
passable = true

[[palette]]
name = "stone"
material = "stone"

[lighting]
probe_spacing = "2m"

[lighting.sun_path]
azimuth = "180deg"
elevation = "45deg"

[knowledge]
graph = "thy:scene/test"
sources = []
"""

MINIMAL_LEGEND = '"." = "air"\n"#" = "stone"\n'


@pytest.fixture
def scene_source(tmp_path: Path):
    """Writes a minimal authoring source and returns a mutator for its layers."""

    def build(layers: Mapping[str, str], scene: str = MINIMAL_SCENE, legend: str = MINIMAL_LEGEND):
        source = tmp_path / "test"
        (source / "layers").mkdir(parents=True, exist_ok=True)
        (source / "scene.toml").write_text(scene, encoding="utf-8")
        (source / "legend.toml").write_text(legend, encoding="utf-8")
        for name, content in layers.items():
            (source / "layers" / name).write_text(content, encoding="utf-8")
        return source

    return build


def test_a_layer_raster_places_voxels_at_the_content_origin(scene_source) -> None:
    """Row 0 is ``content_origin.y`` and column 0 is ``content_origin.x``.

    ``y`` increases downward as the file reads, which is what makes a layer look
    like the room does when viewed from above. Getting this backwards produces a
    scene that loads perfectly and is mirrored.
    """
    source = scene_source({"z000.txt": "#...\n....\n"})
    document, _ = generator.compile_scene(source)
    payload = document["voxels"][0]["payload"]
    assert payload["runs"][0] == {"length": 1, "index": 1}


def test_an_absent_layer_is_air_rather_than_an_error(scene_source) -> None:
    # Files need not be contiguous. A two-storey keep in a 64-tall scene would
    # otherwise need sixty-two empty files.
    source = scene_source({"z001.txt": "####\n####\n"})
    document, _ = generator.compile_scene(source)
    runs = document["voxels"][0]["payload"]["runs"]
    assert runs[0]["index"] == 0, "the z = 0 layer must be air"


def test_an_unlisted_character_is_an_error_rather_than_silent_air(scene_source) -> None:
    # Silently treating an unknown character as empty space is how a mistyped
    # legend produces a scene with holes in it that loads perfectly well.
    source = scene_source({"z000.txt": "#?..\n....\n"})
    with pytest.raises(generator.SourceError, match="not in legend.toml"):
        generator.compile_scene(source)


def test_a_short_row_is_an_error_rather_than_being_padded(scene_source) -> None:
    # Padding hides a truncated edit, which is exactly the mistake this format
    # invites: the author deletes to the end of a line and the missing voxels
    # silently become air.
    source = scene_source({"z000.txt": "##\n####\n"})
    with pytest.raises(generator.SourceError, match="expected 4 columns"):
        generator.compile_scene(source)


def test_a_missing_row_is_an_error(scene_source) -> None:
    source = scene_source({"z000.txt": "####\n"})
    with pytest.raises(generator.SourceError, match="expected 2 rows"):
        generator.compile_scene(source)


def test_trailing_whitespace_is_stripped_before_a_row_is_measured(scene_source) -> None:
    source = scene_source({"z000.txt": "####   \n####\n"})
    document, _ = generator.compile_scene(source)
    assert document["voxels"]


def test_a_layer_past_the_content_extent_is_refused(scene_source) -> None:
    source = scene_source({"z009.txt": "####\n####\n"})
    with pytest.raises(generator.SourceError, match="outwith the content extent"):
        generator.compile_scene(source)


def test_an_all_air_chunk_is_omitted_entirely(scene_source) -> None:
    # The elision that makes a mostly-empty wilderness extent affordable. A
    # chunk of nothing must cost nothing.
    source = scene_source({"z000.txt": "....\n....\n"})
    document, _ = generator.compile_scene(source)
    assert document["voxels"] == []


def test_a_single_valued_chunk_becomes_a_uniform_payload(scene_source) -> None:
    scene = MINIMAL_SCENE.replace("content_extent = [4, 2, 2]", "content_extent = [32, 32, 32]")
    layer = "\n".join("#" * 32 for _ in range(32)) + "\n"
    source = scene_source({f"z{index:03}.txt": layer for index in range(32)}, scene=scene)
    document, _ = generator.compile_scene(source)
    assert document["voxels"][0]["payload"] == {"uniform": 1}


def test_runs_are_maximal_and_never_zero_length(scene_source) -> None:
    source = scene_source({"z000.txt": "##..\n..##\n"})
    document, _ = generator.compile_scene(source)
    runs = document["voxels"][0]["payload"]["runs"]
    assert all(run["length"] > 0 for run in runs)
    assert all(
        earlier["index"] != later["index"] for earlier, later in zip(runs, runs[1:], strict=False)
    ), "two adjacent runs sharing an index means the stream is not canonical"


def test_run_lengths_sum_to_the_chunk_volume(scene_source) -> None:
    source = scene_source({"z000.txt": "##..\n..##\n"})
    document, _ = generator.compile_scene(source)
    runs = document["voxels"][0]["payload"]["runs"]
    assert sum(run["length"] for run in runs) == 32**3


def test_chunk_entries_are_sorted_z_major(scene_source) -> None:
    # Sorted entries are what make the encoding canonical, which is what the
    # content hash design section 12.3 requires depends on.
    scene = MINIMAL_SCENE.replace("dimensions = [32, 32, 32]", "dimensions = [64, 64, 32]").replace(
        "content_extent = [4, 2, 2]", "content_extent = [40, 40, 1]"
    )
    layer = "\n".join("#" * 40 for _ in range(40)) + "\n"
    source = scene_source({"z000.txt": layer}, scene=scene)
    document, _ = generator.compile_scene(source)
    coordinates = [
        (entry["at"]["z"], entry["at"]["y"], entry["at"]["x"]) for entry in document["voxels"]
    ]
    assert coordinates == sorted(coordinates)


def test_prototype_keys_are_sorted(scene_source) -> None:
    # The Rust model holds prototypes in a `BTreeMap`, so the canonical form is
    # sorted. Python dictionaries preserve insertion order, which would
    # otherwise leak authoring order into the bytes and the content hash.
    scene = MINIMAL_SCENE + """
[entities.prototypes.torch]
concept = "thy:Torch"

[entities.prototypes.anvil]
concept = "thy:Anvil"
"""
    source = scene_source({"z000.txt": "####\n####\n"}, scene=scene)
    document, _ = generator.compile_scene(source)
    assert list(document["entities"]["prototypes"]) == ["anvil", "torch"]


def test_authored_angles_compile_to_centidegrees(scene_source) -> None:
    # The promised mitigation for the all-integer document's authoring cost.
    source = scene_source({"z000.txt": "####\n####\n"})
    document, _ = generator.compile_scene(source)
    assert document["lighting"]["sun_path"]["azimuth_centidegrees"] == 18_000
    assert document["lighting"]["sun_path"]["elevation_centidegrees"] == 4_500
    assert document["lighting"]["probe_spacing_mm"] == 2_000


def test_a_fractional_angle_rounds_to_the_nearest_centidegree() -> None:
    assert generator.parse_quantity("17.45deg", "deg", 100, "test") == 1_745


def test_a_quantity_without_its_unit_is_refused() -> None:
    # Guessing that a bare "17.45" meant degrees is how a scene ends up lit
    # from a hundredth of the intended angle.
    with pytest.raises(generator.SourceError, match="does not end in"):
        generator.parse_quantity("17.45", "deg", 100, "test")


def test_a_bare_integer_passes_through_unchanged() -> None:
    assert generator.parse_quantity(1_745, "deg", 100, "test") == 1_745


def test_compilation_is_deterministic(scene_source) -> None:
    # `make scenes-check` compares bytes. A generator that is only usually
    # deterministic makes that check flaky, and a flaky check is one skip marker
    # away from being no check at all.
    source = scene_source({"z000.txt": "#.#.\n.#.#\n"})
    first, first_provenance = generator.compile_scene(source)
    second, second_provenance = generator.compile_scene(source)
    assert generator.render(first) == generator.render(second)
    assert generator.render(first_provenance) == generator.render(second_provenance)


def test_the_rendered_document_is_compact_with_one_trailing_newline(scene_source) -> None:
    source = scene_source({"z000.txt": "####\n####\n"})
    document, _ = generator.compile_scene(source)
    rendered = generator.render(document)
    assert rendered.endswith("\n")
    assert rendered.count("\n") == 1
    assert ", " not in rendered, "compact separators, per the Spike A2 size decision"


def test_the_rendered_document_uses_the_rust_declaration_order(scene_source) -> None:
    # Not sorted. Matching `serde`'s struct field order is what lets the
    # cross-language test in `crates/world/tests/generated_fixtures.rs` compare
    # the Rust re-encoding to this output byte for byte, rather than settling
    # for comparing decoded values — which would miss a field one side encodes
    # differently but both sides parse.
    source = scene_source({"z000.txt": "####\n####\n"})
    document, _ = generator.compile_scene(source)
    parsed = json.loads(generator.render(document))
    assert list(parsed) == [
        "version",
        "name",
        "dimensions",
        "chunk_size",
        "palette",
        "voxels",
        "entities",
        "lighting",
        "knowledge",
    ]


def test_provenance_names_the_layer_file_and_line(scene_source) -> None:
    # What carries a positional diagnostic the last step, from a chunk-local
    # position to something a person actually wrote.
    source = scene_source({"z001.txt": "....\n..#.\n"})
    _, provenance = generator.compile_scene(source)
    assert provenance["chunks"][0]["sources"] == [{"file": "layers/z001.txt", "line": 2}]


def test_provenance_records_nothing_for_an_empty_scene(scene_source) -> None:
    source = scene_source({"z000.txt": "....\n....\n"})
    _, provenance = generator.compile_scene(source)
    assert provenance["chunks"] == []


def test_check_mode_passes_when_the_output_is_current(scene_source, tmp_path: Path) -> None:
    source = scene_source({"z000.txt": "####\n####\n"})
    output = tmp_path / "out"
    assert generator.write_all([source], output) == 0
    scratch = tmp_path / "scratch"
    scratch.mkdir()
    assert generator.check_all([source], output, scratch) == 0


def test_check_mode_fails_on_a_hand_edited_fixture(scene_source, tmp_path: Path) -> None:
    # The guard that stops the authoring sources and the fixtures drifting apart
    # until the sources become decoration.
    source = scene_source({"z000.txt": "####\n####\n"})
    output = tmp_path / "out"
    generator.write_all([source], output)
    (output / "test.scene.json").write_text('{"name":"tampered"}\n', encoding="utf-8")
    scratch = tmp_path / "scratch"
    scratch.mkdir()
    assert generator.check_all([source], output, scratch) == 1


def test_check_mode_fails_on_a_missing_fixture(scene_source, tmp_path: Path) -> None:
    source = scene_source({"z000.txt": "####\n####\n"})
    output = tmp_path / "out"
    output.mkdir()
    scratch = tmp_path / "scratch"
    scratch.mkdir()
    assert generator.check_all([source], output, scratch) == 1
