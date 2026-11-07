using System;
using System.Collections.Generic;
using UnityEngine;
using UnityEngine.Rendering;

namespace MagicCarpet.Native.Voxels
{
    // Presentation only. Native snapshots own existence, movement, color and size;
    // the entity-presentation join supplies the same visibility as the old sprite.
    internal sealed class VoxelManaSpheres : IDisposable
    {
        private const int Grid = 16;
        private sealed class Visual
        {
            internal GameObject Object;
            internal MeshRenderer Renderer;
            internal NativeSim.ManaSphereSnapshot Row;
        }

        private readonly GameObject root;
        private readonly Mesh mesh;
        private readonly Material material;
        private readonly Dictionary<int, Color> colors;
        private readonly Dictionary<ushort, Visual> active = new();
        private readonly Stack<Visual> pool = new();
        private readonly HashSet<ushort> visible = new();
        private readonly HashSet<ushort> seen = new();
        private readonly List<ushort> removed = new();
        private readonly MaterialPropertyBlock properties = new();
        private ulong generation = ulong.MaxValue;
        private bool disposed;
        internal int ActiveCount => active.Count;
        internal int PoolCount => pool.Count;

        internal VoxelManaSpheres(Transform parent, OwnerAssetCache cache)
            : this(parent, ReadColors(cache)) { }

        // This overload permits deterministic rendering/lifecycle checks without an owner package.
        internal VoxelManaSpheres(Transform parent, Dictionary<int, Color> sourceColors)
        {
            colors = sourceColors ?? throw new ArgumentNullException(nameof(sourceColors));
            var shader = Resources.Load<Shader>("VoxelSurface");
            if (shader == null) throw new InvalidOperationException("Missing VoxelSurface shader");
            root = new GameObject("Native voxel mana spheres");
            root.transform.SetParent(parent, false);
            mesh = BuildSphereMesh();
            material = new Material(shader) { name = "Native voxel mana" };
            material.SetFloat("_Emission", .85f);
        }

        internal void Present(NativeSim.ManaSphereSnapshot[] spheres,
            NativeSim.CampaignEntityPresentation[] presentations, ulong worldGeneration)
        {
            if (disposed) throw new ObjectDisposedException(nameof(VoxelManaSpheres));
            if (generation != worldGeneration)
            {
                foreach (var visual in active.Values) Return(visual);
                active.Clear();
                generation = worldGeneration;
            }
            visible.Clear();
            foreach (var row in presentations) visible.Add(row.entityIndex);
            seen.Clear();
            foreach (var row in spheres)
            {
                var slot = row.sourceEntityIndex;
                if (row.active == 0 || !visible.Contains(slot) || !seen.Add(slot)) continue;
                var acquired = false;
                if (!active.TryGetValue(slot, out var visual))
                {
                    visual = pool.Count == 0 ? CreateVisual() : pool.Pop();
                    active.Add(slot, visual);
                    visual.Object.SetActive(true);
                    acquired = true;
                }
                if (acquired || visual.Row.logicalHeight != row.logicalHeight)
                    visual.Object.transform.localScale = Vector3.one * (row.logicalHeight / 64f);
                if (acquired || visual.Row.baseTmap != row.baseTmap)
                {
                    properties.Clear();
                    if (!colors.TryGetValue(row.baseTmap, out var color))
                        throw new InvalidOperationException($"Missing source mana TMAP {row.baseTmap}");
                    properties.SetColor("_BaseColor", color);
                    visual.Renderer.SetPropertyBlock(properties);
                }
                visual.Row = row;
            }
            removed.Clear();
            foreach (var pair in active)
                if (!seen.Contains(pair.Key)) removed.Add(pair.Key);
            foreach (var slot in removed)
            {
                Return(active[slot]);
                active.Remove(slot);
            }
        }

        internal bool Replaces(ushort entityIndex) => active.ContainsKey(entityIndex);

        internal void Render(Camera camera, bool visible = true)
        {
            var origin = camera.transform.position;
            foreach (var visual in active.Values)
            {
                var row = visual.Row;
                var x = origin.x + Mathf.Repeat(row.x / 64f - origin.x + 512f, 1024f) - 512f;
                var z = origin.z + Mathf.Repeat(row.y / 64f - origin.z + 512f, 1024f) - 512f;
                // Sprite coordinates describe the bottom edge, not the centre.
                visual.Object.transform.position = new Vector3(x, row.z / 64f + row.logicalHeight / 128f, z);
                visual.Renderer.enabled = visible;
            }
        }

        private Visual CreateVisual()
        {
            var node = new GameObject("Native mana sphere");
            node.transform.SetParent(root.transform, false);
            node.AddComponent<MeshFilter>().sharedMesh = mesh;
            var renderer = node.AddComponent<MeshRenderer>();
            renderer.sharedMaterial = material;
            renderer.shadowCastingMode = ShadowCastingMode.Off;
            renderer.receiveShadows = false;
            return new Visual { Object = node, Renderer = renderer };
        }

