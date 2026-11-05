# Modern NightSpire renderer

The opt-in desktop renderer uses the same live Rust campaign session as the
original presentation. Rust still owns movement, combat, terrain changes,
input admission, saves and replay. The first supported modern environment is
NightSpire (Level 1); other destinations retain their existing presentation.

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
- Water with layered ripples, depth-based shallow color, shoreline foam and a
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

## Current limits

This is a desktop environment upgrade, not a completed conversion of every game
asset. Creatures and effects still use the original sprite art. Scattered rocks
and vegetation are decorative and do not add Rust collision obstacles. Water
reflection currently assumes NightSpire's verified zero-height water plane.
The original terrain's large-scale angular silhouette is retained.
Its square shoreline remains visible, and the clouds need more motion and
sampling-quality review. These are unfinished visual-quality limits.

VR/OpenXR, Windows runtime validation, complete 3D characters and high-resolution
replacement menus are not delivered here. Original graphics preferences are
not yet all connected to the modern effects. Native campaign work still takes
substantial CPU time; a successful rendering capture is not a 60-fps performance
claim. The Level1 package's missing banishment-state carrier
also remains a separate known native menu-integration failure.
