# Classic HD display composition

`sim-core::classic_hd_display` keeps the Classic HD pipeline split into two
deterministic stages:

1. Rust renders the selected indexed profile into its own 4:3 internal
   framebuffer.
2. The presentation layer fits that framebuffer into the desktop output.

The second stage computes one maintained-aspect content rectangle and four
bar rectangles. It never widens the world camera and never changes the
profile revision or indexed generation hash. A 1920x1080 output therefore
receives a centered 1440x1080 surface; a 3440x1440 output receives a centered
1920x1440 surface. Small windows use the same rules and return `None` for
pointer input that lands in a bar.

The ordinary Classic HD HUD uses remc2's source-style 640x480 canvas. Its
`game_ui_scale` is profile-local and integer-only: 640x480 uses scale 1,
960x720 centers a 640x480 canvas, and 1280x960 uses scale 2. The compatibility
profile remains a separate 320x200 coordinate space. HUD rectangles are
mapped into the internal profile before presentation filtering is applied.

`ClassicHdWindowFilter::{Point,Linear}` is deliberately a window-present
choice. It is not part of indexed world generation, so changing it cannot
alter gameplay, source command order, or frame provenance.

The layer order mirrors the typed overlay stages: minimap terrain precedes
minimap entities and marks; spell/status layers precede menus and objective
text; map layers retain their explicit clear, terrain, entity, world, divider,
and mark ordering. Widescreen world/FOV expansion remains deferred to the
post-v1 decision recorded in `classic-hd-spec.json`.
