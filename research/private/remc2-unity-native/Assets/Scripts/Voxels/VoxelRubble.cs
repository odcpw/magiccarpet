using System;
using System.Collections.Generic;
using System.Diagnostics;
using UnityEngine;
using UnityEngine.Rendering;

namespace MagicCarpet.Native.Voxels
{
    // A bounded, prewarmed pool of independent physical cubes. Large source
    // emissions are represented by spatial samples with source-mass weighting;
    // rendered cube count is intentionally not the number of source cells.
    internal sealed class VoxelRubble : MonoBehaviour
    {
        internal const int MaxBodies = 768;
        internal const int MaxPiecesPerEmission = 192;
        internal const int MaxActivationsPerFrame = 64;
        internal const float MaximumBlockSize = .20f;
        internal const float Density = 1f;
        internal const float MinimumVisibleSeconds = 3f;
        internal const float MaximumActiveSeconds = 45f;
        internal const float FadeSeconds = 2f;
        private const float MinimumMass = .0000001f;
        private readonly List<Piece> active = new List<Piece>(MaxBodies);
        private readonly Stack<Piece> available = new Stack<Piece>(MaxBodies);
        private readonly Queue<Emission> pending = new Queue<Emission>();
        private readonly List<Vector3> activePositions=new(MaxBodies);
        private MaterialPropertyBlock properties;
        private Mesh cubeMesh;
        private Material material;
        private PhysicsMaterial contactMaterial;
        private int reserved, budgetFrame = -1, frameActivations;
        private static readonly int AlphaId = Shader.PropertyToID("_Alpha");
        private static readonly int ColorId = Shader.PropertyToID("_BaseColor");
        private static readonly int EmissionId = Shader.PropertyToID("_Emission");

        internal int ActiveBodies => active.Count;
        internal IReadOnlyList<Vector3> ActivePositions {
            get {activePositions.Clear();foreach(var p in active)activePositions.Add(p.Body.position);return activePositions;}
        }
        internal int PeakBodies { get; private set; }
        internal int PendingEmissions => pending.Count;
        internal int SleepingBodies { get { int n = 0; foreach (var p in active) if (p.Body.IsSleeping()) n++; return n; } }
        internal int MovingBodies => ActiveBodies - SleepingBodies;
        internal int FallingBodies { get { int n = 0; foreach (var p in active) if (p.Body.linearVelocity.y < -1f) n++; return n; } }
        internal float LowestBodyY { get { float v = float.PositiveInfinity; foreach (var p in active) v = Mathf.Min(v, p.Body.position.y); return float.IsPositiveInfinity(v) ? 0f : v; } }
        internal float FastestBodySpeed { get { float v = 0f; foreach (var p in active) v = Mathf.Max(v, p.Body.linearVelocity.magnitude); return v; } }
        internal int CountBelowSurface(Func<float,float,float> height)
        {
            int count=0;
            foreach(var p in active){var at=p.Body.position;if(at.y<height(at.x,at.z)-.25f){
                if(count<5){bool hit=Physics.Raycast(new Vector3(at.x,height(at.x,at.z)+30,at.z),Vector3.down,out var contact,1000,1<<8);
                    UnityEngine.Debug.Log($"MC_VOXEL_BELOW at={at.ToString("F3")} surface={height(at.x,at.z):F3} velocity={p.Body.linearVelocity.ToString("F3")} ray={hit} contact={(hit?contact.point.y:float.NaN):F3} born={p.Born:F3} spawn={p.SpawnPosition.ToString("F3")} size={p.Object.transform.localScale.x:F3} last_contact={p.Probe.LastContact}");}
                count++;}}
            return count;
        }
        internal int ContactEvents { get; private set; }
        internal int ContactPoints { get; private set; }
        internal int ReboundEvents { get; private set; }
        // Source counts and visible counts are separate: one weighted cube may
        // represent the mass of many unsampled source cells.
        internal int EmittedCells { get; private set; }
        internal int SampledCells { get; private set; }
        internal int UnrenderedCells { get; private set; }
        internal int DroppedEmissions { get; private set; }
        internal int DisposedCells { get; private set; }
        internal int SettledPieces { get; private set; }
        internal int ForcedFadePieces { get; private set; }
        internal int PeakActivationsPerFrame { get; private set; }
        internal int EmissionCount { get; private set; }
        internal int SingleClusterEmissions { get; private set; }
        internal int LastEmissionCells { get; private set; }
        internal int LastEmissionGroups { get; private set; }
        internal int MinimumEmissionGroups { get; private set; } = int.MaxValue;
        internal double ManagementMilliseconds { get; private set; }
        internal double PartitionMilliseconds => 0;
        internal double MeshMilliseconds => 0;
        internal double SpawnMilliseconds { get; private set; }
        internal double LastPartitionMilliseconds => 0;
        internal double LastMeshMilliseconds => 0;
        internal double LastSpawnMilliseconds { get; private set; }
        internal double LastEmissionMilliseconds { get; private set; }
        internal int LastGridCandidates => 0;
        internal int LastGeneralCandidates => 0;

