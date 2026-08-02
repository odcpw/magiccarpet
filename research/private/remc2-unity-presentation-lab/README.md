# Unity presentation lab

This is the isolated Unity project for the optional `classic-hd` and
`unity-enhanced` presentation modes. It is deliberately separate from
`remc2-unity-native`, which remains the sealed Exact project.

The lab now provides the safe shell and the owner-bound Classic HD renderer
binding under qualification:

- Unity `6000.3.20f1` with URP `17.3.0`.
- A `Boot` scene with a pre-session mode router.
- External owner-input validation for package, closure, and asset-cache paths.
- A diagnostic `NO OWNER INPUTS` boot that never opens a session.
- A deterministic presentation-session lease that does not own gameplay or
  load owner payloads.
- A source-style Classic HD route with a persistent dimensioned R8 index
  surface, one top-row-to-Unity-row upload per frame, resolved night palette,
  point/bilinear output filtering, and a centered 4:3 display rectangle.
- A size-queried `sim-capi` binding contract. The player-facing Classic HD
  route opens the external owner package, closure, and cache through the Rust
  native session and calls the additive profile-sized Classic HD rerender
  endpoint directly into the persistent HD R8 surface. It never enlarges the
  320x200 Exact FINAL MAIN. The route fails closed when the native plugin is
  absent or when a source sprite family has not yet received a profile-sized
  adapter; the current Level 1 owner smoke records that deliberate blocker.
  The additive Classic HD ABI fixture remains available for contract tests,
  while the direct component `Configure` overload remains a clearly
  synthetic, owner-free Level 1 Night source for EditMode tests.
- JSON settings outside owner data with fail-closed schema/profile validation.
- A procedural Unity Enhanced stack over the same snapshot boundary:
  authoritative height/type/shade/angle terrain planes with generated normals
  and dirty-region updates; pooled generated visuals for the complete Level 1
  Night actor/family coverage; descriptor-driven moon/fog/local-light caps;
  procedural water and shoreline/reflection fallback; ordered replayable spell,
  mana, destruction, weather, and speed VFX; and URP HDR/volume/post controls.
- Enhanced quality tiers `low`, `medium`, `high`, and `whole-hog`, reduced
  motion controls, frame/pool/drop diagnostics, and explicit spatial render
  scale choices (`1.0`, `0.85`, `0.75`). Temporal upscaling is gated off by
  default and cannot be enabled accidentally by a quality preset.
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

The owner-free Enhanced systems have a focused Unity EditMode gate. Because
the test folder is compiled into the editor assembly, include that assembly in
the command:

```text
Unity -batchmode -nographics \
  -projectPath research/private/remc2-unity-presentation-lab \
  -runTests -testPlatform EditMode -assemblyNames Assembly-CSharp-Editor \
  -testResults /tmp/magiccarpet-enhanced-editmode.xml \
  -logFile /tmp/magiccarpet-enhanced-editmode.log
```

The current owner-free check covers terrain, actor pooling, Level 1 Night
lighting caps, water/shoreline geometry, VFX replay/reduced motion, gated
temporal upscaling, and path-free diagnostics. It is not a substitute for the
owner-bound Unity GPU/copy and preserved Exact qualification gates.
