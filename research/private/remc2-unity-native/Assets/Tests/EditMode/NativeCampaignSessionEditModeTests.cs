using System;
using System.IO;
using System.Linq;
using NUnit.Framework;
using UnityEngine;

namespace MagicCarpet.Native.Tests
{
    public sealed class NativeCampaignSessionEditModeTests
    {
        private static string PrivateRoot =>
            Path.GetFullPath(Path.Combine(Application.dataPath, "../.."));

        private static string Level2Package =>
            Environment.GetEnvironmentVariable("MC_CAMPAIGN_PACKAGE") ??
            Path.Combine(
                PrivateRoot,
                "generated/mc-cf02-campaign-package-v2-b/qualified-level2");

        private static string Level2DayCache =>
            Path.Combine(PrivateRoot, "generated/level2-day-owner-cache");

        private static string Level1Package =>
            Path.Combine(PrivateRoot, "generated/level1-export-m");

        private static string Level1CampaignPackage =>
            Environment.GetEnvironmentVariable("MC_CAMPAIGN_LEVEL1_PACKAGE") ??
            Path.Combine(PrivateRoot, "generated/mc-cf15-level1-package-v3-current");

        private static string Level3Package =>
            Environment.GetEnvironmentVariable("MC_CAMPAIGN_PACKAGE_LEVEL3_V1") ??
            Path.Combine(
                PrivateRoot,
                "generated/mc-cf14-campaign-package-v1-a/qualified-level3");

        private static string Level3DayCache =>
            Environment.GetEnvironmentVariable("MC_CAMPAIGN_ASSET_CACHE_LEVEL3_V1") ??
            Path.Combine(PrivateRoot, "generated/level3-day-material-cache");

        private static string Level3OwnerCache =>
            Environment.GetEnvironmentVariable("MC_CAMPAIGN_OWNER_CACHE_LEVEL3_V1") ??
            Path.Combine(PrivateRoot, "generated/level3-day-owner-cache");

        [Test]
        public void Level2ProductionStartupOpensCampaignSessionAndValidatesDayProfile()
        {
            var root = new GameObject("Level 2 production startup");
            root.SetActive(false);
            MagicCarpetRuntime runtime = null;
            try
            {
                runtime = root.AddComponent<MagicCarpetRuntime>();
                runtime.QualificationInitializeLevel2Production(
                    Level2Package,
                    Level2DayCache);

                Assert.That(runtime.Level2ProductionStartupReady, Is.True);
                var info = runtime.Level2ProductionWorldInfo;
                Assert.That(
                    (info.externalLevel, info.internalLevel, info.mapType, info.profile),
                    Is.EqualTo(((ushort)2, (ushort)1, (byte)0, (byte)0)));
                Assert.That(info.entityCount, Is.EqualTo(1000));
                Assert.That(info.activeEntityCount, Is.EqualTo(114));
                Assert.That(info.playerCount, Is.EqualTo(8));
                Assert.That(info.objectiveCount, Is.EqualTo(8));
                Assert.That(info.terrainWidth, Is.EqualTo(256));
                Assert.That(info.terrainHeight, Is.EqualTo(256));
                Assert.That(NativeSession.ActiveSessionCount, Is.Zero);
                Assert.That(NativeCampaignSession.ActiveSessionCount, Is.EqualTo(1));
            }
            finally
            {
                runtime?.QualificationDisposeNativeOwners();
                UnityEngine.Object.DestroyImmediate(root);
            }

            Assert.That(NativeSession.ActiveSessionCount, Is.Zero);
            Assert.That(NativeCampaignSession.ActiveSessionCount, Is.Zero);
        }

        [Test]
        public void Cf13GenericCampaignConsumerOpensStepsDrainsFramesAndResets()
        {
            var selection = NativeSim.CampaignSelection.Level2Day();
            byte[] initialIndices;
            ulong initialFrameHash;
            ulong steppedCursor;
            using (var session = NativeCampaignSession.Open(
                       Level2Package,
                       selection,
                       Level2DayCache))
            {
                var identity = session.PackageIdentity;
                Assert.That(identity.PackagePath, Is.EqualTo(Path.GetFullPath(Level2Package)));
                Assert.That(identity.ManifestSha256.Length, Is.EqualTo(64));
                Assert.That(identity.DefinitionSha256.Length, Is.EqualTo(64));
                Assert.That(identity.ManifestSha256, Is.Not.EqualTo(identity.DefinitionSha256));
                Assert.That(
                    (identity.Selection.externalLevel,
                     identity.Selection.internalLevel,
                     identity.Selection.mapType),
                    Is.EqualTo(((ushort)2, (ushort)1, (byte)0)));
                Assert.That(session.InitialWorldInfo.resetStateHash, Is.Not.Zero);
                Assert.That(session.DrainEffects(), Is.Empty);
                Assert.That(session.EffectCursor, Is.Zero);

                var initialFrame = session.CopyPresentationFrame(0);
                initialIndices = session.CampaignFrameIndices.ToArray();
                initialFrameHash = initialFrame.indexHash;
                Assert.That(initialFrame.newlyCommitted, Is.EqualTo(1));
                Assert.That(initialFrame.provenanceBytes, Is.Zero);
                Assert.That(
                    initialFrame.flags &
                    (NativeSim.PresentationFrameCompositorComplete |
                     NativeSim.PresentationFrameHasProvenance |
                     NativeSim.PresentationFrameOverlaysComposited |
                     NativeSim.PresentationFramePostprocessComplete |
                     NativeSim.PresentationFrameLightingComplete),
                    Is.Zero);

                var repeatedFrame = session.CopyPresentationFrame(0);
                Assert.That(repeatedFrame.newlyCommitted, Is.Zero);
                Assert.That(repeatedFrame.indexHash, Is.EqualTo(initialFrameHash));
                Assert.That(session.CampaignFrameIndices, Is.EqualTo(initialIndices));

                var localPlayer = session.Players()
                    .Single(player => player.slot == 0 && player.active != 0);
                var localEntity = session.Entities()
                    .Single(entity => entity.entityIndex == localPlayer.entityIndex);
                var step = session.Step(localEntity.x, localEntity.y, localEntity.z);
                var effects = session.DrainEffects();
                Assert.That(effects, Is.Not.Empty);
                Assert.That(
                    effects.Select(effect => effect.sequence),
                    Is.EqualTo(Enumerable.Range(1, effects.Length)
                        .Select(index => (ulong)index)));
                Assert.That(
                    effects.All(effect =>
                        effect.generation == session.Generation &&
                        effect.stateHash == step.snapshot.stateHash &&
                        new[]
                        {
                            NativeSim.CampaignEffectDispatch,
                            NativeSim.CampaignEffectSharedSpell,
                            NativeSim.CampaignEffectFlight,
                            NativeSim.CampaignEffectSceneChange
                        }.Contains(effect.kind)),
                    Is.True);
                steppedCursor = session.EffectCursor;
                Assert.That(steppedCursor, Is.EqualTo((ulong)effects.Length));
                Assert.That(session.DrainEffects(), Is.Empty);
                Assert.That(session.EffectCursor, Is.EqualTo(steppedCursor));

                var futureQuery = NativeSim.EventQuery.Create(
                    session.Generation,
                    checked(session.EffectCursor + 1));
                var futureBuffer = new NativeSim.NativeEvent[1];
                var futureStatus = NativeSim.mc_native_campaign_events_copy_since(
                    session.Handle,
                    ref futureQuery,
                    futureBuffer,
                    1,
                    72,
                    out _,
                    56);
                Assert.That(futureStatus, Is.EqualTo(NativeSim.Bounds));
                Assert.That(session.EffectCursor, Is.EqualTo(steppedCursor));
                Assert.That(session.Tick, Is.EqualTo(1));

                var smallQuery = NativeSim.PresentationFrameQuery.CampaignSubset(
                    session.Generation,
                    session.Tick);
                var tooSmall = Enumerable.Repeat(
                    (byte)0xa5,
                    NativeSim.TerrainFramePixels - 1).ToArray();
                var smallStatus = NativeSim.mc_native_campaign_presentation_frame_copy(
                    session.Handle,
                    ref smallQuery,
                    tooSmall,
                    checked((uint)tooSmall.Length),
                    IntPtr.Zero,
                    0,
                    out _,
                    128);
                Assert.That(smallStatus, Is.EqualTo(NativeSim.BufferTooSmall));
                Assert.That(tooSmall.All(value => value == 0xa5), Is.True);
                Assert.That(session.Tick, Is.EqualTo(1));
                Assert.That(session.EffectCursor, Is.EqualTo(steppedCursor));

                var oldGeneration = session.Generation;
                session.Reset();
                Assert.That((session.Generation, session.Tick), Is.EqualTo((1ul, 0ul)));
                Assert.That(session.EffectCursor, Is.Zero);
                Assert.That(session.DrainEffects(), Is.Empty);

                var staleQuery = NativeSim.EventQuery.Create(oldGeneration, steppedCursor);
                var staleStatus = NativeSim.mc_native_campaign_events_copy_since(
                    session.Handle,
                    ref staleQuery,
                    futureBuffer,
                    1,
                    72,
                    out _,
                    56);
                Assert.That(staleStatus, Is.EqualTo(NativeSim.GenerationMismatch));
                var staleFrameQuery = NativeSim.PresentationFrameQuery.CampaignSubset(
                    oldGeneration,
                    0);
                var staleFrameStatus = NativeSim.mc_native_campaign_presentation_frame_copy(
                    session.Handle,
                    ref staleFrameQuery,
                    new byte[NativeSim.TerrainFramePixels],
                    NativeSim.TerrainFramePixels,
                    IntPtr.Zero,
                    0,
                    out _,
                    128);
                Assert.That(staleFrameStatus, Is.EqualTo(NativeSim.GenerationMismatch));

                var resetFrame = session.CopyPresentationFrame(0);
                Assert.That((resetFrame.generation, resetFrame.tick), Is.EqualTo((1ul, 0ul)));
                Assert.That(resetFrame.indexHash, Is.EqualTo(initialFrameHash));
                Assert.That(session.CampaignFrameIndices, Is.EqualTo(initialIndices));
                var retainedDay = session.CopyDayIndexedFrame(0);
                Assert.That(retainedDay.indexHash, Is.EqualTo(resetFrame.indexHash));
                Assert.That(session.DayIndexedFrameIndices, Is.EqualTo(initialIndices));
            }
            Assert.That(NativeCampaignSession.ActiveSessionCount, Is.Zero);

            var root = new GameObject("CF13 generic campaign production consumer");
            root.SetActive(false);
            MagicCarpetRuntime runtime = null;
            try
            {
                runtime = root.AddComponent<MagicCarpetRuntime>();
                runtime.QualificationInitializeLevel2Production(
                    Level2Package,
                    Level2DayCache);
                Assert.That(runtime.Level2ProductionStartupReady, Is.True);
                Assert.That(
                    runtime.Level2ProductionCampaignPackageIdentity.DefinitionSha256,
                    Is.Not.Empty);
                Assert.That(
                    runtime.Level2ProductionCampaignFrameInfo.indexHash,
                    Is.EqualTo(initialFrameHash));
                runtime.QualificationDriveLevel2Frames(1);
                Assert.That(runtime.Level2ProductionCampaignEffects, Is.Not.Empty);
                Assert.That(
                    runtime.Level2ProductionCampaignEffectCursor,
                    Is.EqualTo((ulong)runtime.Level2ProductionCampaignEffects.Length));
                Assert.That(runtime.Level2ProductionCampaignFrameInfo.tick, Is.EqualTo(1));
                runtime.QualificationResetLevel2Production();
                Assert.That(runtime.Level2ProductionCampaignEffectCursor, Is.Zero);
                Assert.That(runtime.Level2ProductionCampaignEffects, Is.Empty);
                Assert.That(
                    (runtime.Level2ProductionCampaignFrameInfo.generation,
                     runtime.Level2ProductionCampaignFrameInfo.tick,
                     runtime.Level2ProductionCampaignFrameInfo.indexHash),
                    Is.EqualTo((1ul, 0ul, initialFrameHash)));
            }
            finally
            {
                runtime?.QualificationDisposeNativeOwners();
                UnityEngine.Object.DestroyImmediate(root);
            }
            Assert.That(NativeCampaignSession.ActiveSessionCount, Is.Zero);
        }

        [Test]
        public void Level2DayMusicLifecycleIsSessionIsolated()
        {
            var root = new GameObject("Level 2 Day music production route");
            root.SetActive(false);
            MagicCarpetRuntime runtime = null;
            ulong initialHash = 0;
            try
            {
                runtime = root.AddComponent<MagicCarpetRuntime>();
                runtime.QualificationInitializeLevel2Production(
                    Level2Package,
                    Level2DayCache);

                var initial = runtime.Level2ProductionDayMusic;
                Assert.That(initial.abiVersion, Is.EqualTo(NativeSim.AbiVersion));
                Assert.That(initial.structSize, Is.EqualTo(80));
                Assert.That(initial.generation, Is.Zero);
                Assert.That(initial.tick, Is.Zero);
                Assert.That(initial.assetSlot, Is.Zero);
                Assert.That(initial.track, Is.EqualTo(2));
                Assert.That(initial.volume, Is.EqualTo(127));
                Assert.That(initial.initialized, Is.EqualTo(1));
                Assert.That(initial.loaded, Is.EqualTo(1));
                Assert.That(initial.active, Is.EqualTo(1));
                Assert.That(initial.playing, Is.EqualTo(1));
                Assert.That(initial.startCount, Is.EqualTo(1));
                Assert.That(initial.updatesEnabled, Is.EqualTo(1));
                Assert.That(initial.targetMode, Is.EqualTo(1));
                Assert.That(initial.currentUnits, Is.Zero);
                Assert.That(initial.direction, Is.EqualTo(-1));
                Assert.That(initial.timerMultiplier, Is.EqualTo(3));
                Assert.That(initial.timerHz, Is.EqualTo(90));
                Assert.That(initial.stateHash, Is.Not.Zero);
                initialHash = initial.stateHash;

                runtime.QualificationDriveLevel2Frames(1);
                var stepped = runtime.Level2ProductionDayMusic;
                Assert.That(stepped.generation, Is.Zero);
                Assert.That(stepped.tick, Is.EqualTo(1));
                Assert.That(stepped.updateCount, Is.EqualTo(1));
                Assert.That(stepped.targetMode, Is.EqualTo(1));
                Assert.That(stepped.timerActive, Is.Zero);

                runtime.QualificationResetLevel2Production();
                var reset = runtime.Level2ProductionDayMusic;
                Assert.That(reset.generation, Is.EqualTo(1));
                Assert.That(reset.tick, Is.Zero);
                Assert.That(reset.startCount, Is.EqualTo(1));
                Assert.That(reset.updateCount, Is.Zero);
                Assert.That(reset.stopCount, Is.Zero);
                Assert.That(reset.endCount, Is.Zero);
                Assert.That(reset.stateHash, Is.EqualTo(initialHash));
            }
            finally
            {
                runtime?.QualificationDisposeNativeOwners();
                UnityEngine.Object.DestroyImmediate(root);
            }
            Assert.That(NativeCampaignSession.ActiveSessionCount, Is.Zero);

            using (var session = NativeCampaignSession.OpenLevel2(
                       Level2Package,
                       Level2DayCache))
            {
                var stale = default(NativeSim.CampaignDayMusicSnapshot);
                var staleStatus = NativeSim.mc_native_campaign_day_music_state(
                    session.Handle,
                    session.Generation + 1,
                    session.Tick,
                    ref stale,
                    80);
                Assert.That(staleStatus, Is.EqualTo(NativeSim.GenerationMismatch));
                Assert.Throws<System.InvalidOperationException>(
                    () => session.AdvanceDayMusic(1_025));

                var stopped = session.StopDayMusic();
                Assert.That(stopped.stopCount, Is.EqualTo(1));
                Assert.That(stopped.playing, Is.Zero);
                Assert.That(stopped.loaded, Is.EqualTo(1));
                Assert.That(stopped.active, Is.EqualTo(1));

                session.Reset();
                var restarted = session.DayMusicState();
                Assert.That(restarted.generation, Is.EqualTo(1));
                Assert.That(restarted.startCount, Is.EqualTo(1));
                Assert.That(restarted.stopCount, Is.Zero);
                Assert.That(restarted.stateHash, Is.EqualTo(initialHash));

                var ended = session.EndDayMusic();
                Assert.That(ended.endCount, Is.EqualTo(1));
                Assert.That(ended.playing, Is.Zero);
                Assert.That(ended.loaded, Is.Zero);
                Assert.That(ended.active, Is.Zero);
            }
            Assert.That(NativeCampaignSession.ActiveSessionCount, Is.Zero);

            using var noCache = NativeCampaignSession.OpenLevel2(Level2Package);
            Assert.Throws<System.InvalidOperationException>(() => noCache.DayMusicState());
        }

