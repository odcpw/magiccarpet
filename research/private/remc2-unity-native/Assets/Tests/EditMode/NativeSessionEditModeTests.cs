using System;
using System.Diagnostics;
using System.IO;
using System.Linq;
using System.Reflection;
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
            Assert.That(
                MagicCarpetRuntime.EventUsesAudioSidecar(
                    NativeSim.EventLevel1FireballImpacted),
                Is.True);
            Assert.That(
                OwnerAudioPresentation.EventPreparesSound(
                    NativeSim.EventLevel1FireballImpacted),
                Is.True);
            Assert.That(
                OwnerAudioPresentation.EventPreparesSound(
                    NativeSim.EventLevel1FireflyFireballSpawned),
                Is.False);
            Assert.That(
                OwnerAudioPresentation.EventSoundId(
                    NativeSim.EventLevel1FireballImpacted, 12),
                Is.EqualTo(3));
            Assert.That(
                OwnerAudioPresentation.EventSoundId(
                    NativeSim.EventPrepareSound, 29),
                Is.EqualTo(29));
            Assert.That(
                MagicCarpetRuntime.SpellTierMenuStartX(0, 13, 8, 48, 3, 66),
                Is.EqualTo(0));
            Assert.That(
                MagicCarpetRuntime.SpellTierMenuStartX(2, 13, 8, 48, 3, 66),
                Is.EqualTo(29));
            Assert.That(
                MagicCarpetRuntime.SpellTierMenuStartX(12, 13, 8, 48, 3, 66),
                Is.EqualTo(442));
            Assert.That(
                MagicCarpetRuntime.SpellTierFromLogicalX(28, 29, 66, 2),
                Is.EqualTo(0));
            Assert.That(
                MagicCarpetRuntime.SpellTierFromLogicalX(110, 29, 66, 2),
                Is.EqualTo(1));
            Assert.That(
                MagicCarpetRuntime.SpellTierFromLogicalX(400, 29, 66, 2),
                Is.EqualTo(2));
        }

        [Test]
        public void ExitPresentationUsesHistoryFovAndIntegerRgb6PaletteMath()
        {
            Assert.That(
                Level1ExitPresentationMath.UnityVerticalFov(128),
                Is.EqualTo(72.38641f).Within(0.0002f));
            Assert.That(
                Level1ExitPresentationMath.UnityVerticalFov(127),
                Is.EqualTo(72.72073f).Within(0.0002f));
            Assert.That(
                Level1ExitPresentationMath.UnityVerticalFov(121),
                Is.EqualTo(75.49361f).Within(0.0002f));
            Assert.Throws<ArgumentOutOfRangeException>(
                () => Level1ExitPresentationMath.UnityVerticalFov(0));

            var original = new byte[256 * 3];
            original[0] = 252;
            original[1] = 0;
            original[2] = 0;
            original[3] = 4;
            original[4] = 8;
            original[5] = 12;
            var effective = new byte[original.Length];
            Level1ExitPresentationMath.MaterializePalette(
                original,
                NativeSim.Level1ExitPaletteEntryFlash,
                4,
                4,
                effective);
            Assert.That(
                new ArraySegment<byte>(effective, 0, 6),
                Is.EqualTo(new byte[] { 0, 0, 0, 204, 136, 140 }));

            Level1ExitPresentationMath.MaterializePalette(
                original,
                NativeSim.Level1ExitPaletteBlack,
                14,
                28,
                effective);
            Assert.That(
                new ArraySegment<byte>(effective, 0, 6),
                Is.EqualTo(new byte[] { 0, 0, 0, 0, 4, 4 }));

            Assert.That(
                Level1ExitPresentationMath.SourceDisplayRect(1600, 900),
                Is.EqualTo(new Rect(0.125f, 0f, 0.75f, 1f)));
            Assert.That(
                Level1ExitPresentationMath.SourceDisplayRect(800, 600),
                Is.EqualTo(new Rect(0f, 0f, 1f, 1f)));
            Assert.That(
                Level1ExitPresentationMath.SourceDisplayRect(1280, 1024),
                Is.EqualTo(new Rect(0f, 0.03125f, 1f, 0.9375f)));
        }

        [Test]
        public void ObjectiveTextboxBlendsAllPaletteInputsThenOverwritesWithForeground()
        {
            const int width = 256;
            var world = Enumerable.Range(0, 256)
                .Select(value => (byte)value)
                .ToArray();
            var tables = new byte[0x4000 + 62 * 256];
            for (var value = 0; value < 256; value++)
                tables[0x4000 + 61 * 256 + value] = (byte)(255 - value);
            var foreground = new byte[width * 2];
            foreground[42] = 119;
            foreground[width + 42] = 1;
            var indices = new byte[width];
            var mask = new byte[width];

            ObjectiveTextboxCompositor.ComposeOverlayIndices(
                1,
                world,
                width,
                1,
                width,
                1,
                0,
                0,
                width,
                1,
                0,
                0,
                width,
                1,
                tables,
                0x4000,
                61,
                foreground,
                indices,
                mask);

            Assert.That(mask, Is.All.EqualTo(1));
            for (var value = 0; value < 256; value++)
            {
                Assert.That(
                    indices[value],
                    Is.EqualTo(value == 42 ? 119 : 255 - value),
                    $"TABLESN destination input {value}");
            }
        }

        [Test]
        public void ObjectiveTextboxUsesPixelCentreNearestSamplingAndClearsAtExpiry()
        {
            var world = new byte[] { 1, 2, 3, 4 };
            var tables = new byte[0x4000 + 62 * 256];
            for (var value = 0; value < 256; value++)
                tables[0x4000 + 61 * 256 + value] = (byte)value;
            var foreground = new byte[4 * 4 * 2];
            var indices = Enumerable.Repeat((byte)0xcc, 16).ToArray();
            var mask = Enumerable.Repeat((byte)0xcc, 16).ToArray();

            ObjectiveTextboxCompositor.ComposeOverlayIndices(
                1,
                world,
                2,
                2,
                4,
                4,
                0,
                0,
                4,
                4,
                0,
                0,
                4,
                4,
                tables,
                0x4000,
                61,
                foreground,
                indices,
                mask);
            Assert.That(
                indices,
                Is.EqualTo(new byte[]
                {
                    1, 1, 2, 2,
                    1, 1, 2, 2,
                    3, 3, 4, 4,
                    3, 3, 4, 4,
                }));
            Assert.That(mask, Is.All.EqualTo(1));

            ObjectiveTextboxCompositor.ComposeOverlayIndices(
                0,
                world,
                2,
                2,
                4,
                4,
                0,
                0,
                4,
                4,
                0,
                0,
                4,
                4,
                tables,
                0x4000,
                61,
                foreground,
                indices,
                mask);
            Assert.That(indices, Is.All.EqualTo(0));
            Assert.That(mask, Is.All.EqualTo(0));
        }

        [Test]
        public void NativePaletteIsAtomicAndMatchesQualifiedPalnAtReset()
        {
            using var session = NativeSession.Open(PackagePath, ClosurePath);
            var mission = session.Level1Mission();
            var rgb8 = new byte[768];
            var info = session.Level1Palette(mission, rgb8);
            Assert.That(info.generation, Is.EqualTo(mission.generation));
            Assert.That(info.tick, Is.EqualTo(mission.tick));
            Assert.That(info.submode, Is.Zero);
            Assert.That(info.effect, Is.Zero);
            Assert.That(info.weight, Is.Zero);
            Assert.That(info.denominator, Is.Zero);
            Assert.That(
                rgb8,
                Is.EqualTo(File.ReadAllBytes(
                    Path.Combine(AssetCachePath, "palette/paln.rgb8"))));
            var commands = session.Level1MinimapEntities(mission, out var minimap);
            Assert.That(minimap.generation, Is.EqualTo(mission.generation));
            Assert.That(minimap.tick, Is.EqualTo(mission.tick));
            Assert.That(minimap.crossX, Is.EqualTo((ushort)63));
            Assert.That(minimap.crossY, Is.EqualTo((ushort)64));
            Assert.That(minimap.crossArmLength, Is.EqualTo(10));
            Assert.That(commands, Is.Not.Empty);
            Assert.That(
                commands.Count(command =>
                    command.kind == NativeSim.Level1MinimapCommandDot &&
                    command.paletteIndex == 23),
                Is.GreaterThanOrEqualTo(28),
                "all reset class-5/model-12..14 civilians cross the ordered native stream");
            Assert.That(
                commands.Select(command => command.order),
                Is.EqualTo(Enumerable.Range(0, commands.Length).Select(value => (ushort)value)));
        }

        [Test]
        public void ExitPresentationResetStateIsGenerationAndTickBound()
        {
            using var session = NativeSession.Open(PackagePath, ClosurePath);
            var mission = session.Level1Mission();
            var exit = session.Level1ExitPresentation(mission);
            Assert.That(exit.generation, Is.EqualTo(mission.generation));
            Assert.That(exit.tick, Is.EqualTo(mission.tick));
            Assert.That(exit.playerAction, Is.Zero);
            Assert.That(exit.actionPhase, Is.Zero);
            Assert.That(exit.paletteEffect, Is.Zero);
            Assert.That(exit.cameraFov, Is.EqualTo(128));
            Assert.That(exit.inputSuppressed, Is.Zero);
            Assert.That(exit.resultCode, Is.Zero);
            Assert.That(exit.active, Is.Zero);
            Assert.That(exit.complete, Is.Zero);
            Assert.That(
                exit.flags,
                Is.EqualTo(
                    NativeSim.Level1ExitFlagPaletteVisualCurrent |
                    NativeSim.Level1ExitFlagFovPostActionNextHistory));
        }

        [Test]
        public void OwnerCacheUsesItsSemanticClosureIdentity()
        {
            var cache = OwnerAssetCache.OpenAndValidate(
                AssetCachePath,
                MagicCarpetRuntime.ExpectedPackageHash,
                MagicCarpetRuntime.ExpectedOwnerAssetClosureHash);
            Assert.That(cache.OutputCount, Is.EqualTo(570));
            Assert.That(
                cache.ContentHash,
                Is.EqualTo(
                    "d299990e300117d065eb3a75008f6a69911b1ba356e4e06452c0a826450a01e5"));
        }

        [Test]
        public void SpellAssignmentOpcodesDriveTheSourceHandAndTierSnapshot()
        {
            using var session = NativeSession.Open(PackagePath, ClosurePath);
            var mission = session.Level1Mission();
            var reset = session.Level1SpellMenu(mission);
            Assert.That(reset.familyEnabled, Is.EqualTo(new byte[] { 1, 1, 0, 0 }));
            Assert.That(
                reset.familyUnlockedTier,
                Is.EqualTo(new sbyte[] { 0, 0, 0, 0 }));
            Assert.That(
                new[]
                {
                    (int)reset.leftFamily,
                    reset.leftTier,
                    (int)reset.rightFamily,
                    reset.rightTier,
                },
                Is.EqualTo(new[] { 0, 0, 1, 0 }));

            session.Step(0, 0, 0, opcode: 31, argument1: 1, argument2: 2);
            mission = session.Level1Mission();
            var leftChanged = session.Level1SpellMenu(mission);
            Assert.That(leftChanged.leftFamily, Is.EqualTo(1));
            Assert.That(leftChanged.leftTier, Is.EqualTo(0));
            Assert.That(leftChanged.selectedUiIndex, Is.EqualTo(1));
            Assert.That(leftChanged.selectedTier, Is.EqualTo(0));

            session.Step(0, 0, 0, opcode: 32, argument1: 0, argument2: 2);
            mission = session.Level1Mission();
            var rightChanged = session.Level1SpellMenu(mission);
            Assert.That(rightChanged.leftFamily, Is.EqualTo(1));
            Assert.That(rightChanged.rightFamily, Is.EqualTo(0));
            Assert.That(rightChanged.rightTier, Is.EqualTo(0));
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
        public void LiveTerrainAndCastleLifecycleAreTypedHashedResettableAndSessionLocal()
        {
            using var first = NativeSession.Open(PackagePath, ClosurePath);
            using var second = NativeSession.Open(PackagePath, ClosurePath);
            var firstSnapshot = first.Snapshot();
            var firstTerrain = first.TerrainState();
            var secondTerrain = second.TerrainState();
            var mission = first.Level1Mission();
            var lifecycle = first.Level1CastleLifecycle(mission);

            Assert.That(firstTerrain.generation, Is.EqualTo(firstSnapshot.generation));
            Assert.That(firstTerrain.tick, Is.Zero);
            Assert.That(firstTerrain.mutationSequence, Is.Zero);
            Assert.That(firstTerrain.terrainHash, Is.EqualTo(secondTerrain.terrainHash));
            Assert.That(
                firstTerrain.terrainHash,
                Is.EqualTo(MagicCarpetRuntime.TerrainHash(
                    first.BytePlane(NativeSim.TerrainTypes),
                    first.BytePlane(NativeSim.TerrainHeight),
                    first.BytePlane(NativeSim.TerrainShade),
                    first.BytePlane(NativeSim.TerrainAngle),
                    first.BytePlane(NativeSim.TerrainSecondary),
                    first.EntityIndexPlane())));
            Assert.That(lifecycle.generation, Is.EqualTo(firstSnapshot.generation));
            Assert.That(lifecycle.tick, Is.Zero);
            Assert.That(lifecycle.rootActive, Is.Zero);
            Assert.That(lifecycle.constructionPresent, Is.Zero);
            Assert.That(lifecycle.terrainMutationSequence, Is.Zero);

            first.Step(0, 0, 1);
            var advancedTerrain = first.TerrainState();
            Assert.That(advancedTerrain.tick, Is.EqualTo(1));
            Assert.That(advancedTerrain.mutationSequence, Is.Zero);
            Assert.That(advancedTerrain.terrainHash, Is.EqualTo(firstTerrain.terrainHash));
            Assert.That(second.TerrainState().tick, Is.Zero);

            first.Reset();
            var resetTerrain = first.TerrainState();
            Assert.That(resetTerrain.generation, Is.Not.EqualTo(firstTerrain.generation));
            Assert.That(resetTerrain.tick, Is.Zero);
            Assert.That(resetTerrain.mutationSequence, Is.Zero);
            Assert.That(resetTerrain.terrainHash, Is.EqualTo(firstTerrain.terrainHash));
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
        public void LevelOneMissionActorsAndFireballAreLiveNativeState()
        {
            using var session = NativeSession.Open(PackagePath, ClosurePath);
            var mission = session.Level1Mission();
            Assert.That(mission.currentStage, Is.Zero);
            Assert.That(mission.textSelector, Is.EqualTo(48));
            Assert.That(mission.playerLife, Is.EqualTo(10_000));
            Assert.That(mission.playerMana, Is.EqualTo(1_000));
            Assert.That(mission.hasFireball, Is.EqualTo(1));
            Assert.That(mission.hasPossession, Is.EqualTo(1));
            Assert.That(mission.projectileCount, Is.Zero);
            Assert.That(mission.impactCount, Is.Zero);

            var actors = session.Level1Actors(mission);
            Assert.That(actors, Has.Length.EqualTo(45));
            Assert.That(
                Array.FindAll(actors, actor => actor.kind == 1),
                Has.Length.EqualTo(4));
            Assert.That(
                Array.FindAll(actors, actor => actor.kind == 2),
                Has.Length.EqualTo(6));
            Assert.That(
                Array.FindAll(actors, actor => actor.kind == 3),
                Has.Length.EqualTo(1));
            Assert.That(
                Array.FindAll(actors, actor => actor.kind == 4),
                Has.Length.EqualTo(1));
            Assert.That(
                Array.FindAll(actors, actor => actor.kind == 5),
                Has.Length.EqualTo(1));
            Assert.That(
                Array.FindAll(actors, actor => actor.kind == 6),
                Has.Length.EqualTo(1));
            Assert.That(
                Array.FindAll(actors, actor => actor.kind == 7),
                Has.Length.EqualTo(4));
            Assert.That(
                Array.FindAll(actors, actor => actor.kind == 8),
                Has.Length.EqualTo(5));
            Assert.That(
                Array.FindAll(actors, actor => actor.kind == 9),
                Has.Length.EqualTo(18));
            Assert.That(
                Array.FindAll(actors, actor => actor.kind == 10),
                Has.Length.EqualTo(1));
            Assert.That(
                Array.FindAll(actors, actor => actor.kind == 11),
                Has.Length.EqualTo(3));
            Assert.That(
                Array.FindAll(actors, actor => actor.active != 0),
                Is.Empty);
            Assert.That(
                Array.Find(actors, actor => actor.kind == 1).baseTmap,
                Is.EqualTo(8));
            Assert.That(
                Array.Find(actors, actor => actor.kind == 2).baseTmap,
                Is.EqualTo(279));
            Assert.That(
                Array.Find(actors, actor => actor.kind == 3).baseTmap,
                Is.EqualTo(59));
            Assert.That(
                Array.Find(actors, actor => actor.kind == 4).renderParameterIndex,
                Is.EqualTo(77));
            Assert.That(
                Array.Find(actors, actor => actor.kind == 5).baseTmap,
                Is.EqualTo(461));
            Assert.That(
                Array.Find(actors, actor => actor.kind == 5).visible,
                Is.Zero);
            Assert.That(
                Array.Find(actors, actor => actor.kind == 6).visible,
                Is.Zero);
            Assert.That(
                Array.Find(actors, actor => actor.kind == 7).baseTmap,
                Is.EqualTo(30));
            Assert.That(
                Array.Find(actors, actor => actor.kind == 8).baseTmap,
                Is.EqualTo(259));
            Assert.That(
                Array.Find(actors, actor => actor.kind == 9).baseTmap,
                Is.EqualTo(75));
            Assert.That(
                Array.Find(actors, actor => actor.kind == 10).baseTmap,
                Is.EqualTo(78));
            Assert.That(
                Array.TrueForAll(
                    actors,
                    actor => actor.yaw == 0 && actor.animationFrame == 0),
                Is.True);

            var fired = session.Step(0, 0, 0x10);
            mission = session.Level1Mission();
            Assert.That(mission.tick, Is.EqualTo(fired.tick));
            Assert.That(
                mission.projectileCount + mission.impactCount,
                Is.GreaterThan(0));
            if (mission.projectileCount != 0)
            {
                var projectiles = session.FireballProjectiles(mission);
                Assert.That(projectiles, Has.Length.EqualTo(mission.projectileCount));
                Assert.That(projectiles[0].renderIndex, Is.EqualTo(340));
                Assert.That(projectiles[0].baseTmap, Is.EqualTo(463));
                Assert.That(projectiles[0].renderMode, Is.EqualTo(21));
                Assert.That(projectiles[0].payload, Is.EqualTo(250));
            }
            if (mission.impactCount != 0)
            {
                var impacts = session.FireballImpacts(mission);
                Assert.That(impacts, Has.Length.EqualTo(mission.impactCount));
                Assert.That(impacts[0].renderParameterIndex, Is.EqualTo(7));
                Assert.That(impacts[0].baseTmap, Is.EqualTo(38));
                Assert.That(impacts[0].renderMode, Is.EqualTo(36));
                Assert.That(impacts[0].payload, Is.EqualTo(250));
            }

            var reset = session.Reset();
            mission = session.Level1Mission();
            Assert.That(mission.generation, Is.EqualTo(reset.generation));
            Assert.That(mission.tick, Is.Zero);
            Assert.That(mission.projectileCount, Is.Zero);
            Assert.That(mission.impactCount, Is.Zero);
        }

        [Test]
        public void SelectorFramesAdvanceOnlyFromNativeCpuAdmissionOrQualificationFeedback()
        {
            using var session = NativeSession.Open(PackagePath, ClosurePath);
            var reset = session.Snapshot();
            var frames = session.SelectorFrames(
                reset.generation,
                reset.tick,
                out var resetBatch);
            Assert.That(frames, Has.Length.EqualTo(3));
            Assert.That(
                new[] { frames[0].selector, frames[1].selector, frames[2].selector },
                Is.EqualTo(new ushort[] { 152, 153, 154 }));
            Assert.That(
                new[]
                {
                    frames[0].sourceFrameIndex,
                    frames[1].sourceFrameIndex,
                    frames[2].sourceFrameIndex
                },
                Is.EqualTo(new ushort[] { 1, 1, 1 }));
            Assert.That(
                new[] { frames[0].streamCursor, frames[1].streamCursor, frames[2].streamCursor },
                Is.EqualTo(new uint[] { 490, 710, 490 }));
            Assert.That(resetBatch.hasDrawFeedback, Is.Zero);

            var ordinaryStep = session.Step(0, 0, 0);
            frames = session.SelectorFrames(
                ordinaryStep.generation,
                ordinaryStep.tick,
                out var ordinaryBatch);
            Assert.That(
                new[]
                {
                    frames[0].sourceFrameIndex,
                    frames[1].sourceFrameIndex,
                    frames[2].sourceFrameIndex
                },
                Is.EqualTo(new ushort[] { 1, 1, 1 }),
                "ordinary Unity stepping must not invent CPU draw admission");
            Assert.That(
                ordinaryBatch.selectorStateHash,
                Is.EqualTo(resetBatch.selectorStateHash));

            var cpuAdmission = session.EvaluateGoatDrawAdmission(
                ordinaryStep.generation,
                ordinaryStep.tick);
            Assert.That(cpuAdmission.hasDrawFeedback, Is.EqualTo(1));
            Assert.That(cpuAdmission.lastDrawTick, Is.EqualTo(ordinaryStep.tick));
            Assert.That(cpuAdmission.lastDrawMask & ~0x07UL, Is.Zero);
            frames = session.SelectorFrames(
                ordinaryStep.generation,
                ordinaryStep.tick,
                out var cpuLatchedBatch);
            for (var ordinal = 0; ordinal < frames.Length; ordinal++)
            {
                var expectedUsed = (cpuAdmission.lastDrawMask & (1UL << ordinal)) != 0;
                Assert.That(
                    (frames[ordinal].flags &
                     NativeSim.SelectorFrameUsedByPrecedingDraw) != 0,
                    Is.EqualTo(expectedUsed));
            }
            Assert.That(
                cpuLatchedBatch.selectorStateHash,
                Is.EqualTo(cpuAdmission.selectorStateHash));

            session.SubmitSelectorDrawFeedbackForQualification(
                ordinaryStep.generation,
                ordinaryStep.tick,
                0x01);
            frames = session.SelectorFrames(
                ordinaryStep.generation,
                ordinaryStep.tick,
                out var latchedBatch);
            Assert.That(
                frames[0].flags & NativeSim.SelectorFrameUsedByPrecedingDraw,
                Is.Not.Zero);
            Assert.That(latchedBatch.lastDrawMask & 0x01UL, Is.EqualTo(0x01));

            var consumed = session.Step(0, 0, 0);
            frames = session.SelectorFrames(
                consumed.generation,
                consumed.tick,
                out _);
            Assert.That(frames[0].sourceFrameIndex, Is.EqualTo(2));
            Assert.That(frames[0].streamCursor, Is.EqualTo(818));
            Assert.That(
                frames[1].sourceFrameIndex,
                Is.EqualTo((latchedBatch.lastDrawMask & 0x02UL) != 0 ? 2 : 1));
            Assert.That(
                frames[2].sourceFrameIndex,
                Is.EqualTo((latchedBatch.lastDrawMask & 0x04UL) != 0 ? 2 : 1));
            Assert.That(
                frames[0].flags & NativeSim.SelectorFrameUsedByPrecedingDraw,
                Is.Zero);
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
        public void LevelOneAutopilotObservationExcludesHiddenEnemyState()
        {
            static string[] FieldNames(Type type)
            {
                var fields = type.GetFields(
                    BindingFlags.Instance |
                    BindingFlags.Public |
                    BindingFlags.NonPublic);
                var names = Array.ConvertAll(fields, field => field.Name);
                Array.Sort(names, StringComparer.Ordinal);
                return names;
            }

            Assert.That(
                FieldNames(typeof(Level1Autopilot.ActorObservation)),
                Is.EqualTo(new[]
                {
                    "active",
                    "authoredIndex",
                    "collected",
                    "kind",
                    "x",
                    "y",
                    "z",
                }));
            Assert.That(
                FieldNames(typeof(Level1Autopilot.GoatObservation)),
                Is.EqualTo(new[] { "active", "x", "y", "z" }));
            Assert.That(
                FieldNames(typeof(Level1Autopilot.SphereObservation)),
                Is.EqualTo(new[]
                {
                    "active",
                    "baseTmap",
                    "renderParameterIndex",
                    "sphereId",
                    "x",
                    "y",
                    "z",
                }));

            var next = typeof(Level1Autopilot).GetMethod(
                "Next",
                BindingFlags.Instance | BindingFlags.NonPublic);
            Assert.That(next, Is.Not.Null);
            var parameters = next.GetParameters();
            Assert.That(parameters, Has.Length.EqualTo(1));
            Assert.That(
                parameters[0].ParameterType,
                Is.EqualTo(typeof(Level1Autopilot.Observation)));
        }

        [Test]
        public void LevelOnePostCompletionPilotKeepsAVisibleMultipartTargetLock()
        {
            static NativeSim.Level1ActorSnapshot Actor(
                ushort authoredIndex,
                byte kind,
                byte active,
                ushort x)
            {
                return new NativeSim.Level1ActorSnapshot
                {
                    authoredIndex = authoredIndex,
                    kind = kind,
                    active = active,
                    x = x,
                    y = 10_000,
                    z = 1_000,
                };
            }

            static Level1Autopilot.Observation Observation(
                ulong tick,
                params NativeSim.Level1ActorSnapshot[] actors)
            {
                var player = new NativeSim.SessionSnapshot
                {
                    tick = tick,
                    x = 10_000,
                    y = 10_000,
                    z = 1_000,
                    selectedSpellUiIndex = 0,
                };
                var mission = new NativeSim.Level1MissionSnapshot
                {
                    currentStage = 5,
                    playerLife = 10_000,
                };
                return new Level1Autopilot.Observation(
                    new Level1Autopilot.PlayerObservation(player),
                    new Level1Autopilot.MissionObservation(mission),
                    Array.ConvertAll(
                        actors,
                        actor => new Level1Autopilot.ActorObservation(actor)),
                    Array.Empty<Level1Autopilot.GoatObservation>(),
                    Array.Empty<Level1Autopilot.SphereObservation>(),
                    new Level1Autopilot.CastleObservation(default));
            }

            var driver = new Level1Autopilot(includePostCompletion: true);
            driver.Next(Observation(1, Actor(306, 2, 1, 11_000)));
            driver.Next(Observation(2, Actor(307, 7, 1, 11_000)));

            driver.Next(
                Observation(
                    3,
                    Actor(309, 7, 1, 11_000),
                    Actor(310, 7, 1, 13_000)));
            Assert.That(driver.LastAimActor, Is.EqualTo(309));

            driver.Next(
                Observation(
                    4,
                    Actor(309, 7, 1, 14_000),
                    Actor(310, 7, 1, 10_500)));
            Assert.That(
                driver.LastAimActor,
                Is.EqualTo(309),
                "the pilot must not retarget merely because another visible head moved closer");

            driver.Next(
                Observation(
                    5,
                    Actor(309, 7, 0, 14_000),
                    Actor(310, 7, 1, 10_500)));
            Assert.That(
                driver.LastAimActor,
                Is.EqualTo(310),
                "the lock releases when the selected head is visibly inactive");
        }

        [Test]
        [Timeout(600_000)]
        public void LevelOneAutopilotFreshSessionsHaveIdenticalDigests()
        {
            var first = RunAutopilotDigest();
            var second = RunAutopilotDigest();

            Assert.That(
                first.Tick,
                Is.LessThanOrEqualTo(40_000),
                "ordinary-input completion remains inside the Level 1 regression budget");
            Assert.That(first.Stage, Is.EqualTo(5));
            Assert.That(first.Result, Is.EqualTo(10));
            Assert.That(
                first.SawGroup33Firefly,
                Is.False,
                "the ordinary exit route never overlaps the row-305 switch");
            Assert.That(
                first.SawGroup34Multipart,
                Is.False,
                "group 34 remains gated when row 305 is not overlapped");
            Assert.That(first.CommandHash, Is.EqualTo(second.CommandHash));
            Assert.That(first.EventHash, Is.EqualTo(second.EventHash));
            Assert.That(first.StateHash, Is.EqualTo(second.StateHash));
            Assert.That(first.Tick, Is.EqualTo(second.Tick));
            Assert.That(first.EventCount, Is.EqualTo(second.EventCount));
            UnityEngine.Debug.Log(
                $"MC_LEVEL1_DETERMINISM PASS tick={first.Tick} " +
                $"commands=0x{first.CommandHash:x16} " +
                $"events={first.EventCount}/0x{first.EventHash:x16} " +
                $"states=0x{first.StateHash:x16}");
        }

        [Test]
        [Timeout(1_200_000)]
        public void LevelOnePostCompletionAutopilotFightsGroups33And34BeforeExit()
        {
            var first = RunAutopilotDigest(
                includePostCompletion: true,
                maximumTicks: 240_000);
            var second = RunAutopilotDigest(
                includePostCompletion: true,
                maximumTicks: 240_000);

            Assert.That(first.SawGroup33Firefly, Is.True);
            Assert.That(first.SawFirstMultipart, Is.True);
            Assert.That(first.SawGroup34Multipart, Is.True);
            Assert.That(first.Tick, Is.LessThanOrEqualTo(240_000));
            Assert.That(first.Stage, Is.EqualTo(5));
            Assert.That(first.Result, Is.EqualTo(10));
            Assert.That(first.CommandHash, Is.EqualTo(second.CommandHash));
            Assert.That(first.EventHash, Is.EqualTo(second.EventHash));
            Assert.That(first.StateHash, Is.EqualTo(second.StateHash));
            Assert.That(first.SourceArenaHash, Is.EqualTo(second.SourceArenaHash));
            Assert.That(first.MultipartHash, Is.EqualTo(second.MultipartHash));
            Assert.That(first.ProjectileHash, Is.EqualTo(second.ProjectileHash));
            Assert.That(first.ManaHash, Is.EqualTo(second.ManaHash));
            Assert.That(first.AudioHash, Is.EqualTo(second.AudioHash));
            Assert.That(first.TerminalHash, Is.EqualTo(second.TerminalHash));
            Assert.That(first.Tick, Is.EqualTo(second.Tick));
            Assert.That(first.EventCount, Is.EqualTo(second.EventCount));

            var incompleteMultipart = RunPostCompletionCombatNegativeControl(
                "fight-first-multipart",
                1_024);
            Assert.That(incompleteMultipart.ReachedSuppressedPhase, Is.True);
            Assert.That(incompleteMultipart.SuppressedCommands, Is.GreaterThan(0));
            Assert.That(incompleteMultipart.FirstMultipartActive, Is.True);
            Assert.That(incompleteMultipart.Group34Active, Is.False);
            Assert.That(incompleteMultipart.ExitComplete, Is.Zero);
            Assert.That(incompleteMultipart.PlayerDead, Is.Zero);

            var insufficientGroup34Combat =
                RunPostCompletionCombatNegativeControl(
                    "fight-group-34-multipart",
                    1_024);
            Assert.That(
                insufficientGroup34Combat.ReachedSuppressedPhase,
                Is.True);
            Assert.That(
                insufficientGroup34Combat.SuppressedCommands,
                Is.GreaterThan(0));
            Assert.That(insufficientGroup34Combat.Group34Active, Is.True);
            Assert.That(insufficientGroup34Combat.ExitComplete, Is.Zero);
            Assert.That(insufficientGroup34Combat.PlayerDead, Is.Zero);
            UnityEngine.Debug.Log(
                $"MC_LEVEL1_POST_COMPLETION_DETERMINISM PASS tick={first.Tick} " +
                $"groups=33/first-multipart/34 " +
                $"commands=0x{first.CommandHash:x16} " +
                $"events={first.EventCount}/0x{first.EventHash:x16} " +
                $"states=0x{first.StateHash:x16} " +
                $"arena=0x{first.SourceArenaHash:x16} " +
                $"multipart=0x{first.MultipartHash:x16} " +
                $"projectiles=0x{first.ProjectileHash:x16} " +
                $"mana=0x{first.ManaHash:x16} " +
                $"audio=0x{first.AudioHash:x16} " +
                $"terminal=0x{first.TerminalHash:x16} " +
                $"negativeControls=no-overlap/incomplete-multipart/" +
                $"insufficient-group34");
        }

        [Test]
        [Timeout(300_000)]
        public void LevelOneAutopilotNegativeControlsStopAtTheirObjectives()
        {
            var noFireball = RunAutopilotNegativeControl(
                maximumTicks: 12_000,
                suppressFireball: true,
                suppressPossession: false);
            Assert.That(noFireball.Stage, Is.EqualTo(2));
            Assert.That(noFireball.ExitComplete, Is.Zero);
            Assert.That(noFireball.SuppressedCommands, Is.GreaterThan(0));

            var noPossession = RunAutopilotNegativeControl(
                maximumTicks: 30_000,
                suppressFireball: false,
                suppressPossession: true);
            Assert.That(noPossession.Stage, Is.EqualTo(3));
            Assert.That(noPossession.CastleExists, Is.EqualTo(1));
            Assert.That(noPossession.SecuredManaPercent, Is.LessThan(15));
            Assert.That(noPossession.ExitComplete, Is.Zero);
            Assert.That(noPossession.SuppressedCommands, Is.GreaterThan(0));
        }

        [Test]
        [Timeout(360_000)]
        public void LevelOneNaturalFireflyRetaliationUsesOnlyOrdinaryPlayerInputs()
        {
            var first = RunNaturalFireflyRetaliation();
            var second = RunNaturalFireflyRetaliation();
            UnityEngine.Debug.Log(
                $"MC_LEVEL1_FIREFLY_RETALIATION_DIAGNOSTIC " +
                $"approach={first} repeat={second}");

            Assert.That(first.StageFourFireCommands, Is.Zero);
            Assert.That(first.FireflyHitCount, Is.Zero);
            Assert.That(first.HostileSpawnCount, Is.GreaterThanOrEqualTo(1));
            Assert.That(first.HostileImpactCount, Is.GreaterThanOrEqualTo(1));
            Assert.That(first.VisibleHostileImpactCount, Is.GreaterThanOrEqualTo(1));
            Assert.That(first.HostilePlayerDamageCount, Is.GreaterThanOrEqualTo(1));
            Assert.That(first.MinimumPlayerLife, Is.LessThanOrEqualTo(9_500));
            Assert.That(first.CommandHash, Is.EqualTo(second.CommandHash));
            Assert.That(first.EventHash, Is.EqualTo(second.EventHash));
            Assert.That(first.StateHash, Is.EqualTo(second.StateHash));
            Assert.That(first.Tick, Is.EqualTo(second.Tick));

            UnityEngine.Debug.Log(
                $"MC_LEVEL1_FIREFLY_RETALIATION PASS tick={first.Tick} " +
                $"fire={first.StageFourFireCommands} hits={first.FireflyHitCount} " +
                $"hostileSpawns={first.HostileSpawnCount} " +
                $"hostileImpacts={first.HostileImpactCount}/" +
                $"{first.VisibleHostileImpactCount} " +
                $"hostileDamage={first.HostilePlayerDamageCount} " +
                $"minLife={first.MinimumPlayerLife} " +
                $"commands=0x{first.CommandHash:x16} " +
                $"events=0x{first.EventHash:x16} states=0x{first.StateHash:x16}");
        }

        [Test]
        [Timeout(180_000)]
        public void LevelOneAutopilotCompletesThroughOrdinaryPlayerInputs()
        {
            using var session = NativeSession.Open(PackagePath, ClosurePath);
            var driver = new Level1Autopilot();
            var snapshot = session.Snapshot();
            var mission = session.Level1Mission();
            var actors = session.Level1Actors(mission);
            var goats = session.Goats(
                snapshot.generation,
                snapshot.tick,
                out _);
            var spheres = Array.Empty<NativeSim.ManaSphereSnapshot>();
            var castle = session.Level1Castle(mission);
            var terrain = session.TerrainState();
            var initialTerrainHash = terrain.terrainHash;
            var peakTerrainMutationSequence = terrain.mutationSequence;
            var priorPhase = string.Empty;
            var fireRequests = 0;
            uint peakProjectiles = 0;
            uint peakImpacts = 0;
            var peakPlayerFireballs = 0;
            var bestFireflyPass = double.MaxValue;
            var bestFireflyPassTick = 0UL;
            var bestFireflyPassAxes = string.Empty;
            var bestPossessionPass = double.MaxValue;
            var bestPossessionPassTick = 0UL;
            var bestPossessionPassAxes = string.Empty;
            var observedCarrier = false;

            var maximumTicks = int.TryParse(
                Environment.GetEnvironmentVariable("MC_LEVEL1_BOT_MAX_TICKS"),
                out var requestedMaximumTicks)
                ? Math.Max(1, requestedMaximumTicks)
                : 120_000;
            for (var tick = 0; tick < maximumTicks; tick++)
            {
                var command = driver.Next(
                    Level1Autopilot.Observe(
                        snapshot,
                        mission,
                        actors,
                        goats,
                        spheres,
                        castle));
                if ((unchecked((byte)command.Flags) & 0x40) != 0)
                {
                    fireRequests++;
                }
                peakProjectiles = Math.Max(peakProjectiles, mission.projectileCount);
                peakImpacts = Math.Max(peakImpacts, mission.impactCount);
                var liveProjectiles = session.FireballProjectiles(mission);
                var playerFireballs = Array.FindAll(
                    liveProjectiles,
                    projectile => projectile.projectileKind == 1).Length;
                peakPlayerFireballs = Math.Max(peakPlayerFireballs, playerFireballs);
                foreach (var projectile in liveProjectiles)
                {
                    if (projectile.projectileKind != 5) continue;
                    foreach (var sphere in spheres)
                    {
                        if (sphere.active == 0 || sphere.ownerEntityIndex != 0) continue;
                        var dx = Math.Abs(unchecked((short)(sphere.x - projectile.x)));
                        var dy = Math.Abs(unchecked((short)(sphere.y - projectile.y)));
                        var dz = Math.Abs(sphere.z - projectile.z);
                        var pass = Math.Sqrt(
                            (double)dx * dx +
                            (double)dy * dy +
                            (double)dz * dz);
                        if (pass >= bestPossessionPass) continue;
                        bestPossessionPass = pass;
                        bestPossessionPassTick = snapshot.tick;
                        bestPossessionPassAxes =
                            $"dx={dx} dy={dy} dz={dz} " +
                            $"projectile={projectile.projectileId} " +
                            $"sphere={sphere.sphereId}";
                    }
                }
                if (mission.currentStage == 4)
                {
                    foreach (var projectile in liveProjectiles)
                    {
                        if (projectile.projectileKind != 1) continue;
                        foreach (var actor in actors)
                        {
                            if (actor.active == 0 || actor.kind != 2) continue;
                            var dx = Math.Abs(unchecked((short)(actor.x - projectile.x)));
                            var dy = Math.Abs(unchecked((short)(actor.y - projectile.y)));
                            var dz = Math.Abs(
                                actor.z + 100 - (projectile.z + 50));
                            var pass = Math.Max(
                                dx / 85.0,
                                Math.Max(dy / 85.0, dz / 101.0));
                            if (pass >= bestFireflyPass) continue;
                            bestFireflyPass = pass;
                            bestFireflyPassTick = snapshot.tick;
                            bestFireflyPassAxes =
                                $"dx={dx} dy={dy} dz={dz} " +
                                $"projectile={projectile.projectileId} " +
                                $"actor={actor.authoredIndex}";
                        }
                    }
                }
                if (command.Phase != priorPhase)
                {
                    priorPhase = command.Phase;
                    UnityEngine.Debug.LogFormat(
                        "MC_LEVEL1_BOT tick={0} stage={1} phase={2} " +
                        "position={3},{4},{5} speed={6}/{7} life={8} mana={9}",
                        tick,
                        mission.currentStage,
                        command.Phase,
                        snapshot.x,
                        snapshot.y,
                        snapshot.z,
                        snapshot.actualSpeed,
                        snapshot.targetSpeed,
                        mission.playerLife,
                        mission.playerMana);
                }
                if (tick % 250 == 0)
                {
                    UnityEngine.Debug.LogFormat(
                        "MC_LEVEL1_BOT_PROGRESS tick={0} stage={1} phase={2} " +
                        "position={3},{4},{5} yaw={6} pitch={7} roll={8} " +
                        "speed={9}/{10} life={11} mana={12} activeActors={13} " +
                        "fireRequests={14} fireSequence={15} selected={16}/{17} " +
                        "projectiles={18} playerFireballs={19} impacts={20} " +
                        "spheres={21} castleMana={22} secured={23} " +
                        "aimActor={24} aimDistance={25:F0} aimError={26},{27} " +
                        "bestFireflyPass={28:F3}",
                        tick,
                        mission.currentStage,
                        command.Phase,
                        snapshot.x,
                        snapshot.y,
                        snapshot.z,
                        snapshot.yaw,
                        snapshot.effectivePitch,
                        snapshot.rollAccumulator,
                        snapshot.actualSpeed,
                        snapshot.targetSpeed,
                        mission.playerLife,
                        mission.playerMana,
                        Array.FindAll(actors, actor => actor.active != 0).Length,
                        fireRequests,
                        snapshot.fireSequence,
                        snapshot.selectedSpellUiIndex,
                        snapshot.selectedSpellHolder,
                        mission.projectileCount,
                        playerFireballs,
                        mission.impactCount,
                        spheres.Length,
                        mission.castleMana,
                        mission.securedManaPercent,
                        driver.LastAimActor,
                        driver.LastAimDistance,
                        driver.LastAimYawError,
                        driver.LastAimPitchError,
                        bestFireflyPass);
                    foreach (var actor in actors)
                    {
                        if (actor.active == 0) continue;
                        UnityEngine.Debug.LogFormat(
                            "MC_LEVEL1_BOT_ACTOR tick={0} authored={1} kind={2} " +
                            "position={3},{4},{5} life={6}/{7} action={8}",
                            tick,
                            actor.authoredIndex,
                            actor.kind,
                            actor.x,
                            actor.y,
                            actor.z,
                            actor.life,
                            actor.maxLife,
                            actor.action);
                    }
                    if (mission.currentStage == 3)
                    {
                        foreach (var goat in goats)
                        {
                            if (goat.active == 0) continue;
                            UnityEngine.Debug.LogFormat(
                                "MC_LEVEL1_BOT_GOAT tick={0} entity={1} target={2} " +
                                "position={3},{4},{5} life={6}/{7} action={8} " +
                                "stage={9} phase={10} damage={11} flags=0x{12:X8}",
                                tick,
                                goat.entityIndex,
                                goat.targetEntityIndex,
                                goat.x,
                                goat.y,
                                goat.z,
                                goat.life,
                                goat.maxLife,
                                goat.action,
                                goat.stageVar2,
                                goat.phase,
                                goat.pendingDamage,
                                goat.controlFlags);
                        }
                        var carrier = session.Level1ManaCarrier(mission);
                        UnityEngine.Debug.LogFormat(
                            "MC_LEVEL1_BOT_CARRIER tick={0} active={1} owner={2} " +
                            "position={3},{4},{5} target={6} returning={7} carried={8}",
                            tick,
                            carrier.active,
                            carrier.ownerEntityIndex,
                            carrier.x,
                            carrier.y,
                            carrier.z,
                            carrier.targetSphereId,
                            carrier.returningToCastle,
                            carrier.carriedMana);
                        foreach (var sphere in spheres)
                        {
                            if (sphere.active == 0) continue;
                            UnityEngine.Debug.LogFormat(
                                "MC_LEVEL1_BOT_SPHERE tick={0} id={1} owner={2} " +
                                "position={3},{4},{5} velocity={6},{7},{8} mana={9}",
                                tick,
                                sphere.sphereId,
                                sphere.ownerEntityIndex,
                                sphere.x,
                                sphere.y,
                                sphere.z,
                                sphere.velocityX,
                                sphere.velocityY,
                                sphere.verticalVelocity,
                                sphere.mana);
                        }
                        foreach (var projectile in session.FireballProjectiles(mission))
                        {
                            if (projectile.projectileKind != 5) continue;
                            UnityEngine.Debug.LogFormat(
                                "MC_LEVEL1_BOT_POSSESSION tick={0} id={1} " +
                                "position={2},{3},{4} yaw={5} pitch={6} life={7}",
                                tick,
                                projectile.projectileId,
                                projectile.x,
                                projectile.y,
                                projectile.z,
                                projectile.yaw,
                                projectile.pitch,
                                projectile.lifeTicks);
                        }
                    }
                }
                snapshot = session.Step(
                    command.Roll,
                    command.Pitch,
                    command.Flags,
                    opcode: command.Opcode,
                    argument1: command.Argument1,
                    requestedSpell: command.RequestedSpell);
                mission = session.Level1Mission();
                actors = session.Level1Actors(mission);
                terrain = session.TerrainState();
                var lifecycle = session.Level1CastleLifecycle(mission);
                Assert.That(terrain.generation, Is.EqualTo(snapshot.generation));
                Assert.That(terrain.tick, Is.EqualTo(snapshot.tick));
                Assert.That(
                    lifecycle.terrainMutationSequence,
                    Is.EqualTo(terrain.mutationSequence));
                peakTerrainMutationSequence = Math.Max(
                    peakTerrainMutationSequence,
                    terrain.mutationSequence);
                if (mission.currentStage == 3)
                {
                    goats = session.Goats(
                        snapshot.generation,
                        snapshot.tick,
                        out _);
                    spheres = session.Level1ManaSpheres(mission);
                    castle = session.Level1Castle(mission);
                    var carrier = session.Level1ManaCarrier(mission);
                    if (carrier.active != 0)
                    {
                        observedCarrier = true;
                        Assert.That(
                            carrier.carrierId,
                            Is.EqualTo(carrier.reserved[0]));
                        Assert.That(
                            castle.carrierId,
                            Is.EqualTo(carrier.carrierId));
                        Assert.That(carrier.targetSphereId, Is.LessThan(1000));
                    }
                    foreach (var sphere in spheres)
                    {
                        Assert.That(sphere.assignedCarrierId, Is.LessThan(1000));
                    }
                }
                if (mission.exitSequenceComplete != 0) break;
                if (mission.playerDead != 0) break;
            }

            UnityEngine.Debug.Log(
                $"MC_LEVEL1_BOT_CLOSEST_FIREFLY_PASS tick={bestFireflyPassTick} " +
                $"normalized={bestFireflyPass:F3} {bestFireflyPassAxes}");
            UnityEngine.Debug.Log(
                $"MC_LEVEL1_BOT_CLOSEST_POSSESSION_PASS " +
                $"tick={bestPossessionPassTick} distance={bestPossessionPass:F3} " +
                bestPossessionPassAxes);
            Assert.That(
                mission.playerDead,
                Is.Zero,
                $"autopilot died at tick {snapshot.tick}, phase {priorPhase}");
            Assert.That(fireRequests, Is.GreaterThan(0));
            Assert.That(peakProjectiles, Is.GreaterThan(0));
            Assert.That(peakPlayerFireballs, Is.GreaterThan(0));
            Assert.That(peakImpacts, Is.GreaterThan(0));
            Assert.That(observedCarrier, Is.True);
            Assert.That(mission.currentStage, Is.EqualTo(5));
            Assert.That(mission.exitSequenceComplete, Is.EqualTo(1));
            Assert.That(mission.exitResultCode, Is.EqualTo(10));
            // Eighteen Castle growth passes plus its final stable resample
            // are followed by one source-exact house action-53 footprint
            // removal on this deterministic combat route.
            Assert.That(peakTerrainMutationSequence, Is.EqualTo(20));
            Assert.That(terrain.terrainHash, Is.Not.EqualTo(initialTerrainHash));
            var resetSnapshot = session.Reset();
            var resetTerrain = session.TerrainState();
            Assert.That(resetTerrain.generation, Is.EqualTo(resetSnapshot.generation));
            Assert.That(resetTerrain.mutationSequence, Is.Zero);
            Assert.That(resetTerrain.terrainHash, Is.EqualTo(initialTerrainHash));
            UnityEngine.Debug.Log(
                $"MC_LEVEL1_INPUT_ROUTE PASS tick={snapshot.tick} " +
                $"stage={mission.currentStage} result={mission.exitResultCode} " +
                $"terrainSequence={peakTerrainMutationSequence}");
        }

        private static AutopilotDigest RunAutopilotDigest(
            bool includePostCompletion = false,
            int maximumTicks = 120_000)
        {
            using var session = NativeSession.Open(PackagePath, ClosurePath);
            var driver = new Level1Autopilot(includePostCompletion);
            var snapshot = session.Snapshot();
            var mission = session.Level1Mission();
            var actors = session.Level1Actors(mission);
            var goats = session.Goats(
                snapshot.generation,
                snapshot.tick,
                out _);
            var spheres = Array.Empty<NativeSim.ManaSphereSnapshot>();
            var castle = session.Level1Castle(mission);
            var eventBuffer = new NativeSim.NativeEvent[256];
            var eventCursor = 0UL;
            var eventCount = 0UL;
            var commandHash = FnvOffset;
            var eventHash = FnvOffset;
            var stateHash = FnvOffset;
            var sourceArenaHash = FnvOffset;
            var multipartHash = FnvOffset;
            var projectileHash = FnvOffset;
            var manaHash = FnvOffset;
            var audioHash = FnvOffset;
            var sawGroup33Firefly = false;
            var sawFirstMultipart = false;
            var sawGroup34Multipart = false;

            for (var tick = 0; tick < maximumTicks; tick++)
            {
                var command = driver.Next(
                    Level1Autopilot.Observe(
                        snapshot,
                        mission,
                        actors,
                        goats,
                        spheres,
                        castle));
                commandHash = HashValue(commandHash, unchecked((byte)command.Roll));
                commandHash = HashValue(commandHash, unchecked((byte)command.Pitch));
                commandHash = HashValue(commandHash, unchecked((byte)command.Flags));
                commandHash = HashValue(
                    commandHash,
                    unchecked((ushort)command.RequestedSpell));
                commandHash = HashValue(commandHash, unchecked((byte)command.Opcode));
                commandHash = HashValue(commandHash, unchecked((byte)command.Argument1));
                commandHash = HashText(commandHash, command.Phase);

                snapshot = session.Step(
                    command.Roll,
                    command.Pitch,
                    command.Flags,
                    opcode: command.Opcode,
                    argument1: command.Argument1,
                    requestedSpell: command.RequestedSpell);
                mission = session.Level1Mission();
                actors = session.Level1Actors(mission);
                sawGroup33Firefly |= Array.Exists(
                    actors,
                    actor => actor.authoredIndex == 306 && actor.active != 0);
                sawFirstMultipart |= Array.Exists(
                    actors,
                    actor => actor.authoredIndex == 307 && actor.active != 0);
                sawGroup34Multipart |= Array.Exists(
                    actors,
                    actor => actor.authoredIndex >= 309 &&
                             actor.authoredIndex <= 311 &&
                             actor.active != 0);
                if (mission.currentStage == 3)
                {
                    goats = session.Goats(
                        snapshot.generation,
                        snapshot.tick,
                        out _);
                    spheres = session.Level1ManaSpheres(mission);
                    castle = session.Level1Castle(mission);
                }
                if (includePostCompletion && tick % 10_000 == 0)
                {
                    var multipartHeads = Array.FindAll(
                        actors,
                        actor => actor.authoredIndex == 307 ||
                                 (actor.authoredIndex >= 309 &&
                                  actor.authoredIndex <= 311));
                    var multipartState = string.Join(
                        ",",
                        Array.ConvertAll(
                            multipartHeads,
                            actor =>
                                $"{actor.authoredIndex}:" +
                                $"{actor.active}/{actor.life}/{actor.action}"));
                    UnityEngine.Debug.Log(
                        $"MC_LEVEL1_POST_PROGRESS tick={snapshot.tick} " +
                        $"stage={mission.currentStage} phase={command.Phase} " +
                        $"life={mission.playerLife} " +
                        $"fireSequence={snapshot.fireSequence} " +
                        $"aim={driver.LastAimActor}/" +
                        $"{driver.LastAimDistance:F0}/" +
                        $"{driver.LastAimYawError}/" +
                        $"{driver.LastAimPitchError} " +
                        $"seen={sawGroup33Firefly}/{sawFirstMultipart}/" +
                        $"{sawGroup34Multipart} multipart=[{multipartState}]");
                }

                stateHash = HashValue(stateHash, snapshot.stateHash);
                stateHash = HashValue(stateHash, snapshot.conformanceHash);
                stateHash = HashValue(stateHash, snapshot.tick);
                stateHash = HashValue(stateHash, mission.currentStage);
                stateHash = HashValue(stateHash, unchecked((uint)mission.playerLife));
                stateHash = HashValue(stateHash, unchecked((uint)mission.playerMana));
                stateHash = HashValue(stateHash, unchecked((uint)mission.castleMana));
                stateHash = HashValue(
                    stateHash,
                    unchecked((uint)mission.securedManaPercent));
                foreach (var actor in actors)
                {
                    stateHash = HashValue(stateHash, actor.authoredIndex);
                    stateHash = HashValue(stateHash, actor.kind);
                    stateHash = HashValue(stateHash, actor.active);
                    stateHash = HashValue(stateHash, actor.x);
                    stateHash = HashValue(stateHash, actor.y);
                    stateHash = HashValue(stateHash, unchecked((ushort)actor.z));
                    stateHash = HashValue(stateHash, unchecked((uint)actor.life));
                    stateHash = HashValue(stateHash, actor.action);
                }
                if (includePostCompletion)
                {
                    sourceArenaHash = HashValue(
                        sourceArenaHash,
                        session.SourceArenaHash());
                    foreach (var segment in session.Level1MultipartSegments(mission))
                    {
                        multipartHash = HashValue(
                            multipartHash,
                            segment.sourceEntityIndex);
                        multipartHash = HashValue(
                            multipartHash,
                            segment.headAuthoredIndex);
                        multipartHash = HashValue(multipartHash, segment.ordinal);
                        multipartHash = HashValue(multipartHash, segment.active);
                        multipartHash = HashValue(multipartHash, segment.visible);
                        multipartHash = HashValue(multipartHash, segment.action);
                        multipartHash = HashValue(multipartHash, segment.x);
                        multipartHash = HashValue(multipartHash, segment.y);
                        multipartHash = HashValue(
                            multipartHash,
                            unchecked((ushort)segment.z));
                        multipartHash = HashValue(
                            multipartHash,
                            unchecked((uint)segment.life));
                    }
                    foreach (var projectile in session.FireballProjectiles(mission))
                    {
                        projectileHash = HashValue(
                            projectileHash,
                            projectile.projectileId);
                        projectileHash = HashValue(
                            projectileHash,
                            projectile.projectileKind);
                        projectileHash = HashValue(
                            projectileHash,
                            projectile.active);
                        projectileHash = HashValue(projectileHash, projectile.x);
                        projectileHash = HashValue(projectileHash, projectile.y);
                        projectileHash = HashValue(
                            projectileHash,
                            unchecked((ushort)projectile.z));
                        projectileHash = HashValue(
                            projectileHash,
                            unchecked((ushort)projectile.lifeTicks));
                    }
                    foreach (var impact in session.FireballImpacts(mission))
                    {
                        projectileHash = HashValue(
                            projectileHash,
                            impact.impactId);
                        projectileHash = HashValue(
                            projectileHash,
                            impact.sourceProjectileId);
                        projectileHash = HashValue(projectileHash, impact.active);
                        projectileHash = HashValue(
                            projectileHash,
                            unchecked((byte)impact.animationFrame));
                        projectileHash = HashValue(projectileHash, impact.x);
                        projectileHash = HashValue(projectileHash, impact.y);
                        projectileHash = HashValue(
                            projectileHash,
                            unchecked((ushort)impact.z));
                    }
                    spheres = session.Level1ManaSpheres(mission);
                    foreach (var sphere in spheres)
                    {
                        manaHash = HashValue(manaHash, sphere.sphereId);
                        manaHash = HashValue(
                            manaHash,
                            sphere.sourceEntityIndex);
                        manaHash = HashValue(manaHash, sphere.ownerEntityIndex);
                        manaHash = HashValue(manaHash, sphere.active);
                        manaHash = HashValue(manaHash, sphere.x);
                        manaHash = HashValue(manaHash, sphere.y);
                        manaHash = HashValue(
                            manaHash,
                            unchecked((ushort)sphere.z));
                        manaHash = HashValue(
                            manaHash,
                            unchecked((uint)sphere.mana));
                        manaHash = HashValue(
                            manaHash,
                            sphere.assignedCarrierId);
                    }
                }

                while (true)
                {
                    var status = session.CopyEvents(
                        snapshot.generation,
                        eventCursor,
                        eventBuffer,
                        out var batch,
                        out var error);
                    Assert.That(
                        status,
                        Is.EqualTo(NativeSim.Ok),
                        $"copy deterministic events: {error}");
                    for (var index = 0; index < batch.written; index++)
                    {
                        var nativeEvent = eventBuffer[index];
                        eventHash = HashValue(eventHash, nativeEvent.stateHash);
                        eventHash = HashValue(eventHash, nativeEvent.kind);
                        eventHash = HashValue(eventHash, nativeEvent.code);
                        eventHash = HashValue(eventHash, nativeEvent.entityIndex);
                        eventHash = HashValue(
                            eventHash,
                            unchecked((ushort)nativeEvent.playerIndex));
                        eventHash = HashValue(
                            eventHash,
                            unchecked((uint)nativeEvent.value0));
                        eventHash = HashValue(
                            eventHash,
                            unchecked((uint)nativeEvent.value1));
                        eventHash = HashValue(eventHash, nativeEvent.assetRefIndex);
                        if (includePostCompletion &&
                            (MagicCarpetRuntime.EventUsesAudioSidecar(
                                 nativeEvent.kind) ||
                             nativeEvent.kind == NativeSim.EventStartMusic ||
                             nativeEvent.kind == NativeSim.EventUpdateMusic))
                        {
                            audioHash = HashValue(audioHash, nativeEvent.kind);
                            audioHash = HashValue(audioHash, nativeEvent.code);
                            audioHash = HashValue(
                                audioHash,
                                nativeEvent.entityIndex);
                            audioHash = HashValue(
                                audioHash,
                                unchecked((uint)nativeEvent.value0));
                            audioHash = HashValue(
                                audioHash,
                                unchecked((uint)nativeEvent.value1));
                            audioHash = HashValue(
                                audioHash,
                                nativeEvent.assetRefIndex);
                        }
                        eventCount++;
                    }
                    eventCursor = batch.nextCursor;
                    if (batch.hasMore == 0) break;
                }
                Assert.That(eventCursor, Is.EqualTo(snapshot.eventHead));
                if (mission.exitSequenceComplete != 0 || mission.playerDead != 0)
                    break;
            }

            Assert.That(
                mission.playerDead,
                Is.Zero,
                $"postgame={includePostCompletion} tick={snapshot.tick} " +
                $"phase={driver.PriorPhase} life={mission.playerLife} " +
                $"seen={sawGroup33Firefly}/{sawFirstMultipart}/" +
                $"{sawGroup34Multipart}");
            Assert.That(
                mission.exitSequenceComplete,
                Is.EqualTo(1),
                $"postgame={includePostCompletion} tick={snapshot.tick} " +
                $"phase={driver.PriorPhase} life={mission.playerLife} " +
                $"seen={sawGroup33Firefly}/{sawFirstMultipart}/" +
                $"{sawGroup34Multipart}");
            var terminalHash = FnvOffset;
            terminalHash = HashValue(terminalHash, snapshot.tick);
            terminalHash = HashValue(terminalHash, mission.currentStage);
            terminalHash = HashValue(terminalHash, mission.exitSequenceComplete);
            terminalHash = HashValue(terminalHash, mission.exitResultCode);
            terminalHash = HashValue(
                terminalHash,
                unchecked((uint)mission.playerLife));
            terminalHash = HashValue(
                terminalHash,
                unchecked((uint)mission.playerMana));
            terminalHash = HashValue(terminalHash, session.SourceArenaHash());
            return new AutopilotDigest(
                snapshot.tick,
                mission.currentStage,
                mission.exitResultCode,
                eventCount,
                commandHash,
                eventHash,
                stateHash,
                sawGroup33Firefly,
                sawFirstMultipart,
                sawGroup34Multipart,
                sourceArenaHash,
                multipartHash,
                projectileHash,
                manaHash,
                audioHash,
                terminalHash);
        }

        private static PostCompletionNegativeResult
            RunPostCompletionCombatNegativeControl(
                string suppressedPhase,
                int observationTicks)
        {
            using var session = NativeSession.Open(PackagePath, ClosurePath);
            var driver = new Level1Autopilot(includePostCompletion: true);
            var snapshot = session.Snapshot();
            var mission = session.Level1Mission();
            var actors = session.Level1Actors(mission);
            var goats = session.Goats(
                snapshot.generation,
                snapshot.tick,
                out _);
            var spheres = Array.Empty<NativeSim.ManaSphereSnapshot>();
            var castle = session.Level1Castle(mission);
            var reachedSuppressedPhase = false;
            var suppressedCommands = 0;
            var ticksAfterSuppression = 0;

            for (var tick = 0; tick < 120_000; tick++)
            {
                var command = driver.Next(
                    Level1Autopilot.Observe(
                        snapshot,
                        mission,
                        actors,
                        goats,
                        spheres,
                        castle));
                if (command.Phase == suppressedPhase)
                {
                    reachedSuppressedPhase = true;
                    suppressedCommands++;
                    command = new Level1Autopilot.Command(
                        command.Roll,
                        command.Pitch,
                        unchecked((byte)command.Flags) & ~0x40,
                        -1,
                        command.Phase,
                        command.Opcode,
                        command.Argument1);
                }

                snapshot = session.Step(
                    command.Roll,
                    command.Pitch,
                    command.Flags,
                    opcode: command.Opcode,
                    argument1: command.Argument1,
                    requestedSpell: command.RequestedSpell);
                mission = session.Level1Mission();
                actors = session.Level1Actors(mission);
                if (mission.currentStage == 3)
                {
                    goats = session.Goats(
                        snapshot.generation,
                        snapshot.tick,
                        out _);
                    spheres = session.Level1ManaSpheres(mission);
                    castle = session.Level1Castle(mission);
                }
                if (tick % 10_000 == 0)
                {
                    UnityEngine.Debug.Log(
                        $"MC_LEVEL1_POST_NEGATIVE_PROGRESS " +
                        $"suppressedPhase={suppressedPhase} " +
                        $"tick={snapshot.tick} stage={mission.currentStage} " +
                        $"phase={command.Phase} " +
                        $"reached={reachedSuppressedPhase} " +
                        $"observed={ticksAfterSuppression}/{observationTicks}");
                }

                if (reachedSuppressedPhase)
                {
                    ticksAfterSuppression++;
                    if (ticksAfterSuppression >= observationTicks) break;
                }
                if (mission.exitSequenceComplete != 0 ||
                    mission.playerDead != 0)
                {
                    break;
                }
            }

            return new PostCompletionNegativeResult(
                reachedSuppressedPhase,
                suppressedCommands,
                mission.exitSequenceComplete,
                mission.playerDead,
                Array.Exists(
                    actors,
                    actor => actor.authoredIndex == 307 && actor.active != 0),
                Array.Exists(
                    actors,
                    actor => actor.authoredIndex >= 309 &&
                             actor.authoredIndex <= 311 &&
                             actor.active != 0),
                snapshot.tick);
        }

        private static NegativeControlResult RunAutopilotNegativeControl(
            int maximumTicks,
            bool suppressFireball,
            bool suppressPossession)
        {
            using var session = NativeSession.Open(PackagePath, ClosurePath);
            var driver = new Level1Autopilot();
            var snapshot = session.Snapshot();
            var mission = session.Level1Mission();
            var actors = session.Level1Actors(mission);
            var goats = session.Goats(
                snapshot.generation,
                snapshot.tick,
                out _);
            var spheres = Array.Empty<NativeSim.ManaSphereSnapshot>();
            var castle = session.Level1Castle(mission);
            var suppressedCommands = 0;
            var ticksAfterSuppression = 0;

            for (var tick = 0; tick < maximumTicks; tick++)
            {
                var command = driver.Next(
                    Level1Autopilot.Observe(
                        snapshot,
                        mission,
                        actors,
                        goats,
                        spheres,
                        castle));
                var flags = unchecked((byte)command.Flags);
                var requestedSpell = command.RequestedSpell;
                if (suppressFireball &&
                    (flags & 0x40) != 0 &&
                    command.Phase == "fight-archers")
                {
                    flags = (byte)(flags & ~0x40);
                    requestedSpell = NativeSim.NoRequestedSpell;
                    suppressedCommands++;
                }
                if (suppressPossession && (flags & 0x20) != 0)
                {
                    flags = (byte)(flags & ~0x20);
                    suppressedCommands++;
                }
                snapshot = session.Step(
                    command.Roll,
                    command.Pitch,
                    unchecked((sbyte)flags),
                    opcode: command.Opcode,
                    argument1: command.Argument1,
                    requestedSpell: requestedSpell);
                mission = session.Level1Mission();
                actors = session.Level1Actors(mission);
                if (mission.currentStage == 3)
                {
                    goats = session.Goats(
                        snapshot.generation,
                        snapshot.tick,
                        out _);
                    spheres = session.Level1ManaSpheres(mission);
                    castle = session.Level1Castle(mission);
                }
                if (mission.exitSequenceComplete != 0 || mission.playerDead != 0)
                    break;
                if (suppressedCommands > 0)
                {
                    ticksAfterSuppression++;
                    if (ticksAfterSuppression >= 1_024)
                        break;
                }
            }

            return new NegativeControlResult(
                mission.currentStage,
                mission.castleExists,
                mission.securedManaPercent,
                mission.exitSequenceComplete,
                suppressedCommands);
        }

        private static NaturalFireflyResult RunNaturalFireflyRetaliation(
            int maximumStageFourTicks = 20_000)
        {
            using var session = NativeSession.Open(PackagePath, ClosurePath);
            var driver = new Level1Autopilot();
            var snapshot = session.Snapshot();
            var mission = session.Level1Mission();
            var actors = session.Level1Actors(mission);
            var goats = session.Goats(
                snapshot.generation,
                snapshot.tick,
                out _);
            var spheres = Array.Empty<NativeSim.ManaSphereSnapshot>();
            var castle = session.Level1Castle(mission);
            var eventBuffer = new NativeSim.NativeEvent[256];
            var eventCursor = 0UL;
            var commandHash = FnvOffset;
            var eventHash = FnvOffset;
            var stateHash = FnvOffset;
            var stageFourStartTick = 0UL;
            var stageFourFireCommands = 0;
            var fireflyHitCount = 0;
            var hostileSpawnCount = 0;
            var hostileImpactCount = 0;
            var hostilePlayerDamageCount = 0;
            var contactPlayerDamageCount = 0;
            var minimumPlayerLife = mission.playerLife;
            var hostileProjectiles =
                new System.Collections.Generic.HashSet<uint>();
            var visibleHostileImpacts =
                new System.Collections.Generic.HashSet<uint>();
            var holdingFireflyEngagement = false;

            for (var tick = 0; tick < 120_000; tick++)
            {
                var command = driver.Next(
                    Level1Autopilot.Observe(
                        snapshot,
                        mission,
                        actors,
                        goats,
                        spheres,
                        castle));
                var flags = unchecked((byte)command.Flags);
                var requestedSpell = command.RequestedSpell;
                if (mission.currentStage == 4)
                {
                    if (stageFourStartTick == 0)
                    {
                        stageFourStartTick = snapshot.tick;
                        minimumPlayerLife = mission.playerLife;
                    }
                    var nearestFireflyDistance =
                        NearestVisibleActorDistance(snapshot, actors, 2);
                    if (nearestFireflyDistance <= 4_000)
                    {
                        holdingFireflyEngagement = true;
                    }
                    else if (nearestFireflyDistance > 4_800)
                    {
                        holdingFireflyEngagement = false;
                    }
                    if (holdingFireflyEngagement)
                    {
                        // Once a visible Firefly enters its source 5,120-unit
                        // engagement radius, hold the carpet still through
                        // ordinary full-stop input. This lets the unobserved
                        // source AI choose ranged or charge behavior without
                        // the test steering from actions, RNG, or projectiles.
                        command = new Level1Autopilot.Command(
                            0,
                            0,
                            0,
                            NativeSim.NoRequestedSpell,
                            "observe-firefly",
                            0x27,
                            1);
                    }
                    flags = unchecked((byte)command.Flags);
                    requestedSpell = command.RequestedSpell;

                    var requestsFireball =
                        (flags & 0x40) != 0 &&
                        requestedSpell == 0;
                    if (requestsFireball)
                    {
                        flags = (byte)(flags & ~0x40);
                        requestedSpell = NativeSim.NoRequestedSpell;
                    }

                    // Keep only ordinary movement and non-Fireball spell input
                    // derived from sanitized presentation state. The event
                    // oracle below never feeds enemy health, AI state, RNG,
                    // projectiles, or damage into the driver.
                    command = new Level1Autopilot.Command(
                        command.Roll,
                        command.Pitch,
                        flags,
                        requestedSpell,
                        command.Phase,
                        command.Opcode,
                        command.Argument1);
                    if ((unchecked((byte)command.Flags) & 0x40) != 0 &&
                        command.RequestedSpell == 0)
                    {
                        stageFourFireCommands++;
                    }
                }

                commandHash = HashValue(commandHash, unchecked((byte)command.Roll));
                commandHash = HashValue(commandHash, unchecked((byte)command.Pitch));
                commandHash = HashValue(commandHash, unchecked((byte)command.Flags));
                commandHash = HashValue(
                    commandHash,
                    unchecked((ushort)command.RequestedSpell));
                commandHash = HashValue(commandHash, unchecked((byte)command.Opcode));
                commandHash = HashValue(commandHash, unchecked((byte)command.Argument1));
                commandHash = HashText(commandHash, command.Phase);

                snapshot = session.Step(
                    command.Roll,
                    command.Pitch,
                    command.Flags,
                    opcode: command.Opcode,
                    argument1: command.Argument1,
                    requestedSpell: command.RequestedSpell);
                mission = session.Level1Mission();
                actors = session.Level1Actors(mission);
                if (mission.currentStage == 3)
                {
                    goats = session.Goats(
                        snapshot.generation,
                        snapshot.tick,
                        out _);
                    spheres = session.Level1ManaSpheres(mission);
                    castle = session.Level1Castle(mission);
                }
                if (stageFourStartTick != 0)
                {
                    minimumPlayerLife = Math.Min(minimumPlayerLife, mission.playerLife);
                }
                foreach (var impact in session.FireballImpacts(mission))
                {
                    if (impact.active != 0 &&
                        impact.model == 0 &&
                        hostileProjectiles.Contains(impact.sourceProjectileId))
                    {
                        visibleHostileImpacts.Add(impact.impactId);
                    }
                }

                stateHash = HashValue(stateHash, snapshot.stateHash);
                stateHash = HashValue(stateHash, snapshot.conformanceHash);
                stateHash = HashValue(stateHash, snapshot.tick);
                stateHash = HashValue(stateHash, mission.currentStage);
                stateHash = HashValue(stateHash, unchecked((uint)mission.playerLife));
                foreach (var actor in actors)
                {
                    stateHash = HashValue(stateHash, actor.authoredIndex);
                    stateHash = HashValue(stateHash, actor.kind);
                    stateHash = HashValue(stateHash, actor.active);
                    stateHash = HashValue(stateHash, actor.x);
                    stateHash = HashValue(stateHash, actor.y);
                    stateHash = HashValue(stateHash, unchecked((ushort)actor.z));
                }

                while (true)
                {
                    var status = session.CopyEvents(
                        snapshot.generation,
                        eventCursor,
                        eventBuffer,
                        out var batch,
                        out var error);
                    Assert.That(
                        status,
                        Is.EqualTo(NativeSim.Ok),
                        $"copy natural Firefly events: {error}");
                    for (var index = 0; index < batch.written; index++)
                    {
                        var nativeEvent = eventBuffer[index];
                        eventHash = HashValue(eventHash, nativeEvent.stateHash);
                        eventHash = HashValue(eventHash, nativeEvent.kind);
                        eventHash = HashValue(eventHash, nativeEvent.code);
                        eventHash = HashValue(eventHash, nativeEvent.entityIndex);
                        eventHash = HashValue(
                            eventHash,
                            unchecked((uint)nativeEvent.value0));
                        eventHash = HashValue(
                            eventHash,
                            unchecked((uint)nativeEvent.value1));

                        if (stageFourStartTick != 0 &&
                            nativeEvent.kind == NativeSim.EventLevel1ActorDamaged &&
                            nativeEvent.value0 == 250 &&
                            Array.Exists(
                                actors,
                                actor =>
                                    actor.authoredIndex == nativeEvent.entityIndex &&
                                    actor.kind == 2))
                        {
                            fireflyHitCount++;
                        }
                        else if (stageFourStartTick != 0 &&
                                 nativeEvent.kind ==
                                 NativeSim.EventLevel1FireflyFireballSpawned)
                        {
                            hostileProjectiles.Add(nativeEvent.code);
                            hostileSpawnCount++;
                        }
                        else if (stageFourStartTick != 0 &&
                                 nativeEvent.kind ==
                                     NativeSim.EventLevel1FireballImpacted &&
                                 hostileProjectiles.Contains(
                                     unchecked((uint)nativeEvent.value0)))
                        {
                            hostileImpactCount++;
                        }
                        else if (stageFourStartTick != 0 &&
                                 nativeEvent.kind ==
                                     NativeSim.EventLevel1PlayerDamaged)
                        {
                            if (nativeEvent.value0 == 500 &&
                                hostileImpactCount > 0)
                            {
                                hostilePlayerDamageCount++;
                            }
                            else if (nativeEvent.value0 == 300)
                            {
                                contactPlayerDamageCount++;
                            }
                        }
                    }
                    eventCursor = batch.nextCursor;
                    if (batch.hasMore == 0) break;
                }

                if (stageFourStartTick != 0 &&
                    hostilePlayerDamageCount > 0)
                {
                    break;
                }
                if (stageFourStartTick != 0 &&
                    snapshot.tick - stageFourStartTick >=
                    (ulong)maximumStageFourTicks)
                {
                    break;
                }
                if (mission.playerDead != 0) break;
            }

            return new NaturalFireflyResult(
                snapshot.tick,
                stageFourFireCommands,
                fireflyHitCount,
                hostileSpawnCount,
                hostileImpactCount,
                visibleHostileImpacts.Count,
                hostilePlayerDamageCount,
                contactPlayerDamageCount,
                minimumPlayerLife,
                commandHash,
                eventHash,
                stateHash);
        }

        private static double NearestVisibleActorDistance(
            NativeSim.SessionSnapshot player,
            NativeSim.Level1ActorSnapshot[] actors,
            byte kind)
        {
            var nearest = double.MaxValue;
            foreach (var actor in actors)
            {
                if (actor.kind != kind || actor.active == 0) continue;
                var dx = ((actor.x - player.x + 32_768) & 0xffff) - 32_768;
                var dy = ((actor.y - player.y + 32_768) & 0xffff) - 32_768;
                nearest = Math.Min(
                    nearest,
                    Math.Sqrt((double)dx * dx + (double)dy * dy));
            }
            return nearest;
        }

        private const ulong FnvOffset = 14_695_981_039_346_656_037UL;
        private const ulong FnvPrime = 1_099_511_628_211UL;

        private static ulong HashValue(ulong hash, ulong value)
        {
            for (var shift = 0; shift < 64; shift += 8)
            {
                hash ^= (byte)(value >> shift);
                hash *= FnvPrime;
            }
            return hash;
        }

        private static ulong HashText(ulong hash, string value)
        {
            foreach (var character in value)
            {
                hash = HashValue(hash, character);
            }
            return hash;
        }

        private readonly struct AutopilotDigest
        {
            internal readonly ulong Tick;
            internal readonly byte Stage;
            internal readonly byte Result;
            internal readonly ulong EventCount;
            internal readonly ulong CommandHash;
            internal readonly ulong EventHash;
            internal readonly ulong StateHash;
            internal readonly bool SawGroup33Firefly;
            internal readonly bool SawFirstMultipart;
            internal readonly bool SawGroup34Multipart;
            internal readonly ulong SourceArenaHash;
            internal readonly ulong MultipartHash;
            internal readonly ulong ProjectileHash;
            internal readonly ulong ManaHash;
            internal readonly ulong AudioHash;
            internal readonly ulong TerminalHash;

            internal AutopilotDigest(
                ulong tick,
                byte stage,
                byte result,
                ulong eventCount,
                ulong commandHash,
                ulong eventHash,
                ulong stateHash,
                bool sawGroup33Firefly,
                bool sawFirstMultipart,
                bool sawGroup34Multipart,
                ulong sourceArenaHash,
                ulong multipartHash,
                ulong projectileHash,
                ulong manaHash,
                ulong audioHash,
                ulong terminalHash)
            {
                Tick = tick;
                Stage = stage;
                Result = result;
                EventCount = eventCount;
                CommandHash = commandHash;
                EventHash = eventHash;
                StateHash = stateHash;
                SawGroup33Firefly = sawGroup33Firefly;
                SawFirstMultipart = sawFirstMultipart;
                SawGroup34Multipart = sawGroup34Multipart;
                SourceArenaHash = sourceArenaHash;
                MultipartHash = multipartHash;
                ProjectileHash = projectileHash;
                ManaHash = manaHash;
                AudioHash = audioHash;
                TerminalHash = terminalHash;
            }

        }

        private readonly struct NegativeControlResult
        {
            internal readonly byte Stage;
            internal readonly byte CastleExists;
            internal readonly int SecuredManaPercent;
            internal readonly byte ExitComplete;
            internal readonly int SuppressedCommands;

            internal NegativeControlResult(
                byte stage,
                byte castleExists,
                int securedManaPercent,
                byte exitComplete,
                int suppressedCommands)
            {
                Stage = stage;
                CastleExists = castleExists;
                SecuredManaPercent = securedManaPercent;
                ExitComplete = exitComplete;
                SuppressedCommands = suppressedCommands;
            }
        }

        private readonly struct PostCompletionNegativeResult
        {
            internal readonly bool ReachedSuppressedPhase;
            internal readonly int SuppressedCommands;
            internal readonly byte ExitComplete;
            internal readonly byte PlayerDead;
            internal readonly bool FirstMultipartActive;
            internal readonly bool Group34Active;
            internal readonly ulong Tick;

            internal PostCompletionNegativeResult(
                bool reachedSuppressedPhase,
                int suppressedCommands,
                byte exitComplete,
                byte playerDead,
                bool firstMultipartActive,
                bool group34Active,
                ulong tick)
            {
                ReachedSuppressedPhase = reachedSuppressedPhase;
                SuppressedCommands = suppressedCommands;
                ExitComplete = exitComplete;
                PlayerDead = playerDead;
                FirstMultipartActive = firstMultipartActive;
                Group34Active = group34Active;
                Tick = tick;
            }
        }

        private readonly struct NaturalFireflyResult
        {
            internal readonly ulong Tick;
            internal readonly int StageFourFireCommands;
            internal readonly int FireflyHitCount;
            internal readonly int HostileSpawnCount;
            internal readonly int HostileImpactCount;
            internal readonly int VisibleHostileImpactCount;
            internal readonly int HostilePlayerDamageCount;
            internal readonly int ContactPlayerDamageCount;
            internal readonly int MinimumPlayerLife;
            internal readonly ulong CommandHash;
            internal readonly ulong EventHash;
            internal readonly ulong StateHash;

            internal NaturalFireflyResult(
                ulong tick,
                int stageFourFireCommands,
                int fireflyHitCount,
                int hostileSpawnCount,
                int hostileImpactCount,
                int visibleHostileImpactCount,
                int hostilePlayerDamageCount,
                int contactPlayerDamageCount,
                int minimumPlayerLife,
                ulong commandHash,
                ulong eventHash,
                ulong stateHash)
            {
                Tick = tick;
                StageFourFireCommands = stageFourFireCommands;
                FireflyHitCount = fireflyHitCount;
                HostileSpawnCount = hostileSpawnCount;
                HostileImpactCount = hostileImpactCount;
                VisibleHostileImpactCount = visibleHostileImpactCount;
                HostilePlayerDamageCount = hostilePlayerDamageCount;
                ContactPlayerDamageCount = contactPlayerDamageCount;
                MinimumPlayerLife = minimumPlayerLife;
                CommandHash = commandHash;
                EventHash = eventHash;
                StateHash = stateHash;
            }

            public override string ToString()
            {
                return $"tick={Tick},fire={StageFourFireCommands}," +
                       $"hits={FireflyHitCount},spawns={HostileSpawnCount}," +
                       $"impacts={HostileImpactCount}/{VisibleHostileImpactCount}," +
                       $"damage={HostilePlayerDamageCount}," +
                       $"contact={ContactPlayerDamageCount},minLife={MinimumPlayerLife}";
            }
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

        [Test]
        public void WorldFrameCompositesSupportedFamiliesWithoutChangingTerrainOnlyContract()
        {
            using var session = NativeSession.Open(PackagePath, ClosurePath);
            var camera = session.RenderCameraState();
            var terrainIndices = new byte[NativeSim.TerrainFramePixels];
            var terrainCoverage = new byte[NativeSim.TerrainFramePixels];
            var worldIndices = new byte[NativeSim.TerrainFramePixels];
            var worldCoverage = new byte[NativeSim.TerrainFramePixels];

            var terrain = session.TerrainFrame(
                camera, terrainIndices, terrainCoverage);
            var world = session.WorldFrame(
                camera, worldIndices, worldCoverage);
            Assert.That(
                terrain.flags & NativeSim.TerrainFrameGoatsComposited,
                Is.Zero,
                "stable terrain-only endpoint remains terrain-only");
            Assert.That(
                terrain.flags & NativeSim.TerrainFrameArchersComposited,
                Is.Zero,
                "stable terrain-only endpoint omits Archer compositing");
            Assert.That(
                terrain.flags & NativeSim.TerrainFrameFirefliesComposited,
                Is.Zero,
                "stable terrain-only endpoint omits Firefly compositing");
            Assert.That(
                terrain.flags & NativeSim.TerrainFrameEconomyComposited,
                Is.Zero,
                "stable terrain-only endpoint omits castle/mana compositing");
            Assert.That(
                terrain.flags & NativeSim.TerrainFrameReflectedSpritesComposited,
                Is.Zero,
                "stable terrain-only endpoint omits reflected sprites");
            Assert.That(
                terrain.flags & NativeSim.TerrainFrameSpriteShadowsComposited,
                Is.Zero,
                "stable terrain-only endpoint omits sprite shadows");
            Assert.That(
                terrain.flags & NativeSim.TerrainFrameFrozenActorsComposited,
                Is.Zero,
                "stable terrain-only endpoint omits reset-frozen actors");
            Assert.That(
                world.flags & NativeSim.TerrainFrameGoatsComposited,
                Is.Not.Zero,
                "versioned world endpoint owns Goat compositing");
            Assert.That(
                world.flags & NativeSim.TerrainFrameArchersComposited,
                Is.Not.Zero,
                "versioned world endpoint owns Archer compositing");
            Assert.That(
                world.flags & NativeSim.TerrainFrameFirefliesComposited,
                Is.Not.Zero,
                "versioned world endpoint owns Firefly compositing");
            Assert.That(
                world.flags & NativeSim.TerrainFrameEconomyComposited,
                Is.Not.Zero,
                "versioned world endpoint owns castle/mana compositing");
            Assert.That(
                (world.flags &
                 NativeSim.TerrainFrameReflectedSpritesComposited) != 0,
                Is.EqualTo(camera.eyeZ < NativeSim.ReflectionEyeZLimit),
                "world endpoint reports reflected sprites only when the " +
                "source eye-height gate ran that pass");
            Assert.That(
                world.flags & NativeSim.TerrainFrameSpriteShadowsComposited,
                Is.Not.Zero,
                "versioned world endpoint owns sprite-shadow compositing");
            Assert.That(
                world.flags & NativeSim.TerrainFrameFrozenActorsComposited,
                Is.Not.Zero,
                "versioned world endpoint owns the 31 reset-frozen actors");
            Assert.That(
                worldCoverage,
                Is.EqualTo(terrainCoverage),
                "Goat pixels do not alter terrain-only coverage");

            var repeated = session.WorldFrame(
                camera, worldIndices, worldCoverage);
            Assert.That(repeated.indexHash, Is.EqualTo(world.indexHash));
            Assert.That(repeated.coverageHash, Is.EqualTo(world.coverageHash));
        }

        [Test]
        public void MissionActorManagedFallbackIsSuppressedOnlyByItsNativeCapability()
        {
            Assert.That(
                MagicCarpetRuntime.ManagedMissionActorVisible(1, 1, 1, 0),
                Is.True,
                "missing Archer bit retains the managed fallback");
            Assert.That(
                MagicCarpetRuntime.ManagedMissionActorVisible(
                    1,
                    1,
                    1,
                    NativeSim.TerrainFrameArchersComposited),
                Is.False,
                "native Archer composition suppresses the duplicate billboard");
            Assert.That(
                MagicCarpetRuntime.ManagedMissionActorVisible(
                    2,
                    1,
                    1,
                    NativeSim.TerrainFrameArchersComposited),
                Is.True,
                "the Archer bit cannot suppress Firefly fallback");
            Assert.That(
                MagicCarpetRuntime.ManagedMissionActorVisible(
                    2,
                    1,
                    1,
                    NativeSim.TerrainFrameFirefliesComposited),
                Is.False,
                "native Firefly composition suppresses the duplicate billboard");
            Assert.That(
                MagicCarpetRuntime.ManagedMissionActorVisible(
                    1,
                    1,
                    1,
                    NativeSim.TerrainFrameFirefliesComposited),
                Is.True,
                "the Firefly bit cannot suppress Archer fallback");
            foreach (var kind in new byte[] { 3, 4, 5 })
            {
                Assert.That(
                    MagicCarpetRuntime.ManagedMissionActorVisible(
                        kind, 1, 1, 0),
                    Is.True,
                    $"missing objective-prop bit retains kind {kind} fallback");
                Assert.That(
                    MagicCarpetRuntime.ManagedMissionActorVisible(
                        kind,
                        1,
                        1,
                        NativeSim.TerrainFrameObjectivePropsComposited),
                    Is.False,
                    $"native objective-prop composition suppresses kind {kind}");
                Assert.That(
                    MagicCarpetRuntime.ManagedMissionActorVisible(
                        kind,
                        1,
                        1,
                        NativeSim.TerrainFrameCombatComposited),
                    Is.True,
                    $"combat capability cannot suppress objective kind {kind}");
            }
            Assert.That(
                MagicCarpetRuntime.ManagedMissionActorVisible(
                    6,
                    1,
                    1,
                    NativeSim.TerrainFrameObjectivePropsComposited),
                Is.True,
                "sprite-less ExitSwitch is outside objective-prop capability");
            Assert.That(
                MagicCarpetRuntime.ManagedMissionActorVisible(1, 0, 1, 0),
                Is.False);
            Assert.That(
                MagicCarpetRuntime.ManagedMissionActorVisible(1, 1, 0, 0),
                Is.False);
        }

        [Test]
        public void CombatManagedFallbackIsSuppressedForEveryNativeFamily()
        {
            Assert.That(
                MagicCarpetRuntime.ManagedMissionProjectileVisible(
                    1, 1, NativeSim.TerrainFrameCombatComposited),
                Is.False);
            Assert.That(
                MagicCarpetRuntime.ManagedMissionProjectileVisible(
                    2, 1, NativeSim.TerrainFrameCombatComposited),
                Is.False);
            Assert.That(
                MagicCarpetRuntime.ManagedMissionProjectileVisible(
                    4, 1, NativeSim.TerrainFrameCombatComposited),
                Is.False);
            Assert.That(
                MagicCarpetRuntime.ManagedMissionProjectileVisible(
                    5, 1, NativeSim.TerrainFrameCombatComposited),
                Is.False);
            Assert.That(
                MagicCarpetRuntime.ManagedMissionImpactVisible(
                    1, NativeSim.TerrainFrameCombatComposited),
                Is.False);
            Assert.That(
                MagicCarpetRuntime.ManagedMissionImpactVisible(1, 0),
                Is.True);
        }

        [Test]
        public void EconomyManagedFallbackIsSuppressedOnlyByItsNativeCapability()
        {
            Assert.That(MagicCarpetRuntime.ManagedEconomyVisible(1, 0), Is.True);
            Assert.That(
                MagicCarpetRuntime.ManagedEconomyVisible(
                    1, NativeSim.TerrainFrameEconomyComposited),
                Is.False);
            Assert.That(
                MagicCarpetRuntime.ManagedEconomyVisible(
                    1, NativeSim.TerrainFrameCombatComposited),
                Is.True,
                "combat capability cannot suppress castle/mana fallback");
            Assert.That(
                MagicCarpetRuntime.ManagedEconomyVisible(
                    0, NativeSim.TerrainFrameEconomyComposited),
                Is.False);
        }

        [Test]
        public void LiveSimulationUsesTheRemc2DefaultThirtyHertzCadence()
        {
            Assert.That(
                MagicCarpetRuntime.AuthoritativeSimulationHz,
                Is.EqualTo(30));
            Assert.That(
                1f / MagicCarpetRuntime.AuthoritativeSimulationHz,
                Is.EqualTo(1f / 30f));
            Assert.DoesNotThrow(() =>
                PlaythroughRecorder.RequireUniqueTopLevelMembers(
                    "{\"kind\":\"event\",\"value0\":1,\"value1\":2}"));
            Assert.Throws<InvalidDataException>(() =>
                PlaythroughRecorder.RequireUniqueTopLevelMembers(
                    "{\"kind\":\"event\",\"value0\":1,\"value0\":2}"));
        }

        [Test]
        public void FrozenActorManagedFallbackIsSuppressedOnlyForTheQualifiedThirtyOne()
        {
            Assert.That(
                MagicCarpetRuntime.ManagedFrozenActorVisible(1, true, 0),
                Is.True);
            Assert.That(
                MagicCarpetRuntime.ManagedFrozenActorVisible(
                    1,
                    true,
                    NativeSim.TerrainFrameFrozenActorsComposited),
                Is.False,
                "native capability suppresses the qualified 28 villagers and 3 dolmens");
            Assert.That(
                MagicCarpetRuntime.ManagedFrozenActorVisible(
                    1,
                    false,
                    NativeSim.TerrainFrameFrozenActorsComposited),
                Is.True,
                "native capability cannot hide unrelated FrozenObserved debug/effect rows");
            Assert.That(
                MagicCarpetRuntime.ManagedFrozenActorVisible(
                    0,
                    true,
                    NativeSim.TerrainFrameFrozenActorsComposited),
                Is.False);
        }
    }
}
