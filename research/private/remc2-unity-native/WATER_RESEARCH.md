# Water, wind and interaction research

2026-11-05. Decision report for the modern renderer at `49c68f15` and the
current Unity 6000.3.20f1 / URP 17.3.0 project. Research only: no runtime,
dependency, gameplay or installed-player changes accompany this report.

The recommended target is a hybrid: a spectrum-driven broad surface, a coastline
field for controlled shore motion, a bounded local GPU wave field for
interactions, persistent surface foam, and separate splash/spray/condensation
particles. Multi-band FFT is the leading desktop candidate for the requested
quality; periodic directional Gerstner is its lightweight baseline/fallback,
not the winner by assumption. Keep the current URP stack. Use nonlinear shallow
water only where actual run-up/current behavior is needed. These are engineering
recommendations, not measured speed claims.

## What the current code actually provides

- `Assets/Resources/ModernWater.shader` has two small vertical sine waves,
  four unconditional procedural noise-gradient evaluations per fragment,
  depth-intersection foam and planar-reflection sampling. Fading the contribution
  of a noise octave does not itself eliminate its calculation.
- `Assets/Scripts/ModernTerrainMesh.cs` shares land/water mesh vertices. Detailed
  spacing is 2 units, coarse spacing 4; classification uses source angle bit 8.
  The map is 256 cells × 4 units, repeating every 1024 units. Larger waves need
  horizontal as well as vertical bounds, and wavelengths the mesh can represent.
- The inspected terrain height is nonnegative byte height × 0.5. NightSpire's
  water corners are at zero. This presenter does **not** expose a submerged
  seabed. Camera scene-depth minus water-depth measures view-ray thickness,
  not vertical bathymetry. A depth-aware solver needs new depth data.
- `ModernWaterReflection.cs` already uses water visibility, half-width targets,
  refresh/reuse rules and a reflection-pass guard. A reflection still rerenders
  much of the scene; a cheap wave update does not make that second view free.
- Water is transparent and `ZWrite Off`; `ModernPipeline.asset` requests camera
  depth but not opaque color. Refraction needs an explicit scene-color path;
  spray/steam intersections cannot assume opaque scene depth contains water.
- Rust already has `Level1MissionEvent::WaterSplashed` and `FireballImpacted`
  producer paths. `world/fields/wind.rs` owns the existing Wind spell, not a
  general weather field. A full modern impact payload/consumer and environmental
  wind contract still need implementation; their names alone do not prove it.

## Broad comparison

| Family | Strength | Limitation | Decision for this game |
| --- | --- | --- | --- |
| Scrolling normal/noise water | Fine surface texture with little geometry | No coherent swell, propagated interactions or persistent foam | Keep only as filtered small-scale shading |
| Spectrum-informed analytic/Gerstner sum | Direct control, continuous shape, cheap CPU queries and straightforward wrapping | Repetition with too few modes; no automatic terrain response | Lightweight baseline/fallback |
| Tessendorf spectral FFT | Many dispersed wind-wave components, reusable displacement/slope maps | Transform passes/bandwidth; periodicity; does not solve a coast by itself | Leading desktop broad-wave candidate |
| Local linear wave heightfield | Many overlapping ripples/pressure footprints with fixed grid cost | Needs stable integration, boundaries and history; no bulk current or overturning | First interaction candidate |
| Nonlinear shallow-water / coupled Airy solver | Depth-driven currents, wet/dry run-up and richer wakes | Bathymetry, extra state/substeps and harder boundary behavior | Local upgrade when those behaviors visibly matter |
| Wave particles, packets, wavelets, boundary-aware wave cages | Sparse propagation or more sophisticated dispersion/boundary handling | More involved management and different accuracy/cost tradeoffs | Escalation candidates, not default infrastructure |
| Full 3D particle/grid fluids | Volumetric jets, sheets, detached water and complex splashes | Greater simulation/surface-reconstruction scope | Optional bounded hero effect, not the whole map |

