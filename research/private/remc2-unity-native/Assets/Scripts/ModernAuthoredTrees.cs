using System;
using System.Collections.Generic;
using System.Runtime.InteropServices;
using UnityEngine;
using UnityEngine.Rendering;

namespace MagicCarpet.Native
{
    // Owns Level 1's live authored tree visuals. Positions and lifecycle come
    // exclusively from the native mission snapshot; the decorative forest is
    // deliberately separate and never substitutes for these objects.
    internal sealed class ModernAuthoredTrees : IDisposable
    {
        [StructLayout(LayoutKind.Sequential)]
        internal struct TreeObservation
        {
            internal uint abiVersion, structSize;
            internal ulong generation, tick;
            internal uint slotGeneration;
            internal ushort entityIndex, sourceId, authoredIndex, renderParameter, renderHeight;
            internal ushort x, y;
            internal short z;
            internal int life;
            internal uint controlFlags;
            internal byte classId, model, action, drawing;
        }

        [DllImport("sim_capi", CallingConvention = CallingConvention.Cdecl)]
        private static extern int mc_native_campaign_authored_trees_copy_v1(
            ulong handle, ulong generation, ulong tick, [Out] TreeObservation[] buffer,
            uint capacity, uint elementSize, out uint written);

        private sealed class Tree
        {
            internal GameObject Root;
            internal MeshRenderer[] Parts;
            internal MeshRenderer[] FarParts;
            internal TreeObservation Observation;
        }

        private readonly Transform parent;
        private readonly Dictionary<ushort, Tree> trees = new();
        private readonly HashSet<ushort> seen = new();
        private readonly HashSet<ushort> identities = new();
        private readonly TreeObservation[] observations = new TreeObservation[1000];
        private readonly Mesh[] modern = new Mesh[3];
        private readonly Mesh[] modernFar = new Mesh[3];
        private readonly Mesh[] voxel = new Mesh[3];
        private Mesh voxelFarLeaves;
        private readonly Material[] modernMaterials = new Material[3];
        private readonly Material[] burnedMaterials = new Material[2];
        private readonly Material voxelMaterial;
        private readonly Material voxelBurnedMaterial;
        private readonly float sourceHeight;

        internal ModernAuthoredTrees(Transform parent)
        {
            this.parent = parent != null ? parent : throw new ArgumentNullException(nameof(parent));
            string[] names = { "tree_trunk_mid", "tree_branches_mid", "tree_leaves_mid" };
            string[] textures = { "tree_small_02", "tree_small_02_branch", "tree_small_02_leaves" };
            var shader = Resources.Load<Shader>("ModernVegetation");
            var voxelShader = Shader.Find("MagicCarpet/VoxelSurface");
            if (shader == null || voxelShader == null) throw new InvalidOperationException("Missing tree presentation shader");
            var source = new Mesh[3];
            for (int i = 0; i < 3; i++)
            {
                source[i] = Resources.Load<Mesh>("ModernNature/" + names[i]);
                if (source[i] == null) throw new InvalidOperationException("Missing tree mesh " + names[i]);
            }
            // The original tree has a broad, dense dome and a substantial trunk.
            // Anchor every component at the source trunk's foot, thicken its
            // bole without spreading the upper forks, and interleave ten rotated
            // alpha-cutout canopies. Far LOD uses two canopies.
            var anchor = RootAnchor(source[0]);
            modern[0] = ComposeTrunk(source[0], anchor);
            modern[1] = Compose(source[1], 1.15f, 1, anchor);
            modern[2] = Compose(source[2], 1.2f, 10, anchor);
            modernFar[0] = modern[0];
            modernFar[1] = modern[1];
            modernFar[2] = Compose(source[2], 1.2f, 2, anchor);
            float top = 0;
            for (int i = 0; i < 3; i++)
            {
                top = Mathf.Max(top, modern[i].bounds.max.y);
                modernMaterials[i] = NatureMaterial(shader, textures[i], modern[i], i == 2);
            }
            sourceHeight = top;
            if (sourceHeight <= 0) throw new InvalidOperationException("Empty tree geometry");
            voxelMaterial = new Material(voxelShader) { name = "Authored tree voxel" };
            voxelBurnedMaterial = new Material(voxelShader) { name = "Authored tree charred voxel" };
            voxelBurnedMaterial.SetColor("_BaseColor", new Color(.24f, .22f, .20f, 1f));
            var lit = Shader.Find("Universal Render Pipeline/Lit");
            if (lit == null) throw new InvalidOperationException("Missing tree char shader");
            for (int i = 0; i < 2; i++)
            {
                burnedMaterials[i] = new Material(lit) { name = "Authored tree charred wood" };
                burnedMaterials[i].SetColor("_BaseColor", new Color(.075f, .061f, .052f, 1f));
                burnedMaterials[i].SetFloat("_Smoothness", .08f);
            }
        }

