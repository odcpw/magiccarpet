# Private Level 1 Unity asset cache

This tool materializes a deterministic, owner-local cache for the qualified
Level 1 Unity presentation. It is bound to the exact export package
`730d2fa0...9104ac8` and closure `1c3774ca...bcc12`.

The cache contains:

- the raw indexed 256×256 Night sky and ordered 256-entry RGB6 `PALN` table,
  plus the exact VGA `value * 4` RGB8 expansion used by Unity;
- the source-locked 256×608 `BL32N` atlas repacked into 152 indexed 32×32
  terrain tiles, the exact 65 Level 1 selectors, and all 32 source
  `UVTable_D4350` rows with the eight persistent `(terrain angle >> 4) & 7`
  Unity mappings;
- the full `TABLESN` and `CLRN-0` payloads, including the qualified 256×256
  static shade lookup `TABLESN[(brightness << 8) | source index]`;
- all 129 scene-reachable `TMAPS1-0` records: 118 exact decoded streams and 11
  source-verified one-byte `b'0'` placeholders;
- a canonical Level 1 entity-presentation descriptor derived from exact
  `MCE1`/`MCV1` package bytes and the terrain head plane. It maps all 177 active
  rows into 17 groups, locks the `83 drawable / 47 suppressed / 47 off-chain`
  reset disposition, and separates the 64 stable qualified actors from 19
  frozen transient rows that diverge before first presentation;
- the 11 MSPRN bitmaps observed at qualified final render entries, with an
  explicit Night low-resolution-bank, nearest-neighbour 2× presentation
  contract for the current 640×480 reference layout;
- MSPRN index 9, included only because the closure proves it as a dynamic
  spell-menu dependency;
- all 273 `FONT1` visibility masks in one offset-addressed blob;
- the exact owned English localization payload binding, selector-48 bytes
  `"Fly towards my beacon."`, and selectors 48–53 hashes;
- a canonical Level 1 HUD descriptor covering panels, reset Fireball/Possess
  slots, authority, dimensions, source functions, bitmap hashes, font
  byte-plus-one mapping, minimap mode, and every deferred field;
- a deterministic 256×38 owner-frame/FONT1 objective raster and a FONT1
  `FROZEN OBSERVED` authority badge; and
- canonical spell family/tier icon indices. The Level 1 reset big icons 123
  and 124 are materialized; the HSPRN bank is deliberately deferred rather
  than silently mixed with MSPRN; and
- a deterministic music dynamic-control census: playback-slot-0 `C2GAME1`
  has no controller-119 tags, while loop-scan-pointer-slot-1 `C2GAME2` tags
  channels 7 and 8. The descriptor records the 90 Hz `30 * 3` envelope and
  leaves audible channel resolution explicit rather than inventing a stem;
  and
- the exact owner `GOAT.WAV` record for typed Rust Goat-sound events. The
  descriptor deliberately withholds exact playback until the source-ordered
  global-RNG pitch value is qualified.

Indexed pixels, visibility masks, and palettes remain separate. `FONT1` is a
proven visibility mask recolored by the caller. For the Night Level 1
objective, the active table is `CLRN-0`: selector 3840 gives PALN index 119,
or exact RGB8 `(252,36,36)`. The historical remc2 pointer name contains
`clrd`, but `PaletteChanges_47760` installs CLRN for Night; the preserved
Level 1 screenshot confirms the resulting color. MSPRN records use base PALN.
Runtime `paletteMod`/`paletteSubMod` fade, hit, monochrome, and flash variants
are not exposed by ABI v2 and remain explicitly deferred.

The HUD authority contract is deliberately mixed:

- `PlayerFlightLive`: minimap center and yaw only;
- `SpellActivationLive`: the left Fireball activation event only;
- `GoatLive`: the 33 Level 1 Goat rows through the Rust-owned bulk ABI;
- `FrozenObserved`: reset terrain, remaining entities, objective, panels, and
  spell selectors; and
- `Deferred`: health/mana bars, castle/balloon state, spell glow/tier/charge,
  exact minimap entity color/filter/reveal and objective marks, high-resolution
  HSPRN parity, and textbox blending against the live framebuffer.

The cached minimap descriptor requires the qualified low-resolution branch:
a native 64×64 player-centered/yaw-rotated view, nearest-neighbor scaled 2× to
128×128. Its aperture is the exact TABLESN `0x14300` scanline curve rather
than a Euclidean circle. The fixed-point row/column transform and remc2 sine
table are cached with cardinal and octant source-coordinate vectors so a
vertical reflection or floating-point drift fails qualification. Terrain coloring uses the original minimap
lookup sequence `TABLESN[0x14000 + terrainType]`, then
`TABLESN[(terrainShade << 8) | base]`, then PALN. Frozen entity positions and
the live player marker are presented separately. The generic white entity dots
are explicitly a **non-original `FrozenObserved` reset reference overlay**;
exact original entity colors, filters, reveal rules, and mark behavior are not
claimed.

World entities use TMAPS1, not MSPRN. Mode-20 actor rows retain the exact
16-sector yaw table and rear-sector mirror rule. Goat/villager TMAPs retain
their complete four-frame decoded delta streams and are described as
selector-global animation; the current qualified Unity view deliberately uses
only their exact base frames.

The descriptor does not claim exact original sprite anchoring. It records the
current viewer policy as camera-facing, bottom-center at entity xyz, and
explicitly defers the `DrawSprite_41BD3` camera-dependent centering offset and
software depth/raster traversal.

The terrain material output is intentionally scoped as an indexed Night
material and static-shade bridge. Camera/distance-dependent `pnt5` fog,
water-wave brightness, and original software-raster interpolation are not
claimed here; they remain the separate `mc-dl0r.17.11.6.8` qualification.

The materializer checks every sprite and glyph against both the proven atlas
decoder and the qualified owner catalog hashes. The canonical manifest records
source asset identities, decoder and generator hashes, output hashes, exact
bindings, inclusion reasons, and palette status.

## Build and validate

From the repository root:

```sh
python3 research/private/remc2-unity-asset-cache/build_cache.py build
python3 research/private/remc2-unity-asset-cache/build_cache.py validate
python3 -m unittest discover \
  -s research/private/remc2-unity-asset-cache/tests \
  -p 'test_*.py' -v
```

The default output is
`research/private/generated/level1-unity-asset-cache`. The generator refuses
outputs outside `research/private` and specifically refuses Unity `Assets`,
`StreamingAssets`, and `Builds`.

Unity must receive the cache directory explicitly at runtime. It must load only
`manifest.json` and the manifest-listed cache outputs. It must validate the
schema, exact package and closure bindings, every output size and SHA-256, and
the aggregate `cache_content_sha256` before creating textures or glyphs. Unity
must never open the original DAT/TAB paths.
