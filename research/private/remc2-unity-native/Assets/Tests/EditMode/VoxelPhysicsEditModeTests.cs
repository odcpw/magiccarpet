using System.Collections.Generic;
using System.IO;
using System.Linq;
using MagicCarpet.Native.Voxels;
using NUnit.Framework;
using UnityEngine;

namespace MagicCarpet.Native.Tests
{
    public sealed class VoxelPhysicsEditModeTests
    {
        [System.Serializable] private sealed class ImpactTerrainFixture
        {
            public int centerX, centerZ, radius;
            public string heights, types, angles;
        }

        // Explicit opt-in diagnostic: the local 13x13 source-plane sample is
        // exported from the actual saved benchmark; this is not gameplay proof.
        [Test] public void ProfileNativeImpactCcdOnCapturedTerrain()
        {
            var fixturePath = System.Environment.GetEnvironmentVariable("MC_VOXEL_PROFILE_TERRAIN_JSON");
            if (string.IsNullOrEmpty(fixturePath)) Assert.Ignore("Set MC_VOXEL_PROFILE_TERRAIN_JSON for isolated PhysX profiling");
            var data = JsonUtility.FromJson<ImpactTerrainFixture>(File.ReadAllText(fixturePath));
            var heights = new byte[65536]; var types = new byte[65536]; var angles = new byte[65536];
            System.Array.Fill(heights, (byte)125); System.Array.Fill(types, (byte)22); System.Array.Fill(angles, (byte)129);
            var source = new[] { System.Convert.FromBase64String(data.heights), System.Convert.FromBase64String(data.types), System.Convert.FromBase64String(data.angles) };
            var dest = new[] { heights, types, angles }; int side = data.radius * 2 + 1;
            Assert.That(source.All(a => a.Length == side * side), Is.True);
            for (int z = -data.radius; z <= data.radius; z++) for (int x = -data.radius; x <= data.radius; x++)
                for (int k = 0; k < 3; k++) dest[k][((data.centerZ + z) & 255) * 256 + ((data.centerX + x) & 255)] = source[k][(z + data.radius) * side + x + data.radius];
            var priorMode = Physics.simulationMode; var priorSolver = Physics.defaultSolverIterations; var priorVelocitySolver = Physics.defaultSolverVelocityIterations;
            var root = new GameObject("Captured native-impact PhysX diagnostic");
            var material = new Material(Shader.Find("Hidden/InternalErrorShader"));
            var physicsMaterial = new PhysicsMaterial("Profile impact contact") { dynamicFriction = .65f, staticFriction = .8f,
                bounciness = .36f, bounceCombine = PhysicsMaterialCombine.Maximum };
            ModernTerrainMesh terrain = null; VoxelTerrainContact contact = null;
            try
            {
                Physics.simulationMode = SimulationMode.Script; Physics.defaultSolverIterations = 8; Physics.defaultSolverVelocityIterations = 2;
                terrain = new ModernTerrainMesh(root.transform, material, material) { ViewDistance = 0 };
                terrain.Update(heights, types, angles, 1, 0);
                contact = new VoxelTerrainContact(root.transform, terrain.SampleHeight);
                contact.Update(new Vector3(212, 66.5f, 808), new Bounds());
                Assert.That(terrain.SampleHeight(212, 808), Is.EqualTo(65.0975f).Within(.01f));
                var cube = new List<Vector3>();
                for (int z = -4; z <= 4; z++) for (int y = -4; y <= 4; y++) for (int x = -4; x <= 4; x++)
                    if (x*x + y*y + z*z <= 16) cube.Add(new Vector3(x, y, z) * .07f);
                Assert.That(cube.Count, Is.EqualTo(257));
                var samples = new Vector3[192];
                for (var i = 0; i < samples.Length; i++) samples[i] = cube[VoxelRubble.SampleIndex(i, cube.Count, samples.Length)];
                var incoming = new[] { new Vector3(-52, -9, -379) * (30f/64), new Vector3(-52, -4, -379) * (30f/64) };
                var times = new[] { new List<double>(), new List<double>() };
                var end = new[] { new Vector3[384], new Vector3[384] };
                var overlaps = new int[2]; var penetrations = new int[2];
                for (int run = 0; run < 12; run++)
                {
                    int variant = run & 1;
                    var mode = variant == 0 ? CollisionDetectionMode.Continuous : CollisionDetectionMode.ContinuousSpeculative;
                    var bodies = new Rigidbody[384];
                    for (var i = 0; i < bodies.Length; i++)
                    {
                        int cloud = i / 192;
                        var go = new GameObject("Profile impact cube"); go.transform.SetParent(root.transform, false);
                        // Two 192-cell clouds are separated by 1m laterally;
                        // both variants start with exactly the same 384 states.
                        go.transform.position = new Vector3(212 + (cloud == 0 ? -.5f : .5f), 66.5f, 808) + samples[i % 192];
                        go.transform.localScale = Vector3.one * .07f;
                        var collider = go.AddComponent<BoxCollider>(); collider.size = Vector3.one * .92f; collider.sharedMaterial = physicsMaterial;
                        collider.contactOffset = VoxelRubble.ContactOffset(.07f);
                        var body = go.AddComponent<Rigidbody>(); body.mass = 257 * .07f * .07f * .07f / 192;
                        body.collisionDetectionMode = mode; body.linearVelocity = incoming[cloud]; bodies[i] = body;
                    }
                    Physics.SyncTransforms();
                    Assert.That(bodies.Length, Is.EqualTo(384));
                    foreach (var body in bodies) Assert.That(body.position.y - .0322f, Is.GreaterThan(terrain.SampleHeight(body.position.x, body.position.z)));
                    for (int turn = 0; turn < 2; turn++)
                    {
                        var timer = System.Diagnostics.Stopwatch.StartNew();
                        for (int step = 0; step < 4; step++) Physics.Simulate(1f / 120);
                        timer.Stop(); times[variant].Add(timer.Elapsed.TotalMilliseconds);
                        var found = new Collider[64];
                        foreach (var body in bodies)
                        {
                            // Diagnostic overlap queries are outside the timed
                            // step; EditMode does not issue collision callbacks.
                            var count = Physics.OverlapBoxNonAlloc(body.position, Vector3.one * .0322f, found, body.rotation);
                            for (var i = 0; i < count; i++) if (found[i].attachedRigidbody != body) overlaps[variant]++;
                            if (body.position.y < terrain.SampleHeight(body.position.x, body.position.z) - .02f) penetrations[variant]++;
                        }
                    }
                    if (run < 2) for (var i = 0; i < bodies.Length; i++) end[variant][i] = bodies[i].position;
                    foreach (var body in bodies) UnityEngine.Object.DestroyImmediate(body.gameObject);
                }
                var sortedA = times[0].OrderBy(v => v).ToArray(); var sortedB = times[1].OrderBy(v => v).ToArray();
                float maxDifference = 0; for (var i = 0; i < 384; i++) maxDifference = Mathf.Max(maxDifference, Vector3.Distance(end[0][i], end[1][i]));
                UnityEngine.Debug.Log($"MC_VOXEL_PHYSX_AB samples={sortedA.Length} sweep_median_ms={sortedA[sortedA.Length/2]:F4} sweep_p95_ms={sortedA[(sortedA.Length-1)*95/100]:F4} speculative_median_ms={sortedB[sortedB.Length/2]:F4} speculative_p95_ms={sortedB[(sortedB.Length-1)*95/100]:F4} sweep_overlaps={overlaps[0]} speculative_overlaps={overlaps[1]} sweep_below={penetrations[0]} speculative_below={penetrations[1]} first_run_max_position_delta={maxDifference:F4}");
                Assert.That(penetrations[0], Is.Zero, "Sweep baseline tunneled below the sampled terrain");
                Assert.That(penetrations[1], Is.Zero, "Speculative candidate tunneled below the sampled terrain");
            }
            finally
            {
                contact?.Dispose(); terrain?.Dispose(); UnityEngine.Object.DestroyImmediate(root); UnityEngine.Object.DestroyImmediate(material); UnityEngine.Object.DestroyImmediate(physicsMaterial);
                Physics.simulationMode = priorMode; Physics.defaultSolverIterations = priorSolver; Physics.defaultSolverVelocityIterations = priorVelocitySolver;
            }
        }

