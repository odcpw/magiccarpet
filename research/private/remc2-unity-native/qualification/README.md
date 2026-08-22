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

## Neutral GameSession client gates

The native candidate is always supplied explicitly. The Python black-box
consumer is the smallest F4/F5 domain check:

```bash
python3 qualification/verify_neutral_game_session.py \
  --plugin /absolute/path/to/candidate/libsim_capi.so
```

The runner opens the real owner definition through `mc_game_session_*_v1`,
submits semantic movement/casting JSON for the fixed 440-command route, pages
semantic events, reads a logical audio sidecar, resets, and replays. Canonical
snapshots and opaque MC2-owned compatibility receipts must equal the existing
F1 state, event, and audio identities. It also rejects stale generation/tick
guards and source-shaped command fields.

The F5 managed and headless TypeScript consumers must use that same plugin:

```bash
python3 qualification/run_engine_session_editmode.py \
  --project "$PWD" \
  --plugin "$PWD/Assets/Plugins/x86_64/libsim_capi.so" \
  --results /tmp/magiccarpet-engine-session-editmode.xml \
  --log /tmp/magiccarpet-engine-session-editmode.log

cd ../remc2-sim-rs/web
pnpm typecheck:contract
MC_GAME_SESSION_PLUGIN=/absolute/path/to/libsim_capi.so pnpm test:contract
```

The Unity runner requires a fresh non-empty report containing only
`EngineSessionEditModeTests`, and verifies that the installed project plugin is
byte-identical to `--plugin`. The TypeScript client spawns the strict NDJSON
host over the same explicit library; it does not use the parked synthetic Wasm
simulation. Candidate behavior is compared with the frozen corpus. Updating a
current-source plugin identity after an additive build requires an explicit
`GOLDEN-CHANGE` provenance note and unchanged fixed observations; it never
authorizes regenerated gameplay, event, or audio expectations.

The F6 one-factor semantic flight gate authors an immutable workspace through
the real `sim-cli game set/preview/apply` transaction, then opens baseline and
variant sessions through the same native plugin in Python, TypeScript, and
Unity:

```bash
python3 qualification/verify_semantic_flight.py \
  --plugin ../remc2-sim-rs/target/release/libsim_capi.so \
  --sim-cli ../remc2-sim-rs/target/release/sim-cli
```

Only `flight.player.max_forward_speed:80->16` may differ in the definition.
The gate requires ordinary Rust target speed, actual speed, and displacement
to change while tick, mana, objective count, action sequence, semantic event
order, reset replay, and the fixed F1 corpus remain unchanged. This proves one
authorable flight field, not live next-tick mutation or complete flight tuning.

The F7 one-factor Fireball gate uses the same immutable workspace and exact
candidate plug-in route:

```bash
python3 qualification/verify_semantic_fireball.py \
  --plugin ../remc2-sim-rs/target/release/libsim_capi.so \
  --sim-cli ../remc2-sim-rs/target/release/sim-cli
```

Only `spell.fireball.mana_cost:100->200` may differ. Python, TypeScript, and
Unity must observe the ordinary accepted-cast debit change from 900 to 800 mana
while projectile identity/trajectory, Rust RNG, semantic event order,
unrelated snapshot fields, and reset replay remain unchanged. Wrong types,
negative values, client-supplied units, unsafe timings, stale definitions, and
read-only surfaces fail closed. This proves one authorable spell rule, not all
spell families or live mid-session mutation.

The F8 one-factor enemy-policy gate publishes the bounded compiled Goat policy
through the same immutable workspace and exact candidate plug-in route:

```bash
python3 qualification/verify_semantic_goat_policy.py \
  --plugin ../remc2-sim-rs/target/release/libsim_capi.so \
  --sim-cli ../remc2-sim-rs/target/release/sim-cli
```

Only `ai.enemy.default.policy:classic_goat_v1->vigilant_goat_v1` may differ.
Python, TypeScript, and Unity must observe non-empty opaque
`ai.engagement_boundary` events from the ordinary owner-state Goat scheduler;
the classic definition must produce none on that pass. Unrelated event kinds,
player state, mana, objectives, action sequence, reset replay, and the fixed F1
corpus remain unchanged. The Rust lifecycle test additionally rejects the
wrong action/stage phase and missing, hidden, drawing-disabled, or stale player
targets on the following retaliation pass. This proves one bounded compiled
policy selection, not arbitrary AI code injection or complete enemy authoring.

The F9 asset gate compiles two independently authored synthetic indexed
sprites through the deterministic source/recipe transform, publishes the
second retained content hash into the existing Fireball presentation slot,
and reads the typed payload through the neutral session contract:

```bash
python3 qualification/verify_semantic_presentation_asset.py \
  --plugin ../remc2-sim-rs/target/release/libsim_capi.so \
  --sim-cli ../remc2-sim-rs/target/release/sim-cli
```

