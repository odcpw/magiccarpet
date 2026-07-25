# Modular carpet engine and game-module contract

> Historical decision: superseded on 2026-07-23. MC2 is no longer the first
> distributable module of the commercial engine. The replacement contract is
> `docs/modular-engine-game-module-contract.md`.

Version: 0.1  
Status: accepted preproduction contract  
Tracking bead: `mc-3stn`  
Depends on: product promise v0.4, portability v0.2, multiplayer session v0.2

## Decision

Build one reusable deterministic carpet-flight engine. Magic Carpet 2 is the
first complete game implementation hosted by that engine, not a privileged
engine mode.

The engine must boot, simulate, save, replay, serve, and render a wholly
synthetic game module with the MC2 module absent. A future total conversion
must be able to replace MC2 without editing engine assemblies. Smaller mods may
replace or extend declared package layers without reaching around the public
module API.

Chrome is a first-class constraint. The normal browser mod path is validated
data, assets, and bounded declarative graphs interpreted by code already in the
build. It never downloads C#, Wasm extensions, native plug-ins, or other
executable code.

## Architectural laws

1. **One simulation truth.** Chrome, native clients, replay tools, tests, and
   Linux authority consume the same deterministic rules and package hashes.
2. **No MC2 knowledge in the engine.** Engine code contains no MC2 spell,
   creature, rival, realm, objective, network-level, HUD, cue, save, or
   progression identifier.
3. **Capabilities, not game switches.** A module requests stable engine
   capabilities. Code never branches on `if MC2`.
4. **Data cannot obtain ambient authority.** A package sees only declared,
   bounded operations and information. It cannot access files, sockets,
   clocks, threads, reflection, processes, environment variables, or Unity
   objects.
5. **Exact identity before use.** Packages are canonicalized, hashed,
   dependency-resolved, budget-checked, provenance-checked, and allowlisted
   before activation or room admission.
6. **Safe absence and removal.** Missing content blocks only the dependent
   module/save/room with an exact remedy. Removing a module does not corrupt
   the engine or unrelated modules.
7. **Explicit evolution.** Schema and behavior changes create versions and
   migrations. The runtime never guesses compatibility.
8. **Presentation is not authority.** Render, audio, camera, UI animation, and
   local accessibility choices may differ without changing authoritative
   state or information eligibility.
9. **Private owner data is not a package distribution tier.** An optional
   approved owner-data overlay lives in separate local private storage and is
   never served by the project, room, CDN, server, or peer.

## Layer and dependency model

Logical assembly names are provisional until `mc-ads`; dependency direction is
normative.

```text
Carpet.Contracts
  <- Carpet.Determinism
  <- Carpet.Simulation.Core
  <- Carpet.Module.Runtime
  <- Carpet.Persistence
  <- Carpet.Network.Protocol

Carpet.Module.Runtime
  <- Carpet.Unity.Client
  <- Carpet.DedicatedServer
  <- Carpet.ReplayRunner
  <- Carpet.Module.Tools

GameModule packages
  -> Carpet.Contracts and declared capability IDs only

Presentation packages
  -> public game-module information/cue IDs only
```

### `Carpet.Contracts`

Pure managed types for stable IDs, fixed-width values, commands, events,
capability descriptors, manifests, validation results, deterministic
collections, read models, package provenance, and compatibility tuples.

It has no Unity, transport, filesystem, JSON-library, MC2, or presentation
dependency.

### `Carpet.Determinism`

Canonical tick, fixed/quantized numeric policy, RNG streams, ordering,
canonical serialization, hashing, bounded graph execution, property/golden
test hooks, and divergence diagnostics.

### `Carpet.Simulation.Core`

Module-neutral world/entity lifecycle and accepted capability primitives:
kinematic carpet motion, spatial queries, terrain operations, damage/effects,
finite-resource custody ledgers, relationships, objectives, scheduled events,
AI command interfaces, and information eligibility.

A primitive belongs here only when at least the MC2 module and synthetic
module can express it without an MC2-specific name or law. Adding a primitive
changes the engine API and simulation build identity.

### `Carpet.Module.Runtime`

