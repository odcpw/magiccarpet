using System.Collections;
using NUnit.Framework;
using UnityEngine;
using UnityEngine.TestTools;

namespace MagicCarpet.Native.Tests
{
    public sealed class NativeSessionPlayModeTests
    {
        [UnityTest]
        public IEnumerator QualifiedStartupCreatesOneCleanRuntimeAndDisposes()
        {
            yield return null;

            var runtimes = Object.FindObjectsByType<MagicCarpetRuntime>(
                FindObjectsSortMode.None);
            Assert.That(runtimes, Has.Length.EqualTo(1));
            var runtime = runtimes[0];
            Assert.That(
                runtime.QualificationFatalError,
                Is.Null.Or.Empty);
            Assert.That(runtime.QualificationStartupReady, Is.True);
            Assert.That(runtime.QualificationStartupPresentationClean, Is.True);
            Assert.That(NativeSession.ActiveSessionCount, Is.EqualTo(1));
            Assert.That(runtime.QualificationGoatLiveCount, Is.EqualTo(33));
            Assert.That(runtime.QualificationGoatLiveSpriteCount, Is.EqualTo(33));

            runtime.QualificationResetSession();
            const float familyZeroCentreX = 8 + 24;
            const float familyOneCentreX = 8 + 48 + 24;
            const float firstSpellRowCentreY = 408 + 18;
            runtime.QualificationDriveSpellMenuInput(
                true,
                familyOneCentreX,
                firstSpellRowCentreY,
                leftDown: true,
                leftHeld: true);
            Assert.That(runtime.QualificationSpellMenuState, Is.EqualTo(5));
            Assert.That(runtime.QualificationSpellMenuStage, Is.EqualTo(1));
            Assert.That(runtime.QualificationPendingSpellMenuCommands, Is.EqualTo(1));
            yield return new WaitForFixedUpdate();
            Assert.That(
                runtime.QualificationSpellMenuSnapshot.selectedUiIndex,
                Is.EqualTo(1));
            runtime.QualificationDriveSpellMenuInput(
                true,
                familyOneCentreX,
                firstSpellRowCentreY,
                leftHeld: true);
            runtime.QualificationDriveSpellMenuInput(
                true,
                familyOneCentreX,
                firstSpellRowCentreY,
                leftHeld: true);
            Assert.That(
                runtime.QualificationPendingSpellMenuCommands,
                Is.EqualTo(1),
                "render-side held samples coalesce before FixedUpdate");
            yield return new WaitForFixedUpdate();
            runtime.QualificationDriveSpellMenuInput(
                true,
                familyOneCentreX,
                firstSpellRowCentreY,
                leftUp: true);
            Assert.That(runtime.QualificationSpellMenuStage, Is.Zero);
            yield return new WaitForFixedUpdate();
            Assert.That(
                (
                    runtime.QualificationSpellMenuSnapshot.leftFamily,
                    runtime.QualificationSpellMenuSnapshot.leftTier),
                Is.EqualTo(((byte)1, (sbyte)0)));

            runtime.QualificationDriveSpellMenuInput(
                true,
                familyZeroCentreX,
                firstSpellRowCentreY,
                rightDown: true,
                rightHeld: true);
            Assert.That(runtime.QualificationSpellMenuStage, Is.EqualTo(2));
            yield return new WaitForFixedUpdate();
            runtime.QualificationDriveSpellMenuInput(
                true,
                familyZeroCentreX,
                firstSpellRowCentreY,
                rightHeld: true);
            yield return new WaitForFixedUpdate();
            runtime.QualificationDriveSpellMenuInput(
                true,
                familyZeroCentreX,
                firstSpellRowCentreY,
                rightUp: true);
            yield return new WaitForFixedUpdate();
            Assert.That(
                (
                    runtime.QualificationSpellMenuSnapshot.leftFamily,
                    runtime.QualificationSpellMenuSnapshot.leftTier,
                    runtime.QualificationSpellMenuSnapshot.rightFamily,
                    runtime.QualificationSpellMenuSnapshot.rightTier),
                Is.EqualTo(((byte)1, (sbyte)0, (byte)0, (sbyte)0)));
            runtime.QualificationDriveSpellMenuInput(
                false,
                familyZeroCentreX,
                firstSpellRowCentreY);
            Assert.That(runtime.QualificationSpellMenuState, Is.Zero);

            runtime.QualificationResetSession();
            Assert.That(
                runtime.QualificationCycleSpell(false, false),
                Is.False,
                "all-zero reset eligibility is a 26-miss no-op");
            Assert.That(runtime.QualificationPendingSpellMenuCommands, Is.Zero);
            runtime.QualificationDriveSpellMenuInput(
                true,
                familyOneCentreX,
                firstSpellRowCentreY,
                leftDown: true,
                configureEligibility: true);
            yield return new WaitForFixedUpdate();
            Assert.That(
                runtime.QualificationSpellMenuSnapshot.familyHandEligibility,
                Is.EqualTo(new byte[] { 0, 1, 0, 0 }));
            runtime.QualificationDriveSpellMenuInput(
                true,
                familyZeroCentreX,
                firstSpellRowCentreY,
                rightDown: true,
                configureEligibility: true);
            yield return new WaitForFixedUpdate();
            Assert.That(
                runtime.QualificationSpellMenuSnapshot.familyHandEligibility,
                Is.EqualTo(new byte[] { 2, 1, 0, 0 }));
            runtime.QualificationDriveSpellMenuInput(
                false,
                familyZeroCentreX,
                firstSpellRowCentreY);
            Assert.That(runtime.QualificationSpellMenuState, Is.Zero);
            Assert.That(runtime.QualificationCycleSpell(false, false), Is.True);
            yield return new WaitForFixedUpdate();
            Assert.That(
                runtime.QualificationSpellMenuSnapshot.leftFamily,
                Is.EqualTo(1));
            Assert.That(runtime.QualificationCycleSpell(true, true), Is.True);
            yield return new WaitForFixedUpdate();
            Assert.That(
                (
                    runtime.QualificationSpellMenuSnapshot.leftFamily,
                    runtime.QualificationSpellMenuSnapshot.rightFamily),
                Is.EqualTo(((byte)1, (byte)0)));

            runtime.QualificationRequestSpellCast(false);
            yield return new WaitForFixedUpdate();
            Assert.That(runtime.QualificationSpellDirectionMask, Is.EqualTo(256));
            var projectilesBeforeRightCast = runtime.QualificationProjectileCount;
            runtime.QualificationRequestSpellCast(true);
            yield return new WaitForFixedUpdate();
            Assert.That(runtime.QualificationSpellDirectionMask, Is.EqualTo(512));
            Assert.That(
                runtime.QualificationProjectileCount,
                Is.GreaterThan(projectilesBeforeRightCast),
                "right-hand family 0 routes to a native fireball launch");

            runtime.QualificationResetSession();
            var baseline = runtime.QualificationGoatPresentation(50);
            var independentCameraX = (long)baseline.CameraX;
            var independentCameraY = (long)baseline.CameraY;
            Assert.That(baseline.UnwrappedCameraX, Is.EqualTo(independentCameraX));
            Assert.That(baseline.UnwrappedCameraY, Is.EqualTo(independentCameraY));
            AssertGoatPresentationMatchesNative(
                runtime, 50, independentCameraX, independentCameraY);
            Assert.That(
                baseline.RendererEnabled,
                Is.False,
                "the native world frame replaces the duplicate Unity Goat billboard");

            var tickBefore = runtime.QualificationNativeTick;
            var goatHashBefore = runtime.QualificationGoatLiveWorldHash;
            var previousCameraX = baseline.CameraX;
            var previousCameraY = baseline.CameraY;
            for (var tick = 0; tick < 8; tick++)
            {
                yield return new WaitForFixedUpdate();
                var tickProbe = runtime.QualificationGoatPresentation(50);
                independentCameraX += ModularDelta(tickProbe.CameraX, previousCameraX);
                independentCameraY += ModularDelta(tickProbe.CameraY, previousCameraY);
                previousCameraX = tickProbe.CameraX;
                previousCameraY = tickProbe.CameraY;
                AssertGoatPresentationMatchesNative(
                    runtime, 50, independentCameraX, independentCameraY);
            }
            Assert.That(
                runtime.QualificationNativeTick,
                Is.GreaterThan(tickBefore));
            Assert.That(
                runtime.QualificationGoatLiveWorldHash,
                Is.Not.EqualTo(goatHashBefore));
            Assert.That(runtime.QualificationGoatLiveCount, Is.EqualTo(33));
            Assert.That(runtime.QualificationGoatLiveSpriteCount, Is.EqualTo(33));

            var generationBeforeDamage =
                runtime.QualificationGoatPresentation(50).Generation;
            var beforeDamage = runtime.QualificationGoatPresentation(50);
            runtime.QualificationInjectGoatDamage(50, 601);
            var pendingDamage = runtime.QualificationGoatPresentation(50);
            Assert.That(pendingDamage.SnapshotTick, Is.EqualTo(beforeDamage.SnapshotTick));
            Assert.That(pendingDamage.GoatBatchTick, Is.EqualTo(beforeDamage.GoatBatchTick));
            Assert.That(
                pendingDamage.GoatLiveWorldHash,
                Is.Not.EqualTo(beforeDamage.GoatLiveWorldHash));
            Assert.That(pendingDamage.Goat.pendingDamage, Is.EqualTo(601));
            Assert.That(pendingDamage.Goat.active, Is.EqualTo(1));
            Assert.That(pendingDamage.Goat.life, Is.EqualTo(beforeDamage.Goat.life));
            Assert.That(pendingDamage.Goat.action, Is.EqualTo(beforeDamage.Goat.action));
            AssertGoatPresentationMatchesNative(
                runtime, 50, independentCameraX, independentCameraY);

            var reclaimed = false;
            var sawPreKill = false;
            var sawFinalKill = false;
            var sawRemovalQueued = false;
            ulong removalQueuedHash = 0;
            ulong reclaimedHash = 0;
            for (var tick = 0; tick < 16; tick++)
            {
                yield return new WaitForFixedUpdate();
                var probe = runtime.QualificationGoatPresentation(50);
                independentCameraX += ModularDelta(probe.CameraX, previousCameraX);
                independentCameraY += ModularDelta(probe.CameraY, previousCameraY);
                previousCameraX = probe.CameraX;
                previousCameraY = probe.CameraY;
                Assert.That(probe.Generation, Is.EqualTo(generationBeforeDamage));
                AssertGoatPresentationMatchesNative(
                    runtime, 50, independentCameraX, independentCameraY);
                if (probe.Goat.action == 12) sawPreKill = true;
                if (sawPreKill && probe.Goat.action == 13) sawFinalKill = true;
                if ((probe.Goat.controlFlags & 0x00000400) != 0)
                {
                    Assert.That(sawFinalKill, Is.True);
                    Assert.That(
                        probe.RendererEnabled,
                        Is.False,
                        "drawing-disabled Goat hides on the removal-queued frame");
                    if (!sawRemovalQueued)
                        removalQueuedHash = probe.GoatLiveWorldHash;
                    sawRemovalQueued = true;
                }
                if (probe.Goat.active == 0)
                {
                    Assert.That(sawRemovalQueued, Is.True);
                    Assert.That(probe.RendererEnabled, Is.False);
                    reclaimedHash = probe.GoatLiveWorldHash;
                    reclaimed = true;
                    break;
                }
            }
            Assert.That(reclaimed, Is.True, "native Goat reached reclamation");
            Assert.That(sawPreKill, Is.True, "native Goat visited action 12");
            Assert.That(sawFinalKill, Is.True, "native Goat visited action 13");
            var lifecycleEvents = runtime.QualificationGoatEvents(50);
            var expectedLifecycleKinds = new uint[]
            {
                NativeSim.EventGoatManaConverted,
                NativeSim.EventGoatExtraManaSphereRequest,
                NativeSim.EventGoatRemovalQueued,
                NativeSim.EventGoatReclaimed
            };
            var observedLifecycle = new System.Collections.Generic.List<
                NativeSim.NativeEvent>();
            ulong priorSequence = 0;
            foreach (var nativeEvent in lifecycleEvents)
            {
                Assert.That(nativeEvent.generation, Is.EqualTo(generationBeforeDamage));
                Assert.That(nativeEvent.sequence, Is.GreaterThan(priorSequence));
                Assert.That(nativeEvent.stateHash, Is.Not.Zero);
                priorSequence = nativeEvent.sequence;
                if (System.Array.IndexOf(expectedLifecycleKinds, nativeEvent.kind) >= 0)
                    observedLifecycle.Add(nativeEvent);
            }
            Assert.That(
                System.Array.ConvertAll(
                    observedLifecycle.ToArray(),
                    nativeEvent => nativeEvent.kind),
                Is.EqualTo(expectedLifecycleKinds),
                "exact Goat lifecycle event order has no duplicates or omissions");
            Assert.That(
                observedLifecycle.Count,
                Is.EqualTo(expectedLifecycleKinds.Length),
                "exact Goat lifecycle event count");
            Assert.That(observedLifecycle[0].value0, Is.EqualTo(300));
            Assert.That(
                observedLifecycle[0].stateHash,
                Is.EqualTo(removalQueuedHash));
            Assert.That(
                observedLifecycle[1].stateHash,
                Is.EqualTo(removalQueuedHash));
            Assert.That(
                observedLifecycle[2].stateHash,
                Is.EqualTo(removalQueuedHash));
            Assert.That(
                observedLifecycle[3].stateHash,
                Is.EqualTo(reclaimedHash));

            runtime.QualificationResetSession();
            var resetProbe = runtime.QualificationGoatPresentation(50);
            Assert.That(resetProbe.Generation, Is.GreaterThan(generationBeforeDamage));
            Assert.That(resetProbe.SnapshotTick, Is.Zero);
            Assert.That(resetProbe.GoatBatchTick, Is.Zero);
            Assert.That(resetProbe.PresentationTick, Is.Zero);
            Assert.That(resetProbe.RenderTick, Is.Zero);
            Assert.That(resetProbe.EventCursor, Is.Zero);
            Assert.That(resetProbe.SnapshotEventHead, Is.Zero);
            Assert.That(resetProbe.Goat.active, Is.EqualTo(1));
            Assert.That(resetProbe.Goat.life, Is.EqualTo(resetProbe.Goat.maxLife));
            Assert.That(resetProbe.Goat.pendingDamage, Is.Zero);
            AssertGoatResetRow(resetProbe.Goat, baseline.Goat);
            AssertResetPresentation(resetProbe, baseline);
            independentCameraX = resetProbe.CameraX;
            independentCameraY = resetProbe.CameraY;
            AssertGoatPresentationMatchesNative(
                runtime, 50, independentCameraX, independentCameraY);
            Assert.Throws<System.InvalidOperationException>(
                () => runtime.QualificationInjectGoatDamageAtGeneration(
                    generationBeforeDamage, 50, 601));
            var afterStaleGeneration = runtime.QualificationGoatPresentation(50);
            AssertGoatResetRow(afterStaleGeneration.Goat, baseline.Goat);
            Assert.That(
                afterStaleGeneration.GoatLiveWorldHash,
                Is.EqualTo(resetProbe.GoatLiveWorldHash));
            AssertResetPresentation(afterStaleGeneration, resetProbe);
            AssertAllMode20Sectors(runtime, 50);

            Object.Destroy(runtime.gameObject);
            yield return null;
            Assert.That(NativeSession.ActiveSessionCount, Is.EqualTo(0));
        }

