using System;
using System.Collections.Generic;
using System.Linq;
using UnityEngine;

namespace MagicCarpet.Native.Voxels
{
    // A second skin on the existing rig leaves the original source renderer
    // available for contact measurements, blend shapes and authored equipment.
    internal sealed class VoxelCharacterSkin : IDisposable
    {
        private sealed class Cells
        {
            internal Vector3[] Centers;
            internal BoneWeight[] Weights;
            internal Matrix4x4[] Bindposes;
        }
        private struct Aggregate
        {
            internal Vector3 Position;
            internal long R, G, B, A, Count;
            internal void Add(Color32 c) { R += c.r; G += c.g; B += c.b; A += c.a; Count++; }
            internal Color32 Color => new Color32((byte)(R / Count), (byte)(G / Count), (byte)(B / Count), (byte)(A / Count));
        }
        private static readonly Dictionary<VoxelGoatAsset, Cells> CachedCells = new();
        private VoxelGoatAsset asset;
        private readonly Transform root;
        private Cells data;
        private readonly Transform[] bones;
        private readonly Matrix4x4[] bindposes, matrices;
        private readonly GameObject visual;
        private readonly Material material;
        private readonly bool sourceWasHidden;
        private bool disposed;
        internal SkinnedMeshRenderer Source { get; }
        internal SkinnedMeshRenderer Renderer { get; }
        internal int Resolution => asset.resolution;

        private static void Validate(VoxelGoatAsset skin)
        {
            if (skin == null) throw new ArgumentNullException(nameof(skin));
            if (skin.mesh == null || skin.sourceMesh == null || skin.cellColors == null || skin.cellSize <= 0 ||
                (skin.meshFormat == 0 && skin.mesh.vertexCount != skin.cellColors.Length * 24) ||
                (skin.meshFormat == 1 && (skin.cellCenters == null || skin.cellWeights == null || skin.cellFaceMasks == null ||
                    skin.cellCenters.Length != skin.cellColors.Length || skin.cellWeights.Length != skin.cellColors.Length ||
                    skin.cellFaceMasks.Length != skin.cellColors.Length || skin.mesh.vertexCount == 0 || skin.mesh.vertexCount % 4 != 0)) ||
                (skin.meshFormat != 0 && skin.meshFormat != 1))
                throw new InvalidOperationException("Voxel character asset is incomplete");
        }

        // Prewarm shared legacy metadata once, outside render/LOD transitions.
        internal static void PrepareDetail(VoxelGoatAsset skin)
        {
            Validate(skin);
            GetCells(skin);
        }

        internal VoxelCharacterSkin(GameObject characterRoot, VoxelGoatAsset skin)
        {
            if (characterRoot == null || skin == null) throw new ArgumentNullException(characterRoot == null ? nameof(characterRoot) : nameof(skin));
            Validate(skin);
            asset = skin; root = characterRoot.transform;
            Source = characterRoot.GetComponentsInChildren<SkinnedMeshRenderer>(true).Single(r => r.sharedMesh == skin.sourceMesh);
            data = GetCells(skin);
            bones = Source.bones; bindposes = data.Bindposes;
            if (bones.Length != bindposes.Length) throw new InvalidOperationException("Voxel skin bone count differs from source rig");
            matrices = new Matrix4x4[bones.Length];
            var shader = Shader.Find("MagicCarpet/VoxelSurface");
            if (shader == null) throw new InvalidOperationException("Missing MagicCarpet/VoxelSurface shader");
            material = new Material(shader) { name = "Voxel character colors" };
            visual = new GameObject("Voxel character skin");
            visual.layer = Source.gameObject.layer;
            visual.transform.SetParent(Source.transform, false);
            Renderer = visual.AddComponent<SkinnedMeshRenderer>();
            Renderer.sharedMesh = skin.mesh; Renderer.sharedMaterial = material;
            Renderer.bones = bones; Renderer.rootBone = Source.rootBone;
            Renderer.localBounds = Source.localBounds;
            // Native actors may be far off camera; current-pose snapshots use
            // rig matrices directly and do not require offscreen skinning.
            Renderer.updateWhenOffscreen = false;
            Renderer.quality = SkinQuality.Bone4;
            Renderer.shadowCastingMode = Source.shadowCastingMode; Renderer.receiveShadows = Source.receiveShadows;
            sourceWasHidden = Source.forceRenderingOff;
            Source.forceRenderingOff = true;
        }

