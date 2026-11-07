# Character animation experiments

Fresh sprite-informed goat experiment: `../generated/goat-animation-20261106/`.
Original sprites inform identity; no rejected prototype mesh, rig or motion is
reused. Generated files, original assets and credentials remain owner-local.

## Checked complete-action Unity characters

The earlier one-second loops below are retained experiments. The current work
uses one synchronized FBX timeline per character, editable Blender scenes,
explicit PBR texture files and a JSON manifest of named clip ranges. Unity's
native importer creates Generic rigs and the review player uses the actual
imported clips through `ModernCharacterInstance`.

The goat has a separately authored four-hoof standing idle and the repaired
walk. The archer has Idle, Walk, a five-second reach/nock/draw/release/recover
action, kneel preparation/descent, a kneeling shot and a rise. Modern motion
between the source game's held sprite stances remains authored interpretation.
The hands retain modeled grips rather than individual finger articulation.

Load-bearing integration fixes:

- Normalize model facing, origin and scale before export; keep one bind pose.
- Export all body, bow, string and arrow tracks on one timeline. Independent
  NLA stacks in FBX lose synchronization between those objects.
- Give packed images distinct filenames and retain explicit albedo, normal and
  ORM maps. Empty filenames caused the FBX reimporter to use albedo as normals.
- `ModernCharacterImport` converts ORM channels into Unity metallic, occlusion
  and smoothness, then creates URP materials and named clips from the manifest.
- Preserve fine contact keys with `ModelImporter.resampleCurves = false`.
  Resampling lost the planted walk and separated the nock from the hand between
  authored presentation frames; the same exported FBX passes with its keys kept.
- Unity geometry checks use `BakeMesh(mesh, true)` before `TransformPoint`.
  Otherwise the goat renderer's import scale is applied twice to the measurement.

The selected assets are `goat-neutral2` and `archer-candidate12` under
`../generated/characters-complete-20261106/`. Each contains its editable blend,
FBX, clip manifest and three source texture maps. All 12 focused Unity checks
pass (`unity-final-check-tests.xml`, 2026-11-07). Actual player frames exposed a
front shoe that lifted/slid during rising and an idle inherited from walking;
the selected archer has a separately grounded idle and planted-front-leg IK.
The rise check measures a maximum 5.8 mm sole height and 12.6 mm horizontal
centroid drift; the previous candidate failed at 182 mm and 412 mm respectively.

The first 36.4-second portrait capture is retained at
`/mnt/pinokio-shared/magiccarpet-character-review-portrait/`, with a separate
visual rejection receipt: its raised bow and held arrow overlapped/crossed the
view boundary. The selected player uses Unity's conservative world culling
bounds to fit the character and held props inside a separate viewport, with
titles outside it. A discarded preview snapshot calculation enlarged scaled
bow props by 100x after the first frame. The corrected actual-player capture
has 874 complete 1080x1920 frames at 24 fps (36.42 seconds), with zero finite/
near/far/viewport failures:
`/mnt/pinokio-shared/magiccarpet-character-checked-unity/characters.mp4`.
Goat gait, standing draw/release, descent, kneeling shot, rise and settled stance
frames were inspected by both the motion owner and integration owner. The real
interactive player also passed keyboard selection/pause, Shoot, slider scrub
and clean Escape exit on a private display. Recording is presentation evidence,
not a desktop performance qualification.

The installed inspection player is launched with
`research/private/remc2-unity-native/run-characters.sh` from the repository root.
Its `Builds/Characters` link selects the checked player on the HDD. The phone
review is available within Oliver's tailnet at
`http://3090.bumblebee-hue.ts.net:8095/`, with chapter buttons and MP4 download.
Its local-only service reuses FitTimer's existing range-serving implementation;
it serves a separate directory containing only this review page and media.

Canonical implementation:

- `goat_complete.py`: load the selected rigged goat blend; author compatible
  idle/walk and bake a ground root, then reopen before FBX export to refresh
  Blender's cached bind matrices.
- `archer_complete.py`: assemble the selected walk, standing and kneeling
  blends into complete actions; bake contact and ground corrections at 96 Hz.