        [TestCase(.025f)] [TestCase(.125f)] [TestCase(.2f)]
        public void DebrisCannotTunnelThroughThinTerrainAt120Hz(float size)
        {
            var mode=Physics.simulationMode;
            var root=new GameObject("Fine debris contact regression");
            var floor=new GameObject("Thin triangle terrain");
            var mesh=new Mesh {vertices=new[]{new Vector3(-2,0,-2),new Vector3(-2,0,2),new Vector3(2,0,-2),new Vector3(2,0,2)},triangles=new[]{0,1,2,2,1,3}};
            floor.AddComponent<MeshCollider>().sharedMesh=mesh;
            try
            {
                Physics.simulationMode=SimulationMode.Script;
                var rubble=root.AddComponent<VoxelRubble>();
                // EditMode does not run the MonoBehaviour lifecycle itself.
                typeof(VoxelRubble).GetMethod("Awake",System.Reflection.BindingFlags.Instance|System.Reflection.BindingFlags.NonPublic).Invoke(rubble,null);
                rubble.Emit(new[]{new VoxelFragment(new Vector3(0,.2f,0),size,new Color32(90,90,90,255))},Vector3.down*30,Vector3.zero,0);
                Physics.SyncTransforms();
                Assert.That(rubble.ActiveBodies,Is.EqualTo(1));
                for(int i=0;i<12;i++)Physics.Simulate(1f/120);
                Assert.That(rubble.LowestBodyY,Is.GreaterThanOrEqualTo(-.015f),"A small falling voxel must contact the real triangle surface, not pass between physics steps");
                // EditMode Physics.Simulate resolves contacts but does not send
                // MonoBehaviour collision callbacks; inspect body motion here.
            }
            finally {Physics.simulationMode=mode;Object.DestroyImmediate(root);Object.DestroyImmediate(floor);Object.DestroyImmediate(mesh);}
        }

