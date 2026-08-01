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

Validate the schema and a selected invocation:

```bash
python3 research/private/remc2-presentation/verify_mode_contract.py
python3 research/private/remc2-presentation/verify_mode_contract.py \
  --mode classic-hd --internal-width 640 --internal-height 480
python3 research/private/remc2-presentation/verify_mode_contract.py \
  --mode unity-enhanced --exact-claim
```

The last command is intentionally a negative check and must fail.
