# Level 1 Modern and Voxel asset inventory

Verified 2026-11-10 against the active Level 1 owner package, current Rust
constructors and the installed Presentation client's source bindings.
Use the playable Rust level as the behavior baseline. This work owns art,
animation and presentation integration, not another mission implementation.

## Existing assets to retain

| Family | Current coverage | Remaining integration |
| --- | --- | --- |
| Goat | Approved master and five actions; Modern and animated voxel skins | Retain source speed, terrain contact and native death joins |
| Bow archer, bow, string and arrow | Approved master and eight actions; Modern and voxel character skin | Retain real native release, aim and child lifetime |
| Villagers, all four appearances | All 28 reset residents: brown robe/cap, olive pilot, striped robe, vessel carrier; five actions, Modern and 64/96 voxel skins | Installed; retain native travel/reaction/death/reset/load and bounded breakup admission |
| Builder | Source-inspired vest/trousers/cap/beard, five actions and64/96Voxel skins; real Level1 house birth, construction, living entry and death | Qualified source7f554577; retain native identity, positive-life consumption and exactly-once breakup |
| Trader | Source-inspired short vest, patched trousers and goods pack; five actions, Modern and64/96Voxel bindings; genuine Level1 demolition birth, public combat and positive-life entry | Candidate Modern/Voxel combat, living-entry and F5/R/coldload checks pass; latest installed launchers and performance remain open in mc-oi5 |
| Firefly | Original-inspired olive/bronze body, violet eyes, warm wings and amber underside; actual Unity25-pose import, coloured Modern/Voxel bindings | Real Level1 combat and F5/R/coldload pass both styles; Modern fade0.167s; clearer breakup camera/media and installation pending in mc-eb8 |
| Worm | Source-inspired red lens/curved jaws and repeating segment master; corrected baked size/culling, actual Unity25+2-pose imports, independent native multipart placement | Natural group4 birth, all16linked bodies, actual combat, one8192-cell breakup and both-style F5/R/coldload verified; clearer breakup media, performance and installation remain open in mc-7uj |
| Terrain, water, shore, sky/clouds, wind and moonlight | Existing shared Modern environment | Whole-map voxel coverage, streaming and real gameplay checks |
| Decorative vegetation and rocks | Existing grass/fern/shrub/tree/rock resources and LODs | Decorative instances do not replace source-owned scenery |
| Buildings | Installed:32 opening buildings/four profiles. Current candidate:all41 architectural buildings/eight profiles, imported in Unity | Candidate gameplay/installation, dynamic construction/damage and Castle stages remain open |
| Mana spheres | Native-owned 3D representation in Voxel | Modern replacement, ownership colours and capture/collection feedback |
| Fireballs | Native-owned voxel representation and impact fragments | Modern replacement and consistent emission/impact/expiry presentation |

Unclaimed visible entities still use original sprite billboards. Existing
attractive scenery does not prove that every gameplay object is converted.

The active candidate's shared native source is6e82c62e, releaseSHA256
2f441ac433d01de189a66b1be1e4027079d9c2615c5ee7e74a94eac29326cab7.
Its strict96-byte creature observation fixes stable Worm part identity and
publishes actual source cadence/particle height. It also fixes the reproduced
Castle holder phase discontinuity and distinguishes positive-life house entry
from lethal death. The earned public command chain is reexecuted unchanged
into new native-created replay cuts because the corrected engine changes
state hashes; old saves/hashes are preserved and not edited or relaxed.
Trader172/gen27 is now proven to originate from House10 demolition at tick8018.
Its public lethal branch reaches life−1/action116 at8648 and one death record;
its neutral branch enters House14 with life999 and no death. Both have cold
native verification. Actual candidate Modern/Voxel lethal/entry and F5/R/coldload
checks pass; matching installed launchers and final performance remain open.

