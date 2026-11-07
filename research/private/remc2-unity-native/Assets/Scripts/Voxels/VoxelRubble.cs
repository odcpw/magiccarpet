using System;
using System.Collections.Generic;
using System.Diagnostics;
using UnityEngine;
using UnityEngine.Rendering;

namespace MagicCarpet.Native.Voxels
{
    // Sandbox-only physical debris. Every input voxel remains in a rendered mesh;
    // a cluster uses one conservative box collider, never one object per cell.
    internal sealed class VoxelRubble : MonoBehaviour
    {
        internal const int MaxBodies = 160;
        internal const int MaxClustersPerEmission = 32;
        internal const float Density = 1f;
        internal const float MinimumVisibleSeconds = 3f;
        // A distant or continuously disturbed piece eventually expires too; the
        // forced path is counted separately from pieces that actually sleep.
        internal const float MaximumActiveSeconds = 45f;
        internal const float FadeSeconds = 2f;
        private readonly List<Piece> pieces = new List<Piece>();
        private readonly Queue<Emission> pending = new Queue<Emission>();
        private Material material;
        private PhysicsMaterial contactMaterial;
        private readonly MaterialPropertyBlock properties = new MaterialPropertyBlock();
        internal int ActiveBodies => pieces.Count;
        internal int PeakBodies { get; private set; }
        internal int PendingEmissions => pending.Count;
        internal int SleepingBodies
        {
            get { int count = 0; foreach (var p in pieces) if (p.Body != null && p.Body.IsSleeping()) count++; return count; }
        }
        internal int MovingBodies => ActiveBodies - SleepingBodies;
        internal int FallingBodies
        {
            get { int count = 0; foreach (var p in pieces) if (p.Body != null && p.Body.linearVelocity.y < -1f) count++; return count; }
        }
        internal float LowestBodyY
        {
            get { float lowest = float.PositiveInfinity; foreach (var p in pieces) if (p.Body != null) lowest = Mathf.Min(lowest, p.Body.position.y); return float.IsPositiveInfinity(lowest) ? 0f : lowest; }
        }
        internal float FastestBodySpeed
        {
            get { float fastest = 0f; foreach (var p in pieces) if (p.Body != null) fastest = Mathf.Max(fastest, p.Body.linearVelocity.magnitude); return fastest; }
        }
        internal int ContactEvents { get; private set; }
        internal int ContactPoints { get; private set; }
        internal int ReboundEvents { get; private set; }
        internal int EmittedCells { get; private set; }
        internal int EmissionCount { get; private set; }
        internal int SingleClusterEmissions { get; private set; }
        internal int LastEmissionCells { get; private set; }
        internal int LastEmissionGroups { get; private set; }
        internal int MinimumEmissionGroups { get; private set; } = int.MaxValue;
        internal int DisposedCells { get; private set; }
        internal int SettledPieces { get; private set; }
        internal int ForcedFadePieces { get; private set; }
        internal double ManagementMilliseconds { get; private set; }
        // Exclusive stage timings in milliseconds, accumulated and for the last
        // emitted group; spawn excludes mesh construction. Neither is PhysX time.
        internal double PartitionMilliseconds { get; private set; }
        internal double MeshMilliseconds { get; private set; }
        internal double SpawnMilliseconds { get; private set; }
        internal double LastPartitionMilliseconds { get; private set; }
        internal double LastMeshMilliseconds { get; private set; }
        internal double LastSpawnMilliseconds { get; private set; }
        internal double LastEmissionMilliseconds { get; private set; }
        internal int LastGridCandidates { get; private set; }
        internal int LastGeneralCandidates { get; private set; }

        internal sealed class Emission
        {
            internal VoxelFragment[] Cells;
            internal Vector3 Velocity, Origin;
            internal float Energy;
            internal float TotalMass;
            internal int TargetClusters;
            internal int MinimumClusters;
            internal bool Spawned;
            internal bool HasImpactPlane;
            internal Vector3 ImpactNormal;
        }
        private sealed class Piece
        {
            internal GameObject Object;
            internal Rigidbody Body;
            internal Mesh Mesh;
            internal MeshRenderer Renderer;
            internal float Born, FadeStart = -1f;
            internal int Cells;
        }

