# First three playable levels — execution list

Requested by Oliver, 11 September 2026. Work in this order; a concrete blocker
may justify advancing the next item. Preserve remc2 behavior in one Rust engine;
Unity presents state, packages define content. Focused checks, not repeated audits.
Unchecked means unfinished, including partially implemented items.

5 October resume: Oliver requested committing/pushing the recovered work and
continuing to real first-three-level play. Fresh checkpoint verification:
format/diff checks, 41 closure tests and eight Unity minimap tests pass. The
workspace stops in sim-capi with 163 passed, one failed and two ignored:
`cf14_level3_uses_generic_campaign_abi` still expects visible sprites in the
empty starting sea view despite the source range fix. Its test needs the same
empty-view/nearby-real-actor discrimination already proved by the Unity test.
Exporter checks initially lacked their configured binary; a clean pinned-source
build is in progress. Cache checks exposed the old capture-runtime signature
validator; it now recognizes the conditional v1/v2 encoder and still rejects a
changed legacy v1 branch. The cache suite is rerunning. UBS's staged Rust scan
is partial: existing monolith output exceeds its AST diagnostic limit. This is
a recovery checkpoint, not three-level acceptance. Next: connect qualified
Night presentation to the same Level 1 campaign session, then exercise actual
New Game and progression.

1. [ ] **Active: finish shared minimap.** Terrain layer is visible in Unity.
   Decode original player/castle/objective symbols; draw native entity/mark
   commands with source colors, rotation, anchors and clipping. Honor collected
   pickups, ownership changes, hidden/reclaimed entities and map settings.
2. [ ] Complete Level 1 Night terrain/world/HUD through the single semantic
   campaign runtime, reusing qualified assets without a second simulation.
3. [ ] Finish corrected Day package/cache consumers for Level 2 as well as 3.
4. [ ] Fix source-backed Castle casting, placement, mana and upgrades;
   Possession ownership/mana effects; acquired spell selection and pickup cleanup.
5. [ ] Complete projectile/effect visibility and lifecycles: Fireball,
   Possession, fireflies, explosions, damage and death. Resolve reported combat
   crashes and missing actors without level-specific patches.
   13 September: live Fireball/Possession selectors, matching ABI entity rows,
   and Fireball impacts are connected; the public-cast movement/reclaim test
   passes. Original spell texture dependencies are now collected from present
   spell holders and projectile-producing creature roots. Both new owner caches
   are built and admitted with Oliver's approval. Unity now verifies textured
   flyers, Fireball impacts and projectile reclaim; the desktop player is
   rebuilt and starts Level 2 using the new caches. Static sprite visibility is
   delivered; animation, exact anchoring and the rest of this item remain open.
6. [ ] Complete package-reachable creature and rival behavior for Levels 1–3:
   spawning, movement, attacks, damage, death, cleanup and ordered randomness.
7. [ ] Complete terrain/building events, monoliths, houses and castle destruction;
   retain working monolith behavior and verify actual production consumers.
8. [ ] Complete each level's objectives, beacons, exit visuals and activation;
   use pinned source/package evidence for the reported mirror/portal question.
9. [ ] Finish campaign progression: title/new game, loading presentation,
   Levels 1→2→3, completion, retry/death, pause, replay/save and reset isolation.
   13 September UI slice: separate presentation-only campaign menus now offer
   New Game, Resume, Save/Load, Restart, Main Menu, Quit and guarded Continue.
   They reuse the qualified indexed loading background; button artwork/layout
   is still Unity UI, not a faithful original-menu port. New Game and completion
   share the generation/destination-bound visible-frame loading gate. Three
   focused Unity checks pass (`/tmp/mc-campaign-menu-checked-unity.xml`), including
   real pause/save/load, same-handle New Game, double-click rejection and native
   incomplete/stale-completion rejection. Desktop rebuild succeeds. Launch with
   `sh research/private/remc2-unity-native/run-campaign.sh`; Escape/P pauses.
   Final load-failure ownership fix rechecked: one owning test passes in
   `/tmp/mc-campaign-menu-final-unity.xml`; final build succeeds in
   `/tmp/mc-campaign-menu-final-build.log`. Actual menu screenshot:
   `/tmp/mc-campaign-menu-desktop.png`. No native plugin or gameplay golden changed.
   This does not resolve item 2 or connect the legacy Level 1 viewer to Level 2.
10. [ ] Finish human-facing controls, readable text, HUD, spell selection, audio,
    speed/pacing and sustained-play responsiveness where defects remain.
11. [x] Resolve the six recorded Level 2 failing scenarios (12 September).
    Fixed shared cast-hand and Possession mana-charge writes being erased by
    movement synchronization. Campaign scenarios now use the semantic loader
    with byte-identical source inputs; stale minimap/constructor expectations
    were aligned with the existing source-backed behavior. All 23 Level 2
    campaign ABI tests pass, including the six recorded failures. This closes
    that regression batch, not the remaining gameplay/presentation milestones.
