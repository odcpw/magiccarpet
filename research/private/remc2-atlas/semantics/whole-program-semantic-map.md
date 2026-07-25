# remc2 whole-program semantic map

Status: active private archaeology  
Build scope: the exact source/build manifest in `../generated/manifest.json`  
Classification: `PRIVATE_ANALYSIS` and owned-data evidence  
Last regenerated: 2026-07-24

## What “complete” means here

The program now has two different completeness layers, and they must not be
confused:

1. **Mechanical coverage is exhaustive inside the pinned build.** Every one of
   the 3,060 parsed functions and every one of the 3,246 referenced globals or
   structure fields has a stable row, subsystem placement, source interval,
   access/call counts, and a next archaeology action.
2. **Semantic recovery is incremental.** An evidence-linked claim means code,
   data, or assets support a particular interpretation. A descriptive remc2
   name is useful evidence, but it is not automatically original intent.

The machine-readable truth is:

- `generated/function-semantic-map.csv`: all 3,060 functions;
- `generated/state-semantic-map.csv`: all 3,246 referenced state targets;
- `generated/semantic-gap-queue.csv`: all still-raw address-named functions;
- `generated/state-gap-queue.csv`: writable raw state likely to affect the
  simulation;
- `generated/subsystem-semantic-coverage.csv`: honest per-subsystem maturity;
- `generated/system-surface-registry.csv`: 47 architectural landmarks;
- `generated/simulation-tick-pipeline.csv`: the 14-stage frame/tick spine; and
- `generated/runtime-class-families.csv`: all 16 runtime class families;
- `generated/runtime-dispatch-rows.csv`: all 845 indirect action/constructor
  rows with exact resolution status;
- `generated/runtime-constructor-registry.csv`: all 284 constructor rows;
- `generated/runtime-model-annotations.csv`: 250 non-sentinel physical-slot overlays;
- `generated/runtime-action-annotations.csv`: all 546 non-sentinel
  physical-slot action reductions, including exact structural-status rows;
- `generated/creature-models.csv`: all 29 class-5 model slots, their regular
  action bands, constructors, initial actions, key stats, asset evidence, and
  confidence;
- `generated/creature-action-exceptions.csv`: the three action rows outside
  the regular class-5 bands; and
- `generated/claim-function-links.csv`: source-interval links from semantic
  claims back into the complete function inventory.

## Current denominator ledger

| Surface | Denominator | Mechanically placed | Evidence-linked or classified |
|---|---:|---:|---:|
| Translation-unit functions | 3,060 | 3,060 | 571 functions intersect 329 of 337 claims |
| Raw address-named functions | 1,279 | 1,279 | 327 linked; 952 remain queued |
| Referenced globals/fields | 3,246 | 3,246 | 256 evidence-linked classifications |
| Writable gameplay-state candidates | 1,687 | 1,687 | parameter classes mostly unknown |
| Raw writable gameplay-state gaps | 456 | 456 | ranked for writer/load/consumer tracing |
| Runtime action/constructor rows | 845 | 845 | 653 resolve to native calls; every other status retained |
| Non-sentinel constructor rows | 269 | 269 | 250 labeled; 19 addressless reserved records |
| Non-sentinel action rows | 546 | 546 | 429 reconstructed behaviors; 117 exact structural statuses; all retain their model/family lineage |
| Spell families | 26 | 26 | 26 identity/display rows |
| Spell tiers | 78 | 78 | 78 owner-data rows |
| Spell icon bindings | 390 | 390 | day/night/cave packs joined |
| Owned HSPR/MSPR bindings | 1,572 | 1,572 | palette-correct contact sheets |
| Architectural surfaces | 47 | 47 | provisional or stronger role per row |
| Ordered tick stages | 14 | 14 | condition and authority class per row |

The action denominator is closed at per-physical-row resolution. The largest
remaining semantic concentration is the ranked writable state queue, followed
by friendly identity questions and formulas whose static evidence still needs
deterministic runtime confirmation. They are finite denominators rather than
invisible gaps.

## Program architecture

