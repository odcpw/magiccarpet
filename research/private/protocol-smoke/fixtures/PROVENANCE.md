# Synthetic fixture provenance

All fixtures in this directory are project-authored, known-source synthetic
programs created for `mc-51m.5`. They contain no Magic Carpet or other owned
game bytes, observations, identifiers, or inferred behavior.

- Generator: none; the Python and JSON files are the canonical originals.
- Runtime: Python standard library only.
- Approved use: private protocol/registry qualification.
- Exposure: local process execution with no network.
- Regeneration: edits require packet hash updates, expected-root regeneration,
  review of the resulting root diff, and a fresh-worker replay.
