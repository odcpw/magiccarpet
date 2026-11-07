using System;
using System.Collections.Generic;
using UnityEngine;
using UnityEngine.Rendering;

namespace MagicCarpet.Native.Voxels
{
    // Visible and physical skirt at the boundary of a destructible voxel patch. The top
    // follows the Modern triangle sampler; it never covers the patch interior.
    internal sealed class VoxelTerrainSeam : IDisposable
    {
        private const float Spacing = .25f;
        private const float Depth = 2f;
        private readonly GameObject root;
        private readonly Mesh mesh;
        private bool disposed;

        internal VoxelTerrainSeam(Transform parent, Bounds patch, Func<float, float, float> sampleHeight, Material land)
        {
            if (sampleHeight == null) throw new ArgumentNullException(nameof(sampleHeight));
            if (land == null) throw new ArgumentNullException(nameof(land));
            var min = patch.min; var max = patch.max;
            if (!float.IsFinite(min.x) || !float.IsFinite(min.z) || !float.IsFinite(max.x) || !float.IsFinite(max.z)
                || min.x >= max.x || min.z >= max.z) throw new ArgumentOutOfRangeException(nameof(patch));
            // The Modern mesh has 2-unit detailed triangles. Along lattice
            // edges its interpolant is piecewise linear with 2-unit breaks.
            bool Aligned(float value) => Mathf.Abs(value / 2f - Mathf.Round(value / 2f)) < .00001f;
            if (!Aligned(min.x) || !Aligned(max.x) || !Aligned(min.z) || !Aligned(max.z))
                throw new ArgumentException("Voxel patch edges must align to the 2-unit terrain mesh.", nameof(patch));
            var vertices = new List<Vector3>();
            var normals = new List<Vector3>();
            var colors = new List<Color32>();
            var triangles = new List<int>();
            void Segment(Vector3 a, Vector3 b)
            {
                a.y = sampleHeight(a.x, a.z); b.y = sampleHeight(b.x, b.z);
                if (!float.IsFinite(a.y) || !float.IsFinite(b.y)) throw new InvalidOperationException("Terrain seam height must be finite.");
                var down = Vector3.down * Depth;
                var normal = Vector3.Cross(down, b - a).normalized;
                int i = vertices.Count;
                vertices.Add(a); vertices.Add(a + down); vertices.Add(b); vertices.Add(b + down);
                vertices.Add(a); vertices.Add(a + down); vertices.Add(b); vertices.Add(b + down);
                for (int n = 0; n < 8; n++)
                {
                    normals.Add(n < 4 ? normal : -normal);
                    colors.Add(new Color32(255, 255, 255, 0));
                }
                triangles.Add(i); triangles.Add(i + 1); triangles.Add(i + 2);
                triangles.Add(i + 2); triangles.Add(i + 1); triangles.Add(i + 3);
                triangles.Add(i + 4); triangles.Add(i + 6); triangles.Add(i + 5);
                triangles.Add(i + 6); triangles.Add(i + 7); triangles.Add(i + 5);
            }
            // Include the global quarter-unit lattice; every 2-unit Modern
            // triangle boundary is a vertex, including changes of slope.
            void Edge(float low, float high, Func<float, Vector3> point, bool reverse)
            {
                float previous = low;
                int first = Mathf.FloorToInt(low / Spacing) + 1;
                int last = Mathf.CeilToInt(high / Spacing) - 1;
                for (int k = first; k <= last; k++)
                {
                    float next = k * Spacing;
                    if (reverse) Segment(point(next), point(previous));
                    else Segment(point(previous), point(next));
                    previous = next;
                }
                if (reverse) Segment(point(high), point(previous));
                else Segment(point(previous), point(high));
            }
            Edge(min.x, max.x, x => new Vector3(x, 0, min.z), false);
            Edge(min.z, max.z, z => new Vector3(max.x, 0, z), false);
            Edge(min.x, max.x, x => new Vector3(x, 0, max.z), true);
            Edge(min.z, max.z, z => new Vector3(min.x, 0, z), true);
            mesh = new Mesh { name = "Modern terrain voxel seam", indexFormat = vertices.Count > 65535 ? IndexFormat.UInt32 : IndexFormat.UInt16 };
            mesh.SetVertices(vertices); mesh.SetNormals(normals); mesh.SetColors(colors); mesh.SetTriangles(triangles, 0); mesh.RecalculateBounds();
            root = new GameObject("Modern terrain voxel seam");
            root.layer=8;
            root.transform.SetParent(parent, true);
            root.AddComponent<MeshFilter>().sharedMesh = mesh;
            // Quantized ground can be lower than the outer triangle surface.
            // Close that vertical lip so small marbles cannot enter underneath
            // the one-sided outer terrain. No horizontal floor caps the crater.
            root.AddComponent<MeshCollider>().sharedMesh=mesh;
            var renderer = root.AddComponent<MeshRenderer>();
            renderer.sharedMaterial = land;
            renderer.shadowCastingMode = ShadowCastingMode.Off;
        }

        private static void DestroyObject(UnityEngine.Object value)
        {
            if (value == null) return;
            if (Application.isPlaying) UnityEngine.Object.Destroy(value);
            else UnityEngine.Object.DestroyImmediate(value);
        }
        public void Dispose()
        {
            if (disposed) return;
            disposed = true;
            root.SetActive(false);
            DestroyObject(root);
            DestroyObject(mesh);
        }
    }
}
