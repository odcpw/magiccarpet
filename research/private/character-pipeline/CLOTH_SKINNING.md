# Villager 241 lower-cloth bake

This is the reproducible source path for the accepted **pre-packaging** draft
`generated/villager-lower-cloth-20261110/death-player03/master-death.blend`
(SHA-256 `d0c2c5c30ecaf1f5a277c08d6482422f5e12c9db0c3f729c2f40c5407a1f9e0e`).
It has not been installed. Footwear masking, the Run ground-support correction,
upper garment preparation, materials, FBX and Unity/native verification belong to
the integration step. Do not modify this master in place.

The raw physics was not periodic. The Walk and Run inputs below are *authored
and contact-corrected target caches*, not a claim of converged cloth. Idle has
an explicit 16-frame seam correction; Hit is one-shot. Death has the reviewed
floor-aware waist correction and 119 actual settle frames after its body stops.
The raw, target, seed, fit and playback files are owner-local ignored data;
retain them with the source. This recipe does not regenerate those artistic
Walk/Run targets from their raw simulations.

The cloth fits share 1,920 rest vertices, 3,712 triangles, 48 deform bones,
and one set of nonnegative normalized four-influence weights. Arrays use
Blender world metres, identity object/rig transforms, +Z up, and column-vector
rigid matrices from rest positions to posed positions. The master keeps the
original editable body and rig, 162 drivers, and muted NLA. Action ranges at
120 fps: Idle 1–121, Walk 1–121, Run 1–77, Hit 1–61, Death 1–264. Body
geometry genuinely holds its Death frame-145 pose through frame 264.

## Inputs

Paths are relative to `research/private/generated/`. Verify SHA-256 before
using the exact approved data; do not add these owner-local files to Git.

| Input | SHA-256 |
| --- | --- |
| `villager-dressed-20261109/mpfb-counter02/dressed-master.blend` | `7c2f11d5e8c0b9107d4c1ee2630e57deb42a37d87dcc15a4289246ee7aa5e30d` |
| `villager-dressed-20261109/death-supported03/death-supported.blend` | `95e092fe568407b50b7e2a8a1c32afb0ab5b10c92f8f05fb08db5e8b321c50cf` |
| `villager-lower-cloth-20261110/trial16/cloth-cache.npz` | `8894326a4edcc19ddd9e75903472ef72e81816285c7c8f10b21dfcbb1f3acc3e` |
| `villager-lower-cloth-20261110/run-raw120/cloth-cache.npz` | `e5e2534eb2d583df8d883fded5d5de395dd0b73442345f3e08b0ad78e90ec10a` |
| `villager-lower-cloth-20261110/walk-loop09/periodic-target.npz` | `f652779e9b612eafb8c9939156cf883dab3d3f0a2ccc085d21095fefa02e7ba2` |
| `villager-lower-cloth-20261110/run120-loop02/periodic-target.npz` | `fb2fca80d6cd5c9c57112b5304b365049543f5793946e6198111a565568a30fa` |
| `villager-dressed-20261109/finite-idle02/cloth-cache.npz` | `1818a6c98e41a1c71c089e32f714ab8b9c104acf9be5daeb835a1d22b381b7cb` |
| `villager-dressed-20261109/finite-hit02/cloth-cache.npz` | `19a536ce44b5568ca264f4d4b88dc398e995080c2edc77208ff91386c266296c` |
| `villager-dressed-20261109/finite-death03/cloth-cache.npz` | `65d8e46f1fa69479d97f6649e959ccc620c72eb03bfcba53dec558c0db4058cc` |
| `villager-lower-cloth-20261110/joint48-seeded/Walk.npz` | `2ded4cb1009b637a8863e16af1988b219d440340a57eecf6c6c523d68fb4ef6c` |
| `villager-lower-cloth-20261110/run-polish01/skinning.npz` | `68a27d02bdbaa06afc9ea63752b825f38c0c2f4ade11d85245a401ec88db54f5` |
| `villager-lower-cloth-20261110/idle-polish02/skinning.npz` | `088d29f75dd8e98abff085d138e58d127fbf3bafdac893922736f76eb4b58ece` |
| `villager-lower-cloth-20261110/hit-polish02/skinning.npz` | `330f0d56b50c903db68cfa683d889099f34694cb6d9d8111c35fffb95c791f0e` |
| `villager-lower-cloth-20261110/death-local01/skinning.npz` | `f7ec6da03170147cbafae9cce007caf80554402091e07b13c3124fc02717c07d` |

The isolated solve used CPython 3.12.16, `py-dem-bones==0.13.1` and
`numpy==2.5.4`; Blender was 4.5.12 LTS. A fresh isolated environment can be
created with `uv venv --python 3.12 PATH` and
`uv pip install --python PATH/bin/python py-dem-bones==0.13.1 numpy==2.5.4`.
Do not globally install the solver. Blender uses its own bundled NumPy.