The foundational [Tessendorf notes](https://jtessen.people.clemson.edu/reports/papers_files/coursenotes2004.pdf)
explain spectral synthesis. The shipped Uru system separates geometric waves
from small surface normals in [GPU Gems](https://developer.nvidia.com/gpugems/gpugems/part-i-natural-effects/chapter-1-effective-water-simulation-physical-models).
[Wave Particles](https://www.cemyuksel.com/research/waveparticles/waveparticles.pdf)
addresses propagating object disturbances; [wave packets](https://visualcomputing.ist.ac.at/publications/2017/WWP/)
and [wavelets](https://visualcomputing.ist.ac.at/publications/2018/WSW/) offer
other dispersion/scaling choices. [Wave Cages](https://visualcomputing.ist.ac.at/publications/2020/MPWWBa/)
is particularly relevant if boundary penetration becomes the decisive defect.
[Position Based Fluids](https://mmacklin.com/pbf_sig_preprint.pdf) is a reference
for the genuinely volumetric alternative, rather than evidence that it is needed.

The [2023 coupled shallow-water/Airy paper](https://research.nvidia.com/labs/prl/shallow-water-simulation/)
is a stronger physical alternative than applying plain shallow-water equations
to every wavelength. Its reported 512² CUDA examples take roughly 10ms for
simulation on an RTX 2080 Max-Q, with decomposition dominating cost. That is
useful research evidence, not a Unity/3090 benchmark or a cheap default.
Its authors also identify thin-boundary leakage, limitations with detailed
seabeds, and no general stability guarantee for the combined operator splitting.

## Broad waves and shared wind

Compare a multi-band GPU FFT surface with a smaller directional Gerstner wave
set, both driven by explicit wind/swell controls. Two 256² FFT bands are a useful
initial experiment, not a promised sufficient resolution or cost. Split wave
energy between bands without double counting, derive displacement and slopes
coherently, and share the result between views. For the analytic baseline,
compare mode counts rather than assuming a handful is automatically sufficient.
Transfer unresolved short wavelengths to filtered normal/slope detail; preserve
that energy as roughness at distance to avoid sparkling water.

FFT is an evaluation method, not the spectrum itself. Prefer explicit controls
for wind sea, fetch, directional spread and independent swell. Horvath's
[Empirical Directional Wave Spectra](https://cdn.shopify.com/s/files/1/0134/1131/9867/files/EmpiricalDirectionalWaveSpectra_DigiPro2015_Jun20_2015_Optimized.pdf?777=)
describes JONSWAP/TMA and directional models with more useful physical/artistic
controls than an arbitrary wave-gain knob. A finite-depth spectrum for a patch
still does not solve propagation over spatially changing seabed.

For exact map repetition in the analytic baseline use wave vectors
`k = (2π / 1024) * (integer_x, integer_z)`. Arbitrary wavelengths can jump at a
wrapped teleport. Texture repeats, shore data and procedural gusts also need
periodic coordinates. FFT cascade periods must likewise divide the world period;
512/64-unit footprints are one experimental pair, not a prescribed optimum.
Integer vectors restrict possible directions, so inspect repetition from flying
altitude. The comparison must establish whether the analytic baseline can match
the FFT candidate's richness and cost, rather than presuming it can.

Phase must remain continuous during wind changes. Fade component energies and
introduce new components gradually; do not instantly rotate all waves or
replace their random phases. Small ripples can respond quickly while existing
swell persists. The specific response times are art/game parameters, not a
full meteorological model.

Define environmental wind as common data: base velocity, gust parameters,
seed/time and localized influence sources. Water, vegetation, cloud drift and
steam consume it. Carpet/rival wake sources have bounded spatial extent and
strength. A procedural turbulent field such as
[curl noise](https://www.cs.ubc.ca/~rbridson/docs/bridson-siggraph2007-curlnoise.pdf)
can give particles controlled swirling motion without a full atmospheric fluid
solver. It is not a physically solved pressure field.

Rust owns any wind that changes flight, damage or other gameplay, including its
saved state and ordering. GPU detail must not feed back into those decisions.
Faithful MC2 rules retain their existing defaults; new wind forces require an
explicit modern gameplay definition. If displaced waves later affect collision,
Rust needs a deterministic coarse surface query, not synchronous GPU readback.

## Coastline, depth and surf

The first data deliverable is a periodic water-body description: level, wet mask,
distance to shore and shore gradient, exposure to incoming waves, and a separately
authored/generated bed-depth field where depth is required. A distance-derived
seabed is new presentation content, not recovered MC2 bathymetry. It must be
identified as such. Shore smoothing is also a deliberate visual approximation;
it cannot silently claim exact agreement with the source's grid-shaped collision.

Update these fields from terrain/wet-mask revisions. Preserve thin channels and
wrap neighbors. Recompute the affected influence region or the small global
field when necessary; a local edit can change distances beyond its own cell.
Use a coherent revision for geometry, wave obstacles and foam. Incompatible
history must be cleared or reconciled after land rises out of the water.
Cache directional exposure information where useful; wind rotation can sample
those directions rather than force a new terrain capture every frame.

For the inexpensive first coast layer, use shore distance/gradient to guide
incoming wavefronts, taper displacement near solids, modulate intensity by
exposure/fetch and generate intermittent lapping fronts. A sheltered pool should
not receive open-ocean surf from all sides. Wetness on adjacent ground should
advance with run-up and dry gradually. This is controlled visual surf, not a
conservative coastal simulation. The [War Thunder presentation](https://developer.download.nvidia.com/assets/gameworks/downloads/regular/events/cgdc15/CGDC2015_ocean_simulation_en.pdf)
is a particularly useful shipped reference for this separation and its LODs.

Apply shore envelopes to horizontal and vertical displacement and include their
derivatives in the normals. Share wave/LOD parameters at mesh borders and across
reflection views. Expand culling bounds for the maximum combined displacement.

True depth-dependent wave propagation obeys a relation such as
`ω² = g |k| tanh(|k| h)` for gravity waves. Changing frequency independently at
each pixel according to its depth does not implement refraction: it tears phase.
Credible bending needs a coherent travel-time/phase construction or an actual
boundary/depth-aware solver. Plain shallow-water equations are non-dispersive
and are not a universal deep-water wake model.

When actual swash/current behavior matters, compare a bounded nonlinear
shallow-water region against the controlled coast layer using the same bed and
incoming waves. A well-balanced wet/dry solver must preserve a lake at rest,
avoid negative depths and withstand terrain mutation. Do not commit to a global
solver merely because a simple foam stripe looks poor.

For curling, overhanging breakers, add a separate close-range representation.
[Horizon Forbidden West](https://advances.realtimerendering.com/s2022/SIGGRAPH2022-Advances-Water-Malan.pdf)
uses reusable animated cross-section deformations along controlled wavefronts;
its final cross-section needed substantial authoring. This supports a small
library of crest sheets/ribbons plus spray here. An ordinary single-valued
heightfield cannot itself create an overturning tube. Around cliffs and boulders,
prioritize impact spray and reflected/lapping motion rather than identical beach
breakers everywhere.

## Carpet wakes, rivals and spell impacts

Use a camera-near local GPU height/velocity field as the first interaction
candidate. Compare 128²/256² and coverage sizes experimentally. Cell-snapped
scrolling preserves overlapping history; newly exposed cells initialize cleanly.
Absorbing margins avoid visible square-patch echoes. A wrapped world coordinate
is different from circular texture storage: opposite local-patch edges must not
become physical neighbors accidentally.

The shipped [DOOM Eternal interaction-heightfield approach](https://advances.realtimerendering.com/s2020/RenderingDoomEternal.pdf)
is a useful precedent for this architecture, not a transferable resolution or
timing guarantee. Crest's pinned
[dynamic-wave compute implementation](https://github.com/wave-harmonic/crest/blob/db0658ff0b2e93e4a9e28cc2867509658b0ecc00/crest/Assets/Crest/Crest/Shaders/Resources/UpdateDynWaves.compute)
shows the practical importance of damping, flow transport and Courant limits.

Use a stable fixed visual step with interpolation. For a simple uniform 2D
explicit wave stencil, the stability constraint is approximately
`c * dt / dx <= 1/sqrt(2)`; other schemes need their own bound. Doubling grid
resolution may also require more time steps. A promised low update rate without
that check is not a performance plan. Bound catch-up work after a stall and
reset/reinitialize cosmetics when appropriate rather than allowing numerical
explosions. Simulation dispatch happens once, shared by all render cameras.

Carpets inject a moving pressure/downwash footprint with height falloff and
motion-dependent strength, not a continually positive height stamp. Use pressure
gradients/momentum or a balanced depression/raised annulus. Stamp swept paths
between observations so fast rivals leave continuous trails. A hovering magical
carpet is not automatically a ship with a fixed Kelvin wake angle. The first
linear field produces plausible interaction waves, not guaranteed physically
correct dispersive wakes or bulk currents.

On a canonical Rust water-contact/quench event, consume a stable ID once and
inject a coordinated visual bundle: surface impulse, splash sheet/droplets, foam
and rising condensation. Coordinate this with canonical prepared-audio commands,
preserving their source timing and preventing duplicate playback. The current
source-backed water-hit path can remain silent until the first live splash
update; child-allocation failure still extinguishes the projectile. Any new
decorative hiss is separately authored rather than another copy of splash audio.
Suggested event data are tick,
water-body ID, position, incident velocity, intensity and visual seed. Extend
the existing producer and observation path; do not infer impact from a projectile
disappearing. Preserve existing source collision, extinguishing, damage and
same-pass consequences. On scene generation/reset or expired history, avoid
replaying obsolete impacts into a new water state.

Initially the original contact plane remains gameplay authority. Put the cosmetic
effect on the sampled rendered surface with a documented bounded offset. If a
wave crest must extinguish a projectile earlier, that is an explicit shared
collision feature, not a decision delegated to the client shader.

## Persistent foam, spray and condensation

Foam needs a history field. Sources are compressed/breaking crests, shoreline
impacts, wakes and splashes; transport and decay keep patches around after the
crest passes. Fine bubble textures shade that coverage. The pinned
[Crest foam compute source](https://github.com/wave-harmonic/crest/blob/db0658ff0b2e93e4a9e28cc2867509658b0ecc00/crest/Assets/Crest/Crest/Shaders/Resources/UpdateFoam.compute)
is a concrete reference for advection, source injection and dissipation.

Horizontal-displacement compression can generate whitecap candidates; a
Jacobian detector cannot be meaningfully extracted from normal-only noise.
Also, wave phase speed is not the velocity of foam. A height/vertical-speed grid
does not automatically supply horizontal material flow. Retain an appropriate
velocity field or explicitly approximate drift using wind/wake/orbital motion.

Spray uses bounded, pooled ballistic droplets with wind drag; close splashes can
use oriented animated sheets, with small distant effects represented by simpler
billboards. Visible white vapor is a condensation/mist effect: use expanding,
rising, wind-advected flipbooks/particles, with procedural turbulence where it
helps. A thermodynamic steam-fluid solver is not needed for the requested look.
Start with the already installed Particle System; VFX Graph or custom GPU
particles become candidates when measured emitter counts justify the dependency.
The [17.3 VFX Graph requirements](https://docs.unity3d.com/Packages/com.unity.visualeffectgraph@17.3/manual/System-Requirements.html)
still qualify URP platform support and require compute/SSBO capability; installing
it is not itself platform qualification.

Cap active sources, particle counts, covered pixels and history lifetime. Merge
or downgrade distant cosmetic inputs without changing Rust outcomes. Surface
foam should be drawn in the water material through its data field, rather than
assuming a generic decal projector can write on this transparent shader.
Use shared water-height/depth information for particle intersection and submerged
clipping; opaque scene-depth soft fading alone does not solve it.

## Rendering and performance architecture

Separate water geometry ownership from the land submesh when independent dense
near-field geometry is needed. Use stitched/geomorphed rings or chunks, with
screen-error and wavelength selection. Small ripples belong in normals when
they cannot move a visible silhouette; avoid tessellating the whole map for them.
World-anchored simulation history must not slide with the camera mesh.

Replace costly per-pixel procedural octave work where filtered slope/normal
textures provide comparable detail. Match the normals to geometric displacement.
Use Fresnel reflection, roughness-filtered glints, depth-based absorption and
restrained scattering. Refraction is an optional explicit opaque-color copy and
sampling path; reject samples that pull foreground objects into the water.

Use a sky/probe fallback at all times, with the existing planar reflection as a
budgeted detail layer. Visibility, projected water coverage, camera motion,
reflected-scene changes and texture age should determine refresh. A static
camera with moving fireballs still needs appropriate updates. Avoid stacking
expensive reflection techniques before measuring the existing one. Large waves
also make a single flat reflection plane an approximation.

The same wind/wave time and local simulation serve main, reflection and eventual
stereo views. Screen depth/color and reflection sampling still require explicit
per-eye implementation and testing for PCVR; PCVR compatibility is not a claim
that the same settings fit standalone Quest.

Measure the complete cost: broad waves, local substeps, foam, field rebuilds,
uploads, geometry, water shading, reflection rendering and particle overdraw.
Use repeated matched release-player shoreline, flight, high-altitude, mutation
and impact-storm runs. Report whole-frame and water-pass CPU/GPU p50/p95 with
clock telemetry. The whole-game 60Hz budget is 16.67ms; research does not establish
a water sub-budget or a 3090 speed guarantee. Initial experiments must determine
that allocation. Benchmark the actual foreground path when it is available.

## Existing-system option

Evaluate Crest's current URP edition as an implementation/reference candidate
before building a general-purpose ocean framework. Its official
[dynamic-wave documentation](https://docs.crest.waveharmonic.com/Manual/Simulation/Ripples.html)
already separates environmental and interaction waves. Its
[shoreline extension](https://docs.crest.waveharmonic.com/Packages/ShallowWater/Shorelines.html)
can follow a viewer in the documented displacement-injection mode; an older
third-party listing saying it cannot move is stale. Terrain profile and resets
still matter, as the [current manual](https://docs.crest.waveharmonic.com/Packages/ShallowWater/Manual.html)
explicitly explains.

The official [release history](https://docs.crest.waveharmonic.com/About/History.html)
observed for this research lists 5.10.2. The public MIT repository and its pinned
compute sources are the older built-in-renderer system, not a drop-in free URP
edition. Verify the actual package, licensing and Unity version before adoption.
Its [platform guide](https://docs.crest.waveharmonic.com/Manual/Guides/Platforms.html)
lists Linux/SteamDeck users but says the authors do not have those platforms
for testing. Its [performance guide](https://docs.crest.waveharmonic.com/Manual/Guides/PerformanceGuide.html)
also cautions that its feature-rich shader needs configuration.

My recommendation is to qualify the narrow required feature set, not adopt every
feature by default. The decisive comparison is integration plus measured runtime
cost on our toroidal, deformable terrain. If Crest meets that requirement, use
it; if it fails, implement the focused hybrid above using the same wind/events
and water-body data. Switching the whole project to HDRP solely for its water
would introduce unrelated renderer migration work.

## Delivery order and decisive checks

1. Water-body/coast data plus the periodic wind-wave surface. Demonstrate calm
   versus windy exposed water, sheltered water, shore contact and 1024-unit wrap.
2. Actual Rust impact/contact observation to one bounded local impulse, splash,
   condensation and sound. Demonstrate a real quench once, including save/load
   and duplicate-observation controls.
3. Player/rival pressure trails and persistent foam. Demonstrate hover, speed,
   altitude falloff, overlapping trails and terrain appearing inside the field.
4. Shore lapping/wetness and selected spray/breaking fronts. Compare the visual
   coast layer with a local shallow-water candidate where it adds visible value.
5. Qualify full-frame pacing, rendering correctness and resource bounds through
   ordinary flight and heavy interactions; retain the cheaper representation
   wherever the more expensive candidate has no visible benefit.

Acceptance includes: no phase/normal/foam seam at wrap or LOD edges; no camera-
dragged history or opposite-patch echoes; stable energy/decay and no NaNs during
stalls; correct dry/wet mutation; no spray clipping through the water; consistent
reflections; and unchanged Rust hashes/RNG/damage when only visual quality changes.
Nonlinear SWE additionally needs lake-at-rest, nonnegative-depth and wet/dry
boundary controls. Actual dynamics such as new wind forces get separate shared
gameplay verification.

Two independent repository-aware reviews agreed on the layered architecture,
the missing bathymetry, and local linear interactions before nonlinear SWE.
The independent first-pass preference for Gerstner minimized integration
uncertainty, rather than demonstrating the best quality/cost for this user's
goal. Adversarial review corrected that distinction: FFT is the leading desktop
quality candidate, with the same local interaction/coast architecture. The
uncertain point is the actual analytic/FFT crossover and whether synthetic coast
control meets the desired look; the experiments above settle those questions.
Oracle CLI is absent and the installed complementary-family CLI is not signed
in, so no external-family Oracle verdict is claimed.
