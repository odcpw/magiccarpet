# Voxel and persistent-destruction option

Status: Track S research; Track P may supply historical observations but no
reconstructed implementation.

## Recommendation

Keep voxels, but do not make the first demo a fully voxelized world.

The strongest first architecture is:

- deterministic 16-bit heightfield for landscape truth;
- bounded voxel/cell-grid volumes for selected structures;
- a small capped set of authoritative rubble clusters;
- many short-lived client-local fragments, sparks, dust, scorch, and sound;
- authored portal/cave volumes initially;
- Blender-authored fracture prefabs behind the same destruction contract as a fallback.

The cell grid can remain a hidden simulation representation. The visible house can be an attractive Blender-authored stucco, stone, timber, dome, or mosaic structure that breaks into irregular chunks rather than cubes.

## Why full voxels are a separate backend program

For an illustrative `1024 × 1024 × 256` world:

| Canonical representation | Raw state before metadata/meshes/collision |
|---|---:|
| UInt16 heightfield | 2 MiB |
| One-bit voxel occupancy | 32 MiB |
| One-byte voxel material | 256 MiB |
| Four-byte density/material | 1 GiB |

Unity Web currently combines browser-dependent heap/asset/GC pressure with a normally single-threaded C# runtime, so native Jobs/Burst claims do not establish Chrome performance. See Unity's [Web technical limitations](https://docs.unity3d.com/6000.0/Documentation/Manual/webgl-technical-overview.html) and [Web memory model](https://docs.unity3d.com/6000.0/Documentation/Manual/webgl-memory.html).

## Authority and persistence classes

```text
TerrainTruth
  quantized heightfield + masks + ordered ops + chunk hashes

StructureTruth
  local cell grid + material/health/support + missing-cell bitset

RubbleTruth
  capped gameplay-relevant clusters + quantized resting pose

ClientPresentation
  authored intact mesh + dirty replacement/fracture islands
  pooled local fragments + dust/embers/scorch/audio
```

Persistence classes:

1. Gameplay persistent: missing walls, exposed rooms, blocked openings, coarse rubble obstacles.
2. Match-persistent presentation: reconstructable settled rubble mesh, scorch, decals.
3. Transient: tiny shards, splinters, sparks, smoke, dust; never networked or saved.

Replicate impact causes and compact canonical results, never every flying fragment. Late join receives map/asset manifest hash, terrain checkpoint, per-structure missing-cell state, capped rubble clusters, and ordered operations after the checkpoint.

AI and collision should query the heightfield plus local occupancy/obstacle grids. A destroyed wall invalidates only intersecting cost cells/region edges. No Fireball should trigger a global NavMesh rebuild, and ordinary debris must not require hundreds of rigid bodies or dynamic concave colliders.

## Proof budgets and kill criteria

The recommended spike is one `32³` structure, ten simultaneous hits, bounded support collapse, and a 10,000-event replay.

Promote localized voxel destruction only if:

- deterministic hashes match across repeated native/server runs;
- Chrome main-thread destruction is amortized to at most 3 ms P95 with no repeated 50 ms frames;
- ordinary presentation rebuild completes within 100 ms;
- the subsystem adds at most 64 MiB peak memory in the stress scene;
- a representative compressed structure snapshot is at most 8 KiB;
- destruction traffic stays around or below 2 KiB/s/player outside checkpoints;
- demo-level late-join destruction is at most 512 KiB and reconstructs within one second;
- a destroyed wall/bridge reroutes ground actors within 250 ms without global rebuild;
- the 20-minute soak shows no growing mesh/collider/fragment/op-log pools.

Do not promote full voxel terrain into the demo if its time-boxed laboratory needs compute shaders or multithreaded C#, adds over 128 MiB working memory, exceeds 6 ms P95 dirty mesh/collision work or repeatedly spikes over 25 ms, needs over 2 MiB for the first-level checkpoint, couples AI to global remeshing, loses the rolling-distance beauty of the heightfield, or consumes comparable art effort merely hiding its voxel character.

## Parallel-provider architecture

Full voxel terrain must not replace the successor island heightfield or masquerade
as an extension of its storage. The reusable engine exposes a deterministic
world capability/query/edit/hash envelope with parallel providers:

- the successor island world selects the deformable heightfield provider;
- bounded destructible structures may layer local cell volumes over it;
- a later voxel-world or destructible-city module selects a volumetric chunk
  provider with different storage, meshing, collision, streaming, navigation
  and persistence algorithms.

Only the representation-neutral seam and synthetic conformance fixture belong
in preproduction. Full voxel implementation remains post-social-demo and must pass the
feasibility ladder in
[`docs/world-representation-backends.md`](../docs/world-representation-backends.md).

## Visual bakeoff

Build the same small shrine/house three ways and hit it with the same Fireball:

1. Explicit jewel-toned stylized voxels.
2. Hidden cell-grid destruction under smooth/low-poly Blender-authored surfaces.
3. Blender-authored fracture islands.

Judge all three at carpet speed, low-altitude flyby, storm lighting, and close post-destruction inspection. The expected first-demo winner is hidden cell-grid destruction with authored surfaces, but the graph preserves the evidence gate rather than assuming it.
