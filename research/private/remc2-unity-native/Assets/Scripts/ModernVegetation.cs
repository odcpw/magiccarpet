using System;
using System.Collections.Generic;
using UnityEngine;
using UnityEngine.Rendering;

namespace MagicCarpet.Native
{
    // Presentation only: these small trees and rocks are not authored, collidable scenery.
    // The terrain remains the sole authority for height and water.
    internal sealed class ModernVegetation : IDisposable
    {
        private const float Cell = 4f;
        private const int WrappedCells = 256;
        private const float DetailRadius = 115f;
        private const float LandmarkRadius = 200f;
        private const int CellsPerChunk = 8;
        private const float ChunkSize = Cell * CellsPerChunk;
        private const float LodDistance = 55f;
        private const float TreeLodDistance = 96f;
        private const float LodHysteresis = 8f;
        private readonly ModernTerrainMesh terrain;
        private readonly Material[] materials;
        private readonly Mesh[] meshes;
        private readonly Mesh[] distantMeshes;
        private readonly long[] detailTriangleCounts;
        private readonly long[] distantTriangleCounts;
        private readonly Dictionary<Vector2Int, Chunk> chunks = new Dictionary<Vector2Int, Chunk>();
        private readonly List<Vector2Int> expired = new List<Vector2Int>();
        private readonly Plane[] frustum = new Plane[6];
        private int centerX = int.MinValue;
        private int centerZ = int.MinValue;
        private ulong terrainRevision = ulong.MaxValue;
        private bool dirty = true;
        private bool disposed;

        internal long SubmittedTriangleCount { get; private set; }
        internal int DrawCount { get; private set; }
        internal int DetailInstanceCount { get; private set; }
        internal int DistantInstanceCount { get; private set; }

        private sealed class Chunk
        {
            internal readonly List<Matrix4x4>[] Instances = new List<Matrix4x4>[7];
            internal bool HasDetail;
            internal bool HasBounds;
            internal bool Distant;
            internal bool DistantTrees;
            internal bool ReflectionDistant;
            internal bool ReflectionDistantTrees;
            internal Bounds Bounds;

            internal Chunk()
            {
                for (var i = 0; i < Instances.Length; i++) Instances[i] = new List<Matrix4x4>();
            }
        }

        internal ModernVegetation(ModernTerrainMesh terrain)
        {
            this.terrain = terrain ?? throw new ArgumentNullException(nameof(terrain));
            var shader = Resources.Load<Shader>("ModernVegetation");
            if (shader == null)
                throw new InvalidOperationException("Missing Resources/ModernVegetation shader");
            meshes = new Mesh[7];
            distantMeshes = new Mesh[7];
            detailTriangleCounts = new long[7];
            distantTriangleCounts = new long[7];
            materials = new Material[7];
            try
            {
                meshes[0] = LoadScannedMesh("grass_medium_01", "grass_medium_01_mid_b_LOD0", 1.4f);
                meshes[1] = LoadScannedMesh("fern_02", "fern_02_b", 1.7f);
                meshes[2] = LoadScannedMesh("shrub_02", "shrub_02_b_LOD0", 1.7f);
                var treeBounds = ScannedTreeBounds();
                meshes[3] = LoadScannedMesh("tree_small_02_derived", "tree_trunk", 4.5f, treeBounds);
                meshes[4] = LoadScannedMesh("rock_moss_set_01", "rock_moss_set_01_rock03", 2.4f);
                meshes[5] = LoadScannedMesh("tree_small_02_derived", "tree_branches", 4.5f, treeBounds);
                meshes[6] = LoadScannedMesh("tree_small_02_derived", "tree_leaves", 4.5f, treeBounds);
                materials[0] = ScannedMaterial(shader, "grass_medium_01", meshes[0], true);
                materials[1] = ScannedMaterial(shader, "fern_02", meshes[1], true);
                materials[2] = ScannedMaterial(shader, "shrub_02", meshes[2], true);
                materials[3] = ScannedMaterial(shader, "tree_small_02", meshes[3], false);
                materials[4] = ScannedMaterial(shader, "rock_moss_set_01", meshes[4], false);
                materials[5] = ScannedMaterial(shader, "tree_small_02_branch", meshes[5], false);
                materials[5].SetFloat("_WindStrength", 0.03f);
                materials[6] = ScannedMaterial(shader, "tree_small_02_leaves", meshes[6], true);
                for (var i = 0; i < meshes.Length; i++)
                {
                    distantMeshes[i] = i == 6 ? MakeDistantLeafCards(meshes[i]) : MakeDistantMesh(meshes[i], i == 5 ? .01f : .045f);
                    detailTriangleCounts[i] = (long)meshes[i].GetIndexCount(0) / 3;
                    distantTriangleCounts[i] = (long)distantMeshes[i].GetIndexCount(0) / 3;
                }
            }
            catch
            {
                Dispose();
                throw;
            }
        }

        // Call after the source terrain planes change, even if the camera has not moved.
        internal void Invalidate() => dirty = true;