        [Test]
        public void Level2IndexedWorldRendersFromCampaignFrame()
        {
            var root = new GameObject("Level 2 indexed frame production route");
            root.SetActive(false);
            MagicCarpetRuntime runtime = null;
            try
            {
                runtime = root.AddComponent<MagicCarpetRuntime>();
                runtime.QualificationInitializeLevel2Production(
                    Level2Package,
                    Level2DayCache);

                Assert.That(runtime.Level2ProductionStartupReady, Is.True);
                Assert.That(runtime.Level2ProductionIndexedFrameLive, Is.True);
                var initialObject = runtime.Level2ProductionIndexedFrameObject;
                var initialRenderer = runtime.Level2ProductionIndexedFrameRenderer;
                var initialTexture = runtime.Level2ProductionIndexedFrameTexture;
                var initialPaletteTexture =
                    runtime.Level2ProductionIndexedFramePaletteTexture;
                var initialMaterial = runtime.Level2ProductionIndexedFrameMaterial;
                var initialIndices = runtime.Level2ProductionIndexedFrameIndices;
                var initialPalette = runtime.Level2ProductionIndexedFramePalette;
                var initialInfo = runtime.Level2ProductionIndexedFrameInfo;
                Assert.That(initialObject, Is.Not.Null);
                Assert.That(initialRenderer, Is.Not.Null);
                Assert.That(initialRenderer.enabled, Is.True);
                Assert.That(initialTexture, Is.Not.Null);
                Assert.That(initialPaletteTexture, Is.Not.Null);
                Assert.That(initialMaterial, Is.Not.Null);
                Assert.That(initialMaterial.GetTexture("_Indices"), Is.SameAs(initialTexture));
                Assert.That(
                    initialMaterial.GetTexture("_Palette"),
                    Is.SameAs(initialPaletteTexture));
                Assert.That(initialTexture.width, Is.EqualTo(320));
                Assert.That(initialTexture.height, Is.EqualTo(200));
                Assert.That(initialPaletteTexture.width, Is.EqualTo(256));
                Assert.That(initialPaletteTexture.height, Is.EqualTo(1));
                Assert.That(initialInfo.generation, Is.Zero);
                Assert.That(initialInfo.tick, Is.Zero);
                Assert.That(initialInfo.indexBytes, Is.EqualTo(64000));
                Assert.That(initialInfo.paletteBytes, Is.EqualTo(768));
                Assert.That(initialInfo.indexHash, Is.Not.Zero);
                Assert.That(initialInfo.paletteHash, Is.Not.Zero);
                Assert.That(initialInfo.terrainInputHash, Is.Not.Zero);
                Assert.That(initialInfo.terrainCommandHash, Is.Not.Zero);
                Assert.That(initialIndices, Has.Length.EqualTo(64000));
                Assert.That(initialPalette, Has.Length.EqualTo(768));
                AssertTextureBytes(initialTexture, initialIndices);
                AssertTextureBytes(initialPaletteTexture, initialPalette);

                var resetIndices = (byte[])initialIndices.Clone();
                var resetPalette = (byte[])initialPalette.Clone();
                var resetInfo = initialInfo;
                var initialUploadCount =
                    runtime.Level2ProductionIndexedFrameUploadCount;

                runtime.QualificationDriveLevel2Frames(1);
                var steppedInfo = runtime.Level2ProductionIndexedFrameInfo;
                var steppedIndices = runtime.Level2ProductionIndexedFrameIndices;
                var steppedPalette = runtime.Level2ProductionIndexedFramePalette;
                var steppedIndexBytes = (byte[])steppedIndices.Clone();
                var steppedPaletteBytes = (byte[])steppedPalette.Clone();
                Assert.That(runtime.Level2ProductionIndexedFrameLive, Is.True);
                Assert.That(steppedInfo.generation, Is.EqualTo(
                    runtime.Level2ProductionTerrainState.generation));
                Assert.That(steppedInfo.tick, Is.EqualTo(
                    runtime.Level2ProductionTerrainState.tick));
                Assert.That(steppedInfo.tick, Is.GreaterThan(initialInfo.tick));
                Assert.That(steppedInfo.indexBytes, Is.EqualTo(64000));
                Assert.That(steppedInfo.paletteBytes, Is.EqualTo(768));
                Assert.That(steppedInfo.indexHash, Is.Not.Zero);
                Assert.That(steppedInfo.paletteHash, Is.Not.Zero);
                Assert.That(steppedInfo.terrainInputHash, Is.Not.Zero);
                Assert.That(steppedInfo.terrainCommandHash, Is.Not.Zero);
                Assert.That(steppedIndices, Is.SameAs(initialIndices));
                Assert.That(steppedPalette, Is.SameAs(initialPalette));
                Assert.That(
                    runtime.Level2ProductionIndexedFrameObject,
                    Is.SameAs(initialObject));
                Assert.That(
                    runtime.Level2ProductionIndexedFrameRenderer,
                    Is.SameAs(initialRenderer));
                Assert.That(
                    runtime.Level2ProductionIndexedFrameTexture,
                    Is.SameAs(initialTexture));
                Assert.That(
                    runtime.Level2ProductionIndexedFramePaletteTexture,
                    Is.SameAs(initialPaletteTexture));
                Assert.That(
                    runtime.Level2ProductionIndexedFrameMaterial,
                    Is.SameAs(initialMaterial));
                Assert.That(
                    runtime.Level2ProductionIndexedFrameUploadCount,
                    Is.EqualTo(initialUploadCount + 1));
                AssertTextureBytes(initialTexture, steppedIndices);
                AssertTextureBytes(initialPaletteTexture, steppedPalette);

                using (var staleSession = NativeCampaignSession.OpenLevel2(
                    Level2Package,
                    Level2DayCache))
                {
                    var staleFrame = staleSession.DayIndexedFrame(0);
                    var staleIndices = (byte[])staleFrame.indices.Clone();
                    var stalePalette = (byte[])staleFrame.palette.Clone();
                    var staleStatus =
                        NativeSim.mc_native_campaign_day_indexed_frame_copy(
                            staleSession.Handle,
                            staleSession.Generation + 1,
                            0,
                            staleSession.DayIndexedFrameIndices,
                            64000,
                            staleSession.DayIndexedFramePalette,
                            768,
                            out _,
                            out _);
                    Assert.That(
                        staleStatus,
                        Is.EqualTo(NativeSim.GenerationMismatch));
                    CollectionAssert.AreEqual(
                        staleIndices,
                        staleSession.DayIndexedFrameIndices,
                        "stale generation leaves the reusable index buffer untouched");
                    CollectionAssert.AreEqual(
                        stalePalette,
                        staleSession.DayIndexedFramePalette,
                        "stale generation leaves the reusable palette buffer untouched");
                    var staleTickStatus =
                        NativeSim.mc_native_campaign_day_indexed_frame_copy(
                            staleSession.Handle,
                            staleSession.Generation,
                            1,
                            staleSession.DayIndexedFrameIndices,
                            64000,
                            staleSession.DayIndexedFramePalette,
                            768,
                            out _,
                            out _);
                    Assert.That(
                        staleTickStatus,
                        Is.EqualTo(NativeSim.GenerationMismatch));
                    CollectionAssert.AreEqual(
                        staleIndices,
                        staleSession.DayIndexedFrameIndices,
                        "stale tick leaves the reusable index buffer untouched");
                    CollectionAssert.AreEqual(
                        stalePalette,
                        staleSession.DayIndexedFramePalette,
                        "stale tick leaves the reusable palette buffer untouched");
                }

                runtime.QualificationResetLevel2Production();
                Assert.That(initialObject == null, Is.True);
                Assert.That(initialRenderer == null, Is.True);
                Assert.That(runtime.Level2ProductionIndexedFrameLive, Is.True);
                var reboundObject = runtime.Level2ProductionIndexedFrameObject;
                var reboundRenderer = runtime.Level2ProductionIndexedFrameRenderer;
                var reboundInfo = runtime.Level2ProductionIndexedFrameInfo;
                Assert.That(reboundObject, Is.Not.Null);
                Assert.That(reboundRenderer, Is.Not.Null);
                Assert.That(reboundObject, Is.Not.SameAs(initialObject));
                Assert.That(reboundRenderer, Is.Not.SameAs(initialRenderer));
                Assert.That(reboundInfo.generation, Is.EqualTo(1));
                Assert.That(reboundInfo.tick, Is.Zero);
                Assert.That(reboundInfo.indexHash, Is.EqualTo(resetInfo.indexHash));
                Assert.That(
                    reboundInfo.paletteHash,
                    Is.EqualTo(resetInfo.paletteHash));
                Assert.That(
                    reboundInfo.terrainInputHash,
                    Is.EqualTo(resetInfo.terrainInputHash));
                Assert.That(
                    reboundInfo.terrainCommandHash,
                    Is.EqualTo(resetInfo.terrainCommandHash));
                Assert.That(
                    runtime.Level2ProductionIndexedFrameIndices,
                    Is.SameAs(initialIndices));
                Assert.That(
                    runtime.Level2ProductionIndexedFramePalette,
                    Is.SameAs(initialPalette));
                CollectionAssert.AreEqual(
                    resetIndices,
                    runtime.Level2ProductionIndexedFrameIndices);
                CollectionAssert.AreEqual(
                    resetPalette,
                    runtime.Level2ProductionIndexedFramePalette);
                AssertTextureBytes(
                    runtime.Level2ProductionIndexedFrameTexture,
                    runtime.Level2ProductionIndexedFrameIndices);
                AssertTextureBytes(
                    runtime.Level2ProductionIndexedFramePaletteTexture,
                    runtime.Level2ProductionIndexedFramePalette);

                runtime.QualificationDriveLevel2Frames(1);
                var replayInfo = runtime.Level2ProductionIndexedFrameInfo;
                Assert.That(replayInfo.generation, Is.EqualTo(1));
                Assert.That(replayInfo.tick, Is.EqualTo(steppedInfo.tick));
                Assert.That(replayInfo.indexHash, Is.EqualTo(steppedInfo.indexHash));
                Assert.That(
                    replayInfo.paletteHash,
                    Is.EqualTo(steppedInfo.paletteHash));
                CollectionAssert.AreEqual(
                    steppedIndexBytes,
                    runtime.Level2ProductionIndexedFrameIndices);
                CollectionAssert.AreEqual(
                    steppedPaletteBytes,
                    runtime.Level2ProductionIndexedFramePalette);

                runtime.QualificationDisposeNativeOwners();
                Assert.That(runtime.Level2ProductionIndexedFrameObject, Is.Null);
                Assert.That(runtime.Level2ProductionIndexedFrameRenderer, Is.Null);
                Assert.That(
                    root.GetComponentsInChildren<Renderer>(true),
                    Is.Empty,
                    "teardown removes the visible Level 2 world frame");
            }
            finally
            {
                runtime?.QualificationDisposeNativeOwners();
                UnityEngine.Object.DestroyImmediate(root);
            }

            Assert.That(NativeCampaignSession.ActiveSessionCount, Is.Zero);
        }

        private static void AssertTextureBytes(Texture2D texture, byte[] expected)
        {
            var observed = texture.GetRawTextureData<byte>();
            Assert.That(observed.Length, Is.EqualTo(expected.Length));
            for (var index = 0; index < observed.Length; index++)
            {
                Assert.That(observed[index], Is.EqualTo(expected[index]));
            }
        }

        [Test]
        public void GenericCampaignPackageRejectsMissingManifestMemberWithoutHandleLeak()
        {
            var temporaryPackage = Path.Combine(
                Path.GetTempPath(),
                $"magiccarpet-level2-package-{Guid.NewGuid():N}");
            try
            {
                Directory.CreateDirectory(temporaryPackage);
                foreach (var source in Directory.GetFiles(Level2Package))
                {
                    File.Copy(
                        source,
                        Path.Combine(temporaryPackage, Path.GetFileName(source)));
                }
                File.Delete(Path.Combine(temporaryPackage, "collision-masks.bin"));

                var error = Assert.Throws<InvalidOperationException>(
                    () => NativeCampaignSession.Open(
                        temporaryPackage,
                        NativeSim.CampaignSelection.Level2Day()));
                StringAssert.Contains("campaign preflight stage 2", error.Message);
                StringAssert.Contains("exact member set", error.Message);
                Assert.That(NativeCampaignSession.ActiveSessionCount, Is.Zero);
            }
            finally
            {
                if (Directory.Exists(temporaryPackage))
                {
                    Directory.Delete(temporaryPackage, true);
                }
            }
        }

        [Test]
        public void GenericCampaignRejectsMutatedDefinitionAndSelectionMismatchWithoutLeaks()
        {
            var temporaryPackage = Path.Combine(
                Path.GetTempPath(),
                $"magiccarpet-campaign-definition-{Guid.NewGuid():N}");
            try
            {
                Directory.CreateDirectory(temporaryPackage);
                foreach (var source in Directory.GetFiles(Level2Package))
                {
                    File.Copy(
                        source,
                        Path.Combine(temporaryPackage, Path.GetFileName(source)));
                }
                File.AppendAllText(
                    Path.Combine(temporaryPackage, "campaign-definition.json"),
                    "\n");

                var mutation = Assert.Throws<InvalidOperationException>(
                    () => NativeCampaignSession.Open(
                        temporaryPackage,
                        NativeSim.CampaignSelection.Level2Day()));
                StringAssert.Contains("campaign preflight stage 3", mutation.Message);
                StringAssert.Contains("campaign-definition.json", mutation.Message);
                Assert.That(NativeCampaignSession.ActiveSessionCount, Is.Zero);

                var mismatch = Assert.Throws<InvalidOperationException>(
                    () => NativeCampaignSession.Open(
                        Level2Package,
                        NativeSim.CampaignSelection.Create(2, 1, 1)));
                StringAssert.Contains("campaign preflight stage 1", mismatch.Message);
                StringAssert.Contains("caller/outer selection", mismatch.Message);
                Assert.That(NativeCampaignSession.ActiveSessionCount, Is.Zero);
            }
            finally
            {
                if (Directory.Exists(temporaryPackage))
                {
                    Directory.Delete(temporaryPackage, true);
                }
            }
        }

        [Test]
        public void Cf14GenericLevel3OpensStepsDrainsAndResetsWithoutLeaks()
        {
            var selection = NativeSim.CampaignSelection.Create(3, 2, 0);
            ulong firstStateHash;
            ulong firstTerrainHash;
            byte[] firstDispatchHash;
            NativeSim.CampaignEvent[] firstEvents;
            NativeSim.NativeEvent[] firstEffects;
            byte[] initialIndices;
            ulong initialFrameHash;
            byte[] steppedIndices;
            ulong steppedFrameHash;
            using (var session = NativeCampaignSession.Open(
                       Level3Package,
                       selection,
                       Level3DayCache))
            {
                Assert.That(NativeCampaignSession.ActiveSessionCount, Is.EqualTo(1));
                var identity = session.PackageIdentity;
                Assert.That(identity.PackagePath, Is.EqualTo(Path.GetFullPath(Level3Package)));
                Assert.That(identity.ManifestSha256.Length, Is.EqualTo(64));
                Assert.That(identity.DefinitionSha256.Length, Is.EqualTo(64));
                Assert.That(
                    (identity.Selection.externalLevel,
                     identity.Selection.internalLevel,
                     identity.Selection.mapType),
                    Is.EqualTo(((ushort)3, (ushort)2, (byte)0)));

                var initial = session.InitialWorldInfo;
                Assert.That(
                    (initial.externalLevel,
                     initial.internalLevel,
                     initial.mapType,
                     initial.entityCount,
                     initial.activeEntityCount,
                     initial.playerCount),
                    Is.EqualTo(((ushort)3, (ushort)2, (byte)0, 1000u, 252u, 8u)));
                Assert.That(initial.resetStateHash, Is.Not.Zero);
                Assert.That(session.DrainEffects(), Is.Empty);
                Assert.That(session.EffectCursor, Is.Zero);
                Assert.That(
                    File.Exists(Path.Combine(
                        Level3DayCache,
                        "components/audio/manifest.json")),
                    Is.False,
                    "the neutral Level 3 cache is an honest material-only component");
                Assert.Throws<InvalidOperationException>(() => session.DayMusicState());

                var initialFrame = session.CopyPresentationFrame(0);
                initialIndices = session.CampaignFrameIndices.ToArray();
                initialFrameHash = initialFrame.indexHash;
                Assert.That(initialFrame.newlyCommitted, Is.EqualTo(1));
                Assert.That(initialFrame.provenanceBytes, Is.Zero);
                var repeatedFrame = session.CopyPresentationFrame(0);
                Assert.That(repeatedFrame.newlyCommitted, Is.Zero);
                Assert.That(repeatedFrame.indexHash, Is.EqualTo(initialFrameHash));
                Assert.That(session.CampaignFrameIndices, Is.EqualTo(initialIndices));

                var localPlayer = session.Players()
                    .Single(player =>
                        player.slot == 0 && player.active != 0 && player.isAi == 0);
                var localEntity = session.Entities()
                    .Single(entity => entity.entityIndex == localPlayer.entityIndex);
                Assert.That(localEntity.active, Is.EqualTo(1));
                var first = session.Step(localEntity.x, localEntity.y, localEntity.z);
                firstStateHash = first.snapshot.stateHash;
                firstTerrainHash = first.snapshot.terrainHash;
                firstDispatchHash = first.snapshot.dispatchHash.ToArray();
                firstEvents = first.events;
                firstEffects = session.DrainEffects();

                Assert.That(first.snapshot.revision, Is.EqualTo(1));
                Assert.That(first.snapshot.stateHash, Is.Not.EqualTo(initial.resetStateHash));
                Assert.That(first.events, Is.Not.Empty);
                Assert.That(firstEffects, Is.Not.Empty);
                Assert.That(
                    firstEffects.Select(effect => effect.sequence),
                    Is.EqualTo(Enumerable.Range(1, firstEffects.Length)
                        .Select(index => (ulong)index)));
                Assert.That(
                    firstEffects.All(effect =>
                        effect.generation == 0 &&
                        effect.stateHash == first.snapshot.stateHash &&
                        new[]
                        {
                            NativeSim.CampaignEffectDispatch,
                            NativeSim.CampaignEffectSharedSpell,
                            NativeSim.CampaignEffectFlight,
                            NativeSim.CampaignEffectSceneChange
                        }.Contains(effect.kind)),
                    Is.True);
                var firstCursor = session.EffectCursor;
                Assert.That(firstCursor, Is.EqualTo((ulong)firstEffects.Length));
                Assert.That(session.DrainEffects(), Is.Empty);
                Assert.That(session.EffectCursor, Is.EqualTo(firstCursor));
                var steppedFrame = session.CopyPresentationFrame(1);
                steppedIndices = session.CampaignFrameIndices.ToArray();
                steppedFrameHash = steppedFrame.indexHash;
                Assert.That((steppedFrame.generation, steppedFrame.tick), Is.EqualTo((0ul, 1ul)));

                session.Reset();
                Assert.That((session.Generation, session.Tick), Is.EqualTo((1ul, 0ul)));
                Assert.That(session.EffectCursor, Is.Zero);
                Assert.That(session.DrainEffects(), Is.Empty);
                Assert.That(
                    session.WorldInfo().resetStateHash,
                    Is.EqualTo(initial.resetStateHash));
                var resetFrame = session.CopyPresentationFrame(0);
                Assert.That((resetFrame.generation, resetFrame.tick), Is.EqualTo((1ul, 0ul)));
                Assert.That(resetFrame.indexHash, Is.EqualTo(initialFrameHash));
                Assert.That(session.CampaignFrameIndices, Is.EqualTo(initialIndices));

                var resetPlayer = session.Players()
                    .Single(player => player.slot == 0 && player.active != 0);
                var resetEntity = session.Entities()
                    .Single(entity => entity.entityIndex == resetPlayer.entityIndex);
                var replay = session.Step(resetEntity.x, resetEntity.y, resetEntity.z);
                var replayEffects = session.DrainEffects();
                Assert.That(replay.snapshot.generation, Is.EqualTo(1));
                Assert.That(replay.snapshot.stateHash, Is.EqualTo(firstStateHash));
                Assert.That(replay.snapshot.terrainHash, Is.EqualTo(firstTerrainHash));
                Assert.That(replay.snapshot.dispatchHash, Is.EqualTo(firstDispatchHash));
                Assert.That(
                    replay.events.Select(row =>
                        (row.ordinal, row.pass, row.kind, row.family,
                         row.flags, row.sourceSlot)),
                    Is.EqualTo(firstEvents.Select(row =>
                        (row.ordinal, row.pass, row.kind, row.family,
                         row.flags, row.sourceSlot))));
                Assert.That(replayEffects.All(row => row.generation == 1), Is.True);
                Assert.That(
                    replayEffects.Select(row =>
                        (row.sequence, row.stateHash, row.kind, row.code,
                         row.entityIndex, row.playerIndex, row.value0, row.value1)),
                    Is.EqualTo(firstEffects.Select(row =>
                        (row.sequence, row.stateHash, row.kind, row.code,
                         row.entityIndex, row.playerIndex, row.value0, row.value1))));
                var replayFrame = session.CopyPresentationFrame(1);
                Assert.That((replayFrame.generation, replayFrame.tick), Is.EqualTo((1ul, 1ul)));
                Assert.That(replayFrame.indexHash, Is.EqualTo(steppedFrameHash));
                Assert.That(session.CampaignFrameIndices, Is.EqualTo(steppedIndices));
            }
            Assert.That(NativeCampaignSession.ActiveSessionCount, Is.Zero);

            // A mismatched caller triple against the same package fails at the
            // outer manifest stage before behavior closure.
            NativeCampaignSession unexpectedSession = null;
            try
            {
                var mismatch = Assert.Throws<InvalidOperationException>(() =>
                {
                    unexpectedSession = NativeCampaignSession.Open(
                        Level3Package,
                        NativeSim.CampaignSelection.Level2Day());
                });
                StringAssert.Contains("campaign preflight stage 1", mismatch.Message);
                StringAssert.Contains("caller/outer selection", mismatch.Message);
            }
            finally
            {
                unexpectedSession?.Dispose();
            }
            Assert.That(NativeCampaignSession.ActiveSessionCount, Is.Zero);

            NativeCampaignSession crossBoundSession = null;
            try
            {
                Assert.Throws<InvalidOperationException>(() =>
                {
                    crossBoundSession = NativeCampaignSession.Open(
                        Level3Package,
                        selection,
                        Level2DayCache);
                });
            }
            finally
            {
                crossBoundSession?.Dispose();
            }
            Assert.That(NativeCampaignSession.ActiveSessionCount, Is.Zero);
        }

        [Test]
        public void Level3OwnerCachePresentsLiveWorldAndEntitiesWithExactInterfaceSurfaces()
        {
            var wrongCache = Assert.Throws<InvalidDataException>(
                () => OwnerAssetCache.OpenAndValidateLevel3(Level2DayCache));
            StringAssert.Contains("Level 3", wrongCache.Message);

            var tamperedRoot = Path.Combine(
                Path.GetTempPath(),
                $"magiccarpet-level3-cache-{Guid.NewGuid():N}");
            Directory.CreateDirectory(tamperedRoot);
            foreach (var source in Directory.GetFiles(
                         Level3OwnerCache,
                         "*",
                         SearchOption.AllDirectories))
            {
                var relative = source.Substring(Level3OwnerCache.Length)
                    .TrimStart(Path.DirectorySeparatorChar);
                var destination = Path.Combine(tamperedRoot, relative);
                Directory.CreateDirectory(Path.GetDirectoryName(destination));
                File.Copy(source, destination);
            }
            File.AppendAllText(
                Path.Combine(
                    tamperedRoot,
                    "components/gameplay/gameplay/descriptor.json"),
                "\n");
            try
            {
                var tampered = Assert.Throws<InvalidDataException>(
                    () => OwnerAssetCache.OpenAndValidateLevel3(tamperedRoot));
                StringAssert.Contains("output size", tampered.Message);
            }
            finally
            {
                Directory.Delete(tamperedRoot, true);
            }

            var cache = OwnerAssetCache.OpenAndValidateLevel3(Level3OwnerCache);
            Assert.That(cache.IsLevel3Aggregate, Is.True);
            Assert.That(cache.OutputCount, Is.EqualTo(644));
            Assert.That(cache.DayHud.bitmap_assets.Length, Is.EqualTo(34));
            Assert.That(cache.DayHud.spell_tier_text.tiers.Length, Is.EqualTo(5));
            Assert.That(cache.Level3Materials.tmaps0.textures.Length, Is.EqualTo(156));

            var root = new GameObject("Level 3 presentation qualification");
            Level3PresentationSurface surface = null;
            try
            {
                using (var session = NativeCampaignSession.Open(
                           Level3Package,
                           NativeSim.CampaignSelection.Create(3, 2, 0),
                           Level3DayCache))
                {
                    surface = new Level3PresentationSurface(root.transform);
                    surface.Present(session, cache);
                    Assert.That((surface.Generation, surface.Tick), Is.EqualTo((0ul, 0ul)));
                    Assert.That(surface.WorldReady, Is.True);
                    Assert.That(surface.FrameHash, Is.Not.Zero);
                    Assert.That(surface.TerrainHash, Is.Not.Zero);
                    Assert.That(surface.EntitySurfaceCount, Is.EqualTo(252));
                    Assert.That(surface.RenderedEntitySurfaceCount, Is.GreaterThan(0));
                    Assert.That(surface.HudReady, Is.True);
                    Assert.That(surface.HudBitmapSurfaceCount, Is.EqualTo(34));
                    Assert.That(surface.SpellTierSurfaceCount, Is.EqualTo(5));
                    Assert.That(surface.MinimapPanelReady, Is.True);
                    Assert.That(
                        surface.LiveMinimapReady,
                        Is.True);
                    Assert.That(
                        surface.CacheContentHash,
                        Is.EqualTo(
                            "74008ec454d0ddbba8cef39a15402f72273e35de9eadedbe4a61ee3108d6569a"));

                    var localPlayer = session.Players()
                        .Single(player => player.slot == 0 && player.active != 0);
                    var localEntity = session.Entities()
                        .Single(entity => entity.entityIndex == localPlayer.entityIndex);
                    session.Step(localEntity.x, localEntity.y, localEntity.z);
                    surface.Present(session, cache);
                    Assert.That((surface.Generation, surface.Tick), Is.EqualTo((0ul, 1ul)));
                    Assert.That(
                        surface.Root.GetComponentsInChildren<Level3EntitySurfaceMarker>()
                            .All(marker =>
                                marker.Generation == session.Generation &&
                                marker.Tick == session.Tick),
                        Is.True);
                    Assert.That(
                        surface.Root.GetComponentsInChildren<Level3EntitySurfaceMarker>()
                            .Where(marker => marker.SpriteReady)
                            .All(marker => cache.Level3Materials.tmaps0.selectors
                                .Contains(marker.AssetSelector)),
                        Is.True);
                }
                Assert.That(NativeCampaignSession.ActiveSessionCount, Is.Zero);
            }
            finally
            {
                surface?.Dispose();
                UnityEngine.Object.DestroyImmediate(root);
            }

            Assert.That(NativeCampaignSession.ActiveSessionCount, Is.Zero);
            Assert.That(
                UnityEngine.Object.FindObjectsByType<Level3EntitySurfaceMarker>(
                    FindObjectsInactive.Include,
                    FindObjectsSortMode.None),
                Is.Empty);
        }

