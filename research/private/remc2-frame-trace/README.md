# Private remc2 first-presented-frame trace

This Track P tool observes the first presented gameplay frame of a lawfully
owned Magic Carpet 2 Level 1 runtime. It does not patch the remc2 checkout.
CMake injects a second Debug executable target by cloning the stock `remc2`
target, and GDB Python breakpoints observe final render/audio/text entry points.
The fixed runtime configuration uses an 800x600 software frame so remc2 selects
`GameRenderHD`, but keeps high-resolution replacement graphics disabled: final
selectors still refer to the lawfully owned original data.

The canonical `trace.jsonl` is deterministic. Its header binds the final M/N
Level 1 package, its compiled-source identity, the static closure, the owner
catalog, upstream commit/diff, trace source/build/binary, GDB, observer,
configuration, argv, recording mode, frame window, camera, and render/audio
settings. Nondeterministic UUID and timestamps live only in `run.json`.

## Build and focused tests

```sh
research/private/remc2-frame-trace/scripts/build.sh
PYTHONDONTWRITEBYTECODE=1 python3 -m unittest discover \
  -s research/private/remc2-frame-trace/tests -p 'test_*.py' -v
```

The Debug build is outside the repository at
`/tmp/magiccarpet-remc2-frame-trace-build` by default.

The exact mutable Goat TMAP states can be extracted separately through the
compiled remc2 delta decoder. The destination must be a new private directory:

```sh
research/private/remc2-frame-trace/scripts/run_goat_tmap_oracle.sh \
  /tmp/magiccarpet-goat-tmap-oracle
```

This captures selectors 152–154 at reset, after each of their four declared
transitions, and after one cycle-closing transition. It does not infer the
render-admission clock or claim that the reset buffer is a displayed frame.

## Run and compare

Destinations must not already exist:

```sh
research/private/remc2-frame-trace/scripts/run_trace.sh \
  research/private/generated/level1-frame-trace-final-v2-a
research/private/remc2-frame-trace/scripts/run_trace.sh \
  research/private/generated/level1-frame-trace-final-v2-b
python3 research/private/remc2-frame-trace/scripts/verify_runs.py \
  research/private/generated/level1-frame-trace-final-v2-a \
  research/private/generated/level1-frame-trace-final-v2-b \
  --require-complete-categories
```

The observer evaluates ordered `DrawAndEventsInGame_47560` candidates and arms
on the first candidate that reaches a final TMAP draw. Palette-only
`VGA_Blit(nullptr)` calls before any final draw are excluded without changing
game state; each is retained in the header with its candidate, reason, zero
final-draw count, and one `SubBlit` proof. The qualified window closes after the
first non-null `VGA_Blit` reaches `SubBlit` exactly once and returns. Candidate
and excluded-present counts are bounded. Every pointer-backed final selector
must resolve to one entry; zero or multiple candidates fail the run.

The complete observer covers final TMAP, post-cull terrain, the active sky
compositor, bitmap draws, cursor source and post-clip extent, exact text bytes,
final glyphs, sample starts, and music-sequence starts. Cursor provenance
distinguishes the nominal global selector from the bitmap most recently passed
to `SetCursor_8CD27`; Level 1's first qualified frame deliberately uses
`POINTERS[0]` (the null cursor) even though the nominal selector is 9. A category
may be empty only with hook-armed lifecycle evidence. In this frame the cursor
has a zero post-clip extent, and no sample or music sequence starts occur.
The sound and music empty proofs retain their GDB breakpoint objects and verify
at both inferior startup and frame close that each breakpoint is valid,
enabled, non-pending, and resolved to exactly its expected source line and
function. The header also binds the exact `trace_format.py` digest, so the
qualifier that interprets those proofs is part of the trace identity.
