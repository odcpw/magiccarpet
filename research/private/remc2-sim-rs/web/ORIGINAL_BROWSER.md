# Original Level 1 in the browser

The native Rust runtime owns gameplay, input admission, RNG, scheduling and
save/load. `original_bridge.py` loads the installed `sim-capi` library and starts
the ordinary campaign from the existing four owner packages. The default route
now renders the terrain, source-selected original sprite frames, water motion
and indexed particle blends locally with Three.js/WebGL2. The browser also
composes the source HUD, spell grid, objectives and minimap. There is no Unity
process, streamed world image, WASM build, or second gameplay simulation.

The prior streamed PoC is retained with `--renderer indexed`. It shares controls
and uses Rust's software world renderer. The optional Modern mesh consumer is
described below; Voxel browser rendering remains separate work.

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

The live geometry route is <http://3090.bumblebee-hue.ts.net:8882/>. The retained
indexed PoC runs separately at <http://3090.bumblebee-hue.ts.net:8883/>. They use
separate Rust sessions and saves. `magiccarpet-browser-geometry.service` and
`magiccarpet-browser-indexed.service` are transient user services on 3090; they
survive the agent's shell but are not enabled boot services. The commands above
remain the reproducible manual launch path.

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
the mouse; Esc releases it. Touch users drag the view to steer and use the
flight pad and Cast button. Spells opens a tap grid with a Left/Right hand
selector; flight controls move clear of it. The world fills portrait/landscape
viewports, while HUD regions stay anchored at the top, centre and bottom.

MCW1 binary sprite rows, MCPO and HUD state must match sequence, runtime
generation, session generation and tick. Terrain planes are fetched per native
revision, compressed on the wire, with only changed terrain chunks rebuilt.
The geometry route retains 32 immutable publications for slower clients. Source
map-chain membership, wrapped XY range, forward near-plane and GPU frustum tests
control sprite admission. Directional selectors and animation actions remain
native. A local top-down terrain view supplies the large map.

Indexed tile arrays and a shared sprite atlas render to a GPU palette-index
target. Destination-dependent particle operations use source TABLESN lookup
tables. Screen-disjoint sprites are batched into ordered layers; GPU copies
cover only affected rectangles. No CPU framebuffer readback is used for play.
Only active instance ranges upload. The native update target is 30 Hz; visual
camera interpolation and GPU draws run on browser animation frames.
`originalWorldMetrics` reports real frame intervals, draw calls and triangles;
`originalMetrics.received_hz` measures state delivery separately. The latter is
not maximum GPU FPS. `bytes_received` counts world packets, not total traffic.
Indexed PoC mode retains its source scene-history path.

Focused checks:

```sh
node --test tests/original-world.test.ts tests/original-adapter.test.ts tests/original-hud.test.ts
pnpm typecheck
python3 tests/test_original_assets.py
python3 tests/test_original_world_assets.py
python3 tests/original_bridge_test.py
ORIGINAL_PROOF=/absolute/disk/path DISPLAY=:1 node tests/original-browser.mjs
```

Set `ORIGINAL_URL` to the exact deployed entry for the browser check. It uses
installed Chrome, FFmpeg for capture inspection, and a fresh Playwright profile,
with real HTTP/native calls. Browser plugin not available; Playwright is the
validation fallback. No physical phone GPU has been benchmarked.
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
Mobile emulation exercised real touch events for steering, casting and hand
assignment; a capture check rejects a repeated top HUD in portrait. Browser
request tracing rejects streamed `/frame` or `/world-map` calls in geometry mode.
The final checked 1280×960 sample averaged 56.9 FPS with 16.8 ms p95 on the 60 Hz
desktop; this is not a maximum-throughput or real-phone FPS claim. Native smoke and source
palette were checked against a separate software-rendered tick-100 reference.
The installed native library used for final browser qualification had SHA-256
`889b470467245f07a552d3e67133b3679db62a297adaa9633cbbdfb909a75d39`;
this identifies the tested binary rather than certifying the moving port source.

UBS on the owned source reported zero critical findings. Full-file warnings
include document-lifetime listeners/timer, dense typed arrays labelled sparse,
the already-caught request promise, typed Three.js uniforms, bounded array
accesses and divisions whose dimensions/depths are checked. The launcher uses
`set -e` for its flagged `cd`. These were reviewed; warnings were not suppressed.
Whole-file totals include the existing HUD and are not a new-defect count.

