# Private presentation-mode contract

This directory owns the shared contract consumed by the Exact launcher,
Classic HD, Unity Enhanced, corpus tooling, and later qualification beads.
It is private Track P metadata; it does not contain reconstructed owner assets
or a player payload.

The three modes are mutually exclusive presentation sessions over the same
Rust simulation:

- `exact` keeps the existing Built-in Unity project, fixed 320x200 R8 indexed
  FINAL MAIN, corrected 4:3 display, and Exact qualification claims.
- `classic-hd` is a true higher-resolution indexed rerender using the remc2
  HD shape. It is not a scale operation over the Exact FINAL MAIN. v1 keeps a
  4:3 internal game viewport and pillarboxes widescreen windows.
- `unity-enhanced` is the separate URP project for modern lighting, terrain,
  water, shadows, atmosphere, VFX, and post-processing. It is non-exact and
  owns no gameplay authority.

Mode selection occurs before the native session and Unity scene are assembled.
Changing mode requires a restart. There is no mid-frame Built-in/URP switch.
Invalid mode values, missing required owner inputs, Exact claims from a
non-Exact mode, corpus crossover, and mode-off are failures.

Level 1 Night visual acceptance is defined in
`quality/level1-night-quality-target.md`. The separate `corpora/` roots hold
dry-run manifests for Exact, Classic HD, and Unity Enhanced;
`capture_manifest.py` embeds mode/profile/build metadata, refuses a
mode/root crossover, and never presents a placeholder as captured evidence.

Validate the schema and a selected invocation:

```bash
python3 research/private/remc2-presentation/verify_mode_contract.py
python3 research/private/remc2-presentation/verify_mode_contract.py \
  --mode classic-hd --internal-width 640 --internal-height 480
python3 research/private/remc2-presentation/verify_mode_contract.py \
  --mode unity-enhanced --exact-claim
```

The last command is intentionally a negative check and must fail.

Corpus checks:

```bash
python3 research/private/remc2-presentation/verify_presentation_corpora.py
python3 -m unittest discover \
  -s research/private/remc2-presentation/tests -p 'test_*.py' -v
```

Classic HD resolution evidence is kept separate from the Unity Enhanced
visual systems in `qualification/classic-hd-resolution-matrix.json`. Validate
the current owner-free matrix with:

```bash
python3 research/private/remc2-presentation/qualification/validate_classic_hd_matrix.py
python3 research/private/remc2-presentation/qualification/verify_three_mode_package.py
```

This matrix promotes 640x480 as the mandatory real-time-oriented Rust and
owner-route row, records 960x720 as an explicit owner-budget rejection, and
records 1280x960 as over the current CPU budget. After streaming the large
mode-26 terrain pass and reusing the generation-bound owner asset digest, the
8+64 owner route measures 31.65 ms p95 total at 640x480 and 56.52 ms at
960x720, with zero measured steady allocation and deterministic reset hashes.
The 960x720 owner route still has a passing 64-frame reset-replay/upload
artifact, but its measured full-route cost is not a real-time or player-facing
production claim. The owner flight route at both profiles reaches the observed
speed limit of 80; the owner route is qualified, but it does not by itself
prove remc2's automatic high-speed postprocess. The explicit two-profile probe
records that the effective mode remains 0 because no legitimate Speed-jar
activation is exposed by the current owner ABI:
`qualification/classic-hd-owner-high-speed-probe.json`. Unity GPU, scenario,
high-speed-postprocess, and preserved corpus evidence remain required for any
broader promotion.

The owner-lab performance and GPU receipts are separate from the owner-free
Rust matrix. `qualification/classic-hd-owner-performance-smoke.json` records
the supported 640x480 row and explicit 960x720 rejection, and proves steady
allocations and deterministic 8+64-frame routes. The GPU receipt
`qualification/classic-hd-owner-gpu-fence.json` proves changing
raw indexed and palette-resolved readbacks with no readback errors; this T14
OpenGLCore platform cannot report the async graphics fence and GPU timing is
not captured.

`qualification/verify_three_mode_package.py` is the final package-integrity
gate. A PASS means the Exact, Classic HD, and Unity Enhanced contracts and
their current evidence are internally consistent. Its output separately lists
`OPEN` gates; placeholder visual corpora, GPU timing/fence capture, and human
visual review must not be promoted by this command.
