# Character animation experiments

Fresh sprite-informed goat experiment: `../generated/goat-animation-20261106/`.
Original sprites inform identity; no rejected prototype mesh, rig or motion is
reused. Generated files, original assets and credentials remain owner-local.

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