        private static void AssertGoatPresentationMatchesNative(
            MagicCarpetRuntime runtime,
            ushort entityIndex,
            long independentCameraX,
            long independentCameraY)
        {
            const float sourceUnitsPerUnityUnit = 64f;
            var probe = runtime.QualificationGoatPresentation(entityIndex);
            Assert.That(probe.GoatBatchGeneration, Is.EqualTo(probe.Generation));
            Assert.That(probe.PresentationGeneration, Is.EqualTo(probe.Generation));
            Assert.That(probe.RenderGeneration, Is.EqualTo(probe.Generation));
            Assert.That(probe.GoatBatchTick, Is.EqualTo(probe.SnapshotTick));
            Assert.That(probe.RenderTick, Is.EqualTo(probe.PresentationTick));
            Assert.That(
                probe.PresentationTick,
                Is.EqualTo(
                    probe.SnapshotTick == 0 ? 0 : probe.SnapshotTick - 1),
                "camera sidecars preserve the pre-entity-update tick");
            Assert.That(probe.Goat.entityIndex, Is.EqualTo(entityIndex));
            Assert.That(probe.UnwrappedCameraX, Is.EqualTo(independentCameraX));
            Assert.That(probe.UnwrappedCameraY, Is.EqualTo(independentCameraY));
            var nearestX = independentCameraX +
                ModularDelta(probe.Goat.x, probe.CameraX);
            var nearestY = independentCameraY +
                ModularDelta(probe.Goat.y, probe.CameraY);
            var expectedPosition = new Vector3(
                nearestX / sourceUnitsPerUnityUnit,
                probe.Goat.z / sourceUnitsPerUnityUnit,
                nearestY / sourceUnitsPerUnityUnit);
            Assert.That(
                Vector3.Distance(probe.NodePosition, expectedPosition),
                Is.LessThan(0.0001f),
                "same-tick native Goat position reaches the Unity node");
            var expectedCameraTarget = new Vector3(
                (independentCameraX + probe.RenderOffsetX) /
                    sourceUnitsPerUnityUnit,
                probe.CameraZ / sourceUnitsPerUnityUnit,
                (independentCameraY + probe.RenderOffsetY) /
                    sourceUnitsPerUnityUnit);
            Assert.That(
                Vector3.Distance(
                    probe.CameraTargetPosition,
                    expectedCameraTarget),
                Is.LessThan(0.0001f),
                "current native camera row determines the Unity camera target");

            var orientation =
                (((probe.Goat.yaw - probe.RenderYaw) >> 3) & 0xf0) >> 4;
            var yawLookup = new[]
                { 0, 0, 0, 1, 1, 1, 2, 2, 2, 2, 2, 1, 1, 1, 0, 0 };
            Assert.That(
                probe.MaterialSelector,
                Is.EqualTo(probe.Goat.baseTmap + yawLookup[orientation]),
                "mode-20 selector comes from native yaw and render camera yaw");
            var expectedIndicesHashes = new[]
            {
                "cd5e773ab7c4e4e88536c5a690a8a1794a1b67361b402ab8365e9df3e59a1e9d",
                "acea2351981a1fd9aefaf7a2713bc00c4c2b2461dd087b9a2621e7fa3fb819a3",
                "7d081998c98ba0aa2bb57d4956926f3b8c80b31ae6ee481f8c9174551d6db722"
            };
            Assert.That(
                probe.IndicesSha256,
                Is.EqualTo(expectedIndicesHashes[probe.MaterialSelector - 152]),
                "bound indexed texture bytes match the owner-cache selector");
            Assert.That(
                probe.IndicesTextureBound,
                Is.True,
                "actual _Indices binding is the selector-owned texture");
            Assert.That(
                probe.MirroredMesh,
                Is.EqualTo(orientation >= 8),
                "rear mode-20 orientations use the mirrored mesh");
            Assert.That(
                probe.RendererEnabled,
                Is.False,
                "source-ordered native Goat pixels replace the Unity billboard");
        }

