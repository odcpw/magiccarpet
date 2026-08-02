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
| 640x480 | 307,200 | 1,232,896 | 10.95 ms | yes | CPU-supported; optimized owner route is deterministic with zero measured steady allocation and p95 total 31.65 ms, clearing the 30 Hz owner budget while GPU/window/corpus promotion remains open |
| 960x720 | 691,200 | 2,768,896 | 26.11 ms | yes | rejected for the current owner route; deterministic and zero-allocation, but p95 total is 56.52 ms, over the 30 Hz budget |
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
owner-bound 8+64 performance smoke is recorded in
`qualification/classic-hd-owner-performance-smoke.json`. After streaming the
large mode-26 terrain pass and reusing the generation-bound owner asset digest,
the private owner rerender plus persistent Unity R8 upload reports p95 native /
total times of 30.96 / 31.65 ms at 640x480 and 55.27 / 56.52 ms at 960x720.
Both routes preserve one allocation epoch, zero measured steady managed
allocation, monotonic generation/ticks, and nonzero indexed/provenance hashes.
640x480 now clears the 33.3 ms 30 Hz owner presentation budget; 960x720 is an
explicit measured rejection at the current profile. The performance receipt
therefore records a qualified resolution decision, while GPU timing and final
player-facing promotion gates remain open. The earlier bounded 1.52-second
terrain trace remains historical and is superseded for promotion decisions by
this full-route receipt. The earlier bounded failure record remains at
`qualification/classic-hd-owner-route-960x720-sequence-failure.json` as a
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
ordinary high-speed flight, lethal player state, and tick-0 restart. This
qualifies the route observation; it does not claim the separate Speed-jar
automatic blur/postprocess path, which remains open. It is still a
route-observation receipt rather than a visual-corpus, sustained-GPU, or
completion/window qualification.

The optional-tier fallback receipt is recorded in
`qualification/classic-hd-owner-asset-fallback.json`. At 960x720, an explicit
`optional-high-resolution` request with no optional pack present resolves to
`original-indexed` before the owner session renders; the full 49-frame route
still passes. Optional native-tier consumption remains open. The bounded
focus/pause and exit-sidecar receipts separately prove pause-safe authoritative
state and reset-safe exit observability, while real OS window/alt-tab capture
remains open. The full authored completion/postgame route is recorded
separately below.

The effects-settings receipt is recorded in
`qualification/classic-hd-owner-effects-matrix.json`. The owner-bound 640x480
route accepts `off`, `spatial-2x2`, and `tablesn` with reflections and dynamic
lighting enabled, and rejects disabled reflections or dynamic lighting before
opening the native session. This qualifies the settings contract; it does not
replace the missing visual corpus or automatic high-speed postprocess proof.

The owner-command replay route is recorded in
`qualification/classic-hd-owner-completion-replay-smoke.json`. It reads the
sealed external `mc-native-command-10-byte-le/v1` stream, passes each raw
record to the Rust owner session, and asserts both clocks: the authoritative
simulation tick advances from 1 through 1,112 while the render-camera
presentation tick intentionally reports 0 through 1,111 for those same
post-update frames. The smoke consumes 1,112 commands, uploads 1,112 frames,
holds one allocation epoch, and keeps generation 2 stable. Its status remains
open because the short owner journal does not reach the authored completion or
postgame result; no synthetic input or completion state is substituted.

The full authored completion/postgame route is recorded in
`qualification/classic-hd-owner-completion-route.json`. It replays all 16,232
records from the sealed external owner journal through the Rust session and
Unity R8 surface, uploads one frame per command, holds allocation epoch 1 and
generation 2, and proves authoritative ticks 1..16,232 alongside presentation
ticks 0..16,231. The route reaches the authored exit result 10 with one
completed task. This receipt was requalified with the source-faithful Classic
HD tile-span guard; the runner used the quarantined native plugin build whose
SHA-256 is recorded in the JSON matrix. Owner payload remains external.

Remaining GFX-O evidence is intentionally explicit in the JSON matrix:
Speed-jar automatic high-speed postprocess, fullscreen/alt-tab,
20-minute allocation growth, GPU timing and window presentation, the rejected
960x720 alternative, visual corpus review, and the complete owner-bound
reflection/lighting/blur matrix still require the private owner-data Unity
capture and preserved qualification artifacts. The invalid-config case is
covered by the launcher/settings focused tests.