        internal void BeginFrame() => seen.Clear();

        internal void Present(NativeCampaignSession session, bool voxelStyle)
        {
            NativeSession.Check(mc_native_campaign_authored_trees_copy_v1(session.Handle,
                session.Generation, session.Tick, observations, (uint)observations.Length,
                (uint)Marshal.SizeOf<TreeObservation>(), out var count), "read authored trees");
            if (count > observations.Length) throw new InvalidOperationException("Tree census exceeds capacity");
            BeginFrame();
            identities.Clear();
            for (int i = 0; i < count; i++)
            {
                var row = observations[i];
                if (row.abiVersion != NativeSim.AbiVersion || row.structSize != 56 ||
                    row.generation != session.Generation || row.tick != session.Tick ||
                    !identities.Add(row.entityIndex))
                    throw new InvalidOperationException("Invalid authored tree identity");
                Observe(row, voxelStyle);
            }
            EndFrame();
        }

        // False preserves the source billboard for an unqualified selector.
        // Render uses the same wrapped coordinates as the campaign presenter.
        internal bool Observe(TreeObservation row, bool voxelStyle)
        {
            if (!IsTree(row)) return false;
            if (voxelStyle) EnsureVoxel();
            seen.Add(row.entityIndex);
            if (!trees.TryGetValue(row.entityIndex, out var tree))
            {
                tree = CreateTree(row.entityIndex);
                trees.Add(row.entityIndex, tree);
            }
            if (tree == null) throw new InvalidOperationException("Authored tree registry contains a null entry");
            tree.Observation = row;
            float height = row.renderHeight / 64f;
            tree.Root.transform.localScale = Vector3.one * (height / sourceHeight);
            // Vary the presentation deterministically without moving the native
            // actor. The source's two selectors retain their own identities.
            tree.Root.transform.rotation = Quaternion.Euler(0f, row.entityIndex * 137.50776f, 0f);
            bool active = row.drawing != 0;
            tree.Root.SetActive(active);
            bool dead = row.action == 2;
            for (int i = 0; i < 3; i++)
            {
                tree.Parts[i].GetComponent<MeshFilter>().sharedMesh = voxelStyle ? voxel[i] : modern[i];
                tree.Parts[i].sharedMaterial = voxelStyle
                    ? (dead ? voxelBurnedMaterial : voxelMaterial)
                    : (dead && i < 2 ? burnedMaterials[i] : modernMaterials[i]);
                tree.Parts[i].enabled = active && (!dead || i != 2);
            }
            for (int i = 0; i < 3; i++)
            {
                int source = i;
                tree.FarParts[i].GetComponent<MeshFilter>().sharedMesh = voxelStyle
                    ? (source == 2 ? voxelFarLeaves : voxel[source]) : modernFar[source];
                tree.FarParts[i].sharedMaterial = voxelStyle
                    ? (dead ? voxelBurnedMaterial : voxelMaterial)
                    : (dead && source < 2 ? burnedMaterials[source] : modernMaterials[source]);
                tree.FarParts[i].enabled = active && (source == 0 || source == 1 && dead || source == 2 && !dead);
            }
            return true;
        }

        internal static bool IsTree(TreeObservation row) =>
            row.classId == 2 && row.model == 0 && row.renderHeight > 0 &&
            ((row.action == 0 || row.action == 1) && (row.renderParameter == 83 || row.renderParameter == 84) ||
             row.action == 2 && (row.renderParameter == 226 || row.renderParameter == 227));

        internal bool Replaces(ushort slot, ushort sourceId) =>
            trees.TryGetValue(slot, out var tree) && tree != null && tree.Observation.sourceId == sourceId && tree.Observation.drawing != 0;

        internal void Render(Camera camera, bool visible)
        {
            var position = camera.transform.position;
            foreach (var tree in trees.Values)
            {
                var row = tree.Observation;
                var x = position.x + Mathf.Repeat(row.x / 64f - position.x + 512f, 1024f) - 512f;
                var z = position.z + Mathf.Repeat(row.y / 64f - position.z + 512f, 1024f) - 512f;
                tree.Root.transform.position = new Vector3(x, row.z / 64f, z);
                tree.Root.SetActive(visible && row.drawing != 0);
            }
        }

