# Blender MCP and deterministic asset-pipeline research

Status: Track S production-pipeline research. Track P original assets are not
inputs to this pipeline.

Snapshot: 2026-07-23.

## Recommendation

Use Blender for original asset production. Use MCP as an interactive authoring
accelerator, not as the authoritative build pipeline. The durable source of
truth should be a pinned Blender LTS patch, reviewed `.blend` sources,
versioned deterministic `bpy` recipes, headless export, machine-readable
validation, and Unity import tests while Unity remains the Track S baseline.

The initial bakeoff should compare plain Blender CLI against two MCP paths:

1. [`HoldMyBeer-gg/blend-ai`](https://github.com/HoldMyBeer-gg/blend-ai)
   (AGPL-3.0-or-later) for broad structured live authoring, validation, and
   Geometry Nodes.
2. [`sandraschi/blender-mcp`](https://github.com/sandraschi/blender-mcp)
   (MIT) for its claimed headless/CI workflow.
3. Plain headless Blender CLI as the mandatory reproducibility control.

[`PatrykIti/blender-ai-mcp`](https://github.com/PatrykIti/blender-ai-mcp)
(Apache-2.0) is a strong challenger for goal-first routing, measurement,
UV/baking, armature, and validation work.
[`ahujasid/blender-mcp`](https://github.com/ahujasid/blender-mcp) (MIT) is the
large community reference, but its most capable escape hatch is unrestricted
Blender Python and it includes optional telemetry/external asset and generation
integrations that are outside our clean original-asset path.

## Pipeline contract

```text
original brief and licensed references
  -> MCP-assisted exploration
  -> reviewed .blend source and procedural recipe
  -> pinned headless bpy build/export
  -> FBX + GLB + textures + metrics + thumbnail
  -> Unity importer and asset-contract validator
  -> Chrome WebGL scene/profile
```

Every accepted asset records generator/tool version, commit and seed; source/license provenance; triangle/vertex/material/texture/bone counts; scale/axis/pivot; LOD and collider status; turntable/contact sheet; and FBX/GLB import results.

Use FBX as the default Unity-native mesh/rig/animation artifact, GLB as the portable validation/future web artifact, separate textures, and Unity-owned URP materials. Keep `.blend` files outside Unity's imported `Assets` tree so editor and CI behavior does not depend on silently launching whatever Blender happens to be installed.

Blender's [production deployment guidance](https://docs.blender.org/manual/en/dev/advanced/deploying_blender.html) supports pinning one LTS version per production. Its [Cycles baking workflow](https://docs.blender.org/manual/ka/4.5/render/cycles/baking.html) covers the normal/AO/PBR and selected-to-active path needed for attractive WebGL assets. If direct GLB import is retained, pin and validate [glTFast](https://github.com/atteneder/glTFast), including its shader variants in Web builds.

## Security constraints

- Run Blender and MCP as the normal user, never with `sudo`.
- Bind bridges only to `127.0.0.1`.
- Pin package versions and commits; do not let `uvx` silently float.
- Disable `.blend` auto-execution, telemetry, external generators, Sketchfab/asset downloads, and arbitrary inline Python during structured-tool tests.
- Restrict file access to an isolated asset workspace and versioned output directory.
- Run headless jobs offline, fail nonzero on Python exceptions, and save into new versioned files.
- Treat Python blocklists as guardrails, not a sandbox. Blender documents the risks of embedded Python in its [scripting security guidance](https://docs.blender.org/manual/sr/4.0/advanced/scripting/security.html).

## Bakeoff assets and scorecard

- Stylized volcanic mana shrine/watchtower: modular masonry, procedural variation, three materials, UV0/UV2, AO/normal bake, LOD0/1/2, collision proxy, and destruction-marked pieces.
- Four-to-eight-bone fantasy bird: skinned mesh, idle/flight clips, two LODs, one atlas, FBX and GLB animation export.

Score time to first usable result, retries/manual repair, clean-scene reproducibility, topology/UV/pivot/export correctness, structured verification, token cost, headless suitability, crash/OOM behavior, WebGL download/load/draw/GPU-vertex cost, and how readily the winning procedure becomes a reviewed `bpy` recipe.