- `character_textures.py`: preserve source map bytes with distinct image paths.
- Unity `ModernCharacterImport.Prepare`: import clips, textures, prefab and
  definition from `Assets/Resources/ModernCharacters/{Goat,Archer}/`.
- Unity `ModernCharacterBuild.Build`: build the interactive inspection player;
  it does not open a campaign or a save.
- Unity `Tools/capture-character-review.py`: record the player on an owned
  private display, encode full frames and retain logs/receipts on failure.

Focused checks and a new player build, after staging owner-local exports:

```sh
UNITY=/home/oliver/Unity/Hub/Editor/6000.3.20f1/Editor/Unity
CLIENT="$PWD/research/private/remc2-unity-native"
"$UNITY" -batchmode -nographics --burst-disable-compilation \
  -projectPath "$CLIENT" -runTests -testPlatform EditMode \
  -testFilter MagicCarpet.Native.Tests.ModernCharacterEditModeTests \
  -testResults /absolute/new/tests.xml -logFile /absolute/new/tests.log
MC_CHARACTER_IMPORT_REPORT=/absolute/new/import.json \
MC_CHARACTER_PLAYER_OUTPUT=/absolute/new/player/MagicCarpetCharacters.x86_64 \
  "$UNITY" -batchmode -nographics -quit -projectPath "$CLIENT" \
  -executeMethod MagicCarpet.Native.Editor.ModernCharacterBuild.Build \
  -logFile /absolute/new/build.log
python3 "$CLIENT/Tools/capture-character-review.py" \
  --player /absolute/new/player/MagicCarpetCharacters.x86_64 \
  --output /absolute/new/capture
```

Stage the selected `goat.fbx` and `goat.clips.json` as `Goat.fbx` and
`Goat.clips.json` in `Assets/Resources/ModernCharacters/Goat/`; likewise capitalize
the archer pair in `Archer/`. Copy each export's `textures/` folder there too.
These files and all importer products remain ignored. The test-only Burst flag
avoids an observed Editor startup/unused-asset-GC deadlock; the built player
retains normal Burst compilation.

Original campaign animation mapping, terrain foot placement, deaths and crowd
LODs are separate from these character assets. This is a working author/export/
import/inspect path for two characters, with character-specific motion repair;
it is not a general automatic image/video-to-rigged-animation system.

The initial experiment separates reference art, a fixed textured mesh, motion
video and deformation reconstruction. A video is only a motion reference. The
deliverable must be independently renderable 3D geometry, inspected from both
sides and the rear. A morph animation does not establish a reusable skeletal rig.

## Skeletal goat follow-up

Oliver accepted the fresh textured goat as the starting shape and requested
Blender repairs. `rig_goat.py` now binds that mesh to 14 bones, preserves the
skull and hoof shapes, solves fixed-length leg chains, and authors a one-second
four-beat walk with planted stance intervals and restrained body/head motion.
Two small reconstruction ridges receive localized smoothing; their faint baked
texture marks remain. This is a manually authored interpretation of the four
original sprite phases, not recovered original animation timing.

The selected asset is `../generated/goat-rigged-20261106/eighth/goat-rigged.glb`
with its editable `.blend`. `goat-multiview-review.mp4` shows four directions;
`goat-treadmill-review.mp4` shows three repetitions with a checker floor moving
at the authored stance speed. These are actual renders of the exported GLB.
Individual side, opposite-side, front, rear and three-quarter contact frames
were inspected. No goat asset has been installed in Unity in this follow-up.

Reproduce from the approved fresh Tripo mesh (not the old rejected prototype):

```sh
BLENDER=/home/oliver/Projects/fittimer-avatar-toolchain/blender-4.5.12-linux-x64/blender
"$BLENDER" -b -t 2 --python-exit-code 1 \
  -P research/private/character-pipeline/rig_goat.py -- \
  --model research/private/generated/goat-animation-20261106/mesh/model.glb \
  --output /absolute/new/goat-output
"$BLENDER" -b -t 2 --python-exit-code 1 \
  -P research/private/character-pipeline/verify_goat_rig.py -- \
  --model /absolute/new/goat-output/goat-rigged.glb \
  --output /absolute/new/goat-output/verification.json
"$BLENDER" -b -t 2 --python-exit-code 1 \
  -P research/private/character-pipeline/render_goat_treadmill.py -- \
  --model /absolute/new/goat-output/goat-rigged.glb \
  --output /absolute/new/goat-output/treadmill
```

