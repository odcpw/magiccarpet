using System;
using System.Diagnostics;
using System.IO;
using NUnit.Framework;
using UnityEngine;

namespace MagicCarpet.Native.Tests
{
    public sealed class NativeSessionEditModeTests
    {
        private const ulong ResetStateHash = 0x9da4bc91af58f3ce;
        private const ulong EightForwardStateHash = 0x956246bd7f270164;

        private string priorAssetCache;

        private static string PrivateRoot =>
            Path.GetFullPath(Path.Combine(Application.dataPath, "../.."));

        private static string PackagePath =>
            Path.Combine(PrivateRoot, "generated/level1-export-m");

        private static string ClosurePath =>
            Path.Combine(PrivateRoot, "generated/level1-asset-closure.json");

        private static string AssetCachePath =>
            Path.Combine(PrivateRoot, "generated/level1-unity-asset-cache");

        [SetUp]
        public void SetUp()
        {
            priorAssetCache =
                Environment.GetEnvironmentVariable("MC_ASSET_CACHE_PATH");
            Environment.SetEnvironmentVariable(
                "MC_ASSET_CACHE_PATH", AssetCachePath);
        }

        [TearDown]
        public void TearDown()
        {
            Environment.SetEnvironmentVariable(
                "MC_ASSET_CACHE_PATH", priorAssetCache);
        }

        [Test]
        public void AbiAndManagedLayoutsArePinned()
        {
            Assert.That(NativeSim.mc_native_abi_version(), Is.EqualTo(2));
            Assert.DoesNotThrow(NativeSim.AssertLayouts);
            Assert.That(
                NativeSim.mc_native_terrain_grid_vertex_count(),
                Is.EqualTo(840));
        }

        [Test]
        public void OpenResetDisposeAndTwoSessionsRemainIndependent()
        {
            var first = NativeSession.Open(PackagePath, ClosurePath);
            var second = NativeSession.Open(PackagePath, ClosurePath);
            try
            {
                var firstReset = first.Snapshot();
                var secondReset = second.Snapshot();
                Assert.That(firstReset.stateHash, Is.EqualTo(ResetStateHash));
                Assert.That(secondReset.stateHash, Is.EqualTo(ResetStateHash));

                var advanced = first.Step(0, 0, 1);
                Assert.That(advanced.tick, Is.EqualTo(1));
                Assert.That(second.Snapshot().tick, Is.EqualTo(0));

                var generation = advanced.generation;
                var reset = first.Reset();
                Assert.That(reset.generation, Is.Not.EqualTo(generation));
                Assert.That(reset.tick, Is.EqualTo(0));
                Assert.That(reset.stateHash, Is.EqualTo(ResetStateHash));
            }
            finally
            {
                first.Dispose();
                second.Dispose();
            }

            Assert.DoesNotThrow(first.Dispose);
            Assert.DoesNotThrow(second.Dispose);
            Assert.Throws<ObjectDisposedException>(() => _ = first.Handle);
            Assert.Throws<ObjectDisposedException>(() => _ = second.Handle);
        }

        [Test]
        public void GoatBulkRowsAreExactReusableResettableAndSessionLocal()
        {
            using var first = NativeSession.Open(PackagePath, ClosurePath);
            using var second = NativeSession.Open(PackagePath, ClosurePath);
            var firstReset = first.Snapshot();
            var secondReset = second.Snapshot();
            var firstGoats = first.Goats(
                firstReset.generation,
                firstReset.tick,
                out var firstBatch);
            var secondGoats = second.Goats(
                secondReset.generation,
                secondReset.tick,
                out var secondBatch);

            Assert.That(firstGoats, Has.Length.EqualTo(33));
            Assert.That(secondGoats, Has.Length.EqualTo(33));
            Assert.That(firstGoats[0].entityIndex, Is.EqualTo(50));
            Assert.That(firstGoats[1].entityIndex, Is.EqualTo(79));
            Assert.That(firstGoats[32].entityIndex, Is.EqualTo(111));
            Assert.That(firstGoats[0].authority, Is.EqualTo(NativeSim.GoatLive));
            Assert.That(firstGoats[0].classId, Is.EqualTo(5));
            Assert.That(firstGoats[0].model, Is.EqualTo(1));
            Assert.That(firstGoats[0].renderParameterIndex, Is.EqualTo(238));
            Assert.That(firstGoats[0].baseTmap, Is.EqualTo(152));
            Assert.That(firstGoats[0].animationFrame, Is.Zero);
            Assert.That(
                firstBatch.liveWorldHash,
                Is.EqualTo(secondBatch.liveWorldHash));

            var resetHash = firstBatch.liveWorldHash;
            var advanced = first.Step(0, 0, 1);
            var advancedGoats = first.Goats(
                advanced.generation,
                advanced.tick,
                out var advancedBatch);
            Assert.That(advancedGoats, Is.SameAs(firstGoats));
            Assert.That(advancedBatch.tick, Is.EqualTo(1));
            Assert.That(
                second.Goats(
                    secondReset.generation,
                    secondReset.tick,
                    out var unchangedBatch),
                Is.SameAs(secondGoats));
            Assert.That(unchangedBatch.liveWorldHash, Is.EqualTo(resetHash));

            var staleGeneration = advanced.generation;
            var reset = first.Reset();
            Assert.Throws<InvalidOperationException>(
                () => first.Goats(staleGeneration, 0, out _));
            var resetGoats = first.Goats(
                reset.generation,
                reset.tick,
                out var resetBatch);
            Assert.That(resetGoats, Is.SameAs(firstGoats));
            Assert.That(resetBatch.liveWorldHash, Is.EqualTo(resetHash));
            Assert.That(resetGoats[0].entityIndex, Is.EqualTo(50));
            Assert.That(resetGoats[0].animationFrame, Is.Zero);
        }