12. [ ] Remove concrete content-loading barriers: semantic package binding,
    realm/asset selection, shared behavior ownership and authoring API. No
    Level-N engines, raw source IDs in neutral APIs, cosmetic renaming or
    unsupported promise that any arbitrary asset automatically works.
13. [ ] Exercise the integrated first-three-level flow and capture actual Unity
    evidence. Claim completion only for working behavior; staged entry/render
    probes do not establish full playthrough fidelity.

Current evidence: corrected Day terrain and minimap terrain layer are visible
in the desktop Level 3 probe. Full gameplay parity is not established.

11 September minimap implementation checkpoint (item 1 remains open):
- Day owner-cache v3 decodes all 16 original player/castle map symbols. Unity
  validates the real assets and draws native selector, mask and anchor commands.
- Fixed objective outlines and source-linked creature-group objectives now feed
  the campaign map. Rust selects the nearest arrow target with source list-order
  tie-breaking; geometry and distance are shared with the retained mission code.
- Unity uses the existing source line renderer for the seven native arrow
  vertices. A separate presentation overlay prevents world sprites covering the
  map. Pulse timing remains Rust-owned.
- Focused real Level 3 package test passed through 66 ordinary ticks, as did
  the arrow ordering/pulse test and four Unity map tests. Desktop integration
  initially exposed the old transport gate rejecting arrow rows; corrected
  global ordering and explicit seven-vertex validation fixed the launch.
  Final player build and startup passed with plugin SHA256
  `68b80ab2bf2e33fbd8435851298e204f8d905991feeecc43facab3895371d11b`.
  Actual display: `/tmp/mc-map-arrow-final-desktop.png`. The screenshot is a
  display check, not a full gameplay or pulse-cadence proof.
- Remaining: map settings/transparency, other reachable objective kinds and
  pickup/ownership/cleanup checks through the integrated first-three-level flow.

Building-objective follow-through: the Day campaign marker producer now handles
the real Level 2 building-clearance objective. Its target query uses the same
validated semantic building chain and shared arena carriers as the objective
evaluator, including dynamic buildings that are absent from imported entity
rows. The owning lifecycle test checks target births, unrelated-objective
exclusion and removal after reclaim. C API compilation and the existing live
Level 3 arrow test pass. This change is now installed in the actual player.

Native v3 cache integration: Rust now reads the material component of the same
`level3-day-minimap-owner-cache-v3` bundle used by Unity. The focused owning test
`day_v3_symbol_cache_preserves_the_qualified_day_frame` passes and compares the
real rendered indices and palette with v2, without changing simulation state.
Native release and Unity startup pass; current installed/built-player plugin
SHA256 is `f099552d53834c65f64eba8abde1fc5218df01fc77bd252636a412db5a2e9611`.
Startup log: `/tmp/mc-day-v3-native-desktop.log`. The current player uses
`-mc-runtime-owner-cache ../generated/level3-day-minimap-owner-cache-v3/components/material`.
This removes the separate v2 native-cache requirement. Level 2's legacy cache
consumer still needs the corrected Day path; map settings remain unfinished.

Shared Day integration checkpoint (supersedes that Level 2 consumer status):
Level 2 now loads its corrected v3 material/HUD/symbol bundle through the same
`DayPresentationSurface` as Level 3. Its corrected owner-export hash includes
the actual `building-parameters.bin` member; historical recipe hashes retain
their original membership. Desktop replay exposed a second bug: the campaign
open path sent material components to the legacy gameplay-cache decoder.
It now validates material/profile/package binding separately without changing
the gameplay reset. Unity exceptions retain the native diagnostic message.

The exact core `corrected_day_recipe_replay_restores_its_exact_package` test
passes for both Day levels, restoring uncached gameplay with attached material
caches. All seven `CampaignMinimapPixelTests` pass, including the runtime replay
render and wrong-level-cache rejection. Format and diff checks, native release,
and desktop build pass. Installed/player plugin SHA256:
`04be5f1212c57ef8361a3e592797214108ae67724145a4233546fb1bd0bd2826`.
Actual Level 2 startup passes in `/tmp/mc-level2-shared-day-fixed-desktop.log`;
display capture: `/tmp/mc-level2-shared-day-fixed-desktop.png`.
This is a staged replay entry, not campaign playthrough proof. The screenshot
reveals creatures apparently floating above terrain: next inspect the shared
sprite/terrain projection. `DayPresentationSurface.PresentView` uses Unity Euler
pitch while `ClassicHdProjection` uses a linear source pitch offset; that is a
concrete mismatch to resolve, not yet a proven complete explanation. Items 1
and 3 remain open, and no full-parity claim is made.