        internal void Render(Camera camera, bool reflection = false, float resolutionScale = 1f)
        {
            if (!reflection)
            {
                SubmittedTriangleCount = 0;
                DrawCount = 0;
                DetailInstanceCount = 0;
                DistantInstanceCount = 0;
            }
            if (disposed || camera == null || !SystemInfo.supportsInstancing) return;
            var position = camera.transform.position;
            RefreshChunks(position);
            GeometryUtility.CalculateFrustumPlanes(camera, frustum);
            foreach (var entry in chunks)
            {
                var distance = DistanceToChunk(position.x, position.z, entry.Key.x, entry.Key.y);
                if (distance > LandmarkRadius) continue;
                var visible = entry.Value.HasBounds && GeometryUtility.TestPlanesAABB(frustum, entry.Value.Bounds);
                if (reflection && !visible) continue;
                // Altitude and target resolution both affect projected size.
                // Reflection keeps its own hysteresis state and the same visible
                // radius, so distant shoreline trees retain their counterparts.
                var lodDistance = entry.Value.HasBounds ? Mathf.Sqrt(entry.Value.Bounds.SqrDistance(position)) : distance;
                if (reflection) lodDistance /= Mathf.Clamp(resolutionScale, .1f, 1f);
                var distant = reflection ? entry.Value.ReflectionDistant : entry.Value.Distant;
                var distantTrees = reflection ? entry.Value.ReflectionDistantTrees : entry.Value.DistantTrees;
                if (lodDistance >= LodDistance + LodHysteresis) distant = true;
                else if (lodDistance <= LodDistance - LodHysteresis) distant = false;
                if (lodDistance >= TreeLodDistance + LodHysteresis) distantTrees = true;
                else if (lodDistance <= TreeLodDistance - LodHysteresis) distantTrees = false;
                if (reflection)
                { entry.Value.ReflectionDistant = distant; entry.Value.ReflectionDistantTrees = distantTrees; }
                else
                { entry.Value.Distant = distant; entry.Value.DistantTrees = distantTrees; }
                for (var kind = 0; kind < meshes.Length; kind++)
                {
                    var treePart = kind == 3 || kind == 5 || kind == 6;
                    var useDistant = treePart ? distantTrees : distant;
                    // Landmark batches are still submitted offscreen: their shadows
                    // can fall into view even when their geometry does not.
                    if (!visible && kind < 3) continue;
                    if (kind < 3 && distance > DetailRadius) continue;
                    var list = entry.Value.Instances[kind];
                    if (list.Count == 0) continue;
                    if (!reflection)
                    {
                        DrawCount++;
                        SubmittedTriangleCount += (useDistant ? distantTriangleCounts[kind] : detailTriangleCounts[kind]) * list.Count;
                        if (useDistant) DistantInstanceCount += list.Count;
                        else DetailInstanceCount += list.Count;
                    }
                    // At most 256 tufts per chunk, and far fewer landmarks: no 1023 split
                    // and the batch bounds stay local for Unity's camera and shadow culling.
                    Graphics.DrawMeshInstanced(useDistant ? distantMeshes[kind] : meshes[kind], 0, materials[kind], list, null,
                        kind >= 3 && !reflection ? ShadowCastingMode.On : ShadowCastingMode.Off,
                        true, 0, camera, LightProbeUsage.Off);
                }
            }
        }

        // Kept independent of draw submission so terrain updates and residency can
        // be checked even when Unity's test runner has no instancing-capable GPU.
        internal void RefreshChunks(Vector3 position)
        {
            if (disposed) return;
            var x = Mathf.FloorToInt(position.x / ChunkSize);
            var z = Mathf.FloorToInt(position.z / ChunkSize);
            if (dirty || terrainRevision != terrain.Revision || x != centerX || z != centerZ)
            {
                if (dirty || terrainRevision != terrain.Revision) chunks.Clear();
                centerX = x;
                centerZ = z;
                UpdateChunks();
                terrainRevision = terrain.Revision;
                dirty = false;
            }
        }

        private static float DistanceToChunk(float x, float z, int chunkX, int chunkZ)
        {
            var dx = Mathf.Max(0f, Mathf.Abs(x - (chunkX + .5f) * ChunkSize) - ChunkSize * .5f);
            var dz = Mathf.Max(0f, Mathf.Abs(z - (chunkZ + .5f) * ChunkSize) - ChunkSize * .5f);
            return Mathf.Sqrt(dx * dx + dz * dz);
        }

        private void UpdateChunks()
        {
            // The whole camera chunk is the residency origin, so crossing its edge
            // changes only a narrow ring. Extra margin covers every position in it.
            var originX = (centerX + .5f) * ChunkSize;
            var originZ = (centerZ + .5f) * ChunkSize;
            var extent = Mathf.CeilToInt((LandmarkRadius + ChunkSize) / ChunkSize);
            expired.Clear();
            foreach (var entry in chunks)
                if (DistanceToChunk(originX, originZ, entry.Key.x, entry.Key.y) > LandmarkRadius + ChunkSize)
                    expired.Add(entry.Key);
            foreach (var key in expired) chunks.Remove(key);
            for (var dz = -extent; dz <= extent; dz++)
            for (var dx = -extent; dx <= extent; dx++)
            {
                var key = new Vector2Int(centerX + dx, centerZ + dz);
                var distance = DistanceToChunk(originX, originZ, key.x, key.y);
                if (distance > LandmarkRadius + ChunkSize) continue;
                var detail = distance <= DetailRadius + ChunkSize;
                if (chunks.TryGetValue(key, out var chunk) && (!detail || chunk.HasDetail)) continue;
                chunk = new Chunk { HasDetail = detail };
                PopulateChunk(key.x, key.y, chunk);
                chunks[key] = chunk;
            }
        }