        [Test]
        public void KnownInputAndCameraRouteMatchesTheQualifiedGolden()
        {
            using var session = NativeSession.Open(PackagePath, ClosurePath);
            var reset = session.Snapshot();
            var resetPresentation = session.PresentationState();
            Assert.That(resetPresentation.tick, Is.EqualTo(0));
            Assert.That(resetPresentation.sourceTurn, Is.EqualTo(1));
            Assert.That(resetPresentation.cameraX, Is.EqualTo(reset.x));
            Assert.That(resetPresentation.cameraY, Is.EqualTo(reset.y));
            Assert.That(resetPresentation.cameraZ, Is.EqualTo(reset.z + 128));

            var expectedPreUpdate = reset;
            var snapshot = reset;
            for (var tick = 0; tick < 8; tick++)
            {
                expectedPreUpdate = snapshot;
                snapshot = session.Step(0, 0, 1);
            }

            Assert.That(snapshot.stateHash, Is.EqualTo(EightForwardStateHash));
            Assert.That(snapshot.tick, Is.EqualTo(8));
            Assert.That(snapshot.x, Is.EqualTo(19840));
            Assert.That(snapshot.y, Is.EqualTo(56480));
            Assert.That(snapshot.z, Is.EqualTo(5040));
            Assert.That(snapshot.phase, Is.EqualTo(161));

            var presentation = session.PresentationState();
            var renderCamera = session.RenderCameraState();
            Assert.That(presentation.tick, Is.EqualTo(expectedPreUpdate.tick));
            Assert.That(presentation.cameraX, Is.EqualTo(expectedPreUpdate.x));
            Assert.That(presentation.cameraY, Is.EqualTo(expectedPreUpdate.y));
            Assert.That(
                presentation.cameraZ,
                Is.EqualTo(expectedPreUpdate.z + 128));
            Assert.That(renderCamera.tick, Is.EqualTo(presentation.tick));
            Assert.That(renderCamera.historyX, Is.EqualTo(presentation.cameraX));
            Assert.That(renderCamera.historyY, Is.EqualTo(presentation.cameraY));
        }

        [Test]
        public void MissingOrTamperedOwnerInputsFailClosed()
        {
            var missing = Path.Combine(
                Path.GetTempPath(),
                "magiccarpet-native-tests",
                Guid.NewGuid().ToString("N"));
            Assert.Throws<InvalidOperationException>(
                () => NativeSession.Open(missing, ClosurePath));
            Assert.Throws<InvalidOperationException>(
                () => NativeSession.Open(PackagePath, missing + ".json"));

            var tamperedClosure = Path.Combine(
                Path.GetTempPath(),
                $"magiccarpet-closure-tamper-{Guid.NewGuid():N}.json");
            File.WriteAllText(tamperedClosure, "{}");
            try
            {
                Assert.Throws<InvalidOperationException>(
                    () => NativeSession.Open(PackagePath, tamperedClosure));
            }
            finally
            {
                File.Delete(tamperedClosure);
            }
        }

        [Test]
        public void BulkTerrainCheckpointReusesBuffersWithinAnAllocationBudget()
        {
            using var session = NativeSession.Open(PackagePath, ClosurePath);
            var camera = session.RenderCameraState();
            var indices = new byte[NativeSim.TerrainFramePixels];
            var coverage = new byte[NativeSim.TerrainFramePixels];

            var terrainGrid = session.TerrainGrid();
            session.TerrainFrame(camera, indices, coverage);
            Assert.That(terrainGrid.Length, Is.EqualTo(840));
            Assert.That(session.TerrainGrid(), Is.SameAs(terrainGrid));

            for (var warmup = 0; warmup < 4; warmup++)
            {
                session.TerrainGrid();
                session.TerrainFrame(camera, indices, coverage);
            }

            const int Iterations = 32;
            var stopwatch = Stopwatch.StartNew();
            var allocationBefore = GC.GetAllocatedBytesForCurrentThread();
            var reused = true;
            for (var iteration = 0; iteration < Iterations; iteration++)
            {
                reused &= ReferenceEquals(session.TerrainGrid(), terrainGrid);
                session.TerrainFrame(camera, indices, coverage);
            }
            var allocated = GC.GetAllocatedBytesForCurrentThread() -
                            allocationBefore;
            stopwatch.Stop();

            TestContext.Progress.WriteLine(
                "MC_BULK_ABI iterations={0} allocated_bytes={1} elapsed_ms={2}",
                Iterations,
                allocated,
                stopwatch.ElapsedMilliseconds);
            UnityEngine.Debug.Log(
                $"MC_BULK_ABI PASS iterations={Iterations} " +
                $"allocatedBytes={allocated} " +
                $"elapsedMs={stopwatch.ElapsedMilliseconds} " +
                "gridVertices=840 framePixels=64000 buffersReused=true");
            Assert.That(reused, Is.True);
            Assert.That(
                allocated,
                Is.LessThanOrEqualTo(64 * 1024),
                "preallocated terrain-grid and terrain-frame checkpoints must " +
                "not recreate their large managed buffers");
        }
    }
}
