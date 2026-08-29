# Agent-addressable game architecture

Status: **binding operator direction, accepted 2026-08-27**

Authority: [`program-charter.md`](program-charter.md) and
[`modular-engine-game-module-contract.md`](modular-engine-game-module-contract.md)

Consumer: every agent designing, implementing, reviewing or testing Carpet
Flight definitions, packages, engine capabilities, clients or the first-three-
level runtime route.
Retirement: only when a newer binding architecture contract subsumes these
rules and every binding/post-compaction pointer is migrated in the same change.

## The product boundary

Carpet Flight is one Rust-authoritative engine driven by immutable, validated
game definitions. Humans and agents author games through a typed semantic
surface. They do not edit raw remc2 selectors, source slots, binary offsets,
live runtime state, Unity gameplay fields or arbitrary JSON.

The private `MC2-owned` package is the first complete game definition. Its
source-shaped records are translated once by the MC2 compatibility adapter.
Future original packages use the same semantic contract without remc2 IDs.

```text
asset sources + package data
        -> validated artifact graph
        -> immutable GameDefinition
             -> LevelDefinitions + RuntimeFlowDefinition
             -> semantic catalogs
             -> compiled-capability bindings and typed configuration
        -> one Rust GameSession / source arena / scheduler
        -> semantic snapshots, events, audio and presentation cues
        -> Unity player/editor and optional inspectors

human or agent
        -> describe/get/set(stage)/validate/diff/apply
        -> guarded definition publication
        -> explicit session reset/reopen onto the new revision
```

## Definition composition

`GameDefinition` is the immutable package composition. It owns:

- game identity, rules profiles and campaign/runtime flow;
- levels and worlds;
- semantic catalogs for creatures, players, spells, items, buildings,
  scenery, terrain/materials, objectives, mechanics, audio, effects and UI
  presentation;
- semantic asset slots and their validated artifact bindings;
- compiled behavior-capability requirements and typed configuration;
- save/replay compatibility, provenance and package graph identity.

`LevelDefinition` composes one playable level from:

- a world/terrain definition;
- semantic entity placements and spawn instructions with stable instance IDs,
  transforms, owner/allegiance and only explicitly permitted overrides;
- objectives, activation/dependency graph and level-local mechanics;
- presentation, music, ambience and loading/transition bindings;
- references to catalog entries and compiled capabilities.

`RuntimeFlowDefinition` composes the in-between game experience: startup/title,
menu and campaign selection, new/load game, loading presentation, level entry,
pause/resume, save/load, level completion/failure, transition to the next level,
results and return to menu. Rust owns the authoritative flow state and save
identity; Unity owns presentation and input adaptation.

Its package-authored campaign catalog names semantic game/level IDs, ordered
entries and next-level edges, availability/completion policy, display metadata
and title/menu/loading/result presentation and audio slots. A Rust `GameRuntime`
above `GameSession` owns guarded operations such as `new_game`, `open_level`,
`pause`, `paused_frame`, `resume`, `save`, `load`, `accept_transition` and
`close`. Receipts bind the package graph, definition revision, runtime/session
generation, tick and semantic level ID. Unity never chooses a raw next-package
path or manufactures completion.

Leaf kinds such as `CreatureKind`, `SpellKind`, `BuildingKind` and
`ObjectiveCondition` are vocabulary inside this composition, not alternative
engines or standalone completion claims.

## Addressable semantic objects

Every supported authorable object has a stable semantic ID and a typed
descriptor containing:

```text
kind, value type, unit, default/effective value, bounds or enum,
access, mutation timing, dependencies, side effects, provenance,
determinism role, capability requirements and concise documentation
```

Examples include a creature's health, collision bounds, movement profile,
perception and behavior binding; a spell's mana cost, effects and sound slots;
a building's construction rules; an objective's condition; an entity
placement's coordinates; and a loading screen's presentation asset.

Not every runtime field is authorable. A field is writable only when its
descriptor explicitly grants access. Internal phase, RNG, allocator, mailbox,
generation, source-selector and replay fields remain engine-owned.

## Assets and presentation

Definitions refer to typed semantic asset slots, never filenames scattered
through behavior code. A slot declares its asset kind and constraints: format,
dimensions, scale, animation or frame contract, audio contract, performance
limits, transform recipe, provenance and runtime binding.