## Rebuild the approved draft

Run from the repository root. `OUT` must not exist. `--python-exit-code 2`
makes script failures visible to automation. The guarded Death merge is owned
by `villager_cloth.py` and checks the source rig and donor action fingerprints.

```sh
P=research/private/generated/villager-lower-cloth-20261110
D=research/private/generated/villager-dressed-20261109
S=research/private/character-pipeline
BLENDER=/home/oliver/Projects/fittimer-avatar-toolchain/blender-4.5.12-linux-x64/blender
OUT=research/private/generated/villager-lower-cloth-20261110/my-fresh-rebuild

"$BLENDER" -b --python-exit-code 2 --python "$S/cloth_bake.py" -- \
  --model "$D/mpfb-counter02/dressed-master.blend" --output "$OUT/four.blend" \
  --clip "Walk=$P/joint48-seeded/Walk.npz" --clip "Run=$P/run-polish01/skinning.npz" \
  --clip "Idle=$P/idle-polish02/skinning.npz" --clip "Hit=$P/hit-polish02/skinning.npz"
"$BLENDER" -b --python-exit-code 2 --python "$S/villager_cloth.py" -- \
  --merge-death --model "$OUT/four.blend" \
  --donor "$D/death-supported03/death-supported.blend" --output "$OUT/death-body.blend"
"$BLENDER" -b --python-exit-code 2 --python "$S/cloth_bake.py" -- \
  --model "$OUT/death-body.blend" --output "$OUT/five.blend" \
  --clip "Death=$P/death-local01/skinning.npz" --capture "$OUT/death-playback.npz"
```

The binder preserves the rig rather than clearing animation data. It checks
rig space, drivers, NLA, topology, common weights, bind matrices, proper
rotations and quaternion loop continuity, and refuses output overwrites.
`--capture` samples the actual evaluated Blender mesh, not predicted arrays.
In the source reproduction at `repro-20261110/`, all 644 source-time body and
cloth samples across the five actions matched the approved draft exactly on
this Blender build. This is not a claim of byte-identical `.blend` files or of
Unity/gameplay conformance.

## Refit or reproduce the corrections

`cloth_skinning.py --help` exposes the solver and `--extract-finite` mappings.
For each finite cache use `--cache CACHE --fixed "$P/joint48-seeded/Walk.npz"
--extract-finite --clip Idle|Hit|Death --output FRESH_DIR`. Death includes the
actual hold. To fit one finite target use `--target TARGET --fixed
"$P/joint48-seeded/Walk.npz" --finite --clip ACTION --transform-seed
"$P/joint48-seeded/Walk.npz" --iterations 100 --output FRESH_DIR`. Run was fit
at all 77 frames using `--target "$P/run120-loop02/periodic-target.npz"
--fixed "$P/joint48-seeded/Walk.npz" --key-step 1 --transform-seed
"$P/joint48-seeded/Run.npz" --iterations 100`. The joint Walk/Run shared
weights are produced by `--target WALK_TARGET --joint-run RUN_TARGET --bones
48 --seed WALK_SEED --seed-run RUN_SEED --iterations N`; a new solve must be
visually and geometrically requalified, not assumed to reproduce the accepted
weights across platforms. The retained seed files are `walk-bone48/skinning.npz`
(`eea49984318e3e71c24b8ee3894a7334ad71225405ee69aaf46884bf83f6e93e`) and
`run-fixed48/skinning.npz` (`8c23c111306989469d73d15be17bc4f1816a8737fcd2b0bfdbbfbe254425d393`).

For an actual Blender playback capture, bake one action with `--capture` as
above. Recompute the Run or standing contact correction with `cloth_bake.py
--model BODY_BLEND --contact-fit FIT --playback ACTUAL_NPZ --action ACTION
--output FRESH_DIR`. For the second, localized standing pass use the original
fit/playback plus `--previous FIRST_PASS/skinning.npz --local`. For Death use
`cloth_skinning.py --target FIT --playback ACTUAL_NPZ --correction floor
--output FRESH_DIR`, bake/capture that result, then repeat with `--correction
exposure`. The latter is a deliberately bounded lift of the specific
fit-induced left-leg exposure. Corrections operate on bone translations;
rotations and shared weights stay fixed. Do not infer contact cleanliness from
solver convergence: audit and render actual Blender playback, including
fractional frames, before using a new result.

For the accepted draft, actual Blender checked all 120 Hz frames and 240 Hz
midpoints. Death cloth floor minima were +0.490 mm and +0.343 mm respectively;
small covered-body/self intersections remain. Death fit error after corrections
was 17.76 mm RMS and 122.38 mm maximum against the authored target. Actual
Unity verification of the earlier common Walk/Run master passed; it does not
prove this five-action draft or the subsequent footwear changes.