        internal sealed class Emission
        {
            internal VoxelFragment[] Cells;
            internal Vector3 Velocity, Origin, Offset;
            internal float Energy, TotalMass;
            internal int Count, Next;
            internal bool Impact, Spawned;
            internal long SpawnTicks;
        }
        private sealed class Piece
        {
            internal GameObject Object;
            internal Rigidbody Body;
            internal BoxCollider Collider;
            internal MeshRenderer Renderer;
            internal ContactProbe Probe;
            internal float Born, FadeStart;
            internal Vector3 SpawnPosition;
            internal int RepresentedCells;
        }
        private sealed class ContactProbe : MonoBehaviour
        {
            private VoxelRubble owner;
            private Rigidbody body, other;
            private Vector3 incoming, normal, point;
            private bool awaitingRebound;
            internal Vector3 LastContact {get;private set;}
            internal void Initialize(VoxelRubble owner, Rigidbody body) { this.owner = owner; this.body = body; }
            internal void ResetProbe(Vector3 velocity) { incoming = velocity; awaitingRebound = false; other = null;LastContact=Vector3.zero; }
            private void FixedUpdate()
            {
                if (awaitingRebound)
                {
                    var relative = body.GetPointVelocity(point) - (other != null ? other.GetPointVelocity(point) : Vector3.zero);
                    if (Vector3.Dot(relative, normal) > .1f) owner.ReboundEvents++;
                    awaitingRebound = false;
                }
                incoming = body.linearVelocity;
            }
            private void OnCollisionEnter(Collision collision)
            {
                owner.ContactEvents++; owner.ContactPoints += collision.contactCount;
                if (collision.contactCount == 0 || awaitingRebound) return;
                var c = collision.GetContact(0); var otherBody = collision.rigidbody;
                LastContact=c.point;
                var otherVelocity = otherBody != null ? otherBody.GetPointVelocity(c.point) : Vector3.zero;
                if (Vector3.Dot(incoming - otherVelocity, c.normal) >= -.1f) return;
                normal = c.normal; point = c.point; other = otherBody; awaitingRebound = true;
            }
        }

        private void Awake()
        {
            properties=new MaterialPropertyBlock();
            var shader = Shader.Find("MagicCarpet/VoxelSurface");
            if (shader == null) throw new InvalidOperationException("Missing MagicCarpet/VoxelSurface shader");
            material = new Material(shader) { name = "Pooled voxel rubble" };
            contactMaterial = new PhysicsMaterial("Voxel rubble") { dynamicFriction = .65f, staticFriction = .8f,
                bounciness = .36f, bounceCombine = PhysicsMaterialCombine.Maximum };
            var white = new[] { new VoxelFragment(Vector3.zero, 1f, new Color32(255, 255, 255, 255)) };
            cubeMesh = BuildMesh(white, Vector3.zero);
            for (int i = 0; i < MaxBodies; i++)
            {
                var go = new GameObject("Pooled voxel rubble");
                // Default layer keeps debris out of the sandbox's terrain-only
                // layer-8 height raycasts while retaining physical collisions.
                go.SetActive(false); go.transform.SetParent(transform, false);
                go.AddComponent<MeshFilter>().sharedMesh = cubeMesh;
                var renderer = go.AddComponent<MeshRenderer>(); renderer.sharedMaterial = material;
                var collider = go.AddComponent<BoxCollider>(); collider.size = Vector3.one * .92f; collider.sharedMaterial = contactMaterial;
                var body = go.AddComponent<Rigidbody>();
                var probe = go.AddComponent<ContactProbe>(); probe.Initialize(this, body);
                available.Push(new Piece { Object = go, Renderer = renderer, Body = body, Collider = collider, Probe = probe });
            }
        }

