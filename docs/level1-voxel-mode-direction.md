# Level 1, VoxelMode, and future structure direction

Date: 2026-07-26
Status: approved planning direction; implementation remains gated
Track: `track:shared`

## Decision

Finish and qualify the native Level 1 Unity experience first. Preserve that
result as an immutable baseline. Then add an explicit, owner-local
**VoxelMode** as a separate world and rendering path for one independently
authored, hollow pointy house. Do not layer voxel buildings over the qualified
native frame and do not make voxel work a new blocker for Level 1.

In parallel, Track S may continue its existing bounded-structure research with
wholly original assets and its own technical gates. The private Level 1
experiment and the successor experiment may learn the same general engineering
lessons, but they do not share code, assets, fixtures, owner-derived evidence,
identifiers, or blocking dependencies.

City-scale ingestion, photogrammetry, many building archetypes, street-scale
rubble, and water interaction remain later programs. They are not implied by a
successful single-house experiment.

## Why VoxelMode is separate

The qualified Level 1 renderer produces a complete native `320x200` indexed
frame. It does not currently expose recovered source depth for Unity to use when
placing 3D geometry into that frame. Level 1 building evidence also supplies
two-dimensional blocker extents and billboard expression, not recoverable
three-dimensional shells, rooms, or furniture.

VoxelMode therefore starts with these invariants:

- ordinary Level 1 and VoxelMode are mutually exclusive before scene assembly;
- the ordinary mode retains its existing renderer, captures, hashes, ABI,
  qualification corpus, collision ownership, and object invariants;
- VoxelMode uses a separate non-qualified 3D world and renderer;
- any VoxelMode depth is synthetic or derived for that mode only, never
  presented as recovered Level 1 depth;
- the client displays a persistent **synthetic, non-qualified VoxelMode**
  indicator;
- VoxelMode output never enters the existing Level 1 qualification corpus; and
- mode-off regression is a release gate for every VoxelMode integration change.

This gives the experiment room to become visually and physically ambitious
without quietly changing what “Level 1 complete” means.

## Program order

### Phase 0 — complete Level 1

`mc-dl0r.17.11.20` remains the P0 gate: clear the declared fidelity flags and
publish the final desktop qualification. No private VoxelMode implementation
bead is actionable before this closes.

The completed baseline records the owner cache, native plug-in, managed
runtime, build identity, qualification corpus, captures, mode-off invariants,
integration files, rollback procedure, and named one-writer checkout.

### Phase 1 — bounded original structure research

`mc-9224` is the existing Track S bounded-structure program. It remains below
Level 1 priority and stays bounded to structures over a heightfield; it does
not authorize full voxel terrain or city ingestion.

Two leaves make the structure idea concrete:

- `mc-72xz` authors one wholly original hollow pointy house, at least two
  deterministic room layouts, openings, stairs, lit-window references,
  material assignments, stable room and furniture IDs, and repeatable Blender
  exports. It extends the existing destruction-cell manifest.
- `mc-dl7p` qualifies that house and the surviving destruction candidate
  against the named `mc-c4p2` experiment profile after the existing Chrome
  preselection soak.

The named profile currently bounds the experiment to one structure no larger
than `32³` cells, ten simultaneous hits, a 10,000-event deterministic replay,
at most 3 ms P95 destruction work, at most 64 MiB peak added memory, at most
100 ms ordinary presentation rebuild, at most 8 KiB representative compressed
snapshot, safe overload behavior, and a clean 20-minute accumulation run.
Those numbers are owned by `mc-c4p2`; downstream work cites rather than copies
or silently changes them.

Track S authors must not inspect corresponding owner-game buildings, sprites,
screenshots, private captures, reconstructed source, or the private Level 1
adapter while producing this kit.

### Phase 2 — private owner-local VoxelMode

`mc-jnt0` is the Track P epic and is blocked by final Level 1 qualification.
Its children form this gated chain:

1. `mc-jnt0.1` freezes the qualified baseline and integration boundary.
2. `mc-jnt0.2` chooses the separate renderer and synthetic-depth policy.
3. `mc-jnt0.3` authors the private original house and freezes its structure
   contract.
