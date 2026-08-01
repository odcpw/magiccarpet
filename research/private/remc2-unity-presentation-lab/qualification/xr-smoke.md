# VR-B PC OpenXR smoke profile

This is an owner-free toolchain proof, not gameplay and not headset support.
The flat profile remains the default and does not initialize an XR loader or
create an XR Origin. The PC profile initializes the selected OpenXR loader,
creates a diagnostic `XR Origin` with tracked-head/left-controller/right-
controller placeholders, reports loader/display/input-device counts, and
tears the loader down deterministically.

Run the static profile/action-map guard from the repository root:

```text
python3 research/private/remc2-unity-presentation-lab/qualification/verify_xr_smoke.py
python3 -m unittest discover \
  -s research/private/remc2-unity-presentation-lab/qualification/tests \
  -p 'test_xr_smoke.py' -v
```

The Unity editor smoke commands are:

```text
Unity -batchmode -nographics -quit \
  -projectPath research/private/remc2-unity-presentation-lab \
  -executeMethod MagicCarpet.PresentationLab.Editor.CreatePresentationLabAssets

Unity -batchmode -nographics -quit \
  -projectPath research/private/remc2-unity-presentation-lab \
  -buildLinux64Player /tmp/mc2-presentation-lab-flat \
  -mc-xr-profile=off
```

The `pc-openxr` profile is run from the editor/player after the same project
has been built with that profile. Missing SteamVR/OpenXR or missing controllers
must be a clear `XR UNAVAILABLE`/identity diagnostic, never a silent flat-mode
fallback that looks like a pass. XR initialization does not open a Rust session,
load owner data, or write gameplay state.
