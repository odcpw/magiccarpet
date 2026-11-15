using System;
using System.Collections.Generic;
using UnityEngine;
using UnityEngine.Rendering;

namespace MagicCarpet.Native.Voxels
{
    // Authoring-time surface sampler. One shared exterior mesh is emitted per
    // kind/LOD; alpha-cutout cards never become filled opaque rectangles.
    internal static class VoxelVegetationMesh
    {
        internal const int KindCount = 7, LodCount = 3;
        private static readonly string[] Names = { "grass", "fern", "shrub", "tree_trunk", "rock", "tree_branches", "tree_leaves" };
        private static readonly string[] Textures = { "grass_medium_01", "fern_02", "shrub_02", "tree_small_02", "rock_moss_set_01", "tree_small_02_branch", "tree_small_02_leaves" };
        private static readonly int[] Resolutions = { 24, 16, 8 };
        internal static string ResourceName(int kind, int lod) => Names[kind] + "_" + Resolutions[lod];
        internal static string KindName(int kind) => Names[kind];
        internal static string ImpostorName(int kind) => Names[kind] + "_lod";
        internal static string TextureName(int kind) => Textures[kind];
        internal static int Resolution(int lod) => Resolutions[lod];
        internal static bool UsesAlpha(int kind) => kind < 3 || kind == 6;

        internal sealed class Samples
        {
            internal readonly Color32[] Pixels;
            internal readonly int Width, Height;
            internal Samples(Color32[] pixels, int width, int height)
            {
                if (pixels == null || width <= 0 || height <= 0 || pixels.Length != width * height)
                    throw new ArgumentException("Invalid texture samples");
                Pixels = pixels; Width = width; Height = height;
            }
            internal Color32 At(Vector2 uv)
            {
                int x = Mathf.Min(Width - 1, Mathf.FloorToInt(Mathf.Repeat(uv.x, 1f) * Width));
                int y = Mathf.Min(Height - 1, Mathf.FloorToInt(Mathf.Repeat(uv.y, 1f) * Height));
                return Pixels[x + y * Width];
            }
        }

        internal readonly struct Result
        {
            internal readonly Mesh Mesh;
            internal readonly int Cells, Faces, AlphaRejected, InteriorFacesCulled;
            internal readonly float CellSize;
            internal Result(Mesh mesh, int cells, int faces, int rejected, int culled, float size)
            { Mesh = mesh; Cells = cells; Faces = faces; AlphaRejected = rejected; InteriorFacesCulled = culled; CellSize = size; }
        }

        internal static Result Build(Mesh source, int resolution, Samples albedo, Samples alpha)
        {
            if (source == null || albedo == null) throw new ArgumentNullException(source == null ? nameof(source) : nameof(albedo));
            if (resolution < 4 || resolution > 64) throw new ArgumentOutOfRangeException(nameof(resolution));
            var positions = source.vertices; var uv = source.uv; var vertexColors = source.colors;
            if (positions.Length == 0 || uv.Length != positions.Length) throw new InvalidOperationException("Vegetation mesh lacks positions or UVs");
            var triangles = source.triangles;
            var bounds = source.bounds; var width = Mathf.Max(bounds.size.x, bounds.size.y, bounds.size.z);
            if (!float.IsFinite(width) || width <= 0) throw new InvalidOperationException("Vegetation mesh has empty bounds");
            var size = width / resolution;
            var cells = new Dictionary<Vector3Int, Color32>();
            int rejected = 0;
            for (int i = 0; i < triangles.Length; i += 3)
            {
                int ia = triangles[i], ib = triangles[i+1], ic = triangles[i+2];
                var a = positions[ia]; var b = positions[ib]; var c = positions[ic];
                int steps = Mathf.Clamp(Mathf.CeilToInt(Mathf.Max((a-b).magnitude, Mathf.Max((b-c).magnitude, (c-a).magnitude)) / size), 1, 64);
                for (int u = 0; u <= steps; u++) for (int v = 0; v <= steps-u; v++)
                {
                    float fu = (float)u / steps, fv = (float)v / steps;
                    var p = a + (b-a)*fu + (c-a)*fv;
                    var t = uv[ia] + (uv[ib]-uv[ia])*fu + (uv[ic]-uv[ia])*fv;
                    Color tint = vertexColors.Length == positions.Length
                        ? vertexColors[ia] + (vertexColors[ib]-vertexColors[ia])*fu + (vertexColors[ic]-vertexColors[ia])*fv : Color.white;
                    Color sample = albedo.At(t);
                    float opacity = sample.a * tint.a * (alpha == null ? 1f : alpha.At(t).r / 255f);
                    if (opacity < .4f) { rejected++; continue; }
                    var key = new Vector3Int(Mathf.FloorToInt(p.x/size), Mathf.FloorToInt(p.y/size), Mathf.FloorToInt(p.z/size));
                    if (!cells.ContainsKey(key))
                    {
                        var color = sample * tint; color.a = 1;
                        cells.Add(key, color);
                    }
                }
            }
            if (cells.Count == 0) throw new InvalidOperationException("Vegetation source has no opaque sampled cells: " + source.name);
            Vector3Int[] directions = { Vector3Int.back, Vector3Int.forward, Vector3Int.left, Vector3Int.right, Vector3Int.up, Vector3Int.down };
            Vector3[] corners = { new(-1,-1,-1), new(1,-1,-1), new(1,1,-1), new(-1,1,-1), new(-1,-1,1), new(1,-1,1), new(1,1,1), new(-1,1,1) };
            int[,] faces = { {0,3,2,1}, {5,6,7,4}, {4,7,3,0}, {1,2,6,5}, {3,7,6,2}, {4,0,1,5} };
            var points = new List<Vector3>(); var normals = new List<Vector3>(); var colors = new List<Color32>();
            var uvs = new List<Vector2>(); var indices = new List<int>(); int culled = 0;
            foreach (var pair in cells) for (int face = 0; face < 6; face++)
            {
                if (cells.ContainsKey(pair.Key + directions[face])) { culled++; continue; }
                int first = points.Count;
                for (int k = 0; k < 4; k++)
                {
                    points.Add(((Vector3)pair.Key + Vector3.one*.5f + corners[faces[face,k]]*.5f)*size);
                    normals.Add((Vector3)directions[face]); colors.Add(pair.Value); uvs.Add(Vector2.zero);
                }
                indices.Add(first); indices.Add(first+1); indices.Add(first+2);
                indices.Add(first); indices.Add(first+2); indices.Add(first+3);
            }
            var mesh = new Mesh { name = source.name + " voxel " + resolution,
                indexFormat = points.Count > 65535 ? IndexFormat.UInt32 : IndexFormat.UInt16 };
            mesh.SetVertices(points); mesh.SetNormals(normals); mesh.SetColors(colors); mesh.SetUVs(0, uvs);
            mesh.SetTriangles(indices, 0); mesh.RecalculateBounds();
            return new Result(mesh, cells.Count, indices.Count / 6, rejected, culled, size);
        }
    }
}