        private void PopulateChunk(int chunkX, int chunkZ, Chunk chunk)
        {
            for (var dz = 0; dz < CellsPerChunk; dz++)
            for (var dx = 0; dx < CellsPerChunk; dx++)
            {
                var cellX = chunkX * CellsPerChunk + dx;
                var cellZ = chunkZ * CellsPerChunk + dz;
                // Hash wrapped cells, but keep positions in the camera's current world copy.
                var seed = Hash(cellX & (WrappedCells - 1), cellZ & (WrappedCells - 1));
                var x = (cellX + 0.12f + 0.76f * Random01(seed)) * Cell;
                var z = (cellZ + 0.12f + 0.76f * Random01(seed ^ 0x9e3779b9u)) * Cell;
                if (terrain.IsWater(x, z)) continue;
                var height = terrain.SampleHeight(x, z);
                var hx = terrain.SampleHeight(x + 1.5f, z) - terrain.SampleHeight(x - 1.5f, z);
                var hz = terrain.SampleHeight(x, z + 1.5f) - terrain.SampleHeight(x, z - 1.5f);
                var slope = Mathf.Sqrt(hx * hx + hz * hz) / 3f;
                var ground = terrain.GroundType(x, z);
                // Terrain selectors affect distribution, not an assumed mapping from IDs to biomes.
                var density = 0.78f + 0.22f * Random01((uint)ground * 7919u + 31u);
                var yaw = Random01(seed ^ 0xc2b2ae35u) * 360f;
                if (chunk.HasDetail && slope < 1.15f)
                {
                    // Several independently grounded tufts form irregular patches.
                    // A broad, wrapped moisture field leaves clearings between them.
                    var cover = density * Mathf.Lerp(.40f, .96f, PatchDensity(x, z));
                    for (var tuft = 0; tuft < 4; tuft++)
                    {
                        var tuftSeed = seed ^ ((uint)(tuft + 1) * 0x27d4eb2fu);
                        if (Random01(tuftSeed) > cover) continue;
                        var px = (cellX + Random01(tuftSeed ^ 0x68bc21ebu)) * Cell;
                        var pz = (cellZ + Random01(tuftSeed ^ 0x02e5be93u)) * Cell;
                        if (terrain.IsWater(px, pz)) continue;
                        var normal = GroundNormal(px, pz);
                        if (normal.y < .65f) continue;
                        var choice = Random01(tuftSeed ^ 0x85ebca6bu);
                        var kind = choice < .68f ? 0 : choice < .88f ? 1 : 2;
                        var scale = .85f + .75f * Random01(tuftSeed ^ 0x165667b1u);
                        Add(chunk, kind, px, terrain.SampleHeight(px, pz) - .05f, pz,
                            Random01(tuftSeed ^ 0xc2b2ae35u) * 360f, scale, normal);
                    }
                }
                var landmark = Random01(seed ^ 0x165667b1u);
                if (landmark < .045f * Mathf.Lerp(.5f, 1.3f, PatchDensity(x, z)) && slope < .55f)
                {
                    // Scanned trees follow the same terrain as ground cover, independently of actors.
                    var scale = .65f + .95f * Random01(seed ^ 0xd3a2646cu);
                    Add(chunk, 3, x, height - .08f, z, yaw, scale, Vector3.up);
                }
                else if (landmark > .88f && slope < 1.15f)
                {
                    var scale = .45f + 1.40f * Random01(seed ^ 0xfd7046c5u);
                    var normal = GroundNormal(x, z);
                    var anchor = new Vector3(x, height, z) - normal * (.24f * scale);
                    Add(chunk, 4, anchor.x, anchor.y, anchor.z, yaw, scale, normal);
                    // Talus gathers around boulders on supported ground. Sparse
                    // isolated boulders on near-vertical faces looked suspended.
                    if (chunk.HasDetail)
                    for (var fragment = 0; fragment < 3; fragment++)
                    {
                        var fragmentSeed = seed ^ ((uint)(fragment + 1) * 0x92d78269u);
                        var angle = Random01(fragmentSeed) * Mathf.PI * 2f;
                        var radius = scale * (1.1f + Random01(fragmentSeed ^ 0x68bc21ebu));
                        var px = x + Mathf.Cos(angle) * radius;
                        var pz = z + Mathf.Sin(angle) * radius;
                        if (terrain.IsWater(px, pz)) continue;
                        var fragmentNormal = GroundNormal(px, pz);
                        if (fragmentNormal.y < .70f) continue;
                        var fragmentScale = .12f + .28f * Random01(fragmentSeed ^ 0x02e5be93u);
                        var fragmentAnchor = new Vector3(px, terrain.SampleHeight(px, pz), pz)
                            - fragmentNormal * (.25f * fragmentScale);
                        Add(chunk, 4, fragmentAnchor.x, fragmentAnchor.y, fragmentAnchor.z,
                            Random01(fragmentSeed ^ 0xc2b2ae35u) * 360f, fragmentScale, fragmentNormal);
                    }
                }
            }
        }

        private Vector3 GroundNormal(float x, float z)
        {
            const float offset = .5f;
            return new Vector3(terrain.SampleHeight(x - offset, z) - terrain.SampleHeight(x + offset, z),
                2 * offset, terrain.SampleHeight(x, z - offset) - terrain.SampleHeight(x, z + offset)).normalized;
        }

