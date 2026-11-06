using System;
using System.Collections.Generic;
using System.Linq;
using UnityEngine;
using UnityEngine.Rendering;

namespace MagicCarpet.Native
{
    // Original animated actors remain visible until individual modern model bindings are authored.
    // Rust supplies selection and visibility, including invisibility and recycled entity slots.
    internal sealed class ModernEntityPresenter : IDisposable
    {
        private readonly Transform root;
        private readonly OwnerAssetCache cache;
        private readonly Dictionary<int, OwnerAssetCache.Level3Texture> assets;
        private readonly Dictionary<int, Material> materials = new();
        private readonly List<Texture2D> textures = new();
        private readonly Dictionary<ushort, GameObject> objects = new();
        private readonly Dictionary<ushort, NativeSim.CampaignEntityPresentation> rows = new();
        private readonly byte[] palette;
        private readonly Mesh quad;
        private readonly Shader shader;
        private readonly Shader smokeShader;
        private bool loggedSmoke;
        private bool loggedFamilies;
        private bool loggedGeometry;
        private ushort? isolatedSmoke;
        private int loggedDebug = -1;
        internal IEnumerable<NativeSim.CampaignEntityPresentation> VisibleRows => rows.Values;
        private readonly MaterialPropertyBlock emission = new MaterialPropertyBlock();

        internal ModernEntityPresenter(Transform parent, OwnerAssetCache cache)
        {
            this.cache = cache;
            if (cache.EnvironmentMaterials == null)
                throw new InvalidOperationException("Modern entities require the validated environment asset bank");
            assets = cache.EnvironmentMaterials.tmaps.textures.Where(row => !row.source_empty).ToDictionary(row => row.index);
            palette = cache.ReadOutput(cache.EnvironmentMaterials.palette.rgb8_path);
            shader = Resources.Load<Shader>("ModernEntity");
            smokeShader = Resources.Load<Shader>("ModernSmoke");
            if (shader == null) throw new InvalidOperationException("Missing ModernEntity shader");
            root = new GameObject("Live campaign actors").transform;
            root.SetParent(parent, false);
            quad = new Mesh { name = "Modern actor billboard" };
            quad.vertices = new[] { new Vector3(-.5f, 0, 0), new Vector3(.5f, 0, 0), new Vector3(-.5f, 1, 0), new Vector3(.5f, 1, 0) };
            quad.uv = new[] { new Vector2(0, 1), new Vector2(1, 1), new Vector2(0, 0), new Vector2(1, 0) };
            quad.triangles = new[] { 0, 2, 1, 2, 3, 1 };
            quad.RecalculateNormals();
            quad.RecalculateBounds();
        }

        internal void Present(NativeCampaignSession session, short yaw)
        {
            rows.Clear();
            foreach (var row in session.EntityPresentations(yaw))
            {
                if (!assets.TryGetValue(row.selectedTmap, out var texture))
                    throw new InvalidOperationException($"Live entity {row.entityIndex} has absent sprite {row.selectedTmap}");
                rows.Add(row.entityIndex, row);
                if (!objects.TryGetValue(row.entityIndex, out var node))
                {
                    node = new GameObject($"Actor {row.entityIndex}");
                    node.transform.SetParent(root, false);
                    node.AddComponent<MeshFilter>().sharedMesh = quad;
                    var renderer = node.AddComponent<MeshRenderer>();
                    renderer.shadowCastingMode = ShadowCastingMode.TwoSided;
                    objects.Add(row.entityIndex, node);
                }
                // These source TMAPs are blend-table operands, not opaque RGB artwork.
                var smoke = (row.baseTmap == 57 || row.baseTmap == 63) && (row.pixelMode == 2 || row.pixelMode == 6);
                var visual = node.GetComponent<MeshRenderer>();
                visual.sharedMaterial = GetMaterial(texture, smoke);
                emission.Clear();
                var lightKind = ModernNightLighting.SourceKind(row);
                emission.SetFloat("_EmissionStrength", lightKind >= 1 && lightKind <= 4 ? 2.5f : 0);
                visual.SetPropertyBlock(emission);
                visual.shadowCastingMode = smoke ? ShadowCastingMode.Off : ShadowCastingMode.TwoSided;
                if (smoke && !loggedSmoke)
                {
                    var actor = session.Entities().First(entity => entity.entityIndex == row.entityIndex);
                    Debug.Log($"MC_MODERN_SMOKE slot={row.entityIndex} source={row.sourceId} class={actor.classId} model={actor.model} parameter={row.renderParameterIndex} base={row.baseTmap} selected={row.selectedTmap} height={row.logicalHeight} pixel_mode={row.pixelMode}");
                    loggedSmoke = true;
                }
                var height = row.logicalHeight / 64f;
                node.transform.localScale = new Vector3(height * texture.width / texture.height * (row.mirrored == 0 ? 1 : -1), height, 1);
            }
            foreach (var key in objects.Keys.Where(key => !rows.ContainsKey(key)).ToArray())
            {
                UnityEngine.Object.Destroy(objects[key]);
                objects.Remove(key);
            }
            if (ModernCapture.VisualDebug && !loggedFamilies)
            {
                Debug.Log("MC_MODERN_ENTITY_FAMILIES " + string.Join(";", rows.Values
                    .GroupBy(row => (row.baseTmap, row.selectedTmap, row.pixelMode))
                    .Select(group => $"base={group.Key.baseTmap},selected={group.Key.selectedTmap},mode={group.Key.pixelMode},count={group.Count()}")));
                loggedFamilies = true;
            }
        }