Manifest loading, canonicalization, dependency/capability resolution,
validation, package lifecycle, bounded behavior execution, catalog
registration, module read models, diagnostics, and safe teardown.

### `Carpet.Persistence`

Save, checkpoint, campaign/profile, replay, result, migration, and
missing-module recovery envelopes. It stores module-owned opaque state only
through versioned schemas and canonical serializers.

### `Carpet.Network.Protocol`

Canonical compatibility tuple, admission, commands, snapshots, permitted
views, reconnect, checkpoint/tail transfer, results, and mismatch failures. It
does not acquire packages or define game rules.

### Unity, server, replay, and tools

- `Carpet.Unity.Client` owns rendering, input adapters, camera, audio, HUD,
  browser lifecycle, package-acquisition UI, and local accessibility.
- `Carpet.DedicatedServer` owns authoritative room orchestration, allowlists,
  rate/budget enforcement, persistence adapters, and operations surfaces. It
  loads no rendering, audio, input, or player-facing UI.
- `Carpet.ReplayRunner` runs the same simulation and module set without Unity
  presentation and reports the first divergent tick/field/hash.
- `Carpet.Module.Tools` owns schemas, validators, packer, signer/provenance
  receipts, migration runner, preview, synthetic fixtures, and documentation.

## Package kinds

Every package has one kind. A package cannot claim capabilities reserved for a
different kind.

| Kind | Owns | May depend on |
|---|---|---|
| `GameModule` | Root game identity, capability/catalog set, state schemas, default campaign and rules references | Engine API and declared libraries/packs |
| `BehaviorLibrary` | Bounded reusable effects, conditions, selectors, objective nodes, and AI policy graphs | Engine capabilities only |
| `CampaignWorldPack` | Worlds/realms, topology, spawns, encounters, objectives, secrets, progression rows, exits, budgets | One compatible game module and behavior libraries |
| `RulesProfile` | Relationships, costs, damage filters, death/rebuild, objectives, victory, timeout, team/co-op/FFA laws | One compatible game module |
| `PresentationPack` | Models, materials, textures, VFX, audio, music, UI/icon/font references, cue bindings | Public semantic and cue IDs |
| `LocalizationPack` | Locale-specific strings, captions, narration text, pronunciation and layout metadata | Public string IDs |
| `PrivateOwnedDataOverlay` | Locally normalized presentation/data references for one approved source profile | Exact compatible game module; never normal package distribution |

`GameModule` is singular for one running game. Other package kinds can be
composed only when the root module explicitly accepts their capability and
schema ranges.

## Manifest identity

The canonical manifest contains at least:

- schema ID and version;
- package kind;
- publisher namespace;
- immutable package ID and semantic version;
- engine API range;
- game-module ID/range where applicable;
- deterministic simulation schema version;
- declared dependencies with exact or bounded versions;
- provided and required capability IDs;
- rules-profile and behavior-graph schema versions;
- ordered content-file table with media type, size, digest, role, provenance
  class, license ID, and budget group;
- canonical rules root and content root;
- save/replay schema and migration IDs;
- localization/presentation compatibility ranges;
- target support and explicit target exclusions;
- compressed/uncompressed/file-count/graph/memory budgets;
- publisher/build provenance and optional signature;
- revocation/support metadata.

Package filenames, archive order, JSON key order, timestamps, and compression
choices are not identity. Canonical manifest bytes, canonical rule records, and
ordered content digests are.

Stable IDs are opaque semantic identifiers, never array positions, display
names, filesystem paths, source addresses, or original private identifiers.

## Resolution and activation lifecycle

1. **Discover** a root game module or room requirement.
2. **Resolve** one acyclic package graph with exactly one version per package
   ID unless a schema explicitly permits isolated side-by-side data.
3. **Acquire** public packages transactionally after user consent. Private
   overlays are selected and imported locally through their separate gate.
4. **Verify** bytes, canonical roots, signatures/revocation, provenance,
   licenses, package kind, schema ranges, capabilities, and budgets.
5. **Compile** declarative graphs to a bounded validated intermediate form.
   This creates no runtime code.
6. **Register** catalogs and schemas into a fresh module scope. Duplicate or
   unresolved stable IDs fail.