An agent may replace a creature model, spell sound, effect, texture, music cue
or loading image only by supplying an artifact which validates for that slot.
The content-addressed artifact graph and definition diff make the replacement
explicit. Polling or replacing presentation never grants gameplay authority.

## Behavior and AI

Packages select and configure compiled Rust capabilities. A behavior binding
contains a stable capability ID/version and only that capability's typed,
bounded parameters. An authoring agent may choose an available capability or
change supported parameters. It cannot upload executable code, invent an
unknown action, or smuggle raw source selectors into the definition.

A genuinely new algorithm is engine work: implement one compiled capability,
prove its deterministic contract and downstream lifecycle, then expose a typed
binding. Package data never substitutes for that implementation.

For scheduled behavior, the complete identity is:

```text
producer/constructor -> numeric visit -> handler -> same-pass epilogue/hide
-> following-pass cleanup/reclaim -> downstream consumer
```

One callback, parser arm, registry entry, refusal or helper test is not a
delivered behavior.

## Strict authoring transaction

The authoring API exposes versioned semantic operations:

```text
capabilities  describe  get  set(stage)  validate  diff
apply  apply-status  rollback  reset/reopen  snapshot
```

The author-facing change vocabulary is closed and typed. It includes bounded
operations such as `SetParameter`, `PlaceEntity`, `MoveEntity`, `RemoveEntity`,
`PutObjective`, `BindBehavior` and `BindAsset`. Requests carry an expected
definition revision/hash, idempotency key and requested mutation timing. Every
request type denies unknown fields. Raw `class`, `model`, `action`, `selector`,
`source_slot`, `offset`, `bytes`, filename and arbitrary-hash fields are not
members of this vocabulary.

Entity placements use stable author-chosen instance IDs rather than ordinal-
derived identities. Asset binding references an already validated retained
artifact ID; artifact ingestion is a separate provenance-bearing operation.
Behavior binding references an available exact capability version and its
closed typed configuration.

`set` accepts typed semantic paths and stages a new immutable candidate. It
does not mutate a running session. Validation must reject, before publication:

- unknown IDs, fields, capabilities, assets or schema versions;
- values outside bounds or invalid cross-field combinations;
- dangling references, cycles, illegal spawn/activation graphs or collisions;
- incompatible artifacts, hashes, provenance or save/replay declarations;
- raw MC2 identifiers outside the compatibility adapter;
- unsupported behavior or a definition whose runtime consumer is absent.

`diff` is deterministic and semantic. `apply` is a guarded compare-and-swap of
one validated definition revision. Existing sessions stay on their original
revision until explicit reset/reopen. Rollback is another guarded publication
of a retained definition, never an untracked history rewrite.

Agent permissions are semantic allowlists over operations, paths, scopes and
bounds. A restricted agent can change a sound slot or creature parameter
without gaining permission to edit unrelated definitions or engine source.

## Runtime authority and observations

Rust owns all live gameplay state, ordering, RNG, physics, AI, objectives,
economy, hashes, saves and replays. One arena, allocator and numeric scheduler
serve all packages and levels. Semantic catalogs and family modules do not
create private runtime authorities.

Clients receive versioned commands, snapshots, events, audio instructions and
presentation bindings addressed by semantic IDs. Zero, one or many observation
polls between identical steps must not change gameplay. Tests and tools may
inspect live state but may not present direct setters as production authoring.

Pause is an explicit runtime operation, not repeated snapshot polling. A
package compatibility adapter may declare deterministic paused-frame work (the
MC2 Level 1 source, for example, retains a bounded pre-gate RNG/presentation
prefix) while gameplay ticks and entity actions remain frozen. That work must
be replay-visible and may run only through `paused_frame`.

Save/replay envelopes are opaque to clients and bind complete authoritative
state or a complete deterministic trace, package/definition compatibility,
runtime mode, semantic level, RNG, journals and pending presentation/audio
lifecycle. Loading creates a new generation and invalidates stale commands and
cursors. Level completion is a semantic event consumed exactly once by the
outer runtime. Presentation/audio pages use stable generation-bound cursors and
semantic asset IDs across all levels.

## MC2 compatibility boundary

