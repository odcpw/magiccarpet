# Original campaign in the browser

The native Rust runtime owns gameplay, input admission, RNG, scheduling and
save/load. `original_bridge.py` loads the installed `sim-capi` library and starts
at NightSpire. It uses the Original launcher's 25 ordinary and five secret
mission packages. Campaign progress unlocks later missions. Use `--packages`
to select a different installed package set for a specific replay. The default route
now renders the terrain, source-selected original sprite frames, water motion
and indexed particle blends locally with Three.js/WebGL2. The browser also
composes the source HUD, spell grid, objectives and minimap. There is no Unity
process, streamed world image, or second gameplay simulation in the default route.

## Private NightSpire WASM playtest

This opt-in route runs the same package-backed Rust runtime in a browser worker.
The existing Three.js client receives guarded world, HUD and input data from the
worker. The worker uses the WASI in-memory filesystem to load installed packages
through the native loader. The initial package download requires the static host.
The one-mission build supports flight, casting, maps, pause and restart. Audio,
save/load and continuing to another mission use the server-backed route.

Build from the `web` directory with its locked dependencies installed:

```sh
rustup target add --toolchain 1.98.0 wasm32-wasip1
cargo +1.98.0 build --locked --manifest-path ../Cargo.toml -p sim-capi --release --target wasm32-wasip1
target=../target/wasm32-wasip1/release
output=../../generated/private-wasm-playtest
python3 prepare_wasm_playtest.py "$output/original-api"
cp "$target/sim_capi.wasm" "$output/original-api/"
pnpm exec vite build --mode original --outDir "$output"
python3 serve_wasm_playtest.py "$output"
```

Open `http://127.0.0.1:8892/original.html?runtime=wasm`. The output includes
owner-local data and stays private. Use a fresh generated directory for each
build. A custom `CARGO_TARGET_DIR` changes the source path for `sim_capi.wasm`.

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

The launcher reads `research/private/generated/original-browser-native/libsim_capi.so`
when a browser native release is installed. Otherwise it uses the installed
Original Unity plugin. Pass `--library PATH` to select a specific qualified
build. Geometry rendering requires the campaign palette export shipped at
`5d1a5546` on `codex/original-ending`.

Use **Speech** on the opening or pause screen to turn spoken objectives on
or off. The setting takes effect on the next cue and persists across launches.
By default the browser stores its player preferences under
`research/private/generated/original-web-20261116/profile`. Pass
`--profile DIRECTORY` to use another profile. The campaign save has its own path.

### Open an Original mission replay

Use the native-backed geometry renderer with the same installed package set,
authored workspace and native revision that accept the replay. For example,
from this directory on 3090:

```sh
./run-original.sh --port 8891 \
  --packages ../../generated/three-exit-20261124/resources.json \
  --replay ../../generated/original-exit-hint-20261124/source4-first-visit-t301.mcs \
  --help-cache ../../generated/exit-help-row83-20261124-candidate \
  --save ../../generated/three-exit-20261124/browser-slot.mcs
```

The runtime validates the replay. The bridge matches its campaign package
hash to the owner-supplied package list and loads that mission's terrain,
sprites, palette and map symbols. The replay file is read-only. Choose
**Save** in the utility menu to write to the separate save slot. To open a
different mission's replay, use the same command with its matching resources.

At an earned ordinary exit, let the exit animation finish. Results shows the
five statistics from the Rust campaign. Choose **Continue**, select an unlocked
mission, then choose **Enter mission**. The browser loads the selected mission’s
terrain and sprites. Rust carries the player’s learned spells and experience
into that mission. Opening an earned Results or mission-selection save starts
at that screen. The browser uses text and a mission picker for these screens.

In T’Klom, open the map with Tab and click or tap the exit symbol to inspect
its English help text. Clicking again or closing the map dismisses it.
The full map uses the native marker positions and current terrain. The
browser draws the map and the three-dimensional flight view locally. This
interaction is specific to the browser; the Unity automatic help popup has
its own admission and lifetime. The top status panel still uses the
experiment's original Night HUD artwork.

On November 24, Chrome on the RTX 3090 loaded the controlled first-visit
T’Klom replay with native `0fc9d27e2b46`. The live slot 298 marker appeared
on the full map. Clicking it displayed “Fly here to exit the level” in the
source font with a pointer to its symbol. Closing the map cleared the hint.
The browser requested geometry, terrain and stamped state. Captures are in
`../../generated/three-exit-20261124/`. This was a map inspection from a
saved state; a complete mission playthrough is still open.

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

