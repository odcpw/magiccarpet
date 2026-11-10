using System;
using System.Collections.Generic;
using UnityEngine;

namespace MagicCarpet.Native
{
    // The native visible presentation stream owns collection, holder and
    // reset state. This view adds no independent animation or pickup logic.
    internal sealed class ModernCastleSpellJar : IDisposable
    {
        private readonly Transform parent;
        private readonly GameObject model;
        private readonly bool voxel;
        private readonly Material silver, rim, red;
        private readonly Dictionary<ushort, Instance> instances = new();
        private readonly HashSet<ushort> current = new();
        private readonly List<ushort> removed = new();

        private sealed class Instance
        {
            internal readonly GameObject node;
            internal readonly ushort sourceId;
            internal readonly ulong generation;
            internal NativeSim.CampaignEntityPresentation row;
            internal Instance(NativeSim.CampaignEntityPresentation row, GameObject model, Transform parent, bool voxel)
            {
                this.row = row;
                sourceId = row.sourceId;
                generation = row.generation;
                node = new GameObject("Castle spell jar " + row.entityIndex);
                node.transform.SetParent(parent, false);
                var visual = UnityEngine.Object.Instantiate(model, node.transform);
                // Blender exports the single voxel mesh Z-up; the multi-object
                // FBX contains its own conversion on its child meshes.
                if (voxel)
                {
                    visual.transform.localRotation = Quaternion.Euler(-90, 0, 0);
                    visual.transform.localScale = Vector3.one;
                }
            }
        }

        internal ModernCastleSpellJar(Transform parent, bool voxel)
        {
            this.parent = parent;
            this.voxel = voxel;
            model = Resources.Load<GameObject>(voxel ?
                "ModernCastleSpellJar/CastleSpellJarVoxel" : "ModernCastleSpellJar/CastleSpellJar");
            var shader = Shader.Find("Universal Render Pipeline/Lit");
            // The original sprite remains available if an asset is missing.
            if (model == null || shader == null) { model = null; return; }
            silver = Material(shader, "Jar silver", new Color(.48f, .51f, .56f), .68f, .72f);
            rim = Material(shader, "Jar dark rim", new Color(.18f, .19f, .21f), .60f, .64f);
            red = Material(shader, "Jar red lid", new Color(.62f, .012f, .018f), .20f, .70f);
        }

        private static Material Material(Shader shader, string name, Color color, float metal, float smoothness)
        {
            var material = new Material(shader) { name = name, enableInstancing = true };
            material.SetColor("_BaseColor", color);
            material.SetFloat("_Metallic", metal);
            material.SetFloat("_Smoothness", smoothness);
            return material;
        }

        internal static bool Matches(NativeSim.CampaignEntitySnapshot entity,
            NativeSim.CampaignEntityPresentation row) =>
            entity.active != 0 && entity.classId == 15 && entity.model == 2 &&
            (entity.action == 7 || entity.action == 8) &&
            entity.entityIndex == row.entityIndex && entity.sourceId == row.sourceId &&
            row.entityIndex != 0 && row.sourceId != 0 && (row.controlFlags & 1) == 0 &&
            row.renderParameterIndex == 77 && row.baseTmap == 59 && row.selectedTmap == 59 &&
            row.renderMode == 1 && row.logicalHeight == 150;

        internal void Present(IReadOnlyDictionary<ushort, NativeSim.CampaignEntitySnapshot> entities,
            IReadOnlyList<NativeSim.CampaignEntityPresentation> rows)
        {
            current.Clear();
            if (model != null)
            {
                foreach (var row in rows)
                {
                    if (!entities.TryGetValue(row.entityIndex, out var entity) || !Matches(entity, row)) continue;
                    current.Add(row.entityIndex);
                    if (instances.TryGetValue(row.entityIndex, out var old) &&
                        (old.generation != row.generation || old.sourceId != row.sourceId))
                    {
                        Destroy(old.node);
                        instances.Remove(row.entityIndex);
                    }
                    if (!instances.TryGetValue(row.entityIndex, out var instance))
                    {
                        instance = new Instance(row, model, parent, voxel);
                        foreach (var renderer in instance.node.GetComponentsInChildren<Renderer>(true))
                        {
                            var materials = renderer.sharedMaterials;
                            for (var i = 0; i < materials.Length; i++)
                            {
                                var name = materials[i] == null ? "" : materials[i].name.ToLowerInvariant();
                                materials[i] = name.Contains("red") ? red : name.Contains("rim") ? rim : silver;
                            }
                            renderer.sharedMaterials = materials;
                        }
                        instances.Add(row.entityIndex, instance);
                    }
                    instance.row = row;
                }
            }
            removed.Clear();
            foreach (var pair in instances)
                if (!current.Contains(pair.Key)) removed.Add(pair.Key);
            foreach (var slot in removed)
            {
                Destroy(instances[slot].node);
                instances.Remove(slot);
            }
        }

        internal bool Replaces(ushort slot) => current.Contains(slot);

        internal void Render(Camera camera, bool visible)
        {
            var eye = camera.transform.position;
            foreach (var instance in instances.Values)
            {
                var row = instance.row;
                var x = eye.x + Mathf.Repeat(row.x / 64f - eye.x + 512f, 1024f) - 512f;
                var z = eye.z + Mathf.Repeat(row.y / 64f - eye.z + 512f, 1024f) - 512f;
                instance.node.transform.SetPositionAndRotation(new Vector3(x, row.z / 64f, z),
                    Quaternion.Euler(0, camera.transform.eulerAngles.y, 0));
                instance.node.transform.localScale = Vector3.one * (row.logicalHeight / 64f);
                instance.node.SetActive(visible);
            }
        }

        public void Dispose()
        {
            foreach (var instance in instances.Values) Destroy(instance.node);
            instances.Clear(); current.Clear();
            Destroy(silver); Destroy(rim); Destroy(red);
        }

        private static void Destroy(UnityEngine.Object obj)
        {
            if (obj == null) return;
            if (obj is GameObject node) node.SetActive(false);
            if (Application.isPlaying) UnityEngine.Object.Destroy(obj);
            else UnityEngine.Object.DestroyImmediate(obj);
        }
    }
}
