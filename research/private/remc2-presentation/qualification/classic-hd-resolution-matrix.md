# Classic HD resolution matrix

This is the current measured matrix for the owner-free Classic HD render
graph plus the separate owner-bound Unity profile-rerender binding smoke. It
keeps the Rust presentation workload distinct from sustained Unity GPU,
copy/upload, owner-route, and preserved corpus qualification. A row is not a
player-facing production claim until those later measurements are attached.

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
| 640x480 | 307,200 | 1,232,896 | 10.95 ms | yes | CPU-supported; owner profile rerender, persistent Unity upload, and owner idle/town/combat/flight/death-restart route smoke pass |
| 960x720 | 691,200 | 2,768,896 | 26.11 ms | yes | candidate; owner profile rerender/upload and 64-frame two-pass reset replay now pass; traced owner terrain raster was ~1.52 s/frame, so real-time/GPU/scenario promotion remains pending |
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
streaming mode-5 terrain path reduced the traced owner terrain stage to about
1.52 seconds per 960x720 frame on T14; therefore the sequence is promoted as
deterministic owner-route evidence, but the profile is not promoted as
real-time or player-facing GPU/scenario support. The earlier bounded failure
record remains at `qualification/classic-hd-owner-route-960x720-sequence-failure.json`
as a historical finding and is explicitly superseded by the passing rerun.

The mandatory 640x480 owner scenario receipt is recorded in
`qualification/classic-hd-owner-route-scenarios.json`. It proves 49 persistent
indexed uploads across idle, moving town, combat projectile/impact rows,
high-speed flight, lethal player state, and tick-0 restart. It is still a
route-observation receipt rather than a visual-corpus, sustained-GPU, or
completion/window qualification.

Remaining GFX-O evidence is intentionally explicit in the JSON matrix:
completion/fullscreen/alt-tab/asset-fallback, 20-minute allocation growth,
960x720 owner/GPU promotion, visual corpus review, and the complete
owner-bound reflection/lighting/blur matrix still require the private
owner-data Unity capture and preserved qualification artifacts. The
invalid-config case is covered by the launcher/settings focused tests.
