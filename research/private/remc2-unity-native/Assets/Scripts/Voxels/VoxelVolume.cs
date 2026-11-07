using System;
using System.Collections.Generic;
using UnityEngine;
using UnityEngine.Rendering;

namespace MagicCarpet.Native.Voxels
{
    // A fixed, axis-aligned world-space grid. Keep its parent stationary after construction.
    // Cells are selected by their centres, and face-connected cells reaching y=0 are supported.
    internal sealed class VoxelVolume : IDisposable
    {
        private const int ChunkSize = 16;
        private static readonly Vector3Int[] Directions = {
            new Vector3Int(1, 0, 0), new Vector3Int(-1, 0, 0),
            new Vector3Int(0, 1, 0), new Vector3Int(0, -1, 0),
            new Vector3Int(0, 0, 1), new Vector3Int(0, 0, -1)
        };
        // Counter-clockwise when viewed from outside; each quad has an independent normal/color.
        private static readonly Vector3[,] Corners = {
            { new Vector3(1,0,0), new Vector3(1,1,0), new Vector3(1,1,1), new Vector3(1,0,1) },
            { new Vector3(0,0,1), new Vector3(0,1,1), new Vector3(0,1,0), new Vector3(0,0,0) },
            { new Vector3(0,1,1), new Vector3(1,1,1), new Vector3(1,1,0), new Vector3(0,1,0) },
            { new Vector3(0,0,0), new Vector3(1,0,0), new Vector3(1,0,1), new Vector3(0,0,1) },
            { new Vector3(1,0,1), new Vector3(1,1,1), new Vector3(0,1,1), new Vector3(0,0,1) },
            { new Vector3(0,0,0), new Vector3(0,1,0), new Vector3(1,1,0), new Vector3(1,0,0) }
        };

        private sealed class Chunk
        {
            internal GameObject Object;
            internal MeshFilter Filter;
            internal MeshCollider Collider;
            internal Mesh Mesh;
        }

        private readonly Vector3 origin;
        private readonly Vector3Int dimensions;
        private readonly float cellSize;
        private readonly Material material;
        private readonly bool[] occupied;
        private readonly Color32[] colors;
        private readonly GameObject root;
        private readonly Dictionary<Vector3Int, Chunk> chunks = new Dictionary<Vector3Int, Chunk>();
        private readonly HashSet<Vector3Int> dirty = new HashSet<Vector3Int>();
        private readonly HashSet<Vector3Int> affected = new HashSet<Vector3Int>();
        private bool disposed;

        internal VoxelVolume(Transform parent, Vector3 origin, Vector3Int dimensions, float cellSize, Material material)
        {
            if (dimensions.x <= 0 || dimensions.y <= 0 || dimensions.z <= 0)
                throw new ArgumentOutOfRangeException(nameof(dimensions));
            if (float.IsNaN(cellSize) || float.IsInfinity(cellSize) || cellSize <= 0)
                throw new ArgumentOutOfRangeException(nameof(cellSize));
            if (material == null) throw new ArgumentNullException(nameof(material));
            int count = checked(checked(dimensions.x * dimensions.y) * dimensions.z);
            this.origin = origin;
            this.dimensions = dimensions;
            this.cellSize = cellSize;
            this.material = material; // Caller owns the shared material.
            occupied = new bool[count];
            colors = new Color32[count];
            root = new GameObject("Voxel Volume");
            root.layer = parent != null ? parent.gameObject.layer : 0;
            root.transform.position = origin;
            root.transform.SetParent(parent, true);
        }

        internal void SetCell(int x, int y, int z, Color32 color)
        {
            ThrowIfDisposed();
            if (!Inside(x, y, z)) throw new ArgumentOutOfRangeException(nameof(x), "Cell is outside the volume.");
            int index = Index(x, y, z);
            if (occupied[index] && colors[index].Equals(color)) return;
            occupied[index] = true;
            colors[index] = color;
            MarkDirty(x, y, z);
        }

        // Build after bulk SetCell calls; subsequent destructive operations rebuild affected chunks.
        internal void Build()
        {
            ThrowIfDisposed();
            foreach (Vector3Int key in dirty) RebuildChunk(key);
            dirty.Clear();
        }