        private static float PatchDensity(float x, float z)
        {
            const int cells = 64; // 16-metre patches repeat with the 1024-metre world.
            var px = x / 16f;
            var pz = z / 16f;
            var ix = Mathf.FloorToInt(px);
            var iz = Mathf.FloorToInt(pz);
            var fx = Mathf.SmoothStep(0, 1, px - ix);
            var fz = Mathf.SmoothStep(0, 1, pz - iz);
            return Mathf.Lerp(Mathf.Lerp(Random01(Hash(ix & (cells - 1), iz & (cells - 1))),
                Random01(Hash((ix + 1) & (cells - 1), iz & (cells - 1))), fx),
                Mathf.Lerp(Random01(Hash(ix & (cells - 1), (iz + 1) & (cells - 1))),
                Random01(Hash((ix + 1) & (cells - 1), (iz + 1) & (cells - 1))), fx), fz);
        }

        private void Add(Chunk chunk, int kind, float x, float y, float z, float yaw, float scale, Vector3 normal)
        {
            var matrix = Matrix4x4.TRS(new Vector3(x, y, z),
                Quaternion.FromToRotation(Vector3.up, normal) * Quaternion.Euler(0f, yaw, 0f), Vector3.one * scale);
            chunk.Instances[kind].Add(matrix);
            IncludeBounds(chunk, meshes[kind].bounds, matrix);
            if (kind == 3)
            {
                chunk.Instances[5].Add(matrix);
                chunk.Instances[6].Add(matrix);
                IncludeBounds(chunk, meshes[5].bounds, matrix);
                IncludeBounds(chunk, meshes[6].bounds, matrix);
            }
        }

        private static void IncludeBounds(Chunk chunk, Bounds local, Matrix4x4 matrix)
        {
            var center = matrix.MultiplyPoint3x4(local.center);
            var e = local.extents;
            var worldExtents = new Vector3(
                Mathf.Abs(matrix.m00) * e.x + Mathf.Abs(matrix.m01) * e.y + Mathf.Abs(matrix.m02) * e.z,
                Mathf.Abs(matrix.m10) * e.x + Mathf.Abs(matrix.m11) * e.y + Mathf.Abs(matrix.m12) * e.z,
                Mathf.Abs(matrix.m20) * e.x + Mathf.Abs(matrix.m21) * e.y + Mathf.Abs(matrix.m22) * e.z);
            // Allow for vertex-shader wind beyond the imported static bounds.
            var bounds = new Bounds(center, worldExtents * 2f + Vector3.one * 2f);
            if (chunk.HasBounds) chunk.Bounds.Encapsulate(bounds);
            else { chunk.Bounds = bounds; chunk.HasBounds = true; }
        }

        private static uint Hash(int x, int z)
        {
            unchecked
            {
                uint h = (uint)x * 0x8da6b343u ^ (uint)z * 0xd8163841u ^ 0xcb1ab31fu;
                h ^= h >> 16; h *= 0x7feb352du; h ^= h >> 15; h *= 0x846ca68bu; h ^= h >> 16;
                return h;
            }
        }

        private static float Random01(uint value)
        {
            unchecked
            {
                value ^= value >> 16; value *= 0x7feb352du; value ^= value >> 15;
                value *= 0x846ca68bu; value ^= value >> 16;
                return (value & 0x00ffffffu) / 16777216f;
            }
        }

        private static Material ScannedMaterial(Shader shader, string asset, Mesh mesh, bool foliage)
        {
            var result = new Material(shader) { name = "Poly Haven " + asset, enableInstancing = true };
            try
            {
            result.SetFloat("_UseTextures", 1f);
            result.SetTexture("_BaseMap", LoadTexture(asset + "_diff_1k"));
            result.SetTexture("_NormalMap", LoadTexture(asset + "_nor_gl_1k"));
            result.SetTexture("_RoughnessMap", LoadTexture(asset + "_rough_1k"));
            result.SetFloat("_UseNormalMap", 1f);
            result.SetFloat("_UseRoughnessMap", 1f);
            result.SetFloat("_WindUseHeight", 1f);
            result.SetFloat("_WindMinY", mesh.bounds.min.y);
            result.SetFloat("_WindInvHeight", 1f / Mathf.Max(mesh.bounds.size.y, 0.001f));
            result.SetFloat("_WindStrength", foliage ? 0.12f : 0f);
            if (foliage)
            {
                result.SetTexture("_AlphaMap", LoadTexture(asset + "_alpha_1k"));
                result.SetFloat("_UseAlphaMap", 1f);
            }
            return result;
            }
            catch
            {
                Destroy(result);
                throw;
            }
        }

        private static Texture2D LoadTexture(string name)
        {
            var texture = Resources.Load<Texture2D>("ModernNature/" + name);
            if (texture == null) throw new InvalidOperationException("Missing scanned nature texture: " + name);
            return texture;
        }