4. `mc-jnt0.4` implements deterministic Rust structure authority.
5. `mc-jnt0.5` builds the separate Unity presentation.
6. `mc-jnt0.6` decides whether one named Level 1 location may receive a
   manufactured representation.
7. `mc-jnt0.7` accepts, limits, or rejects the experiment from evidence.

The private house is original and synthetic. It may evoke the broad category
of a compact pointy-roofed house, but it is not traced, redrawn, generated
from, or described as recovered or voxelized owner geometry. If the placement
gate accepts a particular Level 1 location, it creates a new implementation
slice and a separate qualification corpus; it never edits the completed Level
1 corpus or pass criteria.

### Phase 3 — better assets and more archetypes

Only an accepted or explicitly limited single-house experiment can propose:

- a small library of independently authored building envelopes;
- room-layout grammars constrained by each envelope and floor plan;
- reusable voxel furniture kits with stable semantic IDs;
- additional material profiles, damage responses, windows, doors, roofs, and
  stairs;
- exterior and interior LODs; and
- authoring-time validation and preview tools in Blender.

Each new archetype must remain deterministic under re-export, fit the same
structure contract, meet the accepted cell and runtime budgets, and carry
source and provenance records. Variety is a content-production milestone, not
permission to expand simulation scope.

### Phase 4 — future city vertical slices

`mc-w1r8` owns future streamed destructible worlds and a city vertical slice.
It is parked at P4. Dotonbori, Shinjuku, Bern old town, fictional districts,
and other recognizable places are candidate directions, not selected data
sources.

Photogrammetry is principally exterior surface evidence. It does not provide
clean semantic supports, watertight solids, room graphs, furniture layouts, or
destruction topology. A future licensed-city pipeline therefore needs distinct
steps:

1. prove the source licence, attribution, redistribution, derivative-work, and
   commercial-use terms for every input;
2. ingest georeferenced meshes, point clouds, footprints, heights, and semantic
   metadata without assuming that any one source is complete;
3. tile, clean, simplify, and assign stable building and parcel IDs;
4. derive or independently author semantic envelopes, roofs, facades, supports,
   openings, materials, and damage volumes;
5. generate standard interiors from envelope and floor-plan constraints rather
   than pretending photogrammetry contains them;
6. keep distant city geometry as ordinary streamed meshes and activate detailed
   destruction only in bounded local regions;
7. persist sparse authoritative damage deltas over immutable source tiles; and
8. prove one building, one block, and only then a broader streamed district.

A recognizable real district also needs a separate rights and identity review.
“Openly viewable,” “downloadable,” and “open source” are not interchangeable.

## Structure and destruction model

The prototype should model a semantic structure made of cells, not thousands
of independent Unity rigid bodies.

### Authoring contract

Blender owns original visual authoring and semantic export:

- stable structure, room, furniture, support, opening, and presentation IDs;
- declared units, origin, axes, cell scale, bounds, and material IDs;
- occupied cells and intentionally empty interior volume;
- foundation roots and load-transfer connections;
- immutable exterior and interior presentation references;
- deterministic recipe, schema, source, export, and asset hashes; and
- validation for gaps, overlaps, unsupported islands, thin features, ID drift,
  and budget excess.

The first house contains a hollow shell, roof, floors, openings, at least two
room layouts, limited furniture, and lit windows. The exporter should derive
runtime cells from authored semantic parts rather than requiring artists to
hand-place every cell.

### Runtime authority

Rust owns deterministic gameplay state:

- material, health, occupancy, support, and connection state;
- Fireball damage commands and deterministic affected-cell ordering;
- supported-component discovery and detached-component IDs;
- mass from cell volume and a versioned `density_kg_m3` material field;
- center of mass and coarse inertia for a detached component;
- bounded decisions to remain standing, shed a section, topple, or collapse;
- a small number of authoritative major rubble clusters;
- snapshots, sparse deltas, reset, replay hashes, and work caps; and
- conservative degradation when a cap is reached.

Unity owns presentation:

- meshes, lighting, lit windows, revealed rooms and furniture;
- bounded compound colliders and a small number of cluster bodies;
- pooled transient fragments, dust, sound, and cosmetic secondary breakup; and
- profiling and visible overload diagnostics.