        [Test]
        public void UnknownNormalImpactClearsCommittedTerrainWhileNativeEditIsPending()
        {
            var mode=Physics.simulationMode;
            var root=new GameObject("Pending native ground impact");root.layer=8;
            var material=new Material(Shader.Find("Hidden/InternalErrorShader"));
            VoxelVolume ground=null;
            try
            {
                Physics.simulationMode=SimulationMode.Script;
                ground=new VoxelVolume(root.transform,new Vector3(224,48.625f,770),new Vector3Int(32,10,96),.125f,material);
                for(int z=0;z<96;z++)for(int y=0;y<8;y++)for(int x=0;x<32;x++)
                    ground.SetCell(x,y,z,new Color32(90,90,90,255));
                // The real contact neighborhood climbs by two voxel layers
                // beside the center column, so a center plane alone is unsafe.
                for(int z=0;z<96;z++)for(int x=14;x<32;x++)for(int y=8;y<10;y++)
                    ground.SetCell(x,y,z,new Color32(90,90,90,255));
                ground.Build();
                // Native has removed cells, but this visible chunk has not yet
                // been processed by the bounded presentation edit.
                var removed=new List<Vector3Int>();
                for(int y=4;y<8;y++)removed.Add(new Vector3Int(13,y,67));
                removed.Add(new Vector3Int(16,9,67));
                var edit=ground.BeginRemoveCells(removed);
                Assert.That(edit.Complete,Is.False);
                Physics.SyncTransforms();
                var from=new Vector3(14445,3259,49817)/64f;
                var point=new Vector3(14445,3152,49817)/64f;
                Assert.That(ground.RaycastSurface(from,point,out var hit),Is.True);
                Assert.That(hit.point.y,Is.EqualTo(3176/64f).Within(.001f));
                var fragments=new List<VoxelFragment>();
                for(int z=-4;z<=4;z++)for(int y=-4;y<=4;y++)for(int x=-4;x<=4;x++)
                    if(x*x+y*y+z*z<=16)fragments.Add(new VoxelFragment(point+new Vector3(x,y,z)*.07f,.07f,new Color32(255,120,12,255)));
                Assert.That(fragments.Count,Is.EqualTo(257));
                var original=fragments.Select(f=>f.Position).ToArray();
                Assert.That(VoxelModernWorld.TryUnknownGroundClearance(ground,from,point,fragments,out var offset),Is.True);
                Assert.That(offset.y,Is.GreaterThan(.65f),"The anchor must be the committed surface, not the lower receipt height");
                for(int i=0;i<fragments.Count;i++)
                {
                    Assert.That(Vector3.Distance(fragments[i].Position-original[i],offset),Is.LessThan(.0001f));
                    Assert.That(fragments[i].Position.y-fragments[i].Size*.5f,Is.GreaterThan(hit.point.y));
                    var f=fragments[i];var half=f.Size*.5f;
                    foreach(float dx in new[]{-half,half})foreach(float dz in new[]{-half,half})
                    {
                        Assert.That(ground.TryColumn(f.Position.x+dx,f.Position.z+dz,out var column),Is.True);
                        Assert.That(ground.TryColumnTop(column,out var top),Is.True);
                        Assert.That(f.Position.y-half,Is.GreaterThan(top),"Every footprint must clear its committed stepped column");
                    }
                }
                var rubble=root.AddComponent<VoxelRubble>();
                typeof(VoxelRubble).GetMethod("Awake",System.Reflection.BindingFlags.Instance|System.Reflection.BindingFlags.NonPublic).Invoke(rubble,null);
                var velocity=new Vector3(-19.21875f,-88.125f,-155.15625f);
                rubble.EmitImpact(fragments,velocity,point,Vector3.zero);
                Physics.SyncTransforms();
                Assert.That(rubble.ActiveBodies,Is.GreaterThan(0));
                var active=root.GetComponentsInChildren<Rigidbody>(true).Where(body=>body.gameObject.activeInHierarchy).ToArray();
                Assert.That(active.All(body=>body.linearVelocity==velocity),Is.True,"Clearance must not change native incoming momentum");
                for(int step=0;step<4;step++)Physics.Simulate(1f/120);
                Assert.That(active.Min(body=>body.position.y),Is.GreaterThan(49.375f),"Fast fragments must not pass below the still-committed terrain");

                // Commit the center column's chunk while another chunk remains
                // pending: the central ray now misses, but adjacent stepped
                // colliders still intersect the original sphere footprint.
                edit.Step(1);Physics.SyncTransforms();
                Assert.That(edit.Complete,Is.False);
                Assert.That(ground.RaycastSurface(from,point,out _),Is.False);
                var adjacent=original.Select(position=>new VoxelFragment(position,.07f,new Color32(255,120,12,255))).ToList();
                Assert.That(VoxelModernWorld.TryUnknownGroundClearance(ground,from,point,adjacent,out var adjacentOffset),Is.True);
                Assert.That(adjacentOffset.y,Is.GreaterThan(.65f));
                foreach(var f in adjacent)
                {
                    var half=f.Size*.5f;
                    foreach(float dx in new[]{-half,half})foreach(float dz in new[]{-half,half})
                        if(ground.TryColumn(f.Position.x+dx,f.Position.z+dz,out var column) && ground.TryColumnTop(column,out var top))
                            Assert.That(f.Position.y-half,Is.GreaterThan(top),"Partially committed center must not hide a neighboring collision");
                }
            }
            finally {Physics.simulationMode=mode;ground?.Dispose();Object.DestroyImmediate(root);Object.DestroyImmediate(material);}
        }

