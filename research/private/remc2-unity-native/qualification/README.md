# Level 1 playable-slice qualification

This directory registers the private, owner-local reference corpus for
`mc-dl0r.17.11.7`. The machine-readable contract is
`level1-reference-corpus.json`.

## Why there are three different routes

“Deterministic” and “matches remc2” are not synonyms.

1. `level1-idle-full-frame` is original pinned-remc2 evidence. It binds exact
   state, terrain memory, and complete 320×200 indexed framebuffer bytes at
   reference frames 0, 8, and 19. The package reset is frame 0.
2. `level1-native-eight-forward` is active flight evidence. A private native
   probe injected one forward command from the same Level 1 reset seam and
   observed all eight player passes. Rust and the native ABI already reproduce
   those flight fields; there is no matching original framebuffer claim for
   this probe.
3. `unity-synthetic-presentation-route` is our longer moving-view regression.
   Its ticks 145, 163, 220, 303, and 400 exercise terrain, water, reflection,
   fire events, HUD, actors, and audio. It is deliberately labeled
   `viewer_regression_only`, because those commands and frames were not
   captured from remc2.

The registry also binds the separately qualified first-presented-frame trace.
That trace supplies the exact reset camera pose and the ordered 76-entry final
render event-set identity. It is not relabeled as evidence for the synthetic
440-step route.

The preserved `afterloadtest2` recording supplies another 24 exact
raw-command-to-`FlightState` transitions. It is supporting simulation
evidence, not a Unity start state.

The native package reset exactly matches original idle frame 0. Neutral native
replay also matches the original source projection at frames 8 and 19. Its
ordinary live hash differs because it includes the additive native session
tick, a field absent from the original game state. The manifest records both
hash contracts and the verifier requires the source-normalized comparison.

## Clock rule

Every result names its clock:

- `reference_frame` is an index in the captured remc2 sequence;
- `source_turn` is the turn stored in that original frame;
- `native_step_count` counts completed native session steps from package reset;
- `native_render_tick` is the stored pre-entity-update camera row;
- `unity_checkpoint_tick` is the frozen post-step checkpoint captured at
  `WaitForEndOfFrame`.

After a native step checkpoint `N`, the render camera is intentionally from
tick `N-1`. A verifier must compare that pair explicitly and must never
silently normalize the offset.

## Comparison policy

The original framebuffer is a side-by-side reference. It is not a golden hash
for the Unity window. The current source-mapped native indexed world layer contains
sky, terrain, and the qualified Goat, Archer, Firefly, combat, objective-prop,
castle, mana-carrier, and mana-sphere families in one source-slot compositor.
Its supported families use the remc2 reflection-before-primary and
terrain/entity owner interleave. Unity adds the adapted live HUD. The source
projection scale and 4:3 display rectangle are present. Villagers and dolmens
now share the native painter, reflection, and shadow path; the carpet,
unsupported sprite raster modes, and some animation clocks remain absent or
deferred.

The qualification therefore has separate gates:

- exact original bytes and exact projected flight fields;
- source-derived camera and typed event behavior;
- structural presentation correspondence by palette/material/TMAP/HUD/audio
  identity;
- deterministic Unity-only pixels and lifecycle;
- an explicit differences register.

Passing one category never upgrades another.

## Executable route gate

`verify_reference_corpus.py` checks fixture slices and the eight original
forward-pass projections, then launches the current qualified player headlessly.
The player executes all 440 synthetic route commands in two independent
sessions, resets and replays one session, rejects its stale generation, and
emits five canonical records. The verifier binds each record back to the
manifest:

- command bytes and prefix hash;
- post-step state and conformance hashes;
- entity/profile identity and the package-bound reset source turn;
- generation, event cursor range, normalized event hash, and
  generation-sensitive event hash;
- stored pre-update presentation and render-camera rows;
- terrain generation, dimensions, stride, viewport, frame hashes, triangle
  modes, and pixel-write count.

This proves the deterministic native/Unity route described by the registry. It
does not turn that route into a remc2 or DOS capture.

## Presentation correspondence gate

`presentation-correspondence.json` is the typed presentation contract. It
defines four independent authority timelines, 23 classified layers, explicit
structural-only comparisons, and forbidden claims. In particular:

- the original frames 0, 8, and 19 remain exact indexed bytes on the remc2
  hook clock;
- their review PNGs are adapted base-PALN structural decodes because the
  active DAC palette was not captured;
- Unity checkpoints 145, 163, 220, 303, and 400 remain a separate synthetic
  route and are exact only against the same-build Unity regression;
- the controlled eight-view entity gallery is cache-bound debug evidence,
  has no simulation clock, and is never called an original or qualified view;
- unsupported sprite raster modes, remaining animation clocks, native carpet
  presentation, multiplayer minimap reveal, and wider simulation remain
  deferred. The source-mapped transient palette, 4:3
  projection, HSPRN HUD,
  Level 1 minimap entity/mark pass, and live indexed objective textbox are
  registered adapted layers. The textbox preserves exact source geometry,
  33-turn timing, TABLESN selector-61 blending, HFONT3 text, and HSPRN frame
  order; only the pixel-centre-nearest 320×200-to-640×480 display mapping is
  adapted.