Still incomplete: audio, original options/pause submenus, Beyond Sight labels,
map entity symbols and hand flashes, source shadow/reflection passes and dynamic
palette effects. The source palette matches the normal Night reference, but
field-dependent palette changes have no narrow native export yet. Terrain
waves/fog use local floating-point projection; native fixed-point raster parity
is not claimed. SS2 delta images are decoded from source; their selector-global
animation clock is unavailable, so texture cycling uses a stated presentation
approximation (one transition per four native ticks). Native creature frame
selection remains authoritative. HUD panels are opaque in geometry mode.
This route does not establish a complete mission playthrough or full Original
port completion. Existing Rust gameplay defects remain with the Original team.

## Modern Level 1 assets

Open <http://3090.bumblebee-hue.ts.net:8884/?visual=modern>. This separate service
keeps the Original routes on 8882/8883 and the Unity/Voxel masters intact. It
uses the same native Rust gameplay and source HUD/controls, with local Three.js
PBR meshes, terrain, vegetation, water and night lighting. It needs the backend
on 3090; it is not a standalone WASM game.

The owner-local pack at `../../generated/browser-level1-assets` contains twelve
rigged character GLBs (Goat, Archer, four residents, Builder, Trader, Firefly,
Worm head/segment and Collector), eight world GLBs, source mana colours, terrain
textures, 17 building profiles and seven Castle profiles. Textures are bounded
browser derivatives; approved masters remain unchanged. Reproduce the exports
with `../../character-pipeline/export_browser_characters.py --help` and
`../../character-pipeline/export_browser_world.py --help`. Both are free local
Blender conversions. No paid generation is needed. Generated owner assets must
not be committed or distributed with the source repository.

Build and start a separate Modern client from this directory:

```sh
pnpm exec vite build --mode original --outDir ../../generated/modern-browser-client --emptyOutDir
python3 original_bridge.py --renderer geometry --modern \
  --web-root ../../generated/modern-browser-client --port 8884 \
  --save ../../generated/modern-web-20261116/save.json
```

Append the `--host` and matching `--allow-host` options above for tailnet access.
The installed 8884 service additionally pins `--library` to the already tested
native binary SHA-256 `889b470467245f07a552d3e67133b3679db62a297adaa9633cbbdfb909a75d39`.
It is a transient user service named `magiccarpet-browser-modern.service`.
It loads the native library directly; a Unity process is unnecessary.

Native MCD1 character/building/tree/mana rows and MCW1 geometry must have the
same publication identity. Source controls determine births, actions, shots,
collection and removal. The browser only interpolates positions and drives
rig clips. Native terrain planes are compared byte-for-byte; revisions and
compressed payloads change only when bytes change or reset/load creates a new
epoch. Visual landform/scatter also cache their actual inputs. This removes
repeated terrain reconstruction without reducing geometry or texture quality.
The shoreline uses the source reflective-material table and actual atlas water
pixels, including calm water outside the wave flag. Smoothing and submerged
banks affect presentation only; Rust collision stays authoritative.

Focused Modern checks, in addition to the shared tests above:

```sh
node --test tests/modern-runtime.test.ts tests/modern-characters.test.ts tests/modern-world-objects.test.ts tests/modern-environment.test.ts
MODERN_PROOF=/absolute/disk/path DISPLAY=:1 node tests/modern-browser.mjs
MODERN_PROOF=/absolute/disk/path DISPLAY=:1 node tests/modern-render-profile.mjs
```

The interaction test resets the real session and uses its save slot. It checks
33 native opening Goats, 41 matched architectural buildings, flight/casting,
map, pause, save/load identity and portrait touch controls. The profile check
uses a seven-second warmup followed by a six-second actual animation-frame
sample; `MODERN_CPU=1` adds Chrome CPU sampling. On the 3090 at 1280×960 the
checked opening village sample was 60.0 FPS, 16.8 ms p95/p99 on a 60 Hz desktop.
Startup/reset stalls are excluded from that steady sample; the interaction
run including those stalls averaged about 49–52 FPS. This is not maximum GPU
throughput, a full-mission benchmark, or physical-phone FPS. The shared Original
browser regression also passes against this integrated source build.

Live close-ups covered moving Goat/Villager meshes, masonry, roof surfaces and
lit windows. Archer bow morphs and the arrow shaft/release were inspected with
the actual loaded rig in a staged art check; that is not native Archer firing
proof. `inspect=Goat`/`inspect=Villager` and `inspectBuilding=1` are read-only
art-review cameras; `modernMetrics.camera_mode` labels these inspections.

Still open: native-spawned Archer/Firefly/Worm/Collector visual qualification,
per-hoof contact IK, dynamic Castle/construction placement, live mana collection,
tree burning and low shoreline foundations. The full building library is
prepared, but only the 41 unchanged opening stamps are admitted by the current
building presenter. Water/cloud shaders are inexpensive browser presentation,
not parity with Unity's spectral-water/steam system. Shared audio/menu omissions
and unfinished Original gameplay remain open. No complete Level 1 mission or
finished Voxel browser consumer is claimed.
