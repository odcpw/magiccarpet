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
                runtime.QualificationInitializeLevel2Production(Level2Package);

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
                Assert.That(
                    info.resetStateHash,
                    Is.EqualTo(MagicCarpetRuntime.ExpectedLevel2ResetStateHash));
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
        public void Level2ProductionStartupRejectsCrossLevelInputsBeforeRendering()
        {
            var root = new GameObject("Level 2 cross-level startup");
            root.SetActive(false);
            MagicCarpetRuntime runtime = null;
            try
            {
                runtime = root.AddComponent<MagicCarpetRuntime>();
                Assert.Throws<InvalidDataException>(
                    () => runtime.QualificationInitializeLevel2Production(Level1Package));
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
            var reset = session.WorldInfo();
            Assert.That(reset.resetStateHash, Is.EqualTo(initial.resetStateHash));
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
        public void Level2CampaignGoatSidecarsReachManagedProductionPath()
        {
            using var session = NativeCampaignSession.OpenLevel2(Level2Package);

            var target = session.Goats().Single(goat => goat.entityIndex == 39);
            session.SubmitGoatDamage(target.entityIndex, 1_000_000, 152);

            var sawRemoval = false;
            var sawReclaim = false;
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
            }

            Assert.That(sawRemoval, Is.True);
            Assert.That(sawReclaim, Is.True);
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
            Assert.That(acquired.action, Is.EqualTo(36));
            Assert.That(acquired.holderEntityIndex, Is.EqualTo(38));
            Assert.That(acquired.parentEntityIndex, Is.EqualTo(111));
            Assert.That(acquired.generation, Is.EqualTo(session.Generation));

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
            session.Step(123, 456, 7);
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
            var beforeDamage = session.ShieldState();
            session.SubmitShieldDamage(beforeDamage.parentEntityIndex, 100, 152);
            session.Step(source.x, source.y, source.z);
            var afterDamage = session.ShieldState();
            Assert.That(afterDamage.parentLife, Is.EqualTo(beforeDamage.parentLife - 25));
            Assert.That(afterDamage.experienceAwarded, Is.EqualTo(beforeDamage.experienceAwarded + 1));
            Assert.That(afterDamage.holderEntityIndex, Is.EqualTo(source.sourceEntityIndex));
            Assert.That(afterDamage.generation, Is.EqualTo(session.Generation));

            session.Reset();
            Assert.That(session.SpellPickups(), Is.Empty);
            Assert.That(session.ShieldState().active, Is.EqualTo(0));
        }

        [Test]
        public void Level2CampaignInvisiblePickupUsesManagedNativeSourcePosition()
        {
            using var session = NativeCampaignSession.OpenLevel2(Level2Package);

            Assert.That(session.SpellPickups(), Is.Empty);
            session.Step(123, 456, 7);
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

            session.Reset();
            var reset = session.OpponentState();
            Assert.That(reset.generation, Is.EqualTo(session.Generation));
            Assert.That(reset.active, Is.EqualTo(1));
            Assert.That(reset.entityIndex, Is.EqualTo(112));
        }
    }
}
