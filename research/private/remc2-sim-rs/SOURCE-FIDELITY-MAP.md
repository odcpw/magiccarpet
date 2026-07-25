# Player-flight source fidelity map

## Purpose and gate

This document is the audit ledger for the private MC2 player-flight slice.
The next presentation client does not begin merely because the controls feel
good. It begins when the bounded simulation is **tick/state-perfect** for its
declared state projection:

```text
one ten-byte player command
  -> one command latch
  -> one, four, or eight entity passes using that latch
  -> identical authoritative flight state after every pass
```

Visual frame matching is a later renderer test. A visually similar frame can
hide a wrong accumulator, counter, collision branch, or random sequence.

remc2's preserved regression corpus captures whole state after the rendered
outer loop. The Rust harness therefore distinguishes:

```text
entity pass = one invocation of the physical player-flight routine
outer frame = input latch + 1/4/8 entity passes + later systems + render
```

An outer-frame fixture proves a one-pass transition only when the scheduler is
known to select one pass. Four/eight-pass cases need intermediate pass
checkpoints before they can be called pass-exact.

## Authority and checkout condition

- Upstream repository pin:
  `6806c7204d17220ecc0430759f784aea8ee2ddb2`
- Primary source:
  `upstream/magic-carpet-2-hd/remc2`
- Current relevant working-file hashes:
  - `EventsFunctions.cpp`:
    `fac00f7fb51f7f72996d208fb34600e8f06aee389faa3e60801be66d62fb92ac`
  - `Player.cpp`:
    `bec2a9a6c636165a73d9b869f10b400f437c1c69a00be2cdbe113cc1b08d2d64`
  - `Terrain.cpp`:
    `de3b6f5b3fa332aea832a0a48bb13c82920bbb66a98f1a154be537b39bbde5e9`
  - `Level.cpp`:
    `836326264c5dfa69c61e58ef0a03f564bdb7f0fa597bce74d61b22a3830cdc17`
  - `Maths.cpp`:
    `8bed55e5c132a2485c3258d75cb7714def2c0bd9cf17839b1bfa4b0c659d52c0`
  - `Spells.cpp`:
    `82130fd5347e5e98fa18031d35fbf99514d7d0b75ebecd2e7de2106d106f05d6`
  - `GameUI.cpp`:
    `92b5f63db493277dcdd717c4a94213aafd8a17db395e21d007829cc4eea4650f`

The upstream checkout contains user changes. The current
`EventsFunctions.cpp` diff only adds quit handling at lines before the flight
slice; it does not alter the mapped flight functions. The upstream checkout is
read-only for this work.

## Execution chain

| Order | Source operation | Source evidence | Contract |
|---:|---|---|---|
| 1 | Sample host input | `ReadGameUserInputs_89D10`; `EventsFunctions.cpp:31742` | Presentation/host boundary |
| 2 | Translate host input | `MouseAndKeysEvents_17A00`; `EventsFunctions.cpp:31770` | Produces the ten-byte action |
| 3 | Network/replay substitution and command latch | `PlayerEvents_51BB0`; `EventsFunctions.cpp:37536-38092` | Once per game turn |
| 4 | Roll back prior dynamic lighting | `sub_848A0`; `EventsFunctions.cpp:47268-47463` | Intervening outer-loop system; mutates only night/cave shading, not declared flight state or terrain-provider queries |
| 5 | Select pass count | `DrawAndEventsInGame_47560`; `EventsFunctions.cpp:31808-31823` | Speed index selects 1, 4, or 8 passes; an unsupported value can select 0 |
| 6 | Dispatch class 3, action 0 | `UpdateEntities_57730`; `EventsFunctions.cpp:40124-40181`; `Events.cpp:2878-2880` | Entity-index order, once per entity pass |
| 7 | Player action body | `AddPlayer03_00_5E010`; `EventsFunctions.cpp:60003-60090` | Input decode, flight integration, then player economy/life |
| 8 | Input decode | `sub_5F380`; `EventsFunctions.cpp:60796-60912` | Runs before the flight early-return flag |
| 9 | Flight integration | `sub_5D530`; `EventsFunctions.cpp:59658-59899` | Physical flight plus deterministic side effects |
| 10 | Entity pass counter | `UpdateEntities_57730`; `EventsFunctions.cpp:40179-40181` | Increments even when `sub_5D530` returns early |

The Rust split mirrors this boundary:

```text
latch_command(raw command)
  -> LatchedCommand
  -> step_outer_turn_with_context(speed index)
  -> 0, 1, 4, or 8 step_entity_pass(latched command)
```

The same `LatchedCommand` is deliberately reusable across four or eight
entity passes.

## Command layout and latching

`Type_PlayerInput_0x6E3E` is exactly ten bytes
(`LevelStructs.h:153-162`):

| Byte(s) | remc2 field | Rust field | Flight use |
|---:|---|---|---|
| 0 | `PlayerAction_byte0` | `opcode` | Action/menu operation before the unconditional movement latch |
| 1 | `str_0x6E3E_byte1` | `argument_1` | Action-specific |
| 2 | `str_0x6E3E_byte2` | `argument_2` | Action-specific |
| 3 | `roll` | `roll` | Roll target input |
| 4 | `pitch` | `pitch` | Pitch target input |
| 5 | `entityIndex_..._byte5` | `flags` | Forward, reverse, strafe, and three fire bits |
| 6-7 | `nextEntity_..._word6` | `argument_6` | Spell/target payload |
| 8-9 | `entityIndex2_..._word8` | `argument_8` | Spell/target payload |

After processing the opcode, `PlayerEvents_51BB0` unconditionally computes:

```text
roll_delta  = trunc_zero((2 * command.roll  - roll_accumulator) / 4)
pitch_delta = trunc_zero((2 * command.pitch - pitch_accumulator) / 4)
flags       = command byte 5
word_6      = command word 6
word_8      = command word 8
```

The source expressions at `EventsFunctions.cpp:38068-38074` use
`my_sign32`. That helper returns `-1` for negative values and `0` otherwise
(`engine_support.cpp:2962-2966`), so the sign-corrected shifts implement
ordinary signed division with truncation toward zero. This is proven static
behavior, not an oracle question.

Opcode `0x27` is also inside the bounded movement latch: it zeros raw roll and
pitch, optionally zeros target/actual speed, and clears the exact enabled
speed-spell holder's `word_0x2E_46`. Menu-dependent input rewriting remains an
upstream host/UI concern and is not silently guessed by the core.

Opcode `6` is movement-relevant when fire bit `0x40` is present: it first
stores command byte 1 as the selected UI spell, resolves it through the
29-entry `spellIndex_D94FF` map, and only later lets `sub_5F380` activate the
resolved holder. `build.rs` pins and generates that map; the Rust latch
preserves this same-command ordering.

## Authoritative flight-state projection

| Meaning | remc2 storage | Rust storage | Current disposition |
|---|---|---|---|
| Entity identity | entity-array index | `entity_index` | Implemented |
| Position | entity `position_0x4C_76` | `position` | Implemented |
| Yaw | entity `yaw_0x1C_28` | `yaw` | Implemented |
| Display pitch | entity `pitch_0x1E_30` | `display_pitch` | Implemented |
| Effective movement pitch | transient `pitch_0x24_36` | `effective_pitch` | Implemented |
| Roll accumulator | transient `roll_0x155_341` | `roll_accumulator` | Implemented |
| Pitch accumulator | transient `pitch_0x157_343` | `pitch_accumulator` | Implemented |
| Target speed | transient `speed_0xc_12` | `target_speed` | Implemented |
| Target-speed change marker | transient `word_0xe_14` | `speed_changed_flag` | Implemented, decoded from exact-build frames, and transition-compared |
| Actual speed | entity `actSpeed_0x82_130` | `actual_speed` | Implemented |
| Strafe speed | transient `strafeSpeed_0x10_16` | `strafe_speed` | Implemented |
| Roll/pitch deltas | transient offsets `0x4`, `0x6` | `roll_delta`, `pitch_delta` | Implemented |
| Directional recoil | transient `moveBoost_0x1E_30`, `yaw_0x1E_30` | `recoil_speed`, `recoil_yaw` | Implemented |
| One-pass impulse | transient `x/y/zAdd_0x1A6-0x1AA` | `impulse_x/y/z` | Implemented |
| Material contact counter | transient `waterCounter_0x262` | `water_counter` | Implemented |
| Speed modifier | transient `moveSpeed_0x14C` and counter | `speed_modifier` and counter | Implemented |
| Immobilization | transient `mobilizeCounter_0x14E` and subcounter | `immobilize_counter` and subcounter | Implemented |
| One-pass early return | entity status byte 1 bit `0x08` | `skip_flight_once` | Implemented |
| Recovery latch | transient `byte_0x261` | `recovery_latch` | Implemented and branch-tested |
| Duel/tether target, prior distance, holder and tier | transient offsets `0x142`, `0x146`, `0x14A`; spell slot 14 | typed tether fields | Implemented and branch-tested |
| Active tether target/holder projection | target entity, spell-holder entity and spell table | `TetherContext`; hash-pinned tier constants | Implemented and branch-tested |
| Speed-spell holder reset on collision | enabled spell slot 3, holder life/counter | `FlightContext::speed_spell_holder` | Implemented and branch-tested |
| Entity action-pass byte | entity `byte_0x3E_62` | `entity_phase` | Implemented and exact-build compared |
| Ambient loop distances | transient `0x19A`, `0x19E` | `ambient_loop_5_distance`, `ambient_loop_31_distance` | Implemented and exact-build compared |
| Local sound/music state | transient player color and `word_0x36` | `player_color_index`, `music_countdown` | Implemented and exact-build compared |
| Cave RNG and deterministic presentation calls | global RNG plus timing salt; sound/music calls | `FlightEffectsContext`, `FlightEvent` | Implemented as optional explicit context and branch-tested |
| Fire inputs and selected spell state | flag bits `0x10`, `0x20`, `0x40`; left/right/selected spell holders | `SpellFireContext`; typed `RequestSpellActivation` events | Dispatch order, holder choice, direction masks, pre-skip position, synchronous activation, and holder aliasing implemented and branch-tested |
| Spell activation state | caster model/mana/control flags/control mode; holder model/level/active/reset/cost/action marker | `spell_activation::{SpellCasterState, SpellActivationHolder, activate_spell}` | Complete `sub_5F660`/`sub_5F7B0`/`sub_5F7E0` branch families implemented and source-tested |