        private static void AssertGoatResetRow(
            NativeSim.GoatSnapshot actual,
            NativeSim.GoatSnapshot expected)
        {
            Assert.That(actual.entityIndex, Is.EqualTo(expected.entityIndex));
            Assert.That(actual.abiVersion, Is.EqualTo(expected.abiVersion));
            Assert.That(actual.structSize, Is.EqualTo(expected.structSize));
            Assert.That(actual.targetEntityIndex, Is.EqualTo(expected.targetEntityIndex));
            Assert.That(actual.renderParameterIndex, Is.EqualTo(expected.renderParameterIndex));
            Assert.That(actual.baseTmap, Is.EqualTo(expected.baseTmap));
            Assert.That(actual.authority, Is.EqualTo(expected.authority));
            Assert.That(actual.active, Is.EqualTo(expected.active));
            Assert.That(actual.classId, Is.EqualTo(expected.classId));
            Assert.That(actual.model, Is.EqualTo(expected.model));
            Assert.That(actual.action, Is.EqualTo(expected.action));
            Assert.That(actual.phase, Is.EqualTo(expected.phase));
            Assert.That(actual.stageVar1, Is.EqualTo(expected.stageVar1));
            Assert.That(actual.stageVar2, Is.EqualTo(expected.stageVar2));
            Assert.That(
                actual.proximityCountdown,
                Is.EqualTo(expected.proximityCountdown));
            Assert.That(
                actual.proximitySubcounter,
                Is.EqualTo(expected.proximitySubcounter));
            Assert.That(actual.animationFrame, Is.EqualTo(expected.animationFrame));
            Assert.That(actual.reserved0, Is.EqualTo(expected.reserved0));
            Assert.That(actual.x, Is.EqualTo(expected.x));
            Assert.That(actual.y, Is.EqualTo(expected.y));
            Assert.That(actual.z, Is.EqualTo(expected.z));
            Assert.That(actual.yaw, Is.EqualTo(expected.yaw));
            Assert.That(actual.pitch, Is.EqualTo(expected.pitch));
            Assert.That(actual.roll, Is.EqualTo(expected.roll));
            Assert.That(actual.actualSpeed, Is.EqualTo(expected.actualSpeed));
            Assert.That(actual.minimumSpeed, Is.EqualTo(expected.minimumSpeed));
            Assert.That(actual.maximumSpeed, Is.EqualTo(expected.maximumSpeed));
            Assert.That(actual.rng, Is.EqualTo(expected.rng));
            Assert.That(actual.life, Is.EqualTo(expected.life));
            Assert.That(actual.maxLife, Is.EqualTo(expected.maxLife));
            Assert.That(actual.mana, Is.EqualTo(expected.mana));
            Assert.That(actual.pendingDamage, Is.EqualTo(expected.pendingDamage));
            Assert.That(actual.controlFlags, Is.EqualTo(expected.controlFlags));
            Assert.That(actual.reserved, Is.EqualTo(expected.reserved));
        }