```text
sub_main
  -> outer main/session scheduler
     -> menu or level selection
     -> compiled defaults and owned/level data loading
     -> level decompression and initialization patches
     -> InGameLoop
        -> DrawAndEventsInGame each turn
           -> host input
           -> translated player-action bytes
           -> network/replay substitution
           -> player actions and carpet flight
           -> general gameplay update
           -> entity action dispatch (0/1/4/8 passes by speed mode)
           -> lighting
           -> objective evaluation and presentation
           -> post-objective world updates
           -> entity audio
           -> frame rendering and HUD
        -> save, level transition, or session exit
```

This is not an engine with a single separable `physics()` function. The game is
an ordered deterministic state machine whose feel emerges from:

- fixed-point movement;
- quantized acceleration and impulses;
- terrain-relative pitch and collision clamps;
- player action scheduling;
- entity constructors and per-action handlers;
- loaded values plus level/mode patches;
- Watcom-compatible random-number ordering; and
- sound, camera, HUD, and animation responding to the same state.

## Exact per-frame/tick order

`DrawAndEventsInGame_47560` provides the strongest static ordering boundary:

1. palette transitions;
2. sprite animation when not paused;
3. raw host input sampling;
4. mouse/key translation into turn actions;
5. `PlayerEvents_51BB0`;
6. remove prior dynamic-light contributions from the shading grid;
7. `UpdateEntities_57730`, repeated according to speed mode;
8. follow light owners and reapply current dynamic-light contributions;
9. objective evaluation;
10. objective presentation;
11. promote objective completion into the level-end transition flag;
12. conditionally restore a saved single-player checkpoint;
13. entity sound selection; and
14. final world/frame rendering.

`PlayerEvents_51BB0` is also the important determinism boundary. It advances
the player turn, receives multiplayer action bytes, substitutes recorded
actions during replay, records current actions when requested, then applies
the selected action to each player's entity. This is why a gameplay-equivalent
successor should preserve an explicit turn-action layer even if its transport,
renderer, and asset formats are new.

## Player and carpet flight

The compact flight spine is already suitable for independent implementation
and controlled comparison:

```text
input bytes
  -> roll and pitch deltas
  -> target forward speed and strafe speed
  -> roll accumulator
  -> yaw += signed roll / 8
  -> actual speed approaches target by 16 per tick
  -> terrain-relative pitch adjustment
  -> fixed-point forward displacement
  -> fixed-point strafe displacement at yaw + 512
  -> directional recoil with signed decay
  -> one-tick Cartesian impulse
  -> ground and cave-ceiling clamps
  -> authoritative entity position
  -> camera and presentation
```

Important field-name trap: for common creature movement, recovered
`minSpeed_0x84_132` behaves as a cruise target and upper bound, while recovered
`maxSpeed_0x86_134` behaves as a lower bound in the shared damage response.
Those raw names remain in provenance, but consumers must use contextual
semantic labels. Some constructors/actions overload the fields, so a global
source rename would be unsafe.

## Terrain and world state

The outdoor world is a fixed 256 by 256 cell domain with parallel grids for:

- terrain/material type;
- ground height;
- shading;
- angle and water/contact flags; and
- per-cell entity-chain lookup.

The level itself is a packed `0x6604`-byte record containing the authored realm,
procedural seed and terrain parameters, 1,200 twenty-byte entity/effect/terrain
records, eight wizard configurations, stage definitions, and stage variables.
Generation is deterministic: base height, normalization, rivers/cuts,
smoothing, topology/material assignment, optional cave ceiling, and shading.
Authored terrain and building commands then run in dependency order before
ordinary runtime entities are instantiated at tile centers.

Flight and entity movement consume interpolated ground height. Cave levels add
a second ceiling-height query. Water is not inferred from color or rendering:
authoritative material/contact queries select collision and ambience behavior.
The pinned upstream enhanced terrain buffer, rendered with Oliver's owned
day palette, and the direct renderer lookup now correlate raw terrain types
10, 11, and 12 to a three-member lava family. Their exact
hot/crust/animation-state names remain trace questions.

