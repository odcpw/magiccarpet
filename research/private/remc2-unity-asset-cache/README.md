# Private Level 1 Unity asset cache

This tool materializes a deterministic, owner-local cache for the qualified
Level 1 Unity presentation. It is bound to the exact export package
`730d2fa0...9104ac8` and closure `87812390...75719`.

The cache contains:

- the raw indexed 256×256 Night sky and ordered 256-entry RGB6 `PALN` table,
  plus the exact VGA `value * 4` RGB8 expansion used by Unity;
- the source-locked 256×608 `BL32N` atlas repacked into 152 indexed 32×32
  terrain tiles, the exact 65 Level 1 selectors, and all 32 source
  `UVTable_D4350` rows with the eight persistent `(terrain angle >> 4) & 7`
  Unity mappings;
- the full `TABLESN` and `CLRN-0` payloads, including the qualified 256×256
  static shade lookup `TABLESN[(brightness << 8) | source index]`;
- all 180 Level 1 scene/live-mission and fixed-profile FINAL MAIN
  `TMAPS1-0` dependencies, including the source-family closure added for the
  native compositor;
- the exact four mutable indexed states for Goat selectors 152, 153, and 154,
  decoded through the qualified type-7 SS2 subset and checked against the
  pinned remc2 before/after oracle. Frame 4 restores reset, the next update
  reproduces frame 1, and malformed or unsupported stream forms fail closed;
- a canonical Level 1 entity-presentation descriptor derived from exact
  `MCE1`/`MCV1` package bytes and the terrain head plane. It maps all 177 active
  rows into 17 groups, locks the `83 drawable / 47 suppressed / 47 off-chain`
  reset disposition, and separates the 64 stable qualified actors from 19
  frozen transient rows that diverge before first presentation;
- eighty source-selected HSPRN bitmaps, including the objective,
  minimap-entity, Castle, Speed, spell, panel, and complete 2×13 bottom spell
  menu and three-tier submenu selectors, with their exact Night dimensions and the current 640×480
  reference layout;
- MSPRN index 9, included only because the closure proves it as a dynamic
  spell-menu dependency;
- all 273 `FONT1` visibility masks in one offset-addressed blob;
- the exact owned English localization payload binding, selector-48 bytes
  `"Fly towards my beacon."`, and selectors 48–53 hashes;
- a canonical Level 1 HUD descriptor covering panels, reset Fireball/Possess
  slots, authority, dimensions, source functions, bitmap hashes, font
  byte-plus-one mapping, minimap mode, and every deferred field;
- six source-sized indexed objective foreground rasters using exact HFONT3
  wrapping/advances and HSPRN 171/172/173 frame assembly, each paired with a
  binary overwrite mask, plus the separate HFONT3 `FROZEN OBSERVED`
  authority badge; and
- canonical spell family/tier icon indices. Fireball, Possession, Castle, and
  Speed big icons 123–126 are materialized from HSPRN; and
- `frame/source-frame-census.json`, a pinned remc2 writer/asset/branch census
  for the accepted 320×200 Night, screen-size-0, single-local-player native
  FINAL MAIN. It binds source-tuple disposition, map-chain order, hotspots,
  reflection/shadow adjacency, dynamic lighting, temporal/spatial
  postprocess, supported overlays, atomic COMPLETE receipts, the revisioned C ABI, and
  Unity's one-row-flip/one-R8-plane ownership. Stereo, alternate resolution,
  active-DAC RGB for historical captures, hostile/remote players, unsupported
  branches, and cross-timeline pixel/clock equality remain explicit
  nonclaims. Active tutorial/briefing, recursive help popup, immobilization
  spider-web, and menu-state-7 score-writer states are recorded as reachable
  but fail closed before COMPLETE; and
- a deterministic music dynamic-control census: playback-slot-0 `C2GAME1`
  has no controller-119 tags, while loop-scan-pointer-slot-1 `C2GAME2` tags
  channels 7 and 8. The descriptor records the 90 Hz `30 * 3` envelope and
  leaves audible channel resolution explicit rather than inventing a stem;
  and
- the exact owner `GOAT.WAV` record for typed Rust Goat-sound events, including
  the source-ordered global-RNG pitch families of -10..+9 percent for ordinary
  bleats and +10..+29 percent for action 14; and
- all 30 materialized Level 1 effect records currently reachable through the
  qualified mission/audio mapping, the explicit silent sound-12 contract,
  and the deterministic owner-local Level 1 music stream. This includes exact
  `DRAGROAR.WAV` sound 8 for the multipart creature chain,
  `SPLASH2.WAV` sound 27 for class-10/model-5 water impacts, and
  `SPELLUP.WAV` sound 63 for the group-34 spell scroll.