        internal void EndFrame()
        {
            var removed = new List<ushort>();
            foreach (var pair in trees) if (!seen.Contains(pair.Key)) removed.Add(pair.Key);
            foreach (var id in removed) { Destroy(trees[id].Root); trees.Remove(id); }
        }

        private Tree CreateTree(ushort id)
        {
            var tree = new Tree { Root = new GameObject("Authored tree " + id), Parts = new MeshRenderer[3], FarParts = new MeshRenderer[3] };
            tree.Root.transform.SetParent(parent, false);
            for (int i = 0; i < 3; i++)
            {
                var part = new GameObject(i == 0 ? "Trunk" : i == 1 ? "Branches" : "Leaves");
                part.transform.SetParent(tree.Root.transform, false);
                part.AddComponent<MeshFilter>();
                tree.Parts[i] = part.AddComponent<MeshRenderer>();
            }
            for (int i = 0; i < 3; i++)
            {
                var part = new GameObject(i == 0 ? "Far trunk" : i == 1 ? "Far branches" : "Far leaves");
                part.transform.SetParent(tree.Root.transform, false);
                part.AddComponent<MeshFilter>();
                tree.FarParts[i] = part.AddComponent<MeshRenderer>();
            }
            var lod = tree.Root.AddComponent<LODGroup>();
            lod.SetLODs(new[] { new LOD(.04f, tree.Parts), new LOD(.004f, tree.FarParts) });
            // Bounds are established before meshes are assigned on first Observe.
            lod.localReferencePoint = new Vector3(0f, sourceHeight * .5f, 0f);
            var bounds = modern[2].bounds;
            lod.size = Mathf.Max(sourceHeight, 2f * Mathf.Abs(bounds.min.x), 2f * Mathf.Abs(bounds.max.x),
                2f * Mathf.Abs(bounds.min.z), 2f * Mathf.Abs(bounds.max.z));
            return tree;
        }

        private static Vector3 RootAnchor(Mesh trunk)
        {
            var vertices = trunk.vertices;
            var limit = trunk.bounds.min.y + .2f;
            var sum = Vector3.zero;
            var count = 0;
            foreach (var vertex in vertices)
            {
                if (vertex.y >= limit) continue;
                sum.x += vertex.x; sum.z += vertex.z; count++;
            }
            if (count == 0) throw new InvalidOperationException("Tree trunk has no root vertices");
            return sum / count;
        }

        private static Mesh ComposeTrunk(Mesh source, Vector3 anchor)
        {
            var mesh = UnityEngine.Object.Instantiate(source);
            mesh.name = "Authored tree " + source.name;
            var vertices = mesh.vertices;
            var normals = mesh.normals;
            for (int i = 0; i < vertices.Length; i++)
            {
                var p = vertices[i] - anchor;
                var t = Mathf.Clamp01((p.y - source.bounds.min.y - .5f) / 1.8f);
                var scale = Mathf.Lerp(3.2f, 1.15f, t);
                var derivative = t > 0f && t < 1f ? (1.15f - 3.2f) / 1.8f : 0f;
                vertices[i] = new Vector3(p.x * scale, p.y, p.z * scale);
                if (normals.Length == vertices.Length)
                {
                    var n = normals[i];
                    normals[i] = new Vector3(n.x / scale,
                        n.y - derivative / scale * (p.x * n.x + p.z * n.z), n.z / scale).normalized;
                }
            }
            mesh.vertices = vertices;
            if (normals.Length == vertices.Length) mesh.normals = normals; else mesh.RecalculateNormals();
            mesh.RecalculateTangents();
            mesh.RecalculateBounds();
            return mesh;
        }

        // The input is retained as the editable master; this bounded recipe
        // creates one shared mesh, not per-instance geometry.
        private static Mesh Compose(Mesh source, float width, int copies, Vector3 anchor)
        {
            var instances = new CombineInstance[copies];
            for (int i = 0; i < copies; i++)
            {
                instances[i].mesh = source;
                instances[i].transform = Matrix4x4.TRS(Vector3.zero,
                    Quaternion.Euler(0f, 360f * i / copies, 0f), new Vector3(width, 1f, width)) *
                    Matrix4x4.Translate(-anchor);
            }
            var mesh = new Mesh { name = "Authored tree " + source.name,
                indexFormat = source.vertexCount * copies > 65535 ? IndexFormat.UInt32 : IndexFormat.UInt16 };
            mesh.CombineMeshes(instances, true, true, false);
            mesh.RecalculateBounds();
            return mesh;
        }