The MC2 adapter translates package-present raw class/model/action/family/slot
records and owner artifacts into the semantic definition and compiled behavior
bindings. Raw identities remain private to that adapter and source archaeology.

Pinned remc2 and owner packages determine compatibility behavior. They do not
determine the public vocabulary, create Level-N sessions, or justify duplicating
engine authority. Source archaeology exists to fill a named semantic capability
and its lifecycle, not to select work one callback at a time.

## Unit of delivery

Work proceeds by complete semantic domains, not friendly callback names. A
domain is delivered only when the same work unit includes:

1. semantic catalog definition and authorable descriptors;
2. package/MC2 translation for every occurrence in Levels 1–3;
3. asset and compiled-behavior bindings;
4. the complete package-reachable runtime lifecycle and first downstream
   consumer;
5. strict validation, deterministic diff/hash/reset/save behavior;
6. one real positive route and the nearest plausible wrong implementation as
   a negative; and
7. an explicit No-Claim for anything outside the proved boundary.

For a creature family this includes construction, movement/AI, bounds, damage,
death, outputs, objective census and reclaim wherever the package uses it. For
a spell it includes pickup/ownership, activation, effects, costs, presentation
and cleanup. The same completeness rule applies to items, buildings, terrain,
objectives, audio, UI flow and save/load.

An isolated callback may be an implementation step, but it is not a milestone
and must not displace the enclosing domain deliverable.

## Parallel-agent operating model

Use parallel agents when independent work exists. The normal family/domain
shape is:

- one read-only agent pins package presence and source/runtime chronology;
- one read-only agent audits current Rust ownership, gaps and reusable seams;
- one read-only adversarial agent specifies the positive, planted negative and
  No-Claim;
- the primary agent integrates the decision and owns the write surface, or
  delegates disjoint implementation surfaces with explicit reservations.

Subagent reports are claims. The primary agent rereads the relevant source,
reviews test/gate changes independently and reruns evidence at the exact
candidate. Do not dispatch several writers into the same arena/scheduler or ask
an agent merely to “make tests pass.” Parallelism serves complete capability;
it is not a reason to fragment work into micro-items.

## Physical ownership and build discipline

The semantic architecture must become visible in the Rust crate graph. Apply
the installed isomorphic de-monolithization and simplification skills when a
proven ownership seam is ready: split `sim-core` into real internal crates
whose dependency direction follows domain ownership, while preserving one
authoritative engine and identical observable behavior. Do not create cosmetic
wrapper crates, duplicate schedulers or speculative interfaces merely to make
the graph look modular.

Move a coherent implementation and its focused tests together. Day-to-day
verification compiles and tests the owning crate plus the narrow consumer
bridge; the full workspace suite is reserved for integration milestones and
changed shared contracts. Preserve the existing low-memory compiler profile
mitigation unless measured evidence supports a replacement, and do not restart
yielded or unchanged expensive builds.

Before deleting build output or caches, audit disk usage and active processes.
Delete only artifacts whose provenance and regeneration command are proven.
Never delete source, owner data, dirty work, captures or current evidence.

## First-three-level pause milestone

Continue until one current engine build and the private `MC2-owned` package
prove the following for Levels 1, 2 and 3:

- every package-present semantic object is catalogued, addressable and bound to
  validated assets and supported compiled behavior;
- each level is composed from semantic placements, spawns, objectives,
  activation graph, mechanics and presentation bindings rather than public raw
  MC2 selectors;
- every reachable behavior lifecycle has a real production consumer, with
  unsupported paths failing before a package/session is accepted;
- startup/title -> new/load -> loading -> play Level 1 -> completion/transition
  -> Level 2 -> transition -> Level 3 -> completion/return flow works through
  one Rust runtime contract;
- pause/resume, save/load, reset, deterministic replay and generation/hash
  identity work across that route;
- Unity presents title/menu/loading/HUD/audio/effects from Rust-owned semantic
  state without becoming a gameplay writer;
- an authoring client can discover, stage, validate, diff and apply bounded
  changes including representative asset, sound, tuning, behavior and level
  placement edits without changing engine or client executables; and
- focused crate tests remain cheap, while a milestone integration gate proves
  the complete route with real packages and consumers.