Source49895483 culls only equal-weight coincident interior faces in the three
new64-cell creature skins. All source cells and four exact posed fragment
goldens remain unchanged;140focused tests pass. Same-runtime/native private
3090KWin/Vulkan1280x960 no-readback Voxel combat improves29.25/34.22 to
36.26/39.95FPS, with~44m→~22m triangles and~8.5→~5.3ms medianGPUtime.
Physics tails and~19native ticks/s remain unacceptable; these candidates are
not installed and the level is not performance-qualified. One contaminated
candidate Modern sample is excluded. Current evidence lives under generated/
level1-assets-20261110, including creature-surface-comparison.json. A review
camera lens reset at native removal still needs checked corrected media.

Current normal launchers select generated/settlement-people-20261110/player-reader
(presentation7f554577, nativead803e03 from65724d7b). The previous player is
preserved. Builder's real production/build/entry/kill, both installed styles,
public F5/R and coldloads are qualified; mc-jcy is closed. Trader art/bindings
are installed but mc-oi5 remains open for Level1 production and combat/death.
Checked media and current proofs are under that generated folder. No new asset
family was started before that unfinished acceptance. Matched private3090
Vulkan1280×960 two-run medians218/113FPS compare with old-player229/120;
no speedup/full-level/desktop claim, and>100ms frame outliers remain.

## Remaining living assets

Counts are authored records unless noted. Later groups do not all exist at
the start. Further house/Castle production has conditional, non-fixed counts.

| Asset project | Level 1 occurrence | Required conversion |
| --- | --- | --- |
| Traders, remaining runtime qualification | Conditional house production, model14; all47 opening houses have2 occupants | Art/bindings ready; obtain genuine Level1 production and public combat/death/voxel-lifetime evidence without state injection |
| Worm | Four authored heads, groups 4/34; each can allocate up to 16 segments | Head/repeating segments, native multipart placement, movement/attack/reaction/death and voxel breakup |
| Firefly | Six authored, groups 6/33 | Original silhouette, wing/body animation, flight/turn/attack/reaction/death and attack effects |
| Collector balloon | Spawned by player Castle, class 3/model 3 | mc-chx: candidate06 editable model/export and readonly child consumer; earned birth/flight, four500-mana collections and2000 deposit proven. Darker owner tint passes a checked 150-frame integrated6 night flight; Voxel form, visual reset/load and installation remain open. |
| Castle auxiliary/defender | Produced at higher Castle stages, class 5/model 15 | Inspect original appearance; distinct from balloon and invisible shooting controller; match admitted native states |

Reset villager appearances are TMAP 155 ×3, 180 ×6, 183 ×9 and 199 ×10;
all are converted and installed as of 2026-11-10. Their source logical heights
are 200/250/250/300. Both Day and Night sheets were inspected; Night palette
colours are not neutral fabric colours. See character-pipeline/VILLAGERS.md.
Builders use base TMAP 141 and traders 138. These are source appearances,
both inspected in Day/Night banks. Builder has an open olive vest and pale
trousers with green bands; Trader carries a large pack, blue rolls, sticks
and side bags. Source frames show no Builder hammer. Their reusable free
recipes and exact runtime limits are in character-pipeline/VILLAGERS.md.
Worm base TMAPs are head 30 and segment 66; Firefly base is 279; balloon 88.
Live animation/direction may select other frames.

The complete 2026-11-10 villager roster used no paid 3D generation. All three
new appearances have actual public-input kills, checked Modern falls and
exactly-once Voxel breakups. Both current styles pass R/F5 and fresh loads with
28 unique residents and all four bindings. Those launchers used the qualified
player in generated/villager-roster-20261110/player-view; the previous pilot
pointer is preserved. That source was through `9cf9159f`; native was
qualified `e8a6e707` from `eaf0f1bc`. Matched private KWin/3090 Vulkan1280×960
runs measured ~222/120 FPS Modern/Voxel; >100 ms spikes and native throughput
below 30 ticks/s remain. This does not finish the remaining families or whole-
map voxel coverage. Checked media and proofs are under that generated folder.

No Bee, Spider or Beetle was found in authored rows or audited direct
house/Castle producers. Do not commission generic bugs from the full game roster.
A newly observed natural producer must extend this inventory.

## Remaining structures and world objects

