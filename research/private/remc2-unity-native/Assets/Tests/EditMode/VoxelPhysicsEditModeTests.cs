using System.Collections.Generic;
using MagicCarpet.Native.Voxels;
using NUnit.Framework;
using UnityEngine;

namespace MagicCarpet.Native.Tests
{
    public sealed class VoxelPhysicsEditModeTests
    {
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
