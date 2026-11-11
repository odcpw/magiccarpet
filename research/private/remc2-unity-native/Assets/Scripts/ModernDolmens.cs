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
        private readonly Func<float, float, float> terrainHeight;
        private readonly Dictionary<ushort, GameObject> objects = new();
        private readonly Dictionary<ushort, NativeSim.CampaignEntityPresentation> rows = new();
        private static readonly Vector2[] SupportCenters = {
            new Vector2(-.50f, -.08f), new Vector2(.48f, -.05f), new Vector2(.02f, .23f)
        };
        private static readonly Vector2[] SupportOffsets = {
            Vector2.zero, new Vector2(-.10f, 0), new Vector2(.10f, 0),
            new Vector2(0, -.10f), new Vector2(0, .10f)
        };
        private ulong generation = ulong.MaxValue;
        private ulong tick;

        internal ModernDolmens(Transform parent, bool voxel, Func<float, float, float> terrainHeight)
        {
            this.terrainHeight = terrainHeight ?? throw new ArgumentNullException(nameof(terrainHeight));
            model = Resources.Load<GameObject>(voxel ? "ModernScenery/DolmenVoxel" : "ModernScenery/Dolmen");
            var shader = Shader.Find("Universal Render Pipeline/Lit");
            var texture = Resources.Load<Texture2D>("ModernTextures/RockAlbedo");
            if (model == null || shader == null || texture == null)
                throw new InvalidOperationException("Dolmen mesh, rock texture or URP Lit shader missing");
            stone = new Material(shader) { name = "Dolmen weathered stone", enableInstancing = true };
            stone.SetTexture("_BaseMap", texture);
            stone.SetColor("_BaseColor", new Color(.84f, .86f, .91f, 1));
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
            (view.controlFlags & 0x421) == 0 &&
            view.renderParameterIndex == 39 && view.baseTmap == 79 && view.selectedTmap == 79 &&
            view.logicalHeight != 0;

        internal bool Replaces(ushort slot) => rows.ContainsKey(slot);

        // Modern's filtered heightfield is not identical to native collision
        // height. Sink the common base toward the lowest sampled support
        // footprint, but cap penetration at one quarter of the asset height.
        // A steep slope can leave a lower support exposed; never move gameplay.
        internal static float VisualBaseHeight(float x, float z, float scale,
            Func<float, float, float> height)
        {
            var lowest = float.PositiveInfinity;
            var highest = float.NegativeInfinity;
            foreach (var center in SupportCenters)
                foreach (var offset in SupportOffsets)
                {
                    var sample = height(x + (center.x + offset.x) * scale,
                        z + (center.y + offset.y) * scale);
                    lowest = Mathf.Min(lowest, sample);
                    highest = Mathf.Max(highest, sample);
                }
            return Mathf.Max(lowest, highest - .25f * scale);
        }

        internal void Observe(ulong currentGeneration, ulong currentTick,
            IReadOnlyDictionary<ushort, NativeSim.CampaignEntitySnapshot> entityBySlot,
            NativeSim.CampaignEntityPresentation[] presentations)
        {
            if (generation != currentGeneration || currentTick < tick) Clear();
            generation = currentGeneration;
            tick = currentTick;
            rows.Clear();
            foreach (var view in presentations)
            {
                if (!entityBySlot.TryGetValue(view.entityIndex, out var entity) || !Matches(entity, view)) continue;
                rows.Add(view.entityIndex, view);
                if (objects.ContainsKey(view.entityIndex)) continue;
                // The Voxel FBX carries import-axis and scale corrections on
                // its root; apply world placement to a separate parent.
                var node = new GameObject("Dolmen " + view.entityIndex);
                node.transform.SetParent(root, false);
                UnityEngine.Object.Instantiate(model, node.transform, false);
                foreach (var renderer in node.GetComponentsInChildren<Renderer>(true))
                {
                    renderer.sharedMaterial = stone;
                    renderer.shadowCastingMode = ShadowCastingMode.On;
                }
                objects.Add(view.entityIndex, node);
            }
            foreach (var slot in objects.Keys.Where(slot => !rows.ContainsKey(slot)).ToArray())
            {
                Destroy(objects[slot]);
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
                var scale = row.logicalHeight / 64f;
                node.transform.SetPositionAndRotation(new Vector3(x, VisualBaseHeight(x, z, scale, terrainHeight), z), Quaternion.identity);
                node.transform.localScale = Vector3.one * scale;
                node.SetActive(visible);
            }
        }

        private void Clear()
        {
            rows.Clear();
            foreach (var node in objects.Values) Destroy(node);
            objects.Clear();
        }

        private static void Destroy(UnityEngine.Object value)
        {
            if (Application.isPlaying) UnityEngine.Object.Destroy(value);
            else UnityEngine.Object.DestroyImmediate(value);
        }

        public void Dispose()
        {
            Clear();
            Destroy(root.gameObject);
            Destroy(stone);
        }
    }
}
