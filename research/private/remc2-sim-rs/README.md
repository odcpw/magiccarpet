# Private remc2 atlas-conformance simulator

This owner-local Track P workspace incrementally models the reconstructed MC2
simulation in deterministic Rust. It is private preservation research, not the
commercial successor and not a distributable MC2 port.

The workspace has five deliberately separate parts:

- `atlas-registry` embeds and validates all 24 generated semantic-atlas CSV
  tables: 14,496 keyed rows spanning the complete function, state, claim,
  dispatch, action, constructor, spell, creature, asset, gap, evidence,
  system-surface, and tick-pipeline mappings. Every row remains visible even
  when it is not implemented.
- `sim-core` contains renderer-free integer simulation, exact legacy terrain
  sampling, typed cross-entity projections, and the exact isolated Level 1
  indexed sky/terrain software-raster path.
- `sim-cli` runs mapping audits and deterministic synthetic traces.
- `sim-capi` exposes a scalar owner-local ABI for the native Unity presentation
  harness without duplicating simulation logic.
- `sim-wasm` and `web/` preserve the earlier owner-local browser proof, but
  are not the active client target now that the bounded source-fidelity gate
  has closed.

Production `sim-capi` contains no owner regression recordings. Its 16 legacy
scalar exports remain for ABI compatibility, but require explicit private
fixture paths in `MC_LEGACY_STATE_SEQUENCE` and
`MC_LEGACY_TERRAIN_SEQUENCE` before first use. Missing, unreadable, or invalid
fixtures are contained by the legacy boundary and return the documented zero
sentinel. Compile-time fixtures exist only under `cfg(test)`. The 49
package-backed v2 exports never consult these variables.

Exact source identifiers, addresses, hashes, tables, and mappings in this
workspace must not enter `/home/oliver/Projects/odcpw/carpet-flight` or any
distributed build. Only a separately reviewed, independently worded behavior
specification and synthetic tests may cross the Track P/Track S boundary.

## Current milestone

The first slice maps class 3, action physical slot 0 and its active
flight/terrain dependencies as `Partial`. It now includes the exact source
angle/root tables, recovery, tether, collision spell-holder mutation, and
original terrain-array interpolation. Player settings are generated from the
exact outdoor/cave rows selected by `AddPlayer_4A920`, the outer scheduler
implements the source's 1/4/8/0 pass selection, and deterministic flight
audio/music effects are emitted as a renderer-neutral event journal. The
three fire inputs likewise emit typed spell-activation requests in the exact
left/right/selected dispatch order, before the flight-skip branch. The
downstream `sub_5F660` state machine is reconstructed separately and executed
synchronously against an alias-preserving holder registry, including its
model gates, timers, direction bits, pointer-state clear, help, and sound
effects. Movement opcode `6` resolves selected-spell changes before firing
through the pinned 29-entry source map. The authoritative projection also
includes the target-speed-change word cleared/set by `sub_5F380`; water
redirection truncates 3D distance to the source low word, and cave
ground/ceiling repair preserves the original mutually exclusive ordering. It
also
decodes the preserved original
terrain/state regression images, parses both preserved recording variants, and
reproduces 475 idle transitions across all 25 levels plus five consecutive
raw-command-to-state outdoor acceleration transitions exactly as part of a
24-transition recorded run, plus 19 transitions from a preserved mid-game
save. The in-tree total is 518 exact whole-projection transitions, with two
additional pinned-native active checkpoints for tether and fire activation.
The declared bounded class-3/action-0 simulation gate is closed; this is not a
claim about the complete MC2 engine or presentation. Pinned-native
intermediate checkpoints cover every pass of both the 4/8-pass schedulers. The
registry currently accounts for 43 `Partial`, 359
`StructuralOnly`, and 14,094 `MappedOnly` rows. No mapping is labeled
`Implemented` or `Verified`.

