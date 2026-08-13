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
            Path.Combine(PrivateRoot, "generated/l2-debug3.fJ3PaM/level2");

        private static string Level2DayCache =>
            Path.Combine(PrivateRoot, "generated/level2-day-owner-cache");

        private static string Level1Package =>
            Path.Combine(PrivateRoot, "generated/level1-export-m");

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
                    "identity mismatch",
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

                runtime.QualificationDriveLevel2Frames(1);
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
                AssertDayHudEmpty(initial.left);
                AssertDayHudEmpty(initial.right);
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
                runtime.QualificationDriveLevel2Frames(281);
                var hud = runtime.Level2ProductionDayHud;
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
                AssertDayHudEmpty(hud.right);
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
                AssertDayHudEmpty(reset.left);
                AssertDayHudEmpty(reset.right);
                Assert.That(
                    runtime.QualificationSelectedLevel2DayHudRomanTierTexture(0),
                    Is.Null);
                Assert.That(
                    runtime.Level2ProductionNextStepInput.x,
                    Is.EqualTo((ushort)384));
                Assert.That(
                    runtime.Level2ProductionNextStepInput.y,
                    Is.EqualTo((ushort)62080));
                Assert.That(
                    runtime.Level2ProductionNextStepInput.z,
                    Is.EqualTo((short)256));

                runtime.QualificationDriveLevel2Frames(282);
                var replay = runtime.Level2ProductionDayHud;
                Assert.That(replay.generation, Is.EqualTo(1));
                Assert.That(
                    runtime.Level2ProductionStepSnapshot.stateHash,
                    Is.EqualTo(acquiredStateHash));
                Assert.That(
                    replay.nyphur.visibilityFlags,
                    Is.EqualTo(acquiredNyphurVisibilityFlags));
                Assert.That(runtime.Level2ProductionDayHudPresenterReady, Is.True);
                AssertDayHudFamily12(replay.left, replay.localPlayerEntityIndex);
                AssertDayHudEmpty(replay.right);
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

                runtime.QualificationDriveLevel2Frames(282);
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

                runtime.QualificationDriveLevel2Frames(282);
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

        private static void AssertDayHudEmpty(NativeSim.CampaignDayHudSpellSlot slot)
        {
            Assert.That(slot.rawSpellSelector, Is.LessThan(0));
            Assert.That(slot.holderEntityIndex, Is.Zero);
            Assert.That(slot.parentEntityIndex, Is.Zero);
            Assert.That(slot.frameSelector, Is.Zero);
            Assert.That(slot.iconSelector, Is.Zero);
            Assert.That(slot.levelTextSelector, Is.Zero);
            Assert.That(slot.drawAdmitted, Is.Zero);
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

            var stepped = session.Step(123, 456, 7);
            var goatEvents = stepped.events
                .Where(eventRow => eventRow.kind == NativeSim.CampaignEventGoat)
                .ToArray();

            Assert.That(goatEvents, Is.Not.Empty);
            Assert.That(goatEvents.Select(eventRow => eventRow.sourceSlot), Does.Contain(39));
            Assert.That(goatEvents.Select(eventRow => eventRow.sourceSlot), Does.Contain(71));
            Assert.That(
                goatEvents.All(eventRow =>
                    eventRow.abiVersion == NativeSim.AbiVersion &&
                    eventRow.generation == stepped.snapshot.generation),
                Is.True);
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

            var activated = session.ActivateShield();
            Assert.That(activated.abiVersion, Is.EqualTo(NativeSim.AbiVersion));
            Assert.That(activated.structSize, Is.EqualTo(72));
            Assert.That(activated.active, Is.EqualTo(1));
            Assert.That(activated.action, Is.EqualTo(18));
            Assert.That(activated.activeValue, Is.EqualTo(activated.activationValue));
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