        [Test] public void EmissionSamplingIsBoundedDeterministicAndAccountsForEverySourceCell()
        {
            const int sourceCount = 10099;
            int count = VoxelRubble.SampleCount(sourceCount, VoxelRubble.MaxBodies);
            Assert.That(count, Is.EqualTo(VoxelRubble.MaxPiecesPerEmission));
            Assert.That(VoxelRubble.SampleCount(sourceCount, 17), Is.EqualTo(17));
            Assert.That(VoxelRubble.SampleCount(sourceCount, 0), Is.Zero);
            Assert.That(VoxelRubble.SampleCount(3, 100), Is.EqualTo(3));
            Assert.That(VoxelRubble.MaxActivationsPerFrame, Is.LessThan(VoxelRubble.MaxPiecesPerEmission));
            Assert.That(VoxelRubble.MaxPiecesPerEmission, Is.LessThanOrEqualTo(VoxelRubble.MaxBodies));
            int previous = -1, represented = 0;
            for (int i = 0; i < count; i++)
            {
                int selected = VoxelRubble.SampleIndex(i, sourceCount, count);
                Assert.That(selected, Is.GreaterThan(previous).And.LessThan(sourceCount));
                Assert.That(VoxelRubble.SampleIndex(i, sourceCount, count), Is.EqualTo(selected));
                int weight = VoxelRubble.RepresentedCellCount(i, sourceCount, count);
                Assert.That(weight, Is.GreaterThan(0));
                represented += weight; previous = selected;
            }
            Assert.That(represented, Is.EqualTo(sourceCount));
        }

        [Test] public void CharacterDeathRetriesUnavailableEmissionOnlyUntilItSucceeds()
        {
            var lifetime = new VoxelDeathLifecycle();
            int attempts = 0;
            Assert.That(lifetime.TryExplode(false, () => { attempts++; return true; }), Is.False);
            Assert.That(lifetime.TryExplode(true, () => { attempts++; return false; }), Is.False);
            Assert.That(lifetime.RetainAfterRemoval(true), Is.True, "Keep the frozen lifetime while the pool remains full, regardless of death animation duration");
            for (var i = 0; i < 100; i++)
                Assert.That(lifetime.RetainAfterRemoval(true), Is.True);
            Assert.That(lifetime.TryExplode(true, () => { attempts++; return true; }), Is.True);
            Assert.That(lifetime.RetainAfterRemoval(true), Is.False);
            Assert.That(lifetime.TryExplode(true, () => { attempts++; return true; }), Is.True);
            Assert.That(attempts, Is.EqualTo(2));
            var reusedSlot = new VoxelDeathLifecycle();
            Assert.That(reusedSlot.Exploded, Is.False);
            Assert.That(reusedSlot.TryExplode(true, () => { attempts++; return true; }), Is.True);
            Assert.That(attempts, Is.EqualTo(3));
        }

