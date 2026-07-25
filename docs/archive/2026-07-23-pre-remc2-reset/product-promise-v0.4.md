# Product promise and scope box

> Historical decision: superseded on 2026-07-23 by
> `docs/program-charter.md` and `docs/product-promise.md`. This document
> records the former single-product, distributable-25-realm-MC2 plan. It is
> not an active implementation contract.

Version: 0.4  
Status: MC2-only full-game and modular-engine baseline  
Tracking bead: `mc-w58.1`  
Depends on: provenance policy v0.4

## One-sentence promise

Rebuild Magic Carpet 2 as a deeply faithful, beautiful, moddable Unity game
that runs in desktop Chrome and native builds: first one instant 20-minute
friend island, ultimately the complete 25-realm campaign, spell/creature
catalog, strategy loop, saves, HUD/maps, audio, and multiplayer behavior.

## Player fantasy

The player should feel:

- intoxicating speed and freedom while skimming hills, surf, forests,
  fortresses, and volcanic terrain;
- mastery of a carpet with momentum, bank, altitude, and expressive risk—not a
  floating FPS camera;
- a world that visibly and audibly responds to flight, spells, weather,
  creatures, fire, water, mana, and destruction;
- strategic tension because mana exists in the world, must be claimed and
  transported, and can be stolen or lost;
- escalation from exploration through fortification and contest to spectacular
  sorcerous warfare;
- a readable conflict in which the minimap, objectives, strongholds, carriers,
  creature behavior, sky, sound, and terrain all communicate state;
- a complete arc worth replaying with friends, whether they cooperate, compete,
  or fill different factions beside AI.

The first demo succeeds only if a new player understands the loop, wants one
immediate rematch, and can send the link to a friend without setup support.

## Product identity

### Private MC2 reference project

The private lab studies the exact owned GOG Magic Carpet 2 build closely,
preserves a faithful behavioral baseline, and may use original labels and
owned data locally for comparison. MC1 is no longer a research, parity, or
implementation input. The private lab is not distributed.

### Shareable MC2 reimplementation

The shareable product is an independently implemented, mechanically faithful
MC2 game module running on a reusable carpet-flight engine. It does not ship
owned binaries or copied code, art, audio, maps, prose, UI expression, fonts,
or other original payloads. Redistributable builds use independently created
or properly licensed expression unless a later rights decision explicitly
allows something else. It does not imply EA, Bullfrog, GOG, or Disney
affiliation.

The public title, use of “Magic Carpet,” original spell/creature names,
Bullfrog/EA/GOG marks, and compatibility claims remain release-review
questions. Private validation may use exact names. A friend/browser build is a
distribution candidate even when noncommercial.

## Full-game and mod architecture

The product is not a one-level homage. The first island is the integration and
friend-play milestone for a full MC2 reimplementation:

- all 25 realm slots and their campaign ordering, unlock, tier-experience,
  retry/checkpoint, secret, ending, and save semantics;
- the complete evidenced MC2 spell-family and three-tier catalog;
- the complete evidenced creature, rival, settlement, stronghold/carrier,
  terrain, objective, HUD/map, cue, and multiplayer capability ledgers;
- realm-by-realm learning, pressure, topology, encounter, and objective cards;
- exact baseline behavior where it is understood, with every intentional
  modernization recorded in the deviation ledger; and
- Chrome, native client, and Linux authoritative-server conformance from one
  deterministic simulation contract.

The software is divided into:

1. **Carpet engine** — deterministic simulation primitives, Unity adapters,
   presentation, networking, persistence, tooling, browser lifecycle, and
   module APIs with no MC2 catalog hard-coded into the engine.
2. **Game-module SDK** — versioned manifests, declarative schemas, bounded
   behavior graphs, content-addressed assets, dependency/capability
   negotiation, validation, save/replay migration, and editor/CLI tools.