        // Cluster nearby vertices only when their texture coordinates also agree.
        // Keeping the original vertex attributes preserves the scanned materials,
        // normals and alpha cutouts; degenerate triangles disappear at distance.
        private static Mesh MakeDistantMesh(Mesh source, float cellFraction)
        {
            var result = UnityEngine.Object.Instantiate(source);
            try
            {
                result.name = source.name + " distant";
                var vertices = source.vertices;
                var uv = source.uv;
                var sourceTriangles = new int[source.subMeshCount][];
                var originalCount = 0;
                for (var sub = 0; sub < sourceTriangles.Length; sub++)
                {
                    sourceTriangles[sub] = source.GetTriangles(sub);
                    originalCount += sourceTriangles[sub].Length;
                }
                var width = Mathf.Max(source.bounds.size.x, Mathf.Max(source.bounds.size.y, source.bounds.size.z));
                var step = Mathf.Max(width * cellFraction, .0001f);
                var representatives = new Dictionary<(int, int, int, int, int), int>();
                var indices = new int[vertices.Length];
                for (var i = 0; i < vertices.Length; i++)
                {
                    var p = vertices[i] - source.bounds.min;
                    var tex = uv.Length == vertices.Length ? uv[i] : Vector2.zero;
                    var key = (Mathf.RoundToInt(p.x / step), Mathf.RoundToInt(p.y / step),
                        Mathf.RoundToInt(p.z / step), Mathf.RoundToInt(tex.x * 32f), Mathf.RoundToInt(tex.y * 32f));
                    if (!representatives.TryGetValue(key, out var representative))
                    {
                        representative = i;
                        representatives.Add(key, i);
                    }
                    indices[i] = representative;
                }
                var reduced = new int[sourceTriangles.Length][];
                var reducedCount = 0;
                for (var sub = 0; sub < sourceTriangles.Length; sub++)
                {
                    var triangles = sourceTriangles[sub];
                    var kept = new List<int>(triangles.Length);
                    for (var i = 0; i < triangles.Length; i += 3)
                    {
                        var a = indices[triangles[i]];
                        var b = indices[triangles[i + 1]];
                        var c = indices[triangles[i + 2]];
                        if (a == b || b == c || c == a) continue;
                        kept.Add(a); kept.Add(b); kept.Add(c);
                    }
                    reduced[sub] = kept.ToArray();
                    reducedCount += kept.Count;
                }
                // Never discard a mesh if a pathological input collapses its silhouette.
                var candidateCount = reducedCount;
                if (reducedCount >= originalCount / 3)
                    for (var sub = 0; sub < reduced.Length; sub++) result.SetTriangles(reduced[sub], sub, false);
                else reducedCount = originalCount;
                Debug.Log($"MC_MODERN_NATURE_LOD name={source.name} triangles={originalCount / 3}->{reducedCount / 3} candidate={candidateCount / 3}");
                return result;
            }
            catch
            {
                Destroy(result);
                throw;
            }
        }

        // The scanned crown is thousands of disconnected, two-triangle alpha
        // cards. Collapse would destroy their UV silhouettes, so pair their shared
        // edges and retain whole cards across spatial cells and three orientations.
        private static Mesh MakeDistantLeafCards(Mesh source)
        {
            var result = UnityEngine.Object.Instantiate(source);
            try
            {
                result.name = source.name + " distant";
                var vertices = source.vertices;
                var positions = new Dictionary<Vector3, int>();
                var welded = new int[vertices.Length];
                for (var i = 0; i < vertices.Length; i++)
                {
                    if (!positions.TryGetValue(vertices[i], out var id))
                    {
                        id = positions.Count;
                        positions.Add(vertices[i], id);
                    }
                    welded[i] = id;
                }
                var width = Mathf.Max(source.bounds.size.x, Mathf.Max(source.bounds.size.y, source.bounds.size.z));
                var step = Mathf.Max(width * .035f, .0001f);
                var originalCount = 0;
                var reducedCount = 0;
                var pairedCount = 0;
                for (var sub = 0; sub < source.subMeshCount; sub++)
                {
                    var triangles = source.GetTriangles(sub);
                    var count = triangles.Length / 3;
                    originalCount += count;
                    var pairs = new int[count];
                    for (var i = 0; i < count; i++) pairs[i] = -1;
                    var edges = new Dictionary<(int, int), int>();
                    for (var tri = 0; tri < count; tri++)
                    {
                        for (var edge = 0; edge < 3 && pairs[tri] < 0; edge++)
                        {
                            var a = welded[triangles[tri * 3 + edge]];
                            var b = welded[triangles[tri * 3 + (edge + 1) % 3]];
                            if (a == b) continue;
                            var key = a < b ? (a, b) : (b, a);
                            if (edges.TryGetValue(key, out var other) && pairs[other] < 0)
                            {
                                var n = TriangleNormal(vertices, triangles, tri);
                                var m = TriangleNormal(vertices, triangles, other);
                                if (Vector3.Dot(n, m) > .995f)
                                {
                                    pairs[tri] = other;
                                    pairs[other] = tri;
                                    pairedCount += 2;
                                }
                            }
                            else if (!edges.ContainsKey(key)) edges.Add(key, tri);
                        }
                    }
                    var winners = new Dictionary<(int, int, int, int), (int triangle, float score)>();
                    var extremes = new int[6];
                    var extremeValues = new float[6];
                    for (var i = 0; i < extremes.Length; i++) { extremes[i] = -1; extremeValues[i] = float.NegativeInfinity; }
                    for (var tri = 0; tri < count; tri++)
                    {
                        var other = pairs[tri];
                        if (other < 0 || other < tri) continue;
                        var center = Vector3.zero;
                        for (var v = 0; v < 3; v++)
                        {
                            var a = vertices[triangles[tri * 3 + v]];
                            var b = vertices[triangles[other * 3 + v]];
                            center += a + b;
                            IncludeCardExtremes(a, tri, extremes, extremeValues);
                            IncludeCardExtremes(b, tri, extremes, extremeValues);
                        }
                        center /= 6f;
                        var n = TriangleNormal(vertices, triangles, tri);
                        var ax = Mathf.Abs(n.x); var ay = Mathf.Abs(n.y); var az = Mathf.Abs(n.z);
                        var axis = ax >= ay && ax >= az ? 0 : ay >= az ? 1 : 2;
                        var relative = center - source.bounds.min;
                        var cell = (Mathf.FloorToInt(relative.x / step), Mathf.FloorToInt(relative.y / step),
                            Mathf.FloorToInt(relative.z / step), axis);
                        var score = (center - source.bounds.center).sqrMagnitude;
                        if (!winners.TryGetValue(cell, out var winner) || score > winner.score)
                            winners[cell] = (tri, score);
                    }
                    var selected = new HashSet<int>();
                    foreach (var winner in winners.Values) selected.Add(winner.triangle);
                    foreach (var extreme in extremes) if (extreme >= 0) selected.Add(extreme);
                    var kept = new List<int>(triangles.Length);
                    for (var tri = 0; tri < count; tri++)
                    {
                        var other = pairs[tri];
                        if (other >= 0 && (other < tri || !selected.Contains(tri))) continue;
                        for (var v = 0; v < 3; v++) kept.Add(triangles[tri * 3 + v]);
                        if (other >= 0)
                            for (var v = 0; v < 3; v++) kept.Add(triangles[other * 3 + v]);
                    }
                    reducedCount += kept.Count / 3;
                    result.SetTriangles(kept, sub, false);
                }
                Debug.Log($"MC_MODERN_NATURE_LOD name={source.name} triangles={originalCount}->{reducedCount} paired={pairedCount}");
                return result;
            }
            catch
            {
                Destroy(result);
                throw;
            }
        }