        [Test] public void LivingArcherJoinActionIsNotADeath()
        {
            Assert.That(ModernCharacterActors.SourceVoxelDeath(4, 36, 100), Is.False);
            Assert.That(ModernCharacterActors.SourceVoxelDeath(4, 36, -1), Is.True);
            Assert.That(ModernCharacterActors.SourceVoxelDeath(4, 37, 100), Is.True);
            Assert.That(ModernCharacterActors.SourceVoxelDeath(1, 12, 100), Is.True);
            Assert.That(ModernCharacterActors.SourceVoxelDeath(1, 0, 0), Is.False);
        }

        [Test] public void FireballRowsCannotBeArcherArrowsOrReleaseSignals()
        {
            var row = new NativeSim.CampaignCharacterState { classId = 9, model = 0, hasParent = 1, ageTicks = 0 };
            Assert.That(ModernCharacterActors.IsArcherArrow(row), Is.False);
            row.model = 28;
            Assert.That(ModernCharacterActors.IsArcherArrow(row), Is.False);
            row.model = 13;
            Assert.That(ModernCharacterActors.IsArcherArrow(row), Is.True);
            row.classId = 5;
            Assert.That(ModernCharacterActors.IsArcherArrow(row), Is.False);
        }

        [Test] public void CharacterBreakupImmediatelyActivatesColoredSpinningPhysicalCubes()
        {
            var root = new GameObject("Character explosion regression");
            try
            {
                var rubble = root.AddComponent<VoxelRubble>();
                typeof(VoxelRubble).GetMethod("Awake", System.Reflection.BindingFlags.Instance | System.Reflection.BindingFlags.NonPublic).Invoke(rubble, null);
                var color = new Color32(89, 130, 220, 255);
                var cells = new[] { new VoxelFragment(new Vector3(-1, 2, 0), .05f, color),
                    new VoxelFragment(new Vector3(1, 2, 0), .05f, color) };
                Assert.That(rubble.EmitCharacter(cells, Vector3.zero, Vector3.up, 180), Is.True);
                Assert.That(rubble.ActiveBodies, Is.EqualTo(2));
                var bodies = root.GetComponentsInChildren<Rigidbody>();
                Assert.That(bodies.Length, Is.EqualTo(2));
                foreach (var body in bodies)
                {
                    Assert.That(body.linearVelocity.magnitude, Is.GreaterThan(1));
                    Assert.That(body.angularVelocity.magnitude, Is.GreaterThan(1));
                    Assert.That(body.transform.localScale.x, Is.EqualTo(.0875f).Within(.00001f));
                    Assert.That(body.GetComponent<BoxCollider>().size.x, Is.EqualTo(.92f).Within(.00001f));
                    var properties = new MaterialPropertyBlock();
                    body.GetComponent<Renderer>().GetPropertyBlock(properties);
                    Assert.That((Color32)properties.GetColor("_BaseColor"), Is.EqualTo(color));
                }
            }
            finally { Object.DestroyImmediate(root); }
        }

