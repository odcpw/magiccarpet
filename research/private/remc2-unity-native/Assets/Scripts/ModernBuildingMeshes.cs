using System;
using System.Collections.Generic;
using UnityEngine;
using UnityEngine.Rendering;

namespace MagicCarpet.Native
{
    // A BUILD exterior is a sampled heightfield, not an invented building primitive.
    internal static class ModernBuildingMeshes
    {
        private const float Cell = 4f;
        private const int Stone = 0, Slate = 2, Wood = 3, Metal = 4, Glass = 5, Recess = 6;
        private const int MaxSamples = 65536;

        [Serializable]
        internal sealed class Profile
        {
            public int build_index, columns, rows, diagonal_parity;
            public bool terrain_only;
            public float[] heights;
        }

        internal static Vector2 Footprint(Profile profile)
        {
            Validate(profile);
            return new Vector2((profile.columns - 1) * Cell, (profile.rows - 1) * Cell);
        }

        internal static void Validate(Profile profile)
        {
            if (profile == null) throw new ArgumentNullException(nameof(profile));
            if (profile.columns < 2 || profile.rows < 2 ||
                (long)profile.columns * profile.rows > MaxSamples ||
                (profile.diagonal_parity != 0 && profile.diagonal_parity != 1))
                throw new ArgumentException("Building profile dimensions are invalid.", nameof(profile));
            if (profile.heights == null || profile.heights.Length != profile.columns * profile.rows)
                throw new ArgumentException("Building profile must have one height per sample.", nameof(profile));
            foreach (float h in profile.heights)
                if (float.IsNaN(h) || float.IsInfinity(h) || Mathf.Abs(h) > 1000000f)
                    throw new ArgumentException("Building profile contains an invalid height.", nameof(profile));
        }

        internal static Mesh Create(Profile profile, int lod)
        {
            Validate(profile);
            if (lod < 0 || lod > 2) throw new ArgumentOutOfRangeException(nameof(lod));
            var builder = new Builder();
            ForEachFacet(profile, (a, b, c) => builder.Triangle(a, b, c,
                Vector3.Cross(b - a, c - a).normalized.y < .76f || Mathf.Max(a.y,Mathf.Max(b.y,c.y))<.1f ? Stone : Slate));
            if (!profile.terrain_only)
                foreach (var window in FindWindows(profile)) builder.Window(window, lod);
            return builder.Finish("BUILD " + profile.build_index + " LOD" + lod);
        }

        // Quantize the same source facets into brick-sized exterior columns.
        // Only exposed top and side faces are emitted, with one mesh per LOD.
        internal static Mesh CreateVoxel(Profile profile, int lod)
        {
            Validate(profile);
            if (lod < 0 || lod > 2) throw new ArgumentOutOfRangeException(nameof(lod));
            float step = VoxelStep(profile, lod);
            int nx = Mathf.RoundToInt((profile.columns - 1) * Cell / step);
            int nz = Mathf.RoundToInt((profile.rows - 1) * Cell / step);
            if ((long)nx * nz > 1000000) throw new ArgumentException("Voxel building footprint is too large.", nameof(profile));
            var heights = new int[nx * nz];
            for (int z = 0; z < nz; z++) for (int x = 0; x < nx; x++)
                heights[z * nx + x] = Mathf.Max(0, Mathf.CeilToInt(SampleHeight(profile, (x + .5f) * step, (z + .5f) * step) / step));
            var builder = new Builder();
            float ox = nx * step * .5f, oz = nz * step * .5f;
            int Height(int x, int z) => x < 0 || z < 0 || x >= nx || z >= nz ? 0 : heights[z * nx + x];
            void Quad(Vector3 a, Vector3 b, Vector3 c, Vector3 d, int material)
            { builder.Triangle(a, b, c, material); builder.Triangle(a, c, d, material); }
            void Side(Vector3 a, Vector3 b, Vector3 c, Vector3 d, float bottom, float top)
            {
                if (top <= bottom) return;
                Quad(a, b, c, d, Stone);
            }
            for (int z = 0; z < nz; z++) for (int x = 0; x < nx; x++)
            {
                float top = Height(x, z) * step;
                if (top <= 0) continue;
                float x0 = x * step - ox, x1 = x0 + step, z0 = z * step - oz, z1 = z0 + step;
                Quad(new Vector3(x0, top, z0), new Vector3(x0, top, z1), new Vector3(x1, top, z1), new Vector3(x1, top, z0), Slate);
                float low = Mathf.Min(top, Height(x - 1, z) * step);
                Side(new Vector3(x0, low, z0), new Vector3(x0, low, z1), new Vector3(x0, top, z1), new Vector3(x0, top, z0), low, top);
                low = Mathf.Min(top, Height(x + 1, z) * step);
                Side(new Vector3(x1, low, z1), new Vector3(x1, low, z0), new Vector3(x1, top, z0), new Vector3(x1, top, z1), low, top);
                low = Mathf.Min(top, Height(x, z - 1) * step);
                Side(new Vector3(x1, low, z0), new Vector3(x0, low, z0), new Vector3(x0, top, z0), new Vector3(x1, top, z0), low, top);
                low = Mathf.Min(top, Height(x, z + 1) * step);
                Side(new Vector3(x0, low, z1), new Vector3(x1, low, z1), new Vector3(x1, top, z1), new Vector3(x0, top, z1), low, top);
            }
            foreach (var window in VoxelWindows(profile, step)) builder.Window(window, lod);
            return builder.Finish("Voxel BUILD " + profile.build_index + " LOD" + lod);
        }

