using System;
using System.Collections.Generic;
using UnityEngine;

namespace MagicCarpet.Native
{
    // The target is a source-selected static TMAP, revealed by the native exit
    // switch. The set of presentation rows is the authority for its visibility;
    // this class does not infer an exit phase or create a premature portal.
    internal sealed class ModernExitPortal : IDisposable
    {
        private readonly Transform root;
        private readonly GameObject asset;
        private readonly Dictionary<ushort, Instance> instances = new Dictionary<ushort, Instance>();
        private readonly HashSet<ushort> current = new HashSet<ushort>();
        private readonly List<ushort> removed = new List<ushort>();

        private sealed class Instance
        {
            internal readonly ushort sourceId;
            internal readonly ulong generation;
            internal readonly GameObject node;
            internal readonly Renderer[] renderers;
            internal NativeSim.CampaignEntityPresentation row;
            internal Instance(NativeSim.CampaignEntityPresentation row, GameObject asset, Transform root)
            {
                sourceId = row.sourceId;
                generation = row.generation;
                node = UnityEngine.Object.Instantiate(asset, root);
                node.name = "Exit target " + row.entityIndex;
                renderers = node.GetComponentsInChildren<Renderer>(true);
                this.row = row;
            }
        }

        internal ModernExitPortal(Transform root, bool voxel)
        {
            this.root = root;
            asset = Resources.Load<GameObject>(voxel ? "ModernCharacters/ExitPortal/ExitPortalVoxel" : "ModernCharacters/ExitPortal/ExitPortal");
        }

        internal static bool Matches(NativeSim.CampaignEntityPresentation row) =>
            row.renderParameterIndex == 338 && row.baseTmap == 461 && row.selectedTmap == 461;

        internal void Present(IReadOnlyList<NativeSim.CampaignEntityPresentation> rows)
        {
            current.Clear();
            // A build without owner-local generated art keeps the native sprite.
            if (asset == null) return;
            foreach (var row in rows)
            {
                if (!Matches(row)) continue;
                current.Add(row.entityIndex);
                if (instances.TryGetValue(row.entityIndex, out var prior) &&
                    (prior.generation != row.generation || prior.sourceId != row.sourceId))
                {
                    Destroy(prior.node);
                    instances.Remove(row.entityIndex);
                }
                if (!instances.TryGetValue(row.entityIndex, out var instance))
                {
                    instance = new Instance(row, asset, root);
                    instances.Add(row.entityIndex, instance);
                }
                instance.row = row;
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
            var cameraPosition = camera.transform.position;
            foreach (var instance in instances.Values)
            {
                var row = instance.row;
                var x = cameraPosition.x + Mathf.Repeat(row.x / 64f - cameraPosition.x + 512f, 1024f) - 512f;
                var z = cameraPosition.z + Mathf.Repeat(row.y / 64f - cameraPosition.z + 512f, 1024f) - 512f;
                // TMAP 461 is a static billboard; keep its front facing the
                // viewer while giving the face physical depth in both styles.
                instance.node.transform.SetPositionAndRotation(new Vector3(x, row.z / 64f, z),
                    Quaternion.Euler(0, camera.transform.eulerAngles.y, 0));
                instance.node.transform.localScale = Vector3.one * (row.logicalHeight / 64f);
                foreach (var renderer in instance.renderers) renderer.enabled = visible;
            }
        }

        public void Dispose()
        {
            foreach (var instance in instances.Values) Destroy(instance.node);
            instances.Clear();
            current.Clear();
        }

        private static void Destroy(UnityEngine.Object obj)
        {
            if (obj is GameObject node) node.SetActive(false);
            if (Application.isPlaying) UnityEngine.Object.Destroy(obj);
            else UnityEngine.Object.DestroyImmediate(obj);
        }
    }
}