Rust validates the exact kind, runtime format, dimensions, frames, flattened
RGBA8 palette, animation timing, coordinates, origin, artifact dependencies,
and engine-capped budgets. Python proves asset polling is observational and
gameplay state/events are unchanged; TypeScript invalidates its decoded RGBA
cache; Unity replaces an actual point-filtered `Texture2D`; and a forward
rollback at revision 3 restores the retained baseline binding. The candidate
plugin and both clients are identical across baseline and variant. Package
tamper/path/hash failures and malformed typed assets fail before publication.
This proves one bounded 2D presentation slot, not generated-art quality,
animation authoring, 3D structures, or a complete production asset pipeline.

The F10 foundation gate composes the four accepted changes in the same
redistributable `custom.synthetic` package and runs MC2-owned, the unchanged
custom baseline, four one-factor definitions, and the combined definition
through one exact native plug-in and one shared command trace:

```bash
mkdir -p qualification/artifacts
python3 qualification/verify_custom_package_foundation.py \
  --plugin ../remc2-sim-rs/target/release/libsim_capi.so \
  --sim-cli ../remc2-sim-rs/target/release/sim-cli \
  --output qualification/artifacts/foundation-integrator.json

python3 qualification/verify_custom_package_foundation_evidence.py \
  --report qualification/artifacts/foundation-integrator.json \
  --output qualification/artifacts/foundation-independent.json
```

The integrator requires exact one-factor causality and countermetrics,
combined composition, reset/replay, 0/1/7 polling invariance, a seven-file
package whitelist, no production package-ID branch, malformed/stale/tampered/
partial refusal with unchanged roots, normalized native/Unity/TypeScript
parity, the fixed Level 1 corpus, and the four-layer compatibility guard. It
emits one result for every atomic requirement in the engine plan and fails if
any requirement is uncovered. The independent verifier reopens the retained
workspaces, reruns all three clients and the focused Rust/CLI suites, rechecks
the fixed corpus and compatibility guard, and recomputes executable and
package hashes.

The current custom run is Unity EditMode evidence using the exact runtime and
test assemblies. The older owner-local player remains separately inventoried
and fixed-corpus-qualified; it is not mislabeled as having run the custom
package. Passing F10 proves four bounded extension surfaces over one engine,
not whole MC2, arbitrary games, production generated assets, VR, voxels,
cities, water, multiplayer, or commercial release.

## Presentation correspondence gate

`presentation-correspondence.json` is the typed presentation contract. It
defines five independent authority timelines, 23 classified layers, explicit
structural-only comparisons, and forbidden claims. In particular:

- the original frames 0, 8, and 19 remain exact indexed bytes on the remc2
  hook clock;
- their review PNGs are adapted base-PALN structural decodes because the
  active DAC palette was not captured;
- Unity checkpoints 145, 163, 220, 303, and 400 remain a separate synthetic
  route and are exact only against the same-build Unity regression;
- the controlled eight-view entity gallery is cache-bound debug evidence,
  has no simulation clock, and is never called an original or qualified view;
- the fifth timeline is executable fixed-profile native FINAL MAIN
  correspondence, not a capture pairing. It binds the pinned source anchors,
  CP4 cache/source-frame census, source tuple and linked map chain, exact
  hotspot/clipping and reflection/shadow order, dynamic lighting,
  temporal/spatial postprocess, supported overlay receipts, atomic finalization,
  revisioned C ABI, and current-COMPLETE-gated one-R8-plane Unity ownership;
- its Exact claim covers supported COMPLETE states only. Active
  tutorial/briefing, recursive help popup, immobilization spider-web, and
  menu-state-7 score-writer states remain reachable but fail closed before
  COMPLETE;
- unsupported source branches and animation modes, native carpet
  presentation, multiplayer reveal, stereo/alternate resolution,
  hostile/remote players, and wider simulation remain excluded. Active-DAC
  RGB for historical captures and all cross-timeline pixel/clock equality
  remain explicit nonclaims. The source-mapped transient palette, 4:3
  projection, HSPRN HUD,
  Level 1 minimap entity/mark pass, and live indexed objective textbox are
  registered adapted layers. The textbox preserves exact source geometry,
  33-turn timing, TABLESN selector-61 blending, HFONT3 text, and HSPRN frame
  order; only the pixel-centre-nearest 320×200-to-640×480 display mapping is
  adapted.

The following two-directory comparison is retained for optional historical
presentation determinism review. It is not the PLAN29 acceptance count;
PLAN29 accepts exactly one clean Run A as documented below.

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
  --owner-cache /home/oliver/Projects/odcpw/magiccarpet/research/private/generated/level1-unity-asset-cache-plan29-final \
  --unity-captures /tmp/magiccarpet-level1-qual-v5-a \
  --second-unity-captures /tmp/magiccarpet-level1-qual-v5-b \
  --unity-log-a /tmp/magiccarpet-autoplay-v5-a.log \
  --unity-log-b /tmp/magiccarpet-autoplay-v5-b.log \
  --entity-gallery /tmp/magiccarpet-entity-gallery-v5.png \
  --gallery-log /tmp/magiccarpet-entity-gallery-v5.log
