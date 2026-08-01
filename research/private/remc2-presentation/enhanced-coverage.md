# Unity Enhanced Level 1 coverage audit

`enhanced-coverage-matrix.json` is the handoff contract between the Rust
simulation and the separate Unity Enhanced presentation route. It enumerates
the typed `Level1SourceSpawnKind` census plus the independent Goat family,
terrain/camera/profile inputs, HUD/minimap/objective state, audio references,
and lifecycle semantics.

Every supported family has exactly one of two honest routes:

- `new-gfx-q`: add a fixed, generation-bound Rust-owned record or event to the
  GFX-Q snapshot bridge before Unity can consume it;
- `explicit-exclusion`: the source census already says the tuple is reserved,
  hidden, or intentionally sprite-less, so Unity must not invent a visual
  entity from it.

The matrix also records named `blocking-gap` rows for Level 2 and optional
owner high-resolution-bank coverage. These are not silently treated as
supported Level 1 content. Unknown generation, tuple, selector, family,
profile, duplicate identity, or lifecycle state fails closed.

Validate it with:

```bash
python3 research/private/remc2-presentation/validate_enhanced_coverage.py
python3 -m unittest research/private/remc2-presentation/tests/test_enhanced_coverage.py -v
```

The current Unity lab remains a mode/session diagnostic shell. This artifact
does not claim that the shell already renders the listed content; it defines
the exact fields GFX-Q must add while preserving Rust as gameplay authority.
