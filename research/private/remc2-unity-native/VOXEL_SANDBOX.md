# Level 1 voxel sandbox

This is a separate, owner-local Unity physics experiment. The accepted Modern
and Original players, their assets and campaign saves remain available. It does
not replace Rust's authoritative campaign simulation or implement campaign
terrain persistence/network replication.

Run `research/private/remc2-unity-native/run-voxels.sh` from the repository root.
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