        private static Material NatureMaterial(Shader shader, string name, Mesh mesh, bool foliage)
        {
            var material = new Material(shader) { name = "Authored tree " + name, enableInstancing = true };
            material.SetFloat("_UseTextures", 1f);
            material.SetTexture("_BaseMap", Texture(name + "_diff_1k"));
            material.SetTexture("_NormalMap", Texture(name + "_nor_gl_1k"));
            material.SetTexture("_RoughnessMap", Texture(name + "_rough_1k"));
            material.SetFloat("_UseNormalMap", 1f); material.SetFloat("_UseRoughnessMap", 1f);
            material.SetFloat("_WindUseHeight", 1f); material.SetFloat("_WindMinY", mesh.bounds.min.y);
            material.SetFloat("_WindInvHeight", 1f / Mathf.Max(mesh.bounds.size.y, .001f));
            material.SetFloat("_WindStrength", foliage ? .12f : 0f);
            if (foliage) { material.SetTexture("_AlphaMap", Texture(name + "_alpha_1k")); material.SetFloat("_UseAlphaMap", 1f); }
            return material;
        }

        private static Texture2D Texture(string name) => Resources.Load<Texture2D>("ModernNature/" + name)
            ?? throw new InvalidOperationException("Missing tree texture " + name);

        private sealed class TextureSamples
        {
            internal Color32[] Pixels;
            internal int Width, Height;

            internal Color32 At(Vector2 uv)
            {
                int x = Mathf.Min(Width - 1, Mathf.FloorToInt(Mathf.Repeat(uv.x, 1f) * Width));
                int y = Mathf.Min(Height - 1, Mathf.FloorToInt(Mathf.Repeat(uv.y, 1f) * Height));
                return Pixels[y * Width + x];
            }
        }

        private void EnsureVoxel()
        {
            if (voxel[0] != null) return;
            TextureSamples bark = null, branches = null, leaves = null, alpha = null;
            if (SystemInfo.graphicsDeviceType != GraphicsDeviceType.Null)
            {
                bark = ReadTexture(Texture("tree_small_02_diff_1k"));
                branches = ReadTexture(Texture("tree_small_02_branch_diff_1k"));
                leaves = ReadTexture(Texture("tree_small_02_leaves_diff_1k"));
                alpha = ReadTexture(Texture("tree_small_02_leaves_alpha_1k"));
            }
            // Bake once per presenter, preserving the master mesh's albedo and
            // foliage cutout in the shared cell surfaces. The null-device path
            // permits geometry-only EditMode checks; players sample textures.
            var built = new Mesh[4];
            try
            {
                built[0] = MakeVoxelSurface(modern[0], .19f, new Color32(102, 73, 47, 255), bark, null);
                built[1] = MakeVoxelSurface(modern[1], .19f, new Color32(99, 76, 46, 255), branches, null);
                built[2] = MakeVoxelSurface(modern[2], .19f, new Color32(65, 105, 54, 255), leaves, alpha);
                built[3] = MakeVoxelSurface(modernFar[2], .29f, new Color32(65, 105, 54, 255), leaves, alpha);
                Array.Copy(built, voxel, voxel.Length);
                voxelFarLeaves = built[3];
            }
            catch
            {
                foreach (var mesh in built) Destroy(mesh);
                throw;
            }
        }

        private static TextureSamples ReadTexture(Texture2D texture)
        {
            const int resolution = 256;
            var target = RenderTexture.GetTemporary(resolution, resolution, 0, RenderTextureFormat.ARGB32);
            var previous = RenderTexture.active;
            Texture2D readable = null;
            try
            {
                Graphics.Blit(texture, target);
                RenderTexture.active = target;
                readable = new Texture2D(resolution, resolution, TextureFormat.RGBA32, false);
                readable.ReadPixels(new Rect(0, 0, resolution, resolution), 0, 0);
                readable.Apply();
                return new TextureSamples { Pixels = readable.GetPixels32(), Width = resolution, Height = resolution };
            }
            finally
            {
                RenderTexture.active = previous;
                RenderTexture.ReleaseTemporary(target);
                Destroy(readable);
            }
        }

