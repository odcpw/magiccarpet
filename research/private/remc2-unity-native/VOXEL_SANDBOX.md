# Level 1 voxel experiments

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
