using System;
using System.Collections.Generic;
using UnityEngine;
using UnityEngine.Rendering;

namespace MagicCarpet.Native
{
    // Source class 9/model 10's visible TMAP 61 is a floating human head.
    // Native rows alone own its lifetime, position and apparent height.
    internal sealed class ModernCastleProjectile : IDisposable
    {
        private sealed class Visual
        {
            internal GameObject node;
            internal Renderer[] renderers;
            internal NativeSim.CampaignEntityPresentation row;
        }

        private readonly Transform parent;
        private readonly GameObject asset;
        private readonly Material face, wrap, beard, sclera, pupils;
        private readonly Dictionary<ushort, Visual> active = new();
        private readonly Stack<Visual> pool = new();
        private readonly HashSet<ushort> seen = new();
        private readonly List<ushort> removed = new();
        private bool disposed;
        internal int ActiveCount => active.Count;
        internal int PoolCount => pool.Count;

        internal ModernCastleProjectile(Transform parent, bool voxel)
        {
            this.parent = parent;
            asset = Resources.Load<GameObject>(voxel
                ? "ModernCharacters/CastleProjectile/CastleProjectileVoxel"
                : "ModernCharacters/CastleProjectile/CastleProjectile");
            var shader = Shader.Find("Universal Render Pipeline/Lit");
            if (asset == null || shader == null) { asset = null; return; }
            face = CreateMaterial(shader, "Castle projectile face", new Color(.53f, .32f, .22f));
            wrap = CreateMaterial(shader, "Castle projectile teal wrap", new Color(.09f, .28f, .26f));
            var fabric = Resources.Load<Texture2D>("ModernCharacters/CastleProjectile/CastleProjectileFabric");
            if (!voxel && fabric != null)
            {
                wrap.SetTexture("_BaseMap", fabric);
                wrap.SetTextureScale("_BaseMap", new Vector2(4, 2));
            }
            beard = CreateMaterial(shader, "Castle projectile beard", new Color(.075f, .032f, .018f));
            sclera = CreateMaterial(shader, "Castle projectile sclera", new Color(.72f, .68f, .58f));
            pupils = CreateMaterial(shader, "Castle projectile pupils", new Color(.045f, .025f, .015f));
        }

        private static Material CreateMaterial(Shader shader, string name, Color tint)
        {
            var material = new Material(shader) { name = name, enableInstancing = true };
            material.SetColor("_BaseColor", tint);
            material.SetFloat("_Smoothness", .2f);
            return material;
        }

        internal static bool Matches(NativeSim.CampaignEntitySnapshot entity, NativeSim.CampaignEntityPresentation row) =>
            entity.active != 0 && entity.classId == 9 && entity.model == 10 && entity.action == 10 &&
            entity.entityIndex == row.entityIndex && entity.sourceId == row.sourceId &&
            (row.controlFlags & 1) == 0 &&
            row.renderParameterIndex == 18 && row.baseTmap == 61 && row.selectedTmap == 61 &&
            row.renderMode == 21 && row.logicalHeight > 0;

        internal void Present(IReadOnlyDictionary<ushort, NativeSim.CampaignEntitySnapshot> entities,
            IReadOnlyList<NativeSim.CampaignEntityPresentation> rows)
        {
            if (disposed) throw new ObjectDisposedException(nameof(ModernCastleProjectile));
            seen.Clear();
            if (asset != null)
            {
                foreach (var row in rows)
                {
                    if (!entities.TryGetValue(row.entityIndex, out var entity) || !Matches(entity, row) || !seen.Add(row.entityIndex)) continue;
                    if (active.TryGetValue(row.entityIndex, out var old) &&
                        (old.row.generation != row.generation || old.row.sourceId != row.sourceId))
                    {
                        Return(old); active.Remove(row.entityIndex);
                    }
                    if (!active.TryGetValue(row.entityIndex, out var visual))
                    {
                        visual = pool.Count == 0 ? CreateVisual() : pool.Pop();
                        active.Add(row.entityIndex, visual); visual.node.SetActive(true);
                    }
                    visual.row = row;
                }
            }
            removed.Clear();
            foreach (var pair in active) if (!seen.Contains(pair.Key)) removed.Add(pair.Key);
            foreach (var slot in removed) { Return(active[slot]); active.Remove(slot); }
        }