`state_hash` covers every field in `FlightState`; `conformance_hash` also
covers the typed cross-entity and spell-fire projections, optional effects
context, global RNG, and emitted event journal. It does not claim the
larger per-spell action routines that consume an activated holder on later
entity ticks.

## Source-ordered physical step

| Step | Source | Required behavior | Rust status |
|---:|---|---|---|
| 1 | `sub_5F380:60821-60839` | Clear `word_0xe_14`; forward/reverse changes target by 16 and clamps to `[-80,80]`; reverse wins when both are eligible; set marker only on change | Implemented, exact-build projected, and tested |
| 2 | `sub_5F380:60840-60897` | Strafe changes by 16; right/positive wins; no-input decay is 4 toward zero; clamp `[-80,80]` | Implemented and tested |
| 3 | `sub_5F380:60899-60910`; `sub_5F660:60923-61018` | Activate left/right/selected spell holders in that order with direction masks `256/512/256`, preserving aliasing, before flight skip | Implemented synchronously with typed boundary events and branch-tested |
| 4 | `sub_5D530:59664-59668` | Clear one-pass skip flag and return; input decode already happened | Implemented |
| 5 | `sub_5D530:59669` | Copy authoritative position to proposed position | Implemented |
| 6 | `sub_5D530:59670-59681` | Add roll/pitch deltas, scaled by speed-modifier stage with truncation toward zero | Implemented |
| 7 | `sub_5D530:59682-59683` | `yaw += trunc_zero(roll / 8)` and mask to 11 bits | Implemented |
| 8 | `sub_5D530:59684-59692` | Actual speed moves 16 toward target and may overshoot | Implemented and tested |
| 9 | `sub_5D530:59693-59714` | Terrain-relative movement pitch, altitude factor clamped to `[-256,256]`, with zero-state retention | Implemented and tested |
| 10 | `sub_5D530:59715-59728` | Forward displacement; speed modifier scales, immobilization zeros translation | Implemented |
| 11 | `sub_5D530:59729-59742` | Strafe at `yaw + 512`; same modifier/immobilization rules | Implemented |
| 12 | `sub_5D530:59743-59760` | Recoil cap at positive 128, displacement, signed decay by 4, sub-step remainder to zero | Implemented |
| 13 | `sub_5D530:59761-59766` | Apply Cartesian impulse exactly once and clear it | Implemented |
| 14 | `sub_5D530:59767-59768` | Decrement nonzero material-contact counter | Implemented |
| 15 | `sub_5DE30:59937-59997` | Apply active duel/tether steering and displacement; mutate target mana/life or clear target | Implemented with explicit cross-entity context and tested |
| 16 | `sub_5D530:59770-59792` | Advance speed modifier and immobilization counters in source order | Implemented |
| 17 | `moveTest_5D0A0:59477-59655` | Material redirection with low-word 3D distance truncation, then cave collision/steering | Implemented with exact angle/root helpers, source-width truncation, and branch tests |
| 18 | `sub_5D530:59795-59818` | Ground-relative vertical drift, immobilization drop, then mutually exclusive ground repair or cave ceiling clamp, commit | Implemented and ordering-tested |
| 19 | `sub_5DD50:59902-59930` | Rejected-movement recovery latch and 128-unit push at current yaw | Implemented and branch-tested |
| 20 | `sub_5D530:59824-59847` | Periodic active-spell sounds | Implemented as typed deterministic events and tested |
| 21 | `sub_5D530:59848-59898` | Cave ambience RNG, water/wind/building loops, music state | Implemented as explicit state plus deterministic events and tested |

