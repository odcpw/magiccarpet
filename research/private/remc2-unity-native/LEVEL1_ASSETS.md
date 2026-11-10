# Level 1 Modern and Voxel asset inventory

Verified 2026-11-09 against the active Level 1 owner package, current Rust
constructors and the installed Presentation client's source bindings.
Use the playable Rust level as the behavior baseline. This work owns art,
animation and presentation integration, not another mission implementation.

## Existing assets to retain

| Family | Current coverage | Remaining integration |
| --- | --- | --- |
| Goat | Approved master and five actions; Modern and animated voxel skins | Retain source speed, terrain contact and native death joins |
| Bow archer, bow, string and arrow | Approved master and eight actions; Modern and voxel character skin | Retain real native release, aim and child lifetime |
| Villager241 | Installed pilot: TMAP180–182, six residents; human/robe, five actions, Modern and 64/96 voxel skins | Retain native travel/reaction/death/reset/load; other appearances still need conversion |
| Terrain, water, shore, sky/clouds, wind and moonlight | Existing shared Modern environment | Whole-map voxel coverage, streaming and real gameplay checks |
| Decorative vegetation and rocks | Existing grass/fern/shrub/tree/rock resources and LODs | Decorative instances do not replace source-owned scenery |
| Buildings | Four BUILD profiles: 26, 29, 30, 42, covering 32 opening buildings | Additional selectors and dynamic construction/damage need coverage |
| Mana spheres | Native-owned 3D representation in Voxel | Modern replacement, ownership colours and capture/collection feedback |
| Fireballs | Native-owned voxel representation and impact fragments | Modern replacement and consistent emission/impact/expiry presentation |

Unclaimed visible entities still use original sprite billboards. Existing
attractive scenery does not prove that every gameplay object is converted.

## Remaining living assets

Counts are authored records unless noted. Later groups do not all exist at
the start. Further house/Castle production has conditional, non-fixed counts.

| Asset project | Level 1 occurrence | Required conversion |
| --- | --- | --- |
| Villagers | 28 at reset; four original appearances | TMAP180 ×6 delivered; TMAP155 ×3, TMAP183 ×9 and TMAP199 ×10 remain, with their admitted native states |
| Builders | Produced by houses, model 12 | Original appearance, locomotion/work/reaction/death and native production/removal binding |
| Traders | Produced by houses, model 14 | Original appearance, locomotion/work/reaction/death and native production/removal binding |
| Worm | Four authored heads, groups 4/34; each can allocate up to 16 segments | Head/repeating segments, native multipart placement, movement/attack/reaction/death and voxel breakup |
| Firefly | Six authored, groups 6/33 | Original silhouette, wing/body animation, flight/turn/attack/reaction/death and attack effects |
| Collector balloon | Spawned by player Castle, class 3/model 3 | Balloon, collection/carry/deposit states, ownership/material feedback and native flight |
| Castle auxiliary/defender | Produced at higher Castle stages, class 5/model 15 | Inspect original appearance; distinct from balloon and invisible shooting controller; match admitted native states |

Reset villager appearances are TMAP 155 ×3, 180 ×6, 183 ×9 and 199 ×10.
Builders use base TMAP 141 and traders 138. These are source appearances,
not assumptions about gender, clothes or tools before viewing the sprites.
Worm base TMAPs are head 30 and segment 66; Firefly base is 279; balloon 88.
Live animation/direction may select other frames.

The 2026-11-10 Villager241 pilot used no paid 3D generation. Its actual native
kill, checked Modern fall and Voxel breakup, R/F5 and fresh loads are recorded
under generated/villager-modern-20261109/runtime-character/. Both normal
launchers now use that qualified player; the previous Presentation pointer is
preserved. This does not complete this table's remaining families or whole-map
voxel coverage.

No Bee, Spider or Beetle was found in authored rows or audited direct
house/Castle producers. Do not commission generic bugs from the full game roster.
A newly observed natural producer must extend this inventory.

## Remaining structures and world objects

| Asset project | Source occurrence | Required conversion |
| --- | --- | --- |
| House/building library | 47 initial and 34 deferred constructor records | Complete source geometry/material variants, construction/damage/destruction and night windows |
| Player Castle | Dynamic construction | Source stage silhouettes, growth/construction/removal, owner markings and effects |
| Authored trees | 18, group 1 | Bind suitable existing meshes or remake variants; burning, stump and cleanup |
| Authored stone | One, group 6 | Source geometry, scale and live identity; preserve source interaction behavior |
| Dolmens | Three at reset | Source silhouette, material, contact/charge feedback and live placement |
| Exit portal/target | One, group 4 | Source appearance, inactive/active/entry and exit-sequence effects |

Initial BUILD selectors and counts:

| Selector | Instances | Existing Modern profile |
| ---: | ---: | --- |
| 17 | 1 | Missing |
| 26 | 9 | Present |
| 29 | 11 | Present |
| 30 | 8 | Present |
| 31 | 3 | Missing |
| 34 | 1 | Missing |
| 42 | 4 | Present |
| 47 | 2 | Missing |
| 50 | 3 | Missing |
| 51 | 2 | Missing |
| 52 | 2 | Missing |
| 53 | 1 | Missing |