3. **MC2 module** — the first complete implementation of those APIs:
   campaign, rules, catalog, AI profiles, objectives, maps/content references,
   UI information model, cue graph, saves, and multiplayer presets.

MC2 is one game implementation, not a privileged engine mode. A total
conversion or future carpet-flight game must be able to replace the MC2 module
without editing engine assemblies. Smaller mods may instead provide declared
campaign/world packs, rules profiles, presentation packs, localization packs,
or bounded behavior graphs. Each package kind has explicit capabilities,
dependencies, budgets, provenance, and migration rules; none may reach around
the module API to depend on MC2 internals.

Browser-delivered mods are declarative and validated; arbitrary downloaded C#
or native code is not a baseline mod mechanism. The authoritative server
chooses the exact module set and hashes. Clients must possess or acquire the
same approved set before joining, and saves/replays bind to module IDs,
versions, schemas, and content hashes.

A separately gated owned-data compatibility adapter may let a lawful owner use
their local MC2 files in a private build. It is optional, never required for
the redistributable game, never bundles original data, and needs exact-format,
security, browser-storage, contract, and release review. Until that gate
passes, the MC2 module uses replacement expression.

## Experience differentiation matrix

| Experiential law to preserve | Original expression not copied into a shareable build | New expression to design |
|---|---|---|
| Absolute-displacement, inertial carpet steering with speed, bank, skim, and altitude play | Original code, constants copied in bulk, cursor/cockpit art, exact camera presentation | Independent flight equations validated against behavior; faithful and comfort input adapters |
| Finite world mana moving among creatures, loose spheres, ownership, carriers, buildings, spells, and strongholds | Extracted tables, original mana graphics/sounds, save data | Conserved deterministic mana ledger with original visual language |
| Possess exposed mana, transport it, secure it, spend it, and risk it | Original icons, text, effects, animation, tables | Original claim effects, carrier designs, feedback, and readable state encoding |
| Stronghold as bank, checkpoint, refill station, weapon, territory, and liability | Castle models, stages, textures, layouts, turret art | Original fortress architecture with equivalent strategic functions |
| Creatures defined by silhouette, domain, movement, attack rhythm, mana role, and counterplay | Original models, names as a wholesale roster, animations, sounds, exact stat tables | Original creature cast implementing every evidenced functional role |
| Rival sorcerers share the world loop but express distinct strategies | Original characters, names, dialogue, portraits, scripts | Original rivals built from one controller plus strategy profiles |
| Terrain and environmental magic change navigation, cover, economy, spectacle, and objectives | Original level files, geometry, palettes, textures, scripts | Original islands, deterministic heightfield operations, bounded destruction, hazards |
| MC2 objective chains teach one concept at a time and later combine them across 25 realms | Original mission geometry, objective prose, story presentation, and level payloads | Independently authored realm content implementing the evidenced campaign rules and learning/pressure structure; optional owned-data adapter remains separately gated |
| Spells grow through demonstrated use, making early tools more expressive rather than merely obsolete | Extracted experience tables, exact thresholds, original upgrade effects and text | One independently tuned Fireball upgrade in the demo; the complete evidenced three-tier catalog in the full MC2 module |
| Radar/live map/objective recall make extreme speed strategically readable | Original UI layout/art/icons/fonts/text | Original readable HUD, minimap, tactical map, objectives, accessibility |
| Day, night, caves, weather, music, ambience, and threat sounds define tactical mood | Original music, samples, voices, sound design, palette tables | Original adaptive score, spatial ambience, weather and biome soundscape |
| Large spells should feel extravagant while state remains legible | Original VFX, sprites, timing tables copied in bulk | Original browser-budgeted VFX and audio priority system |

## First playable demo

### Audience and use

- One-click private browser link sent to friends.
- Desktop/laptop players comfortable with action games; no prior Magic Carpet
  knowledge required.
- Solo uses the same default Expedition rules against one AI rival; no separate
  single-player simulation is required.
