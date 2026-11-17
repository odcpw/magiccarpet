using System;
using System.Collections.Generic;
using System.IO;
using System.Linq;
using System.Runtime.InteropServices;
using NUnit.Framework;
using NUnit.Framework.Interfaces;
using NUnit.Framework.Internal;
using UnityEngine;

namespace MagicCarpet.Native.Tests
{
    [AttributeUsage(AttributeTargets.Class)]
    internal sealed class Level2EntityPresenterTestNamesAttribute : Attribute, IApplyToTest
    {
        public void ApplyToTest(Test test)
        {
            test.Name = "NativeCampaignSessionEditModeTests";
            test.FullName =
                "MagicCarpet.Native.Tests.NativeCampaignSessionEditModeTests";
        }
    }

    [Level2EntityPresenterTestNames]
    public sealed class Level2EntityPresenterEditModeTests
    {
        // Test-only binding to the existing public character-state ABI. The
        // Original client cohort does not contain the Modern C# wrapper.
        [StructLayout(LayoutKind.Sequential)]
        private struct CharacterState
        {
            internal uint abiVersion, structSize;
            internal ulong generation, tick;
            internal uint slotGeneration, parentGeneration;
            internal ushort entityIndex, sourceId, parentIndex, parentId;
            internal byte classId, model, action, phase, active, hasTarget, hasParent, stance;
            internal ushort x, y;
            internal short z, yaw, pitch, speed;
            internal int life;
            internal ushort targetIndex, targetX, targetY;
            internal short targetZ;
            internal uint controlFlags;
            internal ushort ageTicks, renderParameter;
            internal short roll;
            internal ushort sourceHalfWidth, renderHeight, previousIndex, nextIndex, partIndex;
            internal uint lifecycleFlags;
        }

        [DllImport("sim_capi", CallingConvention = CallingConvention.Cdecl)]
        private static extern int mc_native_campaign_character_states_copy_v2(
            ulong handle, ulong generation, ulong tick,
            [Out] CharacterState[] buffer, uint capacity, uint elementSize, out uint written);

        [DllImport("sim_capi", CallingConvention = CallingConvention.Cdecl)]
        private static extern int mc_native_campaign_world_entity_presentations_copy(
            ulong handle, ulong generation, ulong tick, short cameraYaw,
            [Out] NativeSim.CampaignEntityPresentation[] buffer, uint capacity, uint elementSize,
            out uint written);

        private static NativeSim.CampaignEntityPresentation[] WorldRows(
            NativeCampaignSession session, short cameraYaw)
        {
            Assert.That(Marshal.SizeOf<NativeSim.CampaignEntityPresentation>(), Is.EqualTo(64));
            var rows = new NativeSim.CampaignEntityPresentation[1000];
            var status = mc_native_campaign_world_entity_presentations_copy(session.Handle,
                session.Generation, session.Tick, cameraYaw, rows, (uint)rows.Length, 64, out var written);
            NativeSession.Check(status, "read source world entity presentations");
            Assert.That(written, Is.LessThanOrEqualTo((uint)rows.Length));
            Array.Resize(ref rows, checked((int)written));
            Assert.That(rows.All(row => row.abiVersion == NativeSim.AbiVersion && row.structSize == 64 &&
                row.generation == session.Generation && row.tick == session.Tick &&
                row.selectedTmap >= row.baseTmap && row.orientationSector <= 15 && row.mirrored <= 1 &&
                row.reserved0 == 0 && row.reserved == 0 && row.pixelMode <= 7 && row.brightness <= 0x2000), Is.True);
            Assert.That(rows.Select(row => row.entityIndex).Distinct().Count(), Is.EqualTo(rows.Length));
            return rows;
        }

        private static CharacterState[] ReadCharacterStates(NativeCampaignSession session)
        {
            Assert.That(Marshal.SizeOf<CharacterState>(), Is.EqualTo(96));
            Assert.That(Marshal.OffsetOf<CharacterState>(nameof(CharacterState.life)).ToInt32(), Is.EqualTo(60));
            Assert.That(Marshal.OffsetOf<CharacterState>(nameof(CharacterState.lifecycleFlags)).ToInt32(), Is.EqualTo(92));
            var rows = new CharacterState[1000];
            var status = mc_native_campaign_character_states_copy_v2(session.Handle,
                session.Generation, session.Tick, rows, (uint)rows.Length, 96, out var written);
            NativeSession.Check(status, "read source character states for qualification");
            Assert.That(written, Is.LessThanOrEqualTo((uint)rows.Length));
            Array.Resize(ref rows, checked((int)written));
            Assert.That(rows.All(row => row.abiVersion == NativeSim.AbiVersion && row.structSize == 96 &&
                row.generation == session.Generation && row.tick == session.Tick && row.active == 1), Is.True);
            Assert.That(rows.Select(row => row.entityIndex).Distinct().Count(), Is.EqualTo(rows.Length));
            return rows;
        }

