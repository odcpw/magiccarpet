# Private BUILD-29 voxel import (editor-only)

This folder is the Unity presentation-lab bridge for the qualified private
BUILD-29 voxel package. The package is **not copied into this Unity project**:
the importer takes an absolute external owner root, validates the contract and
provenance receipt, then writes a deterministic mapping under
`Assets/Editor/VoxelMode/Generated/`.

The default package paths are:

```
contract/voxel-house-manifest-v1.schema.json
exports/build29/voxel-house-manifest-v1.json
exports/build29/surface-recipe-v1.json
exports/build29/provenance-receipt.json
exports/build29/qualification-report.json
```

The Unity editor menu has two actions:

* `MC2 Private/Voxel Mode/Validate qualified BUILD-29 package` checks the
  schema, raw-file and semantic hashes, source anchor, provenance boundary,
  topology, entity references, input-size/array/budget caps, and receipt.
* `MC2 Private/Voxel Mode/Import qualified BUILD-29 package` performs the same
  checks and emits `voxel-house-build29.mapping.json` plus a hash receipt.

The mapping is presentation data only. `cells[]` remain serialized records;
they are not Unity `GameObject`s. A later mesh adapter should build batched
surfaces from `parts[]`/`render_groups[]`, while the source house AABB remains
the gameplay collision/targeting authority. The mapping records the
`source_aabb_center_bottom` anchor and the explicit `SourceToUnity at runtime`
conversion so visual bounds cannot silently change gameplay placement.

Owner-derived output is blocked from any `Assets/Resources`,
`Assets/StreamingAssets`, `Assets/OwnerData`, or `Assets/OwnerAssets` path.
The `Generated/` directory is ignored so private mapping output never becomes
tracked Track-S content. Do not move this importer into `Assets/Scripts` or
call it from runtime/bootstrap code.

## `.5.5` state fixture

`VoxelHouseRenderFixture.cs` is the reviewable editor-side surface adapter
fixture for the next bead. It consumes the generated mapping and emits
deterministic render-group batches for five presentation states:

* `Intact` keeps all occupied cells and both BUILD-29 surface batches.
* `Opposite` keeps the same geometry while recording the opposite camera side.
* `NightWindow` keeps geometry and exposes the two window-opening light IDs.
* `Cutaway` hides the roof batch so the six rooms and six furniture records are
  exposed without adding cell objects.
* `Breach` removes a deterministic one-cell shell neighbourhood around the
  first opening, preserving the rooms/furniture metadata.

Each state records visible cell IDs, hidden parts, batches, room/furniture
metadata, lit windows, zero per-cell scene objects, and budget counters. The
fixture's reset/replay trace must hash identically twice. The editor menu action
`MC2 Private/Voxel Mode/Build deterministic BUILD-29 state fixture report`
writes the ignored `Generated/voxel-house-build29.fixture-report.json`.

This is intentionally not the final Unity mesh/prefab renderer. The remaining
Unity gate is to resolve the pinned URP/test packages and review a real scene
render; the pure fixture and Roslyn compile validate the state contract without
claiming that visual gate is complete.
