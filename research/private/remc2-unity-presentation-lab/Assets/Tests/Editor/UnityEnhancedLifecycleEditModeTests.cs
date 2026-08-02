#if UNITY_EDITOR
using NUnit.Framework;
using UnityEngine;

namespace MagicCarpet.PresentationLab.Tests
{
    public sealed class UnityEnhancedLifecycleEditModeTests
    {
        [Test]
        public void OnlyEnhancedModeCanOpenTheRoot()
        {
            GameObject gameObject = new GameObject("enhanced-root-test");
            UnityEnhancedWorldRoot root = gameObject.AddComponent<UnityEnhancedWorldRoot>();

            Assert.Throws<System.InvalidOperationException>(() => root.OpenDiagnostic(PresentationModeId.ClassicHd));
            Assert.AreEqual(UnityEnhancedLifecycleState.Closed, root.State);

            Object.DestroyImmediate(gameObject);
        }

        [Test]
        public void OpeningBuildsOneExclusiveHierarchy()
        {
            GameObject gameObject = new GameObject("enhanced-root-test");
            UnityEnhancedWorldRoot root = gameObject.AddComponent<UnityEnhancedWorldRoot>();

            root.OpenDiagnostic(PresentationModeId.UnityEnhanced);
            root.OpenDiagnostic(PresentationModeId.UnityEnhanced);

            Assert.AreEqual(UnityEnhancedLifecycleState.Open, root.State);
            Assert.AreEqual(1, gameObject.GetComponents<UnityEnhancedInputAdapter>().Length);
            Assert.AreEqual(1, gameObject.GetComponents<UnityEnhancedPresentationAdapters>().Length);
            Assert.AreEqual(1, gameObject.GetComponentsInChildren<UnityEnhancedCameraRig>().Length);
            Assert.IsNotNull(gameObject.transform.Find("EnhancedWorldObjects"));

            Object.DestroyImmediate(gameObject);
        }

        [Test]
        public void SnapshotLifecycleRejectsStaleAndResetsGeneration()
        {
            GameObject gameObject = new GameObject("enhanced-root-test");
            UnityEnhancedWorldRoot root = gameObject.AddComponent<UnityEnhancedWorldRoot>();
            root.OpenDiagnostic(PresentationModeId.UnityEnhanced);

            UnityEnhancedWorldSnapshot first = new UnityEnhancedWorldSnapshot
            {
                Generation = 4,
                Tick = 10,
                CameraPose = new UnityEnhancedCameraPose(Vector3.zero, Quaternion.identity, 70f),
                Entities = new[]
                {
                    Entity(4, 11, new Vector3(1f, 2f, 3f)),
                    Entity(4, 12, new Vector3(4f, 5f, 6f))
                }
            };

            Assert.IsTrue(root.ApplySnapshot(first));
            Assert.AreEqual(2, root.ActiveEntityCount);

            UnityEnhancedWorldSnapshot stale = new UnityEnhancedWorldSnapshot
            {
                Generation = 4,
                Tick = 9,
                Entities = new[] { Entity(4, 11, Vector3.one) }
            };
            Assert.IsFalse(root.ApplySnapshot(stale));
            Assert.AreEqual(2, root.ActiveEntityCount);

            UnityEnhancedWorldSnapshot reset = new UnityEnhancedWorldSnapshot
            {
                Generation = 5,
                Tick = 1,
                Reset = true,
                Entities = new[] { Entity(5, 11, Vector3.one) }
            };
            Assert.IsTrue(root.ApplySnapshot(reset));
            Assert.AreEqual(1, root.ActiveEntityCount);
            Assert.AreEqual(5UL, root.Generation);
            Assert.AreEqual(1UL, root.LastTick);
            Assert.GreaterOrEqual(root.PooledEntityCount, 1);

            Object.DestroyImmediate(gameObject);
        }

        [Test]
        public void PresentationTransformsCanInterpolateWithoutChangingSnapshotTick()
        {
            GameObject gameObject = new GameObject("enhanced-root-test");
            UnityEnhancedWorldRoot root = gameObject.AddComponent<UnityEnhancedWorldRoot>();
            root.OpenDiagnostic(PresentationModeId.UnityEnhanced);

            Assert.IsTrue(root.ApplySnapshot(new UnityEnhancedWorldSnapshot
            {
                Generation = 1,
                Tick = 1,
                Entities = new[] { Entity(1, 5, Vector3.zero) }
            }));
            Assert.IsTrue(root.ApplySnapshot(new UnityEnhancedWorldSnapshot
            {
                Generation = 1,
                Tick = 2,
                Entities = new[] { Entity(1, 5, new Vector3(10f, 0f, 0f)) }
            }));

            Transform entity = gameObject.transform.Find("EnhancedWorldObjects/EnhancedEntity_0000000000000005");
            Assert.IsNotNull(entity);
            root.InterpolatePresentation(0.5f);
            Assert.That(entity.position.x, Is.EqualTo(5f).Within(0.001f));
            Assert.AreEqual(2UL, root.LastTick);

            Object.DestroyImmediate(gameObject);
        }

