# Future vision: streamed, destructible cities

Status: Track S city vertical-slice direction after the island social demo  
Date: 2026-07-23  
Depends on: the successor stack gate, authoritative multiplayer proof, world
streaming seam and provenance-approved city inputs

## Vision

Use the successor carpet-flight engine for an original world in which players
fly through recognizable cities, fight along streets and between towers, remove
building material with spells or weapons, trigger structurally plausible
collapses, and leave persistent rubble that pours downhill, blocks roads and
spills into rivers or bays.

Candidate worlds include Tokyo, New York, San Francisco and London, subject to
source-data rights, technical budgets and an appropriate fictional or
non-commercial treatment. Entirely fictional cities must remain equally
possible.

This is the planned second Track S vertical slice, not a requirement for the
first browser social demo and not dependent on Track P campaign completion.

## Feasibility boundary

A city-scale experience that *appears* comprehensively destructible is
feasible. A literal city-wide simulation in which every real brick is always a
live voxel or rigid body is not a useful target for a browser or ordinary
consumer machine.

The viable design is multiscale:

- stream distant city geometry as ordinary hierarchical meshes;
- activate detailed structural and volumetric representations only near
  relevant players, impacts and collapses;
- simulate a bounded number of important chunks;
- represent the remaining rubble as instanced debris, particles, aggregate
  fields and settled meshes;
- persist sparse destruction deltas rather than a second complete copy of the
  city.

The illusion should be continuous even though the representation changes with
distance, relevance and motion.

## Target experience

A successful future slice should let players:

- fly at rooftop height, between towers and down street canyons;
- strike a facade, floor, bridge, column or tower with a fireball;
- see material removed at the impact rather than only swapping the whole
  building to a damaged model;
- create tactical openings and new flight paths;
- progressively weaken structural supports;
- trigger a delayed, legible collapse when remaining load paths fail;
- dodge major falling sections while dust and smaller debris fill the street;
- watch rubble travel downhill, accumulate against obstacles and enter water;
- return later and find the important destruction state still present;
- experience the same gameplay-relevant result in multiplayer.

## World ingestion and streaming

### Approved inputs

Potential inputs include:

- OpenStreetMap roads, footprints and semantic metadata;
- openly licensed municipal building, terrain and LiDAR datasets;
- appropriately licensed photogrammetry, BIM, glTF or 3D Tiles;
- project-authored procedural or hand-built geometry.

The import pipeline must record source, license, attribution, coordinate
reference system, units, acquisition date and transformation history. It must
produce stable building and infrastructure IDs so destruction deltas survive
content rebuilds.

OpenStreetMap data is ODbL-licensed and requires attribution and appropriate
handling of adapted databases. Google, Apple and other proprietary map or
photogrammetry assets must not be copied or scraped without explicit rights.

### Runtime representation

Use hierarchical geospatial tiles with:

- world, district, block, building and local-cell bounds;
- screen-space and gameplay-relevance refinement;
- independent visual, collision and destruction payloads;
- per-feature semantic metadata;
- background streaming, bounded caches and deterministic eviction;
- rebasing or high-precision coordinates for large cities.