An independent rebuild and GLB reimport passed: 17,070 vertices, 14 bones,
normalized weights, finite evaluated positions, zero sampled loop seam error,
and maximum sampled stance drift of `2.60e-7` model units relative to the floor.
Rigid-region sampled pair distances and bone lengths remain stable to floating
point precision. Coordinates are model units; the goat is roughly one unit tall.
These checks cover the authored flat-ground cycle, not terrain foot placement,
engine locomotion blending or every possible pose.

## Archer source and rigging

The original Day sprites show a bare torso, orange sash, pale loose trousers,
striped headwrap, brown shoes, back quiver and a **bow**. The low-resolution
weapon was initially misread as a crossbow; Oliver corrected that interpretation.
Do not use the rejected crossbow candidate. Owner-local source references are in
`../generated/archer-reference-20261106/`. Walk TMAP bases 8/9/10 have four phases;
11/12/13 and 121/122/123 are static standing and kneeling attack stances.
Modern draw/release transitions are authored additions. Source walk frame timing
depends on rendered TMAP demand and has not been established as a fixed FPS.

The body references use built-in image generation with those Day sheets.
Requests, receipts, images and meshes are owner-local under
`../generated/archer-modern-20261106/`. The first Tripo H3.1 body looked suitable
in static views (17,509 vertices / 28,928 triangles), but its low underarm bridge
and open hands required unsuccessful skin/curl repairs during animation.

The selected body is `meshy-grip/`: Meshy 7.1, 19,770 vertices / 25,307 triangles
and a 24-joint rig. Its input `archer-grip-reference.png` retains the costume and
face while providing a front T-pose, clear armpits and already curved grasping
hands. `grip-image-prompt.txt` preserves the exact built-in imagegen prompt.
This avoids the torn finger morphs in the earlier candidate. These are modeled
grips, not an articulated finger rig. The earlier Meshy open-hand T-pose is also
retained as a comparison. Both Meshy bodies deform more cleanly at the shoulders
than the Tripo candidate in the same unmodified Archery_Shot clip.

**Normalize and inspect the actual facing direction before auto-rigging.**
The first Meshy result was unusable: the +X-facing mesh received a skeleton
whose forward axis was -Y in Blender. Arm joints passed through the torso and
animated arm motion pulled the trousers upward. Ordered joint names, skin
weights and inverse-bind buffers were identical across the provider's static,
walking and archery exports; transferring their actions would not fix it.
`align_mesh.py --yaw-degrees -90` baked this particular mesh into the matching
facing direction. The second rig's overlay places arm and leg joints inside
their corresponding limbs. Keep both trials as evidence; the rotation is
specific to this observed input, not a universal rule for every generated mesh.

The selected generation uses `meshy/v7.1/image-to-3d`, with rigging and animation
enabled, action 224 (Archery_Shot), quad topology and the already posed input
(`pose_mode=""`). `meshy-grip-request.json` records the complete request. Basic
walk/run clips are returned too. The five-second supplied archery action is a
motion foundation, not a source-exact one-second shot. The earlier standalone
`fal-ai/meshy/rigging` trials also worked after facing correction, but were not
selected. Blender inspection exposes an Icosphere helper after rig import;
direct inspection of the provider and final GLB node lists confirms it is not
character geometry in those files.

The quiver needs rigid skinning while its strap follows the torso. The repair
shares its mask across coincident UV-seam vertices: a pair at the upper
quiver initially received Spine and RightShoulder weights respectively, creating
a long spike during aiming despite a satisfactory walking preview. Check both
poses after reimporting the exported GLB. `archer_props.py` shares the seam-aware
repair between walking and shooting.