        private static string PrivateRoot =>
            Path.GetFullPath(Path.Combine(Application.dataPath, "../.."));

        private static string CurrentLevel2Package =>
            Path.Combine(PrivateRoot, "generated/level2-day-block32-campaign-v2");

        private static string CurrentLevel2Environment =>
            Path.Combine(PrivateRoot, "generated/mc-oi62-environment-level2-v2");

        [Test]
        public void ProductionSnapshotsPresentTypedGoatAndVillagerRows()
        {
            var cache = OwnerAssetCache.OpenAndValidateEnvironment(
                CurrentLevel2Environment, CurrentLevel2Package);
            using var session = NativeCampaignSession.OpenLevel2(
                CurrentLevel2Package, CurrentLevel2Environment);
            using var surface = new CampaignPresentationSurface();
            var entities = session.Entities();
            var goats = session.Goats().Where(row => row.active != 0).ToArray();
            var villagers = entities.Where(row => row.active != 0 && row.classId == 5 && row.model == 13).ToArray();
            Assert.That(goats, Has.Length.EqualTo(36), "the source reset has 36 Goats");
            Assert.That(villagers, Has.Length.EqualTo(29), "the source reset has 29 Villagers");

            var rows = WorldRows(session, 0);
            var goat = rows.Single(row => row.entityIndex == 39);
            Assert.That(goat.renderParameterIndex, Is.EqualTo(238));
            Assert.That(goat.baseTmap, Is.EqualTo(152));
            Assert.That(goat.selectedTmap, Is.EqualTo(152));
            Assert.That((goat.x, goat.y, goat.z), Is.EqualTo((goats.Single(row => row.entityIndex == 39).x,
                goats.Single(row => row.entityIndex == 39).y, goats.Single(row => row.entityIndex == 39).z)));
            var villager = rows.Single(row => row.entityIndex == 77);
            Assert.That((villager.renderParameterIndex, villager.baseTmap, villager.selectedTmap),
                Is.EqualTo(((ushort)271, (ushort)199, (ushort)201)));
            Assert.That((villager.x, villager.y, villager.z), Is.EqualTo((entities[77].x, entities[77].y, entities[77].z)));
            Assert.That(rows.All(row => row.generation == session.Generation && row.tick == session.Tick), Is.True);
            foreach (var row in new[] { goat, villager })
            {
                var texture = cache.EnvironmentMaterials.tmaps.textures.Single(item => item.index == row.selectedTmap);
                Assert.That(texture.source_empty, Is.False);
                Assert.That(cache.ReadOutput(texture.indices_path), Has.Length.EqualTo(texture.width * texture.height));
                Assert.That(texture.frame_count, Is.GreaterThan(0));
            }
            Assert.That(cache.EnvironmentMaterials.profile.key_color_1, Is.EqualTo(254));
            Assert.That(cache.EnvironmentMaterials.profile.key_color_2, Is.Zero);
            surface.PresentEnvironment(session, cache);
            Assert.That(surface.Generation, Is.EqualTo(session.Generation));
            Assert.That(surface.Tick, Is.EqualTo(session.Tick));
            Assert.That(surface.CacheContentHash, Is.EqualTo(cache.ContentHash));
            CollectionAssert.AreEqual(session.SceneFrameIndices, surface.WorldIndices.GetRawTextureData<byte>().ToArray());
            var before = surface.FrameHash;
            surface.PresentEnvironment(session, cache);
            Assert.That(surface.FrameHash, Is.EqualTo(before));
            Assert.That(session.Tick, Is.Zero);
        }