[3D Tiles](https://github.com/CesiumGS/3d-tiles) is a useful reference for
streaming massive heterogeneous geospatial content; adopting it directly is a
later implementation decision.

## Destructible-building model

Each building may progress through several representations:

1. **Distant static representation** — batched mesh and coarse collision.
2. **Nearby semantic representation** — facade, floors, columns, cores, beams,
   walls, roof and foundation with material and support metadata.
3. **Localized damage volume** — voxel, sparse voxel, SDF or cell representation
   around active impacts.
4. **Structural failure representation** — disconnected components and major
   fracture bodies.
5. **Collapse representation** — important rigid chunks plus aggregate debris.
6. **Settled representation** — static rubble geometry, collision and compact
   persistent damage data.

Transitioning between representations must conserve the important mass,
occupied volume, openings, support state and gameplay collision. Presentation
may be approximate; authoritative navigation and damage state may not be.

## Damage and structural failure

Voxel removal alone does not determine when a building should collapse. A
separate structural model must represent:

- support nodes and load-bearing connections;
- slabs, beams, columns, walls, cores and foundations;
- material capacity, damage, fracture and fire/heat state;
- vertical and lateral load paths;
- disconnected or insufficiently supported components;
- bounded local stress or stability propagation.

An impact changes the local damage volume and structural graph. The structural
solver then determines whether the building remains stable, sheds a component,
progressively buckles or enters a major collapse.

The target is convincing, deterministic game physics—not a certified civil
engineering simulation. Structural rules must be explainable, tuneable and
testable so players learn why a collapse occurred.

## Collapse and rubble

One collapse uses several simultaneous fidelity layers:

- a small set of gameplay-relevant structural bodies with authoritative
  collision;
- medium fracture chunks simulated near players;
- GPU-instanced cosmetic fragments;
- dust, sparks, glass, fire, smoke and sound fields;
- an aggregate rubble field that conserves approximate volume;
- a settled rubble mesh generated when motion becomes insignificant.

Rubble movement can use a terrain-aware granular heightfield, cellular
settling model or another bounded aggregate solver. It should:

- flow downhill and along street channels;
- pile against buildings, walls and vehicles;
- create ramps, blockages and cover;
- transition cleanly into static collision;
- interact approximately with wind and water;
- avoid representing every brick as a networked rigid body.

Water interaction may combine a few real floating or sinking chunks with an
aggregate displacement/obstruction field, splashes, waves, turbidity and
sediment effects.

## Persistence

Store the original immutable city tile plus sparse deltas:

- removed or transformed material cells;
- damaged structural elements;
- collapsed-building phase;
- authoritative major remnants;
- settled rubble volume and collision;
- scorch, fire and other gameplay-relevant state;
- version and migration metadata.

Streaming a damaged tile reconstructs it from its source content and validated
delta. If an imported city revision changes stable IDs or geometry
incompatibly, migration must be explicit rather than guessed.

## Multiplayer model

The authoritative host or server owns:

- validated impact events;
- material-removal and structural-state deltas;
- stability and collapse decisions;
- major gameplay collision bodies;
- rubble-field checkpoints;
- persistent aftermath.

Clients receive compact state transitions such as impact, fracture region,
collapse seed, major component state and aggregate-rubble checkpoints. They
generate small debris, dust and other presentation locally.

The protocol must never stream every brick transform. Deterministic seeds help
clients agree visually, but seeds do not replace authoritative checksums and
corrections for gameplay-relevant destruction.

## Browser and native envelopes

The first city experiment should target Chrome with one destructible building
or a small number of blocks. Browser quality must be budget-driven:

- bounded active-destruction radius;
- fixed structural and granular work per tick;
- strict memory and download budgets;
- pooled chunks and particles;
- progressive visual/collision refinement;
- graceful quality levels;
- no assumption that unrestricted C# threading or native sockets exist.

Unity documents material Web-platform limitations around managed threading,
memory, networking and platform APIs. Those constraints must be measured on the
actual selected Unity version before approving a city milestone:
[Unity Web technical limitations](https://docs.unity3d.com/6000.0/Documentation/Manual/webgl-technical-overview.html).

A native build may later enable larger active areas, denser fracture, more
rigid bodies and more advanced structural or granular computation. Browser and
native builds must share authoritative laws even when their cosmetic fidelity
differs.

## Engine seams worth preserving now

The MC2 project should not implement city destruction prematurely. It should
avoid architectural choices that make a later module impossible.

The world-model decision is a parallel-provider architecture, specified in
[`world-representation-backends.md`](world-representation-backends.md):

- MC2 uses a deformable 2.5D heightfield provider;
- selected structures may add bounded local destruction volumes;
- a future Minecraft-like world uses a distinct volumetric chunk provider;
- shared engine contracts cover capabilities, queries, edits, hashes, replay,
  saves and networking without sharing internal storage or meshing.

Potential capability boundaries include:

- `IWorldTileSource` for streamed spatial content;
- `IDamageableVolume` for bounded material queries and edits;
- `IStructuralModel` for support and failure decisions;
- `IDebrisField` for aggregate rubble and settling;
- `IWorldDeltaStore` for persistent sparse changes;
- presentation adapters for fracture, particles, dust, audio and water.

These names are illustrative, not frozen APIs. A capability enters the core
engine only after at least two game modules or a synthetic conformance module
prove that it is genuinely reusable. The MC2 engine must not acquire dormant
city-specific code merely to anticipate this vision.

## Validation ladder

1. **Material-removal lab** — fireball removes bounded material from one
   synthetic wall with deterministic replay.
2. **Structural lab** — one small synthetic tower remains stable under
   non-critical damage and collapses after demonstrated support failure.
3. **Rubble street** — collapse volume travels down a sloped street, settles
   and reconstructs from a saved delta.
4. **Browser block** — fly and fight around one to four streamed blocks while
   meeting measured frame, memory and download budgets.
5. **Network collapse** — two or more clients agree on openings, major
   collision and settled rubble without per-fragment replication.
6. **Licensed city district** — ingest a recognizable, provenance-complete
   district and preserve attribution and stable IDs.
7. **Large-city illusion** — stream a broad city while detailed physics remains
   bounded around active play.

Each rung requires measured acceptance before the next expands world size or
physics fidelity.

## Explicit non-goals

- City-wide brick-resolution simulation.
- Exact engineering prediction of real building failure.
- Networking every fragment.
- Scraping proprietary city imagery or models.
- Blocking the first successor demo or 1.0 work on speculative city
  capabilities.
- Promising browser/native parity in cosmetic debris density.
- Treating a beautiful collapse as proof of authoritative determinism,
  persistence or multiplayer correctness.

## Open decisions for the future

- Voxel, sparse voxel, SDF, tetrahedral or hybrid local damage representation.
- Structural graph, constraint, FEM-inspired or learned approximation.
- Rigid-body, MPM/PIC, cellular or heightfield aggregate rubble solver.
- Unity implementation, external deterministic kernel or hybrid acceleration.
- Real-city fidelity versus fictionalized city layouts.
- Destruction persistence lifetime and world-reset rules.
- Maximum active players, active destruction zones and simultaneous collapses.
- Rating, safety, cultural and disaster-sensitivity treatment for recognizable
  real cities.