        internal void Emit(IReadOnlyList<VoxelFragment> cells, Vector3 velocity, Vector3 origin, float energy)
        { EmitInternal(cells, velocity, origin, energy, false, Vector3.zero); }
        internal void EmitConnected(IReadOnlyList<VoxelFragment> cells, Vector3 velocity, Vector3 origin, float energy)
        { EmitInternal(cells, velocity, origin, energy, false, Vector3.zero); }
        internal Emission EmitImpact(IReadOnlyList<VoxelFragment> cells, Vector3 velocity, Vector3 origin, Vector3 normal)
        { return EmitInternal(cells, velocity, origin, 0f, true, normal); }

        internal static int SampleCount(int cellCount, int freeSlots)
        { return Mathf.Min(Mathf.Max(0, cellCount), Mathf.Min(MaxPiecesPerEmission, Mathf.Max(0, freeSlots))); }
        internal static int SampleIndex(int index, int cellCount, int sampleCount)
        { return (int)(((2L * index + 1) * cellCount) / (2L * sampleCount)); }
        internal static int RepresentedCellCount(int index, int cellCount, int sampleCount)
        { return (int)((long)(index + 1) * cellCount / sampleCount - (long)index * cellCount / sampleCount); }
        internal static float SampleMass(float totalMass, int sampleCount)
        { return Mathf.Max(MinimumMass, totalMass / sampleCount); }
        internal static float BlockSize(float cellSize) => Mathf.Min(cellSize, MaximumBlockSize);
        internal static float ContactOffset(float blockSize) => Mathf.Clamp(blockSize * .92f * .02f, .0001f, .004f);
        internal static float Mass(IReadOnlyList<VoxelFragment> cells)
        {
            float mass = 0f;
            for (int i = 0; i < cells.Count; i++) mass += Density * cells[i].Size * cells[i].Size * cells[i].Size;
            return mass;
        }
        internal static bool ShouldBeginFade(float age, bool sleeping)
        { return age >= MinimumVisibleSeconds && (sleeping || age >= MaximumActiveSeconds); }
        private static double Milliseconds(long ticks) => ticks * 1000.0 / Stopwatch.Frequency;

        private Emission EmitInternal(IReadOnlyList<VoxelFragment> cells, Vector3 velocity, Vector3 origin, float energy, bool impact, Vector3 normal)
        {
            if (cells == null || cells.Count == 0) return null;
            long start = Stopwatch.GetTimestamp();
            int count = SampleCount(cells.Count, available.Count - reserved);
            EmissionCount++; LastEmissionCells = cells.Count; LastEmissionGroups = count;
            MinimumEmissionGroups = Mathf.Min(MinimumEmissionGroups, count);
            if (count == 1) SingleClusterEmissions++;
            UnrenderedCells += cells.Count - count;
            if (count == 0)
            {
                DroppedEmissions++;
                LastSpawnMilliseconds = 0; LastEmissionMilliseconds = Milliseconds(Stopwatch.GetTimestamp() - start);
                return new Emission { Spawned = true };
            }
            var copy = new VoxelFragment[cells.Count];
            float mass = 0;
            for (int i = 0; i < cells.Count; i++) { copy[i] = cells[i]; mass += Density * cells[i].Size * cells[i].Size * cells[i].Size; }
            var emission = new Emission { Cells = copy, Velocity = velocity, Origin = origin, Energy = Mathf.Max(0, energy),
                TotalMass = mass, Count = count, Impact = impact };
            if (impact && normal.sqrMagnitude > .000001f)
                emission.Offset = ImpactClearance(copy, count, origin, normal.normalized);
            reserved += count;
            pending.Enqueue(emission);
            Drain();
            LastEmissionMilliseconds = Milliseconds(Stopwatch.GetTimestamp() - start);
            return emission;
        }

        internal static Vector3 ImpactClearance(IReadOnlyList<VoxelFragment> cells, int count, Vector3 point, Vector3 normal)
        {
            float lowest = float.PositiveInfinity;
            for (int i = 0; i < count; i++)
            {
                var c = cells[SampleIndex(i, cells.Count, count)];
                float half = BlockSize(c.Size) * .5f;
                float projection = Vector3.Dot(c.Position - point, normal) - half * (Mathf.Abs(normal.x) + Mathf.Abs(normal.y) + Mathf.Abs(normal.z));
                lowest = Mathf.Min(lowest, projection);
            }
            return count == 0 ? Vector3.zero : normal * Mathf.Max(0f, .005f - lowest);
        }

