# Classic HD resolution matrix

This is the current measured matrix for the owner-free Classic HD render
graph plus the separate owner-bound Unity profile-rerender binding smoke. It
keeps the Rust presentation workload distinct from sustained Unity GPU,
copy/upload, owner-route, and preserved corpus qualification. The owner-bound
performance receipt is attached separately below; a row is not a player-facing
production claim until all later measurements are attached.

The reproducible command is:

```text
cargo run --release -p sim-cli -- classic-hd-qualify
```

The command renders 32 deterministic frames for each profile, exercises the
TABLESN temporal path, resets the graph, and compares the first/reset indexed
and provenance hashes. The 30 Hz CPU budget is 33,333,333 ns p95. The recorded
T14 run produced:

| Internal profile | Frame bytes | Graph memory | p95 CPU | Reset deterministic | Current status |
| --- | ---: | ---: | ---: | --- | --- |
| 640x480 | 307,200 | 1,232,896 | 10.95 ms | yes | CPU-supported; owner route is deterministic with zero measured steady allocation, but owner p95 total is 487.7 ms, so real-time/GPU promotion remains open |
| 960x720 | 691,200 | 2,768,896 | 26.11 ms | yes | candidate; owner route is deterministic with zero measured steady allocation, but owner p95 total is 4.62 s, so real-time/GPU promotion remains open |
| 1280x960 | 1,228,800 | 4,919,296 | 45.64 ms | yes | rejected by current CPU budget |

The window is independent of the internal render profile. For example,
640x480 can be displayed in a 1920x1080 window with the Classic 4:3 viewport
composition; changing the window does not silently promote the internal
profile.

The current owner-bound idle smoke is recorded in
`qualification/classic-hd-owner-route-smoke.json`. It passed through the real
`PresentationModeRouter`, Rust native session, additive profile-sized Classic
HD endpoint, source-ordered legacy/typed compositor, and persistent Unity R8
upload at 640x480. The sustained sequence is recorded separately in
`qualification/classic-hd-owner-route-sequence-smoke.json`: two 32-frame input
passes, 64 uploads, one allocation epoch, and matching indexed/provenance
hashes after a native session reset. No FINAL MAIN copy is used. These are
renderer-binding and replay evidence, not yet a full player-facing scenario
or visual-corpus claim. The 960x720 candidate has a one-frame owner profile
smoke in `qualification/classic-hd-owner-route-960x720-smoke.json` and a
64-frame two-pass reset replay in
`qualification/classic-hd-owner-route-960x720-sequence-smoke.json`. The
The owner-bound 8+64 performance smoke is recorded in
`qualification/classic-hd-owner-performance-smoke.json`. It measures the
private owner rerender plus persistent Unity R8 upload and reports p95 native /
total times of 487.0 / 487.7 ms at 640x480 and 4.6008 / 4.6166 s at 960x720.
Both routes preserve one allocation epoch, zero measured steady managed
allocation, monotonic generation/ticks, and nonzero indexed/provenance hashes;
neither meets the 33.3 ms 30 Hz owner presentation budget. The earlier bounded
1.52-second terrain trace remains historical and is superseded for promotion
decisions by this full-route receipt. The earlier bounded failure record remains
at `qualification/classic-hd-owner-route-960x720-sequence-failure.json` as a
historical finding and is explicitly superseded by the passing rerun.

The GPU consumption probe is recorded in
`qualification/classic-hd-owner-gpu-fence.json`. Both profiles produce
changing raw indexed and palette-resolved readbacks without readback errors,
but the T14 OpenGLCore device cannot report the requested async graphics-fence
status because async compute is unavailable. Unity GPU timing and real-time
promotion therefore remain open.

The mandatory 640x480 owner scenario receipt is recorded in
`qualification/classic-hd-owner-route-scenarios.json`. It proves 49 persistent
indexed uploads across idle, moving town, combat projectile/impact rows,
high-speed flight, lethal player state, and tick-0 restart. It is still a
route-observation receipt rather than a visual-corpus, sustained-GPU, or
completion/window qualification.

The optional-tier fallback receipt is recorded in
`qualification/classic-hd-owner-asset-fallback.json`. At 960x720, an explicit
`optional-high-resolution` request with no optional pack present resolves to
`original-indexed` before the owner session renders; the full 49-frame route
still passes. Optional native-tier consumption remains open. The bounded
focus/pause and exit-sidecar receipts separately prove pause-safe authoritative
state and reset-safe exit observability, while real OS window/alt-tab capture
and full authored completion/postgame remain open.

Remaining GFX-O evidence is intentionally explicit in the JSON matrix:
completion/fullscreen/alt-tab, 20-minute allocation growth,
960x720 owner/GPU promotion, visual corpus review, and the complete
owner-bound reflection/lighting/blur matrix still require the private
owner-data Unity capture and preserved qualification artifacts. The
invalid-config case is covered by the launcher/settings focused tests.