- The default invitation supports 1–4 humans.
- Controller support is desirable, but keyboard and mouse are the release
  baseline.

### Platform

- Tier 1 demo client: current stable 64-bit desktop Chrome on supported Windows
  and Linux, WebGL2, Unity 6 LTS plus URP candidate. `mc-gzn` pins the exact
  editor revision and package matrix before installation or project creation.
- Server: authoritative native Linux headless build over secure WebSocket.
- Development/verification: Windows/Linux Chrome and Linux native clients
  against the same Linux server.
- Compatibility watch, not a demo gate: Edge, Firefox, Safari, macOS, and other
  Chromium browsers. macOS/Safari become targets only when real Apple hardware
  or CI exists.
- Tier 1 native packages after the browser demo: Windows and Linux x86-64,
  including Steam Deck-class controller-first operation, from the same client
  and simulation contracts rather than a separate implementation.
- Mobile, console, VR, and WebGPU-exclusive features are out of scope.

### Session

- Nominal match: 20 minutes.
- Acceptable first-test range: 15–25 minutes.
- Infrastructure safety cutoff: 30 minutes including a short, explicit
  overtime window. Reaching it produces a visible timeout result; it does not
  silently award the faithful ruleset's victory condition.
- Join-to-flight target: under 60 seconds for a returning player on a normal
  connection; no account required.
- Results, rematch, or new room are available immediately.

The default rules have no countdown victory condition. The 30-minute cutoff is
an operational guardrail for the shareable demo, while 20 minutes remains the
design target. Map distance, mana travel, objective dwell, stronghold growth,
death, and conflict are instrumented before tuning. A faster public preset may
differ from the faithful baseline only through a versioned rules profile.

### Players, factions, and modes

- Default invitation: **Cooperative Expedition**. All humans share one fixed
  faction against neutral ecology and one AI rival faction on one
  objective-bearing island.
- Server cap: four active humans for the first demo.
- The simulation supports four fixed factions, but AI fills only the factions
  required by the selected preset; an empty slot is not automatically another
  rival.
- Human players may share a faction or occupy separate factions.
- Solo, cooperative expedition, free-for-all, and two-team play are presets
  created by pre-match faction assignment over one finite-mana economy and one
  authoritative simulation contract, not four independently implemented modes.
- The playable-demo proof set is exactly: two humans cooperating against AI,
  two humans adversarially, and one mixed human/AI faction run. Other legal
  1–4-human compositions need not be exhaustively balanced or certified.
- PvP and co-op therefore share implementation; they do not share every
  balance or victory preset.
- Mutable mid-match diplomacy/Pacts are deferred until their explicit decision
  bead. No hidden alliance system enters the demo.
- Voice chat is out of scope. A room code and clear connection/status UI are
  sufficient.

### World envelope

- Canonical demo domain: one bounded 4,096 m × 4,096 m square.
- Authored high-value playable area: approximately 3,000–3,500 m across,
  surrounded by readable ocean/horizon and hard simulation bounds.
- Canonical landscape: deterministic UInt16 heightfield, 1,025 × 1,025
  samples, approximately 4 m spacing.
- Provisional terrain range: 64 m below water to 768 m above water.
- Provisional flight ceiling: 1,024 m above water.
- One sea level, islands, shoreline, meadows, forest patches, mountains,
  caldera/volcano, settlements/strongholds, and authored points of interest.
- Local cell/fracture structures may be proved behind a bounded contract.
- Arbitrary voxel terrain, tunnels, overhangs, procedural caves, seamless
  worlds, and spherical planets are deferred.

The 4 km domain is a technical cap, not a requirement to fill every metre.
Flight speed and objective placement must make traversal meaningful without
creating dead travel. The map can shrink inside this envelope after instrumented
tests.

### Minimum functional journey

1. Open link, activate audio, choose a room/preset, load, and gain safe flight
   control.
