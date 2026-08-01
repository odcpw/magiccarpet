# Private MC2 OpenXR gate

This directory is the Track P execution record for `mc-dl0r.19.1`. It is
separate from `docs/future-native-vr.md`, which is a Track S successor vision.
The private path uses the Rust-authoritative MC2 session and the isolated
Unity presentation lab; it does not alter the sealed Exact project.

The machine-readable source of truth is
[`architecture-gate.json`](architecture-gate.json). The accompanying Markdown
record explains the decisions and current evidence. Run the focused validator
from the repository root:

```text
python3 research/private/remc2-vr/validate_architecture_gate.py --json
python3 -m unittest discover -s research/private/remc2-vr/tests -p 'test_*.py'
```

The gate is intentionally allowed to pass while the hardware status is
`provisional`. It does not convert an unconnected headset into a support
claim. PC SteamVR/OpenXR on `legion` is the first runtime candidate; a headset,
controller pair, Unity editor resolution, and a real player smoke build are
still required before the PC profile becomes admitted. Quest 3 is next, and
Quest 2 remains an optional measured compatibility profile.

The Exact project is checked by a negative scan: no OpenXR/XR Plug-in
Management/XR Origin package or object may enter
`research/private/remc2-unity-native`. Built-in Unity module names such as
`com.unity.modules.vr` and `com.unity.modules.xr` are not OpenXR package
admission.