        internal List<VoxelFragment> RemoveSphere(Vector3 worldCenter, float radius)
        {
            ThrowIfDisposed();
            if (float.IsNaN(radius) || float.IsInfinity(radius) || radius < 0)
                throw new ArgumentOutOfRangeException(nameof(radius));
            affected.Clear();
            var removed = new List<VoxelFragment>();
            Vector3 p = (worldCenter - origin) / cellSize - Vector3.one * 0.5f;
            float r = radius / cellSize;
            int minX = Mathf.Max(0, Mathf.CeilToInt(p.x - r));
            int minY = Mathf.Max(0, Mathf.CeilToInt(p.y - r));
            int minZ = Mathf.Max(0, Mathf.CeilToInt(p.z - r));
            int maxX = Mathf.Min(dimensions.x - 1, Mathf.FloorToInt(p.x + r));
            int maxY = Mathf.Min(dimensions.y - 1, Mathf.FloorToInt(p.y + r));
            int maxZ = Mathf.Min(dimensions.z - 1, Mathf.FloorToInt(p.z + r));
            float r2 = radius * radius;
            for (int z = minZ; z <= maxZ; z++)
            for (int y = minY; y <= maxY; y++)
            for (int x = minX; x <= maxX; x++)
            {
                int index = Index(x, y, z);
                Vector3 center = Center(x, y, z);
                if (!occupied[index] || (center - worldCenter).sqrMagnitude > r2) continue;
                removed.Add(new VoxelFragment(center, cellSize, colors[index]));
                occupied[index] = false;
                foreach (Vector3Int d in Directions)
                {
                    var neighbor = new Vector3Int(x + d.x, y + d.y, z + d.z);
                    if (Inside(neighbor.x, neighbor.y, neighbor.z)) affected.Add(neighbor);
                }
                MarkDirty(x, y, z);
            }
            Build();
            return removed;
        }

        internal List<List<VoxelFragment>> DetachUnsupported()
        {
            ThrowIfDisposed();
            affected.Clear();
            // Flood all supported cells first, then collect each disconnected island independently.
            var visited = new bool[occupied.Length];
            var queue = new Queue<Vector3Int>();
            for (int z = 0; z < dimensions.z; z++)
            for (int x = 0; x < dimensions.x; x++)
                Enqueue(x, 0, z, visited, queue);
            Flood(queue, visited, null);
            var groups = new List<List<VoxelFragment>>();
            for (int z = 0; z < dimensions.z; z++)
            for (int y = 0; y < dimensions.y; y++)
            for (int x = 0; x < dimensions.x; x++)
            {
                int index = Index(x, y, z);
                if (!occupied[index] || visited[index]) continue;
                var group = new List<VoxelFragment>();
                Enqueue(x, y, z, visited, queue);
                Flood(queue, visited, group);
                groups.Add(group);
            }
            Build();
            return groups;
        }

        // Call after each RemoveSphere on a previously supported volume. Deleting
        // cells can only disconnect components adjacent to the deleted set; this
        // does not discover islands that were already unsupported beforehand.
        internal List<List<VoxelFragment>> DetachAffectedUnsupported()
        {
            ThrowIfDisposed();
            var groups = new List<List<VoxelFragment>>();
            var supported = new HashSet<Vector3Int>();
            var visited = new HashSet<Vector3Int>();
            var queue = new Queue<Vector3Int>();
            foreach (Vector3Int candidate in affected)
            {
                if (!occupied[Index(candidate.x, candidate.y, candidate.z)] || supported.Contains(candidate)) continue;
                visited.Clear();
                queue.Clear();
                visited.Add(candidate);
                queue.Enqueue(candidate);
                bool anchored = false;
                while (queue.Count != 0)
                {
                    Vector3Int p = queue.Dequeue();
                    if (supported.Contains(p) || HasVerticalSupport(p, supported))
                    {
                        anchored = true;
                        break;
                    }
                    foreach (Vector3Int d in Directions)
                    {
                        Vector3Int next = p + d;
                        if (!Inside(next.x, next.y, next.z) || !occupied[Index(next.x, next.y, next.z)] || !visited.Add(next)) continue;
                        queue.Enqueue(next);
                    }
                }
                if (anchored)
                {
                    // Every queued cell was reached from the anchor search path.
                    foreach (Vector3Int p in visited) supported.Add(p);
                    continue;
                }
                var group = new List<VoxelFragment>(visited.Count);
                foreach (Vector3Int p in visited)
                {
                    int index = Index(p.x, p.y, p.z);
                    group.Add(new VoxelFragment(Center(p.x, p.y, p.z), cellSize, colors[index]));
                    occupied[index] = false;
                    MarkDirty(p.x, p.y, p.z);
                }
                groups.Add(group);
            }
            affected.Clear();
            Build();
            return groups;
        }

        // Most terrain columns have a direct path to the bottom. Cache the
        // proven path; otherwise the ordinary flood finds lateral support.
        private bool HasVerticalSupport(Vector3Int p, HashSet<Vector3Int> supported)
        {
            int y = p.y;
            while (y > 0 && !supported.Contains(new Vector3Int(p.x, y, p.z)))
            {
                if (!occupied[Index(p.x, y - 1, p.z)]) return false;
                y--;
            }
            for (int yy = y; yy <= p.y; yy++) supported.Add(new Vector3Int(p.x, yy, p.z));
            return true;
        }

        private void Flood(Queue<Vector3Int> queue, bool[] visited, List<VoxelFragment> detached)
        {
            while (queue.Count != 0)
            {
                Vector3Int p = queue.Dequeue();
                int index = Index(p.x, p.y, p.z);
                // Enqueue before removing so the flood can still discover adjacent cells.
                foreach (Vector3Int direction in Directions)
                    Enqueue(p.x + direction.x, p.y + direction.y, p.z + direction.z, visited, queue);
                if (detached == null) continue;
                detached.Add(new VoxelFragment(Center(p.x, p.y, p.z), cellSize, colors[index]));
                occupied[index] = false;
                MarkDirty(p.x, p.y, p.z);
            }
        }