        private static void IncludeCardExtremes(Vector3 point, int triangle, int[] extremes, float[] values)
        {
            for (var axis = 0; axis < 3; axis++)
            for (var side = 0; side < 2; side++)
            {
                var index = axis * 2 + side;
                var value = point[axis] * (side == 0 ? -1f : 1f);
                if (value > values[index]) { values[index] = value; extremes[index] = triangle; }
            }
        }

        private static Vector3 TriangleNormal(Vector3[] vertices, int[] triangles, int triangle)
        {
            var a = vertices[triangles[triangle * 3]];
            var b = vertices[triangles[triangle * 3 + 1]];
            var c = vertices[triangles[triangle * 3 + 2]];
            return Vector3.Cross(b - a, c - a).normalized;
        }

        // FBX collections contain separate objects with display offsets. Bake the
        // selected object's import transform, then center its base on the terrain.
        private static Mesh LoadScannedMesh(string asset, string part, float targetWidth, Bounds? sharedBounds = null)
        {
            var resourceName = asset.EndsWith("_derived", StringComparison.Ordinal) ? asset : asset + "_1k";
            var prefab = Resources.Load<GameObject>("ModernNature/" + resourceName);
            if (prefab == null) throw new InvalidOperationException("Missing scanned nature model: " + asset);
            MeshFilter selected = null;
            foreach (var filter in prefab.GetComponentsInChildren<MeshFilter>(true))
                if (filter.name == part) { selected = filter; break; }
            if (selected == null || selected.sharedMesh == null)
                throw new InvalidOperationException("Missing scanned nature mesh: " + part);
            var source = selected.sharedMesh;
            var mesh = UnityEngine.Object.Instantiate(source);
            try
            {
            mesh.name = "Scanned " + part;
            var transform = selected.transform.localToWorldMatrix;
            var normalTransform = transform.inverse.transpose;
            var vertices = mesh.vertices;
            var normals = mesh.normals;
            var tangents = mesh.tangents;
            for (var i = 0; i < vertices.Length; i++)
            {
                vertices[i] = transform.MultiplyPoint3x4(vertices[i]);
                if (normals.Length == vertices.Length) normals[i] = normalTransform.MultiplyVector(normals[i]).normalized;
                if (tangents.Length == vertices.Length)
                {
                    var t = transform.MultiplyVector(new Vector3(tangents[i].x, tangents[i].y, tangents[i].z)).normalized;
                    tangents[i] = new Vector4(t.x, t.y, t.z, tangents[i].w * (transform.determinant < 0f ? -1f : 1f));
                }
            }
            if (transform.determinant < 0f)
            {
                for (var sub = 0; sub < mesh.subMeshCount; sub++)
                {
                    var triangles = mesh.GetTriangles(sub);
                    for (var i = 0; i < triangles.Length; i += 3)
                    { var swap = triangles[i]; triangles[i] = triangles[i + 1]; triangles[i + 1] = swap; }
                    mesh.SetTriangles(triangles, sub);
                }
            }
            mesh.vertices = vertices;
            mesh.RecalculateBounds();
            var bounds = sharedBounds ?? mesh.bounds;
            var width = Mathf.Max(bounds.size.x, bounds.size.z);
            if (width < 0.0001f) throw new InvalidOperationException("Empty scanned nature mesh: " + part);
            var scale = targetWidth / width;
            var center = new Vector3(bounds.center.x, bounds.min.y, bounds.center.z);
            var colors = new Color[vertices.Length];
            for (var i = 0; i < vertices.Length; i++) { vertices[i] = (vertices[i] - center) * scale; colors[i] = Color.white; }
            mesh.vertices = vertices;
            mesh.colors = colors;
            if (normals.Length == vertices.Length) mesh.normals = normals; else mesh.RecalculateNormals();
            if (tangents.Length == vertices.Length) mesh.tangents = tangents; else mesh.RecalculateTangents();
            mesh.RecalculateBounds();
            ulong triangleCount = 0;
            for (var sub = 0; sub < mesh.subMeshCount; sub++) triangleCount += mesh.GetIndexCount(sub) / 3;
            Debug.Log($"MC_MODERN_NATURE_MESH name={part} vertices={mesh.vertexCount} triangles={triangleCount}");
            return mesh;
            }
            catch
            {
                Destroy(mesh);
                throw;
            }
        }