        private void Drain()
        {
            if (budgetFrame != Time.frameCount) { budgetFrame = Time.frameCount; frameActivations = 0; }
            while (pending.Count > 0 && frameActivations < MaxActivationsPerFrame)
            {
                var e = pending.Peek();
                long start = Stopwatch.GetTimestamp();
                while (e.Next < e.Count && frameActivations < MaxActivationsPerFrame)
                {
                    var cell = e.Cells[SampleIndex(e.Next, e.Cells.Length, e.Count)];
                    int represented = RepresentedCellCount(e.Next, e.Cells.Length, e.Count);
                    Activate(available.Pop(), cell, e, represented);
                    e.Next++; reserved--; frameActivations++; SampledCells++;
                }
                long ticks = Stopwatch.GetTimestamp() - start;
                e.SpawnTicks += ticks; SpawnMilliseconds += Milliseconds(ticks);
                LastSpawnMilliseconds = Milliseconds(e.SpawnTicks);
                PeakActivationsPerFrame = Mathf.Max(PeakActivationsPerFrame, frameActivations);
                if (e.Next < e.Count) break;
                pending.Dequeue(); EmittedCells += e.Cells.Length; e.Spawned = true;
            }
        }

        private void Activate(Piece p, VoxelFragment cell, Emission e, int represented)
        {
            float size = BlockSize(cell.Size);
            p.Object.transform.SetPositionAndRotation(cell.Position + e.Offset, Quaternion.identity);
            p.SpawnPosition=cell.Position+e.Offset;
            p.Object.transform.localScale = Vector3.one * size;
            p.Collider.contactOffset = ContactOffset(size);
            properties.Clear(); properties.SetColor(ColorId, cell.Color); properties.SetFloat(AlphaId, 1f);
            properties.SetFloat(EmissionId, e.Impact ? .45f : 0f);
            p.Renderer.SetPropertyBlock(properties);
            p.Object.SetActive(true);
            p.Body.mass = SampleMass(e.TotalMass, e.Count);
            // Fast fragments sweep against static walls. Fragment-to-fragment
            // contacts use discrete physics steps unless the other body requests
            // ContinuousDynamic; the intact projectile remains ContinuousDynamic.
            // Fine character pieces can move farther than their diameter in
            // one 120Hz step even at ordinary falling speed.
            p.Body.collisionDetectionMode = CollisionDetectionMode.Continuous;
            p.Body.linearVelocity = e.Velocity; p.Body.angularVelocity = Vector3.zero;
            if (e.Energy > 0f)
            {
                var direction = cell.Position - e.Origin;
                if (direction.sqrMagnitude > .000001f)
                    p.Body.linearVelocity += direction.normalized * Mathf.Min(5f, Mathf.Sqrt(2f * e.Energy / Mathf.Max(MinimumMass, e.TotalMass)));
            }
            p.Probe.ResetProbe(p.Body.linearVelocity); p.Born = Time.time; p.FadeStart = -1f; p.RepresentedCells = represented;
            active.Add(p); PeakBodies = Mathf.Max(PeakBodies, active.Count);
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
            for (int i = active.Count - 1; i >= 0; i--)
            {
                var p = active[i]; float age = Time.time - p.Born;
                if (p.FadeStart < 0f && ShouldBeginFade(age, p.Body.IsSleeping()))
                {
                    p.FadeStart = Time.time;
                    if (p.Body.IsSleeping()) SettledPieces++; else ForcedFadePieces++;
                }
                if (p.FadeStart < 0f) continue;
                float alpha = 1f - (Time.time - p.FadeStart) / FadeSeconds;
                if (alpha <= 0f)
                {
                    DisposedCells += p.RepresentedCells; p.Object.SetActive(false);
                    active.RemoveAt(i); available.Push(p); continue;
                }
                p.Renderer.GetPropertyBlock(properties); properties.SetFloat(AlphaId, alpha); p.Renderer.SetPropertyBlock(properties);
            }
            Drain();
            ManagementMilliseconds += Milliseconds(Stopwatch.GetTimestamp() - start);
        }

        private void OnDestroy()
        {
            pending.Clear(); active.Clear(); available.Clear();
            if (cubeMesh != null) Destroy(cubeMesh);
            if (material != null) Destroy(material);
            if (contactMaterial != null) Destroy(contactMaterial);
        }
    }
}
