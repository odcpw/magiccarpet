# Classic HD typed sprite path

`sim_core::classic_hd_sprites` is the additive bridge between the sealed
source sprite census and a profile-sized indexed frame.

- `ClassicHdSpriteCamera` converts the checked Classic HD projection into the
  existing fixed-point sprite camera and scales only the render/fog admission
  envelope. The compatibility-320x200 profile uses the original limits.
- `build_classic_hd_sprite_plan` preserves strictly increasing source-slot
  order, rejects slot zero and reordered/duplicate submissions, and emits
  primary, reflected, and terrain-shadow pass records. A reusable plan owns
  its capacity; reset/refill does not grow the steady-state vectors.
- `rasterize_classic_hd_sprite_pass` uses the existing indexed source kernels
  for primary, reflected, shadow, and possession blend passes. The rasterizer
  requires the checked tightly packed profile extent and verifies that the
  TMAP extent did not change between planning and drawing.
- `CLASSIC_HD_TYPED_FAMILIES` covers all 35 typed Level 1 source families plus
  Goat. Each family maps to a named existing typed renderer family or an
  explicit source exclusion; there is no wildcard sprite fallback.

Legacy-family rows remain named delegation points because their family-specific
source clocks and payload lookup already live in the existing source world
compositor. `terrain_frame::raster_level1_source_world_sprites` now feeds that
compositor a profile-sized projection and caller-owned profile plane, so typed
and legacy rows share the established source order, reflection gate, shadow
rules, animation clocks, and family banks without changing the Exact path. The
adapter does not invent a tuple or allow Unity to decide visibility.

Focused gate:

```text
cargo test -p sim-core classic_hd_sprites --lib -j1  # 4 passed
```

The owner-bound 640x480 idle smoke exercises the complete profile-sized source
compositor and one persistent Unity R8 upload; its hashes are preserved in
`qualification/classic-hd-owner-route-smoke.json`. The sequence smoke adds two
32-frame passes and reset replay evidence in
`qualification/classic-hd-owner-route-sequence-smoke.json`. The broader town,
combat, flight, restart, and sustained GPU/performance matrix remains open.

The Exact sprite projector and compatibility route remain unchanged; the new
profile-limit wrappers are additive and use the original constants by default.