Indexed pixels, visibility masks, and palettes remain separate. `FONT1` is a
proven visibility mask recolored by the caller. For the Night Level 1
objective, the active table is `CLRN-0`: selector 3840 gives PALN index 119,
or exact RGB8 `(252,36,36)`. The historical remc2 pointer name contains
`clrd`, but `PaletteChanges_47760` installs CLRN for Night; the preserved
Level 1 screenshot confirms the resulting color. MSPRN records use base PALN.
The native ABI v2 exposes the complete effective 768-byte Level 1 palette,
atomically bound to generation and tick. The Rust simulation owns the
source-ordered hit-red, slowing/impairment, objective-white, exit-entry,
fade-back, and black-fade RGB6 transitions; Unity applies the returned palette uniformly to
the indexed world, sprites, HUD, minimap, and objective graphics. Ordinary
single-player death does not enter the multiplayer/banishment monochrome
path, so no unsupported grayscale effect is invented for this slice.
Speed Up changes the source countdown speed and emits its model-2 no-draw
trail; it does not request a palette tint. The fixed-profile native FINAL
MAIN now implements the exact high-speed temporal branch, including its
first-four-pixel asymmetry and recursive prior-MAIN history.

The HUD authority contract is deliberately mixed:

- `PlayerFlightLive`: minimap center, yaw, terrain sampling, and source
  projection;
- `SpellActivationLive`: the left Fireball activation event only;
- `GoatLive`: the 33 Level 1 Goat rows through the Rust-owned bulk ABI;
- `FrozenObserved`: reset terrain, panels, and immutable owner rasters; and
- `SourceMappedLive`: health/mana bars, castle and spell state, the native
  effective palette, and objective mark/arrow target, blink, and geometry;
  and the Level 1 objective textbox selector, exact source geometry, 33-turn
  clock, TABLESN selector-61 live indexed-world blend, HFONT3 text, and
  HSPRN frame draw order; and
- `Deferred`: multiplayer remote-player minimap reveal.

The objective compositor keeps indices authoritative until final palette
resolution. For every live destination index it applies
`TABLESN[0x4000 + (CLRN[0] << 8) + destination]`, then overwrites the exact
HFONT3 text and HSPRN frame foreground. Unity's 640×480 corrected 4:3 HUD
samples the qualified 320×200 indexed world at pixel centres with nearest
selection; that explicit display transform is the only adaptation in this
Level 1 textbox path.

The cached minimap descriptor requires the qualified high-resolution HSPRN
branch: a native 128×128 player-centered/yaw-rotated view with 256 source
units per pixel. Its aperture is the exact TABLESN `0x14300` scanline curve rather
than a Euclidean circle. The fixed-point row/column transform and remc2 sine
table are cached with cardinal and octant source-coordinate vectors so a
vertical reflection or floating-point drift fails qualification. Terrain coloring uses the original minimap
lookup sequence `TABLESN[0x14000 + terrainType]`, then
`TABLESN[(terrainShade << 8) | base]`, then PALN. Frozen entity positions and
the live player marker are presented separately. The objective mark pass is
Rust-owned and source-mapped, including target selection, source-slot
ordering, wrapped distance, blink gates, arrow placement, and the seven arrow
vertices. The same Rust-owned command stream maps
`DrawMinimapEntities_B_61A00`: castle rope first, then ascending source slots,
with exact Level 1 class/model/action/control/ownership filters, palette
colors, phase rules, dot/sprite anchors, direct/blended writes, and the
single-player reveal behavior.

World entities use TMAPS1, not MSPRN. Mode-20 actor rows retain the exact
16-sector yaw table and rear-sector mirror rule. Goat/villager TMAPs retain
their complete four-frame decoded delta streams and are described as
selector-global animation. The cache materializes every mutable state for
Goat selectors 152–154 and preserves the complete villager selector streams.
The cache descriptor's camera-facing billboard remains a controlled debug
gallery and managed fallback. The qualified player instead consumes these
payloads through the Rust world compositor, whose Goat and villager
selector-global clocks advance only from native draw admission.

The descriptor does not claim that its debug-gallery billboard anchor is the
original sprite anchor. Qualified native families use the separately tested
source projection, clipping, reflection, shadow, and painter traversal.

The terrain material output is intentionally scoped as an indexed Night
material and static-shade bridge. Camera/distance-dependent `pnt5` fog,
water-wave brightness, and original software-raster interpolation are not
claimed here; they remain the separate `mc-dl0r.17.11.6.8` qualification.

The materializer checks every sprite and glyph against both the proven atlas
decoder and the qualified owner catalog hashes. The canonical manifest records
source asset identities, decoder and generator hashes, output hashes, exact
bindings, inclusion reasons, and palette status.

The PLAN29 final sealed cache identity is 1,249 outputs, manifest SHA-256
`1eeb90e39927cb28e214e017c2603f3200ee2d3899f3f81ce53063f6d64a63d6`,
content SHA-256
`cb58b2190d3f6ed7c676e0180d0dfc909c7fe692e42946a927f887568be2eb8a`,
and source-frame-census SHA-256
`60b6be08f57ab7feadf59d014d161129c5df31a8c5a752ceb04bc7fcc9634a21`.
Earlier cache identities remain accepted only on the historical native load
paths that explicitly name them; qualified current Unity presentation pins
this PLAN29 pair and its sealed closure-contract identity.

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
`research/private/generated/level1-unity-asset-cache-plan29-final`. The generator refuses
outputs outside `research/private` and specifically refuses Unity `Assets`,
`StreamingAssets`, and `Builds`.

Unity must receive the cache directory explicitly at runtime. It must load only
`manifest.json` and the manifest-listed cache outputs. It must validate the
schema, exact package and closure bindings, every output size and SHA-256, and
the aggregate `cache_content_sha256` before creating textures or glyphs. Unity
must never open the original DAT/TAB paths.
