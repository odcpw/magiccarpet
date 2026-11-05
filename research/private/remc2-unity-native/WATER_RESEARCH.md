# Water, wind and interaction research

2026-11-05. Initial decision report for the modern renderer at `49c68f15` and the
current Unity 6000.3.20f1 / URP 17.3.0 project. Research only: no runtime,
dependency, gameplay or installed-player changes accompany this report.
The post-capture correction below supersedes the initial condensation-rendering
recommendation where the player must enter and disturb the cloud.

## Post-capture correction: steam, fire and night lighting

The water prototype landed at `dda3a17d`. The subsequent uncommitted night/steam
experiment was rejected visually on 2026-11-05. In capture `night-weather-02`,
steam reads as overlapping cotton balls, torch cards as luminous triangles, and
two staged torches produce an implausibly broad pink horizon. The capture is at
`/mnt/pinokio-shared/magiccarpet-build-proofs-20261105/night-weather-02/`.
This is evidence against the representation, not a reason to tune opacity again.

The current steam uses 24 repeated billboard puffs per parcel, arranged around
a spherical envelope, with approximately uniform lighting. Parcel heat/water
accounting constrains quantities but cannot determine a turbulent 3D density
field. Correct scalar thermodynamics does not validate those images. The 72
passing Modern EditMode tests cover their encoded behavior, not visual quality.

### Production methods and the choice for this game

| Method | Useful property | Important limit | Decision |
| --- | --- | --- | --- |
| Authored smoke flipbook with six-way lighting | Detailed baked motion and directional light response at particle cost | Baked motion, flat close views and no coherent interior between cards | Distant steam and secondary torch smoke |
| Persistent local 3D density with analytic transport | Continuous interior, overlapping plumes and lasting swept disturbances | Advection diffusion, ray sampling and light attenuation cost | First candidate for near steam; stored velocity is an escalation |
| World-scale fluid simulation | General coupled motion | Unnecessary scope and cost for isolated quenches | Do not adopt |
| Authored flame flipbook with separate emission/opacity | Controllable silhouette and exposure | Needs actual quality art, not a generic noise mask | Torch flame baseline |

