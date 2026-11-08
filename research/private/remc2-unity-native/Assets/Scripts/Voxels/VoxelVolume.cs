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
        private SphereEdit activeEdit;
        private SupportEdit activeSupport;
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
            ThrowIfEditing();
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

        // The collider may still reflect an earlier bounded edit chunk while
        // native column heights already describe the completed crater.
        internal bool RaycastSurface(Vector3 from, Vector3 to, out RaycastHit closest)
        {
            ThrowIfDisposed();
            closest=default;
            var segment=to-from;
            var distance=segment.magnitude;
            if(distance<=.00001f)return false;
            var found=false;
            foreach(var hit in Physics.RaycastAll(from,segment/distance,distance,1<<root.layer,QueryTriggerInteraction.Ignore))
            {
                if(hit.collider==null || !hit.collider.transform.IsChildOf(root.transform))continue;
                if(!found || hit.distance<closest.distance){closest=hit;found=true;}
            }
            return found;
        }

        internal bool TryColumn(float x,float z,out Vector2Int column)
        {
            ThrowIfDisposed();
            column=new Vector2Int(Mathf.FloorToInt((x-origin.x)/cellSize),Mathf.FloorToInt((z-origin.z)/cellSize));
            return column.x>=0 && column.y>=0 && column.x<dimensions.x && column.y<dimensions.z;
        }

        // Edits update occupied cells and rebuild their colliders together in
        // SphereEdit.Step, so this is the currently committed physical top.
        internal bool TryColumnTop(Vector2Int column,out float top)
        {
            ThrowIfDisposed();top=0;
            if(column.x<0||column.y<0||column.x>=dimensions.x||column.y>=dimensions.z)return false;
            for(int y=dimensions.y-1;y>=0;y--)
                if(occupied[Index(column.x,y,column.y)]){top=origin.y+(y+1)*cellSize;return true;}
            return false;
        }

        internal List<VoxelFragment> RemoveSphere(Vector3 worldCenter, float radius)
        {
            SphereEdit edit = BeginRemoveSphere(worldCenter, radius);
            while (!edit.Complete) edit.Step(int.MaxValue);
            return edit.Removed;
        }

        // Each step commits at most maxChunks scan chunks and synchronously
        // refreshes every affected mesh/collider before returning. Other edits
        // must wait until Complete, when support can be checked on the result.
        internal SphereEdit BeginRemoveSphere(Vector3 worldCenter, float radius)
        {
            ThrowIfDisposed();
            ThrowIfEditing();
            if (float.IsNaN(radius) || float.IsInfinity(radius) || radius < 0)
                throw new ArgumentOutOfRangeException(nameof(radius));
            if (!float.IsFinite(worldCenter.x) || !float.IsFinite(worldCenter.y) || !float.IsFinite(worldCenter.z))
                throw new ArgumentOutOfRangeException(nameof(worldCenter));
            if (dirty.Count != 0) Build();
            affected.Clear();
            var edit = new SphereEdit(this, worldCenter, radius);
            if (!edit.Complete) activeEdit = edit;
            return edit;
        }

        // Native authority supplies exact local cell indices. Presentation
        // commits those cells in bounded chunks without recomputing the blast.
        internal SphereEdit BeginRemoveCells(IReadOnlyList<Vector3Int> cells)
        {
            ThrowIfDisposed();ThrowIfEditing();
            var edit=new SphereEdit(this,cells);
            if(dirty.Count!=0)Build();
            affected.Clear();
            if(!edit.Complete)activeEdit=edit;
            return edit;
        }

        internal sealed class SphereEdit
        {
            private readonly VoxelVolume volume;
            private readonly Vector3 center;
            private readonly float radiusSquared;
            private readonly Vector3Int min, max, firstChunk, lastChunk;
            private readonly HashSet<int> selected;
            private int chunkX, chunkY, chunkZ;
            internal readonly List<VoxelFragment> Removed = new List<VoxelFragment>();
            internal bool Complete { get; private set; }
            internal int ScannedChunks { get; private set; }
            internal int RebuiltChunks { get; private set; }
            internal double ScanMilliseconds { get; private set; }
            internal double MeshColliderMilliseconds { get; private set; }
            internal double MaximumStepMilliseconds { get; private set; }

            internal SphereEdit(VoxelVolume volume, Vector3 center, float radius)
            {
                this.volume = volume;
                this.center = center;
                radiusSquared = radius * radius;
                Vector3 p = (center - volume.origin) / volume.cellSize - Vector3.one * .5f;
                float r = radius / volume.cellSize;
                min = new Vector3Int(Mathf.Max(0, Mathf.CeilToInt(p.x-r)), Mathf.Max(0, Mathf.CeilToInt(p.y-r)), Mathf.Max(0, Mathf.CeilToInt(p.z-r)));
                max = new Vector3Int(Mathf.Min(volume.dimensions.x-1, Mathf.FloorToInt(p.x+r)), Mathf.Min(volume.dimensions.y-1, Mathf.FloorToInt(p.y+r)), Mathf.Min(volume.dimensions.z-1, Mathf.FloorToInt(p.z+r)));
                Complete = min.x > max.x || min.y > max.y || min.z > max.z;
                firstChunk = new Vector3Int(min.x / ChunkSize, min.y / ChunkSize, min.z / ChunkSize);
                lastChunk = new Vector3Int(max.x / ChunkSize, max.y / ChunkSize, max.z / ChunkSize);
                chunkX = firstChunk.x; chunkY = firstChunk.y; chunkZ = firstChunk.z;
            }

            internal SphereEdit(VoxelVolume volume,IReadOnlyList<Vector3Int> cells):this(volume,Vector3.zero,0)
            {
                if(cells==null)throw new ArgumentNullException(nameof(cells));
                selected=new HashSet<int>();min=volume.dimensions;max=new Vector3Int(-1,-1,-1);
                foreach(var cell in cells){
                    if(!volume.Inside(cell.x,cell.y,cell.z))throw new ArgumentOutOfRangeException(nameof(cells),"Native cell is outside volume");
                    selected.Add(volume.Index(cell.x,cell.y,cell.z));min=Vector3Int.Min(min,cell);max=Vector3Int.Max(max,cell);
                }
                Complete=selected.Count==0;
                firstChunk=new Vector3Int(min.x/ChunkSize,min.y/ChunkSize,min.z/ChunkSize);
                lastChunk=new Vector3Int(max.x/ChunkSize,max.y/ChunkSize,max.z/ChunkSize);
                chunkX=firstChunk.x;chunkY=firstChunk.y;chunkZ=firstChunk.z;
            }

            internal void Step(int maxChunks)
            {
                volume.ThrowIfDisposed();
                if (maxChunks <= 0) throw new ArgumentOutOfRangeException(nameof(maxChunks));
                if (Complete) return;
                if (volume.activeEdit != this) throw new InvalidOperationException("Sphere edit no longer owns the volume.");
                long start = System.Diagnostics.Stopwatch.GetTimestamp();
                int count = 0;
                while (!Complete && count < maxChunks)
                {
                    ScanChunk(); count++; ScannedChunks++;
                    if (++chunkX > lastChunk.x)
                    {
                        chunkX = firstChunk.x;
                        if (++chunkY > lastChunk.y)
                        {
                            chunkY = firstChunk.y;
                            if (++chunkZ > lastChunk.z) Complete = true;
                        }
                    }
                }
                long scanEnd = System.Diagnostics.Stopwatch.GetTimestamp();
                int rebuilt = volume.dirty.Count;
                volume.Build();
                long end = System.Diagnostics.Stopwatch.GetTimestamp();
                double tickMs = 1000.0 / System.Diagnostics.Stopwatch.Frequency;
                ScanMilliseconds += (scanEnd - start) * tickMs;
                MeshColliderMilliseconds += (end - scanEnd) * tickMs;
                MaximumStepMilliseconds = Math.Max(MaximumStepMilliseconds, (end - start) * tickMs);
                RebuiltChunks += rebuilt;
                if (Complete) volume.activeEdit = null;
            }

            private void ScanChunk()
            {
                int firstRemoved = Removed.Count;
                int x0 = Math.Max(min.x, chunkX * ChunkSize), y0 = Math.Max(min.y, chunkY * ChunkSize), z0 = Math.Max(min.z, chunkZ * ChunkSize);
                int x1 = Math.Min(max.x, (chunkX + 1) * ChunkSize - 1), y1 = Math.Min(max.y, (chunkY + 1) * ChunkSize - 1), z1 = Math.Min(max.z, (chunkZ + 1) * ChunkSize - 1);
                var changed = new List<Vector3Int>();
                for (int z = z0; z <= z1; z++) for (int y = y0; y <= y1; y++) for (int x = x0; x <= x1; x++)
                {
                    int index = volume.Index(x, y, z);
                    if (!volume.occupied[index]) continue;
                    Vector3 position = volume.Center(x, y, z);
                    if (selected!=null?!selected.Contains(index):(position - center).sqrMagnitude > radiusSquared) continue;
                    Removed.Add(new VoxelFragment(position, volume.cellSize, volume.colors[index]));
                    volume.occupied[index] = false;
                    changed.Add(new Vector3Int(x, y, z));
                    volume.MarkDirty(x, y, z);
                }
                if (Removed.Count == firstRemoved) return;
                foreach (Vector3Int p in changed) foreach (Vector3Int d in Directions)
                {
                    Vector3Int n = p + d;
                    if (volume.Inside(n.x, n.y, n.z) && volume.occupied[volume.Index(n.x, n.y, n.z)]) volume.affected.Add(n);
                }
            }
        }

        internal List<List<VoxelFragment>> DetachUnsupported()
        {
            ThrowIfDisposed();
            ThrowIfEditing();
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
            ThrowIfEditing();
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

        // Incremental counterpart to DetachAffectedUnsupported. The volume must
        // have been supported before its latest sphere edit. Search and removal
        // are budgeted in cells; every removal step refreshes its colliders.
        internal SupportEdit BeginDetachAffectedUnsupported()
        {
            ThrowIfDisposed();
            ThrowIfEditing();
            var edit = new SupportEdit(this);
            activeSupport = edit;
            return edit;
        }

        internal sealed class SupportEdit
        {
            private enum Phase { Candidate, Search, MarkSupported, Remove }
            private readonly VoxelVolume volume;
            private readonly IEnumerator<Vector3Int> candidates;
            private readonly HashSet<Vector3Int> supported = new HashSet<Vector3Int>();
            private HashSet<Vector3Int> visited;
            private List<Vector3Int> component;
            private Queue<Vector3Int> queue;
            private List<VoxelFragment> currentGroup;
            private Phase phase;
            private int cursor;
            internal readonly List<List<VoxelFragment>> Groups = new List<List<VoxelFragment>>();
            internal bool Complete { get; private set; }
            internal int VisitedCells { get; private set; }
            internal int RebuiltChunks { get; private set; }
            internal double SearchMilliseconds { get; private set; }
            internal double MeshColliderMilliseconds { get; private set; }
            internal double MaximumStepMilliseconds { get; private set; }

            internal SupportEdit(VoxelVolume volume)
            {
                this.volume = volume;
                candidates = volume.affected.GetEnumerator();
            }

            internal void Step(int maxCells)
            {
                volume.ThrowIfDisposed();
                if (maxCells <= 0) throw new ArgumentOutOfRangeException(nameof(maxCells));
                if (Complete) return;
                if (volume.activeSupport != this) throw new InvalidOperationException("Support edit no longer owns the volume.");
                long start = System.Diagnostics.Stopwatch.GetTimestamp();
                for (int work = 0; work < maxCells && !Complete; work++)
                {
                    if (phase == Phase.Candidate)
                    {
                        if (!candidates.MoveNext())
                        {
                            candidates.Dispose();
                            volume.affected.Clear();
                            Complete = true;
                            break;
                        }
                        Vector3Int p = candidates.Current;
                        if (!volume.occupied[volume.Index(p.x, p.y, p.z)] || supported.Contains(p)) continue;
                        visited = new HashSet<Vector3Int> { p };
                        component = new List<Vector3Int> { p };
                        queue = new Queue<Vector3Int>(); queue.Enqueue(p);
                        phase = Phase.Search;
                    }
                    else if (phase == Phase.Search)
                    {
                        Vector3Int p = queue.Dequeue();
                        VisitedCells++;
                        if (supported.Contains(p) || volume.HasVerticalSupport(p, supported))
                        {
                            cursor = 0; phase = Phase.MarkSupported;
                            continue;
                        }
                        foreach (Vector3Int d in Directions)
                        {
                            Vector3Int n = p + d;
                            if (!volume.Inside(n.x, n.y, n.z) || !volume.occupied[volume.Index(n.x, n.y, n.z)] || !visited.Add(n)) continue;
                            component.Add(n); queue.Enqueue(n);
                        }
                        if (queue.Count == 0)
                        {
                            cursor = 0; currentGroup = new List<VoxelFragment>();
                            Groups.Add(currentGroup); phase = Phase.Remove;
                        }
                    }
                    else if (phase == Phase.MarkSupported)
                    {
                        supported.Add(component[cursor++]);
                        if (cursor == component.Count) phase = Phase.Candidate;
                    }
                    else
                    {
                        Vector3Int p = component[cursor++];
                        int index = volume.Index(p.x, p.y, p.z);
                        currentGroup.Add(new VoxelFragment(volume.Center(p.x, p.y, p.z), volume.cellSize, volume.colors[index]));
                        volume.occupied[index] = false;
                        volume.MarkDirty(p.x, p.y, p.z);
                        if (cursor == component.Count) phase = Phase.Candidate;
                    }
                }
                long scanEnd = System.Diagnostics.Stopwatch.GetTimestamp();
                int rebuilt = volume.dirty.Count;
                volume.Build();
                long end = System.Diagnostics.Stopwatch.GetTimestamp();
                double tickMs = 1000.0 / System.Diagnostics.Stopwatch.Frequency;
                SearchMilliseconds += (scanEnd - start) * tickMs;
                MeshColliderMilliseconds += (end - scanEnd) * tickMs;
                MaximumStepMilliseconds = Math.Max(MaximumStepMilliseconds, (end - start) * tickMs);
                RebuiltChunks += rebuilt;
                if (Complete) volume.activeSupport = null;
            }
        }

        // Most terrain columns have a direct path to the bottom. Do not insert
        // the entire deep column into a hash table for every exposed cell.
        // The surrounding flood still caches searched cells and lateral support.
        private bool HasVerticalSupport(Vector3Int p, HashSet<Vector3Int> supported)
        {
            for (int y = p.y - 1; y >= 0; y--)
                if (!occupied[Index(p.x, y, p.z)]) return false;
            supported.Add(p);
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
            int cx = x / ChunkSize, cy = y / ChunkSize, cz = z / ChunkSize;
            dirty.Add(new Vector3Int(cx, cy, cz));
            if (x > 0 && x % ChunkSize == 0) dirty.Add(new Vector3Int(cx - 1, cy, cz));
            if (x + 1 < dimensions.x && x % ChunkSize == ChunkSize - 1) dirty.Add(new Vector3Int(cx + 1, cy, cz));
            if (y > 0 && y % ChunkSize == 0) dirty.Add(new Vector3Int(cx, cy - 1, cz));
            if (y + 1 < dimensions.y && y % ChunkSize == ChunkSize - 1) dirty.Add(new Vector3Int(cx, cy + 1, cz));
            if (z > 0 && z % ChunkSize == 0) dirty.Add(new Vector3Int(cx, cy, cz - 1));
            if (z + 1 < dimensions.z && z % ChunkSize == ChunkSize - 1) dirty.Add(new Vector3Int(cx, cy, cz + 1));
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
        private void ThrowIfEditing() { if (activeEdit != null || activeSupport != null) throw new InvalidOperationException("Complete the active volume edit first."); }
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
