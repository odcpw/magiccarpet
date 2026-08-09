# Exact presentation baseline guard

`verify_exact_baseline.py` is the fast pre-flight lock for the preservation
renderer. It protects the existing Built-in Unity project, the Rust indexed
FINAL MAIN path, the native ABI v2 layouts, the Level 1 readiness pins, and
the accepted qualification corpus before any Classic HD or Unity Enhanced
work is claimed.

The pinned source identity and submodule gitlink are recorded in
`exact-baseline-manifest.json`. The baseline is the accepted source commit
`c21df2e7495bdd7e4a5fe543a8dfa18e33ab4b17` and remc2 submodule commit
`6806c7204d17220ecc0430759f784aea8ee2ddb2`. The Exact profile is Unity
`6000.3.20f1`, a Rust-owned R8 indexed 320x200 world FINAL MAIN, and the
corrected 640x480 4:3 HUD/display contract.

Run the cheap guard from the repository root:

```bash
python3 research/private/remc2-unity-native/qualification/verify_exact_baseline.py
python3 research/private/remc2-unity-native/qualification/verify_exact_baseline.py --json
```

The guard compares protected paths with the pinned committed tree, so it is
safe to run while another worker has unrelated dirty files. It reports those
changes as drift; it never restores, stages, or overwrites them. The native
header is checked semantically: removal or signature changes to existing ABI
v2 functions fail, while genuinely additive declarations are reported as
additive symbols and must still be implemented through a later isolated ABI
bead.

Run the focused synthetic-drift tests with:

```bash
python3 -m unittest discover \
  -s research/private/remc2-unity-native/qualification/tests \
  -p 'test_verify_exact_baseline.py' -v
```

The accepted rebuild route remains the existing private route:

```bash
cd research/private/remc2-sim-rs
cargo build -p sim-capi --release --locked
install -Dm755 target/release/libsim_capi.so \
  ../remc2-unity-native/Assets/Plugins/x86_64/libsim_capi.so
```

Then rebuild the Exact Linux player with the Unity `6000.3.20f1` command in
the Unity project README and rerun the prescribed qualification gate. If a
candidate changes a protected Exact file, preserve the worker's branch and
return to the pinned source identity with a fresh isolated worktree rather
than restoring files in the shared checkout. The full 75–90+ minute gate is
reserved for integration milestones; the guard and its focused tests are the
normal pre-claim check.