2. Learn speed, bank, climb/dive, strafe, braking, minimap, and objective
   recall.
3. Use Fireball to release mana from a neutral target.
4. Use Possession to claim loose mana.
5. Place a first-tier stronghold.
6. Observe a carrier reserve, collect, survive, and bank possessed mana.
7. Choose between creature/ecology opportunity, resource route, and rival
   pressure.
8. Earn and recognize one Fireball upgrade through demonstrated use.
9. Use one bounded Crater spell to reshape a route or deny an area.
10. Survive weather/volcanic escalation and a contested objective.
11. Defeat/banish the required rival or complete the cooperative objective
   chain.
12. Reach an explicit ending, results, and rematch.

### Demo feature floor

- Faithful/modern-adapter carpet flight and comfort options.
- Fireball with one visible gameplay upgrade, Possession, one bounded Crater
  terrain operation, and stronghold placement/growth.
- Conserved exposed/owned/carried/secured/spent mana.
- One carrier family with loss and replacement.
- Health, damage, death, stronghold resurrection, and no-stronghold recovery.
- One rival wizard using the same authoritative rules/interfaces.
- Four clearly different non-wizard roles:
  aerial swarm, ground artillery, mana predator, and caldera area controller.
- Mountains, ocean, shoreline, meadows, trees, settlement/fortress forms,
  volcano/lava hazard, wind, storm, clouds, and day/dusk lighting.
- Sequential objectives, failure, exit/results, checkpoint/reconnect, and
  rematch.
- HUD, minimap, live tactical map, objective recall, spell selection, settings,
  captions/subtitles, reduced roll, and remapping.
- Spatial fire/water/wind/surf/forest/creature/weather soundscape and an
  original adaptive musical sketch.
- Authoritative 1–4-player session, room code, AI fill, reconnection, late
  join policy, and basic abuse/rate limits.
- Chrome/native/server reproducible builds, replay/hash evidence, performance
  captures, provenance manifest, and failure artifacts.

### Demo-only non-goals

- Full three-tier spell catalog during this milestone.
- Full creature/rival roster or 25-realm campaign during this milestone.
- Original maps, assets, text, names, story, music, UI, or level geometry.
- Cave levels, arbitrary volumetric terrain, or full-world voxels.
- Functional settlement ownership, growth, economy, or defenders; a readable
  neutral settlement landmark is sufficient for this demo.
- Persistent metagame, accounts, matchmaking, ranked play, store, cosmetics,
  monetization, or arbitrary-code user mods. The module boundary and a
  synthetic declarative mod must already be proved.
- Eight-player parity, mobile, console, VR, WebGPU-only rendering, or
  peer-to-peer authority.
- Production-scale live operations, localization breadth, or final branding.
- Perfect visual quality everywhere; proxy/licensed assets are acceptable
  where a pipeline or style decision is still being tested.

## Provisional hardware and service floor

These are preproduction targets and become measured gates in `mc-w58.13`.

### Browser client minimum target

- Reference floor to acquire or obtain equivalent hosted access to before the
  playable-demo gate: Intel NUC 11 Performance `NUC11PAHi5`, Core i5-1135G7,
  Iris Xe 80-EU integrated graphics, 8 GiB dual-channel RAM.
- Validate current stable 64-bit Chrome and WebGL2 on both supported Windows
  and Ubuntu LTS installations; exact OS and Chrome build IDs are pinned in the
  portability gate rather than silently drifting.
- 1,280 × 720 low preset at a 30 fps cap: at least 99% of in-match rendered
  frames complete within 50 ms, excluding declared load/transition windows.
- Network reference: 25 Mbit/s down, 5 Mbit/s up, no more than 80 ms round-trip
  time to the server, and no more than 1% steady packet loss.
- Compressed first-visit bootstrap target: at most 250 MiB and under 120
  seconds to flight on the reference network. Cached return target remains
  under 60 seconds.