## Exact numeric helpers

| Helper | Source | Rust treatment |
|---|---|---|
| Sine/cosine projection | `MoveEntity_57FA0`; `Player.cpp:6-19`; `Maths::sin_DB750` | The exact 2,560-entry table is generated from pinned `Maths.cpp` |
| Legacy angle lookup | `Maths::sub_72633_maybe_tan`; `Maths.cpp:764-817` | Exact 257-entry `x_WORD_DE350` table and branch structure are now ported |
| Horizontal heading | `Maths::sub_581E0_maybe_tan2`; `Maths.cpp:819-833` | Uses exact legacy angle lookup with signed 16-bit deltas |
| Movement pitch | `Maths::sub_58210_radix_tan`; `Maths.cpp:757-762` | Uses exact legacy lookup after integer horizontal distance |
| Angular distance | `sub_582B0`; `Sound.cpp:6569-6576` | Exact 11-bit shortest distance |
| 3D distance | `sub_583F0_distance_3d`; `EventsFunctions.cpp:40429-40435` | Exact recovered seed table, bit scan, unsigned wrapped sum and Newton loop; property-tested across sampled `u32` domain |

## Spell activation state machine

`sub_5F660` is a bounded activation gate, not the implementation of every
spell. Its exact branch families are:

| Holder condition | Source result and mutations | Rust status |
|---|---|---|
| Invalid/sentinel holder | Return false; no mutation | Implemented and tested |
| Active model 1 | Set action marker, replace direction bits, conditionally clear pointer/action bit, return true | Implemented and tested |
| Active model 2 | Emit sound 29 for ordinary caster; return false | Implemented and tested |
| Active models 4/6/8/11/12/14, caster model 0 | Reset timer to 7 for model 4 or 1 otherwise; return false | Implemented as a complete model matrix |
| Active gated models 0/7/9/10/13/15-24 | Block activation according to the exact level/active predicates | Implemented as a complete model matrix |
| Mana below holder cost | For ordinary caster, emit help slot 41 state 6 when help is enabled, then sound 29; return false | Implemented and event-order tested |
| Cold activation | Copy unsigned reset timer bit-for-bit into signed active timer, replace direction bits, conditionally clear pointer/action bit, return true | Implemented and tested |
| Caster model 1 | Force direction mask to zero and suppress insufficient-mana presentation effects | Implemented and tested |
| Signed division idioms | `my_sign32` plus corrected shifts | Replaced by Rust signed division only where static equivalence is proven |

The former brute-force 2,048-angle search was deterministic but was not the
original algorithm. It has been removed from movement redirection.

## World-query boundary

The flight routine needs these semantic facts:

```text
ground_height(position)
ceiling_height(position)
material_mask(position)
angle_flags(tile)
```

These are sufficient for either the original 256-by-256 heightfield or a
future voxel world. They are not permission for the host to invent an extra
collision decision after remc2's movement resolver. The former generic
`FlightWorld::resolve_movement` hook had no source counterpart and has been
removed.

The map mode is authoritative global state in remc2. Rust now carries it
explicitly in `FlightProfile::cave_mode`; it is no longer inferred from whether
a provider happens to return a ceiling.

Material results are exact bit identities from
`Terrain.cpp:2058-2142`:

- terrain type 0 -> mask `0x0001`, used by player ambience as water;
- terrain type 8 -> mask `0x0100`, used by `moveTest_5D0A0` and recovery;
- other terrain types map to their distinct recovered masks.

