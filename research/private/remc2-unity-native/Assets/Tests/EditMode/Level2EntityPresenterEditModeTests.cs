using System;
using System.IO;
using System.Linq;
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
        private static string PrivateRoot =>
            Path.GetFullPath(Path.Combine(Application.dataPath, "../.."));

        private static string Level2Package =>
            Environment.GetEnvironmentVariable("MC_CAMPAIGN_PACKAGE_V1") ??
            Path.Combine(
                PrivateRoot,
                "generated/mc-cf02-campaign-package-v1-a/qualified-level2");

        private static string Level2DayCache =>
            Path.Combine(PrivateRoot, "generated/level2-day-owner-cache");

        [Test]
        public void ProductionSnapshotsPresentTypedGoatAndVillagerRows()
        {
            var root = new GameObject("Level 2 entity presenter test root");
            try
            {
                using var session = NativeCampaignSession.OpenLevel2(
                    Level2Package,
                    Level2DayCache);
                var cache = OwnerAssetCache.OpenAndValidateLevel2(Level2DayCache);
                using var presenter = new Level2EntityPresenter(root.transform);

                Assert.That(
                    presenter.Present(session, cache, 0),
                    Is.True,
                    "the validated native Level 2 frame must reach the presenter");
                Assert.That(presenter.Presentations, Has.Length.EqualTo(65));
                Assert.That(presenter.LiveViewCount, Is.EqualTo(65));
                Assert.That(
                    presenter.Presentations.Count(row =>
                        row.Family == Level2EntityFamily.Goat),
                    Is.EqualTo(36));
                Assert.That(
                    presenter.Presentations.Count(row =>
                        row.Family == Level2EntityFamily.Villager),
                    Is.EqualTo(29));
                Assert.That(
                    presenter.Presentations.All(row =>
                        row.Generation == session.Generation &&
                        row.Tick == session.Tick &&
                        row.Visible),
                    Is.True);

                var goat = presenter.Presentations.Single(row =>
                    row.EntityIndex == 39);
                Assert.That(goat.Family, Is.EqualTo(Level2EntityFamily.Goat));
                Assert.That(goat.RenderParameterIndex, Is.EqualTo(238));
                Assert.That(goat.BaseTmap, Is.EqualTo(152));
                Assert.That(goat.Selector, Is.EqualTo(152));
                Assert.That(
                    goat.SourcePosition,
                    Is.EqualTo(new Vector3(36992f / 64f, 2719f / 64f, 7534f / 64f)));
                Assert.That(goat.Scale.y, Is.EqualTo(150f / 64f));
                Assert.That(goat.Rotation, Is.EqualTo(Quaternion.Euler(0f, 180f, 0f)));

                var villager = presenter.Presentations.Single(row =>
                    row.EntityIndex == 77);
                Assert.That(villager.Family, Is.EqualTo(Level2EntityFamily.Villager));
                Assert.That(villager.RenderParameterIndex, Is.EqualTo(271));
                Assert.That(villager.BaseTmap, Is.EqualTo(199));
                Assert.That(villager.Selector, Is.EqualTo(201));
                Assert.That(
                    villager.SourcePosition,
                    Is.EqualTo(new Vector3(31110f / 64f, 1552f / 64f, 46225f / 64f)));
                Assert.That(villager.Scale.y, Is.EqualTo(300f / 64f));
                Assert.That(villager.Rotation, Is.EqualTo(
                    Quaternion.Euler(0f, 180f - 913f * (360f / 2048f), 0f)));

                var goatObject = goat.GameObject;
                var villagerObject = villager.GameObject;
                Assert.That(
                    presenter.Present(session, cache, 0),
                    Is.True,
                    "re-presenting the same production frame is idempotent");
                Assert.That(
                    presenter.Presentations.Single(row => row.EntityIndex == 39).GameObject,
                    Is.SameAs(goatObject));
                Assert.That(
                    presenter.Presentations.Single(row => row.EntityIndex == 77).GameObject,
                    Is.SameAs(villagerObject));
                Assert.That(
                    root.GetComponentsInChildren<MeshRenderer>(true),
                    Has.Length.EqualTo(65));
            }
            finally
            {
                UnityEngine.Object.DestroyImmediate(root);
            }
        }

        [Test]
        public void ProductionRemovalResetAndDisposeClearRowsWithoutLeakingViews()
        {
            var root = new GameObject("Level 2 entity presenter lifecycle test root");
            try
            {
                using var session = NativeCampaignSession.OpenLevel2(
                    Level2Package,
                    Level2DayCache);
                var cache = OwnerAssetCache.OpenAndValidateLevel2(Level2DayCache);
                using var presenter = new Level2EntityPresenter(root.transform);

                Assert.That(presenter.Present(session, cache, 0), Is.True);
                var originalGoat = presenter.Presentations.Single(row =>
                    row.EntityIndex == 39).GameObject;
                session.SubmitGoatDamage(39, 1_000_000, 152);

                var removed = false;
                for (var turn = 0; turn < 32 && !removed; turn++)
                {
                    session.Step(123, 456, 7);
                    Assert.That(presenter.Present(session, cache, 0), Is.True);
                    removed = presenter.Presentations.All(row =>
                        row.EntityIndex != 39);
                }

                Assert.That(removed, Is.True, "the reclaimed Goat disappears on a later frame");
                Assert.That(
                    presenter.LiveViewCount,
                    Is.EqualTo(70),
                    "the removed Goat leaves the 64 other legacy rows plus six ordinary rooted class-5 rows");
                Assert.That(originalGoat == null, Is.True);
                var rendererCount = root.GetComponentsInChildren<MeshRenderer>(true).Length;
                Assert.That(presenter.Present(session, cache, 0), Is.True);
                Assert.That(presenter.LiveViewCount, Is.EqualTo(70));
                Assert.That(
                    root.GetComponentsInChildren<MeshRenderer>(true),
                    Has.Length.EqualTo(rendererCount));

                var oldGeneration = session.Generation;
                session.Reset();
                Assert.That(session.Generation, Is.EqualTo(oldGeneration + 1));
                Assert.That(presenter.Present(session, cache, 0), Is.True);
                Assert.That(presenter.Presentations, Has.Length.EqualTo(65));
                Assert.That(presenter.Presentations.All(row =>
                    row.Generation == session.Generation), Is.True);

                presenter.Reset(session.Generation);
                Assert.That(presenter.Presentations, Is.Empty);
                Assert.That(presenter.LiveViewCount, Is.Zero);
                presenter.Dispose();
                Assert.That(presenter.Presentations, Is.Empty);
                Assert.That(presenter.LiveViewCount, Is.Zero);
                Assert.That(presenter.Present(session, cache, 0), Is.False);
                Assert.That(
                    root.GetComponentsInChildren<MeshRenderer>(true),
                    Is.Empty);
            }
            finally
            {
                UnityEngine.Object.DestroyImmediate(root);
            }
        }

        [Test]
        public void PublicFireballHitPresentsTheProductionBeeFromRustAuthority()
        {
            var root = new GameObject("Level 2 production Bee presenter root");
            try
            {
                using var session = NativeCampaignSession.OpenLevel2(
                    Level2Package,
                    Level2DayCache);
                var cache = OwnerAssetCache.OpenAndValidateLevel2(Level2DayCache);
                using var presenter = new Level2EntityPresenter(root.transform);

                var bee = DriveProductionBeeToAction17(session);
                Assert.That(presenter.Present(session, cache, 0), Is.True);
                var before = presenter.Presentations.Single(row =>
                    row.EntityIndex == bee.entityIndex);
                Assert.That(before.Family, Is.EqualTo(Level2EntityFamily.Bee));
                Assert.That(before.RenderParameterIndex, Is.EqualTo(3));
                Assert.That(before.BaseTmap, Is.EqualTo(14));
                Assert.That(before.Selector, Is.EqualTo(
                    Mode17Selector(14, bee.yaw, 0)));
                Assert.That(before.Scale.y, Is.EqualTo(150f / 64f));
                Assert.That(before.Visible, Is.True);
                var beeObject = before.GameObject;

                var localPlayer = session.Players().Single(row => row.slot == 0);
                var player = session.Entities().Single(row =>
                    row.entityIndex == localPlayer.entityIndex);
                var playerX = unchecked((ushort)(bee.x + 256));
                var playerY = unchecked((ushort)(bee.y + 360));
                var playerZ = unchecked((short)(bee.z + 280 - player.fov));
                session.Step(
                    playerX,
                    playerY,
                    playerZ,
                    flags: 0x40,
                    requestedSpell: 0);
                session.Step(playerX, playerY, playerZ);

                var pending = session.Entities()[bee.entityIndex];
                Assert.That(pending.authority, Is.EqualTo(NativeSim.CampaignLive));
                Assert.That(pending.active, Is.EqualTo(1));
                Assert.That(pending.classId, Is.EqualTo(5));
                Assert.That(pending.model, Is.EqualTo(2));
                Assert.That(pending.action, Is.EqualTo(17));
                Assert.That(presenter.Present(session, cache, 0), Is.True);
                Assert.That(
                    presenter.Presentations.Single(row =>
                        row.EntityIndex == bee.entityIndex).GameObject,
                    Is.SameAs(beeObject),
                    "a public Fireball mailbox write must not replace Unity's Rust-bound view");

                session.Step(playerX, playerY, playerZ);
                var consumed = session.Entities()[bee.entityIndex];
                Assert.That(consumed.authority, Is.EqualTo(NativeSim.CampaignLive));
                Assert.That(consumed.active, Is.EqualTo(1));
                Assert.That(consumed.action, Is.EqualTo(18));
                Assert.That(presenter.Present(session, cache, 0), Is.True);
                Assert.That(
                    presenter.Presentations.Single(row =>
                        row.EntityIndex == bee.entityIndex).GameObject,
                    Is.SameAs(beeObject));
            }
            finally
            {
                UnityEngine.Object.DestroyImmediate(root);
            }
        }

        [Test]
        public void Class5MappingsAndHideThenFollowingRemovalAreExact()
        {
            var root = new GameObject("Level 2 class-5 presenter mapping root");
            try
            {
                using var session = NativeCampaignSession.OpenLevel2(
                    Level2Package,
                    Level2DayCache);
                var cache = OwnerAssetCache.OpenAndValidateLevel2(Level2DayCache);
                using var presenter = new Level2EntityPresenter(root.transform);
                var baseline = session.Entities();
                var goats = session.Goats();
                var rows = Copy(baseline);
                rows[990] = LiveClass5Row(990, 2, 17, 0, 100, 200, 300);
                rows[991] = LiveClass5Row(991, 3, 25, 0, 110, 210, 310);
                rows[992] = LiveClass5Row(992, 4, 33, 0, 120, 220, 320);
                rows[993] = LiveClass5Row(993, 9, 72, 0, 130, 230, 330);
                rows[994] = LiveClass5Row(994, 14, 113, 0, 140, 240, 340);

                Assert.That(
                    presenter.Present(
                        session.Generation,
                        session.Tick,
                        rows,
                        goats,
                        cache,
                        0),
                    Is.True);
                AssertClass5Presentation(presenter, 990, Level2EntityFamily.Bee, 3, 14, 150);
                AssertClass5Presentation(
                    presenter,
                    991,
                    Level2EntityFamily.Multipart,
                    88,
                    30,
                    180);
                AssertClass5Presentation(
                    presenter,
                    992,
                    Level2EntityFamily.Archer,
                    0,
                    8,
                    250);
                AssertClass5Presentation(
                    presenter,
                    993,
                    Level2EntityFamily.Model9,
                    220,
                    158,
                    125);
                AssertClass5Presentation(
                    presenter,
                    994,
                    Level2EntityFamily.Trader,
                    219,
                    138,
                    250);

                var beeObject = presenter.Presentations.Single(row =>
                    row.EntityIndex == 990).GameObject;
                var hidden = Copy(rows);
                hidden[990].active = 0;
                hidden[990].action = 21;
                Assert.That(
                    presenter.Present(
                        session.Generation,
                        session.Tick + 1,
                        hidden,
                        goats,
                        cache,
                        0),
                    Is.True);
                var hiddenBee = presenter.Presentations.Single(row =>
                    row.EntityIndex == 990);
                Assert.That(hiddenBee.GameObject, Is.SameAs(beeObject));
                Assert.That(hiddenBee.Visible, Is.False,
                    "same-pass hide retains the source-slot view but disables drawing");

                var following = Copy(hidden);
                following[990] = baseline[990];
                Assert.That(
                    presenter.Present(
                        session.Generation,
                        session.Tick + 2,
                        following,
                        goats,
                        cache,
                        0),
                    Is.True);
                Assert.That(presenter.Presentations.All(row => row.EntityIndex != 990), Is.True);
                Assert.That(beeObject == null, Is.True,
                    "the following-boundary source-row removal destroys the Unity view");
            }
            finally
            {
                UnityEngine.Object.DestroyImmediate(root);
            }
        }

        [Test]
        public void MalformedStaleAndUnboundRowsFailClosed()
        {
            var root = new GameObject("Level 2 entity presenter negative test root");
            try
            {
                using var session = NativeCampaignSession.OpenLevel2(
                    Level2Package,
                    Level2DayCache);
                var cache = OwnerAssetCache.OpenAndValidateLevel2(Level2DayCache);
                using var presenter = new Level2EntityPresenter(root.transform);
                var entities = session.Entities();
                var goats = session.Goats();

                Assert.That(presenter.Present(session, cache, 0), Is.True);

                var wrongModel = Copy(entities);
                wrongModel[39].model = 99;
                Assert.That(
                    presenter.Present(
                        session.Generation,
                        session.Tick,
                        wrongModel,
                        goats,
                        cache,
                        0),
                    Is.False);
                Assert.That(presenter.Presentations, Is.Empty);
                Assert.That(presenter.LiveViewCount, Is.Zero);

                var wrongAction = Copy(entities);
                wrongAction[77].action = 1;
                Assert.That(
                    presenter.Present(
                        session.Generation,
                        session.Tick,
                        wrongAction,
                        goats,
                        cache,
                        0),
                    Is.False);
                Assert.That(presenter.Presentations, Is.Empty);

                var unknownLiveClass5 = Copy(entities);
                unknownLiveClass5[990] = LiveClass5Row(990, 2, 99, 0, 1, 2, 3);
                Assert.That(
                    presenter.Present(
                        session.Generation,
                        session.Tick,
                        unknownLiveClass5,
                        goats,
                        cache,
                        0),
                    Is.False,
                    "a live authority row cannot invent an unregistered class-5 action");
                Assert.That(presenter.Presentations, Is.Empty);

                var duplicate = Copy(entities);
                duplicate[77].entityIndex = 76;
                Assert.That(
                    presenter.Present(
                        session.Generation,
                        session.Tick,
                        duplicate,
                        goats,
                        cache,
                        0),
                    Is.False);
                Assert.That(presenter.Presentations, Is.Empty);

                Assert.That(presenter.Present(session, cache, 0), Is.True);
                session.Step(123, 456, 7);
                var currentEntities = session.Entities();
                var currentGoats = session.Goats();
                Assert.That(presenter.Present(session, cache, 0), Is.True);
                Assert.That(
                    presenter.Present(
                        session.Generation,
                        session.Tick - 1,
                        currentEntities,
                        currentGoats,
                        cache,
                        0),
                    Is.False);
                Assert.That(presenter.Presentations, Is.Empty);
                Assert.That(
                    presenter.Present(session, null, 0),
                    Is.False,
                    "a missing validated owner cache cannot create a placeholder sprite");
                Assert.That(presenter.Presentations, Is.Empty);
            }
            finally
            {
                UnityEngine.Object.DestroyImmediate(root);
            }
        }

        [Test]
        public void RuntimeProductionRouteUsesThePresenterWithoutManagedGameplayState()
        {
            var root = new GameObject("Level 2 runtime entity presenter route root");
            root.SetActive(false);
            try
            {
                var runtime = root.AddComponent<MagicCarpetRuntime>();
                runtime.QualificationInitializeLevel2Production(
                    Level2Package,
                    Level2DayCache);

                Assert.That(runtime.QualificationPresentLevel2EntityRows(), Is.True);
                Assert.That(
                    runtime.QualificationLevel2EntityPresentations,
                    Has.Length.EqualTo(65));
                Assert.That(
                    runtime.QualificationLevel2EntityPresentations.Count(row =>
                        row.Family == Level2EntityFamily.Goat),
                    Is.EqualTo(36));
                Assert.That(
                    runtime.QualificationLevel2EntityPresentations.Count(row =>
                        row.Family == Level2EntityFamily.Villager),
                    Is.EqualTo(29));
                runtime.QualificationDisposeNativeOwners();
                Assert.That(runtime.QualificationLevel2EntityPresentations, Is.Empty);
                Assert.That(
                    root.GetComponentsInChildren<MeshRenderer>(true),
                    Is.Empty,
                    "native owner disposal removes the entity presentation hierarchy");
            }
            finally
            {
                UnityEngine.Object.DestroyImmediate(root);
            }
        }

        private static NativeSim.CampaignEntitySnapshot[] Copy(
            NativeSim.CampaignEntitySnapshot[] source)
        {
            var copy = new NativeSim.CampaignEntitySnapshot[source.Length];
            Array.Copy(source, copy, source.Length);
            return copy;
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
                var bee = session.Entities().SingleOrDefault(row =>
                    row.authority == NativeSim.CampaignLive &&
                    row.entityIndex == 119 &&
                    row.classId == 5 &&
                    row.model == 2);
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

        private static NativeSim.CampaignEntitySnapshot LiveClass5Row(
            ushort entityIndex,
            byte model,
            byte action,
            short yaw,
            ushort x,
            ushort y,
            short z)
        {
            return new NativeSim.CampaignEntitySnapshot
            {
                abiVersion = NativeSim.AbiVersion,
                structSize = 52,
                entityIndex = entityIndex,
                sourceId = entityIndex,
                nextRelation = ushort.MaxValue,
                parameterRelation = ushort.MaxValue,
                authority = NativeSim.CampaignLive,
                active = 1,
                classId = 5,
                model = model,
                action = action,
                phase = 1,
                x = x,
                y = y,
                z = z,
                yaw = yaw,
                reserved = new uint[4]
            };
        }

        private static void AssertClass5Presentation(
            Level2EntityPresenter presenter,
            ushort entityIndex,
            Level2EntityFamily family,
            ushort renderParameter,
            int baseTmap,
            int logicalHeight)
        {
            var row = presenter.Presentations.Single(candidate =>
                candidate.EntityIndex == entityIndex);
            Assert.That(row.Family, Is.EqualTo(family));
            Assert.That(row.RenderParameterIndex, Is.EqualTo(renderParameter));
            Assert.That(row.BaseTmap, Is.EqualTo(baseTmap));
            Assert.That(row.Selector, Is.EqualTo(baseTmap));
            Assert.That(row.Scale.y, Is.EqualTo(logicalHeight / 64f));
            Assert.That(row.Visible, Is.True);
        }

        private static int Mode17Selector(int baseTmap, short entityYaw, short cameraYaw)
        {
            var orientation = ((((int)entityYaw - (ushort)cameraYaw) >> 3) & 0xf0) >> 4;
            return baseTmap + (orientation < 8 ? orientation : 15 - orientation);
        }
    }
}