        private static void AssertResetPresentation(
            MagicCarpetRuntime.GoatPresentationProbe actual,
            MagicCarpetRuntime.GoatPresentationProbe expected)
        {
            Assert.That(actual.GoatLiveWorldHash, Is.EqualTo(expected.GoatLiveWorldHash));
            Assert.That(actual.CameraX, Is.EqualTo(expected.CameraX));
            Assert.That(actual.CameraY, Is.EqualTo(expected.CameraY));
            Assert.That(actual.CameraZ, Is.EqualTo(expected.CameraZ));
            Assert.That(actual.UnwrappedCameraX, Is.EqualTo(expected.UnwrappedCameraX));
            Assert.That(actual.UnwrappedCameraY, Is.EqualTo(expected.UnwrappedCameraY));
            Assert.That(actual.RenderYaw, Is.EqualTo(expected.RenderYaw));
            Assert.That(actual.RenderOffsetX, Is.EqualTo(expected.RenderOffsetX));
            Assert.That(actual.RenderOffsetY, Is.EqualTo(expected.RenderOffsetY));
            Assert.That(actual.CameraTargetPosition, Is.EqualTo(expected.CameraTargetPosition));
            Assert.That(actual.NodePosition, Is.EqualTo(expected.NodePosition));
            Assert.That(actual.RendererEnabled, Is.EqualTo(expected.RendererEnabled));
            Assert.That(actual.MaterialSelector, Is.EqualTo(expected.MaterialSelector));
            Assert.That(actual.IndicesSha256, Is.EqualTo(expected.IndicesSha256));
            Assert.That(
                actual.IndicesTextureBound,
                Is.EqualTo(expected.IndicesTextureBound));
            Assert.That(actual.MirroredMesh, Is.EqualTo(expected.MirroredMesh));
        }

