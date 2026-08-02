# Classic HD resolution matrix

This is the current measured matrix for the owner-free Classic HD render
graph. It separates the Rust presentation workload from the later Unity GPU,
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
| 640x480 | 307,200 | 1,232,896 | 10.95 ms | yes | CPU-supported; Unity/owner route pending |
| 960x720 | 691,200 | 2,768,896 | 26.11 ms | yes | candidate; Unity/owner route pending |
| 1280x960 | 1,228,800 | 4,919,296 | 45.64 ms | yes | rejected by current CPU budget |

The window is independent of the internal render profile. For example,
640x480 can be displayed in a 1920x1080 window with the Classic 4:3 viewport
composition; changing the window does not silently promote the internal
profile.

Remaining GFX-O evidence is intentionally explicit in the JSON matrix:
idle/town/combat/flight/restart/completion/fullscreen/alt-tab/asset-fallback
and owner-bound reflection/lighting/blur route cases still require the
private owner-data Unity capture and preserved qualification artifacts. The
invalid-config case is covered by the launcher/settings focused tests.
