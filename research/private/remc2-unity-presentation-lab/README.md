# Unity presentation lab

This is the isolated Unity project for the optional `classic-hd` and
`unity-enhanced` presentation modes. It is deliberately separate from
`remc2-unity-native`, which remains the sealed Exact project.

The lab now provides the safe shell and the first player-facing Classic HD
route for renderer work:

- Unity `6000.3.20f1` with URP `17.3.0`.
- A `Boot` scene with a pre-session mode router.
- External owner-input validation for package, closure, and asset-cache paths.
- A diagnostic `NO OWNER INPUTS` boot that never opens a session.
- A deterministic presentation-session lease that does not own gameplay or
  load owner payloads.
- A source-style Classic HD route with a persistent dimensioned R8 index
  surface, one top-row-to-Unity-row upload per frame, resolved night palette,
  point/bilinear output filtering, and a centered 4:3 display rectangle.
- A size-queried `sim-capi` binding contract whose native calls remain
  optional; the lab fallback is a clearly synthetic, owner-free Level 1 Night
  frame source until the private native plugin is supplied.
- JSON settings outside owner data with fail-closed schema/profile validation.
- No copied owner assets, native libraries, Exact scripts, or Exact corpus.

Run the editor setup once after opening the project:

```text
Unity -batchmode -nographics -quit \
  -projectPath research/private/remc2-unity-presentation-lab \
  -executeMethod MagicCarpet.PresentationLab.Editor.CreatePresentationLabAssets.CreateRenderPipelineAssets \
  -logFile /tmp/magiccarpet-presentation-lab-setup.log
```

The runtime accepts `-mc-presentation-mode classic-hd` or
`-mc-presentation-mode unity-enhanced`. The owner inputs remain external:

```text
-mc-package /absolute/path/to/level1-export-m \
-mc-closure /absolute/path/to/level1-asset-closure.json \
-mc-asset-cache /absolute/path/to/level1-unity-asset-cache
```

Classic HD defaults to internal `640x480` and can be overridden before scene
assembly with `-mc-settings /absolute/path/to/classic-hd-settings.json` or
the equivalent flags (`-mc-classic-profile`, `-mc-classic-asset-tier`,
`-mc-classic-view-distance`, `-mc-classic-reflections`,
`-mc-classic-dynamic-lighting`, `-mc-classic-blur`, `-mc-ui-scale`,
`-mc-fullscreen`, `-mc-window-width`, `-mc-window-height`, and
`-mc-output-filter`). The settings file lives under Unity's
`Application.persistentDataPath` when no explicit path is given; it never
uses the owner package, closure, or asset cache as a settings store.

The private three-mode launcher is
`launcher/presentation_launcher.py`. Its checked-in config defaults to the
sealed Exact Built-in project, emits no mode switch for Exact, and points
Classic HD and Unity Enhanced at this isolated lab only when all three owner
inputs are supplied explicitly:

```text
python3 launcher/presentation_launcher.py --validate
python3 launcher/presentation_launcher.py --mode classic-hd \
  --owner-package /absolute/path/to/level1-export-m \
  --owner-closure /absolute/path/to/level1-asset-closure.json \
  --owner-cache /absolute/path/to/level1-unity-asset-cache
```

The lab does not accept `exact`; Exact stays in the protected project. Mode
selection happens before a presentation session opens, and there is no
mid-session pipeline swap.

Verify the project boundary and URP assignment with:

```text
python3 research/private/remc2-unity-presentation-lab/qualification/verify_presentation_lab.py
python3 -m unittest discover \
  -s research/private/remc2-unity-presentation-lab/qualification/tests \
  -p 'test_*.py' -v
python3 -m unittest discover -s launcher/tests -p 'test_*.py' -v
```