        private static void AssertAllMode20Sectors(
            MagicCarpetRuntime runtime,
            ushort entityIndex)
        {
            var expectedOffsets = new[]
                { 0, 0, 0, 1, 1, 1, 2, 2, 2, 2, 2, 1, 1, 1, 0, 0 };
            for (var expectedOrientation = 0;
                 expectedOrientation < expectedOffsets.Length;
                 expectedOrientation++)
            {
                MagicCarpetRuntime.QualificationMode20Selection(
                    (short)(expectedOrientation << 7),
                    0,
                    out var orientation,
                    out var selectorOffset,
                    out var rearHalf);
                Assert.That(orientation, Is.EqualTo(expectedOrientation));
                Assert.That(
                    selectorOffset,
                    Is.EqualTo(expectedOffsets[expectedOrientation]));
                Assert.That(rearHalf, Is.EqualTo(expectedOrientation >= 8));
                var probe = runtime.QualificationBindGoatMode20Sector(
                    entityIndex,
                    expectedOrientation);
                Assert.That(
                    probe.MaterialSelector,
                    Is.EqualTo(152 + expectedOffsets[expectedOrientation]));
                Assert.That(
                    probe.IndicesTextureBound,
                    Is.True,
                    "actual _Indices texture is the selector-owned texture");
                Assert.That(
                    probe.MirroredMesh,
                    Is.EqualTo(expectedOrientation >= 8),
                    "actual mesh binding follows the descriptor mirror rule");
            }
        }

        private static int ModularDelta(ushort current, ushort previous)
        {
            return ((current - previous + 32768) & 0xffff) - 32768;
        }
    }
}