Unity does not decide whether a support failed, which cells were damaged, or
which authoritative component detached.

### Collapse and material mass

Each connected component computes mass and center of mass from occupied cell
volume and its versioned material density. A support test determines whether
the center-of-mass projection and remaining support connections are stable.
When a component detaches, it becomes one bounded compound body or an
authoritative coarse cluster, not one body per voxel. Impact energy may split
it into a strictly capped number of additional clusters while cosmetic debris
remains client-local.

Real-world density ranges can seed profiles, but gameplay density, strength,
damage threshold, fracture behavior, and apparent rubble volume are separate
versioned parameters. No material number becomes a contract until the profile
records its source, unit, calibration fixture, and expected gameplay response.

### Ground and water

Ground contact initially uses bounded cluster settling, sleep, cleanup, and
optional merge-to-rubble rules. A successful house prototype does not require
granular terrain flow.

Water is deferred. A later bounded experiment may use effective cluster
density, displaced volume, drag, buoyancy, sinking, and bed-settling
approximations. It should not create a live fluid simulation or a rigid body for
every brick. Street-scale rubble and water handoff belong to `mc-w1r8.5`.

## Evidence and kill gates

The experiment advances only with reproducible evidence:

- exact build, Unity, Rust, Blender, exporter, runtime, browser where relevant,
  and hardware identities;
- source, recipe, schema, asset, fixture, operation-stream, snapshot, replay,
  capture, and log hashes;
- deterministic re-export and deterministic authoritative replay;
- local wall opening that reveals the intended room and furniture;
- repeated Fireball impacts, partial support, detachment, topple decision, cap
  pressure, reset, cleanup, and 20-minute accumulation;
- first divergent tick or first exceeded budget with a minimal reproduction;
- mode-off Level 1 regression with no changed qualified artifacts; and
- recorded ACCEPT, LIMIT, or REJECT decisions.

Stop, simplify, or reject when semantic IDs drift, the authored house cannot
fit the profile, support or mass is nondeterministic, cleanup accumulates,
performance exceeds an unwaived budget, the Unity client becomes authoritative,
the two modes cannot remain exclusive, or provenance is unclear.

## Parallel-work rule

Level 1 work continues in its current private checkout and remains the program
priority. Track S original-asset and bounded-structure work may proceed through
its own gates in the separate successor repository and checkout. Only one
writer owns a checkout at a time.

The two programs are joined later by reviewed general lessons and documented
interfaces, not by merging their implementations. The private deterministic
VoxelMode authority is intentionally independent from the Track S candidate;
unifying or extracting either implementation requires a new boundary and
architecture decision.

## Bead graph and validation record

- Level 1 gate: `mc-dl0r.17.11.20` at P0.
- Track S bounded program: `mc-9224` at P2, with new direct children
  `mc-72xz` and `mc-dl7p`; branch selection remains `mc-szx2`.
- Track P private experiment: `mc-jnt0` at P2 with children
  `mc-jnt0.1` through `mc-jnt0.7`; both the epic and baseline gate are blocked
  by final Level 1 qualification.
- Future city program: `mc-w1r8` parked at P4.
- No direct Track P/Track S blocking or parent edges were added.
- `br ready --json` returned none of the new experiment beads as actionable.
- `bv --robot-insights` reported `cycles: []` with data hash
  `1651fec3ff50081a` before this document was registered.
- After registration, scoped `bv --robot-insights` passes for both
  `track:preservation` and `track:successor` again reported zero cycles; a
  direct database check reported zero Track P/Track S dependency edges.
- Standalone `br dep cycles` exceeded the practical interactive bound and was
  stopped; this is a known tool-performance limitation, not positive cycle
  evidence.

## Related records

- [`program-charter.md`](program-charter.md)
- [`product-promise.md`](product-promise.md)
- [`preservation-track.md`](preservation-track.md)
- [`stack-decision.md`](stack-decision.md)
- [`provenance-policy.md`](provenance-policy.md)
- [`private-public-boundary.md`](private-public-boundary.md)
- [`world-representation-backends.md`](world-representation-backends.md)
- [`future-destructible-cities.md`](future-destructible-cities.md)
- [`../research/voxel-destruction-option.md`](../research/voxel-destruction-option.md)
