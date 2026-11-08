# Level 1 voxel experiments

## Native-connected Level 1 interaction

`run-voxels.sh --connected` selects the separate `Builds/VoxelsConnected`
player-06. The accepted default player-15 and `--classic` are preserved.
`--connected --goat` and `--connected --archer` open their native character views.
The connected world starts from an earned Level 1 replay near house 38, with the
native player camera and ordinary Fireball selected. Space casts; F5 saves;
P pauses/resumes; F2 restores the native view; B opens the observer; F10 exits.
Observer movement changes the camera, not the native aim of a spell.

The save destination is separate from the accepted sandbox and campaign.
`MC_VOXEL_SAVE=/absolute/path.json` overrides it. To resume an F5 save:

```sh
MC_VOXEL_LOAD=/absolute/path.json research/private/remc2-unity-native/run-voxels.sh --connected
```

Rust now owns projectile contact/damage, exact removed terrain/house cells,
death lifetimes, mana production/ownership/collection and replay persistence.
Unity projects that state into meshes and physical presentation fragments.
The 25 cm house and 12.5 cm ground patch retain their previous extent; this is
one connected interaction, not complete Level 1 assembly or whole-map streaming.
Other buildings, creatures, vegetation and legacy combat effects have not all
been voxelized. The house follows the source BUILD profile, not every ornament
on the Modern architectural meshes.

Goat and archer deaths freeze the current skinned voxel pose and emit separate
physical pieces once per native world/slot/generation/source identity. Removal
alone is not death. Pause rebinds retain the world epoch; slot reuse and replay
rollback do not retain an old occupant. Pool sampling remains bounded at 768
bodies; representative cubes carry sampled mass, not one Rigidbody per cell.
Debris is presentation only and does not create or collect gameplay mana.

Mana uses a shared three-dimensional voxel sphere mesh with native size, value,
palette and ownership. It survives rubble fading and follows native collection
and hide/delete rules. Its observer admission uses a new guarded world census;
ordinary source-camera actors retain their existing admission path. A mana orb
may hover or move according to native state. The presentation corrects its
bottom edge against the Modern terrain without changing gameplay coordinates.

Source: client `73408438`, native geometry/journals `b0db8ae4`, additive world query
d6665e38. Player-06 pins native SHA256 `ea9276a3848ae8cc63907992e8daa6376b6b8e4811378dbcf5a4478977fb6731`
and managed assembly `a1ea55bf4ea29bc7eb892c145ce3f15f01f056349963d3c1cb1999121bda7e40`.
Its isolated shared UI excludes the separate player-lifecycle work. All 424
copied source inputs and 195 read-only resource inputs matched after the build.
The install receipt verifies all 170 payload files before launch. Nine immutable
engine/asset files share inodes with the previous candidate; never patch either
completed player in place. Both independent original trees are archived on Rig.

Focused verification: 71 relevant Unity checks passed (one opt-in profiler
skipped), the revised pending-chunk collision case passed independently, and 9
Python receipt/input checks pass. The earned native E2E checker independently
reproduces archer 273 death at tick 535, sphere 364/value 500 birth at 539,
player 152 claim at 543, defender 372 collection at 1574, reclaim at 1575 and
Castle 113 arrival/500 receipt at 1969.
Cold replay hashes match. The ABI does not expose numeric Castle balance;
credit evidence is the arrival event and its authoritative consumer call.

Actual player-06 ground runs each converge 6,816 native/visual removed cells at
hash `5432479835024191935` with zero below-terrain fragments in every category.
Player-05 emitted 68 impact fragments below terrain: the original Fireball path
can restore old horizontal coordinates while retaining the next column's
height. At this hit the receipt lies0.375 units below the old local surface.
The client now queries currently committed ground colliders and moves the whole
fragment cloud clear of stepped columns, preserving shape and incoming momentum.
The regression also covers a committed center chunk beside a pending neighbor.

Unthrottled 1280×960 on an RTX 3090 private GPU compositor, no image readback:

| Scenario | Median ms | p95 ms | p99 ms |
|---|---:|---:|---:|
| Player-05 quiet, repeat 1 |27.51|32.92|53.33|
| Player-05 quiet, repeat 2 |27.62|37.05|52.47|
| Player-06 native ground impact, repeat 1 |32.42|43.47|54.46|
| Player-06 native ground impact, repeat 2 |32.64|54.62|80.01|

The percentile window starts after the first second of Unity game time; the
initial impact occurs earlier. These are post-impact settling measurements.
Whole-run PhysX maxima are 16.60/8.31 ms and edit-step maxima 5.91/8.92 ms.
The complete initial impact frame's outlier cost has not been isolated.
Physics p95 is 2.99/2.98 ms with 384 peak bodies. These short samples retain
whole-frame variability; they do not establish hitch-free performance or 60 fps.
An earlier same-player debug/release comparison measured 168.21/27.40 ms median,
but that was one quiet pair, not final impact performance. Capture timings
include readback and are never used as frame-rate evidence.

The checked [26-second phone video](http://3090.bumblebee-hue.ts.net:8095/voxel-connected/)
has chapter controls and preserves the full frame. House and terrain play at
0.67×, visibly labelled; character scenes play at their captured rate. Root
inspected bursts, separate pieces, settling, the crater and mana after rubble
fade, and decoded the complete published H.264 file. Video SHA256 is
`d2954a3275b7863cdb450ad92c10abbf5a42f3278a03566f3b8950d80f8b1029`.
The original flame sprites obscure the dying archer early; the standing actor
at the left is a bystander. The native victim is identified by its impact/death
records, not by that bystander's appearance.

Installed-launcher XTest checks pass for B/F2/P/Space/F5/F10. Fresh loads restore
house 626 cells/hash `15883481164168150708` and terrain 6,816 cells/hash
`5432479835024191935`, with matching native/visual counts and no historical
impact emissions. Both close-ground KWin captures failed their separate F5
save checks; they remain failed runs. The earlier Xvfb ground save and independent
installed cold-load provide persistence evidence. The backend difference does
not establish the cause of the missed F5 events.

Evidence starts at `research/private/generated/mc-voxel-combat-20261107/`.
Complete captures, including failed runs, are verified on
`rig:/data/projects/magiccarpet-voxel-final-evidence-wildfox-20261108/mc-voxel-final-20261108`:
1,318 entries, manifest SHA256
`38b39babc0a83368a0bfc845d3dabe4a45bd12ea0ff8df7212def3a6c866d537`.
Root independently regenerated the remote file/mode/hash manifest. HTTP full
download, metadata, byte ranges and fast-start layout pass using the advertised
Tailnet Host and IP. The local OS and remote in-app browser lack MagicDNS
resolution; interactive phone-browser playback, the locked desktop and VR remain
unqualified.
C# UBS is partial: the ground fix has no critical/warning findings; the wider
18-file scan reports controlled dictionary/required First lookups and lacks
complete semantic coverage. Compilation, focused tests and real producer-to-
consumer runs provide the main verification.

## Modern environment with fine voxels

The new opt-in player combines the existing Modern environment with local
voxel destruction and the approved goat/archer rigs driven by native gameplay.
The previous standalone comparison and accepted Modern campaign remain available.
This is still an experimental sandbox, not a fully voxelized campaign.

`run-voxels.sh` selects the separately installed `Builds/VoxelsModern` player.
`run-voxels.sh --classic` runs the preserved first experiment described below.
`run-voxels.sh --goat` and `--archer` start the matching live character views.
The Modern sandbox starts from an earned Level 1 input replay; it then continues
normal native simulation. Its save destination is separate from the campaign;
`MC_VOXEL_SAVE` can override that destination.

Controls: WASD and Q/E move, right-drag looks, Space/click fires a local voxel
ball, B returns to the house/free camera, F2 returns to the native game camera,
N cycles lighting and F10 exits. Only one local shot/destruction operation is
admitted at a time; the on-screen busy message makes this explicit.

The house uses 0.25-world-unit cells at the original full scale. The 32×40-unit
editable ground patch uses 0.125-unit cells, joined to the surrounding Modern
terrain with a visible, collidable vertical seam; distant terrain keeps its
existing mesh detail. The seam closes the quantization lip without placing a
floor across excavations.
World units are treated as metres for display, not a source-game unit assertion.
The shell follows the source BUILD profile; it is not a voxelization of every
ornament on the Modern building meshes. Plants uprooted by a ground edit are
removed from affected vegetation chunks. Textured terrain, vegetation, water,
clouds and lighting come from the existing Modern system.

Nearby characters use 96 cells per authored model height. Distance detail uses
existing 64/40 goat and 64 archer skins with hysteresis, while a followed character
keeps 96. Skins share the original rig, motion and contact correction. The archer's
bow, string and arrow remain their approved separate meshes. Internal faces are
removed only between cells with identical skin weights, preserving animated
seams. Death snapshots skin cached cell centres directly rather than baking
all cube vertices. Native death triggers fragmentation; custom sandbox balls
currently damage local terrain/buildings, not native character health.

Rubble is now a prewarmed pool of 768 independent boxes, with up to 192 spatial
samples per emission and 64 activations per render frame. Source mass is
represented by weighted samples; not every removed voxel is rendered and rubble
volume is not conserved. Blocks are at most 0.20 units; the fireball uses 0.065
cells. Gravity, contact, friction, rebound, sleeping and rest-based fading are
physical. All debris sweeps against static geometry; debris-to-debris contact
is discrete at 120 Hz. The intact ball uses continuous dynamic collision. Native
gameplay stays at 30 Hz. Meshing/support work is spread over render frames.
Local destruction is not written into native terrain, saves or AI pathfinding.
Terrain contacts are retained in 32-unit tiles around the camera and active
fragments, including pieces rolling away downhill. Camera cuts do not cook the
empty space between old debris and the new view. Source terrain revisions refresh
only tiles whose sampled heights changed. Continuous static contact applies to
all fragment sizes; an actual triangle-mesh crossing test covers 2.5, 12.5 and
20 cm cubes at 30 m/s. This avoids the expensive all-pairs dynamic CCD mode.

Builds use `BuildVoxelPlayer.Build` as below, with GPU batched skinning scoped to
this player. `-mc-voxel-modern` composes it with the normal native runtime.
`Tools/capture-voxel-sandbox.py --arguments-json PATH --backend compositor`
accepts the installed native resource/replay arguments. Add `--benchmark` to
measure without capture readback or `--expect-native-death` for the death case.
The world validator requires three completed edits, both house and ground removal,
and a collision probe showing an actual crater. All Modern runs reject active
debris below the unchanged terrain; the editable crater is excluded from that
height test. Character runs require advancing
native ticks. `Tools/check-voxel-controls.py --arguments-json PATH` tests real
keyboard movement, camera switching, a physical impact and exit on a private
X display. Recording uses an authored camera/shot sequence; character behaviour
comes from the running game. Offline video timings are not frame-rate evidence.

## Modern refinement verification

Evidence root: `/mnt/pinokio-shared/magiccarpet-voxel-modern-20261107/`.
Installed `player-15` contains the actual reduced Goat96 mesh (305,872 vertices,
down from 510,600) and Archer96 (294,192, down from 316,080). The importer checks
the stored face count; an earlier CopySerialized failure was caught by the
topology test and repaired before qualification. Original generated assets were
backed up. Matching-weight face seams and death poses are checked independently.

The 36.5-second [phone demo](http://3090.bumblebee-hue.ts.net:8095/avatar-lab/voxels-fine/index.html)
has chapter controls, a 19 MB phone copy and an 80 MB full-quality copy. The local
master is `level1-fine-voxels-checked.mp4` under the evidence root. Final player-15
frames inspected include goat 100, archer 40/60, native death 25, and world
285/690/820: intact fine skins, coherent draw/release, separate fragments,
closed terrain seam, an open crater and settling/fade. Native death's first
frames retain the conspicuous original flame sprite; native combat effects have
not all been voxelized. This is a known visual limitation, not hidden in the video.

`installed-controls-15/controls.json` records actual F2/B/W/Space/F10 input through
the installed launcher, including camera movement and a real impact. The
launcher verifies the installed payload against `voxel-install.json`. Both
`Builds/Voxels` (first sandbox) and `Builds/CharactersGameplay` (approved Modern)
retain their previous targets. The artifact receipt hashes the native plugin
independently and records all C# inputs, including unused pre-existing untracked
SteamWake sources; those unrelated files are not included in this delivery commit.

`tests-4.xml`: 93/93 relevant tests passed. After the final terrain-height cache,
`tests-5.xml`: 29/29 affected terrain, hoof-contact and integration tests passed.
These sets overlap and should not be added as a unique test total. The cache
stores the exact periodic detail lattice and invalidates on terrain changes;
it does not simplify terrain shape. Its mutation test compares warmed sampling
against a freshly constructed terrain across height, material, water and level
generation changes. Existing rendered-triangle agreement checks still pass.

`tests-11.xml`: 18/18 focused physics, sparse contact, source-height refresh and
seam tests passed. The physical seam regression drives a 6.5 cm box horizontally
below the outer terrain height at 120 Hz, requiring the lip to stop it while a
vertical ray confirms the patch interior stays open. Earlier actual-player
failures are retained: player-10 had 92 fragments beyond camera-only contacts;
player-14's remaining fragments last contacted the two patch edges before
escaping beneath the uncollidable visual seam. Qualification now rejects any
active fragment more than 0.25 units below the unchanged surrounding terrain.

Unthrottled 1280×960 runs on the RTX 3090, private GPU compositor, no image readback:

| Scenario | Median ms | p95 ms | Worst frame ms |
|---|---:|---:|---:|
| House/ground, player-15 | 25.27 | 33.55 | 88.48 |
| House/ground, player-15 repeat | 26.87 | 39.25 | 95.25 |
| Native goat death and debris, player-15 | 29.11 | 36.50 | 67.18 |

Both world runs completed three edits: 1,077 house and 6,532 ground cells,
with a collision-verified 2.125-unit crater. No emissions were dropped and zero
active fragments were below surrounding terrain. The 120-Hz physics work's
worst native turn was 9.81 / 23.47 ms; the earlier packed
ContinuousDynamic debris candidate reached 168 ms. Body-to-body CCD was the
destruction spike, not chunk meshing. Whole-frame outliers remain; this is not
a hitch-free or 60-fps claim. These native-game world runs are about 37–40 fps median,
not the 239 fps of the simpler first experiment without the full native game.
In the isolated player-8/player-9 cache comparison, the goat camera previously
measured 50.52 ms median; caching terrain contacts
reduced character-render median from 6.78 to 0.85 ms and full frame median to
28.20 ms. GPU skinning alone and hidden-face reduction were not the main CPU fix.

UBS was run on worker and integrator changes. C# semantic analysis remains
partial. Whole-file actor warnings concern existing dictionary paths and structs,
not a discriminating candidate scan. The final terrain scan had zero critical
or warning findings within its partial scope. Python's executable-selection
warning is the intentional `--player` argument with an argv array and no shell;
processes are terminated/waited in finally blocks, and JSON errors reach the
outer fatal handler. Focused compilation, tests, changed-hunk review and actual
player checks provide the main evidence. No Rust source was changed for this
iteration; the native plugin is the accepted `dc5a1153…2288d` artifact.
The final seam scan exits 2 for unresolved C# callable/null effects; it is not a
clean semantic scan. Its two First() warnings are intentional test fixture lookups
that must fail if the required retained collider is absent.

## First accepted experiment — preserved installation


This is a separate, owner-local Unity physics experiment. The accepted Modern
and Original players, their assets and campaign saves remain available. It does
not replace Rust's authoritative campaign simulation or implement campaign
terrain persistence/network replication.

Run `research/private/remc2-unity-native/run-voxels.sh --classic` from the repository root.
WASD moves the camera; Q/E moves down/up; right-drag looks; click or Space shoots.
1/2/3 rebuild with 0.50 / 0.33 / 0.25-unit terrain/building voxels (fine is the default); R resets;
F10 or Escape exits. All three goat skins are visible together: 24/40/64 cells
per model height, using the approved goat model, sampled colors and clips.

## Source geometry and physics

`Tools/prepare-voxel-inputs.py` verifies the frozen Level 1 terrain and building
patches before exporting ignored owner inputs. The scene uses source house 26,
slot 38, and the surrounding source terrain at uniform half scale. Height
interpolation uses the source's alternating triangle diagonals. The house's
exterior follows the original BUILD height profile; its hollow shell, windows,
door and material colors are prototype interpretation. Shell thickness is 1.6
scene units so all three discretizations begin face-connected to the foundation.

The fine interactive terrain area is 32 by 32 scene units. Outside it, coarser
terrain has collision but is not editable. The voxel goat is a skinned surface,
not a solid tissue simulation. Death bakes its current pose, rasterizes occupied
cells onto a world-aligned grid and forms physical clusters. Quantization moves
a cell center at most half a cell per axis; duplicate cells merge and average
color. Whole disconnected building/terrain islands become rigid bodies.

Fireballs are cohesive voxel spheres in flight. On contact their fragments
inherit incoming velocity and collide against the still-intact surface before
the destruction callback removes cells. A shared contact-plane clearance avoids
spawning fragments inside the wall. Rubble uses gravity, friction, restitution,
real collision and sleeping. Resting pieces fade after at least three seconds;
a separately counted 45-second limit removes persistently moving pieces.

Physical bodies are capped at 160, with normally at most 32 clusters per emission.
Splits prefer the longest spatial axis, avoiding thin, full-height rubble slices.
Each cluster has a slightly inset bounding-box collider. Fine visible cells do
not each have their own rigidbody, and concave chunks use conservative collision
approximations. Structural support is connectivity to the base, not engineering
stress, fatigue or fracture mechanics. The orange fireball is a physical visual
experiment, not a thermodynamic combustion model.

## Rebuild and reproduce

Use the canonical project and serialize Unity imports/builds. Preserve existing
player outputs; `BuildVoxelPlayer.Build` requires a fresh executable path.

```sh
python3 research/private/remc2-unity-native/Tools/prepare-voxel-inputs.py
```

Run the installed Unity editor with `-projectPath` pointing to this client and
`-executeMethod MagicCarpet.Native.Editor.BuildVoxelPlayer.Build`. Set
`MC_VOXEL_PLAYER_OUTPUT` to a new player path. A working graphics display is needed
for texture sampling; the build retains validated, already-generated voxel goats.
The new player scene does not bootstrap the normal campaign runtime.

Focused EditMode filters: `VoxelGoatEditModeTests;VoxelVolumeEditModeTests;VoxelPhysicsEditModeTests;VoxelSandboxEditModeTests`.

```sh
python3 research/private/remc2-unity-native/Tools/capture-voxel-sandbox.py \
  --player /absolute/path/MagicCarpetVoxels.x86_64 \
  --output /absolute/path/new-run --backend compositor
```

Add `--benchmark` for unthrottled timing without image readback; `--size .5` or
`--size .25` changes terrain detail. Each run preserves logs, output, command,
player/managed-assembly hashes and a JSON report. Captures advance at 30 recorded
frames per second and their wall-clock timings are not performance claims.
Physics simulation runs at 120 Hz. Recordings use the actual player and physics,
with an authored camera path and scripted shot inputs.

The private KWin/Xwayland compositor is isolated from the desktop session and
uses the real GPU. Xvfb is retained as a fallback, but it introduced a severe
presentation penalty in this environment: the same diagnostic player measured
31.27 ms median on Xvfb versus 1.82 ms on the private compositor at 1280×960.
Those diagnostic runs predate the final fragmentation fixes. The locked actual
desktop did not advance through window initialization; do not present the private
compositor as unlocked desktop/playtest proof. Unity's release physics profiler
marker was unavailable, reported as -1 rather than a misleading zero.

## Qualified delivery — 2026-11-07

The installed local player is `Builds/Voxels/MagicCarpetVoxels.x86_64`, linked to
`/mnt/pinokio-shared/magiccarpet-voxel-sandbox-20261107/player-7/`. Its managed
runtime SHA-256 is `bd53c880a7e5ca378f646671b06c9a9c66ba123a5de30268fdc8ba869453fc50`.
The previous Modern installation was not replaced.

The 24 focused Unity EditMode tests passed (`tests-compact.xml` under that evidence
root). They cover real generated goat assets, world-space death-pose bounds,
fragment conservation and nonoverlap, compact partition shape, supported house
construction at all presets, collision after removal, newly exposed chunk layers,
and local unsupported-component extraction against a full flood reference.

The 34.03-second `capture-compact/voxel-sandbox.mp4` is 1280×960 at 30 recorded fps.
Inspected comparison, gait, first contact, fragmentation, roof damage, crater,
settling and fading frames. Earlier captures exposed and led to fixes for an
unsupported coarse roof, source diagonal interpolation, missed shots, double
scaling of baked goat debris, unsplittable deformed cells and slab-shaped rubble.
Fine terrain is the default. This is a checked prototype, not a claim of physical
material accuracy or a full voxel campaign.

[Phone video with chapters](http://3090.bumblebee-hue.ts.net:8095/avatar-lab/voxels/index.html)
retains the previous Modern video as a separate link.

Final player-7 benchmarks use the isolated GPU compositor on the RTX 3090 at
1280×960, without captures, VSync or a target frame cap. Each is the same 34-second
camera/shot sequence. These first final runs are reported in milliseconds:

| Terrain cell | Median | 95th percentile | 99th percentile | Worst frame | Peak bodies |
|---|---:|---:|---:|---:|---:|
| 0.50 | 3.93 | 5.85 | 6.80 | 31.51 | 121 |
| 0.33 | 3.92 | 5.64 | 6.72 | 36.74 | 131 |
| 0.25 | 4.19 | 5.82 | 7.73 | 37.71 | 130 |

A second final run at each size (`final-repeat-*`) measured median / p95 / worst
frame: coarse 3.93 / 5.63 / 34.05 ms; medium 4.06 / 5.77 / 39.33 ms;
fine 4.17 / 5.82 / 35.34 ms. Each again completed all four impacts and one goat
death. The fine preset's typical frame cost was consistent across the two runs.

The separate 64-cell goat destruction stress run reached a **68.04 ms worst
frame**; splitting and mesh creation remain synchronous. The normal comparison
shoots the 40-cell goat. Do not call all impact frames smooth or infer full-game
FPS from these sandbox results. All four runs produced four physical impacts,
one goat death, terrain/building removal, contact/rebound observations, sleeping
pieces and fades; no 45-second forced-fade fallback fired. The actual desktop
remains locked, so unlocked desktop interaction has not been qualified.

`Tools/check-voxel-controls.py` injects actual keyboard input into the standalone
player on a private X display, takes screenshots, and verifies preset/reset,
camera movement, Space impact and F10 exit from the real runtime log. This is
separate from the authored recording. The test does not alter the real desktop.
The installed `run-voxels.sh` launcher passed this check (`installed-controls/controls.json`):
keys 3, 1, 2, R, W, Space and F10; four resets and one physical impact.

UBS source checks were reviewed alongside Unity compilation/tests. Some C#
semantic scans are partial because of scanner limits and absent dotnet tooling.
Python public-integrity hash comparisons are not secrets, outer JSON exceptions
fail the command, and process handles are cleaned up in finally blocks. Those
scanner findings were triaged rather than suppressed or called clean.
The final 13-file C# scan reported zero critical and two warnings: numeric CLI
parsing is inside a fatal-error catch that logs and exits nonzero; the test's
dictionary list is initialized on the missing-key branch. Neither is an observed
runtime defect. Semantic coverage remains partial; the scan is not a clean bill
of health.