This milestone does not claim later campaign levels, commercial distribution,
original replacement content, VR or arbitrary user code. Those remain later
work over the same contract.

## Drift stop

If work starts from a raw callback, selector or fixture, name the semantic
domain and the exact missing lifecycle edge it closes. If it cannot be attached
to the current complete-domain deliverable and nearest production consumer,
stop and return to the highest-priority unfinished domain.

Do not create a new tracker, matrix, certificate, dashboard or architecture
round to enforce this document. The enforcement is the code shape, strict
authoring API, focused tests and the first-three-level runtime gate.

## Current migration frontier

As of the semantic-contract 29.0 milestone, this is a binding target with
partial real implementation, not a completion claim:

- the transaction store provides guarded atomic/idempotent publication through
  a discoverable closed operation schema; the first consumed operation binds
  `flight.player.max_forward_speed` through the ordinary neutral campaign
  player callback for the real Level 1, 2 and 3 packages, while operations
  without campaign runtime consumers fail before publication;
- the package-bound `spell.fireball.mana_cost` descriptor is consumed for the
  qualified Level 1 local player's already-enabled tier-zero holder. Explicit
  reopen/reset writes both the generic activation affordability carrier and
  the ordinary source callback's charge carrier before resealing the reset
  baseline; Level 2 and 3 reject cost publication because their local-player
  family-zero holder relation is absent. This does not yet author spell
  acquisition, higher tiers, enemy holders or projectile behavior;
- the package-bound `ai.enemy.default.policy` descriptor selects the
  source-exact classic Goat policy or the compiled vigilant policy for each
  Goat-bearing qualified Level 1-3 definition. Explicit reopen/reset binds
  the selection to the shared ordinary Goat-family configuration, including
  Level 3 Goats which are package-present but not reset-live. The focused
  causal proof covers the Level 1 engagement decision; Goat population,
  movement parameters other than placement, and other creature families
  remain outside this policy slice;
- all eighty package-present Goats in the qualified Level 1-3 definitions now
  carry writable session-reset placement and per-instance `max_life`
  descriptors backed by the exact compiled Goat lifecycle capability. The
  reset-live binding synchronizes the private imported carrier with the typed
  Goat family before ordinary visits, while grouped Level 3 Goats receive the
  authored placement and life value in their real package constructor. Focused
  causal proofs move and raise the life of one real Level 1 Goat before its
  ordinary visit/mailbox-damage branch and bind one real Level 3 group-5 Goat
  before construction, with old-session isolation and reset reproduction.
  This does not author Goat population, policy parameters beyond the separately
  bound policy slot, organic combat/death outputs, objective retargeting,
  save/replay, or Unity UI;
- each of the six package-present Level 1 Fireflies now carries a stable,
  per-instance `max_life` descriptor. Guarded `SetEntityMaxLife` publication
  leaves already-open sessions unchanged; explicit reopen/reset binds the
  authored value before the real package group constructs that Firefly, and
  the ordinary damage callback consumes it through its pre-kill branch. The
  focused causal proof authors one instance from 600 to 1200 life and applies
  identical damage to the old and reopened authorities. This does not author
  another Firefly in the same transaction, other creature families or health
  parameters, projectile travel/impact production, save/replay, or Unity UI;
- every package-present Archer in the qualified Level 1-3 definitions now
  carries the same per-instance `max_life` descriptor, backed by the exact
  compiled Archer lifecycle capability. The focused causal proof authors one
  Level 1 Archer from 1000 to 2000 life: the old session enters its hit branch
  after a source-mailbox damage visit, while explicit reopen/reset constructs
  the authored authority and the same damage remains nonterminal. This does
  not author multiple Archers in one transaction, targeting/child production,
  organic combat, another creature family, save/replay, or Unity UI;
- every package-present Dragon in the qualified Level 3 definition now
  carries a per-instance `max_life` descriptor backed by the exact compiled
  Dragon lifecycle capability. The focused causal proof changes one real
  Dragon root and all sixteen source-copied body rows, then sends identical
  damage through one body's ordinary mailbox after explicit reopen/reset.
  This does not author multiple Dragons in one transaction, prove organic
  combat or targeting, alter body count/motion/fireballs, support another
  creature family, save/replay, or Unity UI;