        [Test] public void CharacterClearanceMovesWholePoseTogetherAndClearsSlopedGround()
        {
            var cells = new[] {
                new VoxelFragment(new Vector3(-.4f, -.08f, 0), .1f, new Color32(90, 40, 10, 255)),
                new VoxelFragment(new Vector3(.4f, .2f, 0), .1f, new Color32(10, 40, 90, 255)) };
            var sizes = VoxelRubble.CharacterSizes(cells, cells.Length);
            float Height(float x, float z) => .2f * x + .1f * z;
            var shift = VoxelRubble.CharacterGroundClearance(cells, cells.Length, sizes, Height);
            Assert.That(shift, Is.GreaterThan(0));
            for (var i = 0; i < cells.Length; i++)
            {
                float half = sizes[i] * .92f * .5f;
                for (int z = -1; z <= 1; z++) for (int x = -1; x <= 1; x++)
                    Assert.That(cells[i].Position.y + shift - half,
                        Is.GreaterThanOrEqualTo(Height(cells[i].Position.x + x * half, cells[i].Position.z + z * half) + .00499f));
            }
            var root = new GameObject("Whole pose clearance regression");
            try
            {
                var rubble = root.AddComponent<VoxelRubble>();
                typeof(VoxelRubble).GetMethod("Awake", System.Reflection.BindingFlags.Instance | System.Reflection.BindingFlags.NonPublic).Invoke(rubble, null);
                Assert.That(rubble.EmitCharacter(cells, Vector3.zero, Vector3.zero, 1, Height), Is.True);
                var bodies = root.GetComponentsInChildren<Rigidbody>().OrderBy(body => body.position.x).ToArray();
                Assert.That(bodies.Length, Is.EqualTo(2));
                Assert.That(Vector3.Distance(bodies[0].position - bodies[1].position, cells[0].Position - cells[1].Position), Is.LessThan(.00001f));
                Assert.That(bodies[0].position.y - cells[0].Position.y, Is.EqualTo(shift).Within(.00001f));
                var color = new MaterialPropertyBlock(); bodies[0].GetComponent<Renderer>().GetPropertyBlock(color);
                Assert.That((Color32)color.GetColor("_BaseColor"), Is.EqualTo(cells[0].Color));
                Assert.That(bodies[0].mass + bodies[1].mass, Is.EqualTo(VoxelRubble.Mass(cells)).Within(.00001f));
            }
            finally { Object.DestroyImmediate(root); }
        }

        [Test] public void BelowTerrainCountsSeparateImpactAndCharacterBodies()
        {
            var root = new GameObject("Debris provenance regression");
            try
            {
                var rubble = root.AddComponent<VoxelRubble>();
                typeof(VoxelRubble).GetMethod("Awake", System.Reflection.BindingFlags.Instance | System.Reflection.BindingFlags.NonPublic).Invoke(rubble, null);
                var impact = new[] { new VoxelFragment(Vector3.zero, .05f, new Color32(255, 120, 0, 255)) };
                var character = new[] { new VoxelFragment(Vector3.right, .05f, new Color32(100, 80, 60, 255)) };
                rubble.EmitImpact(impact, Vector3.zero, Vector3.zero, Vector3.zero);
                Assert.That(rubble.EmitCharacter(character, Vector3.zero, Vector3.zero, 0), Is.True);
                float Floor(float x, float z) => 1;
                Assert.That(rubble.CountBelowSurface(Floor), Is.EqualTo(2));
                Assert.That(rubble.CountBelowSurface(Floor, VoxelRubble.RubbleKind.Impact), Is.EqualTo(1));
                Assert.That(rubble.CountBelowSurface(Floor, VoxelRubble.RubbleKind.Character), Is.EqualTo(1));
                Assert.That(rubble.CountBelowSurface(Floor, VoxelRubble.RubbleKind.Other), Is.Zero);
            }
            finally { Object.DestroyImmediate(root); }
        }

        [Test] public void ResetClearsActiveAndQueuedPiecesWithoutDestroyingPool()
        {
            var root = new GameObject("Rubble reset regression");
            try
            {
                var rubble = root.AddComponent<VoxelRubble>();
                typeof(VoxelRubble).GetMethod("Awake", System.Reflection.BindingFlags.Instance | System.Reflection.BindingFlags.NonPublic).Invoke(rubble, null);
                var cells = new List<VoxelFragment>();
                for (var i = 0; i < 100; i++)
                    cells.Add(new VoxelFragment(new Vector3(i * 100f, 2, 0), .05f, new Color32(60, 90, 120, 255)));
                float Floor(float x, float z) => 3;
                var sizes = VoxelRubble.CharacterSizes(cells, cells.Count);
                var lift = VoxelRubble.CharacterGroundClearance(cells, cells.Count, sizes, Floor);
                Assert.That(rubble.EmitCharacter(cells, Vector3.zero, Vector3.up, 180, Floor), Is.True);
                Assert.That(rubble.ActiveBodies, Is.EqualTo(VoxelRubble.MaxActivationsPerFrame));
                Assert.That(rubble.PendingEmissions, Is.EqualTo(1));
                Assert.That(rubble.ActivePositions.Count, Is.EqualTo(cells.Count));
                Assert.That(rubble.ActivePositions, Does.Contain(cells[64].Position + Vector3.up * lift));
                Assert.That(rubble.ActivePositions, Does.Contain(cells[99].Position + Vector3.up * lift),
                    "Every distant queued sample must retain its actual offset contact tile");
                rubble.ClearEmissions();
                Assert.That(rubble.ActiveBodies, Is.Zero);
                Assert.That(rubble.PendingEmissions, Is.Zero);
                Assert.That(rubble.ActivePositions, Is.Empty);
                Assert.That(root.GetComponentsInChildren<Rigidbody>(true).Length, Is.EqualTo(VoxelRubble.MaxBodies));
            }
            finally { Object.DestroyImmediate(root); }
        }