        [Test]
        public void GenericCampaignConsumerOpensStepsAndResetsLevel1Descriptor()
        {
            var selection = NativeSim.CampaignSelection.Create(1, 0, 1);
            ulong firstStateHash;
            ulong firstTerrainHash;
            byte[] firstDispatchHash;
            NativeSim.CampaignEvent[] firstEvents;
            NativeSim.NativeEvent[] firstEffects;

            using (var session = NativeCampaignSession.Open(
                       Level1CampaignPackage,
                       selection))
            {
                var identity = session.PackageIdentity;
                Assert.That(
                    identity.PackagePath,
                    Is.EqualTo(Path.GetFullPath(Level1CampaignPackage)));
                Assert.That(
                    (identity.Selection.externalLevel,
                     identity.Selection.internalLevel,
                     identity.Selection.mapType),
                    Is.EqualTo(((ushort)1, (ushort)0, (byte)1)));

                var initial = session.InitialWorldInfo;
                Assert.That(
                    (initial.externalLevel,
                     initial.internalLevel,
                     initial.mapType,
                     initial.entityCount,
                     initial.activeEntityCount,
                     initial.playerCount,
                     initial.objectiveCount),
                    Is.EqualTo(((ushort)1, (ushort)0, (byte)1, 1000u, 177u, 1u, 5u)));
                Assert.That(initial.resetStateHash, Is.Not.Zero);
                Assert.That(session.DrainEffects(), Is.Empty);

                var localPlayer = session.Players()
                    .Single(player =>
                        player.slot == 0 && player.active != 0 && player.isAi == 0);
                var localEntity = session.Entities()
                    .Single(entity => entity.entityIndex == localPlayer.entityIndex);
                var first = session.Step(localEntity.x, localEntity.y, localEntity.z);
                firstStateHash = first.snapshot.stateHash;
                firstTerrainHash = first.snapshot.terrainHash;
                firstDispatchHash = first.snapshot.dispatchHash.ToArray();
                firstEvents = first.events;
                firstEffects = session.DrainEffects();

                Assert.That(first.snapshot.revision, Is.EqualTo(1));
                Assert.That(first.snapshot.stateHash, Is.Not.EqualTo(initial.resetStateHash));
                Assert.That(first.events, Is.Not.Empty);
                Assert.That(firstEffects, Is.Not.Empty);

                session.Reset();
                Assert.That((session.Generation, session.Tick), Is.EqualTo((1ul, 0ul)));
                Assert.That(session.EffectCursor, Is.Zero);
                Assert.That(session.DrainEffects(), Is.Empty);
                Assert.That(
                    session.WorldInfo().resetStateHash,
                    Is.EqualTo(initial.resetStateHash));

                var resetPlayer = session.Players()
                    .Single(player => player.slot == 0 && player.active != 0);
                var resetEntity = session.Entities()
                    .Single(entity => entity.entityIndex == resetPlayer.entityIndex);
                var replay = session.Step(resetEntity.x, resetEntity.y, resetEntity.z);
                var replayEffects = session.DrainEffects();
                Assert.That(replay.snapshot.generation, Is.EqualTo(1));
                Assert.That(replay.snapshot.stateHash, Is.EqualTo(firstStateHash));
                Assert.That(replay.snapshot.terrainHash, Is.EqualTo(firstTerrainHash));
                Assert.That(replay.snapshot.dispatchHash, Is.EqualTo(firstDispatchHash));
                Assert.That(
                    replay.events.Select(row =>
                        (row.ordinal, row.pass, row.kind, row.family,
                         row.flags, row.sourceSlot)),
                    Is.EqualTo(firstEvents.Select(row =>
                        (row.ordinal, row.pass, row.kind, row.family,
                         row.flags, row.sourceSlot))));
                Assert.That(
                    replayEffects.Select(row =>
                        (row.sequence, row.stateHash, row.kind, row.code,
                         row.entityIndex, row.playerIndex, row.value0, row.value1)),
                    Is.EqualTo(firstEffects.Select(row =>
                        (row.sequence, row.stateHash, row.kind, row.code,
                         row.entityIndex, row.playerIndex, row.value0, row.value1))));
            }

            Assert.That(NativeCampaignSession.ActiveSessionCount, Is.Zero);
        }

        [Test]
        public void Level2ProductionStartupRejectsCrossLevelInputsBeforeRendering()
        {
            var root = new GameObject("Level 2 cross-level startup");
            root.SetActive(false);
            MagicCarpetRuntime runtime = null;
            try
            {
                runtime = root.AddComponent<MagicCarpetRuntime>();
                Assert.Throws<InvalidDataException>(
                    () => runtime.QualificationInitializeLevel2Production(
                        Level1Package,
                        Level2DayCache));
                Assert.That(runtime.Level2ProductionStartupReady, Is.False);
                Assert.That(runtime.Level2ProductionFatalError, Is.Not.Null.And.Not.Empty);
                StringAssert.Contains(
                    "campaign-definition.json",
                    runtime.Level2ProductionFatalError);
                Assert.That(NativeSession.ActiveSessionCount, Is.Zero);
                Assert.That(NativeCampaignSession.ActiveSessionCount, Is.Zero);
                Assert.That(
                    root.GetComponentsInChildren<Renderer>(true),
                    Is.Empty,
                    "cross-level input fails before Unity presentation setup");
            }
            finally
            {
                runtime?.QualificationDisposeNativeOwners();
                UnityEngine.Object.DestroyImmediate(root);
            }
        }

        [Test]
        public void Level2ProductionFixedUpdateAdvancesDedicatedCampaignRoute()
        {
            var root = new GameObject("Level 2 fixed-step route");
            root.SetActive(false);
            MagicCarpetRuntime runtime = null;
            try
            {
                runtime = root.AddComponent<MagicCarpetRuntime>();
                runtime.QualificationInitializeLevel2Production(
                    Level2Package,
                    Level2DayCache);

                runtime.QualificationClearLevel2PresentationOrder();
                runtime.QualificationDriveLevel2Frames(1);
                Assert.That(
                    runtime.Level2ProductionPresentationOrder,
                    Is.EqualTo(new[] { "audio", "indexed-world", "hud", "spells", "minimap" }),
                    "ordinary Level 2 fixed update preserves presenter order");
                var first = runtime.Level2ProductionStepSnapshot;
                var firstInput = runtime.Level2ProductionLastStepInput;
                var firstAuthoredGroupEvents = runtime.Level2ProductionAuthoredGroupEvents;
                Assert.That(first.abiVersion, Is.EqualTo(NativeSim.AbiVersion));
                Assert.That(first.structSize, Is.EqualTo(112));
                Assert.That(first.generation, Is.Zero);
                Assert.That(first.revision, Is.GreaterThan(0));
                Assert.That(first.eventCount, Is.GreaterThan(0));
                Assert.That(
                    runtime.Level2ProductionStepEvents,
                    Has.Length.EqualTo((int)first.eventCount));
                Assert.That(runtime.Level2ProductionAuthoredGroupEvents, Is.Not.Empty);
                Assert.That(
                    runtime.Level2ProductionAuthoredGroupEvents.All(eventRow =>
                        eventRow.abiVersion == NativeSim.AbiVersion &&
                        eventRow.generation == first.generation &&
                        eventRow.ordinal < (uint)runtime.Level2ProductionStepEvents.Length &&
                        runtime.Level2ProductionStepEvents[(int)eventRow.ordinal].kind ==
                            NativeSim.CampaignEventAuthoredGroup),
                    Is.True);
                Assert.That(
                    runtime.Level2ProductionTerrainState.tick,
                    Is.EqualTo(first.revision));
                Assert.That(NativeSession.ActiveSessionCount, Is.Zero);

                runtime.QualificationDriveLevel2Frames(1);
                var second = runtime.Level2ProductionStepSnapshot;
                var secondInput = runtime.Level2ProductionLastStepInput;
                Assert.That(second.generation, Is.Zero);
                Assert.That(second.revision, Is.GreaterThan(first.revision));
                Assert.That(second.eventCount, Is.GreaterThan(0));
                Assert.That(
                    (secondInput.x, secondInput.y, secondInput.z),
                    Is.EqualTo((first.x, first.y, first.z)),
                    "each Level 2 fixed step reuses the prior authoritative pose");
                Assert.That(
                    runtime.Level2ProductionTerrainState.generation,
                    Is.Zero);
                var secondAuthoredGroupEvents = runtime.Level2ProductionAuthoredGroupEvents;
                Assert.That(secondAuthoredGroupEvents, Is.Not.SameAs(firstAuthoredGroupEvents));
                Assert.That(
                    secondAuthoredGroupEvents,
                    Is.Empty,
                    "the runtime cache reflects the second turn rather than retaining first-turn sidecars");
                Assert.That(
                    runtime.Level2ProductionStepEvents.Any(eventRow =>
                        eventRow.kind == NativeSim.CampaignEventAuthoredGroup),
                    Is.False);

                runtime.QualificationClearLevel2PresentationOrder();
                runtime.QualificationDriveLevel2Frames(1);
                Assert.That(
                    runtime.Level2ProductionPresentationOrder,
                    Is.EqualTo(new[] { "audio", "indexed-world", "hud", "spells", "minimap" }),
                    "following Level 2 fixed update uses the same fixed order");
            }
            finally
            {
                if (runtime != null)
                {
                    runtime.QualificationDisposeNativeOwners();
                    Assert.That(runtime.Level2ProductionAuthoredGroupEvents, Is.Empty);
                }
                UnityEngine.Object.DestroyImmediate(root);
            }

            Assert.That(NativeSession.ActiveSessionCount, Is.Zero);
            Assert.That(NativeCampaignSession.ActiveSessionCount, Is.Zero);
        }

        [Test]
        public void Level2IndexedPresenterRejectsStaleAuthoritativeTick()
        {
            var root = new GameObject("Level 2 stale indexed presenter route");
            root.SetActive(false);
            MagicCarpetRuntime runtime = null;
            try
            {
                runtime = root.AddComponent<MagicCarpetRuntime>();
                runtime.QualificationInitializeLevel2Production(
                    Level2Package,
                    Level2DayCache);

                runtime.QualificationDriveLevel2Frames(1);
                Assert.That(
                    runtime.QualificationRejectsStaleLevel2IndexedFrame(),
                    Is.True,
                    "stale indexed-frame input fails closed before texture upload");
                Assert.That(runtime.Level2ProductionIndexedFrameLive, Is.False);
            }
            finally
            {
                runtime?.QualificationDisposeNativeOwners();
                UnityEngine.Object.DestroyImmediate(root);
            }

            Assert.That(NativeCampaignSession.ActiveSessionCount, Is.Zero);
        }

        [Test]
        public void Level2StatusHudAndSpellHandsFollowRust()
        {
            var root = new GameObject("Level 2 Day HUD route");
            root.SetActive(false);
            MagicCarpetRuntime runtime = null;
            try
            {
                runtime = root.AddComponent<MagicCarpetRuntime>();
                runtime.QualificationInitializeLevel2Production(
                    Level2Package,
                    Level2DayCache);

                var initial = runtime.Level2ProductionDayHud;
                Assert.That(runtime.Level2ProductionDayHudPresenterReady, Is.True);
                Assert.That(initial.nyphur.visibilityFlags, Is.EqualTo(2u));
                AssertDayHudStartingSpell(
                    initial.left,
                    initial.localPlayerEntityIndex,
                    0,
                    115,
                    5);
                AssertDayHudStartingSpell(
                    initial.right,
                    initial.localPlayerEntityIndex,
                    1,
                    116,
                    3);
                Assert.That(
                    runtime.QualificationHasMaterializedLevel2DayHudSprite(1),
                    Is.True);

                runtime.QualificationDriveLevel2Frames(1);
                Assert.That(
                    runtime.Level2ProductionLastStepInput.x,
                    Is.EqualTo((ushort)384));
                Assert.That(
                    runtime.Level2ProductionLastStepInput.y,
                    Is.EqualTo((ushort)62080));
                Assert.That(
                    runtime.Level2ProductionLastStepInput.z,
                    Is.EqualTo((short)256));
                var hud = DriveUntilRustOwnedFamily12Hud(runtime);
                Assert.That(hud.abiVersion, Is.EqualTo(NativeSim.AbiVersion));
                Assert.That(hud.structSize, Is.EqualTo(272));
                Assert.That(
                    (hud.generation, hud.tick),
                    Is.EqualTo((
                        runtime.Level2ProductionTerrainState.generation,
                        runtime.Level2ProductionTerrainState.tick)));
                Assert.That(hud.localPlayerSlot, Is.Zero);
                Assert.That(hud.localPlayerEntityIndex, Is.GreaterThan(0));
                Assert.That(hud.localPlayerMaxLife, Is.GreaterThan(0));
                Assert.That(hud.localPlayerMaxMana, Is.GreaterThan(0));
                Assert.That(hud.globalManaDenominator, Is.GreaterThan(0));
                Assert.That(hud.minimapPanelSelector, Is.EqualTo(40));
                Assert.That(hud.castlePanelSelector, Is.EqualTo(54));
                Assert.That(hud.balloonPanelSelector, Is.EqualTo(54));
                Assert.That(hud.playerPanelSelector, Is.EqualTo(41));
                Assert.That(hud.playerIconSelector, Is.EqualTo(43));
                Assert.That(hud.lifeManaIconSelector, Is.EqualTo(42));
                Assert.That(hud.lifePaletteIndex, Is.EqualTo(0x7B));
                Assert.That(hud.manaPaletteIndex, Is.EqualTo(0x60));
                Assert.That(hud.manaMaxPaletteIndex, Is.EqualTo(0x64));
                Assert.That(hud.nyphur.entityIndex, Is.EqualTo(112));
                Assert.That(hud.nyphur.visibilityFlags, Is.EqualTo(6u));
                var acquiredStateHash = runtime.Level2ProductionStepSnapshot.stateHash;
                var acquiredNyphurVisibilityFlags = hud.nyphur.visibilityFlags;
                Assert.That(runtime.Level2ProductionDayHudPresenterReady, Is.True);
                AssertDayHudFamily12(hud.left, hud.localPlayerEntityIndex);
                AssertDayHudStartingSpell(
                    hud.right,
                    hud.localPlayerEntityIndex,
                    1,
                    116,
                    3);
                Assert.That(
                    runtime.QualificationHasMaterializedLevel2DayHudSprite(
                        hud.left.frameSelector),
                    Is.True);
                Assert.That(
                    runtime.QualificationHasMaterializedLevel2DayHudSprite(
                        hud.left.iconSelector),
                    Is.True);
                Assert.That(
                    runtime.QualificationSelectedLevel2DayHudRomanTierTexture(
                        hud.left.levelTextSelector),
                    Does.Contain("Roman tier I"));
                Assert.That(
                    runtime.QualificationTryDrawLevel2DayHudSprite(999),
                    Is.False);

                using (var boundarySession = NativeCampaignSession.OpenLevel2(Level2Package))
                {
                    var terrain = boundarySession.TerrainState();
                    var oldGeneration = boundarySession.Generation;
                    var staleSentinel = new NativeSim.CampaignDayHudSnapshot
                    {
                        abiVersion = 0xA1B2_C3D4u,
                        generation = 0x1122_3344_5566_7788UL,
                        tick = 0x8877_6655_4433_2211UL,
                        globalManaDenominator = 0x1357_9BDF,
                        right = new NativeSim.CampaignDayHudSpellSlot
                        {
                            holderEntityIndex = 0x4567,
                            rawSpellSelector = -1234,
                            casterMana = 0x0246_8ACE
                        }
                    };
                    var rejectedGeneration = staleSentinel;
                    var status = NativeSim.mc_native_campaign_day_hud_state(
                        boundarySession.Handle,
                        oldGeneration + 1,
                        terrain.tick,
                        ref rejectedGeneration,
                        272);
                    Assert.That(status, Is.EqualTo(NativeSim.GenerationMismatch));
                    Assert.That(rejectedGeneration.abiVersion, Is.EqualTo(staleSentinel.abiVersion));
                    Assert.That(rejectedGeneration.generation, Is.EqualTo(staleSentinel.generation));
                    Assert.That(rejectedGeneration.tick, Is.EqualTo(staleSentinel.tick));
                    Assert.That(
                        rejectedGeneration.globalManaDenominator,
                        Is.EqualTo(staleSentinel.globalManaDenominator));
                    Assert.That(
                        rejectedGeneration.right.holderEntityIndex,
                        Is.EqualTo(staleSentinel.right.holderEntityIndex));
                    Assert.That(
                        rejectedGeneration.right.rawSpellSelector,
                        Is.EqualTo(staleSentinel.right.rawSpellSelector));
                    Assert.That(
                        rejectedGeneration.right.casterMana,
                        Is.EqualTo(staleSentinel.right.casterMana));
                    boundarySession.Reset();
                    boundarySession.Step((ushort)384, (ushort)62080, (short)256);
                    var resetTerrain = boundarySession.TerrainState();
                    rejectedGeneration = staleSentinel;
                    status = NativeSim.mc_native_campaign_day_hud_state(
                        boundarySession.Handle,
                        oldGeneration,
                        resetTerrain.tick,
                        ref rejectedGeneration,
                        272);
                    Assert.That(status, Is.EqualTo(NativeSim.GenerationMismatch));
                    Assert.That(rejectedGeneration.abiVersion, Is.EqualTo(staleSentinel.abiVersion));
                    Assert.That(rejectedGeneration.generation, Is.EqualTo(staleSentinel.generation));
                    Assert.That(rejectedGeneration.tick, Is.EqualTo(staleSentinel.tick));
                    Assert.That(
                        rejectedGeneration.globalManaDenominator,
                        Is.EqualTo(staleSentinel.globalManaDenominator));
                    Assert.That(
                        rejectedGeneration.right.holderEntityIndex,
                        Is.EqualTo(staleSentinel.right.holderEntityIndex));
                    Assert.That(
                        rejectedGeneration.right.rawSpellSelector,
                        Is.EqualTo(staleSentinel.right.rawSpellSelector));
                    Assert.That(
                        rejectedGeneration.right.casterMana,
                        Is.EqualTo(staleSentinel.right.casterMana));
                }

                runtime.QualificationResetLevel2Production();
                var reset = runtime.Level2ProductionDayHud;
                Assert.That(reset.generation, Is.EqualTo(1));
                Assert.That(runtime.Level2ProductionDayHudPresenterReady, Is.True);
                Assert.That(reset.nyphur.visibilityFlags, Is.EqualTo(2u));
                AssertDayHudStartingSpell(
                    reset.left,
                    reset.localPlayerEntityIndex,
                    0,
                    115,
                    5);
                AssertDayHudStartingSpell(
                    reset.right,
                    reset.localPlayerEntityIndex,
                    1,
                    116,
                    3);
                Assert.That(
                    runtime.QualificationSelectedLevel2DayHudRomanTierTexture(0),
                    Does.Contain("Roman tier I"));
                Assert.That(
                    runtime.Level2ProductionNextStepInput.x,
                    Is.EqualTo((ushort)384));
                Assert.That(
                    runtime.Level2ProductionNextStepInput.y,
                    Is.EqualTo((ushort)62080));
                Assert.That(
                    runtime.Level2ProductionNextStepInput.z,
                    Is.EqualTo((short)256));

                var replay = DriveUntilRustOwnedFamily12Hud(runtime);
                Assert.That(replay.generation, Is.EqualTo(1));
                Assert.That(
                    runtime.Level2ProductionStepSnapshot.stateHash,
                    Is.EqualTo(acquiredStateHash));
                Assert.That(
                    replay.nyphur.visibilityFlags,
                    Is.EqualTo(acquiredNyphurVisibilityFlags));
                Assert.That(runtime.Level2ProductionDayHudPresenterReady, Is.True);
                AssertDayHudFamily12(replay.left, replay.localPlayerEntityIndex);
                AssertDayHudStartingSpell(
                    replay.right,
                    replay.localPlayerEntityIndex,
                    1,
                    116,
                    3);
                Assert.That(
                    (replay.left.holderEntityIndex,
                        replay.left.rawSpellSelector,
                        replay.left.frameSelector,
                        replay.left.iconSelector,
                        replay.left.levelTextSelector,
                        replay.left.tier),
                    Is.EqualTo((
                        hud.left.holderEntityIndex,
                        hud.left.rawSpellSelector,
                        hud.left.frameSelector,
                        hud.left.iconSelector,
                        hud.left.levelTextSelector,
                        hud.left.tier)));
            }
            finally
            {
                runtime?.QualificationDisposeNativeOwners();
                UnityEngine.Object.DestroyImmediate(root);
            }

            Assert.That(NativeCampaignSession.ActiveSessionCount, Is.Zero);
        }