### Quality target

- Oliver’s T14 class: Core Ultra 7 258V, Intel Arc 130V/140V, 32 GiB RAM.
- 1080p quality preset at 60 fps during ordinary flight and combat.
- No browser-tab crash, runaway heap, accumulating pools, or repeated
  long-frame failure during a complete match plus rematch.

### Server target

- Native Linux, no GPU or presentation assemblies.
- One four-human match plus AI and creatures on a provisional 2 vCPU / 2 GiB
  instance.
- Fixed authoritative tick, bounded allocations, recoverable match snapshots,
  structured logs, and deterministic failure bundles.

Unsupported hardware gets an explicit minimum-requirements result rather than
an unexplained hang or tab crash.

## Milestone ladder and non-goals

### Technical proof

Proves Chrome/native/server builds, deterministic simulation, one predicted
carpet, one projectile, heightfield query/edit, save/replay, late join/repair,
and browser budgets with disposable visuals.

Non-goals: fun, visual identity, full loop, multiple enemies, campaign, public
distribution, and feature-complete claims.

### Playable demo

Proves the complete first-island journey above with proxy-to-good original
presentation and the named cooperative, adversarial, and mixed human/AI proof
compositions.

Non-goals: near-final art everywhere, caves, full catalog, long campaign,
ranked balance, and production live service.

### Production-quality vertical slice

Proves one near-shippable original mission, final-quality pipelines, broader
ecology/spell interaction, strong narrative/environmental identity, polished
UX/audio/accessibility, measured content throughput, and stable public-style
operations.

Non-goals: complete campaign, every capability family, final content volume,
and 1.0 stability/localization breadth.

### Full MC2-module alpha

Implements every evidenced row in the frozen MC2 capability and 25-realm
manifest: flight, economy, strongholds/carriers, every spell tier,
creatures/rivals, settlements, terrain/environment, objectives/campaign,
multiplayer, saves, HUD/maps/menus, audio/cues, accessibility, tooling, and
module compatibility. Unknowns and waivers are explicit and cannot be hidden
by the word “selected.” Cave realms must preserve their functional navigation,
combat, visibility, and objective laws even if the replacement rendering uses
a bounded separately loaded representation instead of deformable volumetric
geology.

Non-goals: copied original content, unbounded new systems, release polish,
final balance, and unsupported platforms.

### 1.0

Ships the complete independently implemented MC2 module, its 25-realm
replacement-expression campaign, replayable MC2-faithful multiplayer profiles,
and the supported declarative mod workflow with accepted performance,
accessibility, localization, security, operations, support, ratings, privacy,
branding, licensing, and release quality.

Non-goals: arbitrary remote code plugins, promising every dream feature, MMO
persistence, console/mobile/VR parity, or live-service monetization.

### Dream version

May explore deeper dynamic ecology, seasons, richer astronomical/weather
systems, additional game modules and campaigns, procedural or deformable
volumetric caves, advanced destruction, richer creation tools, larger
sessions, native/console/VR targets, and curated community content.

The streamed destructible-city concept is recorded separately in
[`future-destructible-cities.md`](future-destructible-cities.md). It remains a
post-MC2 exploration and does not add city streaming, structural simulation or
granular rubble to the demo or 1.0 commitments.

The native OpenXR direction is recorded in
[`future-native-vr.md`](future-native-vr.md). It reuses the same authoritative
simulation and game modules as a future presentation/input target; browser VR
is not promised.

Dream work never blocks, silently enters, or redefines the approved demo or
1.0 scope.

## Scope-change rule

A proposal changes the scope box when it alters target platforms, authoritative
player count, world representation/bounds, session envelope, required demo
journey, public/private boundary, minimum hardware, or a milestone’s feature
floor. Such a change needs a bead, measured reason, affected dependency list,
cut/replace decision, and Oliver’s approval.