7. **Dry initialize** deterministic state and compute the final compatibility
   tuple without entering a room or mutating a save.
8. **Activate** only after all clients and the authority accept the exact
   tuple.
9. **Run** with per-tick, per-graph, per-entity, and information-flow budgets.
10. **Suspend/checkpoint** through module-owned versioned state.
11. **Teardown** in reverse dependency order and prove no registrations,
    assets, state, subscriptions, or caches leak into the next module.

Any failure is atomic: the previously active known-good set remains usable.

## Deterministic behavior model

Declarative behavior is intentionally less powerful than C#.

The baseline graph families are:

- effect graphs;
- objective/trigger graphs;
- AI policy/selector graphs;
- spawn/ecology graphs;
- progression/unlock graphs;
- UI-information derivation graphs; and
- presentation cue graphs, which consume authoritative events but produce no
  authoritative commands.

Nodes use a reviewed registry of pure operations. Each operation declares:

- input/output types and units;
- authoritative or presentation domain;
- deterministic ordering and RNG-stream use;
- read/write state categories;
- information eligibility;
- maximum fan-out, iterations, allocations, emitted events, and cost units;
- failure behavior; and
- schema/migration owner.

Unbounded loops, recursion, dynamic type creation, reflection, wall-clock time,
ambient RNG, filesystem/network access, arbitrary queries, hidden global
state, and presentation-to-authority callbacks are invalid.

Validation rejects cycles except explicitly bounded state-machine transitions,
unknown nodes, type/unit errors, capability escalation, information leaks,
budget overflow, ambiguous ordering, nondeterministic iteration, and
target-specific authoritative behavior.

## Multiplayer compatibility

The immutable session tuple is:

```text
EngineApiVersion
+ SimulationBuildHash
+ ProtocolVersion
+ GameModuleSetId
+ ContentRoot
+ RulesProfileId
+ RulesHash
[+ OwnedDataCompatibilityRoot]
```

The optional final field appears only for a separately approved local
owner-data overlay. It proves equality of independently normalized inputs, not
ownership or permission to distribute.

The authority publishes required public package IDs, versions, sizes,
provenance, and roots. It never publishes private paths, source hashes,
original payload metadata, analyzer data, or owner-data bytes.

A mismatch is not repaired by choosing the nearest version. Public content may
be acquired with explicit consent through the normal package channel. Private
content can only be selected/imported locally by that participant or server
operator.

## Saves, campaigns, checkpoints, and replays

Every artifact records:

- the canonical compatibility tuple;
- package graph and schema versions;
- module-owned state sections with stable type IDs;
- canonical simulation state/checkpoint hash;
- ordered terrain/world operation tail where applicable;
- command/replay stream identity;
- migration history; and
- presentation/localization references only when required to reproduce a
  player-facing capture, never for authoritative replay.

Opening an artifact follows one of four explicit outcomes:

1. exact open;
2. deterministic reviewed migration to a named target;
3. safe blocked state with missing/revoked/incompatible dependency details;
4. corrupt/unsupported rejection preserving the original artifact.

Removing a module does not delete its saves. Reinstalling the exact compatible
set may make them usable again.

## Chrome delivery and storage

- Public packages use content-addressed immutable objects and a small
  replaceable index/manifest.
- Acquisition preflights compressed size, expanded size, file count, memory,
  browser quota, schema, target support, and revocation.
- Download, verification, staging, and activation are resumable and atomic.
- IndexedDB/cache roles are explicit. Service-worker/CDN caches never contain
  a `PrivateOwnedDataOverlay`.
- Cold/warm module and realm loads, total/peak memory, eviction, private mode,
  interrupted download, corrupt cache, old-client, rollback, and safe removal
  have acceptance vectors.
- A module may stream presentation content, but all authoritative rules needed
  for a match are present and validated before admission.

## Security and provenance

Reject before activation:

- traversal, absolute paths, links, duplicate/case-colliding names, nested
  package tricks, archive/decompression bombs, oversized counts and integers;
