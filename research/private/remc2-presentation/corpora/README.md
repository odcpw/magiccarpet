# Presentation corpora

The three corpus directories are deliberately separate:

- `exact/` contains only a generated manifest pointing at the protected Exact
  Level 1 Night reference. It does not copy or rewrite Exact captures.
- `classic-hd/` is reserved for indexed high-resolution rerender evidence.
- `unity-enhanced/` is reserved for URP/GPU-lit presentation evidence.

Use `capture_manifest.py --dry-run` to create a placeholder manifest. The tool
checks that the output path belongs to the selected mode root, embeds the
mode/profile/build/display/determinism metadata, and refuses an existing
manifest whose mode does not match the root.

These manifests describe capture plans; they are not evidence until a real
capture, hash/region result, and review record is attached.