        // All three scanned tree parts must share one origin and scale.
        private static Bounds ScannedTreeBounds()
        {
            var prefab = Resources.Load<GameObject>("ModernNature/tree_small_02_derived");
            if (prefab == null) throw new InvalidOperationException("Missing scanned nature model: tree_small_02_derived");
            var found = false;
            var bounds = new Bounds();
            foreach (var filter in prefab.GetComponentsInChildren<MeshFilter>(true))
            {
                if (filter.sharedMesh == null || (filter.name != "tree_trunk" &&
                    filter.name != "tree_branches" && filter.name != "tree_leaves")) continue;
                var part = filter.sharedMesh.bounds;
                var matrix = filter.transform.localToWorldMatrix;
                for (var x = -1; x <= 1; x += 2)
                for (var y = -1; y <= 1; y += 2)
                for (var z = -1; z <= 1; z += 2)
                {
                    var point = matrix.MultiplyPoint3x4(part.center + Vector3.Scale(part.extents, new Vector3(x, y, z)));
                    if (!found) { bounds = new Bounds(point, Vector3.zero); found = true; }
                    else bounds.Encapsulate(point);
                }
            }
            if (!found) throw new InvalidOperationException("Missing scanned tree meshes");
            return bounds;
        }

        private sealed class Geometry
        {
            private readonly List<Vector3> vertices = new List<Vector3>();
            private readonly List<Color> colors = new List<Color>();
            private readonly List<Vector2> uv = new List<Vector2>();
            private readonly List<int> triangles = new List<int>();

            internal void Triangle(Vector3 a, Vector3 b, Vector3 c, Color color, float wa, float wb, float wc)
            {
                var index = vertices.Count;
                vertices.Add(a); vertices.Add(b); vertices.Add(c);
                colors.Add(color); colors.Add(color); colors.Add(color);
                uv.Add(new Vector2(0, wa)); uv.Add(new Vector2(0, wb)); uv.Add(new Vector2(0, wc));
                triangles.Add(index); triangles.Add(index + 1); triangles.Add(index + 2);
            }

            internal Mesh Finish(string name)
            {
                var mesh = new Mesh { name = name };
                try
                {
                    mesh.SetVertices(vertices); mesh.SetColors(colors); mesh.SetUVs(0, uv); mesh.SetTriangles(triangles, 0);
                    mesh.RecalculateNormals(); mesh.RecalculateBounds();
                    return mesh;
                }
                catch
                {
                    Destroy(mesh);
                    throw;
                }
            }
        }

        private static Mesh MakeGrass()
        {
            var g = new Geometry();
            var palette = new[] { new Color(0.24f, 0.38f, 0.29f), new Color(0.38f, 0.44f, 0.30f), new Color(0.30f, 0.42f, 0.37f) };
            for (var i = 0; i < 17; i++)
            {
                var a = i * 2.39996f;
                var r = 0.12f + 0.50f * Random01((uint)i + 17u);
                var root = new Vector3(Mathf.Cos(a) * r, 0, Mathf.Sin(a) * r);
                var side = new Vector3(-Mathf.Sin(a), 0, Mathf.Cos(a)) * (0.055f + 0.045f * Random01((uint)i + 91u));
                var tip = root + new Vector3(Mathf.Cos(a) * 0.30f + 0.20f, 0.65f + 0.65f * Random01((uint)i + 81u), Mathf.Sin(a) * 0.30f);
                g.Triangle(root - side, tip, root + side, palette[i % palette.Length], 0, 1, 0);
            }
            return g.Finish("Moor grass clump");
        }

        private static Mesh MakeFern()
        {
            var g = new Geometry();
            var green = new Color(0.25f, 0.39f, 0.31f);
            for (var n = 0; n < 6; n++)
            {
                var angle = n * Mathf.PI / 3f;
                var dir = new Vector3(Mathf.Cos(angle), 0, Mathf.Sin(angle));
                var side = new Vector3(-dir.z, 0, dir.x);
                var root = new Vector3(0, 0.06f, 0);
                var tip = dir * 0.86f + Vector3.up * 0.75f;
                g.Triangle(root - side * 0.025f, tip, root + side * 0.025f, green * 0.75f, 0, 1, 0);
                for (var j = 1; j <= 4; j++)
                {
                    var t = j / 5f;
                    var center = Vector3.Lerp(root, tip, t) + Vector3.up * (0.18f * Mathf.Sin(t * Mathf.PI));
                    var next = Vector3.Lerp(root, tip, t + 0.17f);
                    var width = 0.28f * (1f - t * 0.7f);
                    g.Triangle(center, next, center + side * width + dir * 0.09f, green, t, t + 0.17f, t);
                    g.Triangle(center, center - side * width + dir * 0.09f, next, green * 1.12f, t, t, t + 0.17f);
                }
            }
            return g.Finish("Wind fern");
        }

        private static Mesh MakeShrub()
        {
            var g = new Geometry();
            var colors = new[] { new Color(0.29f, 0.36f, 0.30f), new Color(0.39f, 0.41f, 0.32f), new Color(0.33f, 0.35f, 0.40f) };
            for (var i = 0; i < 11; i++)
            {
                var a = i * 2.39996f;
                var dir = new Vector3(Mathf.Cos(a), 0, Mathf.Sin(a));
                var side = new Vector3(-dir.z, 0, dir.x);
                var basePos = dir * 0.12f;
                var tip = dir * (0.5f + 0.18f * Random01((uint)i + 10u)) + Vector3.up * (0.65f + 0.45f * Random01((uint)i + 30u));
                var mid = Vector3.Lerp(basePos, tip, 0.55f);
                g.Triangle(basePos, tip, mid + side * 0.32f, colors[i % 3], 0, 1, 0.55f);
                g.Triangle(basePos, mid - side * 0.32f, tip, colors[(i + 1) % 3], 0, 0.55f, 1);
            }
            return g.Finish("Moor shrub");
        }

