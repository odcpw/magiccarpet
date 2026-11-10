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
        private readonly Mesh[] voxel = new Mesh[3];
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
            float top = 0;
            for (int i = 0; i < 3; i++)
            {
                modern[i] = Resources.Load<Mesh>("ModernNature/" + names[i]);
                if (modern[i] == null) throw new InvalidOperationException("Missing tree mesh " + names[i]);
                top = Mathf.Max(top, modern[i].bounds.max.y);
                modernMaterials[i] = NatureMaterial(shader, textures[i], modern[i], i == 2);
            }
            sourceHeight = top;
            if (sourceHeight <= 0) throw new InvalidOperationException("Empty tree geometry");
            // Bake the same three scanned components into shared cell surfaces.
            // A single mesh per component is reused by every authored tree.
            voxel[0] = MakeVoxelSurface(modern[0], .19f, new Color32(102, 73, 47, 255));
            voxel[1] = MakeVoxelSurface(modern[1], .19f, new Color32(99, 76, 46, 255));
            voxel[2] = MakeVoxelSurface(modern[2], .19f, new Color32(65, 105, 54, 255));
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
            seen.Add(row.entityIndex);
            if (!trees.TryGetValue(row.entityIndex, out var tree))
            {
                tree = CreateTree(row.entityIndex);
                trees.Add(row.entityIndex, tree);
            }
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
                tree.FarParts[i].GetComponent<MeshFilter>().sharedMesh = voxelStyle ? voxel[source] : modern[source];
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
            trees.TryGetValue(slot, out var tree) && tree.Observation.sourceId == sourceId && tree.Observation.drawing != 0;

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
            lod.size = sourceHeight;
            return tree;
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

        private static Mesh MakeVoxelSurface(Mesh source, float size, Color32 color)
        {
            var vertices = source.vertices;
            var triangles = source.triangles;
            var cells = new HashSet<Vector3Int>();
            // Sample triangle surfaces into a fixed spatial grid; neighboring
            // cells share no interior faces. Work and retained memory are bounded
            // by source triangles and this single fixed-resolution grid.
            for (int i = 0; i < triangles.Length; i += 3)
            {
                var a = vertices[triangles[i]]; var b = vertices[triangles[i + 1]]; var c = vertices[triangles[i + 2]];
                int steps = Mathf.Clamp(Mathf.CeilToInt(Mathf.Max((a-b).magnitude, Mathf.Max((b-c).magnitude, (c-a).magnitude)) / size), 1, 32);
                for (int u = 0; u <= steps; u++) for (int v = 0; v <= steps-u; v++)
                {
                    var p = a + (b-a) * ((float)u / steps) + (c-a) * ((float)v / steps);
                    cells.Add(new Vector3Int(Mathf.FloorToInt(p.x/size), Mathf.FloorToInt(p.y/size), Mathf.FloorToInt(p.z/size)));
                }
            }
            Vector3Int[] directions = { Vector3Int.back, Vector3Int.forward, Vector3Int.left, Vector3Int.right, Vector3Int.up, Vector3Int.down };
            Vector3[] corners = { new Vector3(-1,-1,-1), new Vector3(1,-1,-1), new Vector3(1,1,-1), new Vector3(-1,1,-1), new Vector3(-1,-1,1), new Vector3(1,-1,1), new Vector3(1,1,1), new Vector3(-1,1,1) };
            int[,] faces = { {0,3,2,1}, {5,6,7,4}, {4,7,3,0}, {1,2,6,5}, {3,7,6,2}, {4,0,1,5} };
            var points = new List<Vector3>(); var normals = new List<Vector3>(); var colors = new List<Color32>(); var indices = new List<int>();
            foreach (var cell in cells) for (int f = 0; f < 6; f++)
            {
                if (cells.Contains(cell + directions[f])) continue;
                int first = points.Count;
                for (int k = 0; k < 4; k++)
                {
                    points.Add(((Vector3)cell + Vector3.one * .5f + corners[faces[f,k]] * .5f) * size);
                    normals.Add((Vector3)directions[f]); colors.Add(color);
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
            foreach (var mesh in voxel) Destroy(mesh);
            foreach (var material in modernMaterials) Destroy(material);
            foreach (var material in burnedMaterials) Destroy(material);
            Destroy(voxelMaterial); Destroy(voxelBurnedMaterial);
        }
    }
}