- every package-present authored Bee in the qualified Level 2 and 3
  definitions now carries the same per-instance `max_life` descriptor with
  the source-width bound `1..=32767`. The focused causal proof changes one
  real Level 2 group-21 Bee from 3000 to 6000 life, preserves its constructor's
  half-life mana derivation, and sends identical damage through the ordinary
  action-17 mailbox after explicit reopen/reset. This does not author Morph
  Bees, multiple Bees in one transaction, targeting/combat/death, another
  creature family, save/replay, or Unity UI;
- every package-present Skeleton in the qualified Level 2 and 3 definitions
  now carries a per-instance `max_life` descriptor backed by the exact
  compiled Skeleton lifecycle capability. The Level 2 authored-group
  constructor and Level 3 reset-live importer both consume it after explicit
  reopen/reset. The focused causal proof changes one real Level 3 Skeleton
  from 1000 to 2000 life and sends identical damage through its ordinary
  model-9 mailbox. This does not prove organic projectile travel, later death
  outputs/objective cleanup, other Skeleton instances, save/replay, or Unity UI;
- all twenty-seven package-present Level 2 Traders now carry a per-instance
  `max_life` descriptor backed by their ordinary group-3 constructor and
  action-113 mailbox damage lifecycle. Explicit reopen/reset binds the authored
  value before construction; the focused causal proof changes one Trader from
  1000 to 2000 life so identical mailbox damage takes the package authority
  through pre-kill while the authored authority remains live. This does not
  prove organic group progression, Trader targeting/death cleanup, other
  Traders, save/replay, or Unity UI;
- all fifty-seven package-present reset-live Villagers in Levels 1 and 2 now
  carry a per-instance `max_life` descriptor. Explicit reopen/reset updates
  both the imported source row and the typed Villager-family carrier before
  the ordinary damage mailbox consumes it. The focused causal proof changes
  one Level 2 Villager from 1000 to 2000 life so identical damage kills the
  package authority while the authored authority enters its hit action. This
  does not prove organic targeting/building entry, death cleanup/population
  effects, other Villagers, save/replay, or Unity UI;
- all four Level 1, six Level 2 and five Level 3 package-present Worms now
  carry per-instance `max_life` descriptors backed by one exact compiled
  root/body lifecycle capability. Dynamic group construction and reset-import
  both copy the authored value into the root and all sixteen body carriers on
  explicit reopen/reset. The focused causal proof changes one Worm in each
  representation from 9000 to 18000 life: identical child damage kills the
  package authority but leaves the authored family live. This does not prove
  organic group progression, every Worm instance, targeting/projectiles,
  population authoring, save/replay, or Unity UI;
- the qualified Level 1 campaign package now retains two profile-pinned,
  canonical `carpet-flight-indexed-sprite/v1` one-frame Fireball assets. Its
  semantic projectile slot binds the base payload and discovers the fade
  palette as a compatible package-retained alternative; guarded
  `BindAsset` publication takes effect only on explicit reopen, while the
  neutral session query is observational and reset-stable. The ordinary
  Level 1 Fireball producer now also projects a live opaque semantic
  presentation entity carrying its package-bound asset, pose and logical
  height. The production `MagicCarpetRuntime` resolver mode consumes that
  projection from the same Rust session handle and renders the actual
  content-addressed one-frame texture without consulting an owner cache or
  raw TMAP selector. The additive observation is negotiated through
  game-session contract v3/native-session JSON v3; the semantic and outer-runtime
  contracts are unchanged. Level 2 and 3
  continue to advertise the dangling catalog reference as read-only because
  their packages retain no typed compatible payloads. This does not yet prove
  animation, impacts, other entity presentation, arbitrary artifact ingestion,
  a complete playable scene, or distribution;
- `LevelDefinition` exposes a complete stable-ID census for the qualified real
  Level 1, 2 and 3 package entities; those identities are frozen in the private
  MC2 adapter rather than derived from kind ordinals or runtime slots;