        [Test]
        public void Level2RuntimeHandCastMarshalsToRust()
        {
            var root = new GameObject("Level 2 hand cast command route");
            root.SetActive(false);
            MagicCarpetRuntime runtime = null;
            try
            {
                runtime = root.AddComponent<MagicCarpetRuntime>();
                runtime.QualificationInitializeLevel2Production(
                    Level2Package,
                    Level2DayCache);

                var initial = runtime.Level2ProductionDayHud;
                AssertDayHudStartingSpell(
                    initial.right,
                    initial.localPlayerEntityIndex,
                    1,
                    116,
                    3);

                // This is the ordinary player hand route.  It must be consumed
                // by the production FixedUpdate and reflected by Rust's Day HUD,
                // not a managed spell implementation or direct native helper.
                runtime.QualificationRequestSpellCast(rightHand: true);
                runtime.QualificationDriveLevel2Frames(1);
                var armed = runtime.Level2ProductionDayHud;
                Assert.That(armed.right.rawSpellSelector, Is.EqualTo(1));
                Assert.That(armed.right.activationValue, Is.EqualTo(3));
                Assert.That(
                    armed.right.activeValue,
                    Is.EqualTo(armed.right.activationValue - 1),
                    "player111 arms Possession before holder116 pulses it in the same source pass");
                Assert.That(armed.right.activeValue, Is.EqualTo(2));

                runtime.QualificationDriveLevel2Frames(1);
                var pulsed = runtime.Level2ProductionDayHud;
                Assert.That(pulsed.right.activeValue, Is.EqualTo(armed.right.activeValue - 1));
                Assert.That(
                    runtime.Level2ProductionStepEvents.Any(eventRow =>
                        eventRow.kind == NativeSim.CampaignEventDynamicArena &&
                        eventRow.family == 39),
                    Is.True,
                    "the following Rust source pass emits the Possession dynamic effect");
            }
            finally
            {
                runtime?.QualificationDisposeNativeOwners();
                UnityEngine.Object.DestroyImmediate(root);
            }

            Assert.That(NativeCampaignSession.ActiveSessionCount, Is.Zero);
        }

        [Test]
        public void Level2RuntimeKeyboardMenuPreservesRawTierAndPhysicalHolderAuthority()
        {
            var root = new GameObject("Level 2 production spell-menu route");
            root.SetActive(false);
            MagicCarpetRuntime runtime = null;
            try
            {
                runtime = root.AddComponent<MagicCarpetRuntime>();
                runtime.QualificationInitializeLevel2Production(
                    Level2Package,
                    Level2DayCache);

                // Follow the ordinary startup/source route until the authored
                // family-12 holder is present; no native setter is involved.
                var acquired = DriveUntilRustOwnedFamily12Hud(runtime);
                AssertDayHudFamily12(
                    acquired.left,
                    acquired.localPlayerEntityIndex);

                // Drive the same semantic entry point used by the legacy
                // keyboard dispatcher. Rust remains the only authority that
                // accepts opcode 40 and returns the selected UI index.
                NativeSim.CampaignPlayerSnapshot player = default;
                var family12Selected = false;
                for (var attempt = 0; attempt < 29; attempt++)
                {
                    Assert.That(
                        runtime.SubmitLevel2SpellMenuInput(
                            MagicCarpetRuntime.Level2SpellMenuInputAction.Next),
                        Is.True);
                    runtime.QualificationDriveLevel2Frames(1);
                    player = runtime.Level2ProductionPlayers.Single(
                        candidate => candidate.slot == 0);
                    if (player.selectedSpellUiIndex == 12)
                    {
                        family12Selected = true;
                        break;
                    }
                }
                Assert.That(family12Selected, Is.True);
                Assert.That(
                    runtime.SubmitLevel2SpellMenuInput(
                        MagicCarpetRuntime.Level2SpellMenuInputAction.Tier1),
                    Is.True);
                runtime.QualificationDriveLevel2Frames(1);
                var locked = runtime.Level2ProductionDayHud.left;
                Assert.That(locked.rawSpellSelector, Is.EqualTo(12));
                Assert.That(locked.holderEntityIndex, Is.EqualTo(38));
                Assert.That(locked.action, Is.EqualTo(36));
                Assert.That(locked.tier, Is.EqualTo(0));
                player = runtime.Level2ProductionPlayers.Single(
                    candidate => candidate.slot == 0);
                Assert.That(player.selectedSpellUiIndex, Is.EqualTo(12));
                Assert.That(player.selectedSubspellIndex, Is.EqualTo(1));
                Assert.That(player.leftSubspellIndex, Is.EqualTo(0));

                // Raw opcode 41 selection does not manufacture a Tier-1
                // holder. With the real parent still below the physical
                // Tier-0 cost, the production selected-cast path must remain
                // cold and preserve the source Tier-0 profile.
                var beforeCast = runtime.Level2ProductionBeyondSight;
                Assert.That(beforeCast.tier, Is.EqualTo(0));
                Assert.That(beforeCast.manaCost, Is.EqualTo(1800));
                Assert.That(beforeCast.parentMana, Is.LessThan(1800));
                Assert.That(beforeCast.experienceAwarded, Is.Zero);
                Assert.That(
                    runtime.SubmitLevel2SpellMenuInput(
                        MagicCarpetRuntime.Level2SpellMenuInputAction.CastSelected),
                    Is.True);
                runtime.QualificationDriveLevel2Frames(1);
                var afterCast = runtime.Level2ProductionBeyondSight;
                Assert.That(afterCast.tier, Is.EqualTo(0));
                Assert.That(afterCast.manaCost, Is.EqualTo(1800));
                Assert.That(afterCast.activeValue, Is.EqualTo(0));
                Assert.That(
                    afterCast.experienceAwarded,
                    Is.EqualTo(beforeCast.experienceAwarded));
                Assert.That(
                    runtime.Level2ProductionDayHud.nyphur.visibilityFlags & 0x08,
                    Is.Zero);
            }
            finally
            {
                runtime?.QualificationDisposeNativeOwners();
                UnityEngine.Object.DestroyImmediate(root);
            }

            Assert.That(NativeCampaignSession.ActiveSessionCount, Is.Zero);
        }

        [Test]
        public void Level2MinimapUsesAuthoritativeVisibilityAndSourceOrder()
        {
            var root = new GameObject("Level 2 authoritative minimap route");
            root.SetActive(false);
            MagicCarpetRuntime runtime = null;
            try
            {
                runtime = root.AddComponent<MagicCarpetRuntime>();
                runtime.QualificationInitializeLevel2Production(
                    Level2Package,
                    Level2DayCache);

                var initialState = runtime.Level2ProductionMinimapState;
                var initialEntities = runtime.Level2ProductionMinimapEntities;
                var initialMarks = runtime.Level2ProductionMinimapMarks;
                Assert.That(runtime.Level2ProductionMinimapProjectionReady, Is.True);
                Assert.That(initialEntities, Is.Not.Empty);
                Assert.That(initialMarks, Is.Not.Empty);
                Assert.That(initialState.visibilityFlags, Is.EqualTo(2u));
                Assert.That(
                    initialEntities.Any(row => row.subjectEntityIndex == 112),
                    Is.False,
                    "Invisible Nyphur is absent from the Rust-admitted stream");
                Assert.That(
                    initialMarks.Any(row => row.subjectEntityIndex == 112),
                    Is.False,
                    "the unrelated live-relation stage is not the current source mark");
                Assert.That(
                    initialEntities.Zip(initialEntities.Skip(1), (left, right) =>
                        left.sourceOrder < right.sourceOrder).All(value => value),
                    Is.True,
                    "initial Rust minimap entities preserve source order");
                Assert.That(
                    initialMarks.Zip(initialMarks.Skip(1), (left, right) =>
                        left.sourceOrder < right.sourceOrder).All(value => value),
                    Is.True,
                    "initial Rust minimap marks preserve source order");
                Assert.That(initialMarks[0].kind, Is.EqualTo(2));
                Assert.That(initialMarks[0].paletteIndex, Is.EqualTo(188));
                Assert.That(initialMarks[0].helpSelector, Is.EqualTo(0x51));
                Assert.That(initialMarks[0].visible, Is.EqualTo(1));
                var managedHiddenMark = initialMarks[0];
                managedHiddenMark.visible = 0;
                Assert.DoesNotThrow(() =>
                    NativeCampaignSession.ValidateMinimapMarks(
                        new[] { managedHiddenMark },
                        initialState.generation,
                        initialState.tick));
                managedHiddenMark.visible = 2;
                Assert.Throws<System.InvalidOperationException>(() =>
                    NativeCampaignSession.ValidateMinimapMarks(
                        new[] { managedHiddenMark },
                        initialState.generation,
                        initialState.tick));
                var family4State = initialState;
                family4State.visibilityFlags = 0x10;
                Assert.DoesNotThrow(() =>
                    NativeCampaignSession.ValidateMinimapState(
                        family4State,
                        initialState.generation,
                        initialState.tick));
                var unknownVisibilityState = initialState;
                unknownVisibilityState.visibilityFlags = 0x20;
                Assert.Throws<System.InvalidOperationException>(() =>
                    NativeCampaignSession.ValidateMinimapState(
                        unknownVisibilityState,
                        initialState.generation,
                        initialState.tick));
                Assert.That(
                    initialEntities.Any(row => row.subjectEntityIndex == ushort.MaxValue) ||
                    initialMarks.Any(row => row.subjectEntityIndex == ushort.MaxValue),
                    Is.False,
                    "unknown minimap subjects are absent from the Rust projection");

                DriveUntilRustOwnedFamily12Hud(runtime);
                var acquiredState = runtime.Level2ProductionMinimapState;
                var acquiredEntities = runtime.Level2ProductionMinimapEntities;
                var acquiredMarks = runtime.Level2ProductionMinimapMarks;
                Assert.That(
                    acquiredState.visibilityFlags,
                    Is.EqualTo(6u),
                    "owner mode plus holder-acquired/inactive participant state");
                Assert.That(acquiredMarks, Is.Not.Empty);
                Assert.That(acquiredMarks[0].visible, Is.EqualTo(0));
                Assert.That(
                    acquiredEntities.Any(row => row.subjectEntityIndex == 112),
                    Is.False,
                    "holder-acquired but inactive participant is not an entity blip");
                Assert.That(
                    acquiredMarks.Any(row => row.subjectEntityIndex == 112),
                    Is.False,
                    "the unrelated live-relation stage is not the current source mark");
                Assert.That(acquiredMarks.Any(row => row.subjectEntityIndex == 0), Is.True);
                Assert.That(
                    acquiredEntities.Any(row => row.subjectEntityIndex == 0),
                    Is.False);
                Assert.That(
                    acquiredEntities.Any(row => row.subjectEntityIndex == ushort.MaxValue) ||
                    acquiredMarks.Any(row => row.subjectEntityIndex == ushort.MaxValue),
                    Is.False,
                    "unknown minimap subjects are absent from the Rust projection");
                Assert.That(
                    acquiredEntities.Zip(acquiredEntities.Skip(1), (left, right) =>
                        left.sourceOrder < right.sourceOrder).All(value => value),
                    Is.True,
                    "Rust minimap entities preserve source order");
                Assert.That(
                    acquiredMarks.Zip(acquiredMarks.Skip(1), (left, right) =>
                        left.sourceOrder < right.sourceOrder).All(value => value),
                    Is.True,
                    "Rust minimap marks preserve source order");

                runtime.QualificationDriveLevel2Frames(1);
                Assert.That(
                    runtime.Level2ProductionMinimapEntities
                        .Zip(runtime.Level2ProductionMinimapEntities.Skip(1), (left, right) =>
                            left.sourceOrder < right.sourceOrder).All(value => value),
                    Is.True,
                    "ordinary Rust minimap entity refresh preserves source order");
                Assert.That(
                    runtime.Level2ProductionMinimapMarks
                        .Zip(runtime.Level2ProductionMinimapMarks.Skip(1), (left, right) =>
                            left.sourceOrder < right.sourceOrder).All(value => value),
                    Is.True,
                    "ordinary Rust minimap mark refresh preserves source order");

                runtime.QualificationResetLevel2Production();
                var resetState = runtime.Level2ProductionMinimapState;
                var resetEntities = runtime.Level2ProductionMinimapEntities;
                var resetMarks = runtime.Level2ProductionMinimapMarks;
                Assert.That(resetState.visibilityFlags, Is.EqualTo(2u));
                Assert.That(resetState.markCount, Is.EqualTo(initialState.markCount));
                Assert.That(
                    resetEntities.Any(row => row.subjectEntityIndex == 112),
                    Is.False,
                    "reset restores Invisible visibility");
                Assert.That(resetMarks.Any(row => row.subjectEntityIndex == 112), Is.False);
                Assert.That(
                    resetState.entityHash,
                    Is.EqualTo(initialState.entityHash));
                Assert.That(
                    resetState.markHash,
                    Is.EqualTo(initialState.markHash));

                DriveUntilRustOwnedFamily12Hud(runtime);
                var replayState = runtime.Level2ProductionMinimapState;
                var replayEntities = runtime.Level2ProductionMinimapEntities;
                var replayMarks = runtime.Level2ProductionMinimapMarks;
                Assert.That(replayState.visibilityFlags, Is.EqualTo(acquiredState.visibilityFlags));
                Assert.That(replayState.entityHash, Is.EqualTo(acquiredState.entityHash));
                Assert.That(replayState.markHash, Is.EqualTo(acquiredState.markHash));
                Assert.That(
                    replayEntities.Zip(replayEntities.Skip(1), (left, right) =>
                        left.sourceOrder < right.sourceOrder).All(value => value),
                    Is.True,
                    "replayed Rust minimap entities preserve source order");
                Assert.That(
                    replayMarks.Zip(replayMarks.Skip(1), (left, right) =>
                        left.sourceOrder < right.sourceOrder).All(value => value),
                    Is.True,
                    "replayed Rust minimap marks preserve source order");
            }
            finally
            {
                runtime?.QualificationDisposeNativeOwners();
                UnityEngine.Object.DestroyImmediate(root);
            }

            Assert.That(NativeCampaignSession.ActiveSessionCount, Is.Zero);
        }

        private static void AssertDayHudStartingSpell(
            NativeSim.CampaignDayHudSpellSlot slot,
            ushort localPlayerEntityIndex,
            short rawSpellSelector,
            ushort holderEntityIndex,
            ushort activationValue)
        {
            Assert.That(slot.rawSpellSelector, Is.EqualTo(rawSpellSelector));
            Assert.That(slot.holderEntityIndex, Is.EqualTo(holderEntityIndex));
            Assert.That(slot.parentEntityIndex, Is.EqualTo(localPlayerEntityIndex));
            Assert.That(slot.model, Is.EqualTo((byte)rawSpellSelector));
            Assert.That(slot.tier, Is.Zero);
            Assert.That(slot.action, Is.EqualTo((byte)(rawSpellSelector * 3)));
            Assert.That(slot.actionMarker, Is.Zero);
            Assert.That(slot.activeValue, Is.Zero);
            Assert.That(slot.activationValue, Is.EqualTo(activationValue));
            Assert.That(slot.controlFlags, Is.EqualTo(5u));
            Assert.That(slot.maxMana, Is.EqualTo(100));
            Assert.That(slot.manaRegen, Is.Zero);
            Assert.That(slot.casterMana, Is.GreaterThan(0));
            Assert.That(slot.frameSelector, Is.EqualTo(1));
            Assert.That(slot.iconSelector, Is.EqualTo(123 + rawSpellSelector));
            Assert.That(slot.levelTextSelector, Is.Zero);
            Assert.That(slot.drawAdmitted, Is.EqualTo(1));
        }

        private static void AssertDayHudFamily12(
            NativeSim.CampaignDayHudSpellSlot slot,
            ushort localPlayerEntityIndex)
        {
            Assert.That(slot.holderEntityIndex, Is.EqualTo(38));
            Assert.That(slot.parentEntityIndex, Is.EqualTo(localPlayerEntityIndex));
            Assert.That(slot.parentEntityIndex, Is.EqualTo(111));
            Assert.That(slot.rawSpellSelector, Is.EqualTo(12));
            Assert.That(slot.action, Is.EqualTo(36));
            Assert.That(slot.frameSelector, Is.EqualTo(1));
            Assert.That(slot.iconSelector, Is.EqualTo(135));
            Assert.That(slot.levelTextSelector, Is.EqualTo(0));
            Assert.That(slot.tier, Is.EqualTo(0));
            Assert.That(slot.drawAdmitted, Is.EqualTo(1));
        }

        private static NativeSim.CampaignDayHudSnapshot DriveUntilRustOwnedFamily12Hud(
            MagicCarpetRuntime runtime)
        {
            var sourcePassBound = checked((int)runtime.Level2ProductionWorldInfo.entityCount);
            Assert.That(sourcePassBound, Is.GreaterThan(0));
            var hud = runtime.Level2ProductionDayHud;
            for (var sourcePass = 0; sourcePass < sourcePassBound; sourcePass++)
            {
                var family12 = new[] { hud.left, hud.right }.Any(slot =>
                    slot.rawSpellSelector == 12 &&
                    slot.holderEntityIndex != 0 &&
                    slot.action == 36);
                if (family12)
                {
                    return hud;
                }

                // The source arena census is the bounded ordinary-pass cap;
                // this helper only observes the Rust-owned HUD and never
                // queues a synthetic acquisition or menu command.
                runtime.QualificationDriveLevel2Frames(1);
                hud = runtime.Level2ProductionDayHud;
            }

            Assert.Fail(
                $"Rust-owned family-12 HUD holder did not appear within {sourcePassBound} ordinary passes");
            return hud;
        }

        private static (NativeSim.CampaignStepSnapshot snapshot,
            NativeSim.CampaignEvent[] events) StepLevel2SpellPresentation(
            NativeCampaignSession session,
            Level2SpellPresenter presenter,
            ushort playerX,
            ushort playerY,
            short playerZ,
            sbyte flags = 1,
            short requestedSpell = NativeSim.NoRequestedSpell,
            sbyte roll = 0,
            sbyte pitch = 0)
        {
            var stepped = session.Step(
                playerX,
                playerY,
                playerZ,
                flags: flags,
                requestedSpell: requestedSpell,
                roll: roll,
                pitch: pitch);
            presenter.Present(
                stepped.snapshot,
                stepped.events,
                session.DayHudState(stepped.snapshot.revision),
                session.ShieldState(),
                session.InvisibleState(),
                session.BeyondSightState());
            return stepped;
        }

        private static NativeSim.CampaignSpellPickupSnapshot
            MaterializeLevel2SpellSource(
                NativeCampaignSession session,
                Level2SpellPresenter presenter,
                byte family,
                ushort authoredIndex,
                ushort objectiveX,
                ushort objectiveY,
                int sourcePassBound,
                sbyte neutralFlags = 1)
        {
            for (var sourcePass = 0; sourcePass < sourcePassBound; sourcePass++)
            {
                var source = session.SpellPickups().SingleOrDefault(candidate =>
                    candidate.active == 1 &&
                    candidate.family == family &&
                    candidate.authoredIndex == authoredIndex);
                if (source.active == 1)
                {
                    return source;
                }

                StepLevel2SpellPresentation(
                    session,
                    presenter,
                    objectiveX,
                    objectiveY,
                    256,
                    flags: neutralFlags);
            }

            Assert.Fail(
                $"ordinary Level 2 source family {family} / authored row {authoredIndex} " +
                $"did not materialize within {sourcePassBound} source passes");
            return default;
        }