The cave command map is also no longer one undifferentiated gap:

- `0A:50` is a linked tunnel/path control chain that emits tube-carver
  segments;
- `0A:52` excavates a rectangular chamber or shaft;
- `0A:53` shapes a radial or domed cave volume;
- `0A:54` depresses or hollows the cave ceiling;
- `0A:55` raises a radial floor mound; and
- `0E:02` opens and smooths an oriented, configurable-length passage.

The operations are statically mapped. Controlled before/after map dumps remain
necessary for their original editor-facing nouns and exact parameter
envelopes.

This heightfield representation does not prevent a voxel successor. The clean
boundary is the set of semantic queries:

```text
ground_height(position)
ceiling_height(position, mode)
surface_material(position)
surface/contact flags(position)
spatial_entities(cell or volume)
apply_world_mutation(shape, payload, owner)
```

A voxel implementation can answer those contracts from voxels while retaining
the recovered flight and gameplay semantics. It should not copy the original
raw map representation into the new public engine.

## Entity runtime model

Each runtime entity has at least three independent selectors:

```text
class -> broad table family
model -> constructor/archetype entry within that family
action -> current per-tick behavior entry
```

Creation and updating are separate dispatches:

```text
level/event record
  -> runtime allocator
  -> class/model constructor table
  -> initialized entity and initial action
  -> UpdateEntities pool walk
  -> class/action table
  -> action function
  -> next action, mutation, spawn, damage, audio, or retirement
```

The indirect-table denominator is now complete. The 15 table families declare
561 action rows and 284 constructor rows, for 845 total including 30 null
sentinels. Their current resolution statuses are:

- 653 enabled rows resolve to reconstructed native calls;
- 31 enabled rows reach a reconstructed switch case that makes no native call;
- 41 enabled rows have no reconstructed dispatch case;
- 29 enabled rows contain null addresses, all in reserved classes 1 and 13;
- 46 disabled rows preserve a stored original address; and
- 15 other rows are disabled and addressless.

These categories are intentionally not collapsed. A deliberate no-op, a
reserved addressless record, and a missing reconstruction are different facts.
Of 269 non-sentinel constructor physical rows, 250 now have behavioral or friendly
annotations. The 19 unlabeled selectors are exactly the addressless class-1
and class-13 records.

All 546 non-sentinel action rows now carry an individual annotation keyed by
physical slot. Reconstructed handlers receive per-tick behavior summaries;
disabled holes, missing native cases, selector mismatches, inert dispatches,
and reserved addressless rows receive explicit structural-status annotations.
The proven class-5 model band or class-15 family band is retained separately,
so a behavior label cannot erase its structural lineage.

The physical-slot key is essential. Class-9 constructor model and action
numbers diverge after model 13: action 14 is an internally created Lightning
segment, model 14 starts action 15, and missing constructors at models 16, 18,
and 19 do not prove identities for actions 17, 19, and 20. Class-10 physical
row 63 stores selector 41, so an entity whose action index is 63 fails the
dispatch selector check; it is not a harmless alias of action 41.

Notable constructor hazards include class-3 selectors 4–11, which write global
player spawn positions and return null rather than creating entities; class-9
selectors 16, 18, and 19 with no effective native constructor; class-10 alias
selectors 58→39 and 69→54; class-10 model 77, whose factory row is disabled
but whose entities are manually created by model 76; and class-11 selectors
5–11, whose enabled rows lack reconstructed constructor cases.

Twenty formerly high-ranked raw entity fields are now classified as
`CONTEXT_DEPENDENT_RUNTIME_SLOT`, not as universal parameters. Their packed
offsets are reused with different meanings by creatures, projectiles, effects,
players, buildings, and triggers. The portable atlas must therefore assign
per-class/model/action aliases while retaining the raw offset; one global
rename would be misleading. Five union members are similarly classified as
`CONTEXT_DEPENDENT_STORAGE_VIEW` because byte, word, and dword names overlap
the same storage.