Sprite/terrain defect follow-through: the Day camera now uses the source
map-axis convention, 320x200 projection scale, linear pitch offset and
pixel-space roll. Projection alone did not remove the floating actors. A native
snapshot located them tens of thousands of units away; the campaign draw path
had omitted the source sprite range/near-plane check. The C ABI world sprite
producer now uses the same extracted admission math as the existing indexed
sprite rasterizer (`GameRenderHD.cpp:3576`, range setup at line 685), leaving
the live selector census and simulation untouched.

The focused core admission test covers nearby inclusion, rear exclusion,
64-unit near-plane equality, 5120-unit cutoff equality and observational polls;
the existing fog/cutoff regression also passes. Eight Unity tests pass in
`/tmp/mc-day-sprite-admission-views.xml`. Two previously invalid assertions that
the empty starting sea view must contain sprites were replaced by explicit
empty-view checks plus a real nearby-creature rendering check; this admits
correct culling and still rejects a renderer that hides every actor.
The final built desktop starts successfully and no longer draws the remote
actors over the sky: `/tmp/mc-day-admission-desktop.png` and
`/tmp/mc-day-admission-desktop.log`. Native/player plugin SHA256:
`129c7cd5dde3261b6931f3dfb9eeb9c6e74049fbebeee9b604cb22b4654189fb`.
Native release, desktop build, format and diff checks pass. This does not prove
sprite fog, terrain occlusion, reflections or complete gameplay fidelity.
Resume item 1's source map settings/remaining reachable objective markers next;
the sprite projection/range fix is integrated, not a reason to restart review.

Normal exit arrow and live imported-entity projection checkpoint:
- `campaign_exit_objective_target` resolves the first live, semantic exit-sequence
  trigger after objective completion, including dynamic switch carriers and
  shadowing of stale imported slots. The minimap emits the source arrow only,
  not an invented portal rectangle. The real Level 2 building-clearance test
  proves no target before group-31 creation, the target at the subsequent
  ordinary group birth, observational polling, and removal on reset.
- `mc_native_campaign_entities_copy` incorrectly initialized its rows from
  immutable `inspection.entities()`. A new focused native test reproduced a
  public turn moving the player to `(123,456)` while presentation still returned
  `(384,62080)`. Switching that copy to `reset.entities()` makes movement and
  reset pass. This fixes imported live rows; it does not claim every dynamic
  family is projected.
- Core exit lifecycle, C API arrow pulse/order, and C API live-player/reset tests
  pass. Eight Unity checks pass in `/tmp/mc-exit-live-projection.xml`; format,
  diff and native release checks pass. Updated editor/built-player plugin SHA256:
  `4734795c9d244e12f3f0f51fa910d33b380dd3d373e01576efc45036cb48b4f9`.
  C# is unchanged, so the existing desktop build received that native plugin.
- Next concrete presentation gap: `mc_native_campaign_entities_copy` overlays
  only dynamic Bees and authored class-5 rows; the selector producer and minimap
  also enumerate imported `session.entities()`. Trace the existing canonical
  source-arena carriers into these consumers for package-reachable spawned
  buildings, switches and spells. Avoid adding client-side entity authority.
  Normal-exit full playthrough, alternate end-state markers, map settings and
  complete dynamic presentation remain unverified/unfinished.

Spawned-creature selector checkpoint:
`campaign_entity_presentations` now merges live authored class-5 and Bee
carriers with imported rows, excludes imported slots occupied by dynamic
entities, preserves source-slot ordering, and consumes current particle, yaw,
position and hide flags (including Firefly/Dragon sidecars). These families
already have C API entity snapshots; the missing selector half is now connected.
The focused real-package group-20 constructor/ordinary-movement test passes and
checks observational polling, unique rows, same-pass hiding before reclaim,
and reset. It does not claim the preceding objective chain was played through.

The formerly failing `level_two_group20_materializes_model1_and_archer_rows_from_row214`
scenario now passes (`/tmp/mc-group20-through-movement-reset.log`). It uses the
semantic campaign loader with byte-identical source inputs and checks live
same-pass child callbacks instead of stale constructor-only state. The prior
objective completion is staged; this is not a full objective-chain playthrough.

Focused test: `native_world::campaign::tests::authored_creature_sprite_selectors_follow_live_birth_and_movement`
passes in `/tmp/mc-dynamic-creature-hide-fixed-test.log`. Eight Unity checks pass
in `/tmp/mc-dynamic-creature-unity.xml`; format/diff checks and native release
pass. Editor and built-player plugin updated to SHA256
`1906cb41f008dbc08f6b9348cdafff54f525c8650b05d9a3fab71543aac00999`.
The prior diagnostic player was stopped before replacing its plugin; no new
spawned-creature desktop playthrough is claimed. Dynamic map inputs, other
spawned families, and multipart presentation remain unfinished.

