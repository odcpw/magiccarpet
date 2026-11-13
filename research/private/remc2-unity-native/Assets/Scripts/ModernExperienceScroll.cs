using System;
using System.Collections.Generic;
using UnityEngine;

namespace MagicCarpet.Native
{
    // Source UpdateScroll_59C80 sets terrain height and disables drawing on
    // collection. Presentation rows own visibility and lifetime; this view
    // adds no bob, spin, particles, collision, or pickup state of its own.
    internal sealed class ModernExperienceScroll : IDisposable
    {
        private readonly Transform parent;
        private readonly GameObject model;
        private readonly bool voxel;
        private readonly Material parchment, overlay, lip, binding, edge;
        internal const float FacingOffsetDegrees = 38f;
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
                // Keep the FBX root's axis/unit conversion beneath our world
                // placement. The single-mesh voxel export has a nonidentity
                // import transform; replacing it tilts the parchment upright.
                node = new GameObject("Experience scroll " + row.entityIndex);
                node.transform.SetParent(parent, false);
                var visual = UnityEngine.Object.Instantiate(model, node.transform, false);
                // The voxel importer bakes the required factor into mesh units
                // but also leaves it on the FBX root. Keep its axis rotation
                // while removing that duplicate 100x root scale.
                if (voxel) visual.transform.localScale = Vector3.one;
            }
        }

        internal ModernExperienceScroll(Transform parent, bool voxel)
        {
            this.parent = parent;
            this.voxel = voxel;
            model = Resources.Load<GameObject>(voxel ?
                "ModernExperienceScroll/ExperienceScrollVoxel" : "ModernExperienceScroll/ExperienceScroll");
            var texture = Resources.Load<Texture2D>(voxel ?
                "ModernExperienceScroll/ExperienceScrollAlbedo" : "ModernExperienceScroll/ExperienceScrollPaper");
            var shader = Shader.Find("Universal Render Pipeline/Lit");
            // Missing generated resources leave the original sprite visible.
            if (model == null || texture == null || shader == null) { model = null; return; }
            parchment = MakeMaterial(shader, "Scroll parchment", new Color(.97f, .87f, .72f), texture);
            overlay = MakeMaterial(shader, "Scroll parchment overlay", new Color(1f, .96f, .86f), texture);
            lip = MakeMaterial(shader, "Scroll parchment lip", new Color(1f, .98f, .88f), texture);
            binding = MakeMaterial(shader, "Scroll violet binding", new Color(.32f, .10f, .30f), null);
            edge = MakeMaterial(shader, "Scroll rolled edge", new Color(.44f, .28f, .14f), null);
        }

        private static Material MakeMaterial(Shader shader, string name, Color color, Texture texture)
        {
            var result = new Material(shader) { name = name, enableInstancing = true };
            result.SetColor("_BaseColor", color);
            if (texture != null) result.SetTexture("_BaseMap", texture);
            result.SetFloat("_Smoothness", .15f);
            return result;
        }

        // Require both the live class-14/model-5 occupant and its static
        // presentation descriptor. Slot reuse must not hide another object.
        internal static bool Matches(NativeSim.CampaignEntitySnapshot entity,
            NativeSim.CampaignEntityPresentation row) =>
            entity.active != 0 && entity.classId == 14 && entity.model == 5 && entity.action == 10 &&
            entity.entityIndex == row.entityIndex && entity.sourceId == row.sourceId &&
            row.entityIndex != 0 && row.sourceId != 0 && row.renderParameterIndex == 280 &&
            row.baseTmap == 259 && row.selectedTmap == 259 && row.renderMode == 1 &&
            row.logicalHeight == 75;

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
                                materials[i] = name.Contains("violet") ? binding :
                                    name.Contains("edge") ? edge :
                                    name.Contains("lip") ? lip :
                                    name.Contains("overlay") ? overlay : parchment;
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
                    Quaternion.Euler(0, camera.transform.eulerAngles.y + FacingOffsetDegrees, 0));
                instance.node.transform.localScale = Vector3.one * (row.logicalHeight / 64f);
                instance.node.SetActive(visible);
            }
        }

        public void Dispose()
        {
            foreach (var instance in instances.Values) Destroy(instance.node);
            instances.Clear(); current.Clear();
            Destroy(parchment); Destroy(overlay); Destroy(lip); Destroy(binding); Destroy(edge);
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