Large non-simulation state families are also separated explicitly: host
mouse/button/key/joystick input, per-player spell inventory/selection state,
terrain projection buffers, HUD popup/text-box state, display resolution
mode, texture metadata and frame caches, and the 347-entry particle visual
parameter table. They matter to fidelity, but they are not all carpet physics
or authoritative world state.

The current high-priority state pass also resolves the secondary reclaimable
entity stack, per-tick mana/building category lists, AI hostility records,
creature-attention timer, locomotion-command latch, spell-release counter,
objective and StageVar selectors/unions, lobby action records, normalized
pointer input, frontend DAT/TAB and palette views, local visual options,
derived bitmap pointers, texture animation flags, and software raster modes.
One recovered declaration is demonstrably too small: the AI hostility region
is declared as 32 words but code indexes logical entries 32 and 33 for player
seven. The atlas records the conflict rather than legitimizing the declared
bound.

Class 5 living actors use a 29-slot runtime model array. The game also has text,
editor, sprite, sound, level-subtype, and runtime-action identity spaces. Their
numeric positions are not interchangeable. Asset correlation must follow an
actual consumer, such as an action's sound or sprite selection, rather than
assuming the nth creature label means runtime model n.

The generic class-5 factory refuses models whose constructor-table entry is
zero. Current static evidence marks runtime models 5, 6, 7, 8, and 11 as
disabled through that generic path. “Named in owned data” therefore does not
prove “constructible in normal gameplay.”

The regular class-5 table is now complete: models 0 through 28 each own exactly
eight action rows, covering actions 0 through 231 without gaps or overlaps.
Action 232 is an additional enabled multipart-segment action. Actions 233 and
234 are disabled/null dispatch rows but the Hydra constructor writes them onto
parts; this is a material reconstructed-source/runtime inconsistency reserved
for exact-build tracing.

Strong or confirmed runtime identities now include Dragon, Goat, Bee, Archer,
Townie/Villager, Trader, Wyvern, Firefly/Firebug, Spider, Devil, Mana Worm,
Caveman, Cymmerian, Zombie, Hydra, and Deep One. Models 3, 9, 10, 12, 15, 17,
18, and 23 retain behavioral or sound-family labels where the final friendly
identity is not independently proven.

The common eight-state creature vocabulary is now mapped rather than merely
numbered:

| Ordinal | Common role | Required caveat |
|---:|---|---|
| 0 | idle/wander and damage routing | many multipart or special actors replace it |
| 1 | active roam, awareness, and target acquisition | usually the constructor's initial live state |
| 2 | pursuit or attack | projectile, dive, melee, resource, and scripted forms differ substantially |
| 3 | linked-part, hit, reaction, or reset | role is model-specific |
| 4 | pre-kill/death staging | some civilians and multipart actors override it |
| 5 | final kill, mana conversion, or cleanup | special actors split, deposit, or remove multipart state |
| 6 | hit/retaliation | 13 rows are missing reconstructed handlers; Caveman's row is implemented |
| 7 | authored StageVar2/script control | can deliberately force cross-band actions |

Every model override is recorded in `runtime-action-annotations.csv`. This is
why the portable design should use explicit per-archetype state graphs backed
by a shared behavior library, not hard-code `action % 8` as the meaning.

The positional creature text list is explicitly rejected as a runtime model
map. It would call model 1 “Vulture,” but that model executes named goat actions
and consumes the Goat sound. Runtime model 26 consumes the Zombie sound, so
Zombie is not a nonexistent model 29.

## World effects and authored commands

Strongly mapped world-effect rows now include:

- class 10/model 5: water splash;
- class 10/model 8: legacy mini-volcano slot whose constructor currently
  returns null;
- class 10/model 9: volcano, including mountain uplift, cave repair, summit
  cut/crater, quake cue, and eruption spawn;
- class 10/model 22: wind vortex/tornado built from a root and eleven stacked
  linked segments;
- class 10/model 23: short-lived lightning damage effect;
- class 10/model 29: path/topology-writing terrain command;
- class 10/model 31: canyon/tunnel command modifying ground and ceiling; and
- class 10/model 45: BUILD-driven building placement and terrain-footprint
  repair.

