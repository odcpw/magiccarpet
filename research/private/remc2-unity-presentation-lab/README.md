# Unity presentation lab

This is the isolated Unity project for the optional `classic-hd` and
`unity-enhanced` presentation modes. It is deliberately separate from
`remc2-unity-native`, which remains the sealed Exact project.

The lab currently provides the safe shell for renderer work:

- Unity `6000.3.20f1` with URP `17.3.0`.
- A `Boot` scene with a pre-session mode router.
- External owner-input validation for package, closure, and asset-cache paths.
- A diagnostic `NO OWNER INPUTS` boot that never opens a session.
- A deterministic presentation-session lease that does not own gameplay or
  load owner payloads.
- No copied owner assets, native libraries, Exact scripts, or Exact corpus.

Run the editor setup once after opening the project:

```text
Unity -batchmode -nographics -quit \
  -projectPath research/private/remc2-unity-presentation-lab \
  -executeMethod MagicCarpet.PresentationLab.Editor.CreatePresentationLabAssets \
  -logFile /tmp/magiccarpet-presentation-lab-setup.log
```

The runtime accepts `-mc-presentation-mode classic-hd` or
`-mc-presentation-mode unity-enhanced`. The owner inputs remain external:

```text
-mc-package /absolute/path/to/level1-export-m \
-mc-closure /absolute/path/to/level1-asset-closure.json \
-mc-asset-cache /absolute/path/to/level1-unity-asset-cache
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
```