        private sealed class ContactProbe : MonoBehaviour
        {
            private VoxelRubble owner;
            private Rigidbody body, other;
            private Vector3 incoming, normal, point;
            private bool awaitingRebound;
            internal void Initialize(VoxelRubble owner, Rigidbody body)
            { this.owner = owner; this.body = body; incoming = body.linearVelocity; }

            private void FixedUpdate()
            {
                if (awaitingRebound)
                {
                    Vector3 relative = body.GetPointVelocity(point) - (other != null ? other.GetPointVelocity(point) : Vector3.zero);
                    if (Vector3.Dot(relative, normal) > .1f) owner.ReboundEvents++;
                    awaitingRebound = false;
                }
                incoming = body.linearVelocity;
            }

            private void OnCollisionEnter(Collision collision)
            {
                owner.ContactEvents++;
                owner.ContactPoints += collision.contactCount;
                if (collision.contactCount == 0 || awaitingRebound) return;
                var contact = collision.GetContact(0);
                var otherBody = collision.rigidbody;
                Vector3 otherVelocity = otherBody != null ? otherBody.GetPointVelocity(contact.point) : Vector3.zero;
                if (Vector3.Dot(incoming - otherVelocity, contact.normal) >= -.1f) return;
                normal = contact.normal; point = contact.point; other = otherBody; awaitingRebound = true;
            }
        }

        internal void Emit(IReadOnlyList<VoxelFragment> cells, Vector3 inheritedVelocity, Vector3 impulseOrigin, float energy)
        {
            EmitInternal(cells, inheritedVelocity, impulseOrigin, energy, MaxClustersPerEmission);
        }

        internal Emission EmitImpact(IReadOnlyList<VoxelFragment> cells, Vector3 inheritedVelocity, Vector3 impulseOrigin, Vector3 impactNormal)
        {
            return EmitInternal(cells, inheritedVelocity, impulseOrigin, 0f, MaxClustersPerEmission, 8, impactNormal);
        }

        // Connected volumes stay in one body where possible; the input retains all cells.
        internal void EmitConnected(IReadOnlyList<VoxelFragment> cells, Vector3 inheritedVelocity, Vector3 impulseOrigin, float energy)
        {
            EmitInternal(cells, inheritedVelocity, impulseOrigin, energy, 1);
        }

        private Emission EmitInternal(IReadOnlyList<VoxelFragment> cells, Vector3 velocity, Vector3 origin, float energy, int target, int minimum = 1, Vector3 impactNormal = default)
        {
            if (cells == null || cells.Count == 0) return null;
            var copy = new VoxelFragment[cells.Count];
            for (int i = 0; i < cells.Count; i++) copy[i] = cells[i];
            var emission = new Emission { Cells = copy, Velocity = velocity, Origin = origin,
                Energy = Mathf.Max(0f, energy), TargetClusters = target, MinimumClusters = Mathf.Min(minimum, cells.Count),
                HasImpactPlane = impactNormal.sqrMagnitude > .000001f, ImpactNormal = impactNormal.normalized };
            pending.Enqueue(emission);
            Drain();
            return emission;
        }

        private void EnsureMaterials()
        {
            if (material == null)
            {
                var shader = Shader.Find("MagicCarpet/VoxelSurface");
                if (shader == null) throw new InvalidOperationException("Missing MagicCarpet/VoxelSurface shader");
                material = new Material(shader) { name = "Voxel rubble" };
            }
            if (contactMaterial == null)
                contactMaterial = new PhysicsMaterial("Voxel rubble") { dynamicFriction = .65f, staticFriction = .8f,
                    bounciness = .36f, bounceCombine = PhysicsMaterialCombine.Maximum };
        }

