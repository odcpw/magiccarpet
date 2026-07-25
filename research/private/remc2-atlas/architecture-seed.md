# remc2 architecture seed

Status: provisional static map; feeds `mc-dl0r.13` and `mc-dl0r.16`  
Classification: `PRIVATE_ANALYSIS`  
Source commit: `6806c7204d17220ecc0430759f784aea8ee2ddb2`  
Dirty-diff SHA-256:
`73fcffd66baaeeeab443d65b1e7bc333eba1f7e106e7918a552c5ac10ad33258`

The source checkout already contains Oliver-owned changes in:

- `remc2/engine/Animation.cpp`
- `remc2/engine/EventsFunctions.cpp`
- `remc2/engine/MenusAndIntros.cpp`
- `remc2/portability/port_sdl_vga_mouse.cpp`
- `remc2/portability/port_sdl_vga_mouse.h`

This map describes that exact working tree. It does not silently treat the
nested Git commit as a clean source state.

## Executive shape

remc2 is a reconstructed program with a replaceable world-render interface,
not currently a modular simulation library. Most authoritative gameplay is
dispatched through fixed entity pools, type/class/action tables, and broad
global state. `EventsFunctions.cpp` alone contains 63,684 lines and crosses
nearly every gameplay subsystem.

The useful architecture is therefore recovered by execution phase and state
region, not by source filename.

```text
process/config/data initialization
  -> menus/campaign transition
  -> level decompress and procedural construction
  -> in-game loop
       -> input/replay/network ingestion
       -> player command handling
       -> transient event update
       -> entity dispatch and world simulation
       -> lighting/objectives/lifecycle
       -> entity sound commands
       -> world/HUD rendering
       -> regression comparison
  -> level teardown/progression
  -> process teardown
```

## Entry and lifecycle

| Phase | Primary evidence | Current interpretation |
|---|---|---|
| Process entry | `remc2/remc2.cpp:10-30` | CLI initialization, host support, `sub_main`, teardown |
| Host/game initialization | `remc2/sub_main.cpp:529-660` | Config, logging, event dispatcher, paths, SDL/VGA/gamepad, owner data, network, recording, `Initialize` |
| Reconstructed initialization | `remc2/engine/EventsFunctions.cpp:42858-42997` | Directories, data, spells, palette, video/input, network and sound |
| Campaign/level loop | `EventsFunctions.cpp:31275-31569` | Menus, level load/init, renderer selection, in-game loop, progression |
| In-game loop | `EventsFunctions.cpp:31585-31703` | Repeats one frame step until the level state ends |
| Frame body | `EventsFunctions.cpp:31732-31885` | Canonical first static candidate for a deterministic step boundary |
| Teardown | `remc2/sub_main.cpp:664-687` | Recording, network, memory, dispatcher and VGA cleanup |

## Per-frame candidate order

The statically visible order in `DrawAndEventsInGame_47560` is:

1. frame and regression counters;
2. palette and sprite animation;
3. raw input capture and command translation;
4. debug/load/save hooks;
5. `PlayerEvents_51BB0`;
6. transient event update;
7. `UpdateEntities_57730`, repeated according to speed mode;
8. lighting;
9. objectives, presentation and lifecycle;
10. entity sound command generation;
11. render;
12. regression comparison;
13. help/debug;
14. blend/blit and final counters.

Evidence: `remc2/engine/EventsFunctions.cpp:31732-31885`.

This is a lexical/static order. Timer interaction, callback mutation, indirect
entity handlers, and active build predicates remain later correlation work.

## State-region model

These stable private region IDs seed the sparse matrix:

| ID | Region | Representative evidence |
|---|---|---|
| `S0` | Main session scalars, RNG, player count/settings | `engine/LevelStructs.h:202-230` |
| `S1` | Player slots and camera history | `engine/LevelStructs.h:97-136,235` |
| `S2` | Ten-byte player input commands | `engine/LevelStructs.h:153-162,236` |
| `S3` | Fixed entity pool and entity pointer/free lists | `engine/LevelStructs.h:219-221,237`; `engine/global_types.h:320-392` |
| `S4` | Level descriptor, stages and objective state | `engine/LevelStructs.h:190-200,238,247-250` |
| `T0` | Terrain type grid | `engine/Terrain.cpp:6` |
| `T1` | Terrain height grid | `engine/Terrain.cpp:7` |
| `T2` | Terrain shading grid | `engine/Terrain.cpp:8` |
| `T3` | Terrain angle/water grid | `engine/Terrain.cpp:9` |
| `T4` | Per-cell entity index | `engine/Terrain.cpp:10` |
| `T5` | Cave/secondary terrain representation | `engine/BasicTerrain.h`; terrain generation/query paths |
| `R0` | Runtime/config/host aggregate | `engine/engine_support.h:206-382` |
| `L0` | Entity action/creation descriptor tables | `engine/EventsFunctions.cpp:2060-2078`; `engine/Events.cpp:410-607` |
| `P0` | Palette, framebuffer, textures and UI presentation | renderer/UI modules |
| `A0` | Sound, music and timer state | `engine/Sound.cpp`; `portability/port_sdl_sound.cpp` |
| `N0` | Network session and transport | `engine/Network.cpp`; `portability/port_net.cpp` |
| `F0` | Filesystem, assets, level/save data | data/load/save modules |
| `C0` | CLI and configuration | `engine/CommandLineParser.cpp`; config modules |

## Subsystem map