Class 2/model 6 is now confirmed by pinned enhanced-asset correlation as cave
mushroom scenery. Its constructor selects particle records 324–327, and all
four consume cave TMAP 424 at different scales. The recovered Bee comment is
contradicted by this actual consumer lineage. Class 2/model 5 remains
deliberately raw because its particle parameter has no drawable TMAP identity.

The exact private renderings are preserved as
`generated/evidence/tmap-cave-424-mushroom.png` and
`generated/evidence/terrain-lava-family-10-12.png`, with source hashes and
palette paths in `generated/visual-evidence.csv`. The indexed image buffers
come from the pinned upstream `enhancedassets` tree; the palettes come from
Oliver's owner-data evidence subset.

The recovered legacy `type 7 / subtype 4 = Weather/Wind` label is not a live
weather system: class-7 dispatch is empty and the owned levels have no type-7
records. The real authored vortex is `0A:16`. Global cloud, rain, continuous
weather, and astronomical cycles remain unproven; richer versions belong to
the successor design unless runtime evidence says otherwise.

The realm is authored per level:

- day campaign levels: 2, 3, 5, 9, 13, 17, 20, 23;
- night: 1, 7, 10, 11, 14, 18, 19, 22, 25; and
- cave: 4, 6, 8, 12, 15, 16, 21, 24.

A private one-time decode of Oliver's owned level image found 1,235 trees, 346
standing stones, 101 dolmens, 149 subtype-3 scenery objects, 555 cave-only
subtype-6 objects, 75 barrels, 56 vases, and 1,204 BUILD records across the
active slots. The probe used the existing RNC decoder and temporary extracted
files were removed afterward. Source image and table hashes remain recorded in
the research notes; these counts should become a reproducible owner-data
builder before being used as a regression authority.

## Spells and Fireball vertical slice

The loaded spell table contains 26 families by three owner-data records. The
localized catalogue also contains a fourth displayed variant for Morph and
Summon Army, so “three loaded records” must not be silently generalized into
“exactly three player-facing names” for every family. The live value lineage is:

```text
compiled spell seed
  -> owned SPELLS.DAT overwrite
  -> SetDefaultSpells patches
  -> LevelInit day/night/cave and level patches
  -> class-15 spell holder
  -> charge, affordability, and cast consumers
  -> projectile/effect constructor
  -> per-tick action and target-specific resolution
```

Class-15 physical actions are another independent dimension. They are not the
three tiers. Each family owns an active/equipped state, an ordinary world
pickup, and a persistent/respawning world pickup; action 78 is the shared
detached-spell return state. The selected loaded tier is held in the entity's
`byte_0x46_70` field and can change while the physical action remains the same.

| Physical role | Action formula | Behavior |
|---|---:|---|
| active/equipped | `3 × family` | validates owner/mana, applies or launches the family effect, advances cast and cooldown state |
| ordinary pickup | `3 × family + 1` | settles in world, attaches to an eligible player, updates inventory and hand selection |
| persistent pickup | `3 × family + 2` | performs ordinary acquisition and respawns a replacement at the same position |
| detached return | `78` | shared movement state that returns a removed inventory spell to its ordinary-pickup row |

All 26 active handlers are separately reduced. This preserves important
family-specific asymmetries: Castle does not decrement its countdown in the
same manner as the common handlers; Beyond Sight has no direct radar mutation
inside its active handler; Invisible sets/tests and clears flags on apparently
different objects; and Cave In only advances its cast logic in cave realms.

The owner table differs from the compiled initializer in 269 fields across 71
of 78 tiers, so compiled literals are not the live-value authority.

Corrected ordinary Fireball and Rapid Fire path:

```text
family 0 tier payload (250 or 160 in owned data)
  -> class 15 family-0 spell holder
  -> sub_6DCA0 factory
  -> class 9 model 0 projectile
  -> collision, terrain, ceiling, or lifetime termination
  -> class 10 model 0 impact entity
  -> once-only shared area scan
  -> target pending-damage accumulator and source link
  -> later class/action-specific life, shield, immunity, or death handling
```