The private Level 1 renderer now closes the fixed 320×200 Night,
screen-size-0, single-local-player FINAL MAIN. Beyond exact sky and terrain,
it classifies the complete reachable source-tuple/TMAP census, retains linked
map-chain order, draws reflections before primaries and each terrain shadow
immediately before its sprite, applies normal/special hotspot and clipping
math, advances source-ordered dynamic lighting, executes the exact temporal
or spatial TABLESN branch, consumes the supported overlay writer tree, and
atomically commits a receipt/provenance-bound FINAL MAIN. The revisioned C ABI
feeds exactly one row-flipped point-filtered R8 Unity plane. Stereo, alternate
resolution, active-DAC RGB for historical captures, hostile/remote players,
unsupported branches, and cross-timeline pixel/clock equality remain explicit
nonclaims. Active tutorial/briefing, recursive help popup, immobilization
spider-web, and menu-state-7 score-writer states are reachable but deliberately
fail closed before COMPLETE until their missing live source state is typed.

The live Level 1 mission slice decodes four Archers, five Fireflies, the
Castle-spell jar, and the Speed-spell jar from owner data. Player Fireball,
impact, actor damage mailbox ordering, staged death, spell pickup, mission
activation, and objective predicates are Rust-owned. The source switch chain
now drives G0 spatial activation, G1/G2/G3/G5/G6 objective activation, delayed
G11, auxiliary G8/G12, and post-completion G4 with lowest-free-slot reuse,
scan-cursor ordering, and the one-objective-pass pause. All 51 authored
allocation records in those reachable groups consume the shared source-slot
projection in authored-index order. The Archer core now ports remc2's
constructor RNG, 30/120-tick decision cadence, range/FOV acquisition, 22-unit
yaw turn, attack-pose selection, and aimed class-9/model-13 arrow.
Unity consumes typed actor/projectile/impact snapshots and renders their exact
owner TMAP rows; it does not simulate them.

The mission snapshot keeps the registered fidelity-gap mask visible rather
than silently approximating behavior. The unified source-pool dispatcher
and exact reachable Level 1 Castle placement, construction, and mana-sphere
lifecycle are qualified. The class-3/model-3 mana carrier now preserves its
source-slot identity, scheduler, movement, attraction, collection, deposit,
damage/death, castle overflow, and direct-absorption ordering. The registered
simulation fidelity mask is now zero after exact reachable mission
sound-emitter positioning, pitch, play-type, and voice arbitration. Exit
collision/fade is qualified through its additive palette sidecar. Secondary
groups 33/34 are qualified through their authored allocation/activation
order, four multipart heads with 16 linked segments apiece, six Fireflies,
scroll, combat/death/mana paths, sounds 8/63, rendering snapshots, and reset.
Archer terrain avoidance and linked action 35 are now active and tested. The
native indexed world compositor also owns all 31 reset-frozen Level 1
villager/dolmen rows, twelve selector-global villager clocks, their exact
reset-chain membership, reflection-before-primary order, terrain shadows,
source clipping, and managed-fallback capability.

`COVERAGE.md` records the tested surface, `BRANCH-COVERAGE.md` maps each
bounded source leaf to its focused test, and `DISCREPANCIES.md` records every
known divergence. The canonical locked Cargo inventory now contains 442
tests: 4 atlas-registry, 57 sim-capi, 377 sim-core, and 4 sim-wasm. In
particular,
material class `1` (strongly identified water) remains distinct from exact
movement-redirection class `0x100`.

The complete Level 1 mission-route test reaches objective selectors 48–53,
collects and uses all four Level 1 spell families, drives the Archer,
Firefly, and Goat death paths, builds castle stage 1, possesses and deposits
mana spheres, enters the authored exit, and emits exactly one result-10
event. The native boundary separately proves dead player input suppression
and reset into a fresh generation. The desktop readiness record is
`../remc2-unity-native/LEVEL1-READINESS.md`.