        private static (NativeSim.CampaignStepSnapshot snapshot,
            NativeSim.CampaignEvent[] events,
            NativeSim.CampaignEvent acquisition) AcquireLevel2SpellSource(
                NativeCampaignSession session,
                Level2SpellPresenter presenter,
                NativeSim.CampaignSpellPickupSnapshot source,
                int sourcePassBound,
                sbyte neutralFlags = 1)
        {
            for (var sourcePass = 0; sourcePass < sourcePassBound; sourcePass++)
            {
                var stepped = StepLevel2SpellPresentation(
                    session,
                    presenter,
                    source.x,
                    source.y,
                    source.z,
                    flags: neutralFlags);
                var acquisition = stepped.events.SingleOrDefault(eventRow =>
                    eventRow.kind == NativeSim.CampaignEventSpellAcquisition &&
                    eventRow.family == source.family &&
                    eventRow.sourceSlot == source.sourceEntityIndex &&
                    eventRow.flags == 1);
                if (acquisition.kind == NativeSim.CampaignEventSpellAcquisition)
                {
                    return (stepped.snapshot, stepped.events, acquisition);
                }
            }

            Assert.Fail(
                $"ordinary Level 2 source slot {source.sourceEntityIndex} " +
                $"did not acquire family {source.family} within {sourcePassBound} source passes");
            return default;
        }

        private static void AssertPresentedLevel2Spell(
            Level2SpellPresenter presenter,
            byte family,
            NativeSim.CampaignStepSnapshot snapshot,
            NativeSim.CampaignEvent acquisition,
            ushort holderEntityIndex,
            ushort parentEntityIndex)
        {
            var effect = presenter.ActiveEffects.Single(candidate => candidate.Family == family);
            Assert.That(effect.Generation, Is.EqualTo(snapshot.generation));
            Assert.That(effect.Tick, Is.EqualTo(snapshot.revision));
            Assert.That(effect.SourceSlot, Is.EqualTo(acquisition.sourceSlot));
            Assert.That(effect.EventOrdinal, Is.EqualTo(acquisition.ordinal));
            Assert.That(effect.HolderEntityIndex, Is.EqualTo(holderEntityIndex));
            Assert.That(effect.ParentEntityIndex, Is.EqualTo(parentEntityIndex));
        }

        private static void AssertNoPresentedLevel2Spell(
            Level2SpellPresenter presenter,
            byte family)
        {
            Assert.That(
                presenter.ActiveEffects.Any(candidate => candidate.Family == family),
                Is.False);
        }

        private static NativeSim.CampaignDayHudSnapshot WithLevel2SpellHudSlot(
            NativeSim.CampaignDayHudSnapshot hud,
            ushort holderEntityIndex,
            short rawSpellSelector,
            byte? model = null,
            sbyte? tier = null,
            short? activeValue = null,
            ushort? activationValue = null,
            byte? drawAdmitted = null,
            short? replacementSelector = null)
        {
            var isLeft = hud.left.holderEntityIndex == holderEntityIndex &&
                         hud.left.rawSpellSelector == rawSpellSelector;
            var isRight = hud.right.holderEntityIndex == holderEntityIndex &&
                          hud.right.rawSpellSelector == rawSpellSelector;
            Assert.That(isLeft || isRight, Is.True);
            var slot = isLeft ? hud.left : hud.right;
            if (model.HasValue)
            {
                slot.model = model.Value;
            }
            if (tier.HasValue)
            {
                slot.tier = tier.Value;
            }
            if (activeValue.HasValue)
            {
                slot.activeValue = activeValue.Value;
            }
            if (activationValue.HasValue)
            {
                slot.activationValue = activationValue.Value;
            }
            if (drawAdmitted.HasValue)
            {
                slot.drawAdmitted = drawAdmitted.Value;
            }
            if (replacementSelector.HasValue)
            {
                slot.rawSpellSelector = replacementSelector.Value;
            }
            if (isLeft)
            {
                hud.left = slot;
            }
            else
            {
                hud.right = slot;
            }
            return hud;
        }

        private static (ushort x, ushort y, short z) Level2LocalPlayerPosition(
            NativeCampaignSession session)
        {
            var player = session.Players().Single(candidate =>
                candidate.active == 1 && candidate.slot == 0);
            var entity = session.Entities().Single(candidate =>
                candidate.active == 1 && candidate.entityIndex == player.entityIndex);
            return (entity.x, entity.y, entity.z);
        }

        private static (ushort x, ushort y, short z, ulong rootTick) DriveLevel2OrdinaryManaWindow(
            NativeCampaignSession session,
            Level2SpellPresenter presenter,
            int requiredMana,
            int sourcePassBound)
        {
            // Pinned by the existing source-exact native C-ABI Possession
            // predecessor after the approach pass. These are managed test
            // inputs, not Unity movement or collision authority.
            const ushort StrictPossessionLaunchXOffset = 257;
            const ushort StrictPossessionLaunchYOffset = 197;
            const short StrictPossessionLaunchZOffset = 150;
            const byte ManaSphereClaimedEconomyCode = 2;

            // The established public Castle predecessor starts from the
            // initial player carrier at its ordinary flight height. Bind all
            // three values from the live C-ABI projection before invoking the
            // selected Castle command; Unity does not synthesize movement or
            // gameplay geometry.
            var initialPosition = Level2LocalPlayerPosition(session);
            Assert.That(initialPosition.x, Is.EqualTo((ushort)384));
            Assert.That(initialPosition.y, Is.EqualTo((ushort)62_080));
            Assert.That(initialPosition.z, Is.EqualTo((short)256));
            (ushort x, ushort y, short z) castleInputPosition = (
                initialPosition.x,
                initialPosition.y,
                initialPosition.z);

            // Authored row 11's public switch carrier is entity slot 23;
            // ObjectiveStageState and entity slot 11 are different records.
            // Use that live C-ABI entity projection rather than inventing a
            // managed coordinate or routing this production trigger through
            // a qualification helper.
            var groupOneSwitch = session.Entities().Single(candidate =>
                candidate.active == 1 && candidate.entityIndex == 23);
            var initialSphereIds = session.ManaSpheres()
                .Where(candidate => candidate.active == 1 && candidate.model == 39)
                .Select(candidate => candidate.sphereId)
                .ToArray();
            // The ordinary Day route supplies the regular flight height, then
            // observes the same-pass authored-group event and resulting
            // Rust-owned sphere rows. It has no frame/revision wait.
            var groupOneStep = StepLevel2SpellPresentation(
                session,
                presenter,
                groupOneSwitch.x,
                groupOneSwitch.y,
                256,
                flags: 0);
            Assert.That(
                groupOneStep.events.Any(eventRow =>
                    eventRow.kind == NativeSim.CampaignEventAuthoredGroup &&
                    eventRow.sourceSlot == groupOneSwitch.entityIndex &&
                    eventRow.flags != 0),
                Is.True,
                "ordinary authored row-11 switch must publish group-1 activation");
            var groupOne = session.ManaSpheres()
                .Where(candidate =>
                    candidate.active == 1 &&
                    candidate.model == 39 &&
                    candidate.mana == 512 &&
                    candidate.ownerEntityIndex == 0 &&
                    !initialSphereIds.Contains(candidate.sphereId))
                .OrderBy(candidate => candidate.sphereId)
                .ToArray();
            Assert.That(groupOne, Has.Length.GreaterThanOrEqualTo(6));

            // The target row is selected from the authoritative public sphere
            // projection. The actual player/holder geometry is then supplied
            // back through ordinary C-ABI Step inputs; no C# world mutation
            // or spell activation helper participates.
            var target = groupOne.Single(candidate => candidate.sphereId == 141);
            var approach = StepLevel2SpellPresentation(
                session,
                presenter,
                target.x,
                target.y,
                target.z,
                pitch: 127);
            var player = session.Entities().Single(candidate =>
                candidate.active == 1 && candidate.entityIndex == approach.snapshot.playerEntityIndex);
            Assert.That(player.fov, Is.EqualTo(0));
            target = session.ManaSpheres().Single(candidate =>
                candidate.active == 1 && candidate.sphereId == target.sphereId);
            Assert.That(
                (target.x, target.y, target.z),
                Is.EqualTo(((ushort)640, (ushort)45_142, (short)272)),
                "the managed fixture must observe the source-exact post-approach target carrier");
            var launchX = unchecked((ushort)(target.x + StrictPossessionLaunchXOffset));
            var launchY = unchecked((ushort)(target.y + StrictPossessionLaunchYOffset));
            var launchZ = unchecked((short)(target.z + StrictPossessionLaunchZOffset));
            var stepped = StepLevel2SpellPresentation(
                session,
                presenter,
                launchX,
                launchY,
                launchZ,
                flags: 0x41,
                requestedSpell: 1,
                pitch: 127);
            var localPlayer = stepped.snapshot.playerEntityIndex;
            Assert.That(
                stepped.events.Any(eventRow =>
                    eventRow.kind == NativeSim.CampaignEventDynamicArena &&
                    (eventRow.family == 39 || eventRow.family == 40)),
                Is.True,
                "strict public Possession launch must publish its Rust dynamic source/effect");
            var groupOneIds = groupOne
                .Select(candidate => candidate.sphereId)
                .ToHashSet();
            var observedClaimIds = new System.Collections.Generic.HashSet<uint>();
            var claimedEnough = false;
            for (var sourcePass = 0; sourcePass < sourcePassBound; sourcePass++)
            {
                foreach (var receipt in session.EconomyEvents().Where(receipt =>
                    receipt.code == ManaSphereClaimedEconomyCode &&
                    receipt.entity0 == localPlayer &&
                    groupOneIds.Contains(receipt.primaryId)))
                {
                    observedClaimIds.Add(receipt.primaryId);
                }
                var claimedMana = session.ManaSpheres()
                    .Where(candidate =>
                        candidate.active == 1 &&
                        candidate.model == 39 &&
                        candidate.ownerEntityIndex == localPlayer &&
                        groupOne.Any(grouped => grouped.sphereId == candidate.sphereId))
                    .Sum(candidate => candidate.mana);
                if (claimedMana >= 5 * target.mana)
                {
                    claimedEnough = true;
                    break;
                }
                stepped = StepLevel2SpellPresentation(
                    session,
                    presenter,
                    launchX,
                    launchY,
                    launchZ,
                    flags: 0);
            }
            Assert.That(
                observedClaimIds, Has.Count.EqualTo(6),
                "the strict public Possession route must retain all six group-1 claim receipts");
            Assert.That(
                claimedEnough,
                Is.True,
                "ordinary Possession flight did not retain enough Rust-owned mana for Castle return");

            var possessionCooled = false;
            var sawCastleGateClosed = false;
            var sawPossessionEffect = stepped.events.Any(eventRow =>
                eventRow.kind == NativeSim.CampaignEventDynamicArena && eventRow.family == 40);
            for (var sourcePass = 0; sourcePass < sourcePassBound; sourcePass++)
            {
                var hud = session.DayHudState(stepped.snapshot.revision);
                sawCastleGateClosed |= hud.localPlayerMana < 1_000;
                sawPossessionEffect |= stepped.events.Any(eventRow =>
                    eventRow.kind == NativeSim.CampaignEventDynamicArena && eventRow.family == 40);
                var possession = new[] { hud.left, hud.right }.Single(slot =>
                    slot.rawSpellSelector == 1 && slot.holderEntityIndex != 0);
                if (possession.activeValue == 0)
                {
                    possessionCooled = true;
                    break;
                }
                stepped = StepLevel2SpellPresentation(
                    session,
                    presenter,
                    launchX,
                    launchY,
                    launchZ,
                    flags: 0);
            }
            Assert.That(possessionCooled, Is.True);

            var castleManaReady = false;
            var visitedCastleInputPosition = false;
            for (var sourcePass = 0; sourcePass < sourcePassBound; sourcePass++)
            {
                var hud = session.DayHudState(stepped.snapshot.revision);
                sawCastleGateClosed |= hud.localPlayerMana < 1_000;
                var possessionEffectLive = stepped.events.Any(eventRow =>
                    eventRow.kind == NativeSim.CampaignEventDynamicArena && eventRow.family == 40);
                sawPossessionEffect |= possessionEffectLive;
                if (visitedCastleInputPosition &&
                    sawCastleGateClosed &&
                    hud.localPlayerMana >= 1_000 &&
                    !possessionEffectLive)
                {
                    castleManaReady = true;
                    break;
                }
                stepped = StepLevel2SpellPresentation(
                    session,
                    presenter,
                    castleInputPosition.x,
                    castleInputPosition.y,
                    castleInputPosition.z,
                    flags: 0);
                visitedCastleInputPosition = true;
            }
            Assert.That(
                sawCastleGateClosed,
                Is.True,
                "ordinary Possession upkeep must expose Castle's below-1000 mana gate");
            Assert.That(
                sawPossessionEffect,
                Is.True,
                "strict Possession route must expose its typed family-40 effect before Castle recovery");
            Assert.That(visitedCastleInputPosition, Is.True);
            Assert.That(
                castleManaReady,
                Is.True,
                "ordinary neutral C-ABI controls must retire the family-40 Possession effect before Castle");

            var castlePrecastHud = session.DayHudState(stepped.snapshot.revision);
            var castlePrecastPlayer = session.Entities().Single(candidate =>
                candidate.active == 1 && candidate.entityIndex == stepped.snapshot.playerEntityIndex);
            var castlePrecastPlayerState = session.Players().Single(candidate =>
                candidate.active == 1 && candidate.slot == stepped.snapshot.playerSlot);
            // The public entity projection does not promise a particular
            // class/model row for the currently selected Castle holder.
            // Preserve the observed rows as failure diagnostics instead of
            // making a test-only inference about a private holder layout.
            var castleCarrierRows = session.Entities()
                .Where(candidate => candidate.classId == 15 || candidate.entityIndex == 117)
                .OrderBy(candidate => candidate.entityIndex)
                .Select(candidate =>
                    $"slot={candidate.entityIndex}/src={candidate.sourceId}/active={candidate.active}/" +
                    $"class={candidate.classId}/model={candidate.model}/action={candidate.action}/phase={candidate.phase}")
                .ToArray();
            var castlePrecastDiagnostic =
                "Level 2 Castle pre-cast: " +
                $"input=({castleInputPosition.x},{castleInputPosition.y},{castleInputPosition.z}) " +
                $"inspection=({castlePrecastPlayer.x},{castlePrecastPlayer.y},{castlePrecastPlayer.z}) " +
                $"inspectionFlight=(yaw={castlePrecastPlayer.yaw},pitch={castlePrecastPlayer.pitch},fov={castlePrecastPlayer.fov},actualSpeed={castlePrecastPlayer.actualSpeed}) " +
                $"mana={castlePrecastHud.localPlayerMana}/{castlePrecastHud.localPlayerMaxMana} " +
                $"selectors=(ui={castlePrecastPlayerState.selectedSpellUiIndex},sub={castlePrecastPlayerState.selectedSubspellIndex},leftSlot={castlePrecastPlayerState.leftSpellSlotRaw},rightSlot={castlePrecastPlayerState.rightSpellSlotRaw}) " +
                $"spellRows=[{string.Join(";", castleCarrierRows)}] " +
                $"left=(raw={castlePrecastHud.left.rawSpellSelector},holder={castlePrecastHud.left.holderEntityIndex},action={castlePrecastHud.left.action},active={castlePrecastHud.left.activeValue},activation={castlePrecastHud.left.activationValue}) " +
                $"right=(raw={castlePrecastHud.right.rawSpellSelector},holder={castlePrecastHud.right.holderEntityIndex},action={castlePrecastHud.right.action},active={castlePrecastHud.right.activeValue},activation={castlePrecastHud.right.activationValue})";
            stepped = StepLevel2SpellPresentation(
                session,
                presenter,
                castleInputPosition.x,
                castleInputPosition.y,
                castleInputPosition.z,
                flags: 0x40,
                requestedSpell: 2);
            var requestDynamicEvents = stepped.events
                .Where(eventRow => eventRow.kind == NativeSim.CampaignEventDynamicArena)
                .Select(eventRow => $"{eventRow.family}@{eventRow.sourceSlot}")
                .ToArray();
            var requestFlyers = stepped.events
                .Where(eventRow =>
                    eventRow.pass == 0 &&
                    eventRow.kind == NativeSim.CampaignEventDynamicArena &&
                    eventRow.family == 38)
                .ToArray();
            var following = StepLevel2SpellPresentation(
                session,
                presenter,
                castleInputPosition.x,
                castleInputPosition.y,
                castleInputPosition.z,
                flags: 0);
            var followingDynamicEvents = following.events
                .Where(eventRow => eventRow.kind == NativeSim.CampaignEventDynamicArena)
                .Select(eventRow => $"{eventRow.family}@{eventRow.sourceSlot}")
                .ToArray();
            var followingRoots = following.events
                .Where(eventRow =>
                    eventRow.kind == NativeSim.CampaignEventDynamicArena && eventRow.family == 1)
                .ToArray();
            var castleRouteDiagnostic =
                $"request kind8=[{string.Join(",", requestDynamicEvents)}] " +
                $"following kind8=[{string.Join(",", followingDynamicEvents)}] " +
                $"flyers=[{string.Join(",", requestFlyers.Select(eventRow => eventRow.sourceSlot))}] " +
                $"roots=[{string.Join(",", followingRoots.Select(eventRow => eventRow.sourceSlot))}]";
            Assert.That(
                requestFlyers,
                Has.Length.EqualTo(1),
                "ordinary selected Castle command must publish its one gate-only Rust flyer in the request source pass; " +
                castlePrecastDiagnostic + "; " + castleRouteDiagnostic);
            Assert.That(
                followingRoots,
                Is.Not.Empty,
                "the neutral following source pass must land the ordered Rust Castle root after its flyer; " +
                castlePrecastDiagnostic + "; " + castleRouteDiagnostic);
            var castleRootTick = following.snapshot.revision;
            stepped = following;

            var manaWindowReady = false;
            for (var sourcePass = 0; sourcePass < sourcePassBound; sourcePass++)
            {
                var hud = session.DayHudState(stepped.snapshot.revision);
                if (hud.localPlayerMaxMana >= requiredMana &&
                    hud.localPlayerMana >= requiredMana)
                {
                    manaWindowReady = true;
                    break;
                }
                stepped = StepLevel2SpellPresentation(
                    session,
                    presenter,
                    castleInputPosition.x,
                    castleInputPosition.y,
                    castleInputPosition.z,
                    flags: 0);
            }
            Assert.That(
                manaWindowReady,
                Is.True,
                $"ordinary Castle route did not recover {requiredMana} Rust-owned mana");
            return (
                castleInputPosition.x,
                castleInputPosition.y,
                castleInputPosition.z,
                castleRootTick);
        }

        private static void DriveLevel2FirstCastleDefenderReturn(
            NativeCampaignSession session,
            Level2SpellPresenter presenter,
            (ushort x, ushort y, short z) position,
            int requiredMana,
            int sourcePassBound,
            ulong rootTick)
        {
            // Pin the existing source-exact C-ABI predecessor: the defender
            // first collects the five-sphere merged payload, then returns all
            // six claimed spheres to the Castle. These are ordered public Rust
            // economy receipts, not managed mana mutation.
            const byte CastleDefenderCollectedEconomyCode = 16;
            const byte CastleDefenderArrivedEconomyCode = 7;
            const int FirstCollectionMana = 2_560;
            const int FirstArrivalMana = 3_072;
            NativeSim.CampaignEconomyEvent firstCollection = default;
            NativeSim.CampaignEconomyEvent firstArrival = default;
            ulong collectionTick = 0;
            ulong arrivalTick = 0;
            for (var sourcePass = 0; sourcePass < sourcePassBound; sourcePass++)
            {
                var stepped = StepLevel2SpellPresentation(
                    session,
                    presenter,
                    position.x,
                    position.y,
                    position.z,
                    flags: 0);
                // Poll directly after every ordinary neutral source turn so
                // a return cannot be mistaken for a cached/revision wait.
                var economyEvents = session.EconomyEvents();
                var collections = economyEvents
                    .Where(receipt => receipt.code == CastleDefenderCollectedEconomyCode)
                    .ToArray();
                if (collectionTick == 0 && collections.Length != 0)
                {
                    firstCollection = collections.Single();
                    collectionTick = stepped.snapshot.revision;
                }
                var arrivals = economyEvents
                    .Where(receipt => receipt.code == CastleDefenderArrivedEconomyCode)
                    .ToArray();
                if (arrivalTick == 0 && arrivals.Length != 0)
                {
                    firstArrival = arrivals.Single();
                    arrivalTick = stepped.snapshot.revision;
                }
                var hud = session.DayHudState(stepped.snapshot.revision);
                if (collectionTick > rootTick &&
                    arrivalTick > collectionTick &&
                    hud.localPlayerMaxMana >= requiredMana &&
                    hud.localPlayerMana >= requiredMana)
                {
                    Assert.That(firstCollection.value0, Is.EqualTo(FirstCollectionMana));
                    Assert.That(firstArrival.value0, Is.EqualTo(FirstArrivalMana));
                    Assert.That(firstCollection.primaryId, Is.Not.Zero);
                    Assert.That(firstArrival.primaryId, Is.Not.Zero);
                    Assert.That(firstCollection.entity0, Is.Not.Zero);
                    Assert.That(firstArrival.entity0, Is.Not.Zero);
                    return;
                }
            }
            Assert.Fail(
                $"ordinary f38-to-f1 Castle route did not publish its ordered " +
                $"{FirstCollectionMana}-mana collection and {FirstArrivalMana}-mana return " +
                $"or recover {requiredMana} Rust-owned HUD mana within " +
                $"{sourcePassBound} source passes");
        }

