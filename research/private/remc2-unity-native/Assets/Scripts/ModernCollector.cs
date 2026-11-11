using System;
using System.Collections.Generic;
using UnityEngine;

namespace MagicCarpet.Native
{
    // The Castle balloon is a view of the current native economy occupant.
    // It neither steers nor collects mana in the presentation client.
    internal sealed class ModernCollector : IDisposable
    {
        private sealed class Instance : IDisposable
        {
            internal readonly ModernCharacterInstance Character;
            internal NativeSim.CampaignCastleChild State;
            internal NativeSim.CampaignEntityPresentation Sprite;
            internal bool Admitted;
            internal byte Color = byte.MaxValue;

            internal Instance(ModernCharacterAsset asset, Transform parent) =>
                Character = new ModernCharacterInstance(asset, parent);

            public void Dispose() => Character.Dispose();
        }

        private readonly ModernCharacterAsset asset;
        private readonly Transform root;
        private readonly Dictionary<ushort, Instance> instances = new();
        private readonly Dictionary<ushort, NativeSim.CampaignEntityPresentation> visible = new();
        private readonly HashSet<ushort> seen = new();
        private readonly List<ushort> removed = new();
        private ulong generation = ulong.MaxValue, tick = ulong.MaxValue;

        // remc2 maps playerColorIndex to its eight TMAP owner variants.
        private static readonly byte[] Variant = { 0, 1, 4, 3, 2, 5, 7, 6 };
        private static readonly Color[] OwnerFabric = {
            new(.66f, .82f, .95f), new(.73f, .12f, .15f),
            new(.20f, .45f, .18f), new(.16f, .43f, .68f),
            new(.46f, .23f, .50f), new(.57f, .29f, .31f),
            new(.12f, .30f, .32f), new(.73f, .38f, .09f),
        };

        internal ModernCollector(Transform parent)
        {
            asset = Resources.Load<ModernCharacterAsset>("ModernCharacters/Collector/Character");
            if (asset == null || asset.model == null || asset.height <= 0)
                throw new InvalidOperationException("Missing imported Collector asset");
            asset.Clip("Idle");
            root = new GameObject("Native Castle collectors").transform;
            root.SetParent(parent, false);
        }

        internal void Present(NativeCampaignSession session,
            NativeSim.CampaignEntityPresentation[] presentations)
        {
            if (generation != session.Generation || session.Tick < tick) Clear();
            generation = session.Generation; tick = session.Tick;
            visible.Clear();
            foreach (var sprite in presentations) visible.Add(sprite.entityIndex, sprite);
            seen.Clear();
            foreach (var state in session.CastleChildren())
            {
                seen.Add(state.entityIndex);
                if (instances.TryGetValue(state.entityIndex, out var prior) &&
                    (prior.State.slotGeneration != state.slotGeneration || prior.State.sourceId != state.sourceId))
                {
                    prior.Dispose(); instances.Remove(state.entityIndex);
                }
                if (!instances.TryGetValue(state.entityIndex, out var instance))
                {
                    instance = new Instance(asset, root);
                    instances.Add(state.entityIndex, instance);
                }
                instance.State = state;
                instance.Admitted = (state.relationFlags & 1) != 0 &&
                    state.ownerPlayerColor < Variant.Length && state.drawing != 0 &&
                    visible.TryGetValue(state.entityIndex, out instance.Sprite) &&
                    Matches(state, instance.Sprite);
                if (instance.Admitted && instance.Color != state.ownerPlayerColor)
                {
                    SetOwnerCloth(instance.Character.Root, state.ownerPlayerColor);
                    instance.Color = state.ownerPlayerColor;
                }
                instance.Character.Root.SetActive(instance.Admitted);
            }
            removed.Clear();
            foreach (var pair in instances)
                if (!seen.Contains(pair.Key)) removed.Add(pair.Key);
            foreach (var slot in removed) { instances[slot].Dispose(); instances.Remove(slot); }
        }

        internal static bool Matches(NativeSim.CampaignCastleChild state,
            NativeSim.CampaignEntityPresentation sprite) =>
            state.entityIndex == sprite.entityIndex && state.sourceId == sprite.sourceId &&
            sprite.renderParameterIndex == state.renderParameter &&
            sprite.logicalHeight == state.renderHeight &&
            state.ownerPlayerColor < Variant.Length &&
            sprite.selectedTmap == 88 + Variant[state.ownerPlayerColor];

        internal bool Replaces(ushort slot, ushort sourceId) =>
            instances.TryGetValue(slot, out var instance) && instance.Admitted &&
            instance.State.sourceId == sourceId;

        internal void Render(Camera camera, bool visible)
        {
            var eye = camera.transform.position;
            foreach (var instance in instances.Values)
            {
                var node = instance.Character.Root;
                var enabled = visible && instance.Admitted;
                node.SetActive(enabled);
                if (!enabled) continue;
                var state = instance.State;
                var x = eye.x + Mathf.Repeat(state.x / 64f - eye.x + 512f, 1024f) - 512f;
                var z = eye.z + Mathf.Repeat(state.y / 64f - eye.z + 512f, 1024f) - 512f;
                node.transform.SetPositionAndRotation(new Vector3(x, state.z / 64f, z),
                    Quaternion.Euler(0, 180f - state.yaw * 360f / 2048f, 0));
                node.transform.localScale = Vector3.one * ((state.renderHeight / 64f) / asset.height);
                instance.Character.Tick(Time.deltaTime);
            }
        }

        private static void SetOwnerCloth(GameObject root, byte color)
        {
            var cloth = OwnerFabric[color];
            var crown = new Color(.12f, .38f, .57f, 1);
            var band = new Color(.018f, .028f, .044f, 1);
            var roles = 0;
            var block = new MaterialPropertyBlock();
            foreach (var renderer in root.GetComponentsInChildren<Renderer>(true))
            {
                var materials = renderer.sharedMaterials;
                for (var index = 0; index < materials.Length; index++)
                {
                    var name = materials[index]?.name.Replace('_', ' ').ToLowerInvariant() ?? "";
                    Color tint;
                    if (name.Contains("owner crown")) { tint = crown; roles |= 1; }
                    else if (name.Contains("owner band")) { tint = band; roles |= 2; }
                    else if (name.Contains("owner fabric")) { tint = cloth; roles |= 4; }
                    else if (name.Contains("fabric seams")) { tint = cloth * .55f; roles |= 8; }
                    else if (name.Contains("owner embroidery")) { tint = cloth * .72f; roles |= 16; }
                    else continue;
                    renderer.GetPropertyBlock(block, index);
                    block.SetColor("_BaseColor", tint);
                    if (name.Contains("owner embroidery")) block.SetColor("_EmissionColor", cloth * .08f);
                    renderer.SetPropertyBlock(block, index);
                    block.Clear();
                }
            }
            if (roles != 31)
                throw new InvalidOperationException($"Collector material roles missing: {roles}");
        }

        private void Clear()
        {
            foreach (var instance in instances.Values) instance.Dispose();
            instances.Clear(); visible.Clear(); seen.Clear();
        }

        public void Dispose()
        {
            Clear();
            UnityEngine.Object.Destroy(root.gameObject);
        }
    }
}