        [Test]
        public void DuplicateOrWrongGenerationRecordsFailClosed()
        {
            GameObject gameObject = new GameObject("enhanced-root-test");
            UnityEnhancedWorldRoot root = gameObject.AddComponent<UnityEnhancedWorldRoot>();
            root.OpenDiagnostic(PresentationModeId.UnityEnhanced);

            UnityEnhancedWorldSnapshot duplicate = new UnityEnhancedWorldSnapshot
            {
                Generation = 2,
                Tick = 1,
                Entities = new[] { Entity(2, 7, Vector3.zero), Entity(2, 7, Vector3.one) }
            };
            Assert.IsFalse(root.ApplySnapshot(duplicate));
            Assert.AreEqual(UnityEnhancedLifecycleState.Faulted, root.State);

            Object.DestroyImmediate(gameObject);
        }

        [Test]
        public void InputIsSequenceBoundAndConsumedOnce()
        {
            GameObject gameObject = new GameObject("enhanced-input-test");
            UnityEnhancedInputAdapter adapter = gameObject.AddComponent<UnityEnhancedInputAdapter>();
            UnityEnhancedInputFrame frame = new UnityEnhancedInputFrame(3, Vector2.right, Vector2.up, false, true, 2);
            UnityEnhancedInputFrame consumed;

            Assert.IsTrue(adapter.Publish(frame));
            Assert.IsFalse(adapter.Publish(frame));
            Assert.IsTrue(adapter.TryConsume(out consumed));
            Assert.AreEqual(3UL, consumed.Sequence);
            Assert.IsTrue(consumed.Cast);
            Assert.IsFalse(adapter.TryConsume(out consumed));

            Object.DestroyImmediate(gameObject);
        }

        [Test]
        public void ClosingAndReopeningStartsAResetInputSession()
        {
            GameObject gameObject = new GameObject("enhanced-input-session-test");
            UnityEnhancedWorldRoot root = gameObject.AddComponent<UnityEnhancedWorldRoot>();
            root.OpenDiagnostic(PresentationModeId.UnityEnhanced);

            Assert.IsTrue(root.InputAdapter.Publish(new UnityEnhancedInputFrame(3, Vector2.zero, Vector2.zero, false, false, 0)));
            root.Close();
            root.OpenDiagnostic(PresentationModeId.UnityEnhanced);

            Assert.IsTrue(root.InputAdapter.Publish(new UnityEnhancedInputFrame(1, Vector2.zero, Vector2.zero, false, false, 0)));
            Object.DestroyImmediate(gameObject);
        }

        [Test]
        public void ReusedPoolObjectsStartAtTheNewGenerationPose()
        {
            GameObject gameObject = new GameObject("enhanced-pool-session-test");
            UnityEnhancedWorldRoot root = gameObject.AddComponent<UnityEnhancedWorldRoot>();
            root.OpenDiagnostic(PresentationModeId.UnityEnhanced);

            Assert.IsTrue(root.ApplySnapshot(new UnityEnhancedWorldSnapshot
            {
                Generation = 1,
                Tick = 1,
                Entities = new[] { Entity(1, 20, new Vector3(100f, 0f, 0f)) }
            }));
            root.InterpolatePresentation(1f);

            Assert.IsTrue(root.ApplySnapshot(new UnityEnhancedWorldSnapshot
            {
                Generation = 2,
                Tick = 1,
                Reset = true,
                Entities = new[] { Entity(2, 21, Vector3.zero) }
            }));
            Transform entity = gameObject.transform.Find("EnhancedWorldObjects/EnhancedEntity_0000000000000015");
            Assert.IsNotNull(entity);
            root.InterpolatePresentation(0f);
            Assert.That(entity.position, Is.EqualTo(Vector3.zero));

            Object.DestroyImmediate(gameObject);
        }

        [Test]
        public void PresentationAdaptersRejectDuplicateEvents()
        {
            GameObject gameObject = new GameObject("enhanced-adapter-test");
            UnityEnhancedPresentationAdapters adapters = gameObject.AddComponent<UnityEnhancedPresentationAdapters>();
            UnityEnhancedPresentationEvent presentationEvent = new UnityEnhancedPresentationEvent(9, 2, Vector3.zero);

            Assert.IsTrue(adapters.AcceptEvent(presentationEvent));
            Assert.IsFalse(adapters.AcceptEvent(presentationEvent));
            adapters.Reset(8);
            adapters.ApplyTick(8, 12);
            Assert.AreEqual(8UL, adapters.CurrentGeneration);
            Assert.AreEqual(12UL, adapters.LastTick);

            Object.DestroyImmediate(gameObject);
        }