- typed semantic `MoveEntity` changes are published atomically and consumed on
  explicit reopen by the real Level 1 start-area trigger and every semantic
  spell pickup in the first three qualified packages. Reset-live Level 2
  Beyond Sight and Speed plus Level 3 Shield and Invisibility retain their
  imported carrier; dynamically authored Level 1 Castle and Speed, Level 2
  Shield and Invisibility, and Level 3 Morph bind their immutable authored
  feature before the package group constructs the ordinary pickup. The same
  reset-versus-authored binding covers all fourteen package-present Stones and
  all sixty-seven package-present Trees: Level 1/2 authored-group rows bind
  their immutable feature before construction, while Level 3 reset-live rows
  relocate their imported carrier. Their callbacks consume the authored
  location through terrain ground/material projections.
  All fifty-four package-present position-sensitive switches across Levels
  1-3 use the same reset-versus-authored split: group-zero Area Entry/Exit and
  Exit Sequence triggers relocate their imported carrier, while grouped
  triggers bind their immutable feature before construction. Their ordinary
  overlap cadence consumes the authored location; Objective and Level Exit
  gates remain read-only because their package positions are not gameplay
  inputs. The same authored-feature carrier now makes all sixty-one
  positive-group placements for complete-lifecycle Bee, Dragon, Archer,
  Firefly and Skeleton families writable across Levels 1-3. Their real group
  producers construct the ordinary shared-arena creature at the authored
  location; the focused causal proof follows a Level 1 Firefly through its
  ordinary movement and map-chain visit after reopen/reset. The eleven
  reset-live Level 3 Skeletons now relocate their exact imported carriers;
  one focused causal proof reaches the ordinary model-9 movement and map-chain
  consumer after reopen/reset. The ten positive-group Worm placements in
  Levels 1 and 2 now bind their immutable authored feature before the real
  multipart constructor copies the location into the root and all sixteen body
  rows. One focused Level 1 proof follows that root through ordinary movement
  and map-chain service after reopen/reset. The five reset-live Level 3 Worms
  now relocate their exact imported root and all sixteen linked body rows by
  one semantic tile delta, preserving their relative sub-tile positions and
  map membership before ordinary visits. One focused proof follows a real
  Level 3 root/body family through its source movement after reopen/reset.
  All twenty-seven package-present Level 2 Traders now bind their immutable
  authored-feature position before the real group-3 constructor allocates the
  ordinary action-113 carrier. One focused proof follows a constructed Trader
  through its ordinary numeric visit and map membership after reopen/reset.
  All fifty-seven package-present Level 1 and 2 Villagers now relocate both
  their imported arena row and reset-live typed-family carrier through an
  exact private source-record binding. One focused Level 2 proof follows a
  real free Villager through its ordinary movement and map membership after
  reopen/reset. The eighty-five package-present Houses with an exact
  reset-live source occurrence now relocate both their imported arena row and
  typed `HouseFamily` through the same private occurrence binding. One focused
  Level 1 proof follows a real action-52 House through its ordinary numeric
  visit and map membership after reopen/reset. The thirty-four positive-group
  Level 1 Houses now bind their immutable authored feature before the real
  group producer constructs the ordinary action-51 carrier. One focused proof
  moves group-13 record 107, drives the source countdown and owner-pinned BUILD
  footprint to a live action-52 House, and reproduces the authored placement
  after reset. The remaining eighty-four construction-only or otherwise
  unbound House rows stay read-only. This does not author House construction
  parameters, ownership, population, health, damage or destruction. All
  thirty-nine package-present Experience Scrolls across Levels 1-3 bind both
  their location and per-instance XP award to their immutable authored-feature
  rows before construction. Typed session-reset publication isolates already-open
  sessions; explicit reopen/reset supplies the authored award to the ordinary
  class-14 pickup callback, which applies it to enabled local spell families,
  hides the row in the same pass and reclaims it at the following boundary.
  This does not author scroll population, enabled spell families, remote-player
  reward policy, persistence or Unity UI. The three package-present
  Exit Portal placements use the same authored-feature binding; the real exit
  trigger captures the constructed portal and player action 12 consumes its
  authored position for transition heading and movement. Movement preserves
  sub-tile position and map membership, plus the shared spell projection for
  spell pickups. Every other placement remains read-only until its own runtime
  materialization binding is implemented;