The accepted walk is `animated/root-final-walk/walk.glb` under the archer output
directory, with editable `walk.blend`. This is root's independent rebuild of the
accepted `walk-meshy-grip6` source with the final bow. Its three-cycle video is
`animated/walk-final-review/treadmill-review.mp4`. `verify_archer_walk.py` reimports it and
checks 97 quarter-frame samples, finite geometry, normalized weights, actual
motion, the one-second exported interval, ground contact and three rigid-quiver
probes. Root independently measured a `1.55e-6` maximum vertex loop error and
`4.97e-7` quiver pair-distance variation. The supporting shoe is approximately
one millimetre above the review floor. This is sampled flat-ground evidence,
not a terrain-placement or continuous collision guarantee.

The preserved first walk fails the same checker on quiver rigidity (0.052 model
units of pair-distance variation) and export duration. The duration check also
caught the later corrected walk's NLA start adjustment trimming its interval;
moving both strip endpoints fixes the actual output instead of relaxing the gate.

The standing shot is `animated/final-standing3/standing.glb`, with editable
`standing.blend` and `motion-front.mp4` (three repetitions at 24 fps). Root
inspected held, full-draw, release and reset frames from front, profile and rear.
Independent `archer_contact.py` measurements after GLB reimport put the grip and
held nock within `1e-5` model units of their selected finger-region centroids; this is an
attachment check, not proof of finger anatomy. The string remains 1.16 model
units long at the six checked phases. The release separates string and drawing
hand deliberately. A STEP arrow-scale track prevents a partial arrow appearing
during the loop reset. Each shot clip spans one second; the prop deformation is
authored animation, not a simulated elastic bow.

The kneeling shot is `animated/grip-kneel1/kneel.glb` with editable `kneel.blend`.
`archer_kneel.py` uses fixed-length leg chains on this same rig to place the
front foot and rear knee at the floor. The relaxed rear toe remains above it.
Front, profile and rear aiming/release frames were inspected after GLB export;
root independently reran the hand/string and per-foot measurements. The original
sprites contain held attack stances; these loops do not reconstruct a full
historical draw, reload or stand-to-kneel sequence. No replacement character is
installed in Unity yet. Engine blending, terrain placement and crowd LODs remain
outside this asset-animation review.

`animated/archer-final-review.mp4` is the nine-second review: three walk cycles,
three standing shots and three kneeling shots, encoded from the inspected PNG
frames at 24 fps. Individual critical frames, numeric reports and rejected
candidates remain beside the selected assets. Python compilation and focused
UBS scans cover the twelve new archer source files. UBS's one JSON warning is a
fail-fast parse of the script's own freshly exported GLB before modifying its
arrow interpolation; malformed data raises rather than being ignored. No scan
establishes visual quality: that acceptance comes from the exported-model views.

Rebuild and check the walk from repository root with Blender 4.5.12 (set
`BLENDER` as in the goat example; use new output directories):

```sh
"$BLENDER" -b -t 2 --python-exit-code 1 \
  -P research/private/character-pipeline/archer_bow.py -- \
  --output /absolute/new/bow
"$BLENDER" -b -t 2 --python-exit-code 1 \
  -P research/private/character-pipeline/archer_walk.py -- \
  --model research/private/generated/archer-modern-20261106/meshy-grip/walking.glb \
  --bow /absolute/new/bow/bow.blend --output /absolute/new/walk
"$BLENDER" -b -t 2 --python-exit-code 1 \
  -P research/private/character-pipeline/verify_archer_walk.py -- \
  --model /absolute/new/walk/walk.glb --output /absolute/new/walk/check.json
"$BLENDER" -b -t 2 --python-exit-code 1 \
  -P research/private/character-pipeline/archer_preview.py -- \
  --model /absolute/new/walk/walk.glb --output /absolute/new/walk/review \
  --frames 1 7 13 19 25 --views 0 90 180 225
"$BLENDER" -b -t 2 --python-exit-code 1 \
  -P research/private/character-pipeline/archer_shot.py -- \
  --model research/private/generated/archer-modern-20261106/meshy-grip/animation.glb \
  --bow /absolute/new/bow/bow.blend --rigid-quiver --output /absolute/new/standing
# Add --kneel and use a separate output directory for the kneeling shot.
"$BLENDER" -b -t 2 --python-exit-code 1 \
  -P research/private/character-pipeline/archer_walk_preview.py -- \
  --model /absolute/new/walk/walk.glb --output /absolute/new/walk/treadmill
ffmpeg -n -framerate 24 -i /absolute/new/walk/treadmill/frame-%03d.png \
  -c:v libx264 -pix_fmt yuv420p /absolute/new/walk/treadmill.mp4
```