        internal void Render(Camera camera)
        {
            var position = camera.transform.position;
            if (!ModernCapture.IsolateSmoke) isolatedSmoke = null;
            var logDiagnostic = ModernCapture.VisualDebug && loggedDebug != ModernCapture.SmokeDebug;
            foreach (var pair in rows)
            {
                var row = pair.Value;
                var x = position.x + Mathf.Repeat(row.x / 64f - position.x + 512f, 1024f) - 512f;
                var z = position.z + Mathf.Repeat(row.y / 64f - position.z + 512f, 1024f) - 512f;
                var node = objects[pair.Key];
                var smoke = (row.baseTmap == 57 || row.baseTmap == 63) && (row.pixelMode == 2 || row.pixelMode == 6);
                node.transform.SetPositionAndRotation(new Vector3(x, row.z / 64f, z),
                    Quaternion.Euler(0, camera.transform.eulerAngles.y, 0));
                var visual = node.GetComponent<MeshRenderer>();
                if (ModernCapture.IsolateSmoke && smoke && isolatedSmoke == null)
                {
                    var screen = camera.WorldToViewportPoint(node.transform.TransformPoint(new Vector3(0, .5f, 0)));
                    if (screen.z > camera.nearClipPlane && screen.x > 0 && screen.x < 1 && screen.y > 0 && screen.y < 1)
                    {
                        isolatedSmoke = row.entityIndex;
                        Debug.Log($"MC_MODERN_SMOKE_ISOLATED slot={isolatedSmoke} position={node.transform.position} size={node.transform.localScale} screen={screen} shader={visual.sharedMaterial.shader.name}");
                    }
                }
                visual.enabled = ModernCapture.EntityVisibility != 1 && !(ModernCapture.EntityVisibility == 2 && smoke) &&
                    !(ModernCapture.IsolateSmoke && smoke && row.entityIndex != isolatedSmoke);
                if (logDiagnostic && smoke)
                    Debug.Log($"MC_MODERN_SMOKE_DIAGNOSTIC slot={row.entityIndex} position={node.transform.position} camera={camera.transform.InverseTransformPoint(node.transform.position)} size={node.transform.localScale} shader={visual.sharedMaterial.shader.name} enabled={visual.enabled}");
                if (ModernCapture.VisualDebug && !loggedGeometry && smoke)
                    Debug.Log($"MC_MODERN_SMOKE_GEOMETRY slot={row.entityIndex} position={node.transform.position} camera={camera.transform.InverseTransformPoint(node.transform.position)} size={node.transform.localScale} mode={row.pixelMode} tmap={row.selectedTmap}");
            }
            loggedGeometry = true;
            loggedDebug = ModernCapture.SmokeDebug;
        }

        private Material GetMaterial(OwnerAssetCache.Level3Texture row, bool smoke)
        {
            var key = row.index + (smoke ? 1000 : 0);
            if (materials.TryGetValue(key, out var material)) return material;
            if (smoke)
            {
                if (smokeShader == null) throw new InvalidOperationException("Missing ModernSmoke shader");
                material = new Material(smokeShader);
                material.SetFloat("_Seed", row.index * 1.37f);
                material.SetFloat("_Opacity", .24f);
                materials.Add(key, material);
                return material;
            }
            var indices = cache.ReadOutput(row.indices_path);
            var pixels = new Color32[indices.Length];
            for (var i = 0; i < pixels.Length; i++)
            {
                var c = indices[i];
                pixels[i] = new Color32(palette[c * 3], palette[c * 3 + 1], palette[c * 3 + 2], c == 0 ? (byte)0 : (byte)255);
            }
            var texture = new Texture2D(row.width, row.height, TextureFormat.RGBA32, true) {
                name = $"Live sprite {row.index}", filterMode = FilterMode.Bilinear, wrapMode = TextureWrapMode.Clamp
            };
            texture.SetPixels32(pixels);
            texture.Apply(true, true);
            material = new Material(shader);
            material.SetTexture("_BaseMap", texture);
            materials.Add(key, material);
            textures.Add(texture);
            return material;
        }

        public void Dispose()
        {
            UnityEngine.Object.Destroy(root.gameObject);
            UnityEngine.Object.Destroy(quad);
            foreach (var value in materials.Values) UnityEngine.Object.Destroy(value);
            foreach (var value in textures) UnityEngine.Object.Destroy(value);
        }
    }
}