- all fifteen qualified Level 1, 2 and 3 objectives have stable semantic IDs;
  typed `PutObjective` authoring is published atomically and consumed on
  explicit reopen/reset for the five package-present `ReachLocation`
  conditions, the Level 1/3 `CastleManaPercent` conditions, and all six
  package-present `NoLivingCreatures` conditions. The ordinary Castle and
  complete-lifecycle creature-absence evaluators plus their following-pass
  authored-group gates consume those authored values. The Level 2
  `NoStandingBuildings` condition is now evaluated, but remains read-only, from
  its package-bound private BLDGPRM chain, distinct source frame cadence, and
  frozen pass-start model-45 membership before its following-turn exit-group
  consumer. The Level 2 rival-wizard condition now carries the stable semantic
  target `rival_wizard.nyphur`; the private MC2 adapter binds that identity to
  the validated Nyphur player/entity relation, and the ordinary objective pass
  fails closed if the semantic and runtime targets diverge. It and
  `NoStandingBuildings` remain read-only: this does not provide alternate
  rival selection or author rival players;
- every package-present `AbsenceCountdown` in the first three levels has a
  stable instance ID and a typed `arming_ticks` descriptor; guarded
  `SetMechanicCountdown` publication is consumed by both dynamically authored
  Level 1 carriers and the imported Level 3 carrier, while Level 2 rejects the
  absent mechanic before publication. The live timer still starts at zero and
  loads the configured delay only on its first clear-family visit;
- the first Rust outer runtime owner now guards
  `Title -> Loading -> Playing <-> Paused`, opens a real qualified Level 1
  `GameSession`, rejects a mismatched package without changing runtime state,
  and freezes that same session behind generation-invalidating pause/resume
  boundaries. Each qualified Level 1-3 definition now carries the same closed,
  ordered semantic campaign flow plus its bound semantic level; `new_game`
  admits only the declared entry and Loading/Playing/Paused observations retain
  that definition-owned identity. The neutral session now projects successful
  completion only from the persistent phase-12 level-exit carrier, and the
  Rust runtime consumes it exactly once. A Rust-owned installed-package
  resolver validates the complete unordered Level 1-3 resource set, derives
  every key from immutable semantic flow identity, and rejects mixed,
  duplicate or incomplete sets before play. The native runtime and its managed
  Unity transport now bootstrap that complete resolver-owned set atomically
  from unordered owner-local resource locations and open the resolver-derived
  Level 1 entry on the same handle; neither client supplies a semantic entry or
  next-level identity. A focused real Level 1 route
  accepts only its declared Level 2 edge without a caller level ID or path,
  opens that exact prevalidated definition/resource binding with a monotonic
  nested session generation, and rejects stale Level 1 guards. A separate
  focused staged-boundary Level 2 route now drives the ordinary final-objective,
  exit-group, trigger and phase-12 lifecycle, accepts only the resolver-owned
  Level 3 edge, and opens its exact installed binding on the same handle.
  A corresponding staged-boundary Level 3 route drives its final Bee objective,
  exact exit group and phase-12 carrier into the declared semantic Title node,
  then preserves the nested generation floor when the same handle starts a new
  Level 1 session so stale Level 3 guards cannot collide.
  Native/Unity completion-transition exposure, production package discovery,
  installation and shell wiring,
  an uninterrupted Level 1-to-Title playthrough, and Title/results presentation
  remain open. The
  quiescent boundary also does not yet implement `paused_frame`, so the MC2
  pre-gate RNG/objective/presentation work remains open. Its Unity transport
  now validates that flow identity and borrows snapshot, step, event and reset
  operations from the nested session on that same runtime handle, without a
  second session or destroy path. The production `NativeSession` facade can
  now own that resolver-derived handle and borrow the same generic gameplay
  facet without opening either legacy registry. `MagicCarpetRuntime` has an
  explicit headless production mode that admits an unordered owner-local
  package set, lets Rust select the semantic entry, and drives fixed-step/reset
  through that same handle. Production mouse/JLIK steering and held movement
  input now enter that same command path, while the semantic camera follows
  only the Rust-returned player pose. Production P-key input toggles the same
  Rust-owned quiescent pause boundary and discards gameplay input received
  while paused rather than queuing it for resume. Its first bounded
  presentation consumer renders the live Level 1 Fireball described above.
  A generic flight telemetry strip now also renders actual and target speed,
  strafe speed, altitude, tick and runtime mode from that same returned Rust
  snapshot. After each fixed step the same owner now drains the ordered,
  opaque-actor semantic event pages through the nested session guard and shows
  the latest retained event in that strip without replaying it on another poll.
  Source-art HUD, life/mana/objective UI, terrain, world-space effects and audio
  playback have not yet migrated to this owner. The resolver
  can now serialize one real Level 1 Playing or quiescent Paused session as its immutable
  semantic definition, ordered neutral command trace and exact state/terrain/
  effect receipts, then guard-first replay it into a fresh runtime/session and
  publish only after every receipt matches. A self-consistent envelope with a
  false state receipt is rejected while Title remains unchanged. Additive
  native and managed transports treat those bytes as opaque, bind save to the
  exact Playing/Paused runtime/session generation and tick, and atomically load them
  with a complete unordered package-resource set into one fresh resolver-owned
  handle. The production `NativeSession` and `MagicCarpetRuntime` owner now
  carry that same replay through disposal into one fresh same-authority owner
  and resume ordinary fixed-step play. Its owner-local filesystem seam writes
  through a flushed sibling temporary file and atomic replacement, bounds the
  file before reading, and still treats the contents as opaque Rust data. This
  writer is triggered by F5 or the same internal request seam when
  `-mc-runtime-replay-save <path>` is configured, and the loader is selected by
  the production `-mc-runtime-replay-load` startup option. This is one Level 1 replay
  checkpoint, not paused-frame service, cross-level persistence, menu UI,
  migration or a full campaign save;