        private static float SampleHeight(Profile p, float x, float z)
        {
            float u = x / Cell, v = z / Cell;
            int ix = Mathf.Min((int)u, p.columns - 2), iz = Mathf.Min((int)v, p.rows - 2);
            u -= ix; v -= iz;
            float a = p.heights[ix + iz * p.columns], b = p.heights[ix + 1 + iz * p.columns];
            float c = p.heights[ix + (iz + 1) * p.columns], d = p.heights[ix + 1 + (iz + 1) * p.columns];
            if (((ix + iz + p.diagonal_parity) & 1) == 0)
                return u < v ? a + (d - c) * u + (c - a) * v : a + (b - a) * u + (d - b) * v;
            return u + v < 1 ? a + (b - a) * u + (c - a) * v : d + (c - d) * (1 - u) + (b - d) * (1 - v);
        }

        private static float VoxelStep(Profile profile, int lod)
        {
            long fineColumns = (long)(profile.columns - 1) * (profile.rows - 1) * 256;
            float near = fineColumns <= 16384 ? .25f : fineColumns <= 262144 ? .5f :
                fineColumns <= 1048576 ? 1f : 2f;
            while (fineColumns / ((near * 4) * (near * 4)) > 65536) near *= 2;
            return lod == 0 ? near : lod == 1 ? Mathf.Max(.5f, near * 2) : Mathf.Max(2f, near * 4);
        }

        internal static Vector3[] WindowCenters(Profile profile)
        {
            Validate(profile);
            if (profile.terrain_only) return Array.Empty<Vector3>();
            var windows = FindWindows(profile);
            var result = new Vector3[windows.Count];
            for (int i = 0; i < result.Length; i++) result[i] = windows[i].Anchor;
            return result;
        }

        internal static Vector3[] WindowNormals(Profile profile)
        {
            Validate(profile);
            if (profile.terrain_only) return Array.Empty<Vector3>();
            var windows = FindWindows(profile);
            var result = new Vector3[windows.Count];
            for (int i = 0; i < result.Length; i++) result[i] = windows[i].normal;
            return result;
        }

        internal static Vector3[] VoxelWindowCenters(Profile profile, int lod)
        {
            Validate(profile);
            if (lod < 0 || lod > 2) throw new ArgumentOutOfRangeException(nameof(lod));
            float step = VoxelStep(profile, lod);
            var windows = VoxelWindows(profile, step); var result = new Vector3[windows.Count];
            for (var i = 0; i < result.Length; i++) result[i] = windows[i].Anchor;
            return result;
        }