        [Test]
        public void Level2CampaignCabiIsReachableFromManagedSession()
        {
            using var session = NativeCampaignSession.OpenLevel2(Level2Package);
            var initial = session.WorldInfo();
            Assert.That(initial.abiVersion, Is.EqualTo(NativeSim.AbiVersion));
            Assert.That(initial.structSize, Is.EqualTo(64));
            Assert.That(initial.externalLevel, Is.EqualTo(2));
            Assert.That(initial.internalLevel, Is.EqualTo(1));
            Assert.That(initial.entityCount, Is.EqualTo(1000));
            Assert.That(initial.activeEntityCount, Is.EqualTo(114));
            Assert.That(initial.playerCount, Is.EqualTo(8));
            Assert.That(initial.terrainWidth, Is.EqualTo(256));
            Assert.That(initial.terrainHeight, Is.EqualTo(256));

            var players = session.Players();
            Assert.That(players, Has.Length.EqualTo(8));
            Assert.That(players.Count(player => player.active != 0), Is.EqualTo(2));

            var entities = session.Entities();
            Assert.That(entities, Has.Length.EqualTo(1000));
            Assert.That(entities.Count(entity => entity.active != 0), Is.EqualTo(114));

            var stepped = session.Step(123, 456, 7);
            Assert.That(stepped.snapshot.abiVersion, Is.EqualTo(NativeSim.AbiVersion));
            Assert.That(stepped.snapshot.structSize, Is.EqualTo(112));
            Assert.That(stepped.snapshot.generation, Is.EqualTo(0));
            Assert.That(stepped.snapshot.revision, Is.EqualTo(1));
            Assert.That(stepped.snapshot.eventCount, Is.GreaterThan(0));
            Assert.That(stepped.events, Has.Length.EqualTo((int)stepped.snapshot.eventCount));
            Assert.That(
                stepped.events.Any(eventRow => eventRow.kind != NativeSim.CampaignEventError),
                Is.True);
            var authoredGroupEvents = session.AuthoredGroupEvents();
            Assert.That(authoredGroupEvents, Is.Not.Empty);
            Assert.That(
                authoredGroupEvents.All(eventRow =>
                    eventRow.abiVersion == NativeSim.AbiVersion &&
                    eventRow.structSize == 48 &&
                    eventRow.generation == stepped.snapshot.generation &&
                    eventRow.ordinal < (uint)stepped.events.Length &&
                    stepped.events[(int)eventRow.ordinal].kind ==
                        NativeSim.CampaignEventAuthoredGroup &&
                    eventRow.eventIndex < eventRow.flags),
                Is.True);
            var economyEvents = session.EconomyEvents();
            // The idle Level 2 fixture may legitimately produce no economy
            // sidecars on its first turn. The accessor still has to return a
            // valid managed array, while the native C ABI test exercises a
            // positive carrier/economy route.
            Assert.That(economyEvents, Is.Not.Null);
            Assert.That(
                economyEvents.All(eventRow =>
                    eventRow.abiVersion == NativeSim.AbiVersion &&
                    eventRow.generation == stepped.snapshot.generation),
                Is.True);

            session.Reset();
            Assert.That(session.Generation, Is.EqualTo(1));
            Assert.That(session.AuthoredGroupEvents(), Is.Empty);
            var reset = session.WorldInfo();
            Assert.That(reset.resetStateHash, Is.EqualTo(initial.resetStateHash));
            var rebound = session.Step(123, 456, 7);
            var reboundAuthoredGroupEvents = session.AuthoredGroupEvents();
            Assert.That(reboundAuthoredGroupEvents, Is.Not.Empty);
            Assert.That(
                reboundAuthoredGroupEvents.All(eventRow =>
                    eventRow.generation == rebound.snapshot.generation &&
                    eventRow.generation == 1),
                Is.True);
        }

        [Test]
        public void Level2CampaignTerrainIsExactAndGenerationBound()
        {
            using var session = NativeCampaignSession.OpenLevel2(Level2Package);

            var state = session.TerrainState();
            Assert.That(state.abiVersion, Is.EqualTo(NativeSim.AbiVersion));
            Assert.That(state.structSize, Is.EqualTo(64));
            Assert.That(state.generation, Is.Zero);
            Assert.That(state.tick, Is.Zero);
            Assert.That(state.mutationSequence, Is.Zero);
            Assert.That(
                (state.width, state.height, state.planeCount, state.hashSchema),
                Is.EqualTo(((ushort)256, (ushort)256, (byte)6, (byte)1)));
            Assert.That(state.authority, Is.EqualTo(NativeSim.SessionTerrainLive));
            Assert.DoesNotThrow(() =>
                NativeCampaignSession.ValidateTerrainState(state, 0));
            var malformed = state;
            malformed.reserved = new uint[4];
            malformed.reserved[0] = 1;
            Assert.Throws<System.InvalidOperationException>(() =>
                NativeCampaignSession.ValidateTerrainState(malformed, 0));

            var terrainTypes = session.TerrainPlane(NativeSim.TerrainTypes);
            var groundHeights = session.TerrainPlane(NativeSim.TerrainHeight);
            var shading = session.TerrainPlane(NativeSim.TerrainShade);
            var angles = session.TerrainPlane(NativeSim.TerrainAngle);
            var secondaryHeights = session.TerrainPlane(NativeSim.TerrainSecondary);
            var entityIndices = session.TerrainEntityIndices();
            Assert.That(terrainTypes, Has.Length.EqualTo(256 * 256));
            Assert.That(groundHeights, Has.Length.EqualTo(256 * 256));
            Assert.That(shading, Has.Length.EqualTo(256 * 256));
            Assert.That(angles, Has.Length.EqualTo(256 * 256));
            Assert.That(secondaryHeights, Has.Length.EqualTo(256 * 256));
            Assert.That(entityIndices, Has.Length.EqualTo(256 * 256));
            Assert.That(terrainTypes.Distinct().Count(), Is.GreaterThan(1));
            Assert.That(entityIndices.Any(index => index >= 0), Is.True);

            session.Reset();
            var reset = session.TerrainState();
            Assert.That(reset.generation, Is.EqualTo(1));
            Assert.That(reset.terrainHash, Is.EqualTo(state.terrainHash));
            CollectionAssert.AreEqual(
                terrainTypes,
                session.TerrainPlane(NativeSim.TerrainTypes));
            CollectionAssert.AreEqual(
                entityIndices,
                session.TerrainEntityIndices());

            var stale = NativeSim.mc_native_campaign_terrain_state(
                session.Handle,
                0,
                out var ignored,
                64);
            Assert.That(stale, Is.EqualTo(NativeSim.GenerationMismatch));
        }

        [Test]
        public void Level2CampaignGoatDispatchIsReachableFromManagedSession()
        {
            using var session = NativeCampaignSession.OpenLevel2(Level2Package);

            var initialInfo = session.WorldInfo();
            var initialGoats = session.Goats();
            Assert.That(initialGoats, Has.Length.EqualTo(36));
            CollectionAssert.AreEqual(
                Enumerable.Range(39, 36).Select(index => (ushort)index).ToArray(),
                initialGoats.Select(goat => goat.entityIndex).ToArray());
            var action11 = initialGoats.Single(goat => goat.entityIndex == 39);
            Assert.That(action11.action, Is.EqualTo(11));
            Assert.That(action11.partnerEntityIndex, Is.EqualTo(66));

            var sawAction11Bleat = false;
            for (var turn = 0; turn < 256 && !sawAction11Bleat; turn++)
            {
                var stepped = session.Step(123, 456, 7);
                var goatEvents = session.GoatEvents();
                foreach (var goatEvent in goatEvents)
                {
                    Assert.That(goatEvent.abiVersion, Is.EqualTo(NativeSim.AbiVersion));
                    Assert.That(goatEvent.generation, Is.EqualTo(stepped.snapshot.generation));
                    Assert.That(
                        goatEvent.ordinal,
                        Is.LessThan((uint)stepped.events.Length));
                    var generic = stepped.events[(int)goatEvent.ordinal];
                    Assert.That(generic.ordinal, Is.EqualTo(goatEvent.ordinal));
                    Assert.That(generic.kind, Is.EqualTo(NativeSim.CampaignEventGoat));
                    Assert.That(generic.pass, Is.EqualTo(goatEvent.pass));
                    Assert.That(generic.family, Is.EqualTo(goatEvent.family));
                    Assert.That(generic.sourceSlot, Is.EqualTo(goatEvent.sourceSlot));

                    if (
                        goatEvent.code == NativeSim.CampaignGoatBleat &&
                        goatEvent.action == 11 &&
                        goatEvent.soundId == 46 &&
                        (goatEvent.sourceSlot == 39 || goatEvent.sourceSlot == 71))
                    {
                        Assert.That(goatEvent.entity0, Is.EqualTo(goatEvent.sourceSlot));
                        sawAction11Bleat = true;
                    }
                }
            }

            Assert.That(
                sawAction11Bleat,
                Is.True,
                "managed production route must expose the source action-11 bleat");

            session.Reset();
            var hitBefore = session.Goats().Single(goat => goat.entityIndex == 39);
            Assert.That(hitBefore.action, Is.EqualTo(11));
            Assert.That(hitBefore.pendingDamage, Is.Zero);
            session.SubmitGoatDamage(hitBefore.entityIndex, 100, 152);
            Assert.That(
                session.Goats().Single(goat => goat.entityIndex == hitBefore.entityIndex).pendingDamage,
                Is.EqualTo(100));

            var hitStep = session.Step(123, 456, 7);
            var hitAfter = session.Goats().Single(goat => goat.entityIndex == hitBefore.entityIndex);
            Assert.That(hitAfter.pendingDamage, Is.Zero);
            Assert.That(hitAfter.life, Is.EqualTo(hitBefore.life - 100));
            Assert.That(hitAfter.targetEntityIndex, Is.EqualTo(152));
            Assert.That(new[] { (byte)10, (byte)14 }, Does.Contain(hitAfter.action));
            Assert.That(
                hitStep.events.Any(eventRow =>
                    eventRow.kind == NativeSim.CampaignEventGoat &&
                    eventRow.sourceSlot == hitBefore.entityIndex),
                Is.True);

            session.Reset();
            var lethalBefore = session.Goats().Single(goat => goat.entityIndex == 39);
            Assert.That(lethalBefore.action, Is.EqualTo(11));
            session.SubmitGoatDamage(lethalBefore.entityIndex, 1_000_000, 152);

            var sawRemoval = false;
            var sawReclaim = false;
            var sawExtraSphereRequest = false;
            var sawDeathBurst = false;
            for (var turn = 0; turn < 32 && (!sawRemoval || !sawReclaim); turn++)
            {
                var stepped = session.Step(123, 456, 7);
                var goatEvents = session.GoatEvents();
                var sidecarKeys = goatEvents
                    .Select(goatEvent => (goatEvent.ordinal, goatEvent.eventIndex))
                    .ToArray();
                Assert.That(sidecarKeys.Distinct().Count(), Is.EqualTo(sidecarKeys.Length));
                foreach (var goatEvent in goatEvents)
                {
                    Assert.That(goatEvent.abiVersion, Is.EqualTo(NativeSim.AbiVersion));
                    Assert.That(goatEvent.generation, Is.EqualTo(stepped.snapshot.generation));
                    Assert.That(
                        goatEvent.ordinal,
                        Is.LessThan((uint)stepped.events.Length));
                    var generic = stepped.events[(int)goatEvent.ordinal];
                    Assert.That(generic.ordinal, Is.EqualTo(goatEvent.ordinal));
                    Assert.That(generic.kind, Is.EqualTo(NativeSim.CampaignEventGoat));
                    Assert.That(generic.sourceSlot, Is.EqualTo(goatEvent.sourceSlot));
                    if (
                        goatEvent.sourceSlot == lethalBefore.entityIndex &&
                        goatEvent.code == NativeSim.CampaignGoatRemovalQueued)
                    {
                        Assert.That(goatEvent.entity0, Is.EqualTo(lethalBefore.entityIndex));
                        sawRemoval = true;
                    }
                    else if (
                        goatEvent.sourceSlot == lethalBefore.entityIndex &&
                        goatEvent.code == NativeSim.CampaignGoatReclaimed)
                    {
                        Assert.That(goatEvent.entity0, Is.EqualTo(lethalBefore.entityIndex));
                        sawReclaim = true;
                    }
                    else if (
                        goatEvent.sourceSlot == lethalBefore.entityIndex &&
                        goatEvent.code == NativeSim.CampaignGoatExtraManaSphereRequest)
                    {
                        Assert.That(goatEvent.entity0, Is.EqualTo(lethalBefore.entityIndex));
                        sawExtraSphereRequest = true;
                    }
                }

                foreach (var economyEvent in session.EconomyEvents())
                {
                    Assert.That(economyEvent.abiVersion, Is.EqualTo(NativeSim.AbiVersion));
                    Assert.That(
                        economyEvent.generation,
                        Is.EqualTo(stepped.snapshot.generation));
                    Assert.That(
                        economyEvent.ordinal,
                        Is.LessThan((uint)stepped.events.Length));
                    var generic = stepped.events[(int)economyEvent.ordinal];
                    Assert.That(generic.ordinal, Is.EqualTo(economyEvent.ordinal));
                    Assert.That(generic.kind, Is.EqualTo(NativeSim.CampaignEventEconomy));
                    if (
                        economyEvent.code == NativeSim.CampaignEconomyDeathBurstSpawned &&
                        economyEvent.entity0 == lethalBefore.entityIndex)
                    {
                        sawDeathBurst = true;
                    }
                }
            }

            Assert.That(sawRemoval, Is.True);
            Assert.That(sawReclaim, Is.True);
            Assert.That(sawExtraSphereRequest, Is.True);
            Assert.That(sawDeathBurst, Is.True);

            var staleGeneration = session.Generation;
            session.Reset();
            Assert.That(session.Generation, Is.EqualTo(staleGeneration + 1));
            var resetInfo = session.WorldInfo();
            Assert.That(resetInfo.resetStateHash, Is.EqualTo(initialInfo.resetStateHash));
            var resetGoats = session.Goats();
            CollectionAssert.AreEqual(
                initialGoats.Select(goat =>
                    (
                        goat.entityIndex,
                        goat.targetEntityIndex,
                        goat.action,
                        goat.phase,
                        goat.life,
                        goat.maxLife,
                        goat.mana,
                        goat.pendingDamage,
                        goat.controlFlags,
                        goat.partnerEntityIndex)).ToArray(),
                resetGoats.Select(goat =>
                    (
                        goat.entityIndex,
                        goat.targetEntityIndex,
                        goat.action,
                        goat.phase,
                        goat.life,
                        goat.maxLife,
                        goat.mana,
                        goat.pendingDamage,
                        goat.controlFlags,
                        goat.partnerEntityIndex)).ToArray());
            Assert.That(
                NativeSim.mc_native_campaign_goat_damage_submit(
                    session.Handle,
                    staleGeneration,
                    lethalBefore.entityIndex,
                    1,
                    152),
                Is.EqualTo(NativeSim.GenerationMismatch));
        }

        [Test]
        public void Level2CampaignGoatSnapshotsAreReachableFromManagedSession()
        {
            using var session = NativeCampaignSession.OpenLevel2(Level2Package);

            var goats = session.Goats();

            Assert.That(goats, Has.Length.EqualTo(36));
            Assert.That(goats.Select(goat => goat.entityIndex), Does.Contain(39));
            Assert.That(goats.Select(goat => goat.entityIndex), Does.Contain(71));
            Assert.That(
                goats.Where(goat => goat.entityIndex == 39 || goat.entityIndex == 71)
                    .All(goat =>
                        goat.abiVersion == NativeSim.AbiVersion &&
                        goat.structSize == 84 &&
                        goat.classId == 5 &&
                        goat.model == 1 &&
                        goat.action == 11 &&
                        goat.partnerEntityIndex == 66),
                Is.True);
        }

        [Test]
        public void Level2CampaignGoatDamageMailboxReachesManagedProductionPath()
        {
            using var session = NativeCampaignSession.OpenLevel2(Level2Package);

            var before = session.Goats().Single(goat => goat.entityIndex == 39);
            Assert.That(before.pendingDamage, Is.EqualTo(0));

            session.SubmitGoatDamage(before.entityIndex, 100, 152);
            var queued = session.Goats().Single(goat => goat.entityIndex == before.entityIndex);
            Assert.That(queued.pendingDamage, Is.EqualTo(100));

            var stepped = session.Step(123, 456, 7);
            var after = session.Goats().Single(goat => goat.entityIndex == before.entityIndex);
            Assert.That(after.pendingDamage, Is.EqualTo(0));
            Assert.That(after.life, Is.EqualTo(before.life - 100));
            Assert.That(
                stepped.events.Any(eventRow =>
                    eventRow.kind == NativeSim.CampaignEventGoat &&
                    eventRow.sourceSlot == before.entityIndex),
                Is.True);

            session.Reset();
            Assert.That(session.Generation, Is.EqualTo(1));
            Assert.That(
                session.Goats().Single(goat => goat.entityIndex == before.entityIndex).pendingDamage,
                Is.EqualTo(0));
        }

        [Test]
        public void Level2CampaignAction9GoatDamageUsesManagedSourceTransition()
        {
            using var session = NativeCampaignSession.OpenLevel2(Level2Package);

            var before = session.Goats().Single(goat => goat.entityIndex == 40);
            Assert.That(before.action, Is.EqualTo(9));
            Assert.That(before.pendingDamage, Is.Zero);

            session.SubmitGoatDamage(before.entityIndex, 100, 152);
            var queued = session.Goats().Single(goat => goat.entityIndex == before.entityIndex);
            Assert.That(queued.pendingDamage, Is.EqualTo(100));

            var stepped = session.Step(123, 456, 7);
            var after = session.Goats().Single(goat => goat.entityIndex == before.entityIndex);
            Assert.That(after.pendingDamage, Is.Zero);
            Assert.That(after.life, Is.EqualTo(before.life - 100));
            Assert.That(after.action, Is.EqualTo(14));
            Assert.That(after.targetEntityIndex, Is.EqualTo(152));
            Assert.That(
                stepped.events.Any(eventRow =>
                    eventRow.kind == NativeSim.CampaignEventGoat &&
                    eventRow.sourceSlot == before.entityIndex),
                Is.True);

            session.Reset();
            var reset = session.Goats().Single(goat => goat.entityIndex == before.entityIndex);
            Assert.That(reset.action, Is.EqualTo(9));
            Assert.That(reset.pendingDamage, Is.Zero);
            Assert.That(reset.life, Is.EqualTo(before.life));
        }

