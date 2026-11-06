# Modern NightSpire renderer

The opt-in desktop renderer uses the same live Rust campaign session as the
original presentation. Rust still owns movement, combat, terrain changes,
input admission, saves and replay. The first supported modern environment is
NightSpire (Level 1); other destinations retain their existing presentation.

## Level 1 buildings — 2026-11-06

The sandbox now uses the 32 verified Level 1 house placements and four BUILD
profiles (26, 29, 30, 42). The initial sandstone/dome interpretation was rejected:
it changed the original architectural identity. Its source and captures remain
under ignored `../generated/buildings-20261106/rejected-domes-source` and `c2`.
The replacement uses original sampled envelopes, dark lower masonry, blue-gray
upper walls, short dark roof caps and small rectangular warm windows.

`Tools/prepare_modern_building_layout.py` reads the owner BUILD bank and Level 1
entity package, opens the qualified frozen replay with its matching native library,
and validates all 1,220 positive building-height samples before writing an ignored
`Builds/SandboxInputs/level1-building-source-layout.json` descriptor. The descriptor
contains source profiles and positions; no owner geometry bytes are embedded in
committed source. Its hash is pinned in the installation's `sandbox-inputs.json`.
The launcher checks that hash before supplying `MC_SANDBOX_BUILDING_LAYOUT`.

`ModernBuildingLayout` validates the entire original height-plane hash, every
replacement byte, dimensions, complete positive-cell coverage and mesh samples.
It then flattens those positive cells in a **visual copy only**. Zero-offset skirt
cells, water, Rust simulation and the source snapshot remain intact.
`ModernBuildingMeshes` triangulates the source lattice with its diagonal parity;
all three LODs retain the same body. Meshes are shared per profile, with one material
and renderer per active LOD. Masonry bevels and roof seams are filtered shader
detail. Sparse facet-fitted window frames and glazing are newly authored, not an
exact reconstruction of every original texture aperture. Night windows emit and
contribute outward-facing spill within the existing eight-light budget.

The retained Classic image
`../generated/nightspire-modern-20261103/checkpoint27/nightspire-classic27.png`
and source-indexed `../generated/playtest-frame-20261026/views/night-32.png`
ground the visual comparison. They are project Original-renderer captures, not
original-binary captures. The geometry comes from the BUILD samples, not guesses
from those images. This frozen sandbox binding does **not** implement live house
construction, destruction, interiors or building collision.

Verification is self-checked against these source references. Unity EditMode
`MagicCarpet.Native.Tests.Modern` passed 94/94 checks in 26.33 seconds, including
source-sample preservation across LODs, glazing anchor ray hits, atomic terrain
replacement rejection, staging rollback, shared meshes and bounded window lights.
The removed dome/arch-specific tests were replaced with source-heightfield tests;
the old fallback settlement placer was removed. The canonical descriptor command
independently verified all 32 buildings and 1,220 samples against the installed
frozen native `a98a8317` replay. Installed Linux build: `build-32a44fhy`.

Individual final-player day views cover small/long/tall/stepped buildings and the
zero-base coastal settlement. Five moving night captures retain rectangular
glazing; the first, middle and last were inspected. That inspection also caught
and fixed a window-emitter cutoff that previously prevented facade spill.
The actual sandbox launcher, keys 5–9, day/dusk/moonlight changes, free flight and
normal exit passed on a private X display. The first control harness sent two N
presses in one frame; its missing moonlight result is retained, and the recheck
uses a separate frame between presses. X-display readback times are not used as
render-performance measurements. The physical desktop remains untested.

Three comparable final-player night runs on RTX 3090 / Vulkan / private Wayland
at 1600×900 measured 8.36, 8.53 and 8.75 ms mean; p95 was 8.45, 9.29 and
11.08 ms respectively (120 samples each after 10 seconds of warmup, captures
after measurement). Medians were 8.33–8.36 ms. These qualify the paused sandbox
view with all 32 buildings, not ordinary campaign CPU load, Windows or VR.

Evidence: `../generated/buildings-20261106/` contains input hashes, build/test logs,
source descriptor verification, launcher receipts and rejected candidates;
individual Wayland frames/logs are in `../generated/environment-finish-20261106/wayland/buildings-c3-*`
and `buildings-c4-*`. UBS examined 12 changed C#/Python files: no critical findings;
its five Python JSON-handling warnings are intentional fail-closed exceptions in
the authoring command. C# analysis is partial because of scanner flow/preprocessor
limits; `ubs doctor --fix` reported a healthy installation. This is not a claim
of complete static-analysis coverage.

## Environment sandbox — 2026-11-06

The environment milestone is accepted; buildings are the next visual delivery,
before replacement creatures and menus. Start the installed sandbox from this directory:

```sh
./run-sandbox.sh
```

The launcher reads the qualified Modern installation's ignored
`sandbox-inputs.json`: an owner-local terrain replay, package/cache paths and
exact native/replay hashes. It refuses mismatches. The snapshot is preserved in
`Builds/SandboxInputs/environment-replay.json`; each run gets a separate temporary
save destination. No ordinary campaign save is selected. The first attempt to
use the current campaign defaults failed because the frozen Modern native cannot
open their newer package schema; the retained failure is not bypassed. This
qualified snapshot is an environment input, not earned campaign progression.

After the terrain snapshot arrives, the sandbox owns the camera and controls.
`MagicCarpetRuntime` skips campaign input, fixed updates and campaign GUI only
while this opt-in environment is ready. Source actors are not drawn in the
sandbox. Normal Modern gameplay and the separate Original installation retain
their existing paths. This is free-camera exploration with ground clearance,
not new Rust flight physics or collision against decorative rocks and plants.

| Control | Action |
| --- | --- |
| Hold right mouse; WASD | Look and fly |
| Q / E; Shift / Ctrl | Descend / rise; fast / precise flight |
| 1 / 2 / 3 / 4 | Shoreline / grove / overlook / sky |
| 5 / 6 / 7 / 8 / 9 | Settlement / small house / long house / coastal houses / stepped house |
| N / G | Cycle daylight, dusk, moonlight / calm, breeze, gusts |
| Cloud slider | Adjust cloud coverage independently of wind |
| Space / R | Aim at water for a 2 MJ quench / cold splash and ripple |
| T / Backspace | Toggle the three sandbox torches / clear water and steam |
| F1 / Escape / F10 | Hide controls / release mouse / exit |

Water targeting rejects sky, distant/invalid rays, dry land and intervening
terrain. Free flight drives the same persistent steam wake and water downwash
entry points as the existing presentation effects. These experiments are
cosmetic sandbox inputs; a real gameplay hot-contact ABI remains separate.
`-mc-sandbox-tour` records eleven lighting/water/steam stages;
`-mc-sandbox-study 0..12` holds a view or effect study, including a four-cloud
stress view and a foreground-flame/background-steam control. Studies 13–24 add
the Level 1 buildings in day, dusk and night, including reverse and coastal views.

The finishing pass reconstructs the quantized landform before adding bounded
relief. Original water anchors and flat ground remain stable; terrain revisions
invalidate the reconstruction halo and affected sections. Vegetation uses dry
supporting footprints, and scanned boulders are buried into slopes. These are
presentation changes; Rust retains the original terrain, collision and saves.

The source shoreline helper reads the indexed owner atlas and all eight source
orientations. At sea level, reflective transition tiles with water pixels become
continuous submerged banks in Modern; their opaque source texels are not raised
sand islands. Raised tiles remain land. One-metre filtered source coverage adds
small foam variation; the signed coastal distance controls the continuous water
edge. This deliberately differs from reproducing each original pixel as geometry.
A supported dry-side overlap joins the water to real bank geometry. No owner
atlas is committed. The helper, terrain mutation and orientation cases are tested.

Water keeps the two-band FFT wind sea and bounded local interaction field.
Shore crests break into irregular patches; the shallow bank, lapping displacement,
foam and spray provide the shoreline run-up presentation. This is a real-time
approximation, not an overturning-wave fluid solver. The foam texture now resolves
its cells at 512², with fine bubbles modulating larger patches. This removes the
previous square maze and coarse wire-net appearance without per-pixel hash loops.
Texture generation is startup work, not a frame update.

Daylight, dusk and moonlight share coherent key light, ambient light, fog, cloud
scattering and water reflection. Stars fade with daylight; the moon and clouds
remain world-oriented. Variable weather drives water, plants, clouds and steam.
Torch fixtures search for dry supporting ground. Their moving tapered flames
start inside the brazier, and the local light range and intensity are restrained.
The fixture arrangement demonstrates lighting, not an ordinary settlement spawn.

Steam now retains up to four independent 12 m fields: the first defaults to 48³,
additional fields are allocated lazily at 32³. Nearby impacts can share a field;
separated impacts do not recenter or erase an older cloud. At capacity, the oldest
field fades for one second before accepting the newest pending impact. Repeated
overload replaces that pending impact. This bounded handoff preserves distant
clouds within the four-field budget; it is not unlimited large-scale weather.

Two local lights per field scatter through steam, selected consistently for the
whole volume. Per-ray light ranking had created a straight dark wedge and was
removed. A restrained multiple-scattering approximation reduces the black-core /
white-rim appearance. Advected optical detail and the simulation's transported
mass are separate: optical detail is not a new mass-conservation claim. A dense
front still looked like a solid blue-gray plug with density multiplication alone;
narrowing its noise contrast did not fix that and was reverted. Advected 3D
sampling distortion now shapes visible lobes and gaps. Both the steam renderer
and flame extinction use `ModernSteamOptics.hlsl`, with the matching per-field
clock. Density rejection happens after distortion, and the true volume boundary
still fades out, avoiding clipped wisps or an exposed simulation box.
Foreground steam attenuates flame sheets; steam behind a flame does not. Persistent
wake redistribution, world wrapping, capacity handoff and reset are GPU-tested.