## Tools

- `run_actionmesh.py`: verify all pinned checkpoint hashes before loading the
  reconstruction model, and check them again before declaring completion.
  Accept 2–31 sampled PNGs, their sample rate, a new output
  directory, and either `--mesh` or `--native`. It preserves diagnostics on
  stderr, emits a versioned JSON result on stdout and records runtime/VRAM in
  `run.json`. Existing output directories are refused. A real damaged-checkpoint
  test exits 1 with a typed error before creating output or loading the model.
- `fal_job.py`: submit once and retain the queue receipt, resume polling or
  interrupted output downloads. Load `FAL_KEY` from the project `.env` without
  printing it. `--image-field start_image_url` supports Kling; the default
  `image_url` supports Wan and Tripo.
  Polling defaults to 30 minutes (`--timeout-seconds`). Timeout exits 75 with a
  JSON error and retains the receipt; rerunning resumes that job. Timeout does
  not itself cancel a provider job. HTTP operations also have network timeouts.
  Meshy rigging results also download the rigged GLB/FBX, optional animation,
  and nested basic walking/running GLBs. Each output resumes independently.
- `align_mesh.py`: bake an explicit yaw in world coordinates before motion
  fitting, independent of Blender's Euler/quaternion mode. It verifies every
  transformed vertex. **Do not assume image-view alignment is the model's
  learned canonical orientation.** The utility is mechanically verified; the
  camera-aligned trial with verified weights still failed the face/limb review.
- `export_deformation.py`: Blender conversion of ActionMesh's saved positions
  into linear morph targets on the original textured GLB. It checks the bind
  geometry before exporting and preserves UV seams and materials. Run with
  Blender 4.5.12, `--model`, `--vertices`, `--output`, and the actual sample `--fps`.
- `render_character.py`: independent Blender renders with fixed studio lights.
  Default: four angles at first/middle/last animation frames. `--animate --view 90`
  renders the full clip from a fixed side view. `--engine BLENDER_EEVEE_NEXT`
  enables the verified fast rendering path. Default Cycles provides a second
  rendering check.
  `--frames 1 16 31 46 61 76 91` checks seven phases from four directions.

Example (from repository root; substitute an actual animated model):

```sh
/home/oliver/Projects/fittimer-avatar-toolchain/blender-4.5.12-linux-x64/blender \
  -b -t 8 -P research/private/character-pipeline/render_character.py -- \
  --model /absolute/path/goat-walk.glb --output /absolute/path/review \
  --engine BLENDER_EEVEE_NEXT
```

## Local reconstruction setup

Third-party source: `/home/oliver/Projects/magiccarpet-character-tools/ActionMesh`
at `f1af2cd97f36fd05344f5e0ccbd8d0ddb94e60da`, with its pinned TripoSG submodule.
Weights and isolated Python package overlay live on the HDD at
`/mnt/pinokio-shared/magiccarpet-character-models/`.
The shared Comfy Python environment is read-only; no packages are installed into
it. The overlay uses its Python 3.12 / PyTorch 2.10.0+cu130.

The HDD is exFAT: use local-directory model downloads and copied packages.
uv's cache needs an ext4 location because it uses symlinks. The rig machine's
Tailscale SSH policy denied access; this experiment uses the authorized local
storage alternative. No RunPod instance has been requested.