        // Existing sandbox assets predate center arrays. Derive them once per
        // shared asset, not at every death; newly imported assets serialize them.
        private static Cells GetCells(VoxelGoatAsset skin)
        {
            if (CachedCells.TryGetValue(skin, out var result)) return result;
            if (skin.meshFormat == 1)
            {
                var faces = 0;
                foreach (var mask in skin.cellFaceMasks)
                    for (var face = 0; face < 6; face++) if ((mask & (1 << face)) != 0) faces++;
                if (skin.mesh.vertexCount != faces * 4)
                    throw new InvalidOperationException("Voxel mesh does not match its face metadata");
            }
            if (skin.cellCenters != null && skin.cellCenters.Length == skin.cellColors.Length &&
                skin.cellWeights != null && skin.cellWeights.Length == skin.cellColors.Length)
                result = new Cells { Centers = skin.cellCenters, Weights = skin.cellWeights };
            else
            {
                var vertices = skin.mesh.vertices; var weights = skin.mesh.boneWeights;
                if (weights.Length != vertices.Length) throw new InvalidOperationException("Voxel skin has no bone weights");
                result = new Cells { Centers = new Vector3[skin.cellColors.Length], Weights = new BoneWeight[skin.cellColors.Length] };
                for (var i = 0; i < result.Centers.Length; i++)
                {
                    var center = Vector3.zero;
                    for (var j = 0; j < 24; j++) center += vertices[i * 24 + j];
                    result.Centers[i] = center / 24; result.Weights[i] = weights[i * 24];
                }
            }
            result.Bindposes = skin.mesh.bindposes;
            CachedCells.Add(skin, result); return result;
        }

        // All variants must target the same source mesh and identical bind
        // poses. Reuse this renderer, its material, and the live rig.
        internal void SetDetail(VoxelGoatAsset skin)
        {
            if (disposed) throw new ObjectDisposedException(nameof(VoxelCharacterSkin));
            Validate(skin);
            if (skin == asset) return;
            if (skin.sourceMesh != Source.sharedMesh)
                throw new InvalidOperationException("Voxel detail belongs to a different character rig");
            var next = GetCells(skin);
            if (next.Bindposes.Length != bindposes.Length)
                throw new InvalidOperationException("Voxel detail has a different bone count");
            for (var i = 0; i < bindposes.Length; i++)
                if (!next.Bindposes[i].Equals(bindposes[i]))
                    throw new InvalidOperationException("Voxel detail has different bind poses");
            Renderer.sharedMesh = skin.mesh;
            asset = skin; data = next;
        }

        // Skin one center per occupied source voxel, not 24 cube vertices.
        // Bone matrices are captured once for this pose; cells snap to a common
        // world-axis grid and coincident cells are merged with averaged colors.
        internal List<VoxelFragment> SnapshotFragments()
        {
            if (disposed) throw new ObjectDisposedException(nameof(VoxelCharacterSkin));
            for (var i = 0; i < bones.Length; i++) matrices[i] = bones[i].localToWorldMatrix * bindposes[i];
            var size = asset.cellSize * Mathf.Abs(Source.transform.lossyScale.x);
            var origin = root.position;
            var byGrid = new Dictionary<Vector3Int, int>(data.Centers.Length);
            var ordered = new List<Aggregate>(data.Centers.Length);
            for (var i = 0; i < data.Centers.Length; i++)
            {
                var p = data.Centers[i]; var w = data.Weights[i];
                var world = matrices[w.boneIndex0].MultiplyPoint3x4(p) * w.weight0;
                if (w.weight1 > 0) world += matrices[w.boneIndex1].MultiplyPoint3x4(p) * w.weight1;
                if (w.weight2 > 0) world += matrices[w.boneIndex2].MultiplyPoint3x4(p) * w.weight2;
                if (w.weight3 > 0) world += matrices[w.boneIndex3].MultiplyPoint3x4(p) * w.weight3;
                var relative = (world - origin) / size;
                var key = new Vector3Int(Mathf.RoundToInt(relative.x), Mathf.RoundToInt(relative.y), Mathf.RoundToInt(relative.z));
                if (!byGrid.TryGetValue(key, out var index))
                {
                    index = ordered.Count; byGrid.Add(key, index);
                    ordered.Add(new Aggregate { Position = origin + (Vector3)key * size });
                }
                var cell = ordered[index]; cell.Add(asset.cellColors[i]); ordered[index] = cell;
            }
            var cells = new List<VoxelFragment>(ordered.Count);
            foreach (var cell in ordered) cells.Add(new VoxelFragment(cell.Position, size, cell.Color));
            return cells;
        }

        public void Dispose()
        {
            if (disposed) return;
            disposed = true;
            if (Source != null) Source.forceRenderingOff = sourceWasHidden;
            if (Application.isPlaying) { UnityEngine.Object.Destroy(visual); UnityEngine.Object.Destroy(material); }
            else { UnityEngine.Object.DestroyImmediate(visual); UnityEngine.Object.DestroyImmediate(material); }
        }
    }
}