The v2 native boundary also exposes an additive 88-byte audio-command sidecar
without changing the 72-byte event record. It preserves the original
Level 1 effect mixer as one shared ten-voice pool across ambient loops
1/2/5/31, local feedback 29, and positional IDs 37/39/43. Positional commands
carry wrapped XY, raw integer attenuation/pan, playback rate, pending
arbitration, and assigned voice. Unity reports natural completion by
generation, voice, and event sequence; mismatched ownership is rejected.
Reset clears the pool and invalidates prior-generation commands.
Original infinite ambient loops remain allocated and playing at volume zero
after `EndLoop`; re-prepare updates the same voice without restarting its
sample phase. They leave the pool only on session reset.

Run:

```bash
cargo test --workspace
cargo run -p sim-cli -- atlas-check
cargo run -p sim-cli -- trace
cargo run -p sim-cli -- inspect-regression \
  /absolute/path/to/sequence-002285FF-00356038.bin
cargo run -p sim-cli -- inspect-recording \
  /absolute/path/to/Levels-1-5-Recording.bin legacy 1 0 17 22
```

## Native reference status

The pinned native remc2 reference is now built owner-locally and can read the
owned GOG data. Its standalone two-tick full-state comparison passes, as do
all 23 level-start cases enabled by remc2's regression runner. The after-load
idle case also passes when isolated in a fresh process.

The monolithic runner itself is not a pristine oracle. It leaks shared state
between cases, and its current `InputRecorder` always expects the enriched
recording layout even though the bundled action fixture uses the older bare
layout with the same signature. That action run first reports a wrong
`pitchDelta` at captured frame 18. `DISCREPANCIES.md` records the byte-level
cause and the evidence policy. The Rust parser's explicit format choice is
intentional; the simulator must not reproduce the native loader defect.

The private `remc2-multipass-flight-trace.gdb` probe additionally injects one
forward command after preserved level-1 frame zero and records the local
player after every entity pass. The Rust conformance suite matches all four
speed-index-1 states and all eight speed-index-2 states field-for-field across
the bounded flight projection. Four-pass active traces also match forward
steering, reverse, negative strafe, positive strafe, pitch, and both
positive-capped and negative recoil on original terrain. Staged speed
modification and immobilization also match through every intermediate pass.
Real-map active probes additionally match a level-3 material redirect and a
level-4 cave rejection and recovery push. A real level-1 tether probe matches
the selected target and holder, capped pull, yaw, position, and phase. A
separate exact activation probe shows left fire selecting holder `153`,
copying timer `5`, preserving mana `1000`, and changing control flags from
`13` to `269`, exactly as the Rust owner-state regression expects.

## Parked browser proof

The browser lab remains available as a historical proof, using synthetic
terrain and newly generated presentation art. It does not package original
MC2 audiovisual assets or the private atlas. It is not the active client
target: the agreed next presentation layer is a native fullscreen client, and
that work begins only after the tick/state-perfect gate closes.

```bash
cd web
pnpm install
pnpm play
```

Open <http://127.0.0.1:8878/> in Chrome, then use:

- `W` / `S` to accelerate and brake
- `A` / `D` to strafe
- mouse, arrow keys, or `Q` / `E` to steer
- `Space` to fire
- `P` to pause
- `R` to restart the vignette

The objective is to ignite the six brass beacons on the route to the volcano.
The minimap, speed/altitude/magic meters, deterministic state hash, fireball
impacts, procedural wind, and pause/reset behavior are all live.

The server deliberately binds only to `127.0.0.1`. Keep it owner-local: this
is a private Track P executable proof, not a distributable build.

Useful gates:

```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo test --workspace --locked
cd web
pnpm typecheck
pnpm build
pnpm test:e2e
```

The Wasm adapter remains runnable but parked. A future native adapter remains
a thin client over the same core, not an alternate simulation implementation.