        [Test] public void SaturatedPoolRejectsCharacterDeathForLaterRetry()
        {
            var root = new GameObject("Character explosion saturation regression");
            try
            {
                var rubble = root.AddComponent<VoxelRubble>();
                typeof(VoxelRubble).GetMethod("Awake", System.Reflection.BindingFlags.Instance | System.Reflection.BindingFlags.NonPublic).Invoke(rubble, null);
                var cells = new List<VoxelFragment>();
                for (var i = 0; i < VoxelRubble.MaxPiecesPerEmission; i++)
                    cells.Add(new VoxelFragment(new Vector3(i * .1f, 2, 0), .05f, new Color32(60, 90, 120, 255)));
                for (var i = 0; i < VoxelRubble.MaxBodies / VoxelRubble.MaxPiecesPerEmission; i++)
                    Assert.That(rubble.EmitCharacter(cells, Vector3.zero, Vector3.up, 180), Is.True);
                Assert.That(rubble.EmitCharacter(cells, Vector3.zero, Vector3.up, 180), Is.False);
                Assert.That(rubble.EmissionCount, Is.EqualTo(4), "A rejected death must not count as emitted");
                rubble.ClearEmissions();
                Assert.That(rubble.EmitCharacter(cells, Vector3.zero, Vector3.up, 180), Is.True);
            }
            finally { Object.DestroyImmediate(root); }
        }

        [Test] public void ResetCancelsQueuedAndAlreadySpawnedImpactEmissions()
        {
            var root = new GameObject("Impact cancellation regression");
            try
            {
                var rubble = root.AddComponent<VoxelRubble>();
                typeof(VoxelRubble).GetMethod("Awake", System.Reflection.BindingFlags.Instance | System.Reflection.BindingFlags.NonPublic).Invoke(rubble, null);
                var cell = new[] { new VoxelFragment(Vector3.up, .05f, new Color32(60, 90, 120, 255)) };
                var spawned = rubble.EmitImpact(cell, Vector3.zero, Vector3.zero, Vector3.up);
                Assert.That(spawned.Spawned, Is.True);
                var cells = new List<VoxelFragment>();
                for (var i = 0; i < VoxelRubble.MaxPiecesPerEmission; i++)
                    cells.Add(new VoxelFragment(new Vector3(i * .1f, 2, 0), .05f, new Color32(60, 90, 120, 255)));
                Assert.That(rubble.EmitCharacter(cells, Vector3.zero, Vector3.up, 180), Is.True);
                var queued = rubble.EmitImpact(cell, Vector3.zero, Vector3.zero, Vector3.up);
                Assert.That(queued.Spawned, Is.False);
                Assert.That(spawned.Cancelled || queued.Cancelled, Is.False);
                rubble.ClearEmissions();
                Assert.That(spawned.Cancelled, Is.True);
                Assert.That(queued.Cancelled, Is.True);
            }
            finally { Object.DestroyImmediate(root); }
        }

        [Test] public void SmallBlocksBoundDimensionsAndRetainIncomingMomentumByMassWeight()
        {
            var cells = new List<VoxelFragment>();
            for (int i = 0; i < 300; i++)
                cells.Add(new VoxelFragment(new Vector3(i, 0, 0), .3f, new Color32((byte)i, 30, 200, 255)));
            int count = VoxelRubble.SampleCount(cells.Count, VoxelRubble.MaxBodies);
            float mass = VoxelRubble.Mass(cells);
            float perBody = VoxelRubble.SampleMass(mass, count);
            Assert.That(perBody * count, Is.EqualTo(mass).Within(mass * .00001f));
            Vector3 incoming = new Vector3(3, -2, 4);
            Assert.That((incoming * perBody * count - incoming * mass).magnitude, Is.LessThan(.0001f));
            var colors = new HashSet<Color32>();
            for (int i = 0; i < count; i++)
            {
                var sample = cells[VoxelRubble.SampleIndex(i, cells.Count, count)];
                Assert.That(VoxelRubble.BlockSize(sample.Size), Is.LessThanOrEqualTo(.2f));
                colors.Add(sample.Color);
            }
            Assert.That(colors.Count, Is.GreaterThan(100));
            Assert.That(VoxelRubble.BlockSize(.04f), Is.EqualTo(.04f));
        }