        [Test]
        public void Level2CampaignGoatSidecarsReachManagedProductionPath()
        {
            using var session = NativeCampaignSession.OpenLevel2(Level2Package);

            var target = session.Goats().Single(goat => goat.entityIndex == 39);
            session.SubmitGoatDamage(target.entityIndex, 1_000_000, 152);

            var sawRemoval = false;
            var sawReclaim = false;
            var sawManaSphere = false;
            var sawDeathBurst = false;
            uint? manaSphereOrdinal = null;
            uint? deathBurstOrdinal = null;
            for (var turn = 0; turn < 32 && (!sawRemoval || !sawReclaim); turn++)
            {
                var stepped = session.Step(123, 456, 7);
                var goatEvents = session.GoatEvents()
                    .Where(eventRow => eventRow.sourceSlot == target.entityIndex)
                    .ToArray();
                foreach (var goatEvent in goatEvents)
                {
                    Assert.That(goatEvent.abiVersion, Is.EqualTo(NativeSim.AbiVersion));
                    Assert.That(goatEvent.generation, Is.EqualTo(stepped.snapshot.generation));
                    Assert.That(
                        stepped.events[(int)goatEvent.ordinal].kind,
                        Is.EqualTo(NativeSim.CampaignEventGoat));
                    if (goatEvent.code == NativeSim.CampaignGoatRemovalQueued)
                    {
                        sawRemoval = true;
                    }
                    else if (goatEvent.code == NativeSim.CampaignGoatReclaimed)
                    {
                        sawReclaim = true;
                    }
                }

                foreach (var economyEvent in session.EconomyEvents()
                    .Where(eventRow =>
                        (eventRow.code == NativeSim.CampaignEconomyManaSphereSpawned &&
                            eventRow.sourceSlot == target.entityIndex &&
                            eventRow.entity0 == target.entityIndex) ||
                        (eventRow.code == NativeSim.CampaignEconomyDeathBurstSpawned &&
                            eventRow.entity0 == target.entityIndex)))
                {
                    Assert.That(economyEvent.abiVersion, Is.EqualTo(NativeSim.AbiVersion));
                    Assert.That(economyEvent.structSize, Is.EqualTo(64));
                    Assert.That(economyEvent.generation, Is.EqualTo(stepped.snapshot.generation));
                    Assert.That(
                        economyEvent.ordinal,
                        Is.LessThan((uint)stepped.events.Length));
                    Assert.That(
                        stepped.events[(int)economyEvent.ordinal].kind,
                        Is.EqualTo(NativeSim.CampaignEventEconomy));
                    if (economyEvent.code == NativeSim.CampaignEconomyManaSphereSpawned)
                    {
                        sawManaSphere = true;
                        manaSphereOrdinal = economyEvent.ordinal;
                    }
                    else if (economyEvent.code == NativeSim.CampaignEconomyDeathBurstSpawned)
                    {
                        sawDeathBurst = true;
                        deathBurstOrdinal = economyEvent.ordinal;
                    }
                }
            }

            Assert.That(sawRemoval, Is.True);
            Assert.That(sawReclaim, Is.True);
            Assert.That(sawManaSphere, Is.True);
            Assert.That(sawDeathBurst, Is.True);
            Assert.That(manaSphereOrdinal, Is.Not.Null);
            Assert.That(deathBurstOrdinal, Is.Not.Null);
            Assert.That(manaSphereOrdinal!.Value, Is.LessThan(deathBurstOrdinal!.Value));
        }

        [Test]
        public void Level2CampaignManaArenaSnapshotsAreReachableFromManagedSession()
        {
            using var session = NativeCampaignSession.OpenLevel2(Level2Package);

            var spheres = session.ManaSpheres();
            Assert.That(spheres, Is.Not.Null);
            Assert.That(
                spheres.All(sphere =>
                    sphere.abiVersion == NativeSim.AbiVersion &&
                    sphere.structSize == 64 &&
                    sphere.classId == 10 &&
                    sphere.model == 39 &&
                    sphere.action == 41),
                Is.True);

            // The reset fixture has no carrier rows, but the accessor must
            // preserve the shared-arena contract and return a valid empty
            // array; the native C ABI test covers the positive multi-carrier
            // route and retained-row semantics.
            var carriers = session.ManaCarriers();
            Assert.That(carriers, Is.Not.Null);
            Assert.That(
                carriers.All(carrier =>
                    carrier.abiVersion == NativeSim.AbiVersion &&
                    carrier.structSize == 80 &&
                    carrier.generation == session.Generation),
                Is.True);

            session.Reset();
            Assert.That(session.ManaSpheres(), Has.Length.EqualTo(spheres.Length));
            Assert.That(session.ManaCarriers(), Is.Empty);
        }

        [Test]
        public void Level2CampaignShieldSnapshotBridgeIsReachableAndResetBound()
        {
            using var session = NativeCampaignSession.OpenLevel2(Level2Package);
            var state = session.ShieldState();
            Assert.That(state.abiVersion, Is.EqualTo(NativeSim.AbiVersion));
            Assert.That(state.structSize, Is.EqualTo(72));
            Assert.That(state.generation, Is.EqualTo(session.Generation));
            Assert.That(state.active, Is.EqualTo(0));
            Assert.Throws<System.InvalidOperationException>(() => session.ActivateShield());

            session.Reset();
            var reset = session.ShieldState();
            Assert.That(reset.generation, Is.EqualTo(session.Generation));
            Assert.That(reset.active, Is.EqualTo(0));
        }

        [Test]
        public void Level2CampaignObjectiveStageBridgeUsesLiveAuthoredPoint()
        {
            using var session = NativeCampaignSession.OpenLevel2(Level2Package);
            var state = session.ObjectiveStageState();
            Assert.That(state.abiVersion, Is.EqualTo(NativeSim.AbiVersion));
            Assert.That(state.structSize, Is.EqualTo(48));
            Assert.That(state.generation, Is.EqualTo(session.Generation));
            Assert.That(
                (state.objectiveSlot, state.stageSlot),
                Is.EqualTo(((byte)0, (byte)3)));
            Assert.That(state.stageKind, Is.EqualTo(5));
            Assert.That(state.stageState, Is.EqualTo(1));
            Assert.That(state.done, Is.EqualTo(0));
            Assert.That(state.relationDomain, Is.EqualTo(0));

            var malformed = state;
            malformed.reserved[0] = 1;
            Assert.Throws<System.InvalidOperationException>(() =>
                NativeCampaignSession.ValidateObjectiveStageState(
                    malformed,
                    session.Generation,
                    0,
                    3));
            malformed = state;
            malformed.generation = session.Generation + 1;
            Assert.Throws<System.InvalidOperationException>(() =>
                NativeCampaignSession.ValidateObjectiveStageState(
                    malformed,
                    session.Generation,
                    0,
                    3));

            session.Step((ushort)state.axisX, (ushort)state.axisY, 7);
            var progressed = session.ObjectiveStageState();
            Assert.That(progressed.stageState, Is.EqualTo(2));
            Assert.That(progressed.done, Is.EqualTo(0));

            session.Step((ushort)state.axisX, (ushort)state.axisY, 7);
            var completed = session.ObjectiveStageState();
            Assert.That(completed.stageState, Is.EqualTo(2));
            Assert.That(completed.done, Is.EqualTo(1));

            session.Reset();
            var reset = session.ObjectiveStageState();
            Assert.That(reset.generation, Is.EqualTo(session.Generation));
            Assert.That(reset.stageState, Is.EqualTo(1));
            Assert.That(reset.done, Is.EqualTo(0));
        }

        [Test]
        public void Level2CampaignInvisibleSnapshotBridgeIsReachableAndResetBound()
        {
            using var session = NativeCampaignSession.OpenLevel2(Level2Package);
            var state = session.InvisibleState();
            Assert.That(state.abiVersion, Is.EqualTo(NativeSim.AbiVersion));
            Assert.That(state.structSize, Is.EqualTo(80));
            Assert.That(state.generation, Is.EqualTo(session.Generation));
            Assert.That(state.active, Is.EqualTo(0));
            Assert.Throws<System.InvalidOperationException>(() => session.ActivateInvisible());

            session.Reset();
            var reset = session.InvisibleState();
            Assert.That(reset.generation, Is.EqualTo(session.Generation));
            Assert.That(reset.active, Is.EqualTo(0));
        }

        [Test]
        public void Level2CampaignBeyondSightSnapshotBridgeIsReachableAndResetBound()
        {
            using var session = NativeCampaignSession.OpenLevel2(Level2Package);
            var state = session.BeyondSightState();
            Assert.That(state.abiVersion, Is.EqualTo(NativeSim.AbiVersion));
            Assert.That(state.structSize, Is.EqualTo(80));
            Assert.That(state.generation, Is.EqualTo(session.Generation));
            Assert.That(state.active, Is.EqualTo(0));
            Assert.Throws<System.InvalidOperationException>(() => session.ActivateBeyondSight());

            session.Reset();
            var reset = session.BeyondSightState();
            Assert.That(reset.generation, Is.EqualTo(session.Generation));
            Assert.That(reset.active, Is.EqualTo(0));
        }

        [Test]
        public void Level2CampaignBeyondSightPickupReachesManagedProductionPath()
        {
            using var session = NativeCampaignSession.OpenLevel2(Level2Package);
            var source = session.Entities()
                .Single(entity => entity.entityIndex == 38);

            Assert.That(source.abiVersion, Is.EqualTo(NativeSim.AbiVersion));
            Assert.That(source.classId, Is.EqualTo(15));
            Assert.That(source.model, Is.EqualTo(12));
            Assert.That(source.action, Is.EqualTo(37));

            NativeSim.CampaignStepSnapshot snapshot = default;
            NativeSim.CampaignEvent[] events = null;
            for (var turn = 0; turn < 4; turn++)
            {
                (snapshot, events) = session.Step(source.x, source.y, source.z);
                if (events.Any(eventRow =>
                        eventRow.kind == NativeSim.CampaignEventSpellAcquisition &&
                        eventRow.family == 12 &&
                        eventRow.sourceSlot == 38 &&
                        eventRow.flags == 1))
                {
                    break;
                }
            }

            Assert.That(snapshot.generation, Is.EqualTo(0));
            var acquisitionEvents = events.Where(eventRow =>
                eventRow.abiVersion == NativeSim.AbiVersion &&
                eventRow.structSize == 48 &&
                eventRow.generation == session.Generation &&
                eventRow.kind == NativeSim.CampaignEventSpellAcquisition &&
                eventRow.family == 12 &&
                eventRow.sourceSlot == 38 &&
                eventRow.flags == 1).ToArray();
            Assert.That(acquisitionEvents, Is.Not.Empty);

            var acquired = session.BeyondSightState();
            Assert.That(acquired.active, Is.EqualTo(1));
            Assert.That(acquired.tier, Is.EqualTo(0));
            Assert.That(acquired.action, Is.EqualTo(36));
            Assert.That(acquired.holderEntityIndex, Is.EqualTo(38));
            Assert.That(acquired.parentEntityIndex, Is.EqualTo(111));
            Assert.That(acquired.manaCost, Is.EqualTo(1_800));
            Assert.That(acquired.parentMana, Is.EqualTo(1_000));
            Assert.That(acquired.activeValue, Is.EqualTo(0));
            Assert.That(acquired.generation, Is.EqualTo(session.Generation));

            // The managed route reaches the real holder, but the qualified
            // reset only supplies 1,000 mana while source tier 0 costs 1,800.
            // Activation must reject without mutating the live holder; a
            // positive cast requires the separate source-owned mana
            // carrier/Castle path and is intentionally not fabricated here.
            Assert.Throws<System.InvalidOperationException>(
                () => session.ActivateBeyondSight());
            var rejected = session.BeyondSightState();
            Assert.That(rejected.active, Is.EqualTo(acquired.active));
            Assert.That(rejected.tier, Is.EqualTo(acquired.tier));
            Assert.That(rejected.action, Is.EqualTo(acquired.action));
            Assert.That(rejected.activeValue, Is.EqualTo(acquired.activeValue));
            Assert.That(rejected.parentMana, Is.EqualTo(acquired.parentMana));
            Assert.That(rejected.generation, Is.EqualTo(acquired.generation));

            session.Reset();
            var reset = session.BeyondSightState();
            Assert.That(reset.generation, Is.EqualTo(1));
            Assert.That(reset.active, Is.EqualTo(0));
            Assert.That(reset.holderEntityIndex, Is.EqualTo(0));
        }

        [Test]
        public void Level2CampaignShieldPickupUsesManagedNativeSourcePosition()
        {
            using var session = NativeCampaignSession.OpenLevel2(Level2Package);

            // Distinguish optional managed roll/pitch forwarding from a
            // wrapper that silently substitutes zeros. Both sessions take the
            // same ordinary campaign step, so only the real C-ABI command
            // payload can change this Rust-owned hash.
            using (var neutral = NativeCampaignSession.OpenLevel2(Level2Package))
            using (var steered = NativeCampaignSession.OpenLevel2(Level2Package))
            {
                var neutralSnapshot = neutral.Step(384, 62080, 256, flags: 1);
                var steeredSnapshot = steered.Step(
                    384,
                    62080,
                    256,
                    flags: 1,
                    roll: 17,
                    pitch: -13);
                Assert.That(
                    steeredSnapshot.snapshot.stateHash,
                    Is.Not.EqualTo(neutralSnapshot.snapshot.stateHash),
                    "managed roll/pitch reaches the authoritative campaign command");
            }

            Assert.That(session.SpellPickups(), Is.Empty);
            var objective = session.ObjectiveStageState();
            session.Step((ushort)objective.axisX, (ushort)objective.axisY, 7);
            session.Step((ushort)objective.axisX, (ushort)objective.axisY, 7);
            var source = session.SpellPickups()
                .Single(pickup => pickup.authoredIndex == 155);
            Assert.That(source.abiVersion, Is.EqualTo(NativeSim.AbiVersion));
            Assert.That(source.structSize, Is.EqualTo(64));
            Assert.That(source.family, Is.EqualTo(6));
            Assert.That(source.action, Is.EqualTo(19));
            Assert.That(source.active, Is.EqualTo(1));

            NativeSim.CampaignStepSnapshot snapshot = default;
            NativeSim.CampaignEvent[] events = null;
            for (var turn = 0; turn < 4; turn++)
            {
                (snapshot, events) = session.Step(source.x, source.y, source.z);
                if (events.Any(eventRow =>
                        eventRow.kind == NativeSim.CampaignEventSpellAcquisition &&
                        eventRow.family == 6 &&
                        eventRow.sourceSlot == source.sourceEntityIndex &&
                        eventRow.flags == 1))
                {
                    break;
                }
            }

            Assert.That(snapshot.generation, Is.EqualTo(0));
            Assert.That(events.Any(eventRow =>
                    eventRow.abiVersion == NativeSim.AbiVersion &&
                    eventRow.structSize == 48 &&
                    eventRow.generation == session.Generation &&
                    eventRow.kind == NativeSim.CampaignEventSpellAcquisition &&
                    eventRow.family == 6 &&
                    eventRow.sourceSlot == source.sourceEntityIndex &&
                    eventRow.flags == 1), Is.True);

            var acquired = session.ShieldState();
            Assert.That(acquired.active, Is.EqualTo(1));
            Assert.That(acquired.action, Is.EqualTo(18));
            Assert.That(acquired.holderEntityIndex, Is.EqualTo(source.sourceEntityIndex));
            Assert.That(acquired.generation, Is.EqualTo(session.Generation));

            // The managed wrapper must carry command roll/pitch, selected
            // family, and the ordinary selected-spell flag into the existing
            // C ABI step.  Do not use the direct activation helper as positive
            // evidence: the player route is the contract under test.
            var activatedSnapshot = session.Step(
                source.x,
                source.y,
                source.z,
                flags: 0x41,
                requestedSpell: 6,
                roll: 17,
                pitch: -13);
            Assert.That(
                activatedSnapshot.snapshot.revision,
                Is.GreaterThan(snapshot.revision));
            var activated = session.ShieldState();
            Assert.That(activated.abiVersion, Is.EqualTo(NativeSim.AbiVersion));
            Assert.That(activated.structSize, Is.EqualTo(72));
            Assert.That(activated.active, Is.EqualTo(1));
            Assert.That(activated.action, Is.EqualTo(18));
            Assert.That(activated.activationValue, Is.EqualTo(101));
            Assert.That(
                activated.activeValue,
                Is.EqualTo(activated.activationValue - 1),
                "player111 arms Shield before its later holder pulses it in the same source pass");
            Assert.That(activated.activeValue, Is.GreaterThan(0));
            Assert.That(activated.holderEntityIndex, Is.EqualTo(source.sourceEntityIndex));
            Assert.That(activated.generation, Is.EqualTo(session.Generation));

            session.Step(source.x, source.y, source.z);
            var afterPulse = session.ShieldState();
            Assert.That(afterPulse.active, Is.EqualTo(1));
            Assert.That(afterPulse.activeValue, Is.EqualTo(activated.activeValue - 1));
            Assert.That(afterPulse.countdown, Is.EqualTo(activated.countdown - 1));

            var beforeDamage = session.ShieldState();
            session.SubmitShieldDamage(beforeDamage.parentEntityIndex, 100, 152);
            session.Step(source.x, source.y, source.z);
            var afterDamage = session.ShieldState();
            Assert.That(afterDamage.parentLife, Is.EqualTo(beforeDamage.parentLife - 25));
            Assert.That(afterDamage.experienceAwarded, Is.EqualTo(beforeDamage.experienceAwarded + 1));
            Assert.That(afterDamage.holderEntityIndex, Is.EqualTo(source.sourceEntityIndex));
            Assert.That(afterDamage.generation, Is.EqualTo(session.Generation));

            var expired = afterDamage;
            for (var pulse = 0; pulse < 512 && expired.activeValue > 0; pulse++)
            {
                session.Step(source.x, source.y, source.z);
                expired = session.ShieldState();
            }
            Assert.That(expired.activeValue, Is.EqualTo(0));
            Assert.That(expired.countdown, Is.EqualTo(0));
            Assert.That(expired.action, Is.EqualTo(18));

            session.Reset();
            Assert.That(session.SpellPickups(), Is.Empty);
            Assert.That(session.ShieldState().active, Is.EqualTo(0));
        }