Current overlay: diffusers 0.35.2, transformers 4.57.1, tokenizers 0.22.2,
huggingface-hub 0.36.0, numpy 1.26.4, scipy 1.15.3, scikit-image 0.25.2,
scikit-learn 1.6.1, hydra-core 1.3.2, omegaconf 2.3.0, antlr 4.9.3,
diso 0.1.4, plus inference utilities. PyTorch3D 0.7.9 was built from
`e4ad6e5687a360ccdc6ebd91348e8dbbac97bf31` with Clang for C++ and GCC 15
for the CUDA host compiler, CUDA 13.4,
`CUB_HOME=/usr/include/cccl`, `NVCC_FLAGS=-std=c++20` and architecture 8.6.
The C++20 override addresses the observed NVCC/host parsing failure in PyTorch's
dependent type expression. Clang avoids an observed GCC 15 internal compiler
crash in the Pulsar camera source. The shared PyTorch installation is not patched.

ActionMesh's FAIR Noncommercial Research License restricts its outputs too.
This experiment is a private method evaluation, not a commercial asset pipeline.
The skeletal goat above uses the approved Tripo mesh and Blender-authored motion;
it does not use ActionMesh output. AniGen and fixed-rig video fitting remain
possible future candidates, subject to real output tests.

## Reconstruct an approved motion reference

Before inference, run the existing checksum tool from the weights directory:

```sh
cd /mnt/pinokio-shared/magiccarpet-character-models/actionmesh-weights
sha256sum --check --strict /home/oliver/Projects/odcpw/magiccarpet/research/private/character-pipeline/actionmesh.sha256
```

Require exit 0 and all eight matches. The manifest pins publisher LFS SHA-256
values retrieved on 2026-11-06 from `facebook/ActionMesh`, `facebook/dinov2-large`,
`VAST-AI/TripoSG` and `briaai/RMBG-1.4`. It contains no model data. An existing
filename or a successful tensor load is insufficient: corrupted tensors can
remain finite and produce plausible but wrong geometry.

Extract 16–31 PNG frames spanning the desired motion at a known rate. The goat
experiment uses 31 samples at 8 Hz from the revised five-second Kling clip.
The upstream CLI silently ignores frames beyond 31; do not pass a full video
and assume it will sample the whole duration. Keep the first sample close to
the anchor mesh pose.

Set `PYTHONPATH` to the HDD Python overlay; the launcher adds the ActionMesh
checkout and its TripoSG submodule. Set `HF_HOME` and `TMPDIR` to their HDD
locations. Use the read-only Comfy environment's Python executable:

```sh
python research/private/character-pipeline/run_actionmesh.py \
  --repo /home/oliver/Projects/magiccarpet-character-tools/ActionMesh \
  --frames /absolute/path/frames --fps 8 --mesh /absolute/path/model.glb \
  --output /absolute/path/reconstruction --seed 20261106
```

The launcher selects the absolute upstream script and sets its working directory;
invoke the launcher from the project root. Use `--native` instead of `--mesh`
to generate the starting mesh from the video. It uses the normal upstream model
precision; a precision workaround does not repair corrupted weights.

Pass its `deformations_vertices.npy` and the **same anchor GLB** to
`export_deformation.py --fps 8`. The exporter checks the actual starting
geometry; it must not silently remap a different model or orientation.

## Initial reconstruction outcome

The fresh sprite → reference image → generated walking video → animated 3D
experiment produced actual GLBs, but **none passes final visual acceptance**.
It is implemented and self-verified as an experiment; no creature was installed
in Unity. Files below are relative to `../generated/goat-animation-20261106/`.

| Trial | Evidence | Visual result |
| --- | --- | --- |
| Fresh Tripo H3.1 mesh, original orientation; 31 poses | `goat-walk-verified.glb`, `goat-prototype-review.mp4`, `verified-review/` | Recognizable gait and stable body, but muzzle flattening and poor hoof contact |
| Same mesh, verified camera alignment; 16 poses | `goat-walk-verified-aligned.glb`, `verified-aligned-review/` | Face deformation persists; awkward foreleg crossing |
| Native video → mesh → motion; 16 poses | `goat-walk-native.glb`, `goat-native-review.mp4`, `native-review/` | Better head volume, but stretched/twisted limbs and weak hoof contact |

Blender independently rendered four angles at six or seven motion phases.
Important individual frames were inspected, not just a preview movie. The native
run passed all eight weight hashes **before and after inference**. It used
9.32 GiB peak allocated CUDA memory / 10.10 GiB reserved on the RTX 3090.
The textured clip contains 31 sampled poses at 8 Hz, linearly interpolated to
91 rendered frames at 24 fps. These are morph animations, not skeletal rigs.