        private static Mesh MakeTree()
        {
            var g = new Geometry();
            var bark = new Color(0.25f, 0.23f, 0.22f);
            const int sides = 7;
            for (var i = 0; i < sides; i++)
            {
                var a = i * Mathf.PI * 2f / sides;
                var b = (i + 1) * Mathf.PI * 2f / sides;
                var p = new Vector3(Mathf.Cos(a) * 0.24f, 0, Mathf.Sin(a) * 0.24f);
                var q = new Vector3(Mathf.Cos(b) * 0.24f, 0, Mathf.Sin(b) * 0.24f);
                var top = new Vector3(0.35f, 4.4f, 0);
                g.Triangle(p, top, q, bark * (0.85f + 0.05f * i), 0, 0.25f, 0);
            }
            var needles = new[] { new Color(0.16f, 0.29f, 0.27f), new Color(0.21f, 0.33f, 0.29f), new Color(0.27f, 0.37f, 0.33f) };
            for (var tier = 0; tier < 4; tier++)
            {
                var y = 1.05f + tier * 0.78f;
                var radius = 1.35f - tier * 0.23f;
                var apex = new Vector3(0.18f + tier * 0.07f, y + 1.40f, 0);
                for (var i = 0; i < sides; i++)
                {
                    var a = (i + tier * 0.23f) * Mathf.PI * 2f / sides;
                    var b = (i + 1 + tier * 0.23f) * Mathf.PI * 2f / sides;
                    var p = new Vector3(Mathf.Cos(a) * radius, y + (i % 2) * 0.12f, Mathf.Sin(a) * radius);
                    var q = new Vector3(Mathf.Cos(b) * radius, y + ((i + 1) % 2) * 0.12f, Mathf.Sin(b) * radius);
                    g.Triangle(p, apex, q, needles[(tier + i) % 3], y / 5f, 1, y / 5f);
                    g.Triangle(q, new Vector3(0.18f, y + 0.24f, 0), p, needles[0] * 0.75f, y / 5f, y / 5f, y / 5f);
                }
            }
            // Uneven side boughs break up the regular layered silhouette.
            for (var branch = 0; branch < 3; branch++)
            {
                var angle = branch * 2.4f + 0.4f;
                var dir = new Vector3(Mathf.Cos(angle), 0, Mathf.Sin(angle));
                var side = new Vector3(-dir.z, 0, dir.x);
                var root = Vector3.up * (1.25f + branch * 0.68f);
                var tip = root + dir * (1.48f - branch * 0.12f) + Vector3.up * 0.38f;
                var fork = tip - dir * 0.37f;
                g.Triangle(root - side * 0.07f, tip, root + side * 0.07f, bark, 0.2f, 0.55f, 0.2f);
                g.Triangle(fork - side * 0.43f, tip + Vector3.up * 0.48f, fork + side * 0.43f,
                    needles[(branch + 1) % 3], 0.45f, 0.7f, 0.45f);
                g.Triangle(fork - dir * 0.25f, tip + Vector3.up * 0.48f, tip + dir * 0.30f,
                    needles[branch % 3], 0.45f, 0.7f, 0.55f);
            }
            return g.Finish("Decorative windswept conifer");
        }

        private static Mesh MakeRock()
        {
            var g = new Geometry();
            var colors = new[] { new Color(0.34f, 0.36f, 0.39f), new Color(0.40f, 0.41f, 0.42f), new Color(0.29f, 0.32f, 0.34f) };
            const int sides = 7;
            var top = new Vector3(0.16f, 0.95f, -0.08f);
            for (var i = 0; i < sides; i++)
            {
                var a = i * Mathf.PI * 2f / sides;
                var b = (i + 1) * Mathf.PI * 2f / sides;
                var r = 0.85f + 0.28f * Random01((uint)i + 37u);
                var s = 0.85f + 0.28f * Random01((uint)((i + 1) % sides) + 37u);
                var p = new Vector3(Mathf.Cos(a) * r, 0, Mathf.Sin(a) * r * 0.75f);
                var q = new Vector3(Mathf.Cos(b) * s, 0, Mathf.Sin(b) * s * 0.75f);
                var shoulder = (p + q) * 0.31f + Vector3.up * (0.55f + 0.2f * Random01((uint)i + 11u));
                g.Triangle(p, shoulder, q, colors[i % 3], 0, 0, 0);
                g.Triangle(p, top, shoulder, colors[(i + 1) % 3], 0, 0, 0);
                g.Triangle(shoulder, top, q, colors[(i + 2) % 3], 0, 0, 0);
            }
            return g.Finish("Moor rock outcrop");
        }

        public void Dispose()
        {
            if (disposed) return;
            disposed = true;
            foreach (var mesh in meshes) Destroy(mesh);
            foreach (var mesh in distantMeshes) Destroy(mesh);
            foreach (var value in materials) Destroy(value);
        }

        private static void Destroy(UnityEngine.Object value)
        {
            if (value == null) return;
            if (Application.isPlaying) UnityEngine.Object.Destroy(value);
            else UnityEngine.Object.DestroyImmediate(value);
        }
    }
}