Materialize the private viewer only under a dedicated empty temporary
directory:

```bash
python3 qualification/materialize_presentation_correspondence.py \
  /tmp/magiccarpet-presentation \
  --unity-captures /tmp/magiccarpet-level1-qual-v5-a
```

Then run the composed gate:

```bash
python3 qualification/verify_presentation_correspondence.py \
  /tmp/magiccarpet-presentation \
  --unity-captures /tmp/magiccarpet-level1-qual-v5-a \
  --second-unity-captures /tmp/magiccarpet-level1-qual-v5-b \
  --unity-log-a /tmp/magiccarpet-autoplay-v5-a.log \
  --unity-log-b /tmp/magiccarpet-autoplay-v5-b.log \
  --entity-gallery /tmp/magiccarpet-entity-gallery-v5.png \
  --gallery-log /tmp/magiccarpet-entity-gallery-v5.log
```

The composed gate re-runs the corpus and cache checks; proves the two Unity
runs are byte-identical; verifies HUD and cache-bound owner TMAP gallery
pixels; requires clean qualified-mode runtime markers from both logs; rejects
visible placeholder selectors; and writes
`presentation-qualification-result.json`. Its final marker deliberately says
`original_indexed=3` and `unity_registered=5`, never the ambiguous
`exact=5`.

The focused negative-claim suite is:

```bash
python3 -m unittest discover \
  -s qualification/tests -p 'test_*.py' -v
```

It rejects cross-timeline pixel equality, numeric-clock pairing, exact-RGB
claims for the base-PALN review decode, exact hotspot claims, present omitted
proxies, and unregistered layers.

## Unity lifecycle, bulk ABI, and privacy gate

The production scripts compile into `MagicCarpet.Native.Runtime.dll`; test
code remains in separate EditMode and PlayMode assemblies and is not included
in the player. The focused suite is:

- twenty-eight Linux EditMode native-integration tests for ABI/layout, exact input and
  camera routing, reset/generation behavior, two-session isolation,
  idempotent disposal, bad owner inputs, preallocated bulk copying, and the
  qualified selector-clock seam, live Level 1 actor/exit exposure,
  session-local terrain/lifecycle hashing and reset, owner-cache identity,
  ordinary presentation ownership, and the input-only post-completion combat
  route with no-overlap, incomplete-multipart, and insufficient-combat
  negative controls;
- one graphics-backed PlayMode test for a clean qualified startup, tick
  advancement, absence of diagnostic physics/light/carpet/fire proxies, and
  complete session disposal.

The bulk test performs 32 terrain-grid plus 320×200 terrain-frame checkpoints
after warmup. The caller-owned buffers and the session-owned 840-vertex array
must be reused; the current qualified measurement is zero managed bytes.
Elapsed time is reported for diagnosis but is not a correctness threshold.

Run the tests sequentially:

```bash
/home/oliver/Unity/Hub/Editor/6000.3.20f1/Editor/Unity \
  -batchmode -nographics -projectPath "$PWD" \
  -runTests -testPlatform EditMode \
  -assemblyNames MagicCarpet.Native.EditModeTests \
  -testResults /tmp/magiccarpet-unity-editmode.xml \
  -logFile /tmp/magiccarpet-unity-editmode.log

xvfb-run -a -s "-screen 0 1600x900x24" \
  /home/oliver/Unity/Hub/Editor/6000.3.20f1/Editor/Unity \
  -batchmode -projectPath "$PWD" \
  -runTests -testPlatform PlayMode \
  -assemblyNames MagicCarpet.Native.PlayModeTests \
  -testResults /tmp/magiccarpet-unity-playmode.xml \
  -logFile /tmp/magiccarpet-unity-playmode.log \
  -mc-qualified \
  -mc-package /absolute/path/to/level1-export-m \
  -mc-closure /absolute/path/to/level1-asset-closure.json \
  -mc-asset-cache /absolute/path/to/level1-unity-asset-cache
```

Do not add `-quit` to Unity Test Framework commands. Then run the composed
player-boundary gate:

```bash
python3 qualification/verify_unity_lifecycle_privacy.py \
  --edit-results /tmp/magiccarpet-unity-editmode.xml \
  --edit-log /tmp/magiccarpet-unity-editmode.log \
  --play-results /tmp/magiccarpet-unity-playmode.xml \
  --play-log /tmp/magiccarpet-unity-playmode.log
```

It requires exactly 29/29 and 1/1 tests; pins the rebuilt launcher, runtime
assembly, global managers, plug-in, and build GUID; inspects Unity build
metadata; rejects StreamingAssets, symlinks, owner container/audio
extensions, and registered owner-payload hash intersections; reruns the full
built-player self-test; and exercises seven isolated negative launches.
Missing inputs and package/closure/cache corruption all exit 2 without a PASS.
The ordinary graphical missing-input case is included, so a failed player can
no longer remain as a frozen window.

## Provenance and privacy

All referenced regression sequences, recordings, screenshots, hashes, source
addresses, owner assets, and exact-build details remain inside this private
lab. The Unity player receives package, closure, and cache paths at runtime;
the corpus is never embedded into the player or copied to the commercial
successor.