Unity's [six-way lighting documentation](https://docs.unity3d.com/Packages/com.unity.visualeffectgraph@17.3/manual/six-way-lighting.html)
describes baking directional responses into two RGBA flipbooks and relighting
them at runtime. This is a real asset pipeline; renaming grayscale noise does
not supply the missing directional information. Unity 6000.3 also exposes a
[URP Six Way Shader Graph](https://docs.unity3d.com/6000.3/Documentation/Manual/urp/prebuilt-shader-graphs-urp-sixway.html).
Our installed URP 17.3 contains this support; VFX Graph is not currently a
project dependency. No engine migration is required. The project's custom
eight-light arrays do not automatically become Unity lights: choose an explicit
bridge or native URP light integration, and measure it. The existing builder
already requests per-pixel additional lighting; inspect the installed renderer
asset before attributing performance to a rendering-path change.

[GPU Gems 3, chapter 30](https://developer.nvidia.com/gpugems/gpugems3/part-v-physics-simulation/chapter-30-real-time-simulation-and-rendering-3d-fluids)
provides the concrete density/velocity, moving-obstacle and volume-rendering
foundation. Following the Oracle critique below, first compare 32³, 48³ and 64³
cells over the same fixed, world-anchored 12m region shared by nearby impacts.
These sizes are trial parameters, not demonstrated sufficiency. Keep scalar
thermodynamics separate; inject condensate and advect density with analytic
wind, buoyancy and a bounded list of persistent, decaying carpet-wake primitives.
Do not begin with a stored velocity texture. Procedural [curl noise](https://www.cs.ubc.ca/~rbridson/docs/bridson-siggraph2007-curlnoise.pdf)
can add unresolved eddies; it is not a substitute for persistent state or a
pressure solve. Escalate to pressure projection when divergence/compression
actually breaks the observed result. Pure advection is an explicit visual
approximation, not a claim of resolved multiphase fluid dynamics.

Render density with front-to-back extinction and scattering, scene-consistent
moon/ambient/local light, and bounded light attenuation through the cloud.
Steam is not emissive. Clip rays to the volume and scene depth, skip empty
regions and stop once transmission is negligible. Establish a full-resolution
reference before introducing reduced resolution or temporal reconstruction;
camera entry, fast flight and disocclusion are the difficult cases. Share the
simulation across views rather than stepping it again for reflections or eyes.

For sky clouds, [Guerrilla's Nubis Evolved](https://www.guerrilla-games.com/read/nubis-evolved)
is especially relevant because it explicitly addresses fly-through clouds and
fast-motion artifacts. Its reported timings belong to its engine and content,
not ours. Weather coverage and wind should control density independently of
rendering quality. Two torch sources must not acquire city-scale atmospheric
glow merely because a locality exists.

For flames, use simulated/authored looping silhouettes, controlled emissive
cores, and independent opacity/emission masks. [SideFX's Pyro Bake Volume](https://www.sidefx.com/docs/houdini/nodes/sop/pyrobakevolume.html)
separates smoke, scattering, temperature color and fire intensity; that separation
is the useful production lesson, not a requirement to adopt Houdini. Calibrate
the flame and its ground illumination together under fixed exposure. Six-way
lighting benefits associated smoke; it does not fix a triangular flame silhouette.

### Smallest decisive next experiment

Build one quench cloud in a simple scene under fixed daylight and moonlight.
Record formation, orbit, entry, a swept carpet pass, the wake after exit, and
two overlapping sources. Reject visible disks, spherical shell structure,
camera-attached holes, clipping, temporal trails and unexplained self-glow.
The disturbance must persist after the actor leaves. Compare identical camera,
wind, source timing and exposure with effects off, the rejected renderer and
the volume candidate. Measure simulation, rendering and total frame-time
distributions without video encoding; include the inside-cloud case.

An initial budget of about 2ms incremental GPU cost for one nearby plume at
1080p is a proposed design constraint, not a measurement or guarantee. Bound
active regions, emitters, light samples and screen coverage; do not multiply a
large solver by every fireball. 128³ has eight times as many cells as 64³, and
near-cloud pixel coverage can dominate even when cell counts are small. Distant
effects use baked assets only after the transition is visually checked.

Retain the tested variable-wind and heat/water contracts, with the authored 2MJ
calibration and entrainment assumptions explicit. Retain the wake input contract,
but replace its puff consumer. FrankenSim was evaluated for property calculations;
it does not currently supply the required ready-made moist-plume integrator or
Unity renderer. This research adds no dependency. The initial claim that Oracle
was unavailable was incorrect: only PATH lookup had failed. Oracle 0.22.2 is
installed at
`/home/oliver/.npm/_npx/76f9f399e0e37feb/node_modules/@steipete/oracle/dist/bin/oracle-cli.js`
and its configured browser profile is usable. The source-backed review completed
as `magiccarpe-steam-volume-review`, with verified browser picker selection of
GPT-5.6 Sol and Extra High effort. Six files supplied approximately 17.43k input
tokens; retained output is
`../generated/night-weather-20261105/oracle-steam-review.md`.
The attempted Claude CLI cross-examination failed with expired authentication;
no complementary-family verdict is claimed. The recommendation above is based
on the observed failed capture, inspected project code and cited primary sources.

### Oracle critique and resulting adjustment

The review supports a continuous density volume but challenges the need to
simulate velocity. Start with one persistent scalar density texture transported
by analytic wind/buoyancy and persistent wake primitives. Stored velocity and
pressure projection are separate escalation steps. A 12m box gives 25cm cells
at 48³ and 18.75cm at 64³; growing the box to 40m would degrade those to roughly
83cm and 63cm. Fix world-space coverage before comparing resolutions.

The first experiment therefore excludes local volumetric torch lighting and
uses simple moon/ambient scattering. Verify the explicit transparent-water
intersection separately from opaque depth. Inject overlapping sources into one
field so independently sorted volumes cannot double-count their overlap.
Measure advection smearing and raymarch cost independently. A larger grid does
not automatically repair poor transport or lighting.

Oracle suggests renormalizing density to the parcel's condensed mass after
advection. Accept this only with an explicit mass budget: escaped, transferred
and evaporated material must leave that budget, and injection must not add the
entire existing mass again every frame. Blindly restoring the full parcel mass
inside a cropped box would brighten remaining material as others leave. This
qualification is our engineering review of the suggestion, not a tested result.
A shared field also needs a deliberate treatment of multiple parcels' phase
changes; a global normalization alone cannot establish local thermodynamics.

The decisive comparison is whether density-only transport preserves a convincing
swept wake through overlapping plumes without unacceptable smearing or
compression at the tested resolutions. If it does, no velocity solver is needed.
If it fails, diagnose transport/resolution versus missing flow state before
adding stored velocity. The review changes the next experiment; it proves no
runtime performance or visual quality. It did not inspect the video directly.

## Initial water research at revision 49c68f15

The recommended target is a hybrid: a spectrum-driven broad surface, a coastline
field for controlled shore motion, a bounded local GPU wave field for
interactions, persistent surface foam, and separate splash/spray/condensation
particles. Multi-band FFT is the leading desktop candidate for the requested
quality; periodic directional Gerstner is its lightweight baseline/fallback,
not the winner by assumption. Keep the current URP stack. Use nonlinear shallow
water only where actual run-up/current behavior is needed. These are engineering
recommendations, not measured speed claims.

## What the initial source snapshot provides

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
At that initial checkpoint Oracle was not found on PATH and the installed
complementary-family CLI was not signed in. The later installation discovery
and Oracle review are recorded in the post-capture section above; the earlier
absence claim was incorrect.