GOLDEN-CHANGE (operator-approved minimap contract expansion): material/aggregate
v3 adds original symbol assets; campaign mark kind 3 adds seven ordered arrow
vertices without changing the ABI layout. Existing golden artifacts are not
regenerated. Tests reject incomplete/stale vertex groups and confirm that native
visibility suppresses the arrow. Historical v1/v2 caches retain their identity.

12 September spell-authority delivery (self-verified, `9cdd82e` plus retained
working-tree changes): activation now commits mana/control flags to the numeric
player before movement copies it; Possession uses the existing shared mana-charge
commit, preserving its debit until the next player upkeep. No level patch or
golden regeneration. An unsuccessful test-only terrain-origin experiment was
removed once the stale carrier was identified.

Verification from `research/private/remc2-sim-rs`:
- `cargo test -p sim-core --lib --locked native_world::campaign::tests::selected_cast_survives_player_movement_authority_sync -- --exact`: 1 passed.
- `cargo test -p sim-capi --lib --locked native_v2::tests::level2_campaign -- --nocapture`: 23 passed (`/tmp/mc-level2-campaign-final-regression.log`).
- `cargo fmt --all -- --check`, `git diff --check`, and `cargo build -p sim-capi --release --locked`: passed; existing compiler warnings remain.
- Unity `NativeCampaignSessionEditModeTests.Level2RuntimeHandCastMarshalsToRust`: 1 passed (`/tmp/mc-spell-authority-unity.xml`).

Current editor and desktop-player plugin SHA256:
`2351d1f715f5ceb95984ec41614a468cbe85f501a408e344ec65aa23bd216380`.
No full playthrough or remaining projectile-visibility claim; next delivery is
the live projectile/effect presentation path, not another regression audit.

13 September active candidate: native plugin
`1ee0108e5fdc6ac7ad54e06e91b7d46739710ebddaf3f89a8a7ceb40f2f07617`
is installed in editor and desktop player. `cargo test -p sim-capi --lib --locked native_v2::tests::campaign_spell_sprites_follow_public_cast_movement_impact_and_reclaim -- --exact`
passes (1 test, `/tmp/mc-spell-sprites-lifecycle.log`); release, format, diff,
and the prescribed sim-capi correctness/suspicious Clippy gate pass. No gameplay
or replay golden changed. After the approved asset receipt/count update, all
41 closure tests pass (`/tmp/mc-spell-closure-approved-suite.log`).

New ignored caches preserve the earlier caches:
`level2-day-spell-owner-cache-v3`, manifest SHA256
`698567ad73184b44db05c85ce72238b951f294590b591168d179c2758da7c91b`;
`level3-day-spell-owner-cache-v3`, manifest SHA256
`0d19967b7964f0f97705ae4b69fd160de7586605f0d15e795197bc30ea1b4be6`.
The Level 3 builder uses `level3-day-block32-owner-v2`, not the older export;
its package is `level3-day-block32-campaign-recipe-v2`. Closure outputs are
`level{2,3}-day-spell-closure-v3.json`. The original TMAP 463 and 144 images
contain real nonzero pixels; no substitutes were generated.

`OwnerAssetCache.OpenAndValidateDayV3` now admits both exact new receipts and
their exact texture counts, retaining the old receipts and all child-file/hash
checks. GOLDEN-CHANGE approved by Oliver: new TMAP 463 payloads and constructor
provenance; comparison confirms no existing asset payload changed or disappeared.
The frozen Level 3 closure receipt/count is updated, not gameplay/replay goldens.

`DaySpellSpritesUseLiveNativeSlotsAndDisappearAfterReclaim`: 1 passed, zero
skipped (`/tmp/mc-spell-sprites-follow-unity.xml`). It also accepts the historical
caches and rejects cross-level new caches. Its camera follows the projectile:
the initial stationary-camera test correctly lost the impact beyond the source
draw range. No engine visibility gate was relaxed to make that assertion pass.
Temporary trace instrumentation was removed after establishing this cause.

Unity CLI built the existing configured scene successfully without rewriting it
(`unity --json --non-interactive build <Unity project> --target StandaloneLinux64
--output-path <Unity project>/Builds/Linux/MagicCarpetNative.x86_64 --log-file
/tmp/mc-spell-sprites-approved-build.log --no-tail --allow-dirty-build`). Native
plugin identity remains `1ee0108e5fdc6ac7ad54e06e91b7d46739710ebddaf3f89a8a7ceb40f2f07617`.
Desktop startup and Level 2 presentation succeed with both new Day caches
(`/tmp/mc-spell-sprites-approved-desktop.log`); the exact launch command is in
the Unity README. This is staged Level 2 entry, not a full playthrough.
Animated texture playback and exact sprite anchoring remain unfinished.
