# VR-B PC OpenXR smoke profile

This is an owner-free toolchain proof, not gameplay and not headset support.
The flat profile remains the default and does not initialize an XR loader or
create an XR Origin. The PC profile initializes the selected OpenXR loader,
creates a diagnostic Unity `XROrigin` with tracked-head/left-controller/right-
controller placeholders, reports loader/display/input-device counts, and
tears the loader down deterministically. The lab pins XR Core Utils 2.6.0
explicitly because it owns the `XROrigin` component.

Run the static profile/action-map guard from the repository root:

```text
python3 research/private/remc2-unity-presentation-lab/qualification/verify_xr_smoke.py
python3 -m unittest discover \
  -s research/private/remc2-unity-presentation-lab/qualification/tests \
  -p 'test_xr_smoke.py' -v
```

The Unity editor asset and flat player smoke commands are:

```text
Unity -batchmode -nographics -quit \
  -projectPath research/private/remc2-unity-presentation-lab \
  -executeMethod MagicCarpet.PresentationLab.Editor.CreatePresentationLabAssets

Unity -batchmode -nographics -quit \
  -projectPath research/private/remc2-unity-presentation-lab \
  -buildLinux64Player /tmp/mc2-presentation-lab-flat \
  -mc-xr-profile=off
```

The owner-free smoke scene has a dedicated build method so a player actually
starts in `OpenXrSmoke.unity` rather than merely packaging it as a secondary
scene. The active Unity standalone target must be `StandaloneLinux64` or
`StandaloneWindows64`:

```text
Unity -batchmode -nographics -quit \
  -projectPath research/private/remc2-unity-presentation-lab \
  -executeMethod MagicCarpet.PresentationLab.Editor.CreatePresentationLabAssets.BuildOpenXrSmokePlayer \
  -mc-build-path /tmp/mc2-presentation-lab-xr-smoke \
  -mc-xr-profile=pc-openxr
```

Run the produced player with `-mc-xr-profile=off -mc-xr-smoke-once` to prove
the flat path exits without an XR loader, then with
`-mc-xr-profile=pc-openxr -mc-xr-smoke-once`. The PC run logs the selected
loader, display subsystem count, headset count, left/right controller counts,
and per-device identities, then requests shutdown so `OnDestroy` stops and
deinitializes the loader. Missing SteamVR/OpenXR or missing left/right
controllers is a non-silent `XR UNAVAILABLE`/`XR CONTROLLERS UNAVAILABLE`
diagnostic with a non-zero smoke exit. XR initialization does not open a Rust
session, load owner data, or write gameplay state.
