using System;
using System.Collections.Generic;
using System.Linq;
using UnityEngine;
using UnityEngine.Rendering;

namespace MagicCarpet.Native
{
    // A view of the native scenery carrier, not a charge or collision system.
    // Native action 6 owns contact/recharge and updates the ground Z each pass.
    internal sealed class ModernDolmens : IDisposable
    {
        private readonly Transform root;
        private readonly GameObject model;
        private readonly Material stone;
        private readonly Dictionary<ushort, GameObject> objects = new();
        private readonly Dictionary<ushort, NativeSim.CampaignEntityPresentation> rows = new();
        private ulong generation = ulong.MaxValue;
        private ulong tick;

        internal ModernDolmens(Transform parent, bool voxel)
        {
            model = Resources.Load<GameObject>(voxel ? "ModernScenery/DolmenVoxel" : "ModernScenery/Dolmen");
            var shader = Shader.Find("Universal Render Pipeline/Lit");
            var texture = Resources.Load<Texture2D>("ModernTextures/RockAlbedo");
            if (model == null || shader == null || texture == null)
                throw new InvalidOperationException("Dolmen mesh, rock texture or URP Lit shader missing");
            stone = new Material(shader) { name = "Dolmen weathered stone", enableInstancing = true };
            stone.SetTexture("_BaseMap", texture);
            stone.SetColor("_BaseColor", new Color(.62f, .60f, .57f, 1));
            stone.SetFloat("_Smoothness", .08f);
            root = new GameObject("Native dolmens").transform;
            root.SetParent(parent, false);
        }

        // Both records must name the same current entity. A selector alone is
        // insufficient, since source slots can be recycled for other families.
        internal static bool Matches(NativeSim.CampaignEntitySnapshot entity,
            NativeSim.CampaignEntityPresentation view) =>
            entity.active != 0 && entity.classId == 2 && entity.model == 2 && entity.action == 6 &&
            entity.entityIndex == view.entityIndex && entity.sourceId == view.sourceId &&
            view.renderParameterIndex == 39 && view.baseTmap == 79 && view.selectedTmap == 79 &&
            view.logicalHeight != 0;

        internal bool Replaces(ushort slot) => rows.ContainsKey(slot);

        internal void Observe(ulong currentGeneration, ulong currentTick,
            NativeSim.CampaignEntitySnapshot[] entities, NativeSim.CampaignEntityPresentation[] presentations)
        {
            if (generation != currentGeneration || currentTick < tick) Clear();
            generation = currentGeneration;
            tick = currentTick;
            var entityBySlot = entities.ToDictionary(entity => entity.entityIndex);
            rows.Clear();
            foreach (var view in presentations)
            {
                if (!entityBySlot.TryGetValue(view.entityIndex, out var entity) || !Matches(entity, view)) continue;
                rows.Add(view.entityIndex, view);
                if (objects.ContainsKey(view.entityIndex)) continue;
                var node = UnityEngine.Object.Instantiate(model, root);
                node.name = "Dolmen " + view.entityIndex;
                foreach (var renderer in node.GetComponentsInChildren<Renderer>(true))
                {
                    renderer.sharedMaterial = stone;
                    renderer.shadowCastingMode = ShadowCastingMode.On;
                }
                objects.Add(view.entityIndex, node);
            }
            foreach (var slot in objects.Keys.Where(slot => !rows.ContainsKey(slot)).ToArray())
            {
                UnityEngine.Object.Destroy(objects[slot]);
                objects.Remove(slot);
            }
        }

        internal void Render(Camera camera, bool visible)
        {
            var eye = camera.transform.position;
            foreach (var pair in rows)
            {
                var row = pair.Value;
                var x = eye.x + Mathf.Repeat(row.x / 64f - eye.x + 512f, 1024f) - 512f;
                var z = eye.z + Mathf.Repeat(row.y / 64f - eye.z + 512f, 1024f) - 512f;
                var node = objects[pair.Key];
                node.transform.SetPositionAndRotation(new Vector3(x, row.z / 64f, z), Quaternion.identity);
                node.transform.localScale = Vector3.one * (row.logicalHeight / 64f);
                node.SetActive(visible);
            }
        }

        private void Clear()
        {
            rows.Clear();
            foreach (var node in objects.Values) UnityEngine.Object.Destroy(node);
            objects.Clear();
        }

        public void Dispose()
        {
            Clear();
            UnityEngine.Object.Destroy(root.gameObject);
            UnityEngine.Object.Destroy(stone);
        }
    }
}