The 34 deferred constructor records use BUILD 37. Nine additional selectors
need inspection: 17, 31, 34, 37, 47, 50, 51, 52, 53. Compare BUILD records before
claiming nine unique meshes; some can share geometry. Eight uncovered opening
selectors account for 15 starting buildings.

Modern substitution requires an original intact occupant with expected slot
generation, selector, action, position and stamped terrain cells. It does not
cover every later constructor or changed building. Voxel currently has one
selected house and a local terrain patch; whole-level coverage is integration work.

## Pickups, player presentation and effects

| Asset project | Source occurrence or role | Required result |
| --- | --- | --- |
| Experience scroll | Five authored | Readable high-resolution model, source motion and pickup feedback |
| Castle and Speed jars | One authored each | Original spell identity, shared vessel where appropriate, acquisition effects |
| Mana spheres | Runtime deaths/economy | Native lifetime/owner/amount in both styles, capture/carry/collection and save/load |
| Player presentation | First-person and admitted reflection/possession views | Original-informed carpet/hands/casting cues where visible; no invented rival population |
| Fireball | Available at start | Emission, travel, impact, fire/expiry; only impact debris bounces |
| Possession | Available at start | Projectile/contact/attraction and view transition |
| Castle | Acquired during Level 1 | Build projectile, construction and all reachable stage changes |
| Speed | Acquired during Level 1 | Activation/movement feedback and native expiry |
| Fire/explosion/ash/splash/smoke pack | Authored producers and gameplay children | Bounded effects, lighting, water contacts, burning and expiry |
| HUD and menus | Full Level 1 play | Life/mana, relevant spells/tiers, minimap, objectives, experience notices, pause/save/load/death/exit |

At reset there are nine ambient smoke emitters with nine smoke children, and
eight model-1 explosion/quick-fire producers with 14 model-0 children.
Deferred groups add 34 quick-fire producers, 31 model-5 splash/ash effects and
three checkpoint smoke emitters. Historical source names for model 5 conflict:
the constructor says water splash while its callback says Ash. Use original
frames and behavior to decide the visual, not a label.
Verified bases include scroll 259, jar 59, exit target 461, checkpoint smoke 57,
splash/ash 186, Fireball 463/impact 38, Possession 144/impact 54/attraction 8,
Castle projectile 61/root 96 and mana sphere 58/67.

Original music, speech, SFX and language resources remain in the owner pipeline.
Verify their cues through the full presentation cycle. This inventory does not
commission a new soundtrack or change gameplay effects.

## Delivery order and acceptance

1. Villager appearances and shared human rig, then Builder/Trader variants.
2. Firefly and Worm as independent creature projects.
3. Remaining building profiles, Castle and collector/auxiliary actors.
4. Authored scenery, pickups, portal and remaining effect replacements.
5. Full gameplay HUD and whole-map Voxel integration, then mission playtest.

Each project brings original frame references, editable master, materials,
required actions, Modern binding, a Voxel variant derived from the same master
where appropriate, and live evidence. Animation follows native travel/action/
child release. Rust positions, speeds, collisions and damage are not adjusted
to disguise a presentation problem. Voxel creature death explodes into pieces
and settles/fades, once per actual lifetime. Check scale, facing, contact,
shadows, release, damage/death, reset and load. Inspect important frames.

The final gate is a normal Level 1 run with every required family represented,
all dynamic groups/production covered, usable HUD/input, correct save/load, and
measured frame times through flight/combat/casting/destruction. Preserve art
quality and native simulation speed. Sprite fallbacks remain visible during
development so missing conversions cannot silently remove gameplay objects.

## Evidence and boundaries

The active owner-local package is
generated/settlement-20261105-integration/owner-closed-v6/level1.
remc2-level-exporter/scripts/campaign_census.py decoded 311 nonzero authored
records (308 with nonzero class and three class0 controls) and 177 active reset
slots. These include controls and constructor
children; neither is a mesh count. The owner manifest names Night sources
MSPRN0-0, MWEBN0-0, TMAPS1-0, BUILD0-0, PALN-0 and SKYN0-0, with spell/font/
language banks. Original data stays private.

Mappings were checked in campaign/mc2_level_definition.rs, house.rs,
native_world/creatures/settlements.rs and level1_mission.rs, with upstream
Events.cpp/EventsFunctions.cpp cross-checks. Replacement dispatch is in
ModernCharacterActors.cs and ModernEntityPresenter.cs, building coverage in
ModernBuildings.cs, decorative ownership in ModernVegetation.cs, and patch
coverage in Voxels/VoxelModernWorld.cs.

This is a source/package and installed-binding inventory, not full-mission
runtime proof or a claim that every transitive effect has been seen. Capture
new natural visual types during playtest and close their real conversion
before calling the level finished.