        [Test] public void SampledFireballCubesClearTheWallBeforeIncomingMotionResumes()
        {
            var cells = VoxelFireball.SphereCells(.8f, .1f);
            int count = VoxelRubble.SampleCount(cells.Count, VoxelRubble.MaxBodies);
            Assert.That(count, Is.GreaterThan(1));
            Vector3 normal = new Vector3(1, 1, 0).normalized, point = new Vector3(.5f, 0, 0);
            Vector3 shift = VoxelRubble.ImpactClearance(cells, count, point, normal);
            for (int i = 0; i < count; i++)
            {
                var c = cells[VoxelRubble.SampleIndex(i, cells.Count, count)];
                float half = VoxelRubble.BlockSize(c.Size) * .5f;
                float clearance = Vector3.Dot(c.Position + shift - point, normal) -
                    half * (Mathf.Abs(normal.x) + Mathf.Abs(normal.y) + Mathf.Abs(normal.z));
                Assert.That(clearance, Is.GreaterThanOrEqualTo(.0049f));
            }
            float mass = VoxelRubble.Mass(cells);
            Assert.That(VoxelFireball.ImpactEnergy(mass, new Vector3(3, 4, 0)), Is.EqualTo(12.5f * mass).Within(.0001f));
        }

        [Test] public void SampledFireballCubesStartWithoutMutualOverlap()
        {
            var cells = VoxelFireball.SphereCells(.8f, .1f);
            int count = VoxelRubble.SampleCount(cells.Count, VoxelRubble.MaxBodies);
            for (int i = 0; i < count; i++) for (int j = i + 1; j < count; j++)
            {
                var a = cells[VoxelRubble.SampleIndex(i, cells.Count, count)];
                var b = cells[VoxelRubble.SampleIndex(j, cells.Count, count)];
                Vector3 delta = a.Position - b.Position;
                float half = (VoxelRubble.BlockSize(a.Size) + VoxelRubble.BlockSize(b.Size)) * .5f;
                Assert.That(Mathf.Max(Mathf.Abs(delta.x), Mathf.Max(Mathf.Abs(delta.y), Mathf.Abs(delta.z))),
                    Is.GreaterThanOrEqualTo(half - .00001f));
            }
        }

        [Test] public void ContactOffsetScalesWithColliderAndStaysWithinGridGap()
        {
            foreach (float size in new[] { .0125f, .03f, .065f, .125f, .2f })
            {
                float offset = VoxelRubble.ContactOffset(size);
                Assert.That(offset, Is.EqualTo(size * .92f * .02f).Within(.0000001f));
                // Each collider has 92% of the visual cube's side, leaving an
                // 8% gap between equal adjacent cells on the source grid.
                Assert.That(2f * offset, Is.LessThan(.08f * size));
            }
            Assert.That(VoxelRubble.ContactOffset(.0001f), Is.EqualTo(.0001f));
            Assert.That(VoxelRubble.ContactOffset(10f), Is.EqualTo(.004f));
        }

        [Test] public void CohesiveFireballMeshStillContainsEverySourceCell()
        {
            var cells = VoxelFireball.SphereCells(1f, .0001f);
            Assert.That(cells.Count, Is.LessThan(5000));
            Assert.That(cells[0].Size, Is.EqualTo(.125f).Within(.00001f));
            var mesh = VoxelRubble.BuildMesh(cells, Vector3.zero);
            try
            {
                Assert.That(mesh.vertexCount, Is.EqualTo(cells.Count * 24));
                Assert.That(mesh.triangles.Length, Is.EqualTo(cells.Count * 36));
                Assert.That(mesh.colors32[0], Is.EqualTo(cells[0].Color));
            }
            finally { Object.DestroyImmediate(mesh); }
        }

        [Test] public void RubbleRemainsVisibleBeforeSleepAndReportsHardLifetimeFallback()
        {
            Assert.That(VoxelRubble.ShouldBeginFade(2.99f, true), Is.False);
            Assert.That(VoxelRubble.ShouldBeginFade(3f, true), Is.True);
            Assert.That(VoxelRubble.ShouldBeginFade(44.99f, false), Is.False);
            Assert.That(VoxelRubble.ShouldBeginFade(45f, false), Is.True);
            Assert.That(VoxelRubble.FadeSeconds, Is.EqualTo(2f));
        }
    }
}
