using System;
using System.Collections.Generic;
using System.Linq;
using UnityEngine;

namespace MagicCarpet.Native.Voxels
{
    // Plays the same imported graph as the approved goat, on a voxel skin.
    internal sealed class VoxelGoat : IDisposable
    {
        private const int VerticesPerCell = 24;
        private readonly ModernCharacterInstance character;
        private readonly VoxelGoatAsset asset;
        private readonly SkinnedMeshRenderer renderer;
        private readonly Material material;
        private readonly Mesh baked;
        private readonly float scale;
        private bool disposed;
        internal GameObject Root => character.Root;
        internal string CurrentClip => character.CurrentClip;
        internal float ClipTime => character.ClipTime;

        internal VoxelGoat(ModernCharacterAsset source, VoxelGoatAsset voxels, Transform parent, float height)
        {
            if (source == null || voxels == null) throw new ArgumentNullException(source == null ? nameof(source) : nameof(voxels));
            if (!float.IsFinite(height) || height <= 0 || !float.IsFinite(source.height) || source.height <= 0)
                throw new ArgumentOutOfRangeException(nameof(height));
            if (voxels.mesh == null || voxels.cellColors == null ||
                voxels.mesh.vertexCount != voxels.cellColors.Length * VerticesPerCell || voxels.cellSize <= 0)
                throw new InvalidOperationException("Voxel goat asset is incomplete");
            source.Clip("Idle"); source.Clip("Walk"); source.Clip("Sprint");
            asset = voxels;
            scale = height / source.height;
            character = new ModernCharacterInstance(source, parent);
            try
            {
                var skins = Root.GetComponentsInChildren<SkinnedMeshRenderer>(true);
                renderer = skins.Single(r => r.sharedMesh == voxels.sourceMesh);
                foreach (var r in Root.GetComponentsInChildren<Renderer>(true)) r.enabled = r == renderer;
                renderer.sharedMesh = voxels.mesh;
                renderer.updateWhenOffscreen = true;
                var shader = Shader.Find("MagicCarpet/VoxelSurface");
                if (shader == null) throw new InvalidOperationException("Missing MagicCarpet/VoxelSurface shader");
                material = new Material(shader) { name = "Voxel goat colors" };
                renderer.sharedMaterial = material;
                Root.transform.localScale = Vector3.one * scale;
                baked = new Mesh { name = "Voxel goat current pose" };
            }
            catch { character.Dispose(); throw; }
        }

        // distance is world-space travel since the last rendered frame.
        internal void Render(float dt, float distance, bool sprint = false)
        {
            if (disposed) throw new ObjectDisposedException(nameof(VoxelGoat));
            if (!float.IsFinite(dt) || dt < 0 || !float.IsFinite(distance) || distance < 0)
                throw new ArgumentOutOfRangeException(nameof(distance));
            if (distance <= .00001f)
            {
                character.Play("Idle", .12f, false);
                character.Tick(dt, dt);
                return;
            }
            var clip = sprint ? "Sprint" : "Walk";
            character.Play(clip, .08f, false);
            // Same contact-derived phase calibration as ModernGoatPresenter.
            var cycleTravel = .16f / (sprint ? .075f : .18f) * scale;
            character.Tick(dt, distance / cycleTravel * character.Asset.Clip(clip).length);
        }

        private sealed class FragmentCell
        {
            internal Vector3 Position;
            internal long R, G, B, A, Count;
            internal void Add(Color32 color)
            { R += color.r; G += color.g; B += color.b; A += color.a; Count++; }
            internal Color32 Color => new Color32((byte)(R / Count), (byte)(G / Count), (byte)(B / Count), (byte)(A / Count));
        }

        // Bake after the last Render, before hiding/removing the character. Snap
        // each deformed centre to one world-axis grid centred on the goat root.
        // Nearest-grid quantization moves it at most half a cell on each axis;
        // coincident samples become one cell with their mean source color.
        internal List<VoxelFragment> SnapshotFragments()
        {
            if (disposed) throw new ObjectDisposedException(nameof(VoxelGoat));
            // Unity 6's default bake carries the renderer transform scale into
            // the vertices. Compensate for it because TransformPoint below
            // applies that scale when converting into world space.
            renderer.BakeMesh(baked, true);
            var vertices = baked.vertices;
            var size = asset.cellSize * Mathf.Abs(renderer.transform.lossyScale.x);
            var origin = Root.transform.position;
            var byGrid = new Dictionary<Vector3Int, FragmentCell>();
            var ordered = new List<FragmentCell>();
            for (var i = 0; i < asset.cellColors.Length; i++)
            {
                var center = Vector3.zero;
                for (var j = 0; j < VerticesPerCell; j++) center += vertices[i * VerticesPerCell + j];
                var world = renderer.transform.TransformPoint(center / VerticesPerCell);
                var relative = (world - origin) / size;
                var key = new Vector3Int(Mathf.RoundToInt(relative.x), Mathf.RoundToInt(relative.y), Mathf.RoundToInt(relative.z));
                if (!byGrid.TryGetValue(key, out var cell))
                {
                    cell = new FragmentCell { Position = origin + (Vector3)key * size };
                    byGrid.Add(key, cell);
                    ordered.Add(cell);
                }
                cell.Add(asset.cellColors[i]);
            }
            var cells = new List<VoxelFragment>(ordered.Count);
            foreach (var cell in ordered) cells.Add(new VoxelFragment(cell.Position, size, cell.Color));
            if (cells.Count > 0)
            {
                var bounds = new Bounds(cells[0].Position, Vector3.zero);
                foreach (var cell in cells) bounds.Encapsulate(cell.Position);
                bounds.Expand(size);
                var visibleBounds = renderer.bounds;
                Debug.Log($"MC_VOXEL_GOAT_SNAPSHOT resolution={asset.resolution} cells={cells.Count} cellSize={size:F5} root={origin.ToString("F4")} rendererCenter={visibleBounds.center.ToString("F4")} rendererSize={visibleBounds.size.ToString("F4")} fragmentsCenter={bounds.center.ToString("F4")} fragmentsSize={bounds.size.ToString("F4")} skinScale={renderer.transform.lossyScale.ToString("F4")}");
            }
            return cells;
        }

        public void Dispose()
        {
            if (disposed) return;
            disposed = true;
            if (Application.isPlaying) { UnityEngine.Object.Destroy(material); UnityEngine.Object.Destroy(baked); }
            else { UnityEngine.Object.DestroyImmediate(material); UnityEngine.Object.DestroyImmediate(baked); }
            character.Dispose();
        }
    }
}