        private void Drain()
        {
            while (pending.Count > 0 && pieces.Count < MaxBodies)
            {
                if (MaxBodies - pieces.Count < pending.Peek().MinimumClusters) break;
                var e = pending.Dequeue();
                long emissionStart = Stopwatch.GetTimestamp();
                if (e.Energy > 0f) e.TotalMass = Mathf.Max(.0001f, Mass(e.Cells));
                int count = Mathf.Min(e.Cells.Length, Mathf.Min(e.TargetClusters, MaxBodies - pieces.Count));
                long partitionStart = Stopwatch.GetTimestamp();
                var groups = Partition(e.Cells, count, out int gridCandidates, out int generalCandidates);
                LastGridCandidates = gridCandidates;
                LastGeneralCandidates = generalCandidates;
                EmissionCount++;
                LastEmissionCells = e.Cells.Length;
                LastEmissionGroups = groups.Count;
                MinimumEmissionGroups = Mathf.Min(MinimumEmissionGroups, groups.Count);
                if (groups.Count == 1) SingleClusterEmissions++;
                LastPartitionMilliseconds = Milliseconds(Stopwatch.GetTimestamp() - partitionStart);
                PartitionMilliseconds += LastPartitionMilliseconds;
                EnsureMaterials();
                var offset = e.HasImpactPlane ? ImpactClearance(groups, e.Origin, e.ImpactNormal) : Vector3.zero;
                long spawnStart = Stopwatch.GetTimestamp(), meshTicks = 0;
                foreach (var group in groups) Spawn(group, e, offset, ref meshTicks);
                long spawnTicks = Stopwatch.GetTimestamp() - spawnStart;
                LastMeshMilliseconds = Milliseconds(meshTicks);
                LastSpawnMilliseconds = Milliseconds(spawnTicks - meshTicks);
                MeshMilliseconds += LastMeshMilliseconds;
                SpawnMilliseconds += LastSpawnMilliseconds;
                EmittedCells += e.Cells.Length;
                e.Spawned = true;
                LastEmissionMilliseconds = Milliseconds(Stopwatch.GetTimestamp() - emissionStart);
            }
        }

        private static double Milliseconds(long ticks) => ticks * 1000.0 / Stopwatch.Frequency;

        private sealed class SplitCache
        {
            internal bool Evaluated, CanSplit;
            internal int Axis, Mid;
        }

        // Split only at a separating plane: the resulting cluster AABBs do not
        // overlap each other, including for irregular/rotated voxel positions.
        internal static List<List<VoxelFragment>> Partition(IReadOnlyList<VoxelFragment> cells, int count)
        {
            return Partition(cells, count, out _, out _);
        }

        private static List<List<VoxelFragment>> Partition(IReadOnlyList<VoxelFragment> cells, int count,
            out int gridCandidates, out int generalCandidates)
        {
            gridCandidates = 0; generalCandidates = 0;
            var result = new List<List<VoxelFragment>>();
            if (cells == null || cells.Count == 0 || count <= 0) return result;
            var all = new List<VoxelFragment>(cells.Count);
            for (int i = 0; i < cells.Count; i++) all.Add(cells[i]);
            result.Add(all);
            var candidates = new List<SplitCache> { new SplitCache() };
            count = Mathf.Min(count, all.Count);
            while (result.Count < count)
            {
                int selected = -1, selectedAxis = -1, selectedMid = 0, selectedSize = 0;
                for (int i = 0; i < result.Count; i++)
                {
                    if (result[i].Count <= selectedSize) continue;
                    var candidate = candidates[i];
                    if (!candidate.Evaluated)
                    {
                        candidate.CanSplit = FindSplit(result[i], out candidate.Axis, out candidate.Mid, out bool usedGrid);
                        if (usedGrid) gridCandidates++; else generalCandidates++;
                        candidate.Evaluated = true;
                    }
                    if (candidate.CanSplit)
                    { selected = i; selectedAxis = candidate.Axis; selectedMid = candidate.Mid; selectedSize = result[i].Count; }
                }
                if (selected < 0) break;
                var group = result[selected];
                group.Sort((a, b) => a.Position[selectedAxis].CompareTo(b.Position[selectedAxis]));
                var second = group.GetRange(selectedMid, group.Count - selectedMid);
                group.RemoveRange(selectedMid, group.Count - selectedMid);
                result.Add(second);
                // The two children changed; other cached groups remain exact.
                candidates[selected] = new SplitCache();
                candidates.Add(new SplitCache());
            }
            return result;
        }