```

The composed gate requires the qualified native cache explicitly (or through
`MC_OWNER_ASSET_CACHE`) and verifies its manifest, content, 1,249-output count,
source-frame census, and executable Rust/Unity test inventories before any
native FINAL MAIN claim. The separate historical gallery check remains bound
to the registered 570-output managed-gallery cache and is never reused as the
native cache. The gate also re-runs the corpus checks; proves the two Unity
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
claims for the base-PALN review decode, substituted hotspot targets, missing
FINAL MAIN components or COMPLETE flags, `NON_TERRAIN_OMITTED` acceptance,
non-atomic failure semantics, duplicate Unity planes, missing profile
exclusions, present omitted proxies, and unregistered layers.

## Current Level 2 EditMode development lane

Use the focused current Level 2 command for ordinary iteration. From the
Unity project root, it selects the real `NativeCampaignSessionEditModeTests`
class through Unity's test filter, requires a fresh non-empty EditMode result,
and fails if any selected case is unrelated or non-passing:

```bash
python3 qualification/run_level2_editmode.py \
  --project "$PWD" \
  --results /tmp/magiccarpet-level2-editmode.xml \
  --log /tmp/magiccarpet-level2-editmode.log
```

The report accepts legitimate added Level 2 tests and does not pin the
assembly's global test count. At this revision the selected class contains 19
tests; that number is observational, not an acceptance contract. This lane
proves only the current managed Level 2 EditMode/production-path selection.
It does not run the built-player privacy gate or qualify the historical Level
1 release. A future Level 2 PlayMode class remains a separate explicit
selection rather than being swept into this command.

## PLAN29 archival/distributable milestone: Unity lifecycle, bulk ABI, and privacy gate

The production scripts compile into `MagicCarpet.Native.Runtime.dll`; test
code remains in separate EditMode and PlayMode assemblies and is not included
in the player. The archival/release suite is:

- thirty-one archival PLAN29 Linux EditMode native-integration tests for ABI/layout, exact input and
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
  -testFilter MagicCarpet.Native.Tests.NativeSessionEditModeTests \
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
  --release-milestone \
  --edit-composite /absolute/path/to/editmode-composite.json \
  --edit-log /tmp/magiccarpet-unity-editmode.log \
  --play-results /tmp/magiccarpet-unity-playmode.xml \
  --play-log /tmp/magiccarpet-unity-playmode.log
```

The composite receipt is archival PLAN29 evidence only. It is permitted only
when one full 31-case XML and one focused XML prove the exact failed-case
replacement set. It rejects missing, duplicate, or unrelated focused cases and
binds both XML files plus the native plug-in, runtime source, and EditMode
source hashes. It does not rerun cases that are already green and is never
compared with the evolving current Level 2 assembly.

The privacy gate is distributable-milestone-only. It requires exactly 31/31
archival EditMode and 1/1 PlayMode tests; pins the rebuilt launcher, runtime
assembly, global managers, plug-in, and build GUID; inspects Unity build
metadata; rejects StreamingAssets, symlinks, owner container/audio
extensions, and registered owner-payload hash intersections; reruns the full
built-player self-test; and exercises seven isolated negative launches.
Invocation requires `--release-milestone`, and missing inputs plus
package/closure/cache corruption all exit 2 without a PASS. The ordinary
graphical missing-input case is included, so a failed player can no longer
remain as a frozen window.

## PLAN29 final closure (archival/release-only)

The final qualification policy requires exactly one clean end-to-end run:
run A. Run B is neither required nor permitted. The cache remains sealed to
the reviewed behavioral closure contract; the separate qualification policy
overrides only that contract's historical clean-run count.

To execute a fresh candidate:

```bash
python3 qualification/run_plan29_full_gate.py \
  --exact-runs 1 \
  --output-root /absolute/new/plan29-final-root
```

This 75+ minute route is the intentional Level 1 preservation/distributable
milestone command, not a Level 2 iteration command. It runs the explicit
Level 1-only filter above and invokes the privacy gate with the
`--release-milestone` confirmation.

To finalize preserved run-A checkpoints after targeted failure resumes without
replaying green suites:

```bash
python3 qualification/run_plan29_full_gate.py \
  --exact-runs 1 \
  --resume-run-root /absolute/plan29-final-root/run-a
```

## Provenance and privacy

All referenced regression sequences, recordings, screenshots, hashes, source
addresses, owner assets, and exact-build details remain inside this private
lab. The Unity player receives package, closure, and cache paths at runtime;
the corpus is never embedded into the player or copied to the commercial
successor.