        [Test]
        public void TerrainUsesAuthoritativePlanesDirtyUpdatesAndWrapping()
        {
            GameObject gameObject = new GameObject("enhanced-terrain-test");
            UnityEnhancedTerrainPresentation terrain = gameObject.AddComponent<UnityEnhancedTerrainPresentation>();
            UnityEnhancedTerrainSnapshot first = TerrainSnapshot(1, 1, false, new UnityEnhancedTerrainDirtyRegion[0]);

            Assert.IsTrue(terrain.ApplySnapshot(first));
            Assert.AreEqual(9, terrain.VertexCount);
            Assert.AreEqual(1, terrain.DirtyRegionApplications);
            Assert.AreEqual(2, UnityEnhancedTerrainPresentation.WrapIndex(-1, 3, UnityEnhancedTerrainWrapMode.Repeat));

            UnityEnhancedTerrainSnapshot update = TerrainSnapshot(
                1, 2, false, new[] { new UnityEnhancedTerrainDirtyRegion(1, 1, 1, 1) });
            update.Heights[4] = 3f;
            Assert.IsTrue(terrain.ApplySnapshot(update));
            Assert.AreEqual(2, terrain.DirtyRegionApplications);
            Assert.That(terrain.GeneratedMesh.vertices[4].y, Is.EqualTo(3f).Within(0.001f));

            Object.DestroyImmediate(gameObject);
        }

        [Test]
        public void ActorFamiliesAreCoveredAndGeneratedViewsPool()
        {
            GameObject gameObject = new GameObject("enhanced-actor-test");
            UnityEnhancedActorPresentation actors = gameObject.AddComponent<UnityEnhancedActorPresentation>();
            GameObject view = new GameObject("actor-view");
            view.transform.SetParent(gameObject.transform, false);
            UnityEnhancedEntitySnapshot entity = Entity(1, 42, Vector3.zero);
            entity.PresentationFamily = (int)UnityEnhancedPresentationFamily.Castle;
            entity.FacingDegrees = 90f;
            entity.RenderLayer = 7;

            actors.Configure(view, entity);
            Assert.AreEqual(UnityEnhancedActorPresentation.CoverageCount, 20);
            Assert.AreEqual(1, actors.ActiveVisualCount);
            Assert.IsNotNull(view.transform.Find("EnhancedActorVisual"));
            Assert.AreEqual(7, actors.LastSortingOrder);
            actors.Release(view);
            Assert.AreEqual(0, actors.ActiveVisualCount);
            Assert.AreEqual(1, actors.PooledVisualCount);

            Object.DestroyImmediate(gameObject);
        }

        [Test]
        public void NightLightingUsesDeterministicQualityCaps()
        {
            GameObject gameObject = new GameObject("enhanced-lighting-test");
            UnityEnhancedNightLighting lighting = gameObject.AddComponent<UnityEnhancedNightLighting>();
            lighting.SetQuality(UnityEnhancedLightingQuality.Low);
            var requests = new[]
            {
                new UnityEnhancedLocalLightRequest(4, Vector3.zero, Color.red, 1f, 4f, 1),
                new UnityEnhancedLocalLightRequest(2, Vector3.one, Color.green, 1f, 4f, 3),
                new UnityEnhancedLocalLightRequest(9, Vector3.up, Color.blue, 1f, 4f, 2),
                new UnityEnhancedLocalLightRequest(1, Vector3.forward, Color.white, 1f, 4f, 0)
            };

            lighting.ApplyDescriptor(5, UnityEnhancedNightLightingDescriptor.Level1Night(), requests, true);
            Assert.AreEqual(2, lighting.ActiveLocalLightCount);
            Assert.AreEqual(2, lighting.DroppedLocalLightCount);
            Assert.AreEqual(5UL, lighting.Generation);
            lighting.ResetForGeneration(6);
            Assert.AreEqual(0, lighting.ActiveLocalLightCount);

            Object.DestroyImmediate(gameObject);
        }