        private static bool FindSplit(List<VoxelFragment> group, out int bestAxis, out int bestMid, out bool usedGrid)
        {
            bestAxis = -1; bestMid = 0; usedGrid = false;
            int bestBalance = int.MaxValue;
            if (group.Count < 2) return false;
            if (TryFindGridSplit(group, out bestAxis, out bestMid)) { usedGrid = true; return bestAxis >= 0; }
            bestAxis = -1; bestMid = 0;
            BoundsOf(group, out Vector3 boundsMin, out Vector3 boundsMax);
            Vector3 span = boundsMax - boundsMin;
            float bestSpan = float.NegativeInfinity;
            var sorted = new List<VoxelFragment>(group);
            var rightMin = new float[group.Count];
            for (int axis = 0; axis < 3; axis++)
            {
                int a = axis;
                sorted.Sort((x, y) => x.Position[a].CompareTo(y.Position[a]));
                float min = float.PositiveInfinity;
                for (int i = sorted.Count - 1; i >= 0; i--)
                { min = Mathf.Min(min, sorted[i].Position[a] - sorted[i].Size * .5f); rightMin[i] = min; }
                float max = float.NegativeInfinity;
                for (int i = 1; i < sorted.Count; i++)
                {
                    var previous = sorted[i - 1];
                    max = Mathf.Max(max, previous.Position[a] + previous.Size * .5f);
                    // Tolerate only floating-point noise at touching faces.
                    float tolerance = Mathf.Min(previous.Size, sorted[i].Size) * .0001f;
                    if (max > rightMin[i] + tolerance) continue;
                    int balance = Mathf.Abs(sorted.Count - 2 * i);
                    if (span[axis] > bestSpan || (axis == bestAxis && balance < bestBalance))
                    { bestSpan = span[axis]; bestBalance = balance; bestAxis = axis; bestMid = i; }
                }
            }
            return bestAxis >= 0;
        }

        // Returns false when a lattice cannot be established, so FindSplit
        // falls back to the exact general algorithm. A successful recognition may
        // still find no separating plane (axis == -1).
        internal static bool TryFindGridSplit(IReadOnlyList<VoxelFragment> group, out int bestAxis, out int bestMid)
        {
            bestAxis = -1; bestMid = 0;
            if (group == null || group.Count < 2) return false;
            float size = group[0].Size;
            if (!(size > 0f) || float.IsInfinity(size)) return false;
            for (int i = 1; i < group.Count; i++) if (group[i].Size != size) return false;
            int balanceBest = int.MaxValue;
            BoundsOf(group, out Vector3 boundsMin, out Vector3 boundsMax);
            Vector3 span = boundsMax - boundsMin;
            float bestSpan = float.NegativeInfinity;
            var indices = new int[group.Count];
            float half = size * .5f, tolerance = size * .0001f;
            for (int axis = 0; axis < 3; axis++)
            {
                float origin = group[0].Position[axis];
                if (float.IsNaN(origin) || float.IsInfinity(origin)) return false;
                int low = 0, high = 0;
                for (int i = 0; i < group.Count; i++)
                {
                    float position = group[i].Position[axis];
                    if (float.IsNaN(position) || float.IsInfinity(position)) return false;
                    double scaled = ((double)position - origin) / size;
                    if (scaled < -8192 || scaled > 8192) return false;
                    int index = (int)Math.Round(scaled);
                    // Include float-position rounding at large world coordinates,
                    // but reject any grid too imprecise to disambiguate cells.
                    double allowed = .01 * size + Math.Max(Math.Abs((double)origin), Math.Abs((double)position)) * 2.4e-7;
                    if (allowed >= .2 * size || Math.Abs(((double)position - origin) - index * (double)size) > allowed) return false;
                    indices[i] = index;
                    if (index < low) low = index;
                    if (index > high) high = index;
                }
                int length = high - low + 1;
                if (length > 8192) return false;
                var counts = new int[length];
                var minLeft = new float[length];
                var maxRight = new float[length];
                for (int i = 0; i < length; i++) { minLeft[i] = float.PositiveInfinity; maxRight[i] = float.NegativeInfinity; }
                for (int i = 0; i < group.Count; i++)
                {
                    int bucket = indices[i] - low;
                    float position = group[i].Position[axis];
                    counts[bucket]++;
                    minLeft[bucket] = Mathf.Min(minLeft[bucket], position - half);
                    maxRight[bucket] = Mathf.Max(maxRight[bucket], position + half);
                }
                // A purported bucket must not itself contain a separating cut.
                // The lattice bound above normally makes this true; checking the
                // actual extents also covers large-coordinate float precision.
                for (int i = 0; i < length; i++)
                    if (counts[i] > 1 && maxRight[i] - minLeft[i] >= 2f * size - tolerance) return false;
                var suffix = new float[length];
                float right = float.PositiveInfinity;
                for (int i = length - 1; i >= 0; i--) { right = Mathf.Min(right, minLeft[i]); suffix[i] = right; }
                int leftCount = 0;
                float left = float.NegativeInfinity;
                for (int i = 0; i + 1 < length; i++)
                {
                    leftCount += counts[i]; left = Mathf.Max(left, maxRight[i]);
                    if (counts[i] == 0 || leftCount == group.Count || left > suffix[i + 1] + tolerance) continue;
                    int balance = Mathf.Abs(group.Count - 2 * leftCount);
                    if (span[axis] > bestSpan || (axis == bestAxis && balance < balanceBest))
                    { bestSpan = span[axis]; balanceBest = balance; bestAxis = axis; bestMid = leftCount; }
                }
            }
            return true;
        }