        private void Enqueue(int x, int y, int z, bool[] visited, Queue<Vector3Int> queue)
        {
            if (!Inside(x, y, z)) return;
            int i = Index(x, y, z);
            if (!occupied[i] || visited[i]) return;
            visited[i] = true;
            queue.Enqueue(new Vector3Int(x, y, z));
        }

        private void MarkDirty(int x, int y, int z)
        {
            dirty.Add(new Vector3Int(x / ChunkSize, y / ChunkSize, z / ChunkSize));
            foreach (Vector3Int d in Directions)
            {
                int nx = x + d.x, ny = y + d.y, nz = z + d.z;
                if (Inside(nx, ny, nz)) dirty.Add(new Vector3Int(nx / ChunkSize, ny / ChunkSize, nz / ChunkSize));
            }
        }

        private void RebuildChunk(Vector3Int key)
        {
            var vertices = new List<Vector3>();
            var normals = new List<Vector3>();
            var vertexColors = new List<Color32>();
            var triangles = new List<int>();
            int x0 = key.x * ChunkSize, y0 = key.y * ChunkSize, z0 = key.z * ChunkSize;
            for (int z = z0; z < Math.Min(z0 + ChunkSize, dimensions.z); z++)
            for (int y = y0; y < Math.Min(y0 + ChunkSize, dimensions.y); y++)
            for (int x = x0; x < Math.Min(x0 + ChunkSize, dimensions.x); x++)
            {
                int index = Index(x, y, z);
                if (!occupied[index]) continue;
                Vector3 basePosition = new Vector3(x, y, z);
                for (int face = 0; face < Directions.Length; face++)
                {
                    Vector3Int d = Directions[face];
                    int nx = x + d.x, ny = y + d.y, nz = z + d.z;
                    if (Inside(nx, ny, nz) && occupied[Index(nx, ny, nz)]) continue;
                    int start = vertices.Count;
                    for (int corner = 0; corner < 4; corner++)
                    {
                        vertices.Add((basePosition + Corners[face, corner]) * cellSize);
                        normals.Add(new Vector3(d.x, d.y, d.z));
                        vertexColors.Add(colors[index]);
                    }
                    triangles.Add(start); triangles.Add(start + 1); triangles.Add(start + 2);
                    triangles.Add(start); triangles.Add(start + 2); triangles.Add(start + 3);
                }
            }
            if (vertices.Count == 0)
            {
                if (chunks.TryGetValue(key, out Chunk empty))
                {
                    // Destroy is deferred during play; stop collision/rendering immediately.
                    empty.Object.SetActive(false);
                    DestroyObject(empty.Object);
                    DestroyObject(empty.Mesh);
                    chunks.Remove(key);
                }
                return;
            }
            if (!chunks.TryGetValue(key, out Chunk chunk))
            {
                var go = new GameObject($"Voxel Chunk {key.x},{key.y},{key.z}");
                go.layer = root.layer;
                go.transform.SetParent(root.transform, false);
                chunk = new Chunk { Object = go, Filter = go.AddComponent<MeshFilter>(), Collider = go.AddComponent<MeshCollider>() };
                go.AddComponent<MeshRenderer>().sharedMaterial = material;
                chunks.Add(key, chunk);
            }
            var mesh = new Mesh { name = "Voxel chunk mesh", indexFormat = IndexFormat.UInt32 };
            mesh.SetVertices(vertices);
            mesh.SetNormals(normals);
            mesh.SetColors(vertexColors);
            mesh.SetTriangles(triangles, 0);
            mesh.RecalculateBounds();
            Mesh oldMesh = chunk.Mesh;
            chunk.Mesh = mesh;
            chunk.Filter.sharedMesh = mesh;
            chunk.Collider.sharedMesh = null;
            chunk.Collider.sharedMesh = mesh;
            DestroyObject(oldMesh);
        }

        private int Index(int x, int y, int z) => x + dimensions.x * (y + dimensions.y * z);
        private bool Inside(int x, int y, int z) => x >= 0 && y >= 0 && z >= 0 && x < dimensions.x && y < dimensions.y && z < dimensions.z;
        private Vector3 Center(int x, int y, int z) => origin + (new Vector3(x, y, z) + Vector3.one * 0.5f) * cellSize;
        private void ThrowIfDisposed() { if (disposed) throw new ObjectDisposedException(nameof(VoxelVolume)); }
        private static void DestroyObject(UnityEngine.Object obj)
        {
            if (obj == null) return;
            if (Application.isPlaying) UnityEngine.Object.Destroy(obj);
            else UnityEngine.Object.DestroyImmediate(obj);
        }
        public void Dispose()
        {
            if (disposed) return;
            disposed = true;
            root.SetActive(false);
            foreach (Chunk chunk in chunks.Values) DestroyObject(chunk.Mesh);
            chunks.Clear();
            DestroyObject(root);
        }
    }
}