Mask `0x0100` must not be casually renamed “water”; its higher-level material
name is still unresolved.

`legacy_terrain::LegacyTerrain` now implements concrete sampling from the
original 65,536-byte array surfaces:

- `sub_B5C60_getTerrainAlt2` for ground interpolation;
- `sub_B5D68` for cave-ceiling interpolation;
- `mapTerrainType_10B4E0` for material;
- `mapAngle_13B4E0` for topology/forbidden flags.

It preserves byte-coordinate wrapping, alternating terrain triangles, signed
interpolation shifts, the material switch, and even the reconstructed cave
routine's double-x increment in one triangle. It also decodes the original
seven-surface `0x70000`-byte DOSBox regression frames. The level-1 and
after-load tests now exercise real owned terrain arrays rather than only
synthetic providers.

## Collision branch ledger

| Branch | Expected mutation |
|---|---|
| Proposed material is not `0x0100`, outdoor | Continue with proposed position |
| Proposed material is `0x0100`, first quantized slide becomes non-`0x0100` | Use first slide; refresh contact counter |
| First slide still `0x0100`, second becomes non-`0x0100` | Use second slide; refresh contact counter |
| Both slides remain `0x0100`, outdoor | Reject proposed position without the cave-path target-speed reset; then run recovery |
| Cave has strict clearance and tile flag 8 is clear | Accept candidate directly |
| Cave is blocked but one side probe has a strictly larger gap and no overlap | Redirect and adjust yaw by `(17 * probe_count) / 6` |
| Side gaps tie, forbidden flags block them, or overlap remains | Reject, restore proposed to old position, zero target speed, and disable active speed spell holder |
| Accepted position below ground clearance | Clamp to ground plus entity clearance |
| Accepted cave position above ceiling minus 384 | Clamp to ceiling minus 384 |
| Rejected recovery latch condition succeeds | Set latch and move from old position at current yaw, pitch 0, speed 128 |
| Rejected recovery latch condition fails | Clear latch and keep old position |

Both recovery rows are implemented and tested.

## Deterministic comparison projection

For each entity pass, a fixture must compare at least:

```text
position xyz
yaw
display and effective pitch
roll and pitch accumulators
target and actual speed
strafe speed
recoil speed and direction
one-pass impulse fields after clearing
material counter
speed-modifier stage and counter
immobilization stage and counter
one-pass skip bit
recovery latch
tether target/prior-distance/tier state when active
speed-spell holder mutation when collision rejects
deterministic event journal entries selected for this slice
```

Comparison is after every entity pass, not only after a rendered frame. The
input bytes and initial projection are fixtures. We do not need to imitate a
human mouse trajectory when the exact ten-byte commands are already known.

The post-frame regression image stores the ten-byte raw input array after
`PlayerEvents_51BB0` has cleared it. The harness therefore reads the durable
decoded boundary from the player transient record: movement flags, roll/pitch
deltas, and target payload words. The matching raw recording supplies the
earlier boundary for input-latch tests.

Two same-signature recording layouts are present in the private corpus and are
parsed explicitly, never heuristically:

- the older regression layout begins turn records immediately after each
  player header;
- the current remc2 layout inserts 208 bytes of initial spell metadata.

The enriched-format parser is locked to the existing native/WebAssembly probe
hash. The legacy parser is locked to the level 0 corpus shape (2,932 ten-byte
turns) and rejects that file when asked to parse it as the enriched format.

## Exact-build evidence now in-tree

`regression_state` decodes the original 224,790-byte `0x356038` state image
without compiling or linking remc2:

- 20 level-1 frames validate the player record, player entity, transient
  pointer, command-array, and 168-byte entity-array layout.
- Real player-flight fields are projected directly into `FlightState`.
- The player special-settings pointer resolves to source row 66 outdoors and
  row 104 in caves across all 25 level starts. Those exact rows generate the
  Rust flight profile at build time.
- All 25 level-start corpora reproduce 19 consecutive idle transitions each:
  475 exact transitions across eight day, nine night, and eight cave maps.
  These are real-map baseline/clamp proofs, not active collision-branch proofs.
- The preserved mid-game level-2 save reproduces another 19 idle transitions
  from turn 1,898-era state, including its nonzero orientation accumulators.
- `afterloadtest2` frames 0 through 24 plus level 1 of the matching legacy
  recording provide 24 consecutive one-pass raw-command transitions; the last
  five are forward acceleration.