        internal bool Replaces(ushort slot) => active.ContainsKey(slot);

        internal void Render(Camera camera, bool visible)
        {
            var origin = camera.transform.position;
            foreach (var visual in active.Values)
            {
                var row = visual.row;
                var x = origin.x + Mathf.Repeat(row.x / 64f - origin.x + 512f, 1024f) - 512f;
                var z = origin.z + Mathf.Repeat(row.y / 64f - origin.z + 512f, 1024f) - 512f;
                visual.node.transform.SetPositionAndRotation(new Vector3(x, row.z / 64f, z),
                    Quaternion.Euler(0, camera.transform.eulerAngles.y, 0));
                visual.node.transform.localScale = Vector3.one * (row.logicalHeight / 64f);
                foreach (var renderer in visual.renderers) renderer.enabled = visible;
            }
        }

        private Visual CreateVisual()
        {
            var node = new GameObject("Castle projectile head"); node.transform.SetParent(parent, false);
            var geometry = new GameObject("Normalized head geometry").transform; geometry.SetParent(node.transform, false);
            UnityEngine.Object.Instantiate(asset, geometry, false);
            var renderers = node.GetComponentsInChildren<Renderer>(true);
            if (renderers.Length == 0) { Destroy(node); throw new InvalidOperationException("Castle projectile has no renderers"); }
            foreach (var renderer in renderers)
            {
                var materials = renderer.sharedMaterials;
                for (var i = 0; i < materials.Length; i++)
                    materials[i] = renderer.name.Contains("Wrap") ? wrap :
                        renderer.name.Contains("Beard") || renderer.name.Contains("Brows") ? beard :
                        renderer.name.Contains("Sclera") ? sclera : renderer.name.Contains("Pupils") ? pupils : face;
                renderer.sharedMaterials = materials; renderer.shadowCastingMode = ShadowCastingMode.Off; renderer.receiveShadows = false;
            }
            var bounds = LocalBounds(geometry, renderers);
            if (bounds.size.y <= 0) { Destroy(node); throw new InvalidOperationException("Castle projectile has no height"); }
            float scale = 1 / bounds.size.y;
            geometry.localScale = Vector3.one * scale;
            geometry.localPosition = new Vector3(-bounds.center.x * scale, -bounds.min.y * scale, -bounds.center.z * scale);
            return new Visual { node = node, renderers = renderers };
        }

        private static Bounds LocalBounds(Transform geometry, Renderer[] renderers)
        {
            var result = new Bounds(); bool initialized = false;
            foreach (var renderer in renderers)
            {
                var local = renderer.localBounds; var matrix = geometry.worldToLocalMatrix * renderer.localToWorldMatrix;
                for (int x = -1; x <= 1; x += 2) for (int y = -1; y <= 1; y += 2) for (int z = -1; z <= 1; z += 2)
                {
                    var point = matrix.MultiplyPoint3x4(local.center + Vector3.Scale(local.extents, new Vector3(x, y, z)));
                    if (!initialized) { result = new Bounds(point, Vector3.zero); initialized = true; }
                    else result.Encapsulate(point);
                }
            }
            return result;
        }

        private void Return(Visual visual) { visual.node.SetActive(false); pool.Push(visual); }
        public void Dispose()
        {
            if (disposed) return; disposed = true;
            foreach (var visual in active.Values) Destroy(visual.node);
            foreach (var visual in pool) Destroy(visual.node);
            active.Clear(); pool.Clear(); Destroy(face); Destroy(wrap); Destroy(beard); Destroy(sclera); Destroy(pupils);
        }
        private static void Destroy(UnityEngine.Object obj)
        {
            if (obj == null) return;
            if (obj is GameObject node) node.SetActive(false);
            if (Application.isPlaying) UnityEngine.Object.Destroy(obj); else UnityEngine.Object.DestroyImmediate(obj);
        }
    }
}
