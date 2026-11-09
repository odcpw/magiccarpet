# Character animation experiments

## Fitted human and portable skin export — 2026-11-10

The current Villager241 route uses a real human and separate authored clothing.
The supported MPFB/hm08 clothing fit is under review, with the free CC0 Donitz
monk robe as a starting garment. Its own opaque-clothing body mask is legitimate
game geometry; retain the full anatomical source for editing/collision and check
exposed neck, hands and legs through complete actions. Do not use arbitrary
height cutoffs. [MakeHuman's explanation](https://static.makehumancommunity.org/assets/creatingassets/makeclothes/makeclothes_deletegroups.html)
describes the intended mask and why fitting alone cannot guarantee posed skin
clearance. The [official asset index](https://static.makehumancommunity.org/assets/assetpacks/suits02.html)
identifies the robe as CC0. Static fit does not approve moving cloth.

`human_figure.py` authors procedural own-Rigify actions; they are not mocap.
`export_character_fbx.py --bake-rig` freezes the active anatomical rest morph
mix, retains only weighted deform bones, and concatenates evaluated actions
onto a portable rig. It rejects multiple/DQ armature modifiers, corrective or
cloth simulation, weighted segmented B-bones and non-normalized/>4 influences.
Body and robe remain separate in the editable master; `--join-skins` combines
disconnected components only in the game export. The default prepared-scene
export mode remains available.

For example, with the retained local body diagnostic and a fresh output path:

```sh
/home/oliver/Projects/fittimer-avatar-toolchain/blender-4.5.12-linux-x64/blender --background research/private/generated/human-figure-20261109/candidate08/human-master.blend --python-exit-code 1 --python research/private/character-pipeline/export_character_fbx.py -- --output research/private/generated/portable-human-unique/human.fbx --animations --bake-rig cm_vitruvian_rig --mesh cm_vitruvian --action Idle --action Walk --action Run --action Hit --action Death --join-skins
```

Outputs include the FBX, `.export.json` and little-endian `.samples.bin` pose
samples (`MCSK`, u32 version1, u32 sample count, f64 FPS; each sample has u32 frame,
u32 vertex count, then float32 XYZ). The exporter checks actual posed surfaces
before and after FBX reimport. Blender's importer adds a +1-frame offset by
default and invents connected joints that suppress valid animated translations;
the verifier uses zero offset and restores the compact rig's disconnected
transform semantics. Never loosen the surface gate to hide that discrepancy.

Root self-verified the 86-bone body diagnostic at 25 poses over all five actions:
source-to-game maximum 2.35 micrometres, corrected Blender FBX reimport maximum
0.0235 mm. Actual Unity6000.3.20f1 import then matched every sampled point in both
directions within 0.1 mm, using one fixed `(x,z,y)` axis conversion. Unity split
37,436 source vertices into 42,841 UV/material vertices without losing the
sampled surfaces. Evidence is retained in
`../generated/human-figure-20261109/export-probe/`; the Unity probe runs in the
isolated villager project. This establishes the body export consumer, not the
dressed villager, gameplay, Voxel death, frame rate or installation. `mc-fu7`
remains open and the accepted installed player is preserved.

The matched MPFB motion02 body also passes: 188 weighted deform bones,
source-to-game maximum 1.57 micrometres, FBX reimport maximum 0.0217 mm, and
25 actual Unity poses with no unmatched points within 0.1 mm. Masked fitting
helpers are checked after evaluating REST geometry; deliberately exposing the
16 unweighted helpers is rejected. The full editable body is retained.
`measurements-audited.json` records measured stance speeds 1.4300195/3.7283345 m/s,
action timings, samples and source/master/builder hashes. The garment is still
under repair and is excluded from these body-only export claims.

## Local TRELLIS villager comparison — 2026-11-09

Oliver requested local TRELLIS and a comparison with the handmade Villager241,
with no paid 3D generation. The isolated runtime now executes textured jobs on
the 3090 through loopback port 8189. It uses a project-local Python 3.12/Torch
2.10+cu130 environment, SDPA, the existing pinned ComfyUI/node sources and
17.60 GB of existing model weights. No model download or paid 3D API was used.
The environment occupies 8.9 GiB on SSD; large cached weights remain on HDD.

Owner-local reproduction and actual outputs are under
`../generated/villager-modern-20261109/local-trellis/`. Its `README.md` records
the exact transient user-service start/status/stop commands, environment pins,
kernel checks and saved API graphs. Unit
`magiccarpet-trellis2-local-20261109.service` is manually started, not enabled
at login. Never use the unrelated Pinokio service on 8188 for these jobs, or
overlap model generation with another GPU owner's render. The original pack
doctor does not understand this relocated mirror; it is not a clean doctor
pass. Actual CUDA kernels, HTTP node schema and completed textured jobs are
the working-runtime evidence.

Matched front/back inputs and seed12345 produced these real jobs; times below
are server execution durations. Memory is sampled total GPU use during each
job, not reserved model memory or a repeated performance benchmark:

| Starting mesh | Server time | Largest observed GPU use | Raw triangles | Visual result |
| --- | ---: | ---: | ---: | --- |
| Handmade Blender | Not measured | Not measured | See retained master | More controllable mesh, but flat robe, poor shoulders and visible garment gaps |
| TRELLIS 512, two views | 347 s | 4057 MiB | 486,498 | Dimensional cloth/turban; crude dark face and fused extremities |
| TRELLIS 1024, two views | 390 s | 6711 MiB | 436,135 | Better face and folds; side fold and hem/ankle geometry still distort in motion |
| TRELLIS 1024, four views | 508 s | At least 5513 MiB | 440,982 | Better closed side drape at rest; pale eyes and lower-garment deformation in diagnostic Run pose |

The four-view memory sampler missed 61 seconds after its initial observer
exited; a durable monitor reattached to the same prompt. Its actual peak may
be higher. No job was resubmitted. The server remained active and its successful
history and exact output hash were independently checked by Root.

Raw outputs are reconstruction meshes, not game assets. A roughly 30k-triangle
derived mesh and proxy-assisted rigging were exercised in Blender, but the
moving villager is unapproved. Four-view reconstruction improves the long
side-slit appearance at rest, but a diagnostic nearest-surface transfer to the
same Run rig/phase opens the lower gown into stretched triangles. Matched clay
confirms deformed geometry; this transfer is not a production rig and its
shoes were not yet separated. It does not prove the unrigged gown itself is
broken. All three raw outputs have zero skins and zero animations: TRELLIS does not
deliver the requested character animation. Clean garment/foot separation,
retopology and skinning remain necessary.
The profiles' human filenames are not the node socket names: face-image-left
goes to node right (+X), face-image-right to node left (−X), front is +Z and
back is −Z. The parent generated directory's `reference/selected-prompts.json`
retains the selected imagegen prompts and mapping; TMAP/palette hashes remain
in `reference/source.json` and `reference/day/source.json`. `villager_reference.py`
re-extracts original villager views into a new output directory without
overwriting existing work. A fresh Day241 extraction reproduced all 14 source
PNG/JSON files byte-for-byte.

For example, from the repository root with the existing owner cache:

```sh
uv run --no-project --with pillow python research/private/character-pipeline/villager_reference.py --cache research/private/generated/level1-unity-asset-cache --output research/private/generated/villager-reference-unique
```

Choose an unused output directory. Use `--world day` with a material cache
containing `palette/pald.rgb8` and `terrain/tmaps0` for neutral Day references.

Use matched Eevee/PBR and clay renders to compare geometry. Workbench studio
specular made the robe look metallic despite the actual texture having zero
metallic and approximately 0.99 roughness; that preview is not material proof.
The comparison favors TRELLIS1024 as a sculpting base, followed by deliberate
Blender retopology and one shared rig/timeline. Four-view cloth has better side
geometry; two-view has the better face. Neither is an approved game character,
and no face-swap or automatic-weight shortcut is qualified. This does not
establish complete animations, Unity gameplay, Voxel death or installation.
`mc-fu7` remains open; accepted goat,
archer and installed players are preserved.

## Live Level 1 character delivery

The accepted cycle replaces the actual Level 1 goats, archers and native archer
arrows. Oliver has authorized the remaining Level 1 Modern/Voxel assets; the
current source-verified scope is in
[LEVEL1_ASSETS.md](../remc2-unity-native/LEVEL1_ASSETS.md).
`run-characters.sh` starts the qualified Modern campaign; `--preview` retains
the earlier isolated inspection player. Rust owns movement, damage, projectile
allocation and cleanup. The Unity presenters consume read-only observations;
they do not create damaging arrows or change original movement speeds.

| Owner / asset | Required result | Decisive check |
| --- | --- | --- |
| Goat Terra owner | Sprite identity, idle, gallop, flee, hit, death; distance-driven gait and terrain contact | Original native displacement measurements, imported rig/contact checks and actual Level 1 frames |
| Archer Terra owner | Sprite identity, walk, standing/kneeling draw and recovery, hit/death; complete arrow mesh and nock | Level/up/down shaft checks, actual allocated source child, important release/recovery/death frames |
| Integrator | Native slot generations, terrain placement, camera/input, removal/reset, installation and phone delivery | Same-revision native/player hashes, real gameplay captures and launcher checks |

The new goat export is `../generated/characters-complete-20261107/goat-gallop5/`;
the archer candidate is owner-local at
`/mnt/pinokio-shared/magiccarpet-archer-level-20261107-candidate20/`.
The previous selected assets below remain preserved comparisons. Source speed
18 raw units/tick gives measured ordinary goat travel about 8.438 Unity units/s
at 30 Hz. Flee speed54 gives about 25.322 units/s. These are game coordinates,
not calibrated real-world metres. The former tiny stride forced implausibly
fast foot cycling; the authored gallop and sprint now advance by actual
interpolated horizontal distance. The imported model's front is checked against
native travel at all four cardinal headings.

Pipeline fixes learned from the live cycle:

- Add all required actions to the canonical rig/timeline before calling an
  asset complete; five goat clips and eight archer clips are currently required.
- Keep original source speed and derive animation phase from travel. Do not
  guess animation FPS from a sprite sheet or assume the preview's treadmill
  speed equals gameplay speed.
- Observe health and source actions. Zero goat life is still alive; death is
  negative life or source death actions. Distinguish visibility culling from
  actual removal and retain only a bounded visual death tail.
- Join source slot, source ID, allocation generation and world generation.
  Imported generation zero is valid. Reset/dispose all joins on world changes.
- Use the source archer target field and an actual allocated arrow child.
  A periodic firing attempt may fail allocation; it cannot release a visual
  arrow. Do not re-trigger a shot from an old in-flight child loaded in a save.
- Align the exported shaft, held nock and native flight direction. Correct
  world-space aim after terrain tilt. Same-height targets are horizontal;
  elevated targets pitch coherently. The child starts at the held nock and
  joins its authoritative native trajectory over one simulation tick.
- Inspect transition frames as well as the individual clips. Live recording
  exposed an abrupt recovery-to-next-draw join that isolated clip checks missed.
  Standing aim also needs a continuous exit to idle/walk. Preserve the authored
  walking lower body and its Armature height while joining the aimed upper body;
  preserving leg bones alone left both shoes floating. Compare deformed soles
  with the same imported Walk sample, rather than another crossfade.
- Keep moving damage reactions over locomotion. The goat's authored Hit is a
  head recoil; sampling it as a whole-body clip froze moving feet. Its actual
  imported head track now overlays the distance-driven gait, with a short grace
  for measured zero-displacement interpolation boundaries. A stopped goat still
  uses the stationary Hit action.
- Measure deformed hoof soles against the actual rendered terrain. A flat-floor
  import check does not prove hillside contact. Visible goats use bounded
  two-bone correction, preserving segment lengths and the airborne gait phase.
- Record actual gameplay with the existing capture tool and an earned public-input
  replay. The optional spectator camera admits its selected real actor even
  outside the player's native draw list; captures disclose this override.
  Higher recording cadence resolves fast arrows while native simulation stays
  at 30 Hz. Capture cadence is never a desktop performance measurement.
- Fit the review camera to terrain-oriented body/held-prop bounds and keep it
  above ground; reject every recorded framing failure. The capture receipt
  hashes the player, native library, runtime assembly and build receipt before
  and after recording, so later installs cannot silently change its identity.
- Keep separate Unity import databases for separate qualified projects. An
  observed stale MonoScript association produced zero script pointers in both
  character assets. Import and Modern build now validate the actual script
  binding, required clips and Resources load; unresolved imports fail before
  writing a replacement. A fresh Editor invocation after script reimport
  resolved the observed case, with the broken products preserved as evidence.

The final integrated character checks pass 48/48, including moving-hit
interpolation boundaries and both standing-aim exits. The installed native
is `dc5a115321b1780f5f328ea6c6e2edad1cf4818885e655ff5e32365d58b2288d`.
Normal Modern and character launchers select the qualified campaign player;
`run-characters.sh --preview` retains the earlier inspection player and the
environment sandbox retains its frozen input/profile selection. Source-shaped
buildings and the rest of the approved environment remain included.

This is not a performance signoff. Private unthrottled gameplay at 1000x700
recorded median37.77ms/p9569.40ms; campaign processing dominated measured CPU
time, while the Modern render scope averaged1.97ms in the preceding sample.
The runs were not controlled before/after comparisons and the desktop remains
locked. Keep the gameplay CPU/performance issue open; recording-rate videos
demonstrate visual behavior rather than achievable FPS. UBS found no critical
issues but its C# semantic coverage is partial; Unity compilation, focused
checks, changed-hunk review and live captures supply the remaining evidence.

Live evidence and preserved losing candidates are under
`/mnt/pinokio-shared/magiccarpet-character-level1-evidence-20261107/`.
The checks and installed result are recorded in the current WORK_PLAN section.
The 24.5-second portrait review is at `http://3090.bumblebee-hue.ts.net:8095/`
inside the tailnet, with chapter buttons, mobile/HQ downloads and HTTP range
seeking. Its `phone-delivery-final/delivery.json` records the checked builds and
the complementary standing-exit angles. Important frames were inspected by root
and the Terra owner; terrain/rocks obscure some live shoes, so do not claim
precise archer hillside sole clearance from those images. Flat imported geometry
supplies the measured transition contact check. In captures only,
`-mc-character-follow-yaw <degrees>` selects another inspection angle without
changing native state.

### Level 1 asset inventory — current conversion scope

The 2026-11-09 source/package and installed-presentation audit is recorded in
[LEVEL1_ASSETS.md](../remc2-unity-native/LEVEL1_ASSETS.md). It adds house-produced
Builders/Traders, four Villager appearances, Castle collectors/auxiliaries and
incomplete building/voxel coverage to the authored census below. Goat, archer
and arrow delivery is retained. The remaining assets are now authorized.

Counts below come from the canonical package's `authored-level.bin`, parsed by
`remc2-level-exporter/scripts/campaign_census.py:parse_authored`, and source kind
mapping in `campaign/mc2_level_definition.rs`. They count authored records,
including later activation groups, rather than simultaneous live actors.

| Character family | Authored records | Work after the active cycle |
| --- | ---: | --- |
| Goat | 33 | Retain accepted gait/contact/damage/install cycle |
| Bow archer and arrow | 4 | Retain accepted aiming/shooting/damage/install cycle |
| Villager | 28 | Source-sheet identity; idle/walk/run, interactions and death; native resident binding |
| Worm | 4 | Source-sheet identity; movement/attack/hit/death; burrow/body/ground contact |
| Firefly | 6 | Source-sheet identity; flight/turn/attack/hit/death; wings and projectile binding |

The flying enemies authored in this Level 1 package are Fireflies. Do not infer
Bees from the general game's creature list. House production can create further
residents; its own source runtime determines those occupants.

The earned Level 1 replay's typed `definition.level_definition.entities` also
confirms the non-character inventory below. Emission-site counts are not counts
of simultaneously visible effects or distinct meshes.

| Authored visual/content family | Records | Remaining asset work |
| --- | ---: | --- |
| Houses/building constructors | 81 | 47 initial and 34 deferred; retain four profiles, inspect nine uncovered selectors and complete dynamic construction/destruction |
| Trees | 18 | Reuse suitable environment meshes; add authored live/burning/stump bindings |
| Stone | 1 | Bind source-owned geometry and interaction state |
| Dolmen | 3 | Source-matching model and charge/contact feedback |
| Experience scroll | 5 | High-resolution collectible model/material and pickup feedback |
| Exit portal | 1 | Portal surface, activation/entry and transition presentation |
| Castle pickup | 1 | Pickup model, player castle stages and construction effects |
| Speed pickup | 1 | Pickup model and speed presentation |
| Ambient smoke emitters | 9 | Retain environment work; verify each live emission binding |
| Model-5 splash/ash effects | 31 | Match actual source frames, locations and bounded particles |
| Explosion/quick-fire producers | 42 | Modern source-driven effects and fire children without changing damage |

The 2026-11-09 audit resolves three scenery-selector 2/2 records as Dolmens,
one 3/4 record as the player-start marker, two 10/29 records as linked endpoints
of one terrain/path construction, and three 10/59 records as checkpoint smoke
emitters. They are not extra creature families. The model-5 effect's constructor
says water splash while its historical action says Ash; use actual frames and
behavior. Class0 and typed area/objective/exit/absence gates are control data.

The rest of the Level 1 visual inventory includes terrain/shore/water/sky and
night lighting, trees/stone and decorative scenery, source-shaped house/building
variants and construction/destruction, player carpet/hands and rival/player
presentation, Castle and Speed spell pickups, experience scrolls, mana spheres,
the exit portal, fireballs/arrows/impact/fire/smoke effects, HUD/map/menu assets
and sound. Existing environment/building work is retained; that does not certify
every gameplay construction or destruction transition. Invisible triggers and
objective gates require behavior, not invented visible creature assets. Each
future character follows the same reference → normalized mesh/PBR → rig/full
actions → native binding → live frame repair → qualified install cycle above.

Fresh sprite-informed goat experiment: `../generated/goat-animation-20261106/`.
Original sprites inform identity; no rejected prototype mesh, rig or motion is
reused. Generated files, original assets and credentials remain owner-local.

## Checked complete-action Unity characters

The earlier one-second loops below are retained experiments. The current work
uses one synchronized FBX timeline per character, editable Blender scenes,
explicit PBR texture files and a JSON manifest of named clip ranges. Unity's
native importer creates Generic rigs and the review player uses the actual
imported clips through `ModernCharacterInstance`.

The goat has a separately authored four-hoof standing idle and the repaired
walk. The archer has Idle, Walk, a five-second reach/nock/draw/release/recover
action, kneel preparation/descent, a kneeling shot and a rise. Modern motion
between the source game's held sprite stances remains authored interpretation.
The hands retain modeled grips rather than individual finger articulation.

Load-bearing integration fixes:

- Normalize model facing, origin and scale before export; keep one bind pose.
- Export all body, bow, string and arrow tracks on one timeline. Independent
  NLA stacks in FBX lose synchronization between those objects.
- Give packed images distinct filenames and retain explicit albedo, normal and
  ORM maps. Empty filenames caused the FBX reimporter to use albedo as normals.
- `ModernCharacterImport` converts ORM channels into Unity metallic, occlusion
  and smoothness, then creates URP materials and named clips from the manifest.
- Preserve fine contact keys with `ModelImporter.resampleCurves = false`.
  Resampling lost the planted walk and separated the nock from the hand between
  authored presentation frames; the same exported FBX passes with its keys kept.
- Unity geometry checks use `BakeMesh(mesh, true)` before `TransformPoint`.
  Otherwise the goat renderer's import scale is applied twice to the measurement.

The selected assets are `goat-neutral2` and `archer-candidate12` under
`../generated/characters-complete-20261106/`. Each contains its editable blend,
FBX, clip manifest and three source texture maps. All 12 focused Unity checks
pass (`unity-final-check-tests.xml`, 2026-11-07). Actual player frames exposed a
front shoe that lifted/slid during rising and an idle inherited from walking;
the selected archer has a separately grounded idle and planted-front-leg IK.
The rise check measures a maximum 5.8 mm sole height and 12.6 mm horizontal
centroid drift; the previous candidate failed at 182 mm and 412 mm respectively.

The first 36.4-second portrait capture is retained at
`/mnt/pinokio-shared/magiccarpet-character-review-portrait/`, with a separate
visual rejection receipt: its raised bow and held arrow overlapped/crossed the
view boundary. The selected player uses Unity's conservative world culling
bounds to fit the character and held props inside a separate viewport, with
titles outside it. A discarded preview snapshot calculation enlarged scaled
bow props by 100x after the first frame. The corrected actual-player capture
has 874 complete 1080x1920 frames at 24 fps (36.42 seconds), with zero finite/
near/far/viewport failures:
`/mnt/pinokio-shared/magiccarpet-character-checked-unity/characters.mp4`.
Goat gait, standing draw/release, descent, kneeling shot, rise and settled stance
frames were inspected by both the motion owner and integration owner. The real
interactive player also passed keyboard selection/pause, Shoot, slider scrub
and clean Escape exit on a private display. Recording is presentation evidence,
not a desktop performance qualification.

The installed inspection player is launched with
`research/private/remc2-unity-native/run-characters.sh` from the repository root.
Its `Builds/Characters` link selects the checked player on the HDD. The phone
review is available within Oliver's tailnet at
`http://3090.bumblebee-hue.ts.net:8095/`, with chapter buttons and MP4 download.
Its local-only service reuses FitTimer's existing range-serving implementation;
it serves a separate directory containing only this review page and media.

Canonical implementation:

- `goat_complete.py`: load the selected rigged goat blend; author compatible
  idle/walk and bake a ground root, then reopen before FBX export to refresh
  Blender's cached bind matrices.
- `archer_complete.py`: assemble the selected walk, standing and kneeling
  blends into complete actions; bake contact and ground corrections at 96 Hz.
- `character_textures.py`: preserve source map bytes with distinct image paths.
- Unity `ModernCharacterImport.Prepare`: import clips, textures, prefab and
  definition from `Assets/Resources/ModernCharacters/{Goat,Archer}/`.
- Unity `ModernCharacterBuild.Build`: build the interactive inspection player;
  it does not open a campaign or a save.
- Unity `Tools/capture-character-review.py`: record the player on an owned
  private display, encode full frames and retain logs/receipts on failure.

Focused checks and a new player build, after staging owner-local exports:

```sh
UNITY=/home/oliver/Unity/Hub/Editor/6000.3.20f1/Editor/Unity
CLIENT="$PWD/research/private/remc2-unity-native"
"$UNITY" -batchmode -nographics --burst-disable-compilation \
  -projectPath "$CLIENT" -runTests -testPlatform EditMode \
  -testFilter MagicCarpet.Native.Tests.ModernCharacterEditModeTests \
  -testResults /absolute/new/tests.xml -logFile /absolute/new/tests.log
MC_CHARACTER_IMPORT_REPORT=/absolute/new/import.json \
MC_CHARACTER_PLAYER_OUTPUT=/absolute/new/player/MagicCarpetCharacters.x86_64 \
  "$UNITY" -batchmode -nographics -quit -projectPath "$CLIENT" \
  -executeMethod MagicCarpet.Native.Editor.ModernCharacterBuild.Build \
  -logFile /absolute/new/build.log
python3 "$CLIENT/Tools/capture-character-review.py" \
  --player /absolute/new/player/MagicCarpetCharacters.x86_64 \
  --output /absolute/new/capture
```

Stage the selected `goat.fbx` and `goat.clips.json` as `Goat.fbx` and
`Goat.clips.json` in `Assets/Resources/ModernCharacters/Goat/`; likewise capitalize
the archer pair in `Archer/`. Copy each export's `textures/` folder there too.
These files and all importer products remain ignored. The test-only Burst flag
avoids an observed Editor startup/unused-asset-GC deadlock; the built player
retains normal Burst compilation.

Original campaign animation mapping, terrain foot placement, deaths and crowd
LODs are separate from these character assets. This is a working author/export/
import/inspect path for two characters, with character-specific motion repair;
it is not a general automatic image/video-to-rigged-animation system.

The initial experiment separates reference art, a fixed textured mesh, motion
video and deformation reconstruction. A video is only a motion reference. The
deliverable must be independently renderable 3D geometry, inspected from both
sides and the rear. A morph animation does not establish a reusable skeletal rig.

## Skeletal goat follow-up

Oliver accepted the fresh textured goat as the starting shape and requested
Blender repairs. `rig_goat.py` now binds that mesh to 14 bones, preserves the
skull and hoof shapes, solves fixed-length leg chains, and authors a one-second
four-beat walk with planted stance intervals and restrained body/head motion.
Two small reconstruction ridges receive localized smoothing; their faint baked
texture marks remain. This is a manually authored interpretation of the four
original sprite phases, not recovered original animation timing.

The selected asset is `../generated/goat-rigged-20261106/eighth/goat-rigged.glb`
with its editable `.blend`. `goat-multiview-review.mp4` shows four directions;
`goat-treadmill-review.mp4` shows three repetitions with a checker floor moving
at the authored stance speed. These are actual renders of the exported GLB.
Individual side, opposite-side, front, rear and three-quarter contact frames
were inspected. No goat asset has been installed in Unity in this follow-up.

Reproduce from the approved fresh Tripo mesh (not the old rejected prototype):

```sh
BLENDER=/home/oliver/Projects/fittimer-avatar-toolchain/blender-4.5.12-linux-x64/blender
"$BLENDER" -b -t 2 --python-exit-code 1 \
  -P research/private/character-pipeline/rig_goat.py -- \
  --model research/private/generated/goat-animation-20261106/mesh/model.glb \
  --output /absolute/new/goat-output
"$BLENDER" -b -t 2 --python-exit-code 1 \
  -P research/private/character-pipeline/verify_goat_rig.py -- \
  --model /absolute/new/goat-output/goat-rigged.glb \
  --output /absolute/new/goat-output/verification.json
"$BLENDER" -b -t 2 --python-exit-code 1 \
  -P research/private/character-pipeline/render_goat_treadmill.py -- \
  --model /absolute/new/goat-output/goat-rigged.glb \
  --output /absolute/new/goat-output/treadmill
```

An independent rebuild and GLB reimport passed: 17,070 vertices, 14 bones,
normalized weights, finite evaluated positions, zero sampled loop seam error,
and maximum sampled stance drift of `2.60e-7` model units relative to the floor.
Rigid-region sampled pair distances and bone lengths remain stable to floating
point precision. Coordinates are model units; the goat is roughly one unit tall.
These checks cover the authored flat-ground cycle, not terrain foot placement,
engine locomotion blending or every possible pose.

## Archer source and rigging

The original Day sprites show a bare torso, orange sash, pale loose trousers,
striped headwrap, brown shoes, back quiver and a **bow**. The low-resolution
weapon was initially misread as a crossbow; Oliver corrected that interpretation.
Do not use the rejected crossbow candidate. Owner-local source references are in
`../generated/archer-reference-20261106/`. Walk TMAP bases 8/9/10 have four phases;
11/12/13 and 121/122/123 are static standing and kneeling attack stances.
Modern draw/release transitions are authored additions. Source walk frame timing
depends on rendered TMAP demand and has not been established as a fixed FPS.

The body references use built-in image generation with those Day sheets.
Requests, receipts, images and meshes are owner-local under
`../generated/archer-modern-20261106/`. The first Tripo H3.1 body looked suitable
in static views (17,509 vertices / 28,928 triangles), but its low underarm bridge
and open hands required unsuccessful skin/curl repairs during animation.

The selected body is `meshy-grip/`: Meshy 7.1, 19,770 vertices / 25,307 triangles
and a 24-joint rig. Its input `archer-grip-reference.png` retains the costume and
face while providing a front T-pose, clear armpits and already curved grasping
hands. `grip-image-prompt.txt` preserves the exact built-in imagegen prompt.
This avoids the torn finger morphs in the earlier candidate. These are modeled
grips, not an articulated finger rig. The earlier Meshy open-hand T-pose is also
retained as a comparison. Both Meshy bodies deform more cleanly at the shoulders
than the Tripo candidate in the same unmodified Archery_Shot clip.

**Normalize and inspect the actual facing direction before auto-rigging.**
The first Meshy result was unusable: the +X-facing mesh received a skeleton
whose forward axis was -Y in Blender. Arm joints passed through the torso and
animated arm motion pulled the trousers upward. Ordered joint names, skin
weights and inverse-bind buffers were identical across the provider's static,
walking and archery exports; transferring their actions would not fix it.
`align_mesh.py --yaw-degrees -90` baked this particular mesh into the matching
facing direction. The second rig's overlay places arm and leg joints inside
their corresponding limbs. Keep both trials as evidence; the rotation is
specific to this observed input, not a universal rule for every generated mesh.

The selected generation uses `meshy/v7.1/image-to-3d`, with rigging and animation
enabled, action 224 (Archery_Shot), quad topology and the already posed input
(`pose_mode=""`). `meshy-grip-request.json` records the complete request. Basic
walk/run clips are returned too. The five-second supplied archery action is a
motion foundation, not a source-exact one-second shot. The earlier standalone
`fal-ai/meshy/rigging` trials also worked after facing correction, but were not
selected. Blender inspection exposes an Icosphere helper after rig import;
direct inspection of the provider and final GLB node lists confirms it is not
character geometry in those files.

The quiver needs rigid skinning while its strap follows the torso. The repair
shares its mask across coincident UV-seam vertices: a pair at the upper
quiver initially received Spine and RightShoulder weights respectively, creating
a long spike during aiming despite a satisfactory walking preview. Check both
poses after reimporting the exported GLB. `archer_props.py` shares the seam-aware
repair between walking and shooting.

The accepted walk is `animated/root-final-walk/walk.glb` under the archer output
directory, with editable `walk.blend`. This is root's independent rebuild of the
accepted `walk-meshy-grip6` source with the final bow. Its three-cycle video is
`animated/walk-final-review/treadmill-review.mp4`. `verify_archer_walk.py` reimports it and
checks 97 quarter-frame samples, finite geometry, normalized weights, actual
motion, the one-second exported interval, ground contact and three rigid-quiver
probes. Root independently measured a `1.55e-6` maximum vertex loop error and
`4.97e-7` quiver pair-distance variation. The supporting shoe is approximately
one millimetre above the review floor. This is sampled flat-ground evidence,
not a terrain-placement or continuous collision guarantee.

The preserved first walk fails the same checker on quiver rigidity (0.052 model
units of pair-distance variation) and export duration. The duration check also
caught the later corrected walk's NLA start adjustment trimming its interval;
moving both strip endpoints fixes the actual output instead of relaxing the gate.

The standing shot is `animated/final-standing3/standing.glb`, with editable
`standing.blend` and `motion-front.mp4` (three repetitions at 24 fps). Root
inspected held, full-draw, release and reset frames from front, profile and rear.
Independent `archer_contact.py` measurements after GLB reimport put the grip and
held nock within `1e-5` model units of their selected finger-region centroids; this is an
attachment check, not proof of finger anatomy. The string remains 1.16 model
units long at the six checked phases. The release separates string and drawing
hand deliberately. A STEP arrow-scale track prevents a partial arrow appearing
during the loop reset. Each shot clip spans one second; the prop deformation is
authored animation, not a simulated elastic bow.

The kneeling shot is `animated/grip-kneel1/kneel.glb` with editable `kneel.blend`.
`archer_kneel.py` uses fixed-length leg chains on this same rig to place the
front foot and rear knee at the floor. The relaxed rear toe remains above it.
Front, profile and rear aiming/release frames were inspected after GLB export;
root independently reran the hand/string and per-foot measurements. The original
sprites contain held attack stances; these loops do not reconstruct a full
historical draw, reload or stand-to-kneel sequence. No replacement character is
installed in Unity yet. Engine blending, terrain placement and crowd LODs remain
outside this asset-animation review.

`animated/archer-final-review.mp4` is the nine-second review: three walk cycles,
three standing shots and three kneeling shots, encoded from the inspected PNG
frames at 24 fps. Individual critical frames, numeric reports and rejected
candidates remain beside the selected assets. Python compilation and focused
UBS scans cover the twelve new archer source files. UBS's one JSON warning is a
fail-fast parse of the script's own freshly exported GLB before modifying its
arrow interpolation; malformed data raises rather than being ignored. No scan
establishes visual quality: that acceptance comes from the exported-model views.

Rebuild and check the walk from repository root with Blender 4.5.12 (set
`BLENDER` as in the goat example; use new output directories):

```sh
"$BLENDER" -b -t 2 --python-exit-code 1 \
  -P research/private/character-pipeline/archer_bow.py -- \
  --output /absolute/new/bow
"$BLENDER" -b -t 2 --python-exit-code 1 \
  -P research/private/character-pipeline/archer_walk.py -- \
  --model research/private/generated/archer-modern-20261106/meshy-grip/walking.glb \
  --bow /absolute/new/bow/bow.blend --output /absolute/new/walk
"$BLENDER" -b -t 2 --python-exit-code 1 \
  -P research/private/character-pipeline/verify_archer_walk.py -- \
  --model /absolute/new/walk/walk.glb --output /absolute/new/walk/check.json
"$BLENDER" -b -t 2 --python-exit-code 1 \
  -P research/private/character-pipeline/archer_preview.py -- \
  --model /absolute/new/walk/walk.glb --output /absolute/new/walk/review \
  --frames 1 7 13 19 25 --views 0 90 180 225
"$BLENDER" -b -t 2 --python-exit-code 1 \
  -P research/private/character-pipeline/archer_shot.py -- \
  --model research/private/generated/archer-modern-20261106/meshy-grip/animation.glb \
  --bow /absolute/new/bow/bow.blend --rigid-quiver --output /absolute/new/standing
# Add --kneel and use a separate output directory for the kneeling shot.
"$BLENDER" -b -t 2 --python-exit-code 1 \
  -P research/private/character-pipeline/archer_walk_preview.py -- \
  --model /absolute/new/walk/walk.glb --output /absolute/new/walk/treadmill
ffmpeg -n -framerate 24 -i /absolute/new/walk/treadmill/frame-%03d.png \
  -c:v libx264 -pix_fmt yuv420p /absolute/new/walk/treadmill.mp4
```

## Tools

- `run_actionmesh.py`: verify all pinned checkpoint hashes before loading the
  reconstruction model, and check them again before declaring completion.
  Accept 2–31 sampled PNGs, their sample rate, a new output
  directory, and either `--mesh` or `--native`. It preserves diagnostics on
  stderr, emits a versioned JSON result on stdout and records runtime/VRAM in
  `run.json`. Existing output directories are refused. A real damaged-checkpoint
  test exits 1 with a typed error before creating output or loading the model.
- `fal_job.py`: submit once and retain the queue receipt, resume polling or
  interrupted output downloads. Load `FAL_KEY` from the project `.env` without
  printing it. `--image-field start_image_url` supports Kling; the default
  `image_url` supports Wan and Tripo.
  Polling defaults to 30 minutes (`--timeout-seconds`). Timeout exits 75 with a
  JSON error and retains the receipt; rerunning resumes that job. Timeout does
  not itself cancel a provider job. HTTP operations also have network timeouts.
  Meshy rigging results also download the rigged GLB/FBX, optional animation,
  and nested basic walking/running GLBs. Each output resumes independently.
- `align_mesh.py`: bake an explicit yaw in world coordinates before motion
  fitting, independent of Blender's Euler/quaternion mode. It verifies every
  transformed vertex. **Do not assume image-view alignment is the model's
  learned canonical orientation.** The utility is mechanically verified; the
  camera-aligned trial with verified weights still failed the face/limb review.
- `export_deformation.py`: Blender conversion of ActionMesh's saved positions
  into linear morph targets on the original textured GLB. It checks the bind
  geometry before exporting and preserves UV seams and materials. Run with
  Blender 4.5.12, `--model`, `--vertices`, `--output`, and the actual sample `--fps`.
- `render_character.py`: independent Blender renders with fixed studio lights.
  Default: four angles at first/middle/last animation frames. `--animate --view 90`
  renders the full clip from a fixed side view. `--engine BLENDER_EEVEE_NEXT`
  enables the verified fast rendering path. Default Cycles provides a second
  rendering check.
  `--frames 1 16 31 46 61 76 91` checks seven phases from four directions.

Example (from repository root; substitute an actual animated model):

```sh
/home/oliver/Projects/fittimer-avatar-toolchain/blender-4.5.12-linux-x64/blender \
  -b -t 8 -P research/private/character-pipeline/render_character.py -- \
  --model /absolute/path/goat-walk.glb --output /absolute/path/review \
  --engine BLENDER_EEVEE_NEXT
```

## Local reconstruction setup

Third-party source: `/home/oliver/Projects/magiccarpet-character-tools/ActionMesh`
at `f1af2cd97f36fd05344f5e0ccbd8d0ddb94e60da`, with its pinned TripoSG submodule.
Weights and isolated Python package overlay live on the HDD at
`/mnt/pinokio-shared/magiccarpet-character-models/`.
The shared Comfy Python environment is read-only; no packages are installed into
it. The overlay uses its Python 3.12 / PyTorch 2.10.0+cu130.

The HDD is exFAT: use local-directory model downloads and copied packages.
uv's cache needs an ext4 location because it uses symlinks. The rig machine's
Tailscale SSH policy denied access; this experiment uses the authorized local
storage alternative. No RunPod instance has been requested.

Current overlay: diffusers 0.35.2, transformers 4.57.1, tokenizers 0.22.2,
huggingface-hub 0.36.0, numpy 1.26.4, scipy 1.15.3, scikit-image 0.25.2,
scikit-learn 1.6.1, hydra-core 1.3.2, omegaconf 2.3.0, antlr 4.9.3,
diso 0.1.4, plus inference utilities. PyTorch3D 0.7.9 was built from
`e4ad6e5687a360ccdc6ebd91348e8dbbac97bf31` with Clang for C++ and GCC 15
for the CUDA host compiler, CUDA 13.4,
`CUB_HOME=/usr/include/cccl`, `NVCC_FLAGS=-std=c++20` and architecture 8.6.
The C++20 override addresses the observed NVCC/host parsing failure in PyTorch's
dependent type expression. Clang avoids an observed GCC 15 internal compiler
crash in the Pulsar camera source. The shared PyTorch installation is not patched.

ActionMesh's FAIR Noncommercial Research License restricts its outputs too.
This experiment is a private method evaluation, not a commercial asset pipeline.
The skeletal goat above uses the approved Tripo mesh and Blender-authored motion;
it does not use ActionMesh output. AniGen and fixed-rig video fitting remain
possible future candidates, subject to real output tests.

## Reconstruct an approved motion reference

Before inference, run the existing checksum tool from the weights directory:

```sh
cd /mnt/pinokio-shared/magiccarpet-character-models/actionmesh-weights
sha256sum --check --strict /home/oliver/Projects/odcpw/magiccarpet/research/private/character-pipeline/actionmesh.sha256
```

Require exit 0 and all eight matches. The manifest pins publisher LFS SHA-256
values retrieved on 2026-11-06 from `facebook/ActionMesh`, `facebook/dinov2-large`,
`VAST-AI/TripoSG` and `briaai/RMBG-1.4`. It contains no model data. An existing
filename or a successful tensor load is insufficient: corrupted tensors can
remain finite and produce plausible but wrong geometry.

Extract 16–31 PNG frames spanning the desired motion at a known rate. The goat
experiment uses 31 samples at 8 Hz from the revised five-second Kling clip.
The upstream CLI silently ignores frames beyond 31; do not pass a full video
and assume it will sample the whole duration. Keep the first sample close to
the anchor mesh pose.

Set `PYTHONPATH` to the HDD Python overlay; the launcher adds the ActionMesh
checkout and its TripoSG submodule. Set `HF_HOME` and `TMPDIR` to their HDD
locations. Use the read-only Comfy environment's Python executable:

```sh
python research/private/character-pipeline/run_actionmesh.py \
  --repo /home/oliver/Projects/magiccarpet-character-tools/ActionMesh \
  --frames /absolute/path/frames --fps 8 --mesh /absolute/path/model.glb \
  --output /absolute/path/reconstruction --seed 20261106
```

The launcher selects the absolute upstream script and sets its working directory;
invoke the launcher from the project root. Use `--native` instead of `--mesh`
to generate the starting mesh from the video. It uses the normal upstream model
precision; a precision workaround does not repair corrupted weights.

Pass its `deformations_vertices.npy` and the **same anchor GLB** to
`export_deformation.py --fps 8`. The exporter checks the actual starting
geometry; it must not silently remap a different model or orientation.

## Initial reconstruction outcome

The fresh sprite → reference image → generated walking video → animated 3D
experiment produced actual GLBs, but **none passes final visual acceptance**.
It is implemented and self-verified as an experiment; no creature was installed
in Unity. Files below are relative to `../generated/goat-animation-20261106/`.

| Trial | Evidence | Visual result |
| --- | --- | --- |
| Fresh Tripo H3.1 mesh, original orientation; 31 poses | `goat-walk-verified.glb`, `goat-prototype-review.mp4`, `verified-review/` | Recognizable gait and stable body, but muzzle flattening and poor hoof contact |
| Same mesh, verified camera alignment; 16 poses | `goat-walk-verified-aligned.glb`, `verified-aligned-review/` | Face deformation persists; awkward foreleg crossing |
| Native video → mesh → motion; 16 poses | `goat-walk-native.glb`, `goat-native-review.mp4`, `native-review/` | Better head volume, but stretched/twisted limbs and weak hoof contact |

Blender independently rendered four angles at six or seven motion phases.
Important individual frames were inspected, not just a preview movie. The native
run passed all eight weight hashes **before and after inference**. It used
9.32 GiB peak allocated CUDA memory / 10.10 GiB reserved on the RTX 3090.
The textured clip contains 31 sampled poses at 8 Hz, linearly interpolated to
91 rendered frames at 24 fps. These are morph animations, not skeletal rigs.

The Tripo mesh has 17,070 vertices / 29,016 triangles; the native mesh has 19,946
vertices. Bind matching errors are `4.22e-8` and zero respectively. These checks
verify the exporter, not anatomical quality. The textured static model also has
a weak far-side eye and small neck/back ridges. No money was spent retexturing
the rejected native geometry.

The next candidates were AniGen's joint shape/skeleton/skinning generation and
video fitting, or a validated quadruped rig. The Blender follow-up above takes
the fixed-rig route after Oliver approved repairing this mesh. Require rigid skull
and hoof shape, plausible joint motion, foot contact, a clean loop and Unity
skeletal playback before processing the roster. Check the same boundaries on a
humanoid archer with bow/string/arrow contact. More VRAM is not the demonstrated
bottleneck in these trials; the local cache-integrity anomaly below must also
be understood before unattended bulk generation.

The first Kling in-place clip was rejected for isolated leg lifts; the revised
forward-walking clip supplied these trials. The original Wan request stalled
and cancellation was requested, but it eventually completed. Its existing
result was downloaded without resubmission. Ten sampled frames in
`wan-contact.png` show mostly standing and isolated foreleg lifts; it was not
selected as the walking reference. All provider receipts and results are retained.

## Integrity findings

**The initial animation trials used a damaged DINOv2 checkpoint.** Both local
copies hashed to `b10df4d6666d97b144388c77711c1088d13b0fc042a0127180cb52dc354b5a66`,
whereas the publisher's LFS SHA-256 is
`399fba97a95f22c36834418bc69373364a99af3a1153da1c0fb31db567c92e23`.
One weight in `encoder.layer.9.mlp.fc2.weight` had magnitude `1.038e18`.
Consequently, the initial motion and alignment trials are rejected outputs,
**not valid benchmarks of ActionMesh quality**. Preserve them for diagnosis.
Native half-precision generation also produced nonfinite latents; full precision
avoided overflow but still used the damaged encoder, so that run was stopped.
Validate checkpoint bytes against publisher hashes before another quality trial.

Both damaged encoder copies were preserved with `.corrupt-20261106` suffixes,
then replaced with a fresh download and verified after copying. The unused RMBG
`model.pth` also differed from its publisher hash and was preserved/replaced;
the active RMBG safetensors, both ActionMesh weights and the other TripoSG
weights matched. Direct disk reads of the preserved original DINOv2 and RMBG
files confirmed those mismatches. No third-party source precision patch is
required with the repaired weights.

Later, ordinary cached reads of the previously verified ActionMesh denoiser
returned `5da84053d903f2dbef53b28383bcb312914dd53bfb47c5d85c07a2a9d5217fd8` twice,
while `dd iflag=direct` returned the correct publisher hash. File-specific
`POSIX_FADV_DONTNEED` restored the correct ordinary-read hash. No system-wide
cache drop, file deletion or unrelated workload interruption was performed.
The cause is **unresolved**; this does not diagnose the disk, RAM or loader.
The two external-mesh trials had preflight checks only. The final native trial
has before/after checks, added in response to this observed defect. Preserve
the `*-hash*`, `*-integrity*` and `denoiser-cache-refresh.json` evidence.

Tool self-verification includes Python parsing/compilation, completed FAL receipt
resumption without resubmission, Blender bind/export/import checks and refusal of
the **actual preserved corrupt checkpoint** before output creation. UBS scanned
the five Python sources. Its checksum-comparison critical is a false positive:
these SHA-256 values are public download identifiers, not secrets. FAL URL warnings
are covered by explicit HTTPS/host checks; JSON parse failures reach the typed
top-level error handler. Informational style findings were not treated as defects.

Sources: [ActionMesh](https://github.com/facebookresearch/actionmesh),
[Tripo H3.1 API](https://fal.ai/models/tripo3d/h3.1/image-to-3d/api),
[Kling 4 API](https://fal.ai/models/kling/v4/image-to-video/api),
[AniGen](https://github.com/VAST-AI-Research/AniGen).