Original uses the four movement keys saved in the native profile. By default
Up/Down controls speed, Left/Right strafes and the mouse steers. W/S and A/D
also work for their default actions when those letters have no other binding.
The touch pad controls speed and turns independently of keyboard preferences. The
mouse buttons cast from their assigned hands, and the touch Cast button casts the
selected spell. In Original, Space requests recovery after defeat. Hold the
saved spell-menu key (LeftControl by default) to open the spell grid and click
to assign a tier to a hand. LeftShift or the saved mark key plus a click marks
a spell for that hand. In flight, LeftShift or the saved next key plus a click
advances through its marks; LeftAlt or the saved previous key goes back. O shows
the objective. Press the configured map key (Return by default), or press both
mouse buttons together, to toggle the map. Tab also works while Return remains
the configured map key. On the map, hold LeftAlt, the saved information key, or
the saved previous key for player information. Esc opens the abandon
confirmation and P pauses. F5 toggles sound effects, F6 toggles music
and F9 loads the save slot. The utility menu provides resume, save, load,
restart and fullscreen. Clicking the view captures
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

Beyond Sight uses the Rust session's wizard visibility, coordinates and colours
on both maps. The browser draws each name beside its dot using the selected
environment's FONT1. In the paid Perilium save, the full map displayed Nyphur,
Rahn and Belix. All 142 sampled glyph pixels matched the owner masks and
palette, and the names disappeared when the spell expired at tick 184.

The flight audio player reads source voice commands and samples from the
selected mission bank. One paid Fireball in Payahandra produced the launch
and impact waveforms in Chrome's Web Audio output. Flight music streams the
owner recording for the active Night, Day or Cave bank. The Rust music clock
drives its volume envelope. In NightSpire, Chrome played the idle-to-combat
rise and return to idle. Pausing stopped playback; resuming restarted it.
Chrome also played the Cave track. The Day track passed native and asset
checks. Physical speaker output remains unverified.

The full map briefly shows the chosen hand after an accepted marked-spell
cycle. In Payahandra the left Fireball panel appeared and cleared with its
eight native countdown steps. The remaining original options, pause submenus
and source shadow/reflection passes are still open in this client.
The geometry renderer reads
the current native campaign palette for both world and HUD. On a saved
Payahandra entry, its objective flash and fade matched the indexed renderer
through recovery in Chrome. Terrain
waves/fog use local floating-point projection; native fixed-point raster parity
is not claimed. SS2 delta images are decoded from source; their selector-global
animation clock is unavailable, so texture cycling uses a stated presentation
approximation (one transition per four native ticks). Native creature frame
selection remains authoritative. In Original geometry mode, the minimap uses
the mission's panel setting and blends translucent pixels against the world
using the original palette table.
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
The installed 8884 service is a transient user service named
`magiccarpet-browser-modern.service`. It loads the installed native library
directly. The November 19 lighting comparison used native SHA-256
`327b91e693efe5ef6fc72601e0b2af76ea6be10c4908dff727569b6e6ae3dddc`
for both builds.

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

### Rendering ideas to compare across clients

[Dimillian's notes](https://x.com/Dimillian/status/2123333836268294258) are a
reference for the Three.js, Unity Modern and Voxel visual comparisons.

| Technique | What to compare |
| --- | --- |
| PBR, normal maps and bump maps | Start with material response under the same lighting. Three.js already uses PBR and terrain normal maps. Try bump detail where the texture calls for it. |
| MSAA, FXAA, TAA and temporal upsampling | Three.js uses 2× MSAA. Compare foliage shimmer and motion stability at matched resolution before trying temporal AA or upsampling. FXAA is a cheaper edge pass to compare on phones. |
| HDR, exposure and bloom | Compare warm lights and readable shadow detail without bleaching the night sky. Three.js adds bloom in HDR before tone mapping and composites the HUD afterward. |
| Camera and lens effects | Test stylistic grades, grain and similar effects in separate captures. Keep landmarks and controls readable in motion. |
| Planar reflections | Compare coastal flybys. Three.js renders a low-resolution view of cliffs, buildings and sky when water is in view. Use `reflections=0` to check the same scene with that pass disabled. |
| Volumetric lighting | Try local fog and shafts where geometry and illumination make their depth apparent. Compare their cost in a moving scene. |
| Baked global illumination | Try baked light on static walls and terrain. Check how it holds up when buildings or terrain change. |
| Parallax occlusion mapping | Reserve for prominent materials seen close up. Compare against simple modeling of silhouettes, sills and roof edges. |

The browser village now uses recessed glazing, projecting sills and roof trim.
Nearby windows, observed fire and authored torch emitters share eight local
lights. Fireballs and mana feed the same bloom pass as windows. The reflection
scene shares static mesh geometry and uses separate materials and lights so its
camera and clipping cannot affect the main pass.

The November 19 comparison used `MODERN_FLY=1` with the browser profile command
above. Both builds ran at 60.0 FPS with 16.8 ms p95 and p99 during a six-second
forward flight at 1280×960 on the 3090's 60 Hz desktop. Portrait controls also
passed in emulation. The opening run contained windows and a cast fireball.
Authored torches and mana spheres still need a natural gameplay sighting in
this client.
