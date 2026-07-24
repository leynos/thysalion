# Architectural decision record (ADR) 001: meshed voxel rendering pipeline

## Status

Accepted, 2026-07-22. Chunks are greedy-meshed into raster geometry; raymarched
volume rendering is the documented fallback, not the baseline.

## Date

2026-07-22.

## Context and problem statement

Thysalion renders dense, high-detail voxel dioramas
([design](thysalion-design.md) §8) in the visual style of the reference
concept art. Two rendering families exist in the Rust/Bevy ecosystem: meshing
voxels into triangles and rasterizing them through the standard pipeline, or
raymarching a voxel volume (sparse voxel octree or brickmap) in a custom GPU
pass. The choice determines material and lighting integration, hardware
requirements, and how much of the Bevy renderer can be reused.

## Decision drivers

- The visual target is chunky stylized voxels, already low-poly after
  greedy meshing — not microvoxel detail.
- Baseline hardware is mid-range, without ray-tracing units (design
  §2.2).
- Bevy 0.19's PBR, shadows, clustered lighting, and batched
  multi-draw-indirect path work on raster meshes.
- Ecosystem maintenance reality: the only voxel crate tracking Bevy 0.19
  (`bevy_voxel_world`) is a meshing crate; the raymarching projects surveyed
  are unmaintained or lighting-incomplete.

## Options considered

### Option A: greedy-meshed raster pipeline

Chunks meshed on the CPU task pool (binary greedy mesher) and drawn as standard
Bevy meshes with a custom voxel material.

### Option B: raymarched volume pipeline

Voxels kept in a GPU octree/brickmap and rendered by a WGSL raymarcher (the
VoxelHex / bevy-voxel-engine approach).

| Topic                   | Option A: meshed                | Option B: raymarched             |
| ----------------------- | ------------------------------- | -------------------------------- |
| Bevy integration        | Native (PBR, shadows, batching) | Custom pass, replicates lighting |
| Hardware floor          | Mid-range raster                | GPU-heavy, effectively high-end  |
| Ecosystem support       | Maintained crate on Bevy 0.19   | Unmaintained or lighting-WIP     |
| Per-voxel visual detail | Vertex/texture attributes       | Exact per-voxel                  |
| Edit response           | Re-mesh chunk (~ms, async)      | Update volume texture (fast)     |
| Risk                    | Low, well-trodden               | High, bespoke                    |

_Table 1: comparison of rendering options._

## Decision outcome

Option A. The art style does not need per-voxel raymarching fidelity, and
option A inherits the entire Bevy lighting and batching stack that the tiered
lighting design ([ADR 004](adr-004-tiered-lighting-software-ddgi.md)) builds
on. The voxel grid still resides on the GPU for lighting and simulation
purposes, so a raymarched tier can be added later without restructuring;
VoxelHex's sparse-voxel brick tree is the documented starting point should the
visual-target review fail (design §15).

## Known risks and limitations

- Greedy meshing merges faces per material, so per-voxel variation must
  travel in vertex attributes and texture indices (design §8.1).
- `bevy_voxel_world` carries infinite-world assumptions that must be
  bypassed; if divergence grows, the chunk-management layer may need to be
  forked or replaced. The extension hooks used are its public, documented
  surface, limiting that risk.
