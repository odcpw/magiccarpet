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
            Path.Combine(PrivateRoot, "generated/l2-debug3.fJ3PaM/level2");

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
                Assert.That(presenter.LiveViewCount, Is.EqualTo(64));
                Assert.That(originalGoat == null, Is.True);
                var rendererCount = root.GetComponentsInChildren<MeshRenderer>(true).Length;
                Assert.That(presenter.Present(session, cache, 0), Is.True);
                Assert.That(presenter.LiveViewCount, Is.EqualTo(64));
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
    }
}