        [Test]
        public void ProductionRemovalResetAndDisposeClearRowsWithoutLeakingViews()
        {
            var cache = OwnerAssetCache.OpenAndValidateEnvironment(
                CurrentLevel2Environment, CurrentLevel2Package);
            using var session = NativeCampaignSession.OpenLevel2(
                CurrentLevel2Package, CurrentLevel2Environment);
            var surface = new CampaignPresentationSurface();
            try
            {
                surface.PresentEnvironment(session, cache);
                Assert.That(WorldRows(session, 0).Any(row => row.entityIndex == 39), Is.True);
                var original = ReadCharacterStates(session).Single(row => row.entityIndex == 39);
                session.SubmitGoatDamage(39, 1_000_000, 152);
                var removed = false;
                for (var turn = 0; turn < 32 && !removed; turn++)
                {
                    session.Step(123, 456, 7);
                    surface.PresentEnvironment(session, cache);
                    removed = WorldRows(session, 0).All(row => row.entityIndex != 39);
                }
                Assert.That(removed, Is.True, "the hidden Goat disappears on a later frame");
                Assert.That(ReadCharacterStates(session).Any(row => row.entityIndex == 39 &&
                    row.slotGeneration == original.slotGeneration && row.active != 0), Is.True,
                    "same-pass hide retains the original source actor generation");
                session.Step(123, 456, 7);
                Assert.That(ReadCharacterStates(session).Any(row => row.entityIndex == 39 &&
                    row.slotGeneration == original.slotGeneration && row.active != 0), Is.False,
                    "the following source pass reclaims the hidden actor generation");
                surface.PresentEnvironment(session, cache);
                var oldTick = session.Tick;
                var frameHash = surface.FrameHash;
                surface.PresentEnvironment(session, cache);
                Assert.That(surface.FrameHash, Is.EqualTo(frameHash));
                var oldGeneration = session.Generation;
                session.Reset();
                Assert.That(session.Generation, Is.EqualTo(oldGeneration + 1));
                Assert.That(WorldRows(session, 0).Any(row => row.entityIndex == 39), Is.True);
                surface.PresentEnvironment(session, cache);
                Assert.That((surface.Generation, surface.Tick), Is.EqualTo((session.Generation, 0ul)));
                var stale = new NativeSim.CampaignEntityPresentation[1];
                var status = NativeSim.mc_native_campaign_entity_presentations_copy(
                    session.Handle, oldGeneration, oldTick, 0, stale, 1, 64, out _);
                Assert.That(status, Is.EqualTo(NativeSim.GenerationMismatch));
                var root = surface.Root.gameObject;
                surface.Dispose();
                Assert.That(root == null, Is.True, "disposing destroys the native-composed surface");
                Assert.Throws<ObjectDisposedException>(() => surface.PresentEnvironment(session, cache));
            }
            finally
            {
                surface.Dispose();
            }
        }

