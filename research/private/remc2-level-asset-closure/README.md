# Realm-aware asset closure

This private-track tool joins deterministic initialized level exports to
the qualified owner-data asset catalog. It emits four deliberately distinct
sets:

- `realm_load_roots`: files selected by the realm, resolution, texture-size,
  terrain-flag, and global Level 1 loader paths. The manifest separately lists
  `selected_realm` and `global_open`; a file may legitimately be in both
  (notably `TMAPS1-0.DAT`).
- `scene_reachable`: exact catalog entries and corrected BUILD footprint
  records reachable from initialized Level 1 state.
- `frame_used`: only assets observed at final render/audio/text entry points.
  The qualified A/B trace pair contributes exactly 76 canonical observations;
  each retains its selector, hook, count, and causal edge to the owned catalog
  file or indexed entry.
- `dynamic_dependencies`: banks and entries whose final choice depends on
  animation, yaw, HUD, minimap, text-display, sound, or music state.

Every causal edge records `from`, `to`, `join`, `source_field`, `status`, and
`evidence`.

The global-open roots include both `LEVELS` files, the BUILD and BLDGPRM data,
POINTERS, spell/font/language/audio data, and all three TMAP DAT handles.
Realm selection then chooses the current TMAP handle plus the Night TAB,
terrain blocks, palette/color tables, render tables, sprites, minimap web, and
sky for this Level 1 capture.

## BUILD schema correction

The generic owner catalog previously attempted to decode `BUILD0-0` entries as
bitmap RLE. That interpretation is not used here. remc2's terrain consumers
read a six-byte TAB record (`offset`, `width`, `height`) and then consume
exactly `width * height` two-byte cells. The closure derives distinct
`#footprint-N` records and hashes the exact cell spans.

## Generate and verify

From the repository root:

```sh
python3 research/private/remc2-level-asset-closure/build_closure.py
python3 -m unittest discover \
  -s research/private/remc2-level-asset-closure/tests \
  -p 'test_*.py' -v
```

The default `level1` profile is sealed: its generated JSON and closure hash
remain byte-identical to the accepted Level 1 artifact. The qualified Day
profile can be derived from the Level 2 export with:

```sh
python3 research/private/remc2-level-asset-closure/build_closure.py \
  --profile level2
```

Level 2 validates external level 2/internal level 1, Day map type, the pinned
remc2 commit, owner-file/catalog hashes, and the exact `TMAPS0`/`MSPRD`/
`MWEBD`/`BL32N`/`PALD`/`SKYD` selector tuple. It partitions common realm roots,
scene reachability, dynamic dependencies, and frame observations. Because no
qualified Level 2 final-entry trace pair exists yet, `frame_used` is explicitly
`not_qualified_no_level2_frame_pair` with unresolved evidence; the artifact
does not claim zero unresolved frame assets.

The generated private manifest is
`research/private/generated/level1-asset-closure.json`.

`frame_used` is qualified from two independent, byte-identical Level 1 traces.
They cover final TMAP indices, final bitmap pack/index, exact text and glyph
draws, terrain-block spans, the active sky bank, and final sound/music entry
points. Cursor, sound, and music are armed-empty in this frame and therefore
remain evidence rather than being promoted as assets. Sound/music emptiness
requires valid, enabled, non-pending GDB breakpoints resolved to their exact
source locations both at inferior startup and frame close.

The initialized audio scope records sound bank 1 and the trace observes native
sample quality. It also preserves the documented
presentation divergence: native remc2 selected raw Night music track 1, while
the preserved DOS frames contained track 0 because music was inactive.

The closure consumes final export M and binds its package and compiled-source
hashes, the owner catalog, the baseline static closure, the binary/build/GDB
observer, and `trace_format.py`. Objective string 48 retains its raw paired
quote bytes. Text bytes map to
FONT1 using the engine's one-based glyph convention (`byte + 1`); spaces do
not issue glyph draws. Cursor selector 9 resolves through `POINTERS.DAT`.
The same numeric row in MSPRN is only a conditional spell-menu bitmap and is
therefore dynamic, not cursor proof.