Two earlier tempting interpretations were wrong:

- `sub_65780` performs hit/accuracy bookkeeping, not damage;
- `sub_686D0` stores the last-hit entity link, not damage.

Fire Storm uses a separate class-9/model-28 and class-10/model-76 chain and
remains explicitly unresolved rather than being forced into the ordinary
Fireball equation.

## Damage is staged, not one equation

The Fireball payload is not necessarily the final life lost. The shared impact
writer:

1. walks spatial entity chains;
2. applies collision, ownership, and overlap filters;
3. adds a payload to a pending-damage field; and
4. records the source entity.

Each target's later action can apply shields, immunities, special reactions,
death conversion, statistics, recoil, sounds, or spawns. The complete damage
map therefore needs a target-class/action matrix, not a single universal
`hp -= payload` formula.

## Objectives, multiplayer, replay, and random state

All ten objective predicates found in the objective evaluator have atomic
claims in the backbone. Objective evaluation occurs after entity updates and
before final world/audio/render stages.

The legacy multiplayer surface exchanges player/action and selected state
through reconstructed network functions. Its existence is preservation
evidence, not a recommendation to expose the old protocol to the internet. A
new game should retain deterministic input/state concepts behind a modern
server-authoritative transport.

The replay recorder stores exact per-level, per-player, per-turn action bytes.
The comparison harness saves and compares internal state. The compatible
Watcom random generator is part of the deterministic state. Together these
give three distinct test levels:

- pure unit tests for recovered formulas;
- same-input native regression against a pinned exact build; and
- player-feel comparison in the new engine.

## Audio, music, HUD, and rendering

Audio is downstream of authoritative state but part of perceived feel:

- outdoor water selects ocean ambience;
- outdoor non-water selects crickets;
- caves use a family of intermittent drip sounds;
- entity actions select event sounds;
- music starts after the first in-game turns; and
- sounds are prepared through positional/event playback state.

The minimap, world renderer, HUD, palettes, sprite animation, and music are
mapped as separate presentation systems. They should be treated as behavioral
consumers of simulation state, not mixed into the portable simulation core.
The exact presentation functions are still only provisionally named unless a
claim or runtime observation promotes them.

The owned HSPR and MSPR packs are now fully decoded for day, night, and cave:
1,572 indexed rows and 30 palette-correct contact-sheet pages. This is a visual
lookup surface, not an identity oracle; labels are promoted only when code or
data actually consumes the corresponding sprite or particle index.

## What remains before “as close as possible”

The remaining work is explicit and sortable:

1. finish uncertain living-actor identities using constructor, action, sprite, sound,
   localization, level occurrence, and runtime observation;
2. finish terrain mutation, volcano, crater, earthquake, vortex force, and
   cave parameter envelopes from load/seed through authoritative consumers;
3. partition the remaining raw writable state gaps by writer, initializer, load
   override, and consumer;
4. classify authoritative candidates as gameplay parameter, content,
   derived value, runtime state, structural selector, presentation, platform,
   debug/test, or unknown;
5. map target-specific damage, shields, death, mana, castle, village, creature
   AI, and spell chains;
6. map save and level records field by field;
7. map HUD/minimap/audio/music asset selectors to their actual owned assets;
8. use short deterministic traces for disagreements static evidence cannot
   settle; and
9. independently sample every semantic class and every high-impact unknown.

The goal is not to eliminate the word `UNKNOWN`. The goal is to ensure every
unknown has an address, source interval, surrounding graph, evidence status,
and concrete next experiment.

## Regeneration

```bash
research/private/remc2-atlas/build_semantic_backbone.py
python3 research/private/remc2-atlas/build_dispatch_atlas.py
python3 research/private/remc2-atlas/build_semantic_coverage.py
```

The three builders keep generated evidence owner-only. They read the pinned source
and a small hash-verified owned-asset subset; they do not modify the upstream
reconstruction or mount the GOG image.