        private static Mesh MakeVoxelSurface(Mesh source, float size, Color32 color,
            TextureSamples albedo, TextureSamples alpha)
        {
            var vertices = source.vertices;
            var uv = source.uv;
            var triangles = source.triangles;
            var cells = new Dictionary<Vector3Int, Color32>();
            // Sample triangle surfaces into a fixed spatial grid; neighboring
            // cells share no interior faces. Work and retained memory are bounded
            // by source triangles and this single fixed-resolution grid.
            for (int i = 0; i < triangles.Length; i += 3)
            {
                var a = vertices[triangles[i]]; var b = vertices[triangles[i + 1]]; var c = vertices[triangles[i + 2]];
                int steps = Mathf.Clamp(Mathf.CeilToInt(Mathf.Max((a-b).magnitude, Mathf.Max((b-c).magnitude, (c-a).magnitude)) / size), 1, 32);
                for (int u = 0; u <= steps; u++) for (int v = 0; v <= steps-u; v++)
                {
                    float fu = (float)u / steps, fv = (float)v / steps;
                    var p = a + (b-a) * fu + (c-a) * fv;
                    var tex = uv.Length == vertices.Length
                        ? uv[triangles[i]] + (uv[triangles[i+1]] - uv[triangles[i]]) * fu +
                          (uv[triangles[i+2]] - uv[triangles[i]]) * fv : Vector2.zero;
                    if (alpha != null && alpha.At(tex).r < 102) continue;
                    var cell = new Vector3Int(Mathf.FloorToInt(p.x/size), Mathf.FloorToInt(p.y/size), Mathf.FloorToInt(p.z/size));
                    if (!cells.ContainsKey(cell)) cells.Add(cell, albedo == null ? color : albedo.At(tex));
                }
            }
            if (cells.Count == 0) throw new InvalidOperationException("Tree voxel source produced no visible cells: " + source.name);
            Vector3Int[] directions = { Vector3Int.back, Vector3Int.forward, Vector3Int.left, Vector3Int.right, Vector3Int.up, Vector3Int.down };
            Vector3[] corners = { new Vector3(-1,-1,-1), new Vector3(1,-1,-1), new Vector3(1,1,-1), new Vector3(-1,1,-1), new Vector3(-1,-1,1), new Vector3(1,-1,1), new Vector3(1,1,1), new Vector3(-1,1,1) };
            int[,] faces = { {0,3,2,1}, {5,6,7,4}, {4,7,3,0}, {1,2,6,5}, {3,7,6,2}, {4,0,1,5} };
            var points = new List<Vector3>(); var normals = new List<Vector3>(); var colors = new List<Color32>(); var indices = new List<int>();
            foreach (var pair in cells) for (int f = 0; f < 6; f++)
            {
                var cell = pair.Key;
                if (cells.ContainsKey(cell + directions[f])) continue;
                int first = points.Count;
                for (int k = 0; k < 4; k++)
                {
                    points.Add(((Vector3)cell + Vector3.one * .5f + corners[faces[f,k]] * .5f) * size);
                    normals.Add((Vector3)directions[f]); colors.Add(pair.Value);
                }
                indices.Add(first); indices.Add(first+1); indices.Add(first+2); indices.Add(first); indices.Add(first+2); indices.Add(first+3);
            }
            var mesh = new Mesh { name = "Authored tree voxel " + source.name, indexFormat = points.Count > 65535 ? IndexFormat.UInt32 : IndexFormat.UInt16 };
            mesh.SetVertices(points); mesh.SetNormals(normals); mesh.SetColors(colors); mesh.SetTriangles(indices, 0); mesh.RecalculateBounds();
            return mesh;
        }

        private static void Destroy(UnityEngine.Object value)
        {
            if (value == null) return;
            if (Application.isPlaying) UnityEngine.Object.Destroy(value); else UnityEngine.Object.DestroyImmediate(value);
        }

        public void Dispose()
        {
            foreach (var tree in trees.Values) Destroy(tree.Root);
            trees.Clear();
            foreach (var mesh in modern) Destroy(mesh);
            Destroy(modernFar[2]);
            foreach (var mesh in voxel) Destroy(mesh);
            Destroy(voxelFarLeaves);
            foreach (var material in modernMaterials) Destroy(material);
            foreach (var material in burnedMaterials) Destroy(material);
            Destroy(voxelMaterial); Destroy(voxelBurnedMaterial);
        }
    }
}