        private void Return(Visual visual)
        {
            visual.Object.SetActive(false);
            pool.Push(visual);
        }

        // Use each original indexed sprite as the color source, rather than guessing
        // player-color assignments. Bright pixels contribute most to the luminous orb.
        private static Dictionary<int, Color> ReadColors(OwnerAssetCache cache)
        {
            var descriptor = cache.EnvironmentMaterials;
            if (descriptor == null) throw new InvalidOperationException("Mana requires the environment asset bank");
            var palette = cache.ReadOutput(descriptor.palette.rgb8_path);
            var result = new Dictionary<int, Color>();
            foreach (var texture in descriptor.tmaps.textures)
            {
                if (texture.source_empty || (texture.index != 58 && (texture.index < 67 || texture.index > 74))) continue;
                result.Add(texture.index, ColorFromIndexedSprite(cache.ReadOutput(texture.indices_path), palette));
            }
            return result;
        }

        internal static Color ColorFromIndexedSprite(byte[] indices, byte[] palette)
        {
            double r = 0, g = 0, b = 0, total = 0;
            foreach (var index in indices)
            {
                if (index == 0) continue;
                var offset = index * 3;
                var red = palette[offset]; var green = palette[offset + 1]; var blue = palette[offset + 2];
                var weight = Math.Max(red, Math.Max(green, blue));
                r += red * weight; g += green * weight; b += blue * weight; total += weight;
            }
            if (total == 0) throw new InvalidOperationException("Mana sprite has no visible color");
            return new Color((float)(r / total / 255), (float)(g / total / 255), (float)(b / total / 255), 1);
        }

        // One shared opaque mesh; only exterior cell faces are emitted. A cell is
        // 1/16 of the native particle diameter, with no per-cell GameObjects.
        internal static Mesh BuildSphereMesh()
        {
            var occupied = new bool[Grid, Grid, Grid];
            for (var x = 0; x < Grid; x++) for (var y = 0; y < Grid; y++) for (var z = 0; z < Grid; z++)
            {
                var p = new Vector3(x + .5f, y + .5f, z + .5f) / Grid - Vector3.one * .5f;
                occupied[x, y, z] = p.sqrMagnitude <= .25f;
            }
            Vector3Int[] directions = { Vector3Int.left, Vector3Int.right, Vector3Int.down, Vector3Int.up, new Vector3Int(0, 0, -1), new Vector3Int(0, 0, 1) };
            Vector3[] axesU = { Vector3.forward, Vector3.back, Vector3.right, Vector3.right, Vector3.left, Vector3.right };
            Vector3[] axesV = { Vector3.up, Vector3.up, Vector3.forward, Vector3.back, Vector3.up, Vector3.up };
            var vertices = new List<Vector3>(); var normals = new List<Vector3>();
            var colors = new List<Color32>(); var triangles = new List<int>();
            for (var x = 0; x < Grid; x++) for (var y = 0; y < Grid; y++) for (var z = 0; z < Grid; z++)
            {
                if (!occupied[x, y, z]) continue;
                var center = new Vector3(x + .5f, y + .5f, z + .5f) / Grid - Vector3.one * .5f;
                for (var f = 0; f < directions.Length; f++)
                {
                    var d = directions[f]; var nx = x + d.x; var ny = y + d.y; var nz = z + d.z;
                    if (nx >= 0 && ny >= 0 && nz >= 0 && nx < Grid && ny < Grid && nz < Grid && occupied[nx, ny, nz]) continue;
                    var normal = (Vector3)d; var face = center + normal * (.5f / Grid);
                    var u = axesU[f] * (.5f / Grid); var v = axesV[f] * (.5f / Grid);
                    var first = vertices.Count;
                    vertices.Add(face - u - v); vertices.Add(face + u - v); vertices.Add(face + u + v); vertices.Add(face - u + v);
                    var shade = (byte)(225 + ((x * 17 + y * 29 + z * 11) & 30));
                    for (var i = 0; i < 4; i++) { normals.Add(normal); colors.Add(new Color32(shade, shade, shade, 255)); }
                    triangles.Add(first); triangles.Add(first + 1); triangles.Add(first + 2);
                    triangles.Add(first); triangles.Add(first + 2); triangles.Add(first + 3);
                }
            }
            var mesh = new Mesh { name = "Shared 16-cell voxel mana sphere" };
            mesh.SetVertices(vertices); mesh.SetNormals(normals); mesh.SetColors(colors); mesh.SetTriangles(triangles, 0);
            mesh.RecalculateBounds();
            return mesh;
        }

        public void Dispose()
        {
            if (disposed) return;
            disposed = true;
            Destroy(root); Destroy(mesh); Destroy(material);
            active.Clear(); pool.Clear();
        }

        private static void Destroy(UnityEngine.Object value)
        {
            if (Application.isPlaying) UnityEngine.Object.Destroy(value);
            else UnityEngine.Object.DestroyImmediate(value);
        }
    }
}
