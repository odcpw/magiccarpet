# Parallel world-representation backends

Status: Track S architecture direction; exact interfaces remain subject to the
representation-contract gate  
Date: 2026-07-23  
Related: `docs/modular-engine-game-module-contract.md`,
`research/voxel-destruction-option.md`,
`docs/future-destructible-cities.md`

## Decision

The successor carpet engine must not equate “world” with one heightfield.
It supports independently versioned world-representation providers behind a
small deterministic capability boundary.

The initial providers are conceptually:

1. **Deformable heightfield** — the successor island path: a 2.5D quantized
   surface, water plane, masks, ordered terrain edits and structures grounded
   into or presented as outcroppings of that surface.
2. **Bounded destructible cells** — an optional local structure volume layered
   over a heightfield or authored world.
3. **Volumetric voxel chunks** — a future Minecraft-like world in which solid,
   empty and material cells exist throughout three dimensions and support
   tunnels, overhangs, interiors and arbitrary material removal.

These are parallel providers, not modes implemented by branching inside one
heightfield class. A game module or world package selects the provider and
required capabilities explicitly.

The full voxel backend is a later Track S program. Only the provider seam,
capability negotiation and conformance fixtures affect the first social demo.

## Track P evidence and Track S choice

“Heightfield” is a sensible compact canonical design for the original
successor island. Track P may separately determine the original executable's
exact storage and update algorithm. That finding can inform questions and
test methods, but it does not dictate Track S internals.

Owned observation suggests a continuous deformable surface and buildings or
strongholds visually integrated as terrain-like outcroppings. The private
terrain Ghidra/runtime campaigns must determine whether the original uses a
height array, regular mesh, column field, procedural patches, another
representation or a hybrid.

If Track P evidence contradicts the historical hypothesis, record that result
in Track P. Track S changes only when it represents a useful, independently
specified design lesson. The provider-neutral boundary remains valid either
way.

## Why the distinction matters

MC2 terrain is naturally compact and surface-oriented. A heightfield can answer
ground height, slope and deformation queries cheaply, but it cannot represent
two solid surfaces above one horizontal coordinate. Treating it as a sparse
voxel world would distort the faithful implementation.

A volumetric world has different fundamental costs and algorithms:

- chunk occupancy, density and material storage;
- three-dimensional neighborhood and connectivity queries;
- dirty-chunk meshing and collision;
- tunnels, overhangs and internal spaces;
- lighting and visibility through changing volumes;
- three-dimensional navigation invalidation;
- chunk streaming and sparse persistence;
- potentially much larger snapshots and repair traffic.

The engine may standardize the questions gameplay asks without pretending the
providers have the same internal structure or performance envelope.

## Module and world declaration

A world package declares:

- provider ID, provider schema version and state-codec version;
- required and optional world capabilities;
- coordinate frame, quantization and supported bounds;
- chunk/tile addressing and stable world-feature IDs;
- allowed query and edit command families;
- authoritative budgets and overload behavior;
- persistence, replay, repair and migration support;
- required presentation and navigation adapters;
- compatibility vector included in save, replay and room identity.

Illustrative capabilities include:

- `SurfaceSample`
- `SolidPointQuery`
- `Raycast` and `Sweep`
- `MaterialQuery`
- `BoundedSurfaceEdit`
- `BoundedVolumeEdit`
- `DynamicSolidConnectivity`
- `StreamingChunks`
- `PersistentSparseDelta`
- `StructuralAnchors`

Unsupported capabilities fail module validation or return a deterministic
unsupported result. The runtime never silently approximates a volume edit as a
heightfield edit.

The selected authoritative provider is fixed for a loaded world instance.
Hot-swapping its canonical representation during a match is out of scope.
Later portal-linked worlds may use different providers only through explicit
world transitions and state boundaries.

## Shared deterministic envelope

All providers implement a minimal engine-facing envelope:

- canonical coordinate conversion and bounded numeric policy;
- point, surface, material, ray and sweep queries where declared;
- validated world-edit commands with stable command IDs;
- deterministic affected bounds and change notifications;
- stable chunk/tile/feature identity;
- canonical per-region and world-root hashes;
- versioned snapshots, ordered deltas and explicit migrations;
- replay, late-join, checkpoint and repair hooks;
- collision and navigation invalidation regions;
- resource accounting, cancellation and overload results.

The envelope does not expose Unity meshes, colliders, terrain objects, native
physics bodies, render textures or provider-owned mutable arrays.

Provider-specific commands and results remain provider-specific versioned data
inside the common command envelope. Sharing an envelope does not require one
universal edit operation.

## Proposed successor-island heightfield provider

If accepted after the Track S representation gate, the successor island's
heightfield provider owns:

- quantized surface samples or equivalent compact surface truth;
- water level and shore rules;
- protection, material and anchoring masks;
- ordered raise, lower, crater, smooth and related surface operations;
- surface interpolation, slope and collision queries;
- dirty surface regions and presentation rebuild notices;
- heightfield snapshots, deltas, hashes and repair.

