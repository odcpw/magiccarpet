# Classic HD render graph

The Classic HD presentation path is a separate, indexed renderer owned by the
private Rust simulation core. Exact remains the fixed 320x200 path. A Classic
HD request allocates a profile-sized MAIN, provenance plane, world scratch
plane, and postprocess scratch plane; it never upscales Exact FINAL MAIN.

## Ordered stages

`ClassicHdRenderGraph::render_frame` commits the following source-shaped
sequence:

1. World closure: terrain, typed sprites, shadows, reflected terrain/sprites,
   and optional mode-26 water are submitted by the caller and verified through
   `ClassicHdWorldReceipt`.
2. Reflections receipt: enabled only when the world request asked for the
   reflected pass.
3. Dynamic lighting: optional profile-sized TABLESN shading lookup.
4. Spatial smoothing: optional profile-sized cross-neighbour smoothing.
5. Temporal blur: optional source TABLESN pair-table blending against the
   previous committed MAIN. Automatic mode 1 uses the source speed, menu,
   high-speed, and scratch gates; configured mode 2 bypasses those automatic
   gates. Mode 1 preserves the source first-dword operand reversal.
6. Present: the selected profile-sized plane becomes the new committed MAIN.

Every stage has a `ClassicHdStageReceipt` with input/output hashes, work units,
and elapsed nanoseconds. Elapsed timing is diagnostic only and is not part of
the presentation hash.

## History and failure rules

World and postprocess work is performed in scratch storage. MAIN, its
provenance, the history key, and the last receipt are changed only after all
enabled stages validate and the receipt is complete. A failed world closure,
missing shading plane, malformed TABLESN history request, stale tick, or
profile/generation mismatch therefore cannot advance history or leak pixels.
The source-render input hash is retained for receipt diagnostics but is not a
history compatibility gate: authoritative world state normally changes every
flight tick, and temporal presentation must be able to blend adjacent frames.

`reset` clears all planes and history for a new generation. `reconfigure`
replaces the graph with a fresh profile-sized storage set, so a resolution or
profile change cannot blend old pixels into the new presentation.

## Compatibility proof

The compatibility profile uses 320x200 storage. Its temporal indexed output
and provenance are compared in focused tests with the existing Exact
`finalize_indexed_world` implementation and the same TABLESN pair table. The
graph receipt uses the Classic HD hash domain already used by the Classic HD
profiles and raster adapters; the legacy Exact postprocess receipt retains its
own existing hash domain.
