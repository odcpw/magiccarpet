# MC2 private presentation lab

Status: scaffold for `mc-dl0r.18.4`; owner-free static checks pass, but the
Unity editor/player smoke build is still pending on a machine with Unity
`6000.3.20f1` installed.

This is a separate private presentation project for Classic HD and Unity
Enhanced. It is not the sealed Exact project at
`research/private/remc2-unity-native`. It has its own package manifest,
project settings, scenes, scripts, and future Library/build output. The lab
does not contain owner assets, generated owner packages, qualification output,
or a recursive copy of the Exact project.

## Contract

- Rust remains the gameplay authority. This project is presentation and
  adapter code only.
- `classic-hd` and `unity-enhanced` are mutually exclusive startup modes.
- `--mc-mode=classic-hd` and `--mc-mode=enhanced` select the mode before the
  scene starts. There is no mid-session renderer swap.
- `--mc-owner-root=<absolute-path>` is an external owner-data input. The
  default boot is synthetic/no-owner and never searches the repository for
  payloads.
- `--mc-require-owner-data` turns a missing owner root into a clear startup
  error. It is intended for later private qualification, not for an empty
  smoke build.
- A presentation session is created and disposed by the boot component. The
  current session is deliberately a no-payload lifecycle shell; native Rust
  ingress is a downstream bead.

## Pinned package intent

The manifest and lock record the current project pin. Unity must resolve them
with its package manager before a player build can be called qualified:

| Package | Pin |
| --- | --- |
| Unity Editor | `6000.3.20f1` (`c9ba695d4f07`) |
| URP | `17.3.0` |
| Input System | `1.14.2` |
| XR Plug-in Management | `4.7.0` |
| OpenXR Plugin | `1.17.1` |
| XR Interaction Toolkit | `3.0.9` |
| Test Framework | `1.6.0` |

The XR packages are present only in this isolated lab. They must never be
added to the Exact project or to the browser build. If Unity resolves a
different compatible transitive version, update this project's lock and the
VR architecture gate together, with the resulting package metadata recorded.

## Checks

From the repository root:

```text
python3 research/private/remc2-unity-presentation-lab/tools/verify_presentation_lab.py --json
python3 -m unittest discover -s research/private/remc2-unity-presentation-lab/tests -p 'test_*.py'
```

The static check proves project separation, package pins, scene/bootstrap
presence, and absence of owner payload paths. It does not claim that Unity has
opened the project or that a headset has rendered a frame. The remaining
runtime gate is:

```text
Unity -batchmode -nographics -quit -projectPath research/private/remc2-unity-presentation-lab -executeMethod MagicCarpet.PresentationLab.EditorSmoke.Run
```

That command is intentionally documented before the editor script is added;
the editor/player smoke route is the next implementation slice after the
Unity editor is available.
