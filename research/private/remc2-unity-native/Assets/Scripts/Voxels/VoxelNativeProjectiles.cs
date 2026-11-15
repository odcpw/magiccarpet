using System;
using System.Collections.Generic;
using UnityEngine;
using UnityEngine.Rendering;

namespace MagicCarpet.Native.Voxels
{
    // Native snapshots own movement, visibility and lifetime. No collider or
    // gameplay state is created by this presentation.
    internal sealed class VoxelNativeProjectiles : IDisposable
    {
        private sealed class Visual { internal GameObject Node; internal MeshRenderer Renderer; internal Vector3 Target; internal bool Positioned; internal NativeSim.CampaignCharacterState Row; internal ulong LoggedTick=ulong.MaxValue; internal ulong WorldEpoch; }
        private readonly GameObject root;
        private readonly Mesh mesh;
        private readonly Material material;
        private readonly bool voxel;
        private readonly Dictionary<ushort, Visual> active = new();
        private readonly Stack<Visual> pool = new();
        private readonly HashSet<ushort> visible = new(), seen = new();
        private readonly Dictionary<ushort, NativeSim.CampaignEntityPresentation> views = new();
        private readonly List<ushort> stale = new();
        private ulong generation = ulong.MaxValue;
        private bool disposed;

        internal VoxelNativeProjectiles(Transform parent, bool voxel = true)
        {
            this.voxel = voxel;
            var shader = Resources.Load<Shader>(voxel ? "VoxelSurface" : "ModernFireball");
            if (shader == null) throw new InvalidOperationException("Missing native fireball shader");
            root = new GameObject(voxel ? "Native voxel fireballs" : "Native modern fireballs"); root.transform.SetParent(parent, false);
            mesh = voxel ? VoxelManaSpheres.BuildSphereMesh() : VoxelManaSpheres.BuildSmoothSphereMesh();
            material = new Material(shader) { name = voxel ? "Native voxel fireball" : "Native modern fireball" };
            material.SetColor("_BaseColor", new Color(1f, .32f, .035f));
            material.SetFloat("_Emission", 1.4f);
        }

        // The ordinary Fireball initializer uses class 9/model 0, or model 28
        // for source life >= 2. Other class 9 entities include arrows.
        internal static bool IsFireball(byte classId, byte model) => classId == 9 && (model == 0 || model == 28);
        internal void Present(NativeSim.CampaignCharacterState[] entities, NativeSim.CampaignEntityPresentation[] presentations, ulong worldGeneration, ulong worldEpoch = 0)
        {
            if (disposed) throw new ObjectDisposedException(nameof(VoxelNativeProjectiles));
            if (generation != worldGeneration) { foreach (var item in active.Values) Return(item); active.Clear(); generation = worldGeneration; }
            visible.Clear(); views.Clear();
            foreach (var row in presentations) { visible.Add(row.entityIndex); views.Add(row.entityIndex, row); }
            seen.Clear();
            foreach (var row in entities)
            {
                if (row.active == 0 || !IsFireball(row.classId, row.model) || !visible.Contains(row.entityIndex)) continue;
                var view = views[row.entityIndex];
                if (view.logicalHeight == 0 || !seen.Add(row.entityIndex)) continue;
                if(active.TryGetValue(row.entityIndex,out var previous) && previous.Row.slotGeneration!=row.slotGeneration)
                {Return(previous);active.Remove(row.entityIndex);}
                if (!active.TryGetValue(row.entityIndex, out var visual) || visual == null)
                {
                    visual = pool.Count == 0 ? CreateVisual() : pool.Pop(); active.Add(row.entityIndex, visual);
                    visual.Positioned = false; visual.LoggedTick=ulong.MaxValue; visual.Node.SetActive(true);
                }
                // Presentation heights/positions include the source sprite anchor.
                float diameter = view.logicalHeight / 64f;
                var target = new Vector3(view.x / 64f, view.z / 64f + diameter * .5f, view.y / 64f);
                if (!voxel && visual.Positioned)
                {
                    var travel = target - visual.Target;
                    travel.x = Mathf.Repeat(travel.x + 512f, 1024f) - 512f;
                    travel.z = Mathf.Repeat(travel.z + 512f, 1024f) - 512f;
                    if (travel.sqrMagnitude > .000001f)
                        visual.Node.transform.rotation = Quaternion.LookRotation(travel);
                }
                visual.Node.transform.localScale = Vector3.one * diameter;
                visual.Row=row;visual.WorldEpoch=worldEpoch;
                visual.Target = target;
            }
            stale.Clear(); foreach (var pair in active) if (!seen.Contains(pair.Key)) stale.Add(pair.Key);
            foreach (var slot in stale) { Return(active[slot]); active.Remove(slot); }
        }
        internal bool Replaces(ushort slot) => active.ContainsKey(slot);
        internal void Render(Camera camera, bool visible = true)
        {
            var origin = camera.transform.position;
            foreach (var visual in active.Values)
            {
                var target = visual.Target;
                target.x = origin.x + Mathf.Repeat(target.x - origin.x + 512f, 1024f) - 512f;
                target.z = origin.z + Mathf.Repeat(target.z - origin.z + 512f, 1024f) - 512f;
                if (!visual.Positioned || Vector3.Distance(visual.Node.transform.position, target) > 30f) { visual.Node.transform.position = target; visual.Positioned = true; }
                else visual.Node.transform.position = Vector3.Lerp(visual.Node.transform.position, target, 1f - Mathf.Exp(-Time.unscaledDeltaTime * 30f));
                visual.Renderer.enabled = visible;
                if(visible && VoxelModernWorld.ReceiptLogging && visual.LoggedTick!=visual.Row.tick)
                {
                    var row=visual.Row;visual.LoggedTick=row.tick;
                    Debug.Log($"MC_VOXEL_RECEIPT kind=projectile generation={row.generation} world_epoch={visual.WorldEpoch} tick={row.tick} slot={row.entityIndex} slot_generation={row.slotGeneration} source={row.sourceId} model={row.model} x={row.x} y={row.y} z={row.z}");
                }
            }
        }
        private Visual CreateVisual()
        {
            var node = new GameObject("Native fireball"); node.transform.SetParent(root.transform, false); node.transform.localScale = Vector3.one * .7f;
            node.AddComponent<MeshFilter>().sharedMesh = mesh;
            var renderer = node.AddComponent<MeshRenderer>(); renderer.sharedMaterial = material; renderer.shadowCastingMode = ShadowCastingMode.Off; renderer.receiveShadows = false;
            return new Visual { Node = node, Renderer = renderer };
        }
        private void Return(Visual visual) { visual.Node.SetActive(false); visual.Node.transform.rotation=Quaternion.identity; pool.Push(visual); }
        public void Dispose()
        {
            if (disposed) return; disposed = true;
            Destroy(root); Destroy(mesh); Destroy(material); active.Clear(); pool.Clear();
        }
        private static void Destroy(UnityEngine.Object value) { if (Application.isPlaying) UnityEngine.Object.Destroy(value); else UnityEngine.Object.DestroyImmediate(value); }
    }
}
