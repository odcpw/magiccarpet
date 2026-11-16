# Original Level 1 in the browser

The native Rust runtime owns gameplay, input admission, RNG, scheduling and
save/load. `original_bridge.py` loads the installed `sim-capi` library and starts
the ordinary campaign from the existing four owner packages. Three.js displays
the original indexed scene and palette; the browser composes the source HUD,
spell grid, objective text and minimap from native observations and owner assets.
This first Original route uses Rust's existing software world renderer. It does
not reconstruct terrain as Three.js geometry, run Unity, or compile the game to
WASM. Modern/Voxel geometry remains a separate presentation task.

From this directory, after `pnpm install --frozen-lockfile`:

```sh
./run-original.sh
```

Open <http://127.0.0.1:8881/>. The launcher builds only the Original browser
entry, then serves it and the native API from one origin. Owner packages, native
library and decoded source assets must already exist locally. Missing inputs
produce an explicit loading error. There is no synthetic game fallback.

For private tailnet access on this machine:

```sh
./run-original.sh --host 100.81.187.69 --port 8882 \
  --allow-host 3090.bumblebee-hue.ts.net:8882 \
  --save ../../generated/original-web-20261116/tailnet-save.json
```

The explicit Host alias is required. POST requests also require matching Origin
and the controller cookie. Only one active controller is admitted; closing or
backgrounding the page pauses the game. The backend additionally pauses after
1.5 seconds without input. This is a local/private experimental game service,
not a public multiplayer server. Separate processes own separate sessions.

Controls: W/S speed, A/D strafe, mouse or arrows steer, Space casts the selected
spell, mouse buttons cast assigned hands. Hold Ctrl for the source spell grid;
left/right click assigns the indicated tier to that hand. O shows the objective,
Tab toggles the map, Esc pauses, F5 saves and F9 loads. The utility menu also
provides resume, save, load, restart and fullscreen. Clicking the view captures
the mouse; Esc releases it. A phone-sized layout and basic touch controls are
present; desktop keyboard/mouse is the qualified control path.

The binary scene and MCPO observations must match sequence, runtime generation,
session generation and tick. Stale publications are dropped. Scene history uses
the source lookup table once per native scene revision, before HUD composition.
The split map is a separate producer and never enters flight-scene history.
Native updates target 30 Hz; `originalMetrics.received_hz` measures publication
delivery, not maximum GPU FPS. `bytes_received` counts indexed scene packets
only. Pause/resume resets the delivery sample. This is not a comparative engine
performance benchmark.

Focused checks:

```sh
node --test tests/original-adapter.test.ts tests/original-hud.test.ts
pnpm typecheck
python3 tests/test_original_assets.py
python3 tests/original_bridge_test.py
ORIGINAL_PROOF=/absolute/disk/path DISPLAY=:1 node tests/original-browser.mjs
```

Set `ORIGINAL_URL` to the exact deployed entry for the browser check. It uses
installed Chrome and a fresh Playwright profile, with real HTTP/native calls.
`?automation=1` only skips pointer capture; it does not replace the simulation.
The check restarts Level 1 and overwrites that session's save slot. Use a separate
service/save path for qualification when preserving a play session matters.

Self-verification on 2026-11-16 exercised visible source terrain/sprites/HUD,
flight with changed native pose, objective and map admission, the spell grid,
casting with native mana consumption, pause with stable tick, and save/load with
restored pose and tick. Desktop 1280×960 and portrait 390×844 captures are kept
outside Git. Chrome reported the real NVIDIA RTX 3090 renderer; no console or
HTTP errors were observed in the passing interaction run. The deployed built
route also passed actual mouse capture and Escape pause after reloading.
The qualified native library SHA-256 was
`6451e447039315dfb3c1fd4f4a1b3dc0c3bfdf77a71927f0334505602f9db6bc`;
this identifies the tested binary, not the latest source revision of the port.

Changed-source UBS reported zero critical findings. Reviewed warnings concern
document-lifetime event listeners/timer, dense typed arrays incorrectly labelled
sparse, and a request-queue promise whose rejection is caught on the same line.
The missing timeout on the startup request was fixed. No warning was suppressed
to obtain a passing result.

Still incomplete: audio playback, original options/pause submenus, Beyond Sight
map labels and map-mode hand flashes. This route does not establish a complete
mission playthrough or full Original port completion. Existing Rust gameplay
defects remain owned by the Original port team.