        internal static Vector3 ImpactClearance(List<List<VoxelFragment>> groups, Vector3 point, Vector3 normal)
        {
            float lowest = float.PositiveInfinity;
            foreach (var group in groups)
            {
                BoundsOf(group, out Vector3 min, out Vector3 max);
                Vector3 center = (min + max) * .5f, half = (max - min) * .5f;
                float projection = Vector3.Dot(center - point, normal) -
                    (Mathf.Abs(normal.x) * half.x + Mathf.Abs(normal.y) * half.y + Mathf.Abs(normal.z) * half.z);
                lowest = Mathf.Min(lowest, projection);
            }
            return normal * Mathf.Max(0f, .005f - lowest);
        }

        private static void BoundsOf(IReadOnlyList<VoxelFragment> cells, out Vector3 min, out Vector3 max)
        {
            min = Vector3.one * float.PositiveInfinity; max = Vector3.one * float.NegativeInfinity;
            foreach (var c in cells) { var h = Vector3.one * (c.Size * .5f); min = Vector3.Min(min, c.Position - h); max = Vector3.Max(max, c.Position + h); }
        }

        internal static float Mass(IReadOnlyList<VoxelFragment> cells)
        {
            float mass = 0f;
            for (int i = 0; i < cells.Count; i++) mass += Density * Mathf.Pow(cells[i].Size, 3f);
            return mass;
        }

        internal static bool ShouldBeginFade(float age, bool sleeping)
        {
            return age >= MinimumVisibleSeconds && (sleeping || age >= MaximumActiveSeconds);
        }

        private void Spawn(List<VoxelFragment> cells, Emission emission, Vector3 offset, ref long meshTicks)
        {
            BoundsOf(cells, out Vector3 min, out Vector3 max);
            Vector3 center = (min + max) * .5f;
            var go = new GameObject("Voxel rubble cluster");
            go.transform.SetParent(transform, false); go.transform.position = center + offset;
            long meshStart = Stopwatch.GetTimestamp();
            var mesh = BuildMesh(cells, center);
            meshTicks += Stopwatch.GetTimestamp() - meshStart;
            go.AddComponent<MeshFilter>().sharedMesh = mesh;
            var renderer = go.AddComponent<MeshRenderer>(); renderer.sharedMaterial = material;
            var box = go.AddComponent<BoxCollider>(); box.size = (max - min) * .98f; box.sharedMaterial = contactMaterial;
            var body = go.AddComponent<Rigidbody>(); body.mass = Mathf.Max(.0001f, Mass(cells));
            body.collisionDetectionMode = CollisionDetectionMode.ContinuousDynamic;
            body.linearVelocity = emission.Velocity;
            go.AddComponent<ContactProbe>().Initialize(this, body);
            if (emission.Energy > 0f)
            {
                // Energy is shared by mass; a bounded outward impulse supplements inherited motion.
                Vector3 outward = center - emission.Origin;
                if (outward.sqrMagnitude > .000001f)
                    body.linearVelocity += outward.normalized * Mathf.Min(5f, Mathf.Sqrt(2f * emission.Energy / emission.TotalMass));
            }
            pieces.Add(new Piece { Object = go, Body = body, Mesh = mesh, Renderer = renderer, Born = Time.time, Cells = cells.Count });
            PeakBodies = Mathf.Max(PeakBodies, pieces.Count);
        }

