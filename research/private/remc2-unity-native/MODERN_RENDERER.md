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
