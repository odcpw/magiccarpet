using System;
using System.Collections.Generic;
using System.IO;
using System.Linq;
using NUnit.Framework;
using UnityEngine;

namespace MagicCarpet.Native.Tests
{
    public sealed class Level2WorldObjectPresenterEditModeTests
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

        [Test]
        public void QualifiedRowsMapToOwnerTmapsAndWrongSubtypesHaveNoView()
        {
            var stone = EntityRow(23, 1, 2, 1, 3, 640, 39040, 16);
            var dolmen = EntityRow(108, 108, 2, 2, 6, 38016, 53632, 0);
            var badStone = EntityRow(33, 33, 2, 3, 9, 640, 52096, 0);
            var building = EntityRow(1, 1, 10, 45, 52, 512, 39168, 0);
            var castle = EntityRow(115, 115, 3, 2, 5, 4096, 8192, 128);

            Assert.That(
                Level2WorldObjectProjection.TryDescribe(
                    stone,
                    out var stoneDescription),
                Is.True);
            Assert.That(
                (stoneDescription.Kind, stoneDescription.AssetSelector),
                Is.EqualTo((Level2WorldObjectKind.Scenery, (ushort)78)));
            Assert.That(
                Level2WorldObjectProjection.TryDescribe(
                    dolmen,
                    out var dolmenDescription),
                Is.True);
            Assert.That(dolmenDescription.AssetSelector, Is.EqualTo((ushort)79));
            Assert.That(
                Level2WorldObjectProjection.TryDescribe(
                    badStone,
                    out var badStoneDescription),
                Is.True);
            Assert.That(badStoneDescription.AssetSelector, Is.EqualTo((ushort)198));
            Assert.That(
                Level2WorldObjectProjection.TryDescribe(
                    building,
                    out var buildingDescription),
                Is.True);
            Assert.That(
                (buildingDescription.Kind, buildingDescription.AssetSelector),
                Is.EqualTo((Level2WorldObjectKind.Building, (ushort)96)));
            Assert.That(
                Level2WorldObjectProjection.TryDescribe(
                    castle,
                    out var castleDescription),
                Is.True);
            Assert.That(
                (castleDescription.Kind, castleDescription.AssetSelector),
                Is.EqualTo((Level2WorldObjectKind.Castle, (ushort)96)));

            foreach (var switchRow in new[]
            {
                EntityRow(23, 23, 11, 0, 0, 640, 39040, 16),
                EntityRow(37, 37, 11, 4, 4, 640, 39040, 16),
                EntityRow(32, 32, 11, 32, 32, 640, 39040, 16)
            })
            {
                Assert.That(
                    Level2WorldObjectProjection.TryDescribe(switchRow, out _),
                    Is.False,
                    "class-11 switch callbacks are not scenery projections");
            }

            var wrongBuilding = building;
            wrongBuilding.action = 51;
            Assert.That(
                Level2WorldObjectProjection.TryDescribe(
                    wrongBuilding,
                    out _),
                Is.False);
            var wrongCastle = castle;
            wrongCastle.model = 1;
            Assert.That(
                Level2WorldObjectProjection.TryDescribe(
                    wrongCastle,
                    out _),
                Is.False);
        }

        [Test]
        public void MalformedOrDuplicateRowsFailClosedBeforeProjection()
        {
            var first = EntityRow(1, 1, 10, 45, 52, 512, 39168, 0);
            var duplicate = first;
            duplicate.sourceId = 99;
            Assert.That(
                Level2WorldObjectProjection.TryProjectRows(
                    new[] { first, duplicate },
                    out _),
                Is.False,
                "one source slot cannot produce two world-object views");

            var malformed = first;
            malformed.structSize = 51;
            Assert.That(
                Level2WorldObjectProjection.TryProjectRows(
                    new[] { malformed },
                    out _),
                Is.False);

            var unknown = first;
            unknown.classId = 10;
            unknown.model = 44;
            unknown.action = 52;
            Assert.That(
                Level2WorldObjectProjection.TryProjectRows(
                    new[] { unknown },
                    out var noPlaceholder),
                Is.True);
            Assert.That(noPlaceholder, Is.Empty);
        }