        internal static Mesh BuildMesh(IReadOnlyList<VoxelFragment> cells, Vector3 center)
        {
            // Four separate vertices per face allow hard normals and faithful cell colors.
            Vector3[] corners = { new Vector3(-1,-1,-1), new Vector3(1,-1,-1), new Vector3(1,1,-1), new Vector3(-1,1,-1),
                new Vector3(-1,-1,1), new Vector3(1,-1,1), new Vector3(1,1,1), new Vector3(-1,1,1) };
            int[,] faces = { { 0,3,2,1 }, { 5,6,7,4 }, { 4,7,3,0 }, { 1,2,6,5 }, { 3,7,6,2 }, { 4,0,1,5 } };
            Vector3[] normals = { Vector3.back, Vector3.forward, Vector3.left, Vector3.right, Vector3.up, Vector3.down };
            var vertices = new Vector3[cells.Count * 24]; var colors = new Color32[vertices.Length];
            var ns = new Vector3[vertices.Length]; var triangles = new int[cells.Count * 36];
            for (int i = 0; i < cells.Count; i++) for (int f = 0; f < 6; f++)
            {
                int v = i * 24 + f * 4, t = i * 36 + f * 6;
                for (int k = 0; k < 4; k++) { vertices[v+k] = cells[i].Position - center + corners[faces[f,k]] * (cells[i].Size * .5f); ns[v+k] = normals[f]; colors[v+k] = cells[i].Color; }
                triangles[t] = v; triangles[t+1] = v+1; triangles[t+2] = v+2; triangles[t+3] = v; triangles[t+4] = v+2; triangles[t+5] = v+3;
            }
            var mesh = new Mesh { name = "Voxel cluster", indexFormat = vertices.Length > 65535 ? IndexFormat.UInt32 : IndexFormat.UInt16 };
            mesh.vertices = vertices; mesh.normals = ns; mesh.colors32 = colors; mesh.triangles = triangles; mesh.RecalculateBounds(); return mesh;
        }

        private void Update()
        {
            long start = Stopwatch.GetTimestamp();
            for (int i = pieces.Count - 1; i >= 0; i--)
            {
                var p = pieces[i]; float age = Time.time - p.Born;
                if (p.FadeStart < 0f && ShouldBeginFade(age, p.Body.IsSleeping()))
                {
                    p.FadeStart = Time.time;
                    if (p.Body.IsSleeping()) SettledPieces++; else ForcedFadePieces++;
                }
                if (p.FadeStart < 0f) continue;
                float alpha = 1f - (Time.time - p.FadeStart) / FadeSeconds;
                if (alpha <= 0f) { DisposedCells += p.Cells; Destroy(p.Object); Destroy(p.Mesh); pieces.RemoveAt(i); continue; }
                p.Renderer.GetPropertyBlock(properties); properties.SetFloat("_Alpha", alpha); p.Renderer.SetPropertyBlock(properties);
            }
            Drain();
            ManagementMilliseconds += (Stopwatch.GetTimestamp() - start) * 1000.0 / Stopwatch.Frequency;
        }

        private void OnDestroy()
        {
            foreach (var p in pieces) { if (p.Object != null) Destroy(p.Object); if (p.Mesh != null) Destroy(p.Mesh); }
            pieces.Clear(); pending.Clear();
            if (material != null) Destroy(material);
            if (contactMaterial != null) Destroy(contactMaterial);
        }
    }
}
