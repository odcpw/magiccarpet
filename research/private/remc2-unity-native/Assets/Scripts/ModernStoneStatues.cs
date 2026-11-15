using System;
using System.Collections.Generic;
using System.Linq;
using UnityEngine;
using UnityEngine.Rendering;

namespace MagicCarpet.Native
{
    // Presentation only: native action 3 controls placement and lifetime.
    internal sealed class ModernStoneStatues : IDisposable
    {
        private readonly Transform root;
        private readonly Func<float, float, float> surfaceHeight;
        private readonly GameObject model;
        private readonly Material material;
        private readonly bool enabled;
        private readonly Dictionary<ushort, GameObject> objects = new();
        private readonly Dictionary<ushort, NativeSim.CampaignEntityPresentation> rows = new();
        private ulong generation = ulong.MaxValue;
        private ulong tick;

        internal ModernStoneStatues(Transform parent, bool voxel, string textureBank, Func<float, float, float> surfaceHeight = null)
        {
            enabled = SupportsBank(textureBank);
            this.surfaceHeight = surfaceHeight;
            model = Resources.Load<GameObject>(voxel ? "ModernScenery/StoneStatueVoxel" : "ModernScenery/StoneStatue");
            if (model == null) throw new InvalidOperationException("Stone statue mesh missing");
            material = CreateMaterial();
            root = new GameObject("Native stone statues").transform;
            root.SetParent(parent, false);
        }

        internal static Material CreateMaterial()
        {
            var shader = Shader.Find("Universal Render Pipeline/Lit");
            var texture = Resources.Load<Texture2D>("ModernScenery/StoneStatueAlbedo");
            if (shader == null || texture == null)
                throw new InvalidOperationException("Stone statue albedo or URP Lit shader missing");
            var material = new Material(shader) { name = "Statue blue-grey stone", enableInstancing = true };
            material.SetTexture("_BaseMap", texture);
            material.SetColor("_BaseColor", new Color(.42f, .51f, .57f, 1));
            material.SetFloat("_Smoothness", .08f);
            return material;
        }

        // TMAP 78 in the day bank depicts a different object entirely.
        internal static bool SupportsBank(string textureBank) => textureBank == "TMAPS1-0";

        internal static bool Matches(NativeSim.CampaignEntitySnapshot entity,
            NativeSim.CampaignEntityPresentation view) =>
            entity.active != 0 && entity.classId == 2 && entity.model == 1 && entity.action == 3 &&
            entity.entityIndex != 0 && entity.sourceId != 0 &&
            entity.entityIndex == view.entityIndex && entity.sourceId == view.sourceId &&
            view.renderParameterIndex == 79 && view.baseTmap == 78 && view.selectedTmap == 78 &&
            view.logicalHeight != 0;

        internal static bool MatchesCurrent(
            IReadOnlyDictionary<ushort, NativeSim.CampaignEntitySnapshot> entities,
            NativeSim.CampaignEntityPresentation view) =>
            entities.TryGetValue(view.entityIndex, out var entity) && Matches(entity, view);

        internal bool Replaces(ushort slot) => rows.ContainsKey(slot);

        internal void Observe(ulong currentGeneration, ulong currentTick,
            IReadOnlyDictionary<ushort, NativeSim.CampaignEntitySnapshot> entities,
            NativeSim.CampaignEntityPresentation[] presentations)
        {
            if (generation != currentGeneration || currentTick < tick) Clear();
            generation = currentGeneration;
            tick = currentTick;
            if (!enabled) { Clear(); return; }
            rows.Clear();
            foreach (var view in presentations)
            {
                if (!MatchesCurrent(entities, view)) continue;
                rows.Add(view.entityIndex, view);
                if (objects.ContainsKey(view.entityIndex)) continue;
                // Preserve any axis and scale correction on the imported model root.
                // Runtime placement belongs on a separate parent, not the FBX root.
                var node = new GameObject("Stone statue " + view.entityIndex);
                node.transform.SetParent(root, false);
                UnityEngine.Object.Instantiate(model, node.transform, false);
                foreach (var renderer in node.GetComponentsInChildren<Renderer>(true))
                {
                    renderer.sharedMaterial = material;
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
                node.transform.SetPositionAndRotation(new Vector3(x, surfaceHeight?.Invoke(x, z) ?? row.z / 64f, z), Quaternion.identity);
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
            UnityEngine.Object.Destroy(material);
        }
    }
}