        private static List<WindowSite> VoxelWindows(Profile profile, float step)
        {
            var result = new List<WindowSite>();
            if (profile.terrain_only) return result;
            float halfX = (profile.columns - 1) * Cell * .5f, halfZ = (profile.rows - 1) * Cell * .5f;
            foreach (var original in FindWindows(profile))
            {
                var window = original; float offset = step * 1.1f;
                // Keep every corner of every layer inside the source footprint.
                for (int x = -1; x <= 1; x += 2) for (int y = -1; y <= 1; y += 2)
                {
                    var corner = window.center + window.across * (x * window.halfWidth) + window.up * (y * window.halfHeight);
                    if (window.normal.x > 0) offset = Mathf.Min(offset, (halfX - corner.x) / window.normal.x - .05f);
                    if (window.normal.x < 0) offset = Mathf.Min(offset, (-halfX - corner.x) / window.normal.x - .05f);
                    if (window.normal.z > 0) offset = Mathf.Min(offset, (halfZ - corner.z) / window.normal.z - .05f);
                    if (window.normal.z < 0) offset = Mathf.Min(offset, (-halfZ - corner.z) / window.normal.z - .05f);
                }
                window.center += window.normal * Mathf.Max(0, offset);
                result.Add(window);
            }
            return result;
        }

        // Source diagonals alternate with the global lattice parity.
        private static void ForEachFacet(Profile p, Action<Vector3, Vector3, Vector3> add)
        {
            float ox = (p.columns - 1) * Cell * .5f, oz = (p.rows - 1) * Cell * .5f;
            for (int z = 0; z < p.rows - 1; z++)
            for (int x = 0; x < p.columns - 1; x++)
            {
                var a = new Vector3(x * Cell - ox, p.heights[x + z * p.columns], z * Cell - oz);
                var b = new Vector3((x + 1) * Cell - ox, p.heights[x + 1 + z * p.columns], z * Cell - oz);
                var c = new Vector3(x * Cell - ox, p.heights[x + (z + 1) * p.columns], (z + 1) * Cell - oz);
                var d = new Vector3((x + 1) * Cell - ox, p.heights[x + 1 + (z + 1) * p.columns], (z + 1) * Cell - oz);
                if (((x + z + p.diagonal_parity) & 1) == 0) { add(a, c, d); add(a, d, b); }
                else { add(a, c, b); add(b, c, d); }
            }
        }

        private struct WindowSite
        {
            internal Vector3 center, normal, across, up;
            internal float halfWidth, halfHeight;
            internal int sector;
            // Offset from the central lattice bar and within the visible glazing.
            internal Vector3 Anchor => center + across * (halfWidth * .27f) + up * (halfHeight * .27f) + normal * .025f;
        }

        private static List<WindowSite> FindWindows(Profile profile)
        {
            float min = float.PositiveInfinity, max = float.NegativeInfinity;
            foreach (float h in profile.heights) { min = Mathf.Min(min, h); max = Mathf.Max(max, h); }
            var candidates = new List<WindowSite>();
            var footprint = new Vector2((profile.columns - 1) * Cell, (profile.rows - 1) * Cell);
            ForEachFacet(profile, (a, b, c) =>
            {
                Vector3 n = Vector3.Cross(b - a, c - a).normalized;
                if (n.y > .68f || n.y < .00001f) return;
                Vector3 center = (a + b + c) / 3f;
                if (center.y < min + (max - min) * .12f || center.y > min + (max - min) * .7f) return;
                if (n.x * center.x + n.z * center.z <= .01f) return;
                float clearance = Mathf.Min(DistanceToLine(center, a, b),
                    Mathf.Min(DistanceToLine(center, b, c), DistanceToLine(center, c, a)));
                // Keep the normal offset and the whole rectangle inside the profile footprint.
                clearance = Mathf.Min(clearance, Mathf.Min(footprint.x * .5f - Mathf.Abs(center.x), footprint.y * .5f - Mathf.Abs(center.z)) - .06f);
                float halfWidth = Mathf.Min(.6f, clearance * .52f);
                float halfHeight = Mathf.Min(.9f, clearance * .72f);
                if (halfWidth < .12f || halfHeight < .12f) return;
                Vector3 across = Vector3.Cross(Vector3.up, n).normalized;
                Vector3 up = Vector3.Cross(n, across).normalized;
                int sector = Mathf.Abs(n.x) > Mathf.Abs(n.z) ? (n.x < 0 ? 0 : 1) : (n.z < 0 ? 2 : 3);
                candidates.Add(new WindowSite { center = center, normal = n, across = across, up = up,
                    halfWidth = halfWidth, halfHeight = halfHeight, sector = sector });
            });
            // Spread a few windows across available exterior directions, then fill gaps.
            var selected = new List<WindowSite>();
            for (int sector = 0; sector < 4; sector++)
                SelectCandidate(candidates, selected, sector);
            while (selected.Count < 6 && SelectCandidate(candidates, selected, -1)) { }
            return selected;
        }

