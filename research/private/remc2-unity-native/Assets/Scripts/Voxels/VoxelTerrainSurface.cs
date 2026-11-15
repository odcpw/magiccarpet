using System;
using System.Collections.Generic;
using UnityEngine;
using UnityEngine.Rendering;

namespace MagicCarpet.Native.Voxels
{
    // Exterior columns only: no volume allocation, collider, or per-cell object.
    // The 2m section-edge band is shared by both LODs, including periodic edges.
    internal static class VoxelTerrainSurface
    {
        internal const float SectionSize = 128f;
        internal const float NearCell = 1f;
        internal const float FarCell = 2f;
        private const float HeightStep = .5f;
        private const float Collar = 4f;

        internal static bool InPatchCollar(float x, float z, Bounds? patch) => patch.HasValue &&
            x > patch.Value.min.x - Collar && x < patch.Value.max.x + Collar &&
            z > patch.Value.min.z - Collar && z < patch.Value.max.z + Collar;

        private static float Column(float x, float z, Func<float, float, float> height)
        {
            // Border heights use the common far lattice in both representations.
            float sx = Mathf.Repeat(x, SectionSize), sz = Mathf.Repeat(z, SectionSize);
            if (sx < FarCell || sx >= SectionSize - FarCell || sz < FarCell || sz >= SectionSize - FarCell)
            {
                x = Mathf.Floor(x / FarCell) * FarCell + FarCell * .5f;
                z = Mathf.Floor(z / FarCell) * FarCell + FarCell * .5f;
            }
            return Mathf.Round(height(x, z) / HeightStep) * HeightStep;
        }

        internal static float SurfaceHeight(float x, float z, float cell, Func<float, float, float> height) =>
            Column(Mathf.Floor(x / cell) * cell + cell * .5f,
                Mathf.Floor(z / cell) * cell + cell * .5f, height);

        internal static void Build(Mesh destination, Mesh smooth, Vector2 origin, float cell,
            Func<float, float, float> height, Func<float, float, bool> water,
            Func<float, float, Color> color, Bounds? patch)
        {
            var vertices = new List<Vector3>(smooth.vertices);
            var normals = new List<Vector3>(smooth.normals);
            var colors = new List<Color>(smooth.colors);
            var uv = new List<Vector2>(smooth.uv);
            var land = new List<int>();
            var waterIndices = new List<int>(smooth.GetTriangles(1));
            var smoothLand = smooth.GetTriangles(0);
            var shared = new Dictionary<(Vector3 position, Vector3 normal, Color color), int>();
            bool InExpandedPatch(float x, float z) => InPatchCollar(x, z, patch);
            // Retain the original two-metre triangles as a collar around the
            // existing destructible patch and its source-height skirt.
            if (patch.HasValue)
                for (int i = 0; i < smoothLand.Length; i += 3)
                {
                    var p = (vertices[smoothLand[i]] + vertices[smoothLand[i + 1]] + vertices[smoothLand[i + 2]]) / 3f;
                    if (!InExpandedPatch(origin.x + p.x, origin.y + p.z)) continue;
                    land.Add(smoothLand[i]); land.Add(smoothLand[i + 1]); land.Add(smoothLand[i + 2]);
                }

            int Vertex(Vector3 p, Vector3 normal)
            {
                var tint = color(origin.x + p.x, origin.y + p.z);
                var key = (p, normal, tint);
                if (shared.TryGetValue(key, out var index)) return index;
                index = vertices.Count; shared.Add(key, index);
                vertices.Add(p); normals.Add(normal); colors.Add(tint);
                uv.Add(new Vector2(origin.x + p.x, origin.y + p.z));
                return index;
            }
            void Quad(Vector3 a, Vector3 b, Vector3 c, Vector3 d, Vector3 normal, bool twoSided = false)
            {
                int ia = Vertex(a, normal), ib = Vertex(b, normal), ic = Vertex(c, normal), id = Vertex(d, normal);
                land.Add(ia); land.Add(ib); land.Add(ic);
                land.Add(ia); land.Add(ic); land.Add(id);
                if (twoSided)
                {
                    land.Add(ic); land.Add(ib); land.Add(ia);
                    land.Add(id); land.Add(ic); land.Add(ia);
                }
            }
            int side = Mathf.RoundToInt(SectionSize / cell);
            for (int z = 0; z < side; z++) for (int x = 0; x < side; x++)
            {
                float x0 = x * cell, z0 = z * cell;
                float cx = origin.x + x0 + cell * .5f, cz = origin.y + z0 + cell * .5f;
                if (water(cx, cz) || InExpandedPatch(cx, cz)) continue;
                float h = Column(cx, cz, height);
                Quad(new Vector3(x0,h,z0), new Vector3(x0,h,z0+cell),
                    new Vector3(x0+cell,h,z0+cell), new Vector3(x0+cell,h,z0), Vector3.up);
                void Edge(float nx, float nz, Vector3 a, Vector3 b, Vector3 normal)
                {
                    bool collar = InExpandedPatch(nx, nz);
                    float lowA, lowB;
                    if (collar)
                    {
                        lowA = height(origin.x + a.x, origin.y + a.z);
                        lowB = height(origin.x + b.x, origin.y + b.z);
                    }
                    else { lowA = lowB = Column(nx, nz, height); if (lowA >= h) return; }
                    if (Mathf.Abs(h-lowA) < .00001f && Mathf.Abs(h-lowB) < .00001f) return;
                    Quad(a, new Vector3(a.x,lowA,a.z), new Vector3(b.x,lowB,b.z), b, normal, collar);
                }
                Edge(cx-cell,cz,new Vector3(x0,h,z0),new Vector3(x0,h,z0+cell),Vector3.left);
                Edge(cx+cell,cz,new Vector3(x0+cell,h,z0+cell),new Vector3(x0+cell,h,z0),Vector3.right);
                Edge(cx,cz-cell,new Vector3(x0+cell,h,z0),new Vector3(x0,h,z0),Vector3.back);
                Edge(cx,cz+cell,new Vector3(x0,h,z0+cell),new Vector3(x0+cell,h,z0+cell),Vector3.forward);
            }
            destination.Clear();
            destination.indexFormat = vertices.Count > 65535 ? IndexFormat.UInt32 : IndexFormat.UInt16;
            destination.SetVertices(vertices); destination.SetNormals(normals); destination.SetColors(colors); destination.SetUVs(0, uv);
            destination.subMeshCount = 2;
            destination.SetTriangles(land, 0, false); destination.SetTriangles(waterIndices, 1, false);
            destination.RecalculateBounds();
            if (waterIndices.Count != 0)
            {
                var bounds = destination.bounds; var datum = bounds.center; datum.y = .02f;
                bounds.Encapsulate(datum); bounds.Expand(new Vector3(6, 10.3f, 6)); destination.bounds = bounds;
            }
        }
    }
}