| # | Subsystem | Roots and representative evidence | Principal regions |
|---:|---|---|---|
| 1 | Boot/config/lifecycle | `remc2.cpp`, `sub_main.cpp`, `Initialize` | `R0 C0 F0 A0 N0` |
| 2 | Menus/campaign/level transition | `sub_46830_main_loop` | `S0 S4 P0 F0` |
| 3 | Data and save/load/pointer repair | `LevelDecompress_533B0`, `LevelInitGame_56A30`, `Level.cpp` save/load | `F0 S0-S4 T0-T5` |
| 4 | Frame scheduler/timer/pause/speed | `InGameLoop_47320`, `DrawAndEventsInGame_47560`, timer thread/callback | `S0 R0 A0` |
| 5 | Input/replay/network ingestion | `PlayerInput.cpp`, `PlayerEvents_51BB0`, `InputRecorder`, `ReceiveSendAll_7438A` | `S1 S2 N0 C0` |
| 6 | Player/flight/camera | `PlayerEvents_51BB0`, `sub_5D530`, `AddPlayer03_00_5E010`, `sub_5F380`, `sub_57B20` | `S1 S2 S3 T0-T5 P0` |
| 7 | Entity pool/spatial index/lifecycle | `NewEvent_4A050`, `ApplyEvents_498A0`, `UpdateEntities_57730` | `S0 S3 L0 T4` |
| 8 | Entity action dispatch/state machines | descriptor/action tables and original-address dispatch bridge | `S3 L0` |
| 9 | Terrain generation/query/mutation/caves/water | `Terrain.cpp`, `BasicTerrain.h`, distributed handlers | `T0-T5 S3 S4` |
| 10 | Spells/projectiles/effects | `Spells.h/.cpp`, `SetSpell_6D5E0`, class-15 spell holders, class-9 projectiles and class-10 effects | `S1 S2 S3 S4 L0 T0-T5` |
| 11 | Creatures/rival-wizard AI | class-5 table, AI wizard dispatcher and handlers | `S0 S3 S4 L0 T0-T5` |
| 12 | Mana/castle/balloon/economy | player/castle update, mana sphere and castle helpers | `S1 S3 S4 L0` |
| 13 | Objectives/stages/progression | objective evaluator and level-end helpers | `S0 S3 S4` |
| 14 | World rendering | `GameRenderInterface::DrawWorld_411A0` implementations | `P0 S1 S3 T0-T5` |
| 15 | HUD/minimap/menu/palette/blit | `DrawGameFrame_2BE30`, `GameUI.cpp` | `P0 S0-S4 T0-T5` |
| 16 | Sound/music/spatial audio | sound command generation/consumption and SDL/Miles adapters | `A0 S1 S3` |
| 17 | Network session/transport | legacy fixed-input relay and BSD/Winsock adapters | `N0 S2 S0` |
| 18 | Regression/oracle/debug | `InputRecorder`, `CompareMaps`, state monitor and regression tests | all declared regions |

## First flight chain

This is the first gameplay chain to classify after the full mechanical census:

```text
mouse/keyboard
  -> ten-byte input command
  -> PlayerEvents_51BB0
       -> roll/pitch target filtering
       -> selected action/spell/target state
  -> AddPlayer03_00_5E010
       -> sub_5F380 input decode, speed bits, strafe, spell fire
       -> sub_5D530 roll/pitch/yaw/speed/terrain/impulse integration
       -> mana/life/castle update
  -> sub_57B20 camera-history copy
  -> DrawGameFrame camera consumption
  -> sound command spatialization
```

Representative evidence:

- raw mouse normalization: `engine/PlayerInput.cpp:2100-2141`;
- player command handling: `engine/EventsFunctions.cpp:37536-38093`;
- fixed-table displacement primitive: `engine/Player.cpp:6-19`;
- flight integration: `engine/EventsFunctions.cpp:59658-59780`;
- player action and regen/castle work: `EventsFunctions.cpp:60003-60090`;
- speed/strafe/fire input decode: `EventsFunctions.cpp:60795-60912`;
- camera-history construction: `EventsFunctions.cpp:40197-40283`;
- camera render consumption: `EventsFunctions.cpp:21571-21578`.

## Loaded-value warning

Source initializers are candidate seeds, not automatically live parameters.
For example:

```text
compiled spell records
  -> SPELLS.DAT decompression/load
  -> spell initialization
  -> level/map-mode patch
  -> entity/runtime derivation
  -> mana-cost and effect consumers
```

The later table-lineage pass must close each such chain before reporting a
live gameplay value.

## Candidate deterministic core seam

The first architecture hypothesis is:

```text
bounded commands
  -> player command update
  -> transient update
  -> entity/world update
  -> objectives/lifecycle
  -> canonical state snapshot and event journal
```

Current blockers to calling this a library boundary:

- timer and wall-clock ownership;
- SDL input/audio/render ownership;
- network exchange inside player processing;
- filesystem and owner-data loading;
- callback and descriptor dispatch;
- global/packed/aliased state;
- renderer/HUD/global-buffer coupling; and
- no explicit initialize/step/shutdown API.

The sparse matrix will test this hypothesis rather than assuming it.

## Required matrix columns

The human classification view will contain:

```text
private row ID
exact source/build/config/data manifest
function and source location
original-address suffix when present
execution phase and tick order
entity class/model/action selector
reads and writes by state region/field
RNG stream and ordering effects
direct calls
indirect/callback/descriptor candidates
bulk-memory and pointer-escape effects
host/platform side effects
loaded-value lineage
replay/hash/runtime coverage
semantic class
evidence stage
confidence axes
unknowns and contradictions
private/transfer disposition
```

The generated SQLite graph is the mechanical evidence index. This document is
only its provisional subsystem overlay.