- Starting from frame 0, Rust reads each raw ten-byte recorded command,
  reproduces remc2's decoded flags/deltas exactly, applies the matching real
  terrain frame, and reproduces the complete declared `FlightState` projection,
  including the raw target-speed-change marker, entity phase, and ambient/music state,
  at every following frame exactly.
- This includes the observed target/actual-speed sequence
  `0 -> 16 -> 32 -> 48 -> 64 -> 80` and its exact positions.

The in-tree total is now 518 exact-build transitions: 475 idle level-start
transitions, 19 mid-game idle transitions, and 24 recorded outdoor
transitions. These compare the complete declared `FlightState`.

The pinned native reference has also been exercised directly. Its standalone
two-tick comparison, all 23 enabled level-start regressions, and an isolated
after-load idle regression pass. The bundled native action runner is not used
as contrary evidence for frame 18: its current loader consumes a 208-byte
spell-metadata block from a legacy bare-layout recording that has no such
block. The raw turn-19 command is `[0,0,0,0,2,0,0,0,0,0]`; with a prior pitch
accumulator of zero, the mapped source expression yields the preserved
`pitchDelta = 1`. Rust's explicit legacy parser and the DOS state image agree.
`DISC-008` records the full harness defect.

The private `remc2-multipass-flight-trace.gdb` probe also starts the pinned
native build from preserved level-1 frame zero, injects one forward command,
and observes the player after every entity pass. Rust now matches all four
speed-index-1 checkpoints and all eight speed-index-2 checkpoints exactly for
position, yaw, both pitch fields, both rotation accumulators, target and
actual speed, the target-change marker, strafe speed, and entity phase. This
closes the intermediate scheduler checkpoint gap without modifying remc2.
The same probe now locks four-pass active reference traces for forward
steering, reverse acceleration, negative strafe, and positive strafe on the
real level-1 terrain. Rust matches every observed intermediate player field,
including integer yaw progression and terrain-shaped altitude.
Active pitch and recoil probes also agree at every pass. The recoil vectors
cover the positive `128` cap and subsequent decay as well as negative recoil
without that cap, including the resulting real-terrain altitude repair.
Staged speed-modifier and immobilization probes agree as well, including
counter reset/decrement order, same-pass translation scaling, translation
suppression, and signed immobilization-subcounter wrap.
On original level-3 terrain, a direct move into material class `0x100`
matches native boundary redirection, contact-counter mutation, speed
preservation, and vertical order. On original level-4 cave terrain, an active
blocked move matches native rejection, target-speed stop, actual-speed
retention, and unchanged position. Starting inside the same forbidden cave
region also matches the separate native recovery branch: target stop followed
by the exact 128-unit forward push and recovery-latch set.

The same owner-data approach now covers both remaining cross-entity active
paths. On real level-1 state, the tether checkpoint resolves target `155` and
holder `153`, then matches position `(19856,56842,5024)`, yaw `44`, phase
`154`, and unchanged tier-0 target state. The left-fire activation checkpoint
resolves holder `153`, copies reset timer `5` into the active timer, preserves
mana `1000`, and changes caster control flags from `13` to `269`. Both are
locked by Rust regressions.

## Native-client gate disposition

The declared class-3/action-0 tick/state gate is closed:

1. every mapped leaf has a focused source-derived executable test;
2. all 518 preserved whole-projection transitions remain exact;
3. scheduler and active control families have intermediate native checkpoints;
4. real-map material, cave, recovery, tether, and spell activation paths have
   owner-data native checkpoints; and
5. no known divergence or unresolved ambiguity remains in the bounded
   projection.

The owned remc2/oracle remains available if later integration exposes a
genuine source ambiguity. Native input, camera, rendering, audio, HUD, and
assets now form a separate presentation gate.

## Current verdict

The Rust core is a source-derived bounded port with 518 exact whole-projection
transitions over original state and terrain plus active native tether and fire
checkpoints. The declared class-3/action-0 simulation slice is tick/state exact
under its documented projection.
The ordinary outdoor flight spine, rejected recovery, active tether,
collision-time speed-spell mutation, exact root/angle helpers, and original
terrain sampling algorithm are mapped and branch-tested. Real owned-level
terrain and state snapshots are decoded directly. This is deliberately not a
global claim about every MC2 system or rendered-frame equivalence. The next
work is the native fullscreen presentation client, which must consume this
core without reimplementing simulation.