The accepted steam sampling candidate uses `max(.25, rayLength/48)` metre steps,
retaining four light-attenuation samples. Matched cloud-edge frames showed no new
obvious banding or opacity discontinuity. The crossing study includes grazing and
clearing views; it must not be described as an opaque-interior benchmark solely
because its study number is four. Capture and measurement clocks start at
different points around initialization, so inspect actual images and camera poses.

The last hard green shoreline edge was a render-order defect. The renderer asset
copied scene depth after transparents, but water, steam and spray read it during
those passes. A diagnostic showed saturated intersection distance right up to
the bank. Changing the alpha fade alone did not help and was reverted. Setting
`CopyDepthMode.AfterOpaques` in both the authored renderer and its build preparation
restored shallow contact blending. The setting has a regression test; close and
high views were rechecked. See the matching installed URP 17.3 renderer source and
[Unity's depth-copy enum documentation](https://docs.unity.cn/Packages/com.unity.render-pipelines.universal%4017.0/api/UnityEngine.Rendering.Universal.CopyDepthMode.html).

Sandbox flight now requests one asynchronous GPU height reduction over a nearby
17×17 lattice of actual displaced water vertices. The cached maximum plus camera
and latency margins replaces the earlier permanent worst-case wave floor. Samples
must be younger than 150 ms, within the protected central region and from the
current coast revision. Missing, failed or stale results use the conservative
bound; resetting effects or terrain invalidates old callbacks. Frame updates
never wait for a readback. Teardown drains only this probe's outstanding request
before idempotently releasing its buffer: relying on a later callback leaked at
application quit, when Unity stopped pumping callbacks. This teardown-only wait
has no API timeout; it is not a global wait for other GPU clients. The broad-wave
time margin is empirical, so this is visual
clearance for the sandbox, not guaranteed Rust physics collision. Real close-bank
captures now reach roughly 1.6 m instead of the previous 5.5 m safety floor, with
no water-plane clipping in those sampled views.

The low camera exposed a separate rectangular reflection cutoff. A temporary
UV/validity shader isolated it to sampling outside the planar texture: flattening
displaced wave vertices to their rest-grid positions caused the projection to
diverge near eye height. The final shader projects the mirrored displaced
surface through the retained reflection VP, then applies normal distortion.
Three adjacent low-bank frames show the rectangular patch gone. The diagnostic
build is preserved separately and is not the installed player.

Verification: 84/84 Modern EditMode cases passed in 26.6319 seconds
(`tests13/tests.xml`), including actual GPU density transport, independent clouds,
world wrapping, shared optical clocks, height readback and immediate teardown.
Build `build-p87tf4pa` retains native `a98a8317e89a`; all 33 changed/new Unity input
hashes match the qualified project. Its real `run-sandbox.sh` input check passed:
lighting modes, torches, cold/hot impacts, free flight, clear, panel and F10 exit.
The panel height was increased after a real capture exposed a clipped footer;
the final captured panel is complete. The final normal exits have no earlier
ComputeBuffer GC warning. Private-display audio is not qualified.

The 110-second `environment-finish-07` tour contains 3300 recorded frames.
Eighteen full-resolution daytime, night, cloud, flame, adjacent shore, high
terrain, two-plume and torch-adjacent steam frames were examined individually.
The earlier square shelves, hard green shore contact, floating rocks, dark
steam wedge and central rectangular reflection cutoff are absent in the reviewed
final views. Close steam studies additionally check soft lobes and flame
extinction through their gaps. The external X11 movie has a horizontal capture
tear at 95 seconds; it is retained as a diagnostic recording, not clean final
video evidence. A separate 28.56-second sequence uses 360 Unity complete-frame
screenshots (plus one encoded endpoint repeat), with file-completion intervals
retained. Its roughly 12–13 captures/second are not engine FPS. First, middle and
last images of each of its shore, steam and crossing studies were also examined
individually. See the frame review for the
observations and capture provenance; no claim is made about every possible view.

Final performance, RTX 3090 / Vulkan / 1600×900 / private KWin Wayland,
120 frames per run, with the campaign paused and no encoding during measurement:

| View / run | Mean ms | Median ms | p95 ms |
| --- | ---: | ---: | ---: |
| Quiet shore 1 | 8.45 | 8.36 | 8.71 |
| Quiet shore 2 | 8.63 | 8.35 | 11.40 |
| Four separated clouds 1 | 8.55 | 8.36 | 9.38 |
| Four separated clouds 2 | 8.54 | 8.35 | 10.01 |
| Cloud-crossing path 1 | 13.25 | 14.06 | 16.86 |
| Cloud-crossing path 2 | 12.84 | 12.62 | 17.08 |

Quiet/four-field windows start after 10 seconds; crossing windows after 28.
The earlier 80-step shader measured 18.30 / 15.55 ms mean on the crossing
study, with p95 21.69 / 20.15. Subsequent camera clearance and optical shaping
also changed, so this is historical context rather than an isolated shader
speedup claim. These are small repeated path samples, not a
sustained gameplay or locked-desktop performance claim. Present pacing is about
120 Hz. Five-second-warmup four-cloud runs included initialization and are
retained separately; they are not steady-state comparisons. Video/readback
captures are likewise excluded from performance qualification.

Evidence lives under `../generated/environment-finish-20261106/` and
`/mnt/pinokio-shared/magiccarpet-build-proofs-20261106/environment-finish-07/`.
The complete-frame clip and final gallery are in the sibling
`environment-final-frames/` directory. `performance24.json` and
`candidate24-source-receipt.json` identify the measured build and matching inputs.
The actual desktop remains locked. Windows, VR, ordinary gameplay performance
and replacement 3D game assets remain separate qualifications.

The changed-source UBS run covers 16 C# files and is partial: terrain/helper
analysis hits dataflow limits, and vegetation requires a conditional source
view. There are no critical findings. Its two nullable dictionary warnings are
in unchanged test helper lines whose values are created non-null; they were
reviewed rather than suppressed. The later five-file addition scan has 11
warnings, all unchanged builder lines with the same findings in the baseline
scan. The final four-file steam/probe scan is complete with zero warnings or
critical findings. `ubs doctor --fix` reports a healthy scanner installation,
not complete semantic coverage. Unity compilation, focused tests,
changed-hunk review and real player captures provide the other checks.

Build and install the combined player with `./build-player.sh`, then launch
`./run-modern.sh`. `./run-campaign.sh` retains the original presentation by
default. Both launchers accept the same campaign package and save options.
The install command preserves the previous player and checks its input receipt.
When original and modern delivery are being developed concurrently, the ignored
`Builds/Modern` symlink selects a separate qualified Unity project. Its own
`run-player.sh`, native plugin, source receipt and install lock are used.
The canonical campaign launcher still supplies owner resources from the real
generated directories; native validation deliberately rejects symlink cache roots.
`MC_PLAYER_PROJECT` carries the selected project to the existing launcher.
The optional `Builds/Original` pointer provides the corresponding qualified
default for `run-campaign.sh`; explicit modern selection takes precedence.
Both selections keep source and artifact checks intact, including rejection
of a broken pointer, and use canonical real owner-resource directories.
Rebuild that project with its `build-player.sh`; do not attribute a reused native
artifact to a different, concurrently edited Rust source tree. A broken pointer
fails explicitly. With no pointer, `run-modern.sh` uses this project's player.

The environment includes:

- Subdivided terrain with original source vertices, bounded surface relief,
  triplanar albedo/normal/roughness maps, and live terrain mutation updates.
- Deterministic, terrain-grounded scanned grass, ferns, shrubs, trees and
  boulders, with wind, instancing and finite draw distances.
- Moonlight, cascaded shadows, a world-space volumetric cloud layer with
  extinction and moonward self-shadowing, distance fog and tone mapping.
- Spectrum-driven wind waves, bounded local ripples and persistent foam,
  generated visual coast/depth fields, splash/condensation particles and a
  separate camera reflecting the actual scene, including instanced vegetation.

The nature models and surface textures are CC0 Poly Haven assets. Their source
URLs, hashes and derivations are recorded in
`Assets/Resources/ModernNature/PROVENANCE.json` and
`Assets/Resources/ModernTextures/sources.json`. Original owner game data remains
local and is not included in these assets.

## Visual checks

For a reproducible review of the live shoreline:

```sh
./run-modern.sh --new-game --save /tmp/mc-modern-review-save.json -- \
  -mc-modern-review-camera 125,8,842,75,2 \
  -mc-modern-hide-hud -mc-modern-capture /tmp/mc-modern-shore.png \
  -mc-modern-capture-delay 12 -mc-modern-exit-after-capture
```

Enter NightSpire through the normal campaign frontend. The review camera changes
only presentation; it does not move the Rust player and is not evidence of flight
or collision behavior. Omit it for normal play. `-mc-modern-metrics` logs bounded
frame and campaign timings. `-mc-modern-visual-debug` enables capture and lighting
diagnostics; F9 captures to the configured capture directory.

## Measured checkpoint

The guarded-advance optimization is committed as `20f57538`. Native artifact
`e547bac85fa3` was built from `41b44596` plus exactly that commit's five Rust
files. The isolated build passed six focused session, rejection, preference,
pause, replay and audio checks and the workspace all-target check. All 738
recorded source and compile-time fixture hashes matched the remote build.

Two runs before and after in the same Unity build22 shoreline view at
1600×900/Vulkan on the RTX 3090 gave these medians:

| Measurement | Before | After |
| --- | --- | --- |
| Campaign step and snapshot | 17.50 / 17.33 ms | 14.54 / 14.51 ms |
| Complete frame | 49.76 / 50.92 ms | 49.61 / 49.37 ms |

This demonstrates a bounded CPU saving; the scene is still about 20 fps.
These are short 120-frame samples after warmup, not a performance guarantee.
Seventeen focused Unity checks pass with this native: twelve terrain/JSON checks,
then five actual production pause, close/restart, save/replay and stale-transition
checks. The latter require the current package override
`MC_CAMPAIGN_LEVEL1_PACKAGE=../generated/mc-bhq8-level1-collision-v5-night`
(use an absolute path when invoking Unity from another directory); the four
older checks' default v3 fixture correctly fails the current collision-bank
preflight. The new paused-menu reload check uses the launcher's actual package
and cache directories directly.
Seventeen launcher regression checks pass, including separate modern/original
delivery, explicit selection precedence and rejection of broken installation
pointers. This is automated verification;
human flight/combat and hardware audio remain unverified.

Local evidence, native artifact and input receipts are under
`../generated/nightspire-modern-20261103/native-e547/`.

Build26's actual moon, shoreline and close-vegetation captures use the same
`e547bac85fa3` native. The cloud pass replaces the rejected bright outlines with
a finite 20-sample volume. The shoreline and close-vegetation views show actual
scanned geometry, grounded boulders and scene reflections. The moon and close
views measured median frame times of 46.66 and 47.36 ms respectively at
1600×900/Vulkan. These different views do not establish a performance comparison
against the earlier shader, and CPU scope timings do not measure GPU cost.

The ordinary player also flew from `(310,80.5,890)` to `(310,70.16,650)` using
normal input, stopped, wrote an F5 save at tick360 and displayed the original
pause menu over the modern environment. This is a bounded automated input
check, not a human playthrough.

That run exposed a reload defect: temporary preference-installation pause moved
the native generation while the borrowed music reader retained the previous
generation. The paused-menu regression reproduced that exact rejection before
the fix. It now verifies current music/session identity, unchanged saved pose,
continued play, destruction of the previous presentation root, and rejection
of a malformed save while retaining the playable current owner. Reload also
recomposes the previous presentation if replacement attachment fails; successful
recomposition after that particular failure is source-reviewed, not dynamically
verified. Its failed-recovery path stops input explicitly.

The exact added check is
`ProductionPausedMenuReloadRebindsMusicBeforeInstallingPreferences` with Unity's
`-runTests -testPlatform EditMode -testFilter` options. Its final XML/log are
`/tmp/magiccarpet-modern-reload-final.{xml,log}`; the preceding five-check pass is
`/tmp/magiccarpet-modern-reload-after.{xml,log}`. Authored-source diff and shell
syntax checks and the 17 launcher tests pass. The full staged whitespace check
reports Unity's generated empty-value trailing spaces in metadata/settings;
those generated files are retained as authored by Unity. UBS's C# scan remains partial because its
conditional-compilation and accessor analysis cannot cover these files; reported
legacy warnings and temporary-fixture GUID findings are outside the changed
hunks. The launcher scan reports no shell warnings and six Python warnings
about uncaught JSON parsing in test assertions, where malformed output should
fail the test. This is not a clean whole-file scanner claim.

Build27 is installed in the isolated `magiccarpet-modern-native-41b44596` Unity
project and selected by `Builds/Modern`. All 317 recorded Unity inputs match
the canonical source. Its actual player passed normal frontend entry, flight,
F5 saving, paused-menu reload, resumed flight, music off/on, and exit. The same
combined player also loaded NightSpire with the original rendering mode.
The canonical `run-modern.sh` route was also exercised through the real frontend.
The canonical default `run-campaign.sh` independently loaded the original
NightSpire/HUD through `Builds/Original` with native `955cf9b7df8e` (Duel).
The modern preview remains pinned to native `e547bac85fa3`; newer shared-gameplay
batches remain in the separately qualified original client. Neither original
player artifacts nor its native plugin were replaced by this installation.
Local screenshots, test results and the delivery evidence are in
`../generated/nightspire-modern-20261103/checkpoint27/`.

## CPU performance pass, 2026-11-04

The installed modern player caches its immutable command-line options instead
of retrieving and allocating the argument array during every actor draw. The
terrain, nature assets, shaders, lighting and reflection settings are unchanged.
The same 1600×900/Vulkan shoreline camera on the RTX 3090/i7-10700 measured:

| Measurement | Before | Cached options |
| --- | --- | --- |
| Complete frame median | 47.22 / 47.72 ms | 40.20 / 40.25 ms |
| Modern scene preparation median | 5.15 / 5.13 ms | 0.86 / 0.85 ms |

The final rebuilt player measured 40.29 ms median (about 25 fps), with scene
preparation at 0.84 ms. These are 120-frame samples after five seconds of warmup;
the final run's p95 was 57.73 ms, so this is not a smooth-frame-time or 60-fps
claim. Captures were inspected at the same camera and retain the same visual
content. One competing-build run was excluded from the comparison.

`-mc-modern-metrics` now distinguishes native observation calls, managed JSON
decoding and Unity main/render-thread timing. Diagnostic scope names allocate
only while measurement is active. Unity reported main-thread medians around
34–40 ms and render-thread medians around 0.45 ms; GPU frame timing was unavailable
in this Linux/Vulkan run. Sampled GPU utilization was 35–42%. A separate flat CPU
profile identified SHA-256 and terrain hashing as substantial native costs.
Those samples cover all process threads, and the scope samples are not an
additive breakdown of an individual frame.

A second optimization computes the terrain digest once per campaign observation
and passes it through the existing component hashes. It retains the outer
SourceArena effects and dynamic-lighting hash. There is no cache across ticks or
terrain mutations. Installed native `7fe307bd304e` uses the qualified `e547` source
baseline plus these six Rust files. All 738 source/fixture inputs were recorded.

| Native measurement | Cached options, old native | Reused terrain digest |
| --- | --- | --- |
| Campaign snapshot median | 4.09 / 4.05 ms | 2.84 / 2.93 ms |
| Step and snapshot median | 15.29 / 15.26 ms | 13.84 / 13.40 ms |
| Complete frame median | 40.20 / 40.25 ms | 40.39 / 41.03 ms |

This second change saves native CPU work but does not demonstrate another
whole-frame throughput gain. The installed scene remains about 24–25 fps.
The corrected native matches all 115,680 rows of a 240-step/reset trace from
`e547`, including state/terrain/dispatch hashes, positions and events. Its four
focused native checks pass, as do the current canonical workspace all-target
check and both integrated hash/polling checks. The integrated tests use the same
qualified Level 2 fixture explicitly, because the ordinary remote source sync
excludes generated owner packages.

The comparison first rejected an incomplete candidate which bypassed the outer
SourceArena hash through Deref. The corrected implementation retains that owner,
and a new regression checks that changing only its lighting state changes the
snapshot fingerprint. The rejected source/binary and a byte-identical rebuild
of the old native are preserved with the evidence. Restoring experimental source
also updates modification times so Cargo does not reuse a newer candidate binary.

The final Unity build and twelve existing JSON-boundary/terrain checks pass.
UBS's C# scan is partial; its seven possible-null warnings concern the private
timing dictionary, whose sole insertion always supplies a new non-null list.
Changed source was also reviewed directly. This checkpoint was self-verified.
The Rust scanner's changed-line findings are test assertions/unwraps; its broad
legacy findings are not candidate findings. The package-wide format check
remains non-clean and was not used to justify unrelated reformatting.
Logs, captures, profiler data and exact command receipts are under
`../generated/nightspire-performance-20261104/`.

## Rendering architecture pass, 2026-11-04

Implemented in the qualified Modern player, retaining native `7fe307bd304e`:

1. Terrain retains detailed and distant meshes. Distant homogeneous land and
   water use original source-cell geometry; section borders and a shoreline
   ring remain detailed, with stitched transitions. Both meshes update after
   deformation. LOD uses distance to the chunk's 3D bounds with hysteresis, so
   flight altitude reduces detail too. Water visibility uses water-only bounds.
2. Vegetation uses retained 32 m spatial batches instead of rebuilding its whole
   surrounding area after each 4 m movement. Overlapping batches survive camera
   movement; terrain revisions re-ground them. Nearby scanned meshes remain;
   distant meshes simplify indices while retaining their UVs and attributes.
   Tree crowns retain whole leaf cards, orientation coverage and outer extrema.
   Trees switch at 104/88 m, smaller vegetation at 63/47 m. Shadows can still be
   cast by offscreen landmarks. Reflection LOD has separate hysteresis and uses
   its actual render-target scale, with the same visibility radii as the main view.
3. Water reflections skip invisible water, retain their texture and matching
   projection for up to 1/30 s during negligible camera motion, and refresh on
   camera cuts, projection changes or terrain mutations. The reflection camera
   omits its separate shadow pass and unused color copy. Its depth texture stays:
   reflected smoke consumes it for soft intersections. Shorter reflection ranges
   and removing that depth texture were rejected during independent source review.
4. Clouds sample a 128-cubed R16 lattice (4 MiB) prepared once per environment,
   instead of hashing eight corners for every noise sample in the volume march.
   Quintic interpolation, animated world-space density and moonward self-shadowing
   remain. The lattice repeats outside its stored domain. The diagnostic switch
   `-mc-modern-procedural-clouds` retains the previous procedural calculation.
5. The unused main-camera opaque-color copy is disabled. Texture import settings
   were checked: nature maps retain 1K compressed mipmaps, ground maps retain their
   2K maximum and mipmaps. Main-view moon shadows, texture resolutions and artwork
   were not reduced.

The actual imported distant meshes have 477/1,257 grass triangles, 2,323/5,242
shrub triangles, 3,056/5,000 rock triangles, 2,532/4,500 branch triangles, and
4,448/10,172 leaf triangles. These are mesh counts, not total frame GPU work.
At the same shoreline camera the terrain's active range budget fell from
851,968 triangles (104 fully detailed copies) to 447,766, about 47% lower.
This range includes offscreen chunks; it is not a rasterized-triangle measurement.

Two fresh 1600x900 Vulkan runs on the RTX 3090/i7-10700 measured:

| Measurement | Before | Final renderer |
| --- | --- | --- |
| Frame median | 40.26 / 40.36 ms | 39.61 / 39.48 ms |
| Frame p95 | 43.07 / 52.97 ms | 49.52 / 47.21 ms |

Each run samples 120 frames after five seconds of warmup. This is a small median
improvement, without a consistent p95 gain; overall throughput remains about
25 fps. Main-thread and campaign work remain substantial. No 60-fps claim or
GPU-timing attribution follows from these captures. Rust gameplay, collision,
tick scheduling and the simulation/presentation data contract are unchanged.

All nine focused Unity EditMode checks pass on the final source, covering
terrain sampling, mixed water/land topology, shoreline edges, wrapped bounds,
mutation, altitude/hysteresis, actual imported mesh indices/cards/UVs, chunk
retention/re-grounding, cloud lattice quantization and reflection configuration.
An independent bounded source review found the two reflection issues corrected
above; visual checking remains necessary for crown density and LOD transitions.
UBS scans are partial (C# semantic work limits and unavailable standalone dotnet);
reported possible-null findings in the pipeline creator and timing dictionary
are guarded by construction and are not newly introduced defects. Unity itself
compiled the actual assemblies and player. No scanner suppression was added.

The final build is `Builds/build-ns465lsd` in the qualified project. All 323 Unity
inputs and the exact native hash passed installation checks. A preceding Burst/
Mono build crash and a later input-stability rejection are retained in their
build logs; the unchanged final retry succeeded with no differences between
independent before/after input manifests. Their causes were not established.
The canonical launcher was exercised through normal frontend entry. Root viewed
the final shoreline, close vegetation, sky and ordinary-flight captures. Normal
Up-arrow input moved the actual camera from `(310,80.5,890)` to
`(310,91.61,778.75)`; Backspace stopped movement and F5 wrote a runtime save.
This is a bounded automated flight/save check, not a human playthrough or proof
of every LOD transition. Final runs had no logged runtime exceptions or shader
errors. Both Modern and Original installation validators pass; Original remains
on its separately qualified native `ab1095b7514a`.
Commands, manifests, XML, logs and actual captures are retained under
`../generated/nightspire-rendering-20261104/`.

## Native CPU, shader and display-path diagnosis, 2026-11-05

This pass is installed but does **not** establish smooth foreground play.
The actual 3090 Wayland session remained locked. No input was sent to that
locked desktop and no unlock was attempted. Native CPU improvements are measurable; overall frame throughput
still needs work and an unlocked-desktop check.

Source commits: `956e756f` (native CPU), `23792e0e` (terrain sampling/diagnostics),
`ae06965c` (identity decoder), `0b4cabc2` (release timing queries), and
`ed1f1549` (Wayland launcher). These commits were made locally; this work did
not push the shared branch. The separate Original installation validator also
passes, retaining its own native `ecbf096983e4`.

Delivered changes:

- Seven source-slot lookups use the parser's existing slot/index invariant,
  retaining their exact predicates and source priority. The presentation adapter
  shares the campaign's existing immutable inspection `Arc` instead of making
  a deep copy. Rust commit: `956e756f`.
- Pure soil terrain pixels skip 18 unused rock texture reads; pure rock pixels
  skip nine unused soil reads. Mixed pixels retain all 27. Texture gradients
  are calculated before branching, including the rotated rock UVs, to preserve
  mip selection. `-mc-modern-full-terrain-material` runs the full sampling path.
- Canonical runtime identities use a faster field-based JSON decoder. Required
  fields, primitive shapes, case aliases, duplicates, UTF8 and legacy XML
  noncharacters are checked before admission. Other syntax falls back to the
  original DataContract decoder; other receipt types always retain it. Objects
  remain fresh. The official Unity Newtonsoft package is pinned at 3.2.2.
  `-mc-runtime-legacy-json` selects the old decoder within the same player.
- The NVIDIA native-Wayland launcher sets `__NV_DISABLE_EXPLICIT_SYNC=1` for
  this process when the operator has not supplied a value. This avoids the
  reproduced fatal "surface already exists" protocol error. X11/default routing
  and explicit overrides remain available; no driver or desktop configuration
  was changed.
- Always-on Frame Timing Statistics are disabled in release player settings.
  A control trace confirmed that GPU query-result polling disappeared. This
  does not change artwork or rendering settings. Normal play retains VSync;
  `-mc-modern-no-vsync` is only an explicit diagnostic option. Metrics log the
  actual device/settings and accept a bounded `-mc-modern-warmup` interval.

Native measurements use optimized release baseline and candidate artifacts,
the same input sequence and hardware, with two sequential A/B pairs:

| Measurement | Baseline | Candidate |
| --- | --- | --- |
| Step median, pair 1 | 12.891 ms | 8.776 ms |
| Step median, pair 2 | 12.890 ms | 8.746 ms |
| Snapshot median, pair 2 | 2.943 ms | 2.967 ms |

The step reduction is about 32%; snapshot time did not improve. Every turn's
state, terrain and journal digests match for 240 turns, and the complete final
snapshot matches. Twelve focused Rust tests pass: scheduler/order/RNG controls,
holder expiry, slot replacement/reclaim, and actual runtime inspection pointer
sharing with snapshot equality. Native SHA256:
`2fdafc823e977fa7bc2791aadc0e1c6b9b8a0b3b8aca65419aae596dbe5bde86`.
Its frozen source is `793b7ef62973d4153ab1372ffee3afc559585813` plus the three
performance-owned Rust files, with all 444 inputs recorded. It intentionally
excludes the other team's concurrent, unfinished gameplay edits. An earlier
installed development artifact was unoptimized and is excluded from speedup
comparisons.

The endpoint shader's same-build screening changed frame median from 39.81 to
38.55 ms at 1600x900. Root inspected the full captures and terrain crops without
finding an obvious detail loss. Animated vegetation/shadows prevent claiming
bit-identical full images. A separate 64 MiB terrain noise cache gave no useful
frame-time gain and was removed; its candidate source/evidence is preserved.
Texture resolution, vegetation density, view distances, main shadows, MSAA and
lighting quality were not lowered in this pass.

Six final Unity EditMode decoder cases pass, including four runtime states,
nulls, fresh/concurrent reads, Unicode and integer boundaries, and negative
admission controls. A standalone bundled-Mono differential sweep matches the
old decoder across 38 cases. The compatibility-gated microbenchmark measured
about 0.269 to 0.129 ms/read; the actual player showed a smaller gain:

| Same-build ABBA run | Legacy 1 | Fast 1 | Fast 2 | Legacy 2 |
| --- | --- | --- | --- | --- |
| Identity decode median | 0.33 ms | 0.25 ms | 0.25 ms | 0.36 ms |
| Frame median | 39.36 ms | 38.86 ms | 38.93 ms | 38.78 ms |
| Frame p95 | 47.54 ms | 45.36 ms | 47.17 ms | 46.20 ms |

There is no consistent whole-frame improvement from the decoder. Each run uses
1600x900 Vulkan, the same replay/camera/native, 20 seconds of warmup and 120
samples per scope. Scope windows are independent and must not be added together.

An eight-second Nsight Systems trace contains 194 matched presentation calls:
GPU drawing median 13.43 ms, `vkQueuePresentKHR` median 36.16 ms, and median
22.09 ms between GPU completion and that API returning. The build has no debug
pass markers, so this is not a per-URP-pass attribution. Disabling the in-game
metrics did not remove that wait. Disabling global frame statistics removed
query polling but left approximately 22 ms of post-GPU waiting; its small
single-run frame change is not a strong speedup claim.

The final canonical launcher ran successfully on the actual locked Wayland
desktop without a caller-supplied explicit-sync override, captured the real
player at 1920x1080 and exited normally: median 50.46 ms, p95 60.18 ms. No fatal
runtime/shader/protocol error was logged. A private KWin virtual display, using
its own socket, configuration and D-Bus session, measured 27.50 ms median and
38.56 ms p95 at 1600x900. This demonstrates sensitivity to the display path;
it is one isolation experiment, not an unlocked-desktop or 60-fps qualification.

Installed player: `Builds/build-ezjziev5` in
`/home/oliver/Projects/odcpw/magiccarpet-modern-performance-20261105/research/private/remc2-unity-native`.
The canonical `Builds/Modern` selects it; the previous selector and players
are retained. All 12 changed Unity/launcher inputs match canonical source and
the install validator passes. Build output now resides on the larger disk under
`/mnt/pinokio-shared/magiccarpet-build-proofs-20261105/current-modern-builds`,
with the original directory retained alongside the qualified project. Older
generated Modern rollback players were preserved on that disk with verified
file hashes. Profiler downloads also have a verified archive there; their local
copies remain because cleanup/relocation was rejected by the deletion guard.

The final player also passed the normal frontend-to-flight/save route on a
private X display: Up-arrow input moved its actual camera from
`(310,80.50,890)` to `(310,89.13,783.75)`, Backspace stopped movement and F5
wrote a runtime replay save. It captured the resulting view and exited zero,
with no fatal runtime or shader error. Root inspected that capture. This is a
bounded input/save smoke, not human playthrough or flight-pacing qualification.
Evidence: `../generated/nightspire-rendering-20261104/performance_final_flight_20261105*`.

UBS coverage remains partial: missing standalone dotnet and bounded C# semantic
analysis. Reported dictionary values are non-null by their private construction;
decoder catches deliberately fall back to the old decoder, and the six critical
credential-pattern hits are unchanged Guid test fixtures. No suppression or
relaxed scanner gate was added. Unity compiled the actual assemblies/player.

Primary API references: [Unity JSON package](https://docs.unity3d.com/Packages/com.unity.nuget.newtonsoft-json@3.2/manual/index.html),
[DataContract serializer](https://learn.microsoft.com/en-us/dotnet/api/system.runtime.serialization.json.datacontractjsonserializer),
and [Unity VSync behavior](https://docs.unity.com/en-us/engine/6000.0/script-reference/unityengine/qualitysettings/vsynccount).
Reproduction commands, source/artifact manifests, per-turn comparisons, test XML,
profiling data and captures are retained under
`../generated/nightspire-performance-20261105/` and
`../generated/nightspire-diagnosis-20261104/`. Remaining acceptance is open:
smooth foreground performance, p95 pacing during ordinary flight and visual
transition checks on the actual desktop. No tick-keyed gameplay/hash cache or
asynchronous simulation redesign was introduced without mutation/race proof.

## Terrain and vegetation representation rework

Terrain now packs unmodified source albedo RGB and linear roughness R into
RGBA BC7 textures; normal maps and both rock texture scales remain. Continuous
sparse triplanar weights skip axes with zero contribution. Logical reads per
active axis are two for soil and four for the two-scale rock material, versus
three and six previously. These are sampling budgets, not measured GPU speedups.
Geometry, source heights, wrapping and deformation behavior are unchanged.
Matched wall/shore captures showed no obvious new material seam or loss of
surface detail. Exact offline channel checks reject separate one-byte color and
roughness corruptions; actual Vulkan sampling at three mip sizes remains within
the original 0.015 mean linear-channel compression budget. The exact check uses
pinned Pillow 13.0.0 because unchanged native Unity pixel conversions were
intermittently inconsistent; no textures were regenerated to accept that failure.

Vegetation uses three representations selected by projected bounding size:
full scanned geometry, prebuilt middle meshes, then camera-facing multiview
impostors. Complementary dither bands are 40–56px and 12–20px. Spatial groups
entirely within an endpoint avoid per-instance classification; transition
partitions are cached separately for the main and reflection cameras. Density,
placement seeds, terrain anchors and 115/200-metre residency ranges remain.
Offscreen landmark proxies still cast shadows. Counters distinguish submitted
detail/middle/distant instances and shadow-only triangles; transition draws count
both meshes and are not a count of unique visible plants.

Seven middle Mesh assets preserve the qualifying pilot's exact normalized
vertices, normals, tangents, colors, UVs, topology, indices and bounds. Their
reducers compile only in the Editor. The player loads and clones the assets.
The one-time batch authoring entry is
`MagicCarpet.Native.Editor.ModernVegetationMeshLods.Generate`; it refuses existing
assets or metadata. Preserve prior outputs before deliberate regeneration.
`Prepare()` validates the resources rather than silently generating a fallback.

Four foliage species have 40 full-sphere captured views each (8 azimuths ×
5 elevations), stored in 1024×640 guarded atlases. Four neighboring views blend
continuously on one quad. Unlit sRGB color/coverage is separate from linear
object normals and view depth; the source normal maps participate in the bake.
Depth reconstruction is shared by color/depth passes and light-oriented shadow
views. Source opacity is clipped before capture, and transparent-border dilation
preserves opacity. Bake bounds are checked against actual Unity-imported vertices,
including rejection of displaced/scaled capture metadata. The CC0 source and
derived output hashes and authoring commands are in
`Assets/Resources/ModernNature/LODS_PROVENANCE.json`; CPU Blender/Pillow tools
live under `Tools/`, outside player Resources.

Early two/three-plane and early-takeover multiview candidates reduced geometry
but produced dark plant clumps and were rejected. Moving the middle meshes back
into the chain restored open stems/fronds in matched actual-player views.
The first three-tier screening submits 3,553,604 vegetation triangles versus
6,544,292 in the pinned baseline, but 475 versus 297 batches. Its single private
Wayland run measured 26.42ms mean / 28.23ms median / 38.31ms p95 frames, versus
25.20/27.54/37.51ms baseline: **this is not a frame-rate improvement**.
Campaign CPU processing remained approximately 20ms per fixed tick; terrain
observation and actor presentation each measured below 1ms. Independent timing
scope windows must not be added together. Ordinary foreground smoothness remains
unqualified while the desktop is locked.

For same-build controls use `-mc-modern-full-vegetation`, or
`-mc-modern-unpacked-terrain -mc-modern-full-terrain-projection`.
`-mc-modern-review-orbit center-x,y,z,radius,height,degrees-per-second` plus
`-mc-modern-capture-count` and `-mc-modern-capture-interval` provide bounded
presentation-only angle captures. They bypass gameplay camera collision; an
orbit entering a cliff is not a terrain-hole or flight-collision test. Readback
capture sequences are not performance benchmarks. The safe shoreline orbit
used `145,2,862,20,25,90` with 24 captures at 0.2-second intervals.

Research references:
[Crytek's vegetation rendering](https://developer.nvidia.com/gpugems/gpugems3/part-iii-rendering/chapter-16-vegetation-procedural-animation-and-shading-crysis),
[Epic's multiview impostors](https://dev.epicgames.com/documentation/en-us/unreal-engine/impostor-baker-plugin-in-unreal-engine),
and [Unity's grouped instancing culling](https://docs.unity3d.com/6000.0/Documentation/ScriptReference/Graphics.DrawMeshInstanced.html).

Integrated self-verification: all 32 selected Modern EditMode tests pass,
including exact middle-mesh parity, imported bake bounds, terrain wrapping and
deformation, packed texture imports and corrupted-channel controls. Build7
(`build-_zq1ip_d`) is installed and its validator passes; its native at this
rendering checkpoint was
`2fdafc823e97`. All 64 renderer source/asset/tool inputs match the qualified
project. Build6's identical runtime geometry/code passed the ordinary frontend,
Up-arrow flight, Backspace stop and F5 save on a private X display: actual pose
`(310,80.50,890)` to `(310,91.00,780)`, normal exit and no fatal/shader error.
Build7 adds only the recorded capture-history caveat to provenance. Root viewed
shore, close, safe-orbit samples and flight captures; the source/orbit reviews
do not establish continuous motion quality on the locked desktop.
Evidence: `../generated/nightspire-rework-20261105/` and
`../generated/nightspire-rendering-20261104/rework_offline6_flight*`.

Changed-source UBS found zero critical issues but returned partial C# coverage
at its dataflow limit. Dictionary values are private non-null constructions;
diagnostic argument parsing intentionally fails malformed input; test task
results are read only after bounded successful completion. The PNG packer's
single-frame Pillow conversions fully load their images. No suppression or
weakened gate was used. Unity compiled and ran the actual assemblies/player.

## Clock-state qualification of isolated measurements

The 2026-11-05 rework added one-second NVIDIA clock/power telemetry to the
isolated Vulkan traces. The eight-second `gpu_terrain3` capture starts at
17:28:12.374 local time. Its initial samples use 705 MHz SM / 810 MHz memory
(P5); subsequent samples use 420→330 MHz SM / 405 MHz memory (P8).
The 1695/9751 MHz P0 samples belong to startup before the captured interval.
The unchanged scene's GPU workload duration rises from approximately 6.46 to
13.31 ms as the clocks fall. Thus the earlier aggregate GPU-duration comparison
does **not** establish a shader speedup: clock state explains a large change
inside one unchanged run. Private Wayland runs also downclock, although their
clock behavior and presentation waits differ from Xvfb. No GPU power, clock,
driver, compositor or desktop setting was changed for these measurements.

Use matched camera/save/native, repeated runs, actual device/clock records,
and foreground desktop pacing before making a complete-frame performance claim.
Triangle submissions, logical texture-read budgets and CPU submission scopes
describe different costs; none alone establishes a frame-rate improvement.
The native campaign still takes substantial CPU time independently of terrain
and vegetation. Evidence and timestamped telemetry:
`../generated/nightspire-rework-20261105/gpu_terrain3*`,
`gpu_terrain_control3*`, and `clocks_wayland3*`.

NVIDIA documents a swapchain-scoped boost hint through
[`VK_NV_low_latency2`](https://docs.nvidia.com/datacenter/tesla/driver-installation-guide/latest/gaming.html#reflex-native),
but the inspected official Unity Reflex package contains a Windows DLL and no
Linux native plugin. It is not a ready Linux/URP integration for this project.
No undocumented environment power toggle, global maximum-performance setting,
or custom Vulkan interception layer was added to this renderer task.

## Retained event-history CPU fix

A user-only Linux LBR profile of the real release player found approximately
8.2% of sampled process cycles in `EffectJournal::append` shifting the retained
16,384-record array. After filling, every new event drained its first element
and moved the whole remaining array. The journal now exposes the same contiguous
logical slice over a private active window. Physical compaction is batched;
expired payloads are released immediately. A full spare window is reserved on
first expiry, including awkward partial-length clones, so tiny repeated
compactions cannot return. Clones copy only live records; equality, debug,
paging and receipts exclude retired storage. Requested capacity stays bounded
by two retention windows, as the focused capacity/compaction controls verify.

Five focused journal tests pass, including actual source event ordering and
failed-command atomicity, plus an independent eager-queue comparison across
multiple retention/compaction boundaries with boxed events and prepared audio.
The old assertion on private physical length now checks the public retained
slice, with separate per-append physical length/capacity and compaction bounds.
This preserves retention rather than relaxing its limit. The change was also
independently reviewed. No hash format, event order, cursor/expiry policy,
simulation arithmetic, scheduling, RNG or public ABI changed.

Two sequential release-native A/B pairs use identical inputs, 60 warmup turns
and 540 measured turns. All 600 per-turn state/terrain/journal digests and
event heads, and complete final snapshots, match exactly. Final event head
69,613 crosses three physical compactions.

| Native measurement | Baseline 1 / 2 | Candidate 1 / 2 |
| --- | --- | --- |
| Step median | 9.199 / 8.861 ms | 5.314 / 5.217 ms |
| Step p95 | 9.893 / 9.693 ms | 5.878 / 5.804 ms |
| Snapshot median | 3.047 / 2.975 ms | 2.997 / 2.988 ms |

Candidate compaction-turn steps were 5.16–5.38ms, without a new observed spike;
overall candidate step p99 was 13.00–13.14ms versus 19.35–21.05ms baseline.
Peak process RSS was 155,140–155,292KiB versus 155,512–155,576KiB baseline.
These bounded runs do not prove all future gameplay pacing or memory behavior.

The actual Unity A/B control contains 170 identical player files except the
native library. On private Wayland at 1600×900, with the same scene/save,
20-second warmup and 120 samples per scope:

| Complete frame | Baseline 1 / 2 | Candidate 1 / 2 |
| --- | --- | --- |
| Mean | 25.94 / 26.25 ms | 20.24 / 19.68 ms |
| Median | 28.46 / 28.14 ms | 23.99 / 23.50 ms |
| p95 | 31.30 / 37.99 ms | 32.41 / 32.69 ms |

Mean and median improve repeatedly; p95 is mixed. This is approximately
38→50fps from mean frame time in this isolated display path, not a 60fps or
unlocked-desktop qualification. Graphics/geometry and submission scopes remain
unchanged between these native controls. GPU clock telemetry is retained;
no graphics quality or machine power settings changed.

Native artifact SHA256:
`a98a8317e89a0cf5cebffb6e05091ecf78970394458b766647aa0234e29f3d09`.
It is built from the recorded 444-input `2fdafc823e97` source closure with only
`sim-core/src/campaign/journal.rs` changed, excluding concurrent gameplay work.
Native-only refresh `build-43318akm` is installed into the same build7 player
and its install validator passes. Ordinary frontend/flight/stop/F5 save also
passes with this native: `(310,80.50,890)`→`(310,92.88,776.25)`; normal exit,
no fatal/shader error. Root inspected both ordinary-flight captures, including
this new native consumer path.

UBS on the journal reports zero critical findings. Warnings are test assertions,
bounded conversions/indexing and deliberate cloned test controls; no suppression
was added. The source formatter and frozen workspace all-target check pass.
The latter initially found a missing existing `sim-input` compile-time recording
fixture in the execution copy; copying the unchanged canonical fixture with its
recorded checksum resolved it, without modifying a test or production input.
Reproduction commands, frozen input
hashes, native benchmark raw distributions, LBR data, whole-frame A/B commands,
clock records and resource measurements are under
`../generated/nightspire-rework-20261105/`. Flight evidence is
`../generated/nightspire-rendering-20261104/rework_journal7_flight*`.

## Spatial vegetation submission

The three-tier selector still culls 32-metre chunks and chooses the same
instances, meshes and complementary fades. A second stage now combines the
already-selected instances into reusable buffers of at most 256. Camera,
species, representation and shadow mode remain separate. Visible shadow casters
also retain 64-metre X/Z bins, so one large instanced bound does not unnecessarily
span the whole visible landscape. A fixed direct-mapped slot set flushes on
spatial collision; it cannot combine distinct world bins. Non-casting foliage
and cheap offscreen shadow-only proxies remain globally grouped.

The regression check compares complete matrix/fade streams per key and covers
negative coordinates, bin boundaries/collisions, capacity splitting, mixed
material defaults/overrides, separate cameras and reuse. All 33 selected Modern
EditMode tests pass on the spatial candidate. Preallocated managed buffers are
bounded; the spatial pilot adds about 11.2MiB of matrix/fade arrays over the
global-batch pilot, including currently unused reflected shadow-caster slots.
First-use native property-array storage is not claimed allocation-free.

The initial global-batch pilot reduced the fixed-view submissions from 475 to
33, but widened instance bounds. Its last-100 Nsight GPU busy median was 10.93ms
versus 10.11ms control; mixed P5/P8 clocks prevent attributing that difference
solely to shadow grouping. The spatial candidate instead submits 139 calls.
All selected triangle/instance counts remain identical to control:
3,553,604 vegetation triangles, 153,010 shadow-only triangles and
226/1,632/1,953 detail/middle/distant instances. Density, source geometry,
placement, wind, ranges and selected LOD fades were not reduced for batching.

Sequential C–B–B–C release-player runs at 1600×900 use the same native
`a98a8317e89a`, scene, save, camera, 20-second warmup and 120 samples. No profiler,
compilation or other owned player ran concurrently with these four runs.

| Complete frame / submission scope | Control 1 / 2 | Spatial candidate 1 / 2 |
| --- | --- | --- |
| Frame mean | 20.53 / 20.95 ms | 18.18 / 18.61 ms |
| Frame median | 23.86 / 24.04 ms | 23.49 / 23.39 ms |
| Frame p95 | 33.98 / 34.39 ms | 27.86 / 28.90 ms |
| `modern_render` mean | 1.18 / 1.17 ms | 0.99 / 1.02 ms |
| `modern_render` median | 1.39 / 1.37 ms | 0.72 / 0.75 ms |

Mean and p95 improve in both pairs; median whole-frame gains are modest.
Reflection cadence/reuse changes with frame pacing, so the submission scope
is not a pure isolated batching measurement. The result is approximately
54–55fps from mean frame time in private Wayland, with 28–29ms p95; it does not
establish smooth foreground 60fps. Root and an independent reviewer found no
new coverage, placement, silhouette, visible-shadow or reflection regression in
the matched shore stills. Motion/cascade transitions remain a separate check.
Evidence: `../generated/nightspire-rework-20261105/frame_spatial_*`.

The spatial candidate's last-100 Nsight GPU busy mean/median/p95 was
11.63/10.58/16.60ms versus control 11.32/10.11/16.33ms. The traced clock states
are recorded separately from startup clocks. These mixed, sampled P5/P8 states
do not establish a GPU speedup, and a small GPU regression is not excluded.
The spatial version is retained for its repeated unprofiled whole-frame/p95
improvement and bounded shadow grouping, rather than claiming that batching
makes every stage faster. No global GPU clock, power or driver setting changed.

The middle-mesh provenance retains the actual generation-time factory hash at
revision `12f45229`. Rendering-only batching edits did not regenerate those
assets; the exact current-factory geometry/attribute/index parity test passes.

Build9 (`build-8xxdkmdt`) was installed with native `a98a8317e89a`; both Modern
and separate Original install validators pass. The ordinary frontend, five-second
Up-arrow flight, Backspace stop and F5 save pass on the private X display:
`(310,80.50,890)`→`(310,90.50,755)`, normal exit and no fatal/shader error.
Root inspected the flight capture. This is a live input/save check, not a
performance benchmark. Its evidence is
`../generated/nightspire-rendering-20261104/rework_spatial9_flight*`.
Fresh changed-source UBS reports zero critical findings and two test-only
`TryGetValue` warnings: both dictionaries are private test constructions whose
only inserted values are non-null lists. C# conditional-compilation analysis
remains partial; Unity compiled and ran the actual tests/player. No scanner
suppression was added. Root self-verified this integration against the request.

The final source-closure check exposed a stale `__pycache__` in the isolated
project's Resources directory. Its bytecode and generated metadata are preserved
outside Assets, along with the canonical bytecode copy. Build10
(`build-pdk4fzxx`) packages the cleanup: all 375 recorded Unity inputs now match
canonical source, and no shared input or native hash changed from build9.
Only the seven obsolete bytecode/metadata inputs disappeared. The earlier
negative source-check result is retained separately from the successful check.
The 24-frame safe orbit was also recaptured with the spatial renderer; root
inspected angles 0, 6, 12 and 18 without finding a new gap or coverage defect.
These samples do not establish artifact-free continuous motion.
The final build10 ordinary frontend/flight/stop/F5 path also passes with normal
exit and a saved replay: `(310,80.50,890)`→`(310,91.00,780)`. Root inspected the
capture; evidence is
`../generated/nightspire-rendering-20261104/rework_spatial10_flight*`.

## Wind and water video prototype, 2026-11-05

The implementation follows `WATER_RESEARCH.md` as a focused URP prototype:

- `ModernWind` supplies continuous wind changes and integrated advection to
  vegetation, clouds, waves and particles. This is cosmetic weather; Rust's
  Wind spell, flight forces, collision, damage and RNG remain authoritative.
- `ModernWaterSpectrum` evaluates two 256² GPU FFT bands with 256/32-unit
  periods, stable Gaussian phases and a directional JONSWAP-shaped spectrum.
  Broad wavelengths are at least 16 units; the short band contributes filtered
  shading on the existing two-unit water mesh. Wind changes relax amplitudes
  rather than reseeding the surface. Displacement, slopes and compression
  derive from the same spectrum. Both periods divide the 1024-unit world.
- `ModernWaterCoast` computes periodic Euclidean coast distances from the actual
  wet mask, gradients and explicitly generated visual depth. It refreshes on
  terrain revisions and invalidates incompatible local wave history. This is
  not recovered seabed geometry or depth-buffer-derived bathymetry.
- `ModernWaterInteractions` uses a 256², 128-unit local GPU field, cell-snapped
  history scrolling, a stable 60Hz fixed step, absorbing borders and solid land
  barriers. Bounded swept pressure sources generate ripples without steadily
  adding water. Foam persists, drifts approximately with wind and decays.
- `ModernWaterSystem` composes these once per main frame; reflection views reuse
  the results. Shore attenuation affects horizontal and vertical displacement;
  normals differentiate the complete surface. A small controlled incoming wave
  accompanies lapping foam. Bounds include 3 units of spectral displacement,
  2 units of local height and 0.15 units of lapping.
- `ModernWaterEffects` owns bounded droplet and condensation pools. It adds no
  audio. Water-height clipping includes an approximate inverse horizontal
  displacement lookup; this is not exact particle/fluid collision.

Actual player/rival class-3 positions feed cosmetic downwash. The presenter
samples actors at bounded intervals and connects positions with swept stamps.
The normal event drain forwards already-consumed effects; water does not poll
the journal again. A source sound-27 update on a still-live class-10/model-5
splash can produce droplets and a ripple. This is a splash-update consumer,
**not** a new exact fireball-contact contract. A dedicated stable contact payload
is still needed to identify hot quenches independently of audio/actor lifetime.
The demonstration's fireballs and condensation are explicitly staged visual
inputs, labeled in the video. No gameplay quench/audio proof is claimed.

The first video exposed overly solid white foam. Subsequent iterations changed
the foam's coverage/bubble masks, pressure-to-foam response and coast injection,
and added a closer camera approach. The ordinary runtime leaves review controls
off. Run the reproducible owner-local capture tool with an installed player:

```sh
python3 Tools/capture-water-review.py /path/to/new/output-directory \
  --player /path/to/Builds/Linux/MagicCarpetNative.x86_64 --contact-sheet
```

It loads the existing NightSpire review replay, creates a private X display,
records 30 seconds at 1600×900, retains commands/logs/MP4/receipt, and stops only
its owned processes. `--baseline-command` selects another complete owner-local
replay command. Capture/encoding overhead is not a performance measurement.
The desktop remained locked throughout this work.

All 51 focused Modern EditMode tests passed on Unity 6000.3.20f1 with Vulkan,
including the actual FFT versus an independent direct-Fourier reference, GPU
impulse propagation, land barriers, scrolling/wrap/reset, periodic coast
distances and continuous wind. No test was skipped. Evidence:
`../generated/water-build-20261105/tests.xml` and `test-command.json`.
The matching player is installed as `build-x_vc2jl7`, retaining the same frozen
native `a98a8317e89a0cf5cebffb6e05091ecf78970394458b766647aa0234e29f3d09`
used for the rendering control; unrelated concurrent Rust edits are not part of
this Modern installation. This remains a video prototype, not the complete
water research acceptance or a foreground 60Hz/VR qualification.

Final video: `../generated/water-review-20261105/water-review.mp4` (30s,
900 encoded frames); `frame22.png` and `impact-sequence.jpg` retain the close
splash review. Root inspected the contact sheets and consecutive impact samples,
including the final droplet/condensation/foam sequence. These show improved
breakup without a visible local-patch boundary in the sampled view; they do not
establish every viewpoint or exact wave/particle collision. The capture receipt
confirms its private processes stopped. All 401 recorded Unity inputs match
canonical source and `build-player.sh --check-installed` passes.

Repeated no-encoder, 1600×900 private-Wayland runs, same replay/camera/native:

| Run | Mean ms | Median ms | p95 ms |
| --- | ---: | ---: | ---: |
| Final water 1 | 18.76 | 23.67 | 26.68 |
| Prior water control 1 | 17.47 | 23.10 | 26.69 |
| Prior water control 2 | 21.90 | 25.57 | 30.77 |
| Final water 2 | 17.73 | 23.25 | 26.46 |

Each samples 120 frames after a 20s warmup. Final means correspond to roughly
53–56fps, not a 60Hz guarantee. Clocks vary in this isolated display path; the
comparison supports bounded whole-frame cost, not an isolated GPU speedup.
An earlier candidate reread the event journal and all actors every tick and
took 22.76ms mean; the final shared-event/swept-observation path removes that
avoidable CPU work. Water simulation submission is about 0.03ms CPU here,
**excluding GPU execution**. Logs, captures and clock telemetry are
`../generated/nightspire-rework-20261105/water_final_*`; structured results are
`../generated/water-build-20261105/performance-comparison.json`.

New water/wind C# scanning reports no critical/warning findings. The broader
terrain scan remains partial due to dataflow limits; the one-line optional
event-forwarding join in `MagicCarpetRuntime.cs` was reviewed directly rather
than relabeling its whole-file legacy scanner backlog as candidate findings.
Root self-verified the installed prototype and video; the exact hot-contact
payload, nonlinear run-up, curling breakers, independent water geometry LOD,
all-view motion checks and foreground performance remain unfinished.

## Density-volume steam prototype (2026-11-06)

`ModernSteamVolume` replaces the condensation cards with persistent 3D density.
Four plume channels share a 12m region, one renderer and an analytically authored
wind/buoyancy/eddy field. Carpet sweeps create decaying wake primitives that keep
transporting density after the actor leaves. There is no velocity grid, pressure
solver or gameplay force. The ordinary player/rival observation path feeds the
same wake entry point as the explicitly staged review flight.

The default is 48³ cells; `-mc-steam-resolution 32|48|64` selects the comparison
sizes. Three RGBAFloat volumes consume about 5.06MiB at 48³, plus about 54KiB of
reduction storage. Fixed 30Hz updates perform limited MacCormack transport,
source/evaporation updates and a GPU mass ceiling independently per plume.
The ceiling uses actual remaining density plus new condensate and only scales
excess down. It never restores escaped mass to the box. The finite region has
an absorbing margin and matching soft render boundary. Source tracking follows
the same carrier field; rendering across the 1024m world seam shifts only the
display copy, preserving density history. Simulation advances once per update,
not once per camera/reflection.

Rendering integrates extinction and moon/ambient scattering through the volume,
including a bounded four-sample light attenuation estimate. Steam has no emission.
Opaque depth bounds the march; the broad water surface separately masks submerged
density. Rays stop at the volume boundary or sufficiently low transmission.
There is no temporal reconstruction. Small water-interaction displacement,
general terrain/actor fluid collision, detailed microphysics and distant-volume
handoff remain limits. The region holds four plumes; capacity replacement is
bounded presentation eviction, not conservation of every emitted cloud globally.

The first readback test accidentally measured only one z slice. It was corrected
to require/read every slice before computing mass. A later 28s test caught real
MacCormack mass growth (0.565kg at 7s versus a 0.189kg budget); the GPU ceiling
fixes this without weakening the test. The retained final sequence is 0.189kg,
0.128kg, 0.070kg and 0.033kg at 7/14/21/28s. Falling below the parcel total reflects
finite-region/transport losses, not a claim of an exact conservative CFD solver.

The clean steam candidate passes all 68 applicable Modern EditMode tests,
including real GPU readback of mass, overlapping source channels, persistent
wake differences, reset and world-seam display placement. The 76-test draft
run also passed before separating eight tests belonging to the paused night/
puff experiment; no acceptance test was deleted from canonical source. UBS
scanned the ten staged C# files with zero critical/warning findings; shader
validation comes from Unity compilation and execution, not UBS support.

The tested player is built in the existing isolated Modern project, with frozen
native SHA `a98a8317e89a0cf5cebffb6e05091ecf78970394458b766647aa0234e29f3d09`.
All 21 changed Unity inputs match the staged candidate. It does not replace
the separate Original campaign installation. Evidence, test XML, source hashes,
build commands and diagnostics are `../generated/steam-volume-20261106/`.
The actual desktop remains locked. Review captures are private-display evidence;
they do not prove foreground performance, VR or the gameplay Fireball contact ABI.

Use the existing capture tool with a baseline command containing both
`-mc-water-review` and `-mc-steam-review`. The labelled study stages two 2MJ
quenches over verified open water, uses a controlled variable breeze, enters the
cloud, then orbits its wake. Ordinary play retains autonomous variable weather.
`-mc-steam-off` disables density simulation/drawing while retaining parcel motion
and the review camera for matched cost comparisons. Frame timing statistics are
now enabled so the existing metrics consumer can request CPU/GPU frame timings;
the driver may still return no GPU samples, which must remain an explicit limit.

Final candidate is `build-j8d3tkao`; `final-build.log` records its installation.
No-encoder private-Wayland measurements use the same scripted fly-through,
1600×900, v-sync disabled, 13s warmup and 120 samples per run:

| Mode | Mean ms | Median ms | p95 ms |
| --- | ---: | ---: | ---: |
| 48³ steam, run 1 | 19.44 | 23.78 | 28.41 |
| Effects off, run 1 | 18.87 | 23.40 | 26.30 |
| 48³ steam, run 2 | 18.37 | 23.17 | 26.97 |
| 64³ steam comparison | 19.15 | 23.31 | 26.79 |
| Effects off, run 2 | 18.58 | 23.34 | 26.17 |

The two 48³ means correspond to approximately 51–54fps, with materially slower
p95 frames. This does not establish steady 60fps or a speedup. The observed
increment is smaller than run variation. Frame timing support reports enabled
and CPU counters work, but no GPU-frame samples return on this driver/display
path, so the proposed 2ms isolated GPU budget remains unverified. GPU clock
telemetry and commands are retained beside the logs. One effects-off attempt
timed out before scene startup and is excluded; its owned remaining private
session processes were stopped. Earlier Xvfb timing and pre-fix builds are also
excluded from this comparison. Summary: `performance-summary.json` in the same
evidence directory.

Final video: `/mnt/pinokio-shared/magiccarpet-build-proofs-20261106/steam-volume-04/water-review.mp4`
(24s, 720 frames, 1600×900). Root self-reviewed the contact sheet, full-resolution
7s frame and 16–21s wake sequence. The result shows connected curls, a continuous
interior during entry and lingering dispersal after the pass. The obvious
billboard disks and hard volume edge from earlier candidates are absent in
these inspected views. Fine-scale wisps remain soft; this is a working bounded
prototype, not completion of the wider environment/lighting art-quality target.
The capture receipt confirms all its owned processes stopped. Only the steam,
wind and thermodynamic prerequisite changes are landed; the rejected night/
torch work stays uncommitted in canonical source and was excluded from this
qualified player's Modern inputs.

## Thermodynamic context and unfinished night-lighting experiment

**Historical rejected experiment; superseded by the sandbox delivery above.** The following records
the earlier combined draft; the density-volume delivery above supersedes its
puff representation. Wind and thermodynamic prerequisites are part of the steam
candidate, while the separate night-lighting/shader drafts are preserved. The
65s `night-weather-02` capture of installed
candidate `build-8wjst540` showed cotton-ball steam, triangular luminous flames
and excessive horizon glow from two staged torches. Oliver rejected the result
and requested research. The replacement recommendation is recorded in
[WATER_RESEARCH.md](WATER_RESEARCH.md#post-capture-correction-steam-fire-and-night-lighting).
Source drafts are preserved; they are not an accepted visual upgrade. The later
sky-color and torch-mask edits have not been built or viewed. The 72 passing
Modern EditMode tests do not establish visual quality or performance. Both
captures used a private display, not the locked actual desktop; there is no new
foreground-performance result for this candidate.

`ModernWind` now supplies autonomous speed and direction changes with analytic
time integration. Explicit overrides and a continuous return to weather remain
available. This is authored cosmetic weather, not a new Rust flight-force model.

The fireball calibration treats a 0.65m luminous core as a magical store of
2MJ **remaining thermal energy at water contact**. Its size alone does not imply
that energy; MC2 damage values are not joules. At full quenching the default
transfers all of that heat to water. Heating 20°C water to 100°C and vaporizing
it costs approximately 2.592MJ/kg, yielding at most 0.772kg of steam occupying
about 1.29m³ at atmospheric pressure/100°C. Warming surrounding water without
boiling it would reduce that maximum. Reference: [NIST/IAPWS saturation
tables](https://www.nist.gov/document/nistir5078-tab1pdf).

Visible white mist is suspended liquid droplets, not the hot invisible vapor.
The initial 10%-condensation/0.5g/m³ illustration (6.66m equivalent diameter)
has been replaced in the renderer by an evolving moist parcel. Ambient
temperature, relative humidity and entrainment are explicit inputs rather than
an assumed fixed condensed fraction. NightSpire's authored review uses 12°C
and 75% RH. Puffs approximate the parcel's volume and optical depth; they are
not a resolved multiphase CFD simulation. See [NOAA's condensation
explanation](https://www.noaa.gov/education/multimedia/video/cooking-up-storm-atmospheric-science-in-your-kitchen).

`ModernSteamPlume` solves a constant-pressure, well-mixed parcel: entrained dry
air brings its ambient humidity and enthalpy, saturation partitions vapor from
liquid, and 48 bounded bisections close sensible plus latent heat. Ideal-mixture
volume sets the envelope. The default entrainment coefficient is explicitly
authored at 8kg dry air per kg initial steam per second; it is not a universal
measured plume law. The renderer uses at most eight parcels with 24 overlapping
puffs each, estimates extinction for 10µm droplets and advects their centers
with wind and a bounded buoyant rise. It does not solve bubble dynamics, pressure
waves, turbulent velocity fields or individual droplet growth.

Players and rivals disturb individual puff positions/velocities through
`ModernSteamWake`: a compact 3D swept path with radial displacement and paired
rolling cores. Motion is evaluated relative to the cloud's moving air. Additional
mixing goes through the same enthalpy/water solve; spreading reduces optical
depth rather than abruptly erasing the cloud. Teleports, invalid inputs and
unbounded speeds are rejected. This is a bounded visual wake approximation,
not a resolved turbulent flow or a new force on gameplay actors.

Actual C# parcel values for the authored night conditions (no carpet disturbance):

| Age | Temperature | Condensed droplets | Equivalent parcel diameter |
| --- | ---: | ---: | ---: |
| 1s | 55.4°C | 0.088kg | 2.35m |
| 4s | 32.1°C | 0.169kg | 3.50m |
| 8s | 23.5°C | 0.192kg | 4.34m |
| 18s | 16.8°C | 0.170kg | 5.62m |

These diameters include entrained air; they do not describe solid opaque balls.
Incoming humid air contributes water. Numerical heat residuals in the retained
timeline are below 9e-8J, which verifies the chosen model's algebra, not real
plume accuracy. CSV: `../generated/night-weather-20261105/plume-12c-75rh.csv`.

FrankenSim was evaluated at `2cac4277a1ae7db2e7d269d3252fef776c85c1aa`.
Its [water-property APIs](https://github.com/Dicklesworthstone/frankensim/blob/2cac4277a1ae7db2e7d269d3252fef776c85c1aa/crates/fs-thermochem/src/water.rs)
and IF97 module provide saturation/enthalpy calculations, but not a ready moist
plume time integrator. No FrankenSim code was copied or dependency adopted;
the current model is independently implemented from NIST caloric anchors and
the [published Buck saturation formula](https://www.eol.ucar.edu/data-software/conventions-and-standards/water-vapor-pressure-formulations).
Future property-table cross-checking would first need dependency and license
qualification; upstream's license has additional restrictions beyond MIT.

`ModernNightLighting` consumes existing cached presentation rows, avoiding an
extra arena serialization. Fireball/flame emission uses verified parameter/base
TMAP pairs, including `(340,463)`, `(42,55)`, `(41,54)`, `(7,38)` and
`(228..235,77)`. The `(41,54)` appearance can also be an explosion; it is not
assumed to be a permanent torch. House rows `(177..184,96..103)` receive
explicitly authored modern exterior torches; no original lamp socket is claimed.
Eight selected shadowless local lights illuminate terrain, plants, actors,
mist and water. Up to eight house localities contribute bounded sky/cloud glow.
Torch geometry is pooled; no Unity collision or gameplay entities are added.

The sky keeps a bounded cloud march, with wind-advected weather and detail,
moonward scattering, antialiased angular stars and moon disk. Review controls
show water and a flight through steam for 35 seconds, moon/clouds for 12, then
a torch study on dry terrain.
The staged quench label remains: the current campaign ABI still lacks a reliable
hot-contact payload. Source ambient fires/projectiles and house observations
drive ordinary lighting, while demonstration impacts remain explicit inputs.

The frozen `a98a8317` native replay has no active houses or fire sources at
ticks240/300/540; all16 source-yaw observations show smoke only. Fresh startup
briefly presents fire/impact rows but these disappear before the capture warmup.
Consequently this review explicitly stages its torch locations and makes no
live-house gameplay claim. A public Fireball cast also reproduces the separately
owned legacy auxiliary-record failure (index589 into573 bytes); the isolated
probe stopped without creating a replay. Evidence is `native-light-census.json`,
`native-light-startup.json` and `native-light-fireball.json` in the same evidence
directory. No Rust/ABI change is bundled with this presentation work.

## Current limits

This is a desktop environment upgrade, not a completed conversion of every game
asset. Creatures and effects still use the original sprite art. Scattered rocks
and vegetation are decorative and do not add Rust collision obstacles. Water
reflection currently assumes NightSpire's verified zero-height water plane.
The original terrain layout remains recognizable, with reconstructed slopes and
continuous Modern banks. Water run-up, cloud illumination and steam optical
detail are bounded real-time approximations, not a fully resolved fluid solver.
The four-field steam budget fades an old cloud before admitting an overflow.

VR/OpenXR, Windows runtime validation, complete 3D characters and high-resolution
replacement menus are not delivered here. Original graphics preferences are
not yet all connected to the modern effects. Native campaign work still takes
substantial CPU time; a successful rendering capture is not a 60-fps performance
claim. The Level1 package's missing banishment-state carrier
also remains a separate known native menu-integration failure.
