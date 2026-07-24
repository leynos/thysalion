# Architectural decision record (ADR) 004: tiered lighting with software-ray-marched DDGI

## Status

Accepted, 2026-07-22. Lighting is three additive tiers — clustered forward
direct lighting, a flood-fill voxel light field, and a DDGI-style probe grid
whose rays are DDA-marched through the voxel grid in compute — with no hardware
ray-tracing dependency.

## Date

2026-07-22.

## Context and problem statement

The reference art demands warm local lights against cool ambient fill,
day/night and weather moods, and believable interior bounce
([design](thysalion-design.md) §9), on mid-range GPUs without ray-tracing
hardware (design §2.2). Candidate mechanisms: Bevy's experimental
hardware-raytraced Solari, voxel cone tracing, flood-fill lighting alone, or an
irradiance-probe system with software ray generation.

## Decision drivers

- Hardware floor: Solari requires ray-tracing GPUs and currently an
  Nvidia-only denoiser — unacceptable as a baseline.
- The voxel grid is already GPU-resident for the flood-fill field, so
  probe rays can DDA-march it with no BVH or RT hardware; published precedent
  exists (Wang et al. 2019, <https://doi.org/10.1145/3306131.3317024>; Hu et
  al. 2020, <https://arxiv.org/abs/2007.14394>).
- DDGI's depth-moment visibility test is the strongest published defence
  against light leaking through walls — critical for cut-away interiors lit
  differently from exteriors (Majercik et al. 2019,
  <https://jcgt.org/published/0008/02/01/>).
- Bounded dioramas keep probe counts in the low thousands (design §9.3),
  far below the volumes the production literature hardens against.
- Gameplay needs an instant, CPU-visible light level (stealth,
  perception); a flood-fill field provides it regardless of the GI tier.

## Options considered

### Option A: tiered — direct + flood-fill + software-DDGI probes

### Option B: voxel cone tracing

The best-documented shipped voxel-game GI (The Tomorrow Children,
<https://doi.org/10.1145/2775280.2792546>).

### Option C: flood-fill only

### Option D: Solari (hardware ray tracing) as baseline

| Topic                          | A: tiered probes        | B: cone tracing       | C: flood-fill only | D: Solari         |
| ------------------------------ | ----------------------- | --------------------- | ------------------ | ----------------- |
| Hardware floor                 | Compute only            | Compute only          | Trivial            | RT GPU + DLSS     |
| Leak control                   | Depth-moment visibility | Weak at thin walls    | N/A (no bounce)    | Good              |
| Interior bounce / colour bleed | Yes                     | Yes                   | No                 | Yes               |
| Cost profile                   | Amortized probe budget  | Full-volume per frame | Negligible         | High              |
| Gameplay light level           | Via tier 1              | Needs extra field     | Native             | Needs extra field |
| Maturity for this stack        | Composes with Bevy PBR  | Bespoke build         | Trivial            | Experimental      |

_Table 1: comparison of lighting options._

## Decision outcome

Option A. Option C is insufficient for the visual target but is retained inside
A as tier 1 — it is the low-spec preset, the gameplay light source, and the
probe fallback, so the investment is never wasted. Option B loses on leak
control and full-volume cost in scenes dominated by interiors. Option D
violates the hardware non-goal; it remains a possible ultra tier once stable
(design §9.7). Production hardening follows the published playbook: self-shadow
bias, probe state machine, per-scene volumes (Majercik et al. 2020,
<https://arxiv.org/abs/2009.10796>), with importance-based ray allocation (Liu
et al. 2023, <https://doi.org/10.1145/3585500>) as the named budget mitigation.

## Known risks and limitations

- DDGI is diffuse-only and low-frequency; contact detail comes from
  ambient occlusion and shadow maps, and specular GI is out of scope.
- Probe hysteresis trades response latency for stability; fast-moving
  light sources (a thrown torch) will lag in the GI term while tier 0 responds
  instantly — accepted as consistent with the art style.
- The DDA march samples the same grid the flood-fill uses; non-voxel
  meshes (characters, props) do not occlude probe rays. Accepted: their scale
  is below the GI frequency the probes represent.
- Verification of leak behaviour is empirical, per fixture scenes
  (design invariant I5), not a geometric proof.