| Asset project | Source occurrence | Required conversion |
| --- | --- | --- |
| House/building library | 47 initial and 34 deferred constructor records | Complete source geometry/material variants, construction/damage/destruction and night windows |
| Player Castle | Dynamic construction | Source stage silhouettes, growth/construction/removal, owner markings and effects |
| Authored trees | 18, group 1 | Source-backed teal canopy passes 150-frame live view; fireball live→burning→bare native sequence proven; Voxel graphics, performance and installation remain open |
| Authored stone | One, group 6 | Candidate07 passes five Unity import/material checks, exact earned current-native replay and checked 120-frame Modern night view. Its first Voxel capture used the wrong camera and showed a building; corrected view and installation remain open. Ordinary Fireballs do not directly collide with unchanged stone in source. |
| Dolmens | Three at reset | Source-grounded revised model passes 150-frame normal-camera hillside view; charge feedback, Voxel view and installation remain open |
| Exit portal/target | One, group 4 | Source appearance, inactive/active/entry and exit-sequence effects |

Initial BUILD selectors and counts, corrected by the actual material/height
stamps on 2026-11-10. The47 owner records contain41 architectural buildings
and six terrain/control stamps; they are not47 house meshes.

| Selector | Instances | Existing Modern profile |
| ---: | ---: | --- |
| 17 | 1 | Empty terrain/control stamp; no house mesh |
| 26 | 9 | Present |
| 29 | 11 | Present |
| 30 | 8 | Present |
| 31 | 3 | Source profile prepared; actual Unity geometry gate passes |
| 34 | 1 | Source profile prepared; actual Unity geometry gate passes |
| 42 | 4 | Present |
| 47 | 2 | Source profile prepared; actual Unity geometry gate passes |
| 50 | 3 | Source profile prepared; actual Unity geometry gate passes |
| 51 | 2 | Water/terrain stamp; preserve native terrain |
| 52 | 2 | Water/terrain stamp; preserve native terrain |
| 53 | 1 | Water/terrain stamp; preserve native terrain |

The34 deferred constructor records use BUILD37 and still need source-specific
terrain/fire presentation qualification. Four missing architectural profiles
31/34/47/50 cover nine buildings. The candidate layout validates all41 exact
source positions,1729 stamped cells, eight profile shapes and the frozen
height-plane hash; Unity generated all three mesh LODs per profile. This is
preparation evidence, not installed dynamic-building coverage.

Builder construction can choose additional selectors19/22/32/33/35/39/40/62/74,
along with the opening architectural styles. The source construction payload
uses effective dimensions rawWidth>>1/rawHeight>>1 and the contiguous prefix
of its cells; downsampling the entire raw matrix gives the wrong geometry.
These later templates and their actual living constructor consumers remain open.

Modern substitution requires an original intact occupant with expected slot
generation, selector, action, position and stamped terrain cells. It does not
cover every later constructor or changed building. Voxel currently has one
selected house and a local terrain patch; whole-level coverage is integration work.

## Pickups, player presentation and effects

| Asset project | Source occurrence or role | Required result |
| --- | --- | --- |
| Experience scroll | Five authored, group 4 | Current-native earned cut shows five live rows and ordinary controls collect all five with effect4 and sound63 each; focused source test proves exact +4 XP. First integrated6 Modern view was rejected as a dark cylinder. Revised lighter layered parchment and spiral ends pass 2/2 Unity import tests; integrated7 live views and installation remain open. |
| Castle and Speed jars | One authored each | Both share source TMAP59 vessel; Castle live Modern/Voxel views and native pickup/reset/cold-load pass. Speed naturally spawns at t1107 and is collected by t1183 through ordinary input; shared-presenter Unity tests pass 4/4. Checked 90-frame integrated6 Modern/Voxel views show stable jar form. Exact contact, on-screen earned disappearance, actual player F5/R and installation remain open. |
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

1. Villagers and Builder are converted; Trader's full runtime acceptance stays open. Oliver's later all-assets authorization permits independent Firefly/Worm preparation while its native checks continue.
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