        [Test]
        public void CastleLifecycleRequiresBoundGenerationAndReclaimsAfterDestroy()
        {
            var state = default(Level2CastleLifecycleState);
            var built = CastleEvent(4, 7, 10, 115, owner: 38);
            Assert.That(
                Level2WorldObjectProjection.TryApplyCastleEvent(
                    built,
                    4,
                    12,
                    dispatchBound: true,
                    ref state),
                Is.True);
            Assert.That(
                (state.SourceSlot, state.OwnerEntityIndex, state.Action,
                    state.Active, state.Lifecycle),
                Is.EqualTo((
                    (ushort)115,
                    (ushort)38,
                    (byte)5,
                    true,
                    Level2WorldObjectLifecycle.Built)));

            var stage = CastleEvent(4, 8, 14, 115, value0: 1, value1: 0);
            Assert.That(
                Level2WorldObjectProjection.TryApplyCastleEvent(
                    stage,
                    4,
                    13,
                    dispatchBound: true,
                    ref state),
                Is.True);
            Assert.That(
                (state.Stage, state.Action, state.Active, state.Lifecycle),
                Is.EqualTo((
                    (byte)0,
                    (byte)6,
                    false,
                    Level2WorldObjectLifecycle.StageDestroyed)));

            var destroyed = CastleEvent(4, 9, 11, 115);
            Assert.That(
                Level2WorldObjectProjection.TryApplyCastleEvent(
                    destroyed,
                    4,
                    14,
                    dispatchBound: true,
                    ref state),
                Is.True);
            Assert.That(state.Active, Is.False);
            Assert.That(
                state.Lifecycle,
                Is.EqualTo(Level2WorldObjectLifecycle.Destroyed));

            var stale = CastleEvent(3, 10, 10, 115, owner: 38);
            Assert.That(
                Level2WorldObjectProjection.TryApplyCastleEvent(
                    stale,
                    4,
                    15,
                    dispatchBound: true,
                    ref state),
                Is.False);
            var unknown = CastleEvent(4, 11, 255, 115);
            Assert.That(
                Level2WorldObjectProjection.TryApplyCastleEvent(
                    unknown,
                    4,
                    16,
                    dispatchBound: true,
                    ref state),
                Is.False);
        }

        [Test]
        public void Level2ProductionProjectsOwnerBackedWorldObjectsAcrossResetAndDispose()
        {
            var root = new GameObject("Level 2 world-object presenter production route");
            root.SetActive(false);
            MagicCarpetRuntime runtime = null;
            try
            {
                runtime = root.AddComponent<MagicCarpetRuntime>();
                runtime.QualificationInitializeLevel2Production(
                    Level2Package,
                    Level2DayCache);

                var initial = root.GetComponentsInChildren<Level2WorldObjectMarker>(true);
                Assert.That(initial, Has.Length.EqualTo(29));
                AssertProjectionSet(initial, generation: 0, tick: 0);
                Assert.That(
                    initial.Count(marker => marker.Kind == Level2WorldObjectKind.Building),
                    Is.EqualTo(22));
                Assert.That(
                    initial.Count(marker => marker.Kind == Level2WorldObjectKind.Scenery),
                    Is.EqualTo(7));

                var initialBySlot = initial.ToDictionary(marker => marker.SourceSlot);
                var initialObjectIds = initial.ToDictionary(
                    marker => marker.SourceSlot,
                    marker => marker.gameObject.GetInstanceID());
                runtime.QualificationDriveLevel2Frames(1);
                var stepped = root.GetComponentsInChildren<Level2WorldObjectMarker>(true);
                Assert.That(stepped, Has.Length.EqualTo(initial.Length));
                AssertProjectionSet(
                    stepped,
                    generation: 0,
                    tick: 1);
                foreach (var marker in stepped)
                {
                    Assert.That(
                        marker.gameObject,
                        Is.SameAs(initialBySlot[marker.SourceSlot].gameObject));
                }

                runtime.QualificationResetLevel2Production();
                var reset = root.GetComponentsInChildren<Level2WorldObjectMarker>(true);
                Assert.That(reset, Has.Length.EqualTo(initial.Length));
                AssertProjectionSet(reset, generation: 1, tick: 0);
                Assert.That(
                    reset.Any(marker =>
                        initialObjectIds[marker.SourceSlot] ==
                        marker.gameObject.GetInstanceID()),
                    Is.False,
                    "reset destroys the old projection before rebuilding it");

                runtime.QualificationDisposeNativeOwners();
                Assert.That(
                    root.GetComponentsInChildren<Level2WorldObjectMarker>(true),
                    Is.Empty);
                Assert.That(
                    root.GetComponentsInChildren<Renderer>(true),
                    Is.Empty,
                    "dispose removes indexed-frame and world-object renderers");
            }
            finally
            {
                runtime?.QualificationDisposeNativeOwners();
                UnityEngine.Object.DestroyImmediate(root);
            }

            Assert.That(NativeCampaignSession.ActiveSessionCount, Is.Zero);
        }