- undeclared files, media-type mismatches, executable formats, scripts,
  assemblies, native libraries, Wasm modules, shaders outside the approved
  build-time pipeline, and hostile serialized objects;
- unknown schemas/capabilities/nodes, cyclic dependencies, graph cost bombs,
  excessive assets, pathological meshes/textures/audio, and information-flow
  violations;
- missing/incompatible licenses, absent provenance, private/reference hashes,
  revoked publishers/packages, signature failure, and content-root mismatch.

Module diagnostics contain stable public IDs and bounded validation details.
They contain no secrets, original payload, private paths, player personal data,
or raw owner-data evidence.

## MC2 implementation mapping

The private engineering name for implementation one is `game.mc2`; a public
release ID/name is a later branding decision.

Its package graph will include:

- one root `GameModule` with the full accepted capability/catalog registry;
- all 25 realm packages grouped into three campaign arcs;
- complete spell/tier, ordinary-creature, rival, stronghold/carrier, economy,
  settlement/ecology, terrain/environment, objective, HUD/map/message, cue,
  save/progression, and multiplayer-profile data;
- every evidenced network level as a separate replacement-expression world
  package after its exact roster is known;
- faithful MC2 rules profiles and visibly separate modern co-op/team/FFA
  profiles;
- independently created or properly licensed presentation/localization packs;
  and
- explicit `UNKNOWN` or bounded waiver rows for unresolved behavior, never
  silent omissions.

The redistributable MC2 implementation ships no original executable, code,
map/geometry, art, audio, music, prose, font, table, UI expression, or other
payload.

## Synthetic conformance game

Before the MC2 demo can depend on the module system, a tiny synthetic game must
prove:

- engine boot with no MC2 package installed;
- one carpet-like controllable actor using generic movement capability;
- one resource, one effect, one objective, one AI policy, one result, one
  world, one presentation cue, one save, and one replay;
- Chrome/native/server loading of identical authoritative roots;
- exact room negotiation and deliberate mismatch rejection;
- package install, upgrade/migration, rollback, disable, remove, and reinstall;
- teardown followed by loading MC2 without leaked IDs/state, and the reverse;
- dependency and assembly scans finding no MC2 reference in engine code; and
- malformed, hostile, oversized, revoked, and corrupt package rejection.

The synthetic game is test material, not a second production title.

## Change control

- Engine capability changes require an ADR, new `EngineApiVersion`, synthetic
  and MC2 compatibility vectors, browser/AOT proof, and migration impact.
- Module schema changes require versioned canonical fixtures and migrations.
- A rule change creates a new `RulesHash`, even when display version text is
  unchanged.
- An asset/content change creates a new `ContentRoot`.
- A compiler or authoritative-code change creates a new
  `SimulationBuildHash`.
- No package or migration is considered supported until native, Chrome, Linux
  authority, save/replay, security, and provenance gates name it.

## Explicitly out of scope

- downloadable arbitrary code on the browser baseline;
- hot-swapping authoritative modules during a running match;
- mixing incompatible game modules in one simulation;
- historical MC2 transport/protocol compatibility;
- executing the original MC2 executable through the owner-data adapter;
- server, CDN, or peer delivery of owner-supplied original data;
- claiming legal clean-room status, public-release approval, or permission to
  use original branding; and
- allowing mod support to weaken deterministic, security, accessibility,
  performance, or provenance acceptance.

## Acceptance evidence owned by downstream beads

| Evidence | Owner |
|---|---|
| Package trust, provenance, security and browser-delivery contract | `mc-0fkp` |
| Module host and assembly guards | `mc-u7ja` |
| Declarative schemas and bounded graph engine | `mc-8oqh` |
| Content-addressed packer/validator | `mc-2tt0` |
| Room module negotiation | `mc-41mu` |
| Chrome acquisition/cache/quota/rollback | `mc-m07b` |
| Save/campaign/replay binding and migration | `mc-mgfw` |
| Authoring and validation tools | `mc-ac1z` |
| Synthetic game and two-module isolation | `mc-r7gu` |
| Mod architecture acceptance gate | `mc-bprr` |
| Complete MC2 module integration | `mc-in7t` |
| Optional owner-data decision | `mc-i6z9` |