MC2-style buildings may be grounded into, protected by or visually continuous
with this surface. That historical presentation must not force every future
structure or world into heightfield storage.

## Bounded-cell structure provider

Bounded destructible structures may coexist with heightfield terrain:

- the heightfield remains landscape truth;
- each selected structure owns a local cell/material/support domain;
- queries compose terrain and active structure solids deterministically;
- structure destruction invalidates only bounded collision/navigation regions;
- authoritative rubble is capped separately from cosmetic debris.

This path is useful for the first destruction bakeoff, but it is not a full
volumetric-world implementation.

## Volumetric voxel provider

The future voxel backend owns:

- sparse or bounded three-dimensional chunks;
- cell occupancy/density, material and damage state;
- deterministic cross-chunk edits and seam behavior;
- dirty-region scheduling, meshing and collision refresh;
- solid connectivity and bounded local structural hooks;
- chunk interest, streaming, snapshots, deltas, compaction and repair;
- explicit memory, edit-rate, rebuild, bandwidth and save budgets.

Its authoritative truth is the cell/volume state, not the generated mesh.
Greedy meshing, smooth meshing, dual contouring or another presentation may be
selected independently when it preserves the same authoritative state.

“Minecraft-like” describes the three-dimensional world model, not a mandatory
cube aesthetic. Original authored surfaces may hide or stylize the cells.

## Structural destruction is a separate capability

Voxels answer what material exists and what an impact removes. They do not by
themselves answer when a building should collapse.

A structural capability consumes stable cells, authored semantic elements or
support anchors and produces deterministic:

- support connectivity;
- unsupported component sets;
- bounded failure propagation;
- collapse-cluster IDs and schedules;
- handoff to authoritative rubble.

The initial bounded support graph is a game-physics approximation. A later city
module may add floors, columns, cores and load classes without replacing the
world provider.

Likewise, granular rubble flow is not the voxel backend. It is a separate
bounded aggregate simulation that receives collapsed mass and returns
gameplay-relevant occupancy, blockages and settled deltas.

## Streaming and persistence

World streaming operates on provider-neutral region identities and budgets,
then delegates payload interpretation to the selected provider.

Every streamed region has:

- immutable source-content identity;
- provider and schema identity;
- canonical base hash;
- bounded current delta;
- current region hash and world-root contribution;
- dependency-safe load, activation, suspension and eviction states;
- migration and unsupported-version behavior.

Heightfield chunks and voxel chunks may use entirely different codecs. Saves,
replays and rooms bind to the exact provider compatibility vector so a
similar-looking provider cannot join or load by accident.

## Networking

The server or authoritative host owns accepted world edits and canonical world
state. Networking transmits:

- validated edit commands or compact accepted results;
- region hashes and acknowledgements;
- interest-scoped snapshots and ordered deltas;
- correction and repair payloads;
- explicit overload, rejection and migration outcomes.

Generated meshes, small debris and cosmetic fracture are never authoritative.
A full voxel provider must prove late join, reconnect, interest entry/exit and
repair without streaming the entire world after each edit.

## Flight, actors and navigation

Carpet flight and projectiles consume representation-neutral ray, sweep,
material and occupancy queries. They must not read a height array or voxel
chunk directly.

Ground and structure actors consume bounded navigation-query and invalidation
services. The heightfield provider may use a surface graph; the voxel provider
may use layered or three-dimensional regions. A world edit reports affected
bounds, never demands a global navigation rebuild by default.

## Presentation

Presentation adapters consume immutable snapshots and ordered change notices.
They may use:

- Unity Terrain or custom tiled meshes for heightfields;
- authored intact/fracture meshes over bounded cells;
- greedy, smooth-cell or dual-contour meshes for voxel chunks;
- pooled rigid chunks, GPU instances, particles and settled rubble meshes.

Presentation quality may vary by browser/native tier. Canonical collision,
openings, material removal and settled gameplay obstacles may not.

## Preproduction obligations

Before `Simulation.Core` freezes its world boundary:

- specify the capability/provider manifest and compatibility vector;
- prove one tiny heightfield adapter and one tiny volumetric synthetic adapter
  can satisfy shared query/edit/hash/replay fixtures;
- prove unsupported operations fail explicitly;
- enforce assembly tests preventing provider internals from leaking into
  flight, spells, actors, networking, saves or presentation;
- keep the complete voxel implementation and city systems off the first
  successor demo critical path.

This is an interface proof, not permission to build the voxel world early.

## Later validation ladder

1. Synthetic `16³` or similarly bounded voxel world with deterministic edits.
2. Chunk seams, meshing, collision, snapshot, replay and repair.
3. High-speed carpet traversal and projectiles across streamed chunks.
4. Bounded structural failure and authoritative rubble handoff.
5. Multiplayer interest, late join, reconnect and sustained edit soak.
6. One destructible building and sloped rubble street.
7. One streamed destructible city block.
8. Evidence-based decision to proceed, limit or reject the large-city module.

No rung is promoted solely because a visual prototype looks impressive.
