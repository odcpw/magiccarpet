#if UNITY_EDITOR
using System;
using NUnit.Framework;
using UnityEngine;
using UnityEngine.Rendering;
using Object = UnityEngine.Object;

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
            Assert.AreEqual(1, gameObject.GetComponentsInChildren<UnityEnhancedHudPresentation>().Length);
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
                CameraPose = new UnityEnhancedCameraPose(Vector3.zero, Quaternion.identity, 70f),
                Entities = new[] { Entity(4, 11, Vector3.one) }
            };
            Assert.IsFalse(root.ApplySnapshot(stale));
            Assert.AreEqual(2, root.ActiveEntityCount);

            UnityEnhancedWorldSnapshot reset = new UnityEnhancedWorldSnapshot
            {
                Generation = 5,
                Tick = 0,
                Reset = true,
                CameraPose = new UnityEnhancedCameraPose(Vector3.zero, Quaternion.identity, 70f),
                Entities = new[] { Entity(5, 11, Vector3.one) }
            };
            Assert.IsTrue(root.ApplySnapshot(reset));
            Assert.AreEqual(1, root.ActiveEntityCount);
            Assert.AreEqual(5UL, root.Generation);
            Assert.AreEqual(0UL, root.LastTick);
            Assert.GreaterOrEqual(root.PooledEntityCount, 1);

            Object.DestroyImmediate(gameObject);
        }

        [Test]
        public void HudConsumesMissionStateAndRendersGenerationBoundBars()
        {
            GameObject gameObject = new GameObject("enhanced-hud-test");
            UnityEnhancedWorldRoot root = gameObject.AddComponent<UnityEnhancedWorldRoot>();
            root.OpenDiagnostic(PresentationModeId.UnityEnhanced);

            UnityEnhancedWorldSnapshot snapshot = new UnityEnhancedWorldSnapshot
            {
                Generation = 3,
                Tick = 4,
                CameraPose = new UnityEnhancedCameraPose(Vector3.zero, Quaternion.identity, 58f),
                Entities = new UnityEnhancedEntitySnapshot[0],
                Hud = new UnityEnhancedHudSnapshot
                {
                    Generation = 3,
                    Tick = 4,
                    PlayerLife = 80,
                    PlayerMaxLife = 100,
                    PlayerMana = 20,
                    PlayerMaxMana = 50,
                    SecuredManaPercent = 35,
                    TargetSpeed = 80,
                    ActualSpeed = 40,
                    CurrentStage = 2,
                    TasksCompleted = 1,
                    HasFireball = true,
                    HasSpeedSpell = true
                }
            };

            Assert.IsTrue(root.ApplySnapshot(snapshot));
            Assert.IsNotNull(root.HudPresentation);
            Assert.IsTrue(root.HudPresentation.IsVisible);
            Assert.AreEqual(3UL, root.HudPresentation.Generation);
            Assert.AreEqual(4UL, root.HudPresentation.LastTick);
            Assert.AreEqual(0.8f, root.HudPresentation.LifeFillAmount, 0.001f);
            Assert.AreEqual(0.4f, root.HudPresentation.ManaFillAmount, 0.001f);
            StringAssert.Contains("FIRE", root.HudPresentation.SpellText);
            StringAssert.Contains("SPEED", root.HudPresentation.SpellText);
            StringAssert.Contains("CASTER READY", root.HudPresentation.StatusText);

            root.Close();
            Assert.IsFalse(root.HudPresentation.IsVisible);
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
                CameraPose = new UnityEnhancedCameraPose(Vector3.zero, Quaternion.identity, 70f),
                Entities = new[] { Entity(1, 5, Vector3.zero) }
            }));
            Assert.IsTrue(root.ApplySnapshot(new UnityEnhancedWorldSnapshot
            {
                Generation = 1,
                Tick = 2,
                CameraPose = new UnityEnhancedCameraPose(Vector3.zero, Quaternion.identity, 70f),
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
                CameraPose = new UnityEnhancedCameraPose(Vector3.zero, Quaternion.identity, 70f),
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
        public void FixedTickInputHoldsAxesButConsumesEdgesOnce()
        {
            GameObject gameObject = new GameObject("enhanced-fixed-input-test");
            UnityEnhancedInputAdapter adapter = gameObject.AddComponent<UnityEnhancedInputAdapter>();
            UnityEnhancedInputFrame first = new UnityEnhancedInputFrame(
                1, Vector2.right, Vector2.up, false, true, 2);
            UnityEnhancedInputFrame consumed;

            Assert.IsTrue(adapter.Publish(first));
            Assert.IsTrue(adapter.TryConsumeForTick(out consumed));
            Assert.AreEqual(Vector2.right, consumed.Flight);
            Assert.AreEqual(Vector2.up, consumed.Look);
            Assert.IsTrue(consumed.Cast);
            Assert.AreEqual(2, consumed.SpellSelection);

            Assert.IsTrue(adapter.TryConsumeForTick(out consumed));
            Assert.AreEqual(Vector2.right, consumed.Flight);
            Assert.AreEqual(Vector2.up, consumed.Look);
            Assert.IsFalse(consumed.Cast);
            Assert.AreEqual(-1, consumed.SpellSelection);

            Assert.IsTrue(adapter.Publish(new UnityEnhancedInputFrame(
                2, Vector2.left, Vector2.down, false, false, -1)));
            Assert.IsTrue(adapter.TryConsumeForTick(out consumed));
            Assert.AreEqual(Vector2.left, consumed.Flight);
            Assert.AreEqual(Vector2.down, consumed.Look);
            Assert.IsFalse(consumed.Cast);
            Assert.AreEqual(-1, consumed.SpellSelection);

            Object.DestroyImmediate(gameObject);
        }

        [Test]
        public void InvalidNestedSnapshotDoesNotMutateAcceptedWorld()
        {
            GameObject gameObject = new GameObject("enhanced-invalid-snapshot-test");
            UnityEnhancedWorldRoot root = gameObject.AddComponent<UnityEnhancedWorldRoot>();
            root.OpenDiagnostic(PresentationModeId.UnityEnhanced);

            Assert.IsTrue(root.ApplySnapshot(new UnityEnhancedWorldSnapshot
            {
                Generation = 6,
                Tick = 1,
                CameraPose = new UnityEnhancedCameraPose(Vector3.zero, Quaternion.identity, 70f),
                Entities = new[] { Entity(6, 31, Vector3.zero) }
            }));

            Assert.IsFalse(root.ApplySnapshot(new UnityEnhancedWorldSnapshot
            {
                Generation = 6,
                Tick = 2,
                CameraPose = new UnityEnhancedCameraPose(Vector3.zero, Quaternion.identity, 70f),
                Entities = new[] { Entity(6, 31, new Vector3(float.NaN, 0f, 0f)) }
            }));
            Assert.AreEqual(1, root.ActiveEntityCount);
            Assert.AreEqual(6UL, root.Generation);
            Assert.AreEqual(1UL, root.LastTick);
            Assert.AreEqual(UnityEnhancedLifecycleState.Faulted, root.State);

            Object.DestroyImmediate(gameObject);
        }

        [Test]
        public void MissingTerrainSnapshotPreservesTheLastTerrainBuild()
        {
            GameObject gameObject = new GameObject("enhanced-terrain-suppression-test");
            UnityEnhancedWorldRoot root = gameObject.AddComponent<UnityEnhancedWorldRoot>();
            root.OpenDiagnostic(PresentationModeId.UnityEnhanced);

            Assert.IsTrue(root.ApplySnapshot(new UnityEnhancedWorldSnapshot
            {
                Generation = 7,
                Tick = 1,
                CameraPose = new UnityEnhancedCameraPose(Vector3.zero, Quaternion.identity, 70f),
                Terrain = TerrainSnapshot(7, 1, false, new UnityEnhancedTerrainDirtyRegion[0]),
                Entities = new[] { Entity(7, 41, Vector3.zero) }
            }));
            int builds = root.TerrainPresentation.DirtyRegionApplications;
            Assert.Greater(builds, 0);

            Assert.IsTrue(root.ApplySnapshot(new UnityEnhancedWorldSnapshot
            {
                Generation = 7,
                Tick = 2,
                CameraPose = new UnityEnhancedCameraPose(Vector3.zero, Quaternion.identity, 70f),
                Entities = new[] { Entity(7, 41, Vector3.one) }
            }));
            Assert.AreEqual(builds, root.TerrainPresentation.DirtyRegionApplications);
            Assert.AreEqual(1UL, root.TerrainPresentation.LastTick);

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
                CameraPose = new UnityEnhancedCameraPose(Vector3.zero, Quaternion.identity, 70f),
                Entities = new[] { Entity(1, 20, new Vector3(100f, 0f, 0f)) }
            }));
            root.InterpolatePresentation(1f);

            Assert.IsTrue(root.ApplySnapshot(new UnityEnhancedWorldSnapshot
            {
                Generation = 2,
                Tick = 0,
                Reset = true,
                CameraPose = new UnityEnhancedCameraPose(Vector3.zero, Quaternion.identity, 70f),
                Entities = new[] { Entity(2, 21, Vector3.zero) }
            }));
            Transform entity = gameObject.transform.Find("EnhancedWorldObjects/EnhancedEntity_0000000000000015");
            Assert.IsNotNull(entity);
            root.InterpolatePresentation(0f);
            Assert.That(entity.position, Is.EqualTo(Vector3.zero));

            Object.DestroyImmediate(gameObject);
        }

        [Test]
        public void ClosingDestroysPooledEntitiesImmediatelyInEditMode()
        {
            GameObject gameObject = new GameObject("enhanced-close-pool-test");
            UnityEnhancedWorldRoot root = gameObject.AddComponent<UnityEnhancedWorldRoot>();
            root.OpenDiagnostic(PresentationModeId.UnityEnhanced);

            Assert.IsTrue(root.ApplySnapshot(new UnityEnhancedWorldSnapshot
            {
                Generation = 1,
                Tick = 1,
                CameraPose = new UnityEnhancedCameraPose(Vector3.zero, Quaternion.identity, 70f),
                Entities = new[] { Entity(1, 30, Vector3.zero) }
            }));
            Assert.IsTrue(root.ApplySnapshot(new UnityEnhancedWorldSnapshot
            {
                Generation = 2,
                Tick = 0,
                Reset = true,
                CameraPose = new UnityEnhancedCameraPose(Vector3.zero, Quaternion.identity, 70f),
                Entities = new UnityEnhancedEntitySnapshot[0]
            }));
            Assert.AreEqual(1, root.PooledEntityCount);

            root.Close();

            Assert.AreEqual(UnityEnhancedLifecycleState.Closed, root.State);
            Assert.AreEqual(0, root.PooledEntityCount);
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
            update.Origin = new Vector3(4f, 0f, 8f);
            update.Heights[4] = 3f;
            Assert.IsTrue(terrain.ApplySnapshot(update));
            Assert.AreEqual(2, terrain.DirtyRegionApplications);
            Assert.That(terrain.GeneratedMesh.vertices[4].y, Is.EqualTo(3f).Within(0.001f));
            Assert.That(terrain.GeneratedMesh.vertices[0].x, Is.EqualTo(4f).Within(0.001f));
            Assert.That(terrain.GeneratedMesh.vertices[0].z, Is.EqualTo(8f).Within(0.001f));

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
        public void NightLightingAppliesReadableDescriptorAndRestoresSceneState()
        {
            GameObject root = new GameObject("enhanced-readable-night-test");
            GameObject cameraObject = new GameObject("enhanced-readable-night-camera");
            cameraObject.transform.SetParent(root.transform, false);
            Camera camera = cameraObject.AddComponent<Camera>();
            camera.clearFlags = CameraClearFlags.Depth;
            camera.backgroundColor = Color.magenta;
            UnityEnhancedNightLighting lighting = root.AddComponent<UnityEnhancedNightLighting>();
            AmbientMode ambientModeBefore = RenderSettings.ambientMode;
            FogMode fogModeBefore = RenderSettings.fogMode;
            Color backgroundBefore = camera.backgroundColor;
            Material skyboxBefore = RenderSettings.skybox;

            lighting.ApplyDescriptor(
                7,
                UnityEnhancedNightLightingDescriptor.Level1Night(),
                Array.Empty<UnityEnhancedLocalLightRequest>(),
                true,
                camera);

            Assert.AreEqual(0.65f, lighting.AppliedExposure, 0.0001f);
            Assert.AreEqual(AmbientMode.Trilight, RenderSettings.ambientMode);
            Assert.AreEqual(FogMode.ExponentialSquared, RenderSettings.fogMode);
            Shader enhancedNightSky = Shader.Find("MagicCarpet/Enhanced/NightSky");
            Shader proceduralSky = Shader.Find("Skybox/Procedural");
            Assert.AreEqual(
                enhancedNightSky != null || proceduralSky != null
                    ? CameraClearFlags.Skybox
                    : CameraClearFlags.SolidColor,
                camera.clearFlags);
            if (enhancedNightSky == null && proceduralSky == null)
            {
                Assert.AreNotEqual(backgroundBefore, camera.backgroundColor);
            }
            else
            {
                Assert.IsNotNull(RenderSettings.skybox);
                Assert.AreNotSame(skyboxBefore, RenderSettings.skybox);
            }

            lighting.ResetForSession();
            Assert.AreEqual(ambientModeBefore, RenderSettings.ambientMode);
            Assert.AreEqual(fogModeBefore, RenderSettings.fogMode);
            Assert.AreSame(skyboxBefore, RenderSettings.skybox);
            Assert.AreEqual(CameraClearFlags.Depth, camera.clearFlags);
            Assert.AreEqual(backgroundBefore, camera.backgroundColor);

            Object.DestroyImmediate(root);
        }

        [Test]
        public void WaterBuildsReflectionSurfaceAndShorelineMesh()
        {
            Assert.IsNotNull(Shader.Find("MagicCarpet/Enhanced/Water"));
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
            MeshFilter shoreline = gameObject.transform.Find("EnhancedShoreline").GetComponent<MeshFilter>();
            Assert.That(shoreline.sharedMesh.vertices[0].x, Is.EqualTo(-4f).Within(0.001f));
            Assert.That(shoreline.sharedMesh.vertices[0].z, Is.EqualTo(-4f).Within(0.001f));

            Object.DestroyImmediate(gameObject);
        }

        [Test]
        public void VfxEventsDeduplicateReplayAndRespectReducedMotion()
        {
            Assert.IsNotNull(Shader.Find("MagicCarpet/Enhanced/Vfx"));
            GameObject gameObject = new GameObject("enhanced-vfx-test");
            UnityEnhancedVfxPresentation vfx = gameObject.AddComponent<UnityEnhancedVfxPresentation>();
            vfx.ResetForGeneration(3);
            UnityEnhancedVfxEvent first = new UnityEnhancedVfxEvent(
                1, 3, 10, 17, UnityEnhancedVfxKind.SpellCast, Vector3.zero, 1f, 4);
            Assert.IsTrue(vfx.ApplyEvent(first));
            MeshRenderer renderer = gameObject.transform.Find("EnhancedVfxPool/EnhancedVfx").GetComponent<MeshRenderer>();
            Assert.AreEqual("MagicCarpet/Enhanced/Vfx", renderer.sharedMaterial.shader.name);
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
            Assert.AreEqual(0f, quality.AppliedPostExposure);
            Assert.IsFalse(quality.TemporalUpscalingEnabled);
            Assert.IsNotNull(quality.PostVolume);
            Assert.IsTrue(quality.Apply(
                UnityEnhancedQualitySettings.ForTier(UnityEnhancedQualityTier.WholeHog),
                camera,
                0.35f));
            Assert.AreEqual(0.35f, quality.AppliedPostExposure, 0.0001f);
            quality.ResetForSession();

            Object.DestroyImmediate(gameObject);
        }

        [Test]
        public void EnhancedVertexColorShaderIsAvailableForGeneratedPresentation()
        {
            Assert.IsNotNull(Shader.Find("MagicCarpet/Enhanced/VertexColorLit"));
            Assert.IsNotNull(Shader.Find("MagicCarpet/Enhanced/Terrain"));
            Assert.IsNotNull(Shader.Find("MagicCarpet/Enhanced/NightSky"));
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