- broader placement/spawn/behavior/asset authoring and the remaining objective
  fields are not yet complete, and
  package definition/behavior artifacts are not yet fully parsed as the typed
  composition above;
- the MC2 adapter produces useful semantic bindings, while source-shaped
  compatibility types remain implementation-visible. The shared, pure
  ground-creature retry and terrain-transition algorithm is now owned by the
  internal `sim-ground-motion` crate; `sim-core` retains only its fixed-width
  `FlightWorld` projection while Archer, Dragon and Firefly lifecycles keep
  their existing scheduler/RNG authority. The dependency-free Goat source-map
  chain is now owned by the internal `sim-goat-map-chain` crate, while the
  existing `sim_core::goat_map_chain` path remains a compatibility façade and
  Goat scheduling stays in `sim-core`. The pure terrain-slope camera and
  terrain/object projection gates are now owned by the internal
  `sim-render-camera` crate, while terrain-grid, Classic HD and Goat draw
  scheduling remain in `sim-core`. The dependency-free dynamic-music control
  envelope is now owned by the internal `sim-music` crate, while campaign
  scheduling, event delivery and audio playback remain with their existing
  owners. The exact selector-global Goat texture-map bank and latch clocks are
  now owned by the internal `sim-goat-tmap` crate, while owner-cache decoding,
  terrain draw admission and native selector-frame transport remain with their
  existing owners. The dependency-free renderable source-map chain is now
  owned by the internal `sim-renderable-map-chain` crate, while reset import,
  live entity synchronization and projectile collision remain with their
  existing `sim-core` owners. Checked Classic HD render profiles, frame buffers
  and writer primitives are now owned by the internal `sim-classic-hd` crate,
  while terrain/raster production and native session scheduling remain with
  their existing owners. The source-compatible F1FA/type-7 mutable SS2 delta
  decoder and hash/header checks are now owned by the internal `sim-ss2-tmap`
  crate, while selector qualification, clocks and draw scheduling remain in
  their existing family owners. These are eight bounded physical
  extractions, not a claim that the remaining `sim-core` monolith has been
  decomposed; and
- the complete title/loading/save/transition flow and full Levels 1–3 domain
  coverage remain open; the current Unity build enters one gameplay scene
  directly, neutral campaign saves/loads do not yet span Levels 1-3 or a
  production menu flow, and level completion has not yet been
  proved as one uninterrupted Rust-owned 1 -> 2 -> 3 -> Title playthrough or
  exposed through the production client.

The next foundation seam extends the same stable definition-to-runtime binding
to the next package-present semantic domain without adding another arena,
scheduler or Level-N authority. Creature-family closure continues as capability
implementation under that definition; it does not replace the authoring seam
or become another public architecture.