        [Test]
        public void Level2SpellPresenterPresentsRustOwnedShieldAndFailsClosed()
        {
            using var session = NativeCampaignSession.OpenLevel2(Level2Package);
            var presenter = new Level2SpellPresenter();
            presenter.Reset(session.Generation);
            var sourcePassBound = checked((int)session.WorldInfo().entityCount);
            Assert.That(sourcePassBound, Is.GreaterThan(0));

            var objective = session.ObjectiveStageState();
            // The source stage axes are signed ABI fields interpreted as
            // wrapped world coordinates at the input boundary.
            var objectiveX = unchecked((ushort)objective.axisX);
            var objectiveY = unchecked((ushort)objective.axisY);
            // These are ordinary C-ABI turns at the authored objective point;
            // no fixture or managed acquisition setter supplies the holder.
            StepLevel2SpellPresentation(
                session,
                presenter,
                objectiveX,
                objectiveY,
                256);
            StepLevel2SpellPresentation(
                session,
                presenter,
                objectiveX,
                objectiveY,
                256);

            var source = MaterializeLevel2SpellSource(
                session,
                presenter,
                family: 6,
                authoredIndex: 155,
                objectiveX: objectiveX,
                objectiveY: objectiveY,
                sourcePassBound: sourcePassBound);
            var acquired = AcquireLevel2SpellSource(
                session,
                presenter,
                source,
                sourcePassBound);
            var acquiredHud = session.DayHudState(acquired.snapshot.revision);
            var acquiredShield = session.ShieldState();
            var acquiredInvisible = session.InvisibleState();
            var acquiredBeyondSight = session.BeyondSightState();
            var receipt = presenter.Receipts.Single(candidate => candidate.Family == 6);
            Assert.That(receipt.Generation, Is.EqualTo(acquired.snapshot.generation));
            Assert.That(receipt.Tick, Is.EqualTo(acquired.snapshot.revision));
            Assert.That(receipt.SourceSlot, Is.EqualTo(source.sourceEntityIndex));
            Assert.That(receipt.EventOrdinal, Is.EqualTo(acquired.acquisition.ordinal));
            Assert.That(acquiredShield.active, Is.EqualTo(1));
            Assert.That(acquiredShield.activeValue, Is.EqualTo(0));
            AssertNoPresentedLevel2Spell(presenter, 6);

            // These altered rows are rejection-only probes. The positive
            // receipt above is the unmodified ordered stream returned by Rust.
            var acquisitionIndex = System.Array.FindIndex(
                acquired.events,
                candidate => candidate.ordinal == acquired.acquisition.ordinal);
            Assert.That(acquisitionIndex, Is.GreaterThanOrEqualTo(0));
            var sourceLessEvents = (NativeSim.CampaignEvent[])acquired.events.Clone();
            sourceLessEvents[acquisitionIndex].sourceSlot = 0;
            var sourceLess = new Level2SpellPresenter();
            sourceLess.Present(
                acquired.snapshot,
                sourceLessEvents,
                acquiredHud,
                acquiredShield,
                acquiredInvisible,
                acquiredBeyondSight);
            Assert.That(sourceLess.Receipts, Is.Empty);

            var unknownFamilyEvents = (NativeSim.CampaignEvent[])acquired.events.Clone();
            unknownFamilyEvents[acquisitionIndex].family = 99;
            var unknownFamily = new Level2SpellPresenter();
            unknownFamily.Present(
                acquired.snapshot,
                unknownFamilyEvents,
                acquiredHud,
                acquiredShield,
                acquiredInvisible,
                acquiredBeyondSight);
            Assert.That(unknownFamily.Receipts, Is.Empty);

            var duplicateOrdinalEvents = new NativeSim.CampaignEvent[acquired.events.Length + 1];
            System.Array.Copy(acquired.events, duplicateOrdinalEvents, acquired.events.Length);
            duplicateOrdinalEvents[duplicateOrdinalEvents.Length - 1] = acquired.acquisition;
            var duplicateOrdinalStep = acquired.snapshot;
            duplicateOrdinalStep.eventCount = (uint)duplicateOrdinalEvents.Length;
            var duplicateOrdinal = new Level2SpellPresenter();
            duplicateOrdinal.Present(
                duplicateOrdinalStep,
                duplicateOrdinalEvents,
                acquiredHud,
                acquiredShield,
                acquiredInvisible,
                acquiredBeyondSight);
            Assert.That(duplicateOrdinal.Receipts, Is.Empty);
            Assert.That(duplicateOrdinal.ActiveEffects, Is.Empty);

            var armed = StepLevel2SpellPresentation(
                session,
                presenter,
                source.x,
                source.y,
                source.z,
                flags: 0x41,
                requestedSpell: 6,
                roll: 17,
                pitch: -13);
            var shield = session.ShieldState();
            Assert.That(shield.active, Is.EqualTo(1));
            Assert.That(shield.activeValue, Is.GreaterThan(0));
            AssertPresentedLevel2Spell(
                presenter,
                6,
                armed.snapshot,
                acquired.acquisition,
                shield.holderEntityIndex,
                shield.parentEntityIndex);

            var armedHud = session.DayHudState(armed.snapshot.revision);
            var invisible = session.InvisibleState();
            var beyondSight = session.BeyondSightState();
            var wrongHolder = shield;
            wrongHolder.holderEntityIndex = unchecked((ushort)(wrongHolder.holderEntityIndex + 1));
            presenter.Present(
                armed.snapshot,
                armed.events,
                armedHud,
                wrongHolder,
                invisible,
                beyondSight);
            AssertNoPresentedLevel2Spell(presenter, 6);

            var wrongParent = shield;
            wrongParent.parentEntityIndex = unchecked((ushort)(wrongParent.parentEntityIndex + 1));
            presenter.Present(
                armed.snapshot,
                armed.events,
                armedHud,
                shield,
                invisible,
                beyondSight);
            presenter.Present(
                armed.snapshot,
                armed.events,
                armedHud,
                wrongParent,
                invisible,
                beyondSight);
            AssertNoPresentedLevel2Spell(presenter, 6);

            var wrongAction = shield;
            wrongAction.action = unchecked((byte)(wrongAction.action + 1));
            presenter.Present(
                armed.snapshot,
                armed.events,
                armedHud,
                wrongAction,
                invisible,
                beyondSight);
            AssertNoPresentedLevel2Spell(presenter, 6);

            var wrongTier = shield;
            wrongTier.tier = unchecked((byte)(wrongTier.tier + 1));
            presenter.Present(
                armed.snapshot,
                armed.events,
                armedHud,
                wrongTier,
                invisible,
                beyondSight);
            AssertNoPresentedLevel2Spell(presenter, 6);

            var wrongStepPlayer = armed.snapshot;
            wrongStepPlayer.playerEntityIndex = unchecked((ushort)(
                wrongStepPlayer.playerEntityIndex + 1));
            presenter.Present(
                wrongStepPlayer,
                armed.events,
                armedHud,
                shield,
                invisible,
                beyondSight);
            AssertNoPresentedLevel2Spell(presenter, 6);

            var wrongStepSlot = armed.snapshot;
            wrongStepSlot.playerSlot = unchecked((byte)(wrongStepSlot.playerSlot + 1));
            presenter.Present(
                wrongStepSlot,
                armed.events,
                armedHud,
                shield,
                invisible,
                beyondSight);
            AssertNoPresentedLevel2Spell(presenter, 6);

            var missingDrawAdmission = WithLevel2SpellHudSlot(
                armedHud,
                shield.holderEntityIndex,
                6,
                drawAdmitted: 0);
            presenter.Present(
                armed.snapshot,
                armed.events,
                missingDrawAdmission,
                shield,
                invisible,
                beyondSight);
            AssertNoPresentedLevel2Spell(presenter, 6);

            var mismatchedModel = WithLevel2SpellHudSlot(
                armedHud,
                shield.holderEntityIndex,
                6,
                model: 99);
            presenter.Present(
                armed.snapshot,
                armed.events,
                mismatchedModel,
                shield,
                invisible,
                beyondSight);
            AssertNoPresentedLevel2Spell(presenter, 6);

            var mismatchedHudTier = WithLevel2SpellHudSlot(
                armedHud,
                shield.holderEntityIndex,
                6,
                tier: unchecked((sbyte)(shield.tier + 1)));
            presenter.Present(
                armed.snapshot,
                armed.events,
                mismatchedHudTier,
                shield,
                invisible,
                beyondSight);
            AssertNoPresentedLevel2Spell(presenter, 6);

            var mismatchedActiveValue = WithLevel2SpellHudSlot(
                armedHud,
                shield.holderEntityIndex,
                6,
                activeValue: unchecked((short)(shield.activeValue + 1)));
            presenter.Present(
                armed.snapshot,
                armed.events,
                mismatchedActiveValue,
                shield,
                invisible,
                beyondSight);
            AssertNoPresentedLevel2Spell(presenter, 6);

            var mismatchedActivationValue = WithLevel2SpellHudSlot(
                armedHud,
                shield.holderEntityIndex,
                6,
                activationValue: unchecked((ushort)(shield.activationValue + 1)));
            presenter.Present(
                armed.snapshot,
                armed.events,
                mismatchedActivationValue,
                shield,
                invisible,
                beyondSight);
            AssertNoPresentedLevel2Spell(presenter, 6);

            var unknownSelector = WithLevel2SpellHudSlot(
                armedHud,
                shield.holderEntityIndex,
                6,
                replacementSelector: 99);
            presenter.Present(
                armed.snapshot,
                armed.events,
                unknownSelector,
                shield,
                invisible,
                beyondSight);
            AssertNoPresentedLevel2Spell(presenter, 6);

            var staleTick = armed.snapshot;
            staleTick.revision--;
            var staleHud = armedHud;
            staleHud.tick--;
            presenter.Present(
                staleTick,
                armed.events,
                staleHud,
                shield,
                invisible,
                beyondSight);
            AssertNoPresentedLevel2Spell(presenter, 6);

            // Restore only from the unmodified current Rust frame before
            // observing normal source expiration.
            presenter.Present(
                armed.snapshot,
                armed.events,
                armedHud,
                shield,
                invisible,
                beyondSight);
            AssertPresentedLevel2Spell(
                presenter,
                6,
                armed.snapshot,
                acquired.acquisition,
                shield.holderEntityIndex,
                shield.parentEntityIndex);

            var expired = shield;
            for (var sourcePass = 0;
                 sourcePass < sourcePassBound && expired.activeValue > 0;
                 sourcePass++)
            {
                StepLevel2SpellPresentation(
                    session,
                    presenter,
                    source.x,
                    source.y,
                    source.z);
                expired = session.ShieldState();
            }
            Assert.That(expired.activeValue, Is.EqualTo(0));
            AssertNoPresentedLevel2Spell(presenter, 6);

            session.Reset();
            presenter.Reset(session.Generation);
            Assert.That(session.ShieldState().active, Is.EqualTo(0));
            Assert.That(presenter.Receipts, Is.Empty);
            Assert.That(presenter.ActiveEffects, Is.Empty);

            // The last pre-reset Rust frame is stale for the reset presenter;
            // it cannot revive a cleared managed label.
            presenter.Present(
                armed.snapshot,
                armed.events,
                armedHud,
                shield,
                invisible,
                beyondSight);
            Assert.That(presenter.ActiveEffects, Is.Empty);
        }

        [Test]
        public void Level2SpellPresenterPresentsOrdinaryInvisibleLifecycle()
        {
            // Match the production runtime and the Rust predecessor's owner
            // cache-backed Level 2 route. This is runtime input, not a test
            // fixture: the ordinary Castle/economy chain depends on it.
            using var session = NativeCampaignSession.OpenLevel2(Level2Package, Level2DayCache);
            var presenter = new Level2SpellPresenter();
            presenter.Reset(session.Generation);
            var sourcePassBound = checked((int)session.WorldInfo().entityCount * 8);
            var castlePosition = DriveLevel2OrdinaryManaWindow(
                session,
                presenter,
                requiredMana: 1_000,
                sourcePassBound: sourcePassBound);

            var objective = session.ObjectiveStageState();
            var source = MaterializeLevel2SpellSource(
                session,
                presenter,
                family: 11,
                authoredIndex: 156,
                objectiveX: unchecked((ushort)objective.axisX),
                objectiveY: unchecked((ushort)objective.axisY),
                sourcePassBound: sourcePassBound,
                neutralFlags: 0);
            var acquired = AcquireLevel2SpellSource(
                session,
                presenter,
                source,
                sourcePassBound,
                neutralFlags: 0);
            var cold = session.InvisibleState();
            Assert.That(cold.active, Is.EqualTo(1));
            Assert.That(cold.action, Is.EqualTo(33));
            Assert.That(cold.activeValue, Is.EqualTo(0));
            AssertNoPresentedLevel2Spell(presenter, 11);

            // The newly acquired higher-slot holder is created after the
            // player callback that acquired it. One or more ordinary neutral
            // Castle turns refresh that source-owned command mirror; the
            // public code-7 receipt proves its first defender return before
            // the selected family-11 command is eligible to pulse.
            DriveLevel2FirstCastleDefenderReturn(
                session,
                presenter,
                (castlePosition.x, castlePosition.y, castlePosition.z),
                cold.manaCost,
                sourcePassBound,
                castlePosition.rootTick);
            var armed = StepLevel2SpellPresentation(
                session,
                presenter,
                castlePosition.x,
                castlePosition.y,
                castlePosition.z,
                flags: 0x40,
                requestedSpell: 11);
            var invisible = session.InvisibleState();
            Assert.That(invisible.active, Is.EqualTo(1));
            Assert.That(invisible.activeValue, Is.GreaterThan(0));
            Assert.That(invisible.cleanupRequested, Is.EqualTo(0));
            AssertPresentedLevel2Spell(
                presenter,
                11,
                armed.snapshot,
                acquired.acquisition,
                invisible.holderEntityIndex,
                invisible.parentEntityIndex);

            var cleanupRequested = invisible;
            cleanupRequested.cleanupRequested = 1;
            presenter.Present(
                armed.snapshot,
                armed.events,
                session.DayHudState(armed.snapshot.revision),
                session.ShieldState(),
                cleanupRequested,
                session.BeyondSightState());
            AssertNoPresentedLevel2Spell(presenter, 11);
            presenter.Present(
                armed.snapshot,
                armed.events,
                session.DayHudState(armed.snapshot.revision),
                session.ShieldState(),
                invisible,
                session.BeyondSightState());
            AssertPresentedLevel2Spell(
                presenter,
                11,
                armed.snapshot,
                acquired.acquisition,
                invisible.holderEntityIndex,
                invisible.parentEntityIndex);

            var expired = invisible;
            for (var sourcePass = 0;
                 sourcePass < sourcePassBound && expired.activeValue > 0;
                 sourcePass++)
            {
                StepLevel2SpellPresentation(
                    session,
                    presenter,
                    castlePosition.x,
                    castlePosition.y,
                    castlePosition.z,
                    flags: 0);
                expired = session.InvisibleState();
            }
            Assert.That(expired.activeValue, Is.EqualTo(0));
            AssertNoPresentedLevel2Spell(presenter, 11);

            session.Reset();
            presenter.Reset(session.Generation);
            Assert.That(session.InvisibleState().active, Is.EqualTo(0));
            Assert.That(presenter.Receipts, Is.Empty);
            Assert.That(presenter.ActiveEffects, Is.Empty);
        }

        [Test]
        public void Level2SpellPresenterPresentsOrdinaryBeyondSightLifecycle()
        {
            // The cache-backed session is the production Level 2 route used
            // by the existing public Castle/economy predecessor.
            using var session = NativeCampaignSession.OpenLevel2(Level2Package, Level2DayCache);
            var presenter = new Level2SpellPresenter();
            presenter.Reset(session.Generation);
            var sourcePassBound = checked((int)session.WorldInfo().entityCount * 8);
            var castlePosition = DriveLevel2OrdinaryManaWindow(
                session,
                presenter,
                requiredMana: 1_000,
                sourcePassBound: sourcePassBound);

            // Entities() is the immutable owner-package inspection surface.
            // Use it solely for the static family-12 source identity and
            // source-authored overlap coordinate; no dynamic state is read
            // from this projection.
            var source = session.Entities().Single(candidate => candidate.entityIndex == 38);
            Assert.That(
                (source.active, source.classId, source.model, source.action, source.phase),
                Is.EqualTo(((byte)1, (byte)15, (byte)12, (byte)37, (byte)39)));

            // The static model-12 source has no timeout, but the ordinary
            // Castle path above has already advanced its runtime cadence.
            // Follow the existing strict C-ABI predecessor: stay on the
            // source-authored overlap coordinate for at most four ordinary
            // speed-0 turns and require its one Rust acquisition receipt.
            // This keeps the first two genuine overlap turns while avoiding
            // a false claim that every prior public route reaches the source
            // on a fixed second callback.
            const int BeyondSightOverlapPassBound = 4;
            var acquired = false;
            NativeSim.CampaignEvent acquisition = default;
            for (var overlapPass = 0;
                 overlapPass < BeyondSightOverlapPassBound;
                 overlapPass++)
            {
                var overlap = StepLevel2SpellPresentation(
                    session,
                    presenter,
                    source.x,
                    source.y,
                    source.z);
                var acquisitions = overlap.events.Where(eventRow =>
                    eventRow.kind == NativeSim.CampaignEventSpellAcquisition &&
                    eventRow.family == 12 &&
                    eventRow.sourceSlot == source.entityIndex &&
                    eventRow.flags == 1).ToArray();
                Assert.That(
                    acquisitions,
                    Has.Length.LessThanOrEqualTo(1),
                    "one ordinary source pass must not publish duplicate family-12 pickups");
                if (acquisitions.Length == 1)
                {
                    acquisition = acquisitions[0];
                    acquired = true;
                    break;
                }
            }
            Assert.That(
                acquired,
                Is.True,
                "bounded ordinary source overlap must acquire static family-12 row 38");
            var cold = session.BeyondSightState();
            Assert.That(
                (cold.active, cold.tier, cold.action, cold.activeValue, cold.holderEntityIndex),
                Is.EqualTo(((byte)1, (byte)0, (byte)36, (short)0, source.entityIndex)));
            AssertNoPresentedLevel2Spell(presenter, 12);

            // The f38-to-f1 Castle route is already established above. Poll
            // its public code-7 defender receipt after this late holder has
            // been acquired, then issue the ordinary selected spell command.
            DriveLevel2FirstCastleDefenderReturn(
                session,
                presenter,
                (castlePosition.x, castlePosition.y, castlePosition.z),
                cold.manaCost,
                sourcePassBound,
                castlePosition.rootTick);
            var armed = StepLevel2SpellPresentation(
                session,
                presenter,
                castlePosition.x,
                castlePosition.y,
                castlePosition.z,
                flags: 0x40,
                requestedSpell: 12);
            var beyondSight = session.BeyondSightState();
            Assert.That(beyondSight.active, Is.EqualTo(1));
            Assert.That(beyondSight.tier, Is.EqualTo(0));
            Assert.That(beyondSight.action, Is.EqualTo(36));
            Assert.That(beyondSight.activeValue, Is.GreaterThan(0));
            Assert.That(beyondSight.cleanupRequested, Is.EqualTo(0));
            var armedHud = session.DayHudState(armed.snapshot.revision);
            var armedShield = session.ShieldState();
            var armedInvisible = session.InvisibleState();
            AssertPresentedLevel2Spell(
                presenter,
                12,
                armed.snapshot,
                acquisition,
                beyondSight.holderEntityIndex,
                beyondSight.parentEntityIndex);

            // A cleanup-marked typed snapshot must fail dark even when every
            // other source-owned field still matches the current HUD frame.
            var cleanupRequested = beyondSight;
            cleanupRequested.cleanupRequested = 1;
            presenter.Present(
                armed.snapshot,
                armed.events,
                armedHud,
                armedShield,
                armedInvisible,
                cleanupRequested);
            AssertNoPresentedLevel2Spell(presenter, 12);
            presenter.Present(
                armed.snapshot,
                armed.events,
                armedHud,
                armedShield,
                armedInvisible,
                beyondSight);
            AssertPresentedLevel2Spell(
                presenter,
                12,
                armed.snapshot,
                acquisition,
                beyondSight.holderEntityIndex,
                beyondSight.parentEntityIndex);

            var expired = beyondSight;
            for (var sourcePass = 0;
                 sourcePass < sourcePassBound && expired.activeValue > 0;
                 sourcePass++)
            {
                StepLevel2SpellPresentation(
                    session,
                    presenter,
                    castlePosition.x,
                    castlePosition.y,
                    castlePosition.z,
                    flags: 0);
                expired = session.BeyondSightState();
            }
            Assert.That(expired.activeValue, Is.EqualTo(0));
            AssertNoPresentedLevel2Spell(presenter, 12);

            session.Reset();
            presenter.Reset(session.Generation);
            Assert.That(session.BeyondSightState().active, Is.EqualTo(0));
            Assert.That(presenter.Receipts, Is.Empty);
            Assert.That(presenter.ActiveEffects, Is.Empty);

            // A frame retained from the prior generation cannot revive the
            // reset presentation surface.
            presenter.Present(
                armed.snapshot,
                armed.events,
                armedHud,
                armedShield,
                armedInvisible,
                beyondSight);
            Assert.That(presenter.ActiveEffects, Is.Empty);
        }

        [Test]
        public void Level2CampaignInvisiblePickupUsesManagedNativeSourcePosition()
        {
            using var session = NativeCampaignSession.OpenLevel2(Level2Package);

            Assert.That(session.SpellPickups(), Is.Empty);
            var objective = session.ObjectiveStageState();
            session.Step((ushort)objective.axisX, (ushort)objective.axisY, 7);
            session.Step((ushort)objective.axisX, (ushort)objective.axisY, 7);
            var source = session.SpellPickups()
                .Single(pickup => pickup.authoredIndex == 156);
            Assert.That(source.abiVersion, Is.EqualTo(NativeSim.AbiVersion));
            Assert.That(source.structSize, Is.EqualTo(64));
            Assert.That(source.family, Is.EqualTo(11));
            Assert.That(source.action, Is.EqualTo(34));
            Assert.That(source.active, Is.EqualTo(1));

            NativeSim.CampaignStepSnapshot snapshot = default;
            NativeSim.CampaignEvent[] events = null;
            for (var turn = 0; turn < 4; turn++)
            {
                (snapshot, events) = session.Step(source.x, source.y, source.z);
                if (events.Any(eventRow =>
                        eventRow.kind == NativeSim.CampaignEventSpellAcquisition &&
                        eventRow.family == 11 &&
                        eventRow.sourceSlot == source.sourceEntityIndex &&
                        eventRow.flags == 1))
                {
                    break;
                }
            }

            Assert.That(snapshot.generation, Is.EqualTo(0));
            Assert.That(events.Any(eventRow =>
                    eventRow.abiVersion == NativeSim.AbiVersion &&
                    eventRow.structSize == 48 &&
                    eventRow.generation == session.Generation &&
                    eventRow.kind == NativeSim.CampaignEventSpellAcquisition &&
                    eventRow.family == 11 &&
                    eventRow.sourceSlot == source.sourceEntityIndex &&
                    eventRow.flags == 1), Is.True);

            var acquired = session.InvisibleState();
            Assert.That(acquired.active, Is.EqualTo(1));
            Assert.That(acquired.action, Is.EqualTo(33));
            Assert.That(acquired.holderEntityIndex, Is.EqualTo(source.sourceEntityIndex));
            Assert.That(acquired.generation, Is.EqualTo(session.Generation));

            session.Reset();
            Assert.That(session.SpellPickups(), Is.Empty);
            Assert.That(session.InvisibleState().active, Is.EqualTo(0));
        }

        [Test]
        public void Level2CampaignOpponentSnapshotBridgeIsReachableAndResetBound()
        {
            using var session = NativeCampaignSession.OpenLevel2(Level2Package);
            var state = session.OpponentState();
            Assert.That(state.abiVersion, Is.EqualTo(NativeSim.AbiVersion));
            Assert.That(state.structSize, Is.EqualTo(104));
            Assert.That(state.generation, Is.EqualTo(session.Generation));
            Assert.That(state.active, Is.EqualTo(1));
            Assert.That(state.playerSlot, Is.EqualTo(1));
            Assert.That(state.isAi, Is.EqualTo(1));
            Assert.That(state.colorIndex, Is.EqualTo(1));
            Assert.That(state.entityIndex, Is.EqualTo(112));
            Assert.That(state.entityClass, Is.EqualTo(3));
            Assert.That(state.entityModel, Is.EqualTo(1));
            Assert.That(state.entityAction, Is.EqualTo(1));
            Assert.That(state.fireballHolderIndex, Is.EqualTo(113));
            Assert.That(state.fireballAction, Is.EqualTo(0));
            Assert.That(state.possessionHolderIndex, Is.EqualTo(114));
            Assert.That(state.possessionAction, Is.EqualTo(3));
            Assert.That(state.objectiveRelationDomain, Is.EqualTo(2));
            Assert.That(state.objectiveRelationValue, Is.EqualTo(112));
            Assert.That(state.visibilityFlags, Is.EqualTo(2u));

            session.Reset();
            var reset = session.OpponentState();
            Assert.That(reset.generation, Is.EqualTo(session.Generation));
            Assert.That(reset.active, Is.EqualTo(1));
            Assert.That(reset.entityIndex, Is.EqualTo(112));
        }
    }
}