        [Test]
        public void WaterBuildsReflectionSurfaceAndShorelineMesh()
        {
            GameObject gameObject = new GameObject("enhanced-water-test");
            UnityEnhancedWaterPresentation water = gameObject.AddComponent<UnityEnhancedWaterPresentation>();
            UnityEnhancedWaterSnapshot snapshot = new UnityEnhancedWaterSnapshot
            {
                Generation = 2,
                Tick = 4,
                Level = 1.5f,
                Size = new Vector2(8f, 8f),
                ShorelineWidth = 2,
                ShorelineHeight = 2,
                ShorelineMask = new byte[] { 1, 0, 0, 1 }
            };

            Assert.IsTrue(water.ApplySnapshot(snapshot));
            Assert.IsTrue(water.ReflectionEnabled);
            Assert.AreEqual(UnityEnhancedWaterReflectionMode.ScreenSpaceWhenSupported, water.ReflectionMode);
            Assert.AreEqual(2, water.ShorelineCellCount);
            Assert.AreEqual(8, water.ShorelineVertexCount);

            Object.DestroyImmediate(gameObject);
        }

        [Test]
        public void VfxEventsDeduplicateReplayAndRespectReducedMotion()
        {
            GameObject gameObject = new GameObject("enhanced-vfx-test");
            UnityEnhancedVfxPresentation vfx = gameObject.AddComponent<UnityEnhancedVfxPresentation>();
            vfx.ResetForGeneration(3);
            UnityEnhancedVfxEvent first = new UnityEnhancedVfxEvent(
                1, 3, 10, 17, UnityEnhancedVfxKind.SpellCast, Vector3.zero, 1f, 4);
            Assert.IsTrue(vfx.ApplyEvent(first));
            Assert.IsFalse(vfx.ApplyEvent(first));
            Assert.AreEqual(1, vfx.DuplicateEffectCount);
            vfx.SetReducedMotion(true);
            Assert.IsTrue(vfx.ReducedMotion);
            Assert.AreEqual(1, vfx.ApplyReplay(new[] { first }, 3));
            Assert.AreEqual(1, vfx.ActiveEffectCount);
            vfx.Advance(14);
            Assert.AreEqual(0, vfx.ActiveEffectCount);

            Object.DestroyImmediate(gameObject);
        }

        [Test]
        public void QualityRejectsUngatedTemporalAndAppliesSpatialWholeHogDefaults()
        {
            GameObject gameObject = new GameObject("enhanced-quality-test");
            Camera camera = gameObject.AddComponent<Camera>();
            UnityEnhancedQualityAndPost quality = gameObject.AddComponent<UnityEnhancedQualityAndPost>();
            UnityEnhancedQualitySettings invalid = UnityEnhancedQualitySettings.ForTier(UnityEnhancedQualityTier.WholeHog);
            invalid.TemporalMode = UnityEnhancedTemporalMode.GatedExperimental;
            string reason;
            Assert.IsFalse(invalid.TryValidate(out reason));
            Assert.IsTrue(quality.Apply(UnityEnhancedQualitySettings.ForTier(UnityEnhancedQualityTier.WholeHog), camera));
            Assert.AreEqual(1f, quality.AppliedRenderScale);
            Assert.IsFalse(quality.TemporalUpscalingEnabled);
            Assert.IsNotNull(quality.PostVolume);
            quality.ResetForSession();

            Object.DestroyImmediate(gameObject);
        }

        [Test]
        public void DiagnosticsReportContainsMetricsWithoutMachinePaths()
        {
            GameObject gameObject = new GameObject("enhanced-diagnostics-test");
            UnityEnhancedDiagnostics diagnostics = gameObject.AddComponent<UnityEnhancedDiagnostics>();
            diagnostics.SetAccessibility(true, true);
            string report = diagnostics.BuildSupportReport();
            StringAssert.Contains("enhanced.schema=1", report);
            StringAssert.Contains("enhanced.reduced_motion=true", report);
            Assert.IsFalse(report.Contains("/"));

            Object.DestroyImmediate(gameObject);
        }

        private static UnityEnhancedTerrainSnapshot TerrainSnapshot(
            ulong generation,
            ulong tick,
            bool reset,
            UnityEnhancedTerrainDirtyRegion[] dirtyRegions)
        {
            return new UnityEnhancedTerrainSnapshot
            {
                Generation = generation,
                Tick = tick,
                Reset = reset,
                Width = 3,
                Height = 3,
                CellSize = 1f,
                Heights = new float[9],
                TerrainTypes = new byte[9],
                Shade = new byte[] { 255, 220, 200, 180, 160, 140, 120, 100, 80 },
                Angles = new byte[9],
                DirtyRegions = dirtyRegions,
                WrapMode = UnityEnhancedTerrainWrapMode.Repeat
            };
        }

        private static UnityEnhancedEntitySnapshot Entity(ulong generation, ulong identity, Vector3 position)
        {
            return new UnityEnhancedEntitySnapshot
            {
                Generation = generation,
                StableIdentity = identity,
                Position = position,
                Rotation = Quaternion.identity,
                Visible = true
            };
        }
    }
}
#endif