The Tripo mesh has 17,070 vertices / 29,016 triangles; the native mesh has 19,946
vertices. Bind matching errors are `4.22e-8` and zero respectively. These checks
verify the exporter, not anatomical quality. The textured static model also has
a weak far-side eye and small neck/back ridges. No money was spent retexturing
the rejected native geometry.

The next candidates were AniGen's joint shape/skeleton/skinning generation and
video fitting, or a validated quadruped rig. The Blender follow-up above takes
the fixed-rig route after Oliver approved repairing this mesh. Require rigid skull
and hoof shape, plausible joint motion, foot contact, a clean loop and Unity
skeletal playback before processing the roster. Check the same boundaries on a
humanoid archer with bow/string/arrow contact. More VRAM is not the demonstrated
bottleneck in these trials; the local cache-integrity anomaly below must also
be understood before unattended bulk generation.

The first Kling in-place clip was rejected for isolated leg lifts; the revised
forward-walking clip supplied these trials. The original Wan request stalled
and cancellation was requested, but it eventually completed. Its existing
result was downloaded without resubmission. Ten sampled frames in
`wan-contact.png` show mostly standing and isolated foreleg lifts; it was not
selected as the walking reference. All provider receipts and results are retained.

## Integrity findings

**The initial animation trials used a damaged DINOv2 checkpoint.** Both local
copies hashed to `b10df4d6666d97b144388c77711c1088d13b0fc042a0127180cb52dc354b5a66`,
whereas the publisher's LFS SHA-256 is
`399fba97a95f22c36834418bc69373364a99af3a1153da1c0fb31db567c92e23`.
One weight in `encoder.layer.9.mlp.fc2.weight` had magnitude `1.038e18`.
Consequently, the initial motion and alignment trials are rejected outputs,
**not valid benchmarks of ActionMesh quality**. Preserve them for diagnosis.
Native half-precision generation also produced nonfinite latents; full precision
avoided overflow but still used the damaged encoder, so that run was stopped.
Validate checkpoint bytes against publisher hashes before another quality trial.

Both damaged encoder copies were preserved with `.corrupt-20261106` suffixes,
then replaced with a fresh download and verified after copying. The unused RMBG
`model.pth` also differed from its publisher hash and was preserved/replaced;
the active RMBG safetensors, both ActionMesh weights and the other TripoSG
weights matched. Direct disk reads of the preserved original DINOv2 and RMBG
files confirmed those mismatches. No third-party source precision patch is
required with the repaired weights.

Later, ordinary cached reads of the previously verified ActionMesh denoiser
returned `5da84053d903f2dbef53b28383bcb312914dd53bfb47c5d85c07a2a9d5217fd8` twice,
while `dd iflag=direct` returned the correct publisher hash. File-specific
`POSIX_FADV_DONTNEED` restored the correct ordinary-read hash. No system-wide
cache drop, file deletion or unrelated workload interruption was performed.
The cause is **unresolved**; this does not diagnose the disk, RAM or loader.
The two external-mesh trials had preflight checks only. The final native trial
has before/after checks, added in response to this observed defect. Preserve
the `*-hash*`, `*-integrity*` and `denoiser-cache-refresh.json` evidence.

Tool self-verification includes Python parsing/compilation, completed FAL receipt
resumption without resubmission, Blender bind/export/import checks and refusal of
the **actual preserved corrupt checkpoint** before output creation. UBS scanned
the five Python sources. Its checksum-comparison critical is a false positive:
these SHA-256 values are public download identifiers, not secrets. FAL URL warnings
are covered by explicit HTTPS/host checks; JSON parse failures reach the typed
top-level error handler. Informational style findings were not treated as defects.

Sources: [ActionMesh](https://github.com/facebookresearch/actionmesh),
[Tripo H3.1 API](https://fal.ai/models/tripo3d/h3.1/image-to-3d/api),
[Kling 4 API](https://fal.ai/models/kling/v4/image-to-video/api),
[AniGen](https://github.com/VAST-AI-Research/AniGen).