        private static bool SelectCandidate(List<WindowSite> candidates, List<WindowSite> selected, int sector)
        {
            int best = -1;
            float bestScore = float.NegativeInfinity;
            for (int i = 0; i < candidates.Count; i++)
            {
                var candidate = candidates[i];
                if (sector >= 0 && candidate.sector != sector) continue;
                float nearest = 1000000f;
                foreach (var existing in selected) nearest = Mathf.Min(nearest, (candidate.center - existing.center).sqrMagnitude);
                if (nearest < 4f) continue;
                // Prefer low wall positions; spread the remaining windows apart.
                float score = Mathf.Min(nearest, 100f) - candidate.center.y * .01f;
                if (score > bestScore) { best = i; bestScore = score; }
            }
            if (best < 0) return false;
            selected.Add(candidates[best]);
            candidates.RemoveAt(best);
            return true;
        }

        private static float DistanceToLine(Vector3 p, Vector3 a, Vector3 b)
        {
            return Vector3.Cross(p - a, b - a).magnitude / (b - a).magnitude;
        }

        private sealed class Builder
        {
            private readonly List<Vector3> vertices = new List<Vector3>();
            private readonly List<Vector3> normals = new List<Vector3>();
            private readonly List<Vector2> uv = new List<Vector2>(), palette = new List<Vector2>();
            private readonly List<Color> colors = new List<Color>();
            private readonly List<int> indices = new List<int>();

            internal void Triangle(Vector3 a, Vector3 b, Vector3 c, int material)
            {
                var n = Vector3.Cross(b - a, c - a).normalized;
                int start = vertices.Count;
                Add(a, n, material); Add(b, n, material); Add(c, n, material);
                indices.Add(start); indices.Add(start + 1); indices.Add(start + 2);
            }

            private void Add(Vector3 p, Vector3 n, int material)
            {
                vertices.Add(p); normals.Add(n); uv.Add(new Vector2(p.x, p.z));
                palette.Add(new Vector2(material, 0)); colors.Add(Color.white);
            }

            private void Rectangle(WindowSite site, float x0, float y0, float x1, float y1, float depth, int material)
            {
                Vector3 a = site.center + site.across * x0 + site.up * y0 + site.normal * depth;
                Vector3 b = site.center + site.across * x1 + site.up * y0 + site.normal * depth;
                Vector3 c = site.center + site.across * x1 + site.up * y1 + site.normal * depth;
                Vector3 d = site.center + site.across * x0 + site.up * y1 + site.normal * depth;
                Triangle(a, b, c, material); Triangle(a, c, d, material);
            }

            internal void Window(WindowSite s, int lod)
            {
                float w = s.halfWidth, h = s.halfHeight;
                // Each layer is a millimetre-scale projection parallel to its host facet.
                Rectangle(s, -w, -h, w, h, .012f, Recess);
                Rectangle(s, -w * .78f, -h * .82f, w * .78f, h * .82f, .025f, Glass);
                if (lod == 2) return;
                float t = Mathf.Min(w, h) * .18f;
                Rectangle(s, -w, -h, -w + t, h, .035f, Wood);
                Rectangle(s, w - t, -h, w, h, .035f, Wood);
                Rectangle(s, -w, -h, w, -h + t, .035f, Wood);
                Rectangle(s, -w, h - t, w, h, .035f, Wood);
                if (lod != 0) return;
                Rectangle(s, -w * .045f, -h * .8f, w * .045f, h * .8f, .04f, Metal);
                Rectangle(s, -w * .75f, -h * .035f, w * .75f, h * .035f, .04f, Metal);
                Rectangle(s, -w, -h, w, -h + t * .55f, .045f, Stone);
            }

            internal Mesh Finish(string name)
            {
                var mesh = new Mesh { name = name };
                if (vertices.Count > 65535) mesh.indexFormat = IndexFormat.UInt32;
                mesh.SetVertices(vertices); mesh.SetNormals(normals); mesh.SetUVs(0, uv); mesh.SetUVs(1, palette);
                mesh.SetColors(colors); mesh.SetTriangles(indices, 0); mesh.RecalculateBounds();
                return mesh;
            }
        }
    }
}
