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