        private static void AssertProjectionSet(
            Level2WorldObjectMarker[] markers,
            ulong generation,
            ulong tick)
        {
            var sourceSlots = new HashSet<ushort>();
            var selectors = new HashSet<ushort> { 78, 79, 96, 198 };
            foreach (var marker in markers)
            {
                Assert.That(sourceSlots.Add(marker.SourceSlot), Is.True);
                Assert.That(marker.Generation, Is.EqualTo(generation));
                Assert.That(marker.Tick, Is.EqualTo(tick));
                Assert.That(marker.Live, Is.True);
                Assert.That(selectors.Contains(marker.AssetSelector), Is.True);
                Assert.That(marker.AssetPath, Does.StartWith("components/material/"));
                var renderer = marker.GetComponent<MeshRenderer>();
                Assert.That(renderer, Is.Not.Null);
                Assert.That(renderer.enabled, Is.True);
                Assert.That(renderer.sharedMaterial, Is.Not.Null);
                Assert.That(renderer.sharedMaterial.GetTexture("_Indices"), Is.Not.Null);
                Assert.That(renderer.sharedMaterial.GetTexture("_Palette"), Is.Not.Null);
                Assert.That(marker.transform.localScale.x, Is.GreaterThan(0f));
                Assert.That(marker.transform.localScale.y, Is.GreaterThan(0f));
            }
        }

        private static NativeSim.CampaignEntitySnapshot EntityRow(
            ushort entityIndex,
            ushort sourceId,
            byte classId,
            byte model,
            byte action,
            ushort x,
            ushort y,
            short z)
        {
            return new NativeSim.CampaignEntitySnapshot
            {
                abiVersion = NativeSim.AbiVersion,
                structSize = 52,
                entityIndex = entityIndex,
                sourceId = sourceId,
                active = 1,
                classId = classId,
                model = model,
                action = action,
                phase = 1,
                x = x,
                y = y,
                z = z,
                reserved = new uint[4]
            };
        }

        private static NativeSim.CampaignEconomyEvent CastleEvent(
            ulong generation,
            uint ordinal,
            byte code,
            ushort sourceSlot,
            ushort owner = 0,
            int value0 = 0,
            int value1 = 0)
        {
            return new NativeSim.CampaignEconomyEvent
            {
                abiVersion = NativeSim.AbiVersion,
                structSize = 64,
                generation = generation,
                ordinal = ordinal,
                pass = 1,
                family = 3,
                code = code,
                sourceSlot = sourceSlot,
                primaryId = sourceSlot,
                entity0 = owner,
                value0 = value0,
                value1 = value1,
                reserved = new uint[4]
            };
        }
    }
}
