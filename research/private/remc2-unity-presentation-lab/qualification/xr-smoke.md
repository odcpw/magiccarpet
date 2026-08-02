# VR-B PC OpenXR smoke profile

This is an owner-free toolchain proof, not gameplay and not headset support.
The flat profile remains the default and does not initialize an XR loader or
create an XR Origin. The PC profile initializes the selected OpenXR loader,
creates a diagnostic Unity `XROrigin` with tracked-head/left-controller/right-
controller placeholders, reports loader/display/input-device counts, and
tears the loader down deterministically. The lab pins XR Core Utils 2.6.0
explicitly because it owns the `XROrigin` component.

## Reproducibility, licensing, cleanup, and rollback

The reproducibility tuple is versioned in the lab rather than inferred from a
machine cache:

- Unity `6000.3.20f1`, revision `c9ba695d4f07`, from
  `ProjectSettings/ProjectVersion.txt`.
- Resolved packages are pinned by `Packages/packages-lock.json`: Input System
  `1.14.2`, XR Core Utils `2.6.0`, XR Interaction Toolkit `3.0.9`, XR Plug-in
  Management `4.7.0`, and OpenXR `1.17.1`.
- The Windows gate uses Unity's `windowsstandalonesupport` Win64 Mono player
  variation; the Linux gate uses the matching `StandaloneLinux64` target.

Unity Editor activation is a host prerequisite, not a repository artifact.
Never commit, copy into the lab, or put in a qualification result an `.alf`,
`.ulf`, serial, password, Hub token, or Unity licensing cache. A missing
Windows license is a harness/build failure (the Editor returns 198 and the
PowerShell wrapper returns 1); it must not be misreported as an XR runtime
failure. The lab itself remains private and owner-free.

Generated state is disposable and must stay outside source review. After
preserving the requested logs and `qualification-result.json`, verify that the
paths point into the temporary staging project/output directory, then remove
only those explicit paths, for example:

```powershell
Remove-Item -LiteralPath $OutputRoot -Recurse -Force
Remove-Item -LiteralPath "$ProjectPath\Library", "$ProjectPath\Temp", "$ProjectPath\Logs" -Recurse -Force -ErrorAction SilentlyContinue
```

Do not run those cleanup commands against the canonical checkout or another
agent's worktree. To uninstall the host toolchain, remove the Editor from the
Unity Hub Installs view, then remove the unique temporary staging directory;
the private source worktree and its package lock remain intact.

Rollback is deliberately local: discard or delete only the isolated
`codex/vr-b` worktree/branch, remove the named temporary staging/output paths,
and restore the pre-VR presentation-lab commit if a review rejects the slice.
The Exact project, Rust authority, owner data, and active Level 2/GFX
worktrees are never rollback targets for this bead.

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

For the Legion/Windows gate, run the checked-in PowerShell harness from a
logged-in Windows desktop session with the headset and both controllers
connected. The PC invocation intentionally keeps graphics enabled; a
headless Tailscale SSH session is suitable for inspection and staging, not for
claiming physical XR startup:

```powershell
PowerShell -ExecutionPolicy Bypass -File .\qualification\run_xr_smoke.ps1 `
  -ProjectPath .\research\private\remc2-unity-presentation-lab
```

Exit code `0` means active OpenXR plus left/right controller identities and
clean loader shutdown. Exit code `2` is a captured runtime/controller
diagnostic; exit code `1` is a harness/build error. The output directory holds
`build.log`, `flat.log`, `pc-openxr.log`, and `qualification-result.json`.