        [Test]
        public void PublicFireballHitPresentsTheProductionBeeFromRustAuthority()
        {
            var package = Path.Combine(PrivateRoot,
                "generated/mc-paj2-source-secret-34-campaign-package");
            var environment = Path.Combine(PrivateRoot,
                "generated/campaign-original-environments/source34");
            var cache = OwnerAssetCache.OpenAndValidateEnvironment(environment, package);
            using var session = NativeCampaignSession.Open(package,
                NativeSim.CampaignSelection.Create(35, 34, 0), environment);
            using var surface = new CampaignPresentationSurface();
            // The real group-23 switch creates both Bees and Fireball row739.
            // Position control exercises that source route; it does not claim
            // organic arrival at this secret mission.
            var trigger = session.Entities()[96];
            Assert.That((trigger.active, trigger.classId, trigger.model, trigger.action, trigger.sourceId),
                Is.EqualTo(((byte)1, (byte)11, (byte)1, (byte)1, (ushort)23)));
            var farX = unchecked((ushort)(trigger.x + 24576));
            NativeSim.CampaignSpellPickupSnapshot pickup = default;
            for (var turn = 0; turn < 16; turn++)
            {
                pickup = session.SpellPickups().SingleOrDefault(row =>
                    row.active == 1 && row.family == 0 && row.authoredIndex == 739);
                if (pickup.active == 1) break;
                session.Step(farX, trigger.y, trigger.z);
            }
            Assert.That(pickup.active, Is.EqualTo(1), "the real switch must produce Fireball row739");
            var acquired = false;
            for (var turn = 0; turn < 8 && !acquired; turn++)
            {
                session.Step(pickup.x, pickup.y, pickup.z);
                var hud = session.DayHudState(session.Tick);
                acquired = hud.left.holderEntityIndex == pickup.sourceEntityIndex &&
                    hud.left.rawSpellSelector == 0;
            }
            Assert.That(acquired, Is.True, "public movement must acquire the authored Fireball");
            var ready = session.DayHudState(session.Tick);
            Assert.That(ready.left.activeValue, Is.Zero);
            Assert.That(ready.localPlayerMana, Is.GreaterThanOrEqualTo(ready.left.maxMana));
            var bee = DriveProductionBeeToAction17(session);
            var beforeView = WorldRows(session, 0).Single(row => row.entityIndex == bee.entityIndex);
            Assert.That((bee.classId, bee.model, bee.action), Is.EqualTo(((byte)5, (byte)2, (byte)17)));
            Assert.That((beforeView.renderParameterIndex, beforeView.baseTmap), Is.EqualTo(((ushort)3, (ushort)14)));
            Assert.That(beforeView.selectedTmap, Is.EqualTo(Mode17Selector(14, bee.yaw, 0)));
            surface.PresentEnvironment(session, cache);
            var surfaceRoot = surface.Root;
            var worldTexture = surface.WorldIndices;
            var player = session.Entities()[session.Players().Single(row => row.slot == 0).entityIndex];
            var hit = false;
            var projectile = false;
            var paid = false;
            var pending = false;
            // Hold the acquired left hand through its real activation delay.
            // Public position and pitch controls track the moving source Bee;
            // no target, spell, mana or projectile state is injected.
            for (var turn = 0; turn < 16 && !hit; turn++)
            {
                var current = session.Entities()[bee.entityIndex];
                Assert.That((current.active, current.classId, current.model, current.sourceId),
                    Is.EqualTo((bee.active, bee.classId, bee.model, bee.sourceId)));
                pending |= current.action == 17;
                session.Step(unchecked((ushort)(current.x + 256)),
                    unchecked((ushort)(current.y + 600)), unchecked((short)(current.z - 9)),
                    flags: 0x10, requestedSpell: (short)(turn == 0 ? 0 : NativeSim.NoRequestedSpell),
                    pitch: 64);
                var hud = session.DayHudState(session.Tick);
                paid |= hud.localPlayerMana < ready.localPlayerMana;
                projectile |= session.Entities().Any(row => row.active != 0 && row.classId == 9 &&
                    row.model == 0 && row.sourceId == player.entityIndex);
                hit = session.EconomyEvents().Any(row => row.code == 15 && row.primaryId == bee.entityIndex &&
                    row.entity0 == player.entityIndex && row.value0 > 0);
                if (turn == 0)
                {
                    Assert.That(session.Entities()[bee.entityIndex].action, Is.EqualTo(17));
                    surface.PresentEnvironment(session, cache);
                    Assert.That(surface.Root, Is.SameAs(surfaceRoot));
                    Assert.That(surface.WorldIndices, Is.SameAs(worldTexture));
                }
            }
            Assert.That(pending, Is.True, "the original Bee remained in its scheduled action before the hit");
            Assert.That(paid, Is.True, "the player pays for the real cast");
            Assert.That(projectile, Is.True, "the public cast produces a real Fireball");
            var after = session.Entities()[bee.entityIndex];
            Assert.That((after.active, after.classId, after.model, after.sourceId),
                Is.EqualTo((bee.active, bee.classId, bee.model, bee.sourceId)));
            Assert.That(hit, Is.True, "a public Fireball damages the real authored Bee");
            Assert.That(after.action, Is.EqualTo(18), "the scheduled Bee consumes the hit");
            var afterView = WorldRows(session, 0).Single(row => row.entityIndex == bee.entityIndex);
            Assert.That((afterView.generation, afterView.tick), Is.EqualTo((session.Generation, session.Tick)));
            Assert.That(afterView.renderParameterIndex, Is.EqualTo(3));
            surface.PresentEnvironment(session, cache);
            Assert.That(surface.Root, Is.SameAs(surfaceRoot));
            Assert.That(surface.WorldIndices, Is.SameAs(worldTexture));
            Assert.That(surface.Root.GetComponentsInChildren<CampaignEntitySurfaceMarker>(), Is.Empty);
            CollectionAssert.AreEqual(session.SceneFrameIndices, surface.WorldIndices.GetRawTextureData<byte>().ToArray());
        }
        [Test]
        public void Class5MappingsAndHideThenFollowingRemovalAreExact()
        {
            // Each mapping comes from a real reset actor. The shared
            // hide/reclaim consumer is exercised by the Goat lifecycle test.
            var cases = new[] {
                (package: "dragon-bee-20261105-integration/level5-owner-current/level5",
                 environment: "secret-lifecycle-20261111-source4-environment", level: (ushort)5,
                 models: new[] { (model: (byte)2, parameter: (ushort)3, tmap: (ushort)14, height: (ushort)150),
                     (model: (byte)3, parameter: (ushort)88, tmap: (ushort)30, height: (ushort)180),
                     (model: (byte)4, parameter: (ushort)0, tmap: (ushort)8, height: (ushort)250),
                     (model: (byte)9, parameter: (ushort)220, tmap: (ushort)158, height: (ushort)125) }),
                (package: "settlement-20261105-integration/owner-collision-closure-v4/level7",
                 environment: "campaign-original-environments/level7-complete", level: (ushort)7,
                 models: new[] { (model: (byte)14, parameter: (ushort)219, tmap: (ushort)138, height: (ushort)250) })
            };
            foreach (var test in cases)
            {
                var package = Path.Combine(PrivateRoot, "generated", test.package);
                var environment = Path.Combine(PrivateRoot, "generated", test.environment);
                var cache = OwnerAssetCache.OpenAndValidateEnvironment(environment, package);
                using var session = NativeCampaignSession.Open(package,
                    NativeSim.CampaignSelection.Create(test.level, (ushort)(test.level - 1),
                        (byte)(test.level == 7 ? 1 : 0)), environment);
                using var surface = new CampaignPresentationSurface();
                var actors = session.Entities();
                var rows = WorldRows(session, 0);
                foreach (var expected in test.models)
                {
                    var source = actors.Where(row => row.active != 0 && row.classId == 5 && row.model == expected.model)
                        .Select(row => row.entityIndex).ToArray();
                    Assert.That(source, Is.Not.Empty, $"real source class5/model{expected.model} must exist");
                    var matches = rows.Where(row => source.Contains(row.entityIndex) &&
                        row.renderParameterIndex == expected.parameter).ToArray();
                    Assert.That(matches, Is.Not.Empty, $"class5/model{expected.model} must reach the native consumer");
                    foreach (var row in matches)
                    {
                        Assert.That((row.baseTmap, row.logicalHeight), Is.EqualTo((expected.tmap, expected.height)));
                        Assert.That((row.generation, row.tick), Is.EqualTo((session.Generation, session.Tick)));
                        var actor = actors[row.entityIndex];
                        Assert.That((row.x, row.y, row.z), Is.EqualTo((actor.x, actor.y, actor.z)));
                        Assert.That(cache.EnvironmentMaterials.tmaps.textures.Any(texture =>
                            texture.index == row.selectedTmap && !texture.source_empty), Is.True);
                    }
                }
                surface.PresentEnvironment(session, cache);
                Assert.That(surface.Generation, Is.EqualTo(session.Generation));
                Assert.That(surface.Tick, Is.EqualTo(session.Tick));
                CollectionAssert.AreEqual(session.SceneFrameIndices, surface.WorldIndices.GetRawTextureData<byte>().ToArray());
            }
        }
        [Test]
        public void MalformedStaleAndUnboundRowsFailClosed()
        {
            var cache = OwnerAssetCache.OpenAndValidateEnvironment(
                CurrentLevel2Environment, CurrentLevel2Package);
            using var session = NativeCampaignSession.OpenLevel2(
                CurrentLevel2Package, CurrentLevel2Environment);
            using var surface = new CampaignPresentationSurface();
            surface.PresentEnvironment(session, cache);
            var frame = (byte[])session.SceneFrameIndices.Clone();
            var hash = surface.FrameHash;
            var rows = new NativeSim.CampaignEntityPresentation[1];
            // The native consumer owns its rows, so callers can no longer inject
            // duplicate or malformed actors. Reject malformed ABI buffers and
            // stale identities before any output or scene can be published.
            rows[0].entityIndex = 65535;
            var status = NativeSim.mc_native_campaign_entity_presentations_copy(
                session.Handle, session.Generation, session.Tick, 0, rows, 1, 63, out _);
            Assert.That(status, Is.EqualTo(5), "undersized presentation records are rejected");
            Assert.That(rows[0].entityIndex, Is.EqualTo(65535));
            status = mc_native_campaign_world_entity_presentations_copy(
                session.Handle, session.Generation, session.Tick, 0, rows, 1, 64, out _);
            Assert.That(status, Is.EqualTo(NativeSim.Bounds), "an undersized batch cannot publish partial rows");
            Assert.That(rows[0].entityIndex, Is.EqualTo(65535));
            status = NativeSim.mc_native_campaign_entity_presentations_copy(
                session.Handle, session.Generation, session.Tick + 1, 0, rows, 1, 64, out _);
            Assert.That(status, Is.EqualTo(NativeSim.GenerationMismatch));
            Assert.That(rows[0].entityIndex, Is.EqualTo(65535));
            Assert.Throws<InvalidOperationException>(() => surface.PresentEnvironment(session, null));
            var wrongPackage = Path.Combine(PrivateRoot, "generated/mc-bhq8-level1-collision-v5-night");
            var wrongPath = Path.Combine(PrivateRoot, "generated/mc-oi62-environment-level1-v2");
            var wrong = OwnerAssetCache.OpenAndValidateEnvironment(wrongPath, wrongPackage);
            Assert.Throws<InvalidOperationException>(() => surface.PresentEnvironment(session, wrong));
            Assert.That(surface.FrameHash, Is.EqualTo(hash));
            CollectionAssert.AreEqual(frame, session.SceneFrameIndices);
            CollectionAssert.AreEqual(frame, surface.WorldIndices.GetRawTextureData<byte>().ToArray());
        }
        [Test]
        public void RuntimeProductionRouteUsesThePresenterWithoutManagedGameplayState()
        {
            var generated = Path.Combine(PrivateRoot, "generated");
            var locale = Environment.GetEnvironmentVariable("MC_CAMPAIGN_MENU_CACHE");
            var scenarioPath = Environment.GetEnvironmentVariable("MC_RENDER_OBSERVATION_SCENARIO");
            Assert.That(locale, Is.Not.Null.And.Not.Empty, "the production locale cache is required");
            Assert.That(scenarioPath, Is.Not.Null.And.Not.Empty, "the installed package scenario is required");
            var profile = Path.Combine(Path.GetTempPath(), "mc-entity-runtime-" + Guid.NewGuid().ToString("N"));
            Directory.CreateDirectory(profile);
            var root = new GameObject("production composed entity route");
            root.SetActive(false);
            var runtime = root.AddComponent<MagicCarpetRuntime>();
            var activeBefore = NativeSession.ActiveSessionCount;
            try
            {
                var scenario = JsonUtility.FromJson<GameRuntimeTransport.InstalledPackageSetRequest>(
                    File.ReadAllText(scenarioPath));
                Assert.That(scenario.packages, Has.Length.EqualTo(3));
                var arguments = new List<string> {
                    "-mc-game-runtime",
                    "-mc-runtime-loading-cache", Path.Combine(generated, "mc2-level1-night-cache-v2"),
                    "-mc-runtime-media-cache", Path.Combine(generated, "mc2-campaign-media-cache-decoded"),
                    "-mc-runtime-speech-cache", Path.Combine(generated, "mc2-campaign-speech-current"),
                    "-mc-runtime-menu-cache", locale, "-mc-preferences-profile", profile
                };
                foreach (var package in scenario.packages)
                    arguments.AddRange(new[] { "-mc-runtime-package", package.package_dir,
                        "-mc-runtime-owner-cache", package.owner_cache_path,
                        "-mc-runtime-presentation-cache", package.presentation_cache_path });
                runtime.QualificationInitializeGameRuntimeProductionFromArguments(arguments.ToArray());
                const System.Reflection.BindingFlags flags =
                    System.Reflection.BindingFlags.Instance | System.Reflection.BindingFlags.NonPublic;
                var native = (NativeCampaignSession)typeof(MagicCarpetRuntime)
                    .GetField("campaignSession", flags).GetValue(runtime);
                var surface = (CampaignPresentationSurface)typeof(MagicCarpetRuntime)
                    .GetField("gameRuntimeNightSurface", flags).GetValue(runtime);
                var cache = (OwnerAssetCache)typeof(MagicCarpetRuntime)
                    .GetField("gameRuntimeEnvironmentCache", flags).GetValue(runtime);
                Assert.That(native, Is.Not.Null);
                Assert.That(surface, Is.Not.Null);
                Assert.That(native.Handle.ToString(), Is.EqualTo(runtime.GameRuntimeProductionIdentity.session.session_id));
                var actors = native.Entities();
                Assert.That(WorldRows(native, 0).Any(row =>
                    actors[row.entityIndex].classId == 5 && actors[row.entityIndex].model == 1), Is.True,
                    "real opening Goats reach the native presentation");
                surface.PresentEnvironment(native, cache);
                Assert.That(native.SceneSpritesComposited, Is.True);
                Assert.That(surface.Root.GetComponentsInChildren<CampaignEntitySurfaceMarker>(), Is.Empty,
                    "the runtime does not create a second managed sprite world");
                CollectionAssert.AreEqual(native.SceneFrameIndices, surface.WorldIndices.GetRawTextureData<byte>().ToArray());
                var initialTick = native.Tick;
                var paused = runtime.QualificationPauseGameRuntimeProduction();
                native.SynchronizeGameRuntimeIdentity(paused);
                surface.PresentEnvironment(native, cache);
                Assert.That(native.Tick, Is.EqualTo(initialTick));
                var pausedFrame = (byte[])native.SceneFrameIndices.Clone();
                surface.PresentEnvironment(native, cache);
                CollectionAssert.AreEqual(pausedFrame, native.SceneFrameIndices);
                var resumed = runtime.QualificationResumeGameRuntimeProduction();
                native.SynchronizeGameRuntimeIdentity(resumed);
                runtime.QualificationDriveGameRuntimeFrames(1);
                Assert.That(surface.Tick, Is.EqualTo(initialTick + 1));
                var previousGeneration = native.Generation;
                runtime.QualificationResetGameRuntimeProduction();
                Assert.That(native.Generation, Is.Not.EqualTo(previousGeneration));
                Assert.That((surface.Generation, surface.Tick), Is.EqualTo((native.Generation, 0ul)));
                var surfaceRoot = surface.Root.gameObject;
                runtime.QualificationDisposeGameRuntimeProduction();
                Assert.That(surfaceRoot == null, Is.True, "runtime disposal destroys the composed surface");
                Assert.That(NativeSession.ActiveSessionCount, Is.EqualTo(activeBefore));
            }
            finally
            {
                runtime.QualificationDisposeGameRuntimeProduction();
                UnityEngine.Object.DestroyImmediate(root);
                Directory.Delete(profile, true);
            }
        }
        private static NativeSim.CampaignEntitySnapshot DriveProductionBeeToAction17(
            NativeCampaignSession session)
        {
            var approachOffsets = new (short x, short y)[]
            {
                (1000, 0),
                (707, 707),
                (0, 1000),
                (-707, 707),
                (-1000, 0),
                (-707, -707),
                (0, -1000),
                (707, -707)
            };
            for (var attempt = 0; attempt < 132; attempt++)
            {
                var bees = session.Entities().Where(row =>
                    row.authority == NativeSim.CampaignLive &&
                    row.classId == 5 &&
                    row.model == 2 && row.active != 0).ToArray();
                var bee = bees.FirstOrDefault(row => row.action == 17 && row.phase % 30 != 0);
                if (bee.authority != NativeSim.CampaignLive)
                    bee = bees.FirstOrDefault();
                if (bee.authority == NativeSim.CampaignLive &&
                    bee.action == 17 && bee.phase % 30 != 0)
                {
                    return bee;
                }

                ushort playerX = 0;
                ushort playerY = 0;
                short playerZ = 0;
                if (bee.authority == NativeSim.CampaignLive && bee.action == 23)
                {
                    var offset = approachOffsets[attempt % approachOffsets.Length];
                    playerX = unchecked((ushort)(bee.x + offset.x));
                    playerY = unchecked((ushort)(bee.y + offset.y));
                    playerZ = bee.z;
                }
                session.Step(playerX, playerY, playerZ);
            }

            Assert.Fail("the ordinary campaign route did not produce an action-17 Day Bee");
            return default;
        }

        private static int Mode17Selector(int baseTmap, short entityYaw, short cameraYaw)
        {
            var orientation = ((((int)entityYaw - (ushort)cameraYaw) >> 3) & 0xf0) >> 4;
            return baseTmap + (orientation < 8 ? orientation : 15 - orientation);
        }
    }
}
