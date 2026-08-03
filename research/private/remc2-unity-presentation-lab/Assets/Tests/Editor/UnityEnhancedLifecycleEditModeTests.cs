#if UNITY_EDITOR
using System;
using System.IO;
using NUnit.Framework;
using UnityEngine;
using UnityEditor;
using UnityEngine.Rendering;
using UnityEngine.Rendering.Universal;
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
            Assert.AreEqual(0, gameObject.GetComponentsInChildren<UnityEnhancedMinimapPresentation>().Length);
            Assert.IsNotNull(gameObject.transform.Find("EnhancedWorldObjects"));

            Object.DestroyImmediate(gameObject);
        }

        [Test]
        public void EnhancedQualityAppliesReachableFireballBloomContract()
        {
            GameObject gameObject = new GameObject("enhanced-bloom-test");
            UnityEnhancedWorldRoot root = gameObject.AddComponent<UnityEnhancedWorldRoot>();

            root.OpenDiagnostic(PresentationModeId.UnityEnhanced);

            UnityEnhancedQualityAndPost quality = root.QualityAndPost;
            Assert.IsNotNull(quality);
            Assert.IsNotNull(quality.PostVolume);
            Assert.IsTrue(quality.PostVolume.isGlobal);
            Assert.AreEqual(100f, quality.PostVolume.priority);
            Assert.IsTrue(quality.PostVolume.profile.TryGet<Bloom>(out Bloom bloom));
            Assert.IsTrue(bloom.active);
            Assert.That(bloom.threshold.value, Is.EqualTo(0.85f).Within(0.0001f));
            Assert.That(bloom.intensity.value, Is.EqualTo(0.65f).Within(0.0001f));
            Assert.That(bloom.scatter.value, Is.EqualTo(0.78f).Within(0.0001f));
            Assert.That(bloom.tint.value.r, Is.EqualTo(1f).Within(0.0001f));
            Assert.That(bloom.tint.value.g, Is.EqualTo(0.38f).Within(0.0001f));
            Assert.That(bloom.tint.value.b, Is.EqualTo(0.12f).Within(0.0001f));

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
        public void HudRendersOwnerIndexedFrameAndGatesNonOverlayPixels()
        {
            GameObject gameObject = new GameObject("enhanced-hud-test");
            UnityEnhancedWorldRoot root = gameObject.AddComponent<UnityEnhancedWorldRoot>();
            root.OpenDiagnostic(PresentationModeId.UnityEnhanced);

            UnityEnhancedWorldSnapshot snapshot = OwnerHudWorldSnapshot(3, 4);

            Assert.IsTrue(root.ApplySnapshot(snapshot));
            Assert.IsNotNull(root.HudPresentation);
            Assert.IsTrue(root.HudPresentation.IsVisible);
            Assert.AreEqual(3UL, root.HudPresentation.Generation);
            Assert.AreEqual(4UL, root.HudPresentation.LastTick);
            Assert.AreEqual(3UL, root.HudPresentation.FrameGeneration);
            Assert.AreEqual(4UL, root.HudPresentation.FrameTick);
            Assert.AreEqual(2, root.HudPresentation.FrameWidth);
            Assert.AreEqual(1, root.HudPresentation.FrameHeight);
            Assert.IsNull(gameObject.transform.Find("EnhancedHud/EnhancedOwnerHudCanvas/EnhancedHudPanel"));
            Assert.IsNotNull(gameObject.transform.Find("EnhancedHud/EnhancedOwnerHudCanvas/EnhancedOwnerHudFrame"));
            Assert.IsNotNull(root.HudPresentation.OwnerHudMaterial);

            RenderTexture resolved = new RenderTexture(
                2,
                1,
                0,
                RenderTextureFormat.ARGB32,
                RenderTextureReadWrite.Linear);
            Texture2D readback = new Texture2D(2, 1, TextureFormat.RGBA32, false, true);
            Texture2D backgroundTexture = new Texture2D(2, 1, TextureFormat.RGBA32, false, true);
            RenderTexture previousActive = RenderTexture.active;
            Color background = new Color(0.2f, 0.4f, 0.7f, 1f);
            try
            {
                backgroundTexture.SetPixels(new[] { background, background });
                backgroundTexture.Apply(false, false);
                resolved.Create();
                Graphics.SetRenderTarget(resolved);
                GL.Clear(true, true, background);
                Graphics.Blit(backgroundTexture, resolved);
                Graphics.Blit(Texture2D.whiteTexture, resolved, root.HudPresentation.OwnerHudMaterial);
                RenderTexture.active = resolved;
                readback.ReadPixels(new Rect(0f, 0f, 2f, 1f), 0, 0, false);
                readback.Apply(false, false);
                Color hiddenPixel = readback.GetPixel(0, 0);
                Color visiblePixel = readback.GetPixel(1, 0);
                Assert.That(hiddenPixel.r, Is.EqualTo(background.r).Within(0.03f));
                Assert.That(hiddenPixel.g, Is.EqualTo(background.g).Within(0.03f));
                Assert.That(hiddenPixel.b, Is.EqualTo(background.b).Within(0.03f));
                Assert.That(visiblePixel.r, Is.EqualTo(44f / 255f).Within(0.03f));
                Assert.That(visiblePixel.g, Is.EqualTo(55f / 255f).Within(0.03f));
                Assert.That(visiblePixel.b, Is.EqualTo(66f / 255f).Within(0.03f));
            }
            finally
            {
                RenderTexture.active = previousActive;
                Object.DestroyImmediate(readback);
                Object.DestroyImmediate(backgroundTexture);
                resolved.Release();
                Object.DestroyImmediate(resolved);
            }

            root.Close();
            Assert.IsFalse(root.HudPresentation.IsVisible);
            Object.DestroyImmediate(gameObject);
        }

        [Test]
        public void HudCaptureCompositorFitsOwnerFrameInsideWidescreenTarget()
        {
            GameObject gameObject = new GameObject("enhanced-hud-capture-test");
            UnityEnhancedWorldRoot root = gameObject.AddComponent<UnityEnhancedWorldRoot>();
            root.OpenDiagnostic(PresentationModeId.UnityEnhanced);
            Assert.IsTrue(root.ApplySnapshot(OwnerHudWorldSnapshot(3, 4)));

            RenderTexture resolved = new RenderTexture(
                1280,
                720,
                0,
                RenderTextureFormat.ARGB32,
                RenderTextureReadWrite.Linear);
            Texture2D readback = new Texture2D(1280, 720, TextureFormat.RGBA32, false, true);
            RenderTexture previousActive = RenderTexture.active;
            Color background = new Color(0.2f, 0.4f, 0.7f, 1f);
            try
            {
                resolved.Create();
                Graphics.SetRenderTarget(resolved);
                GL.Clear(true, true, background);
                string reason;
                Assert.IsTrue(root.HudPresentation.CompositeOwnerHudTo(resolved, out reason), reason);

                RenderTexture.active = resolved;
                readback.ReadPixels(new Rect(0f, 0f, 1280f, 720f), 0, 0, false);
                readback.Apply(false, false);
                Color sideMargin = readback.GetPixel(80, 360);
                Color centeredHidden = readback.GetPixel(300, 360);
                Color centeredOverlay = readback.GetPixel(800, 360);
                Assert.That(sideMargin.r, Is.EqualTo(background.r).Within(0.03f));
                Assert.That(sideMargin.g, Is.EqualTo(background.g).Within(0.03f));
                Assert.That(sideMargin.b, Is.EqualTo(background.b).Within(0.03f));
                Assert.That(centeredHidden.r, Is.EqualTo(background.r).Within(0.03f));
                Assert.That(centeredHidden.g, Is.EqualTo(background.g).Within(0.03f));
                Assert.That(centeredHidden.b, Is.EqualTo(background.b).Within(0.03f));
                Assert.That(centeredOverlay.r, Is.EqualTo(44f / 255f).Within(0.03f));
                Assert.That(centeredOverlay.g, Is.EqualTo(55f / 255f).Within(0.03f));
                Assert.That(centeredOverlay.b, Is.EqualTo(66f / 255f).Within(0.03f));
            }
            finally
            {
                RenderTexture.active = previousActive;
                Object.DestroyImmediate(readback);
                resolved.Release();
                Object.DestroyImmediate(resolved);
                Object.DestroyImmediate(gameObject);
            }
        }

        [Test]
        public void HudRejectsStaleFrameGenerationAndTickWithoutReplacingVisibleFrame()
        {
            GameObject gameObject = new GameObject("enhanced-hud-stale-test");
            UnityEnhancedWorldRoot root = gameObject.AddComponent<UnityEnhancedWorldRoot>();
            root.OpenDiagnostic(PresentationModeId.UnityEnhanced);

            UnityEnhancedWorldSnapshot first = OwnerHudWorldSnapshot(5, 10);
            Assert.IsTrue(root.ApplySnapshot(first));
            Material material = root.HudPresentation.OwnerHudMaterial;

            UnityEnhancedHudSnapshot staleTick = OwnerHudSnapshot(5, 9);
            Assert.IsFalse(root.HudPresentation.ApplySnapshot(
                staleTick,
                root.CameraRig.PresentationCamera));
            Assert.AreSame(material, root.HudPresentation.OwnerHudMaterial);
            Assert.AreEqual(10UL, root.HudPresentation.LastTick);

            UnityEnhancedHudSnapshot staleGeneration = OwnerHudSnapshot(4, 11);
            Assert.IsFalse(root.HudPresentation.ApplySnapshot(
                staleGeneration,
                root.CameraRig.PresentationCamera));
            Assert.AreEqual(5UL, root.HudPresentation.FrameGeneration);
            Assert.AreEqual(10UL, root.HudPresentation.FrameTick);

            Object.DestroyImmediate(gameObject);
        }

        [Test]
        public void OwnerHudReceiptCacheRequiresMatchingCameraTickAndGeneration()
        {
            const uint validFlags = 0x2000u | 0x4000u | 0x8000u | 0x10000u | 0x20000u;

            Assert.IsTrue(UnityEnhancedOwnerSessionSource.CanReuseOwnerHudFrame(
                2,
                128,
                7,
                12,
                7,
                12,
                1,
                1,
                320,
                200,
                320,
                320u * 200u,
                320u * 200u,
                validFlags,
                1,
                1));
            Assert.IsFalse(UnityEnhancedOwnerSessionSource.CanReuseOwnerHudFrame(
                2,
                128,
                7,
                12,
                7,
                13,
                1,
                1,
                320,
                200,
                320,
                320u * 200u,
                320u * 200u,
                validFlags,
                1,
                1));
            Assert.IsFalse(UnityEnhancedOwnerSessionSource.CanReuseOwnerHudFrame(
                2,
                128,
                7,
                12,
                8,
                12,
                1,
                1,
                320,
                200,
                320,
                320u * 200u,
                320u * 200u,
                validFlags,
                1,
                1));
            Assert.IsFalse(UnityEnhancedOwnerSessionSource.CanReuseOwnerHudFrame(
                2,
                128,
                7,
                12,
                7,
                12,
                1,
                1,
                320,
                200,
                320,
                320u * 200u,
                320u * 200u,
                0x4000u,
                1,
                1));
            Assert.IsFalse(UnityEnhancedOwnerSessionSource.CanReuseOwnerHudFrame(
                2,
                128,
                7,
                12,
                7,
                12,
                1,
                1,
                320,
                200,
                320,
                320u * 200u,
                320u * 200u,
                validFlags,
                0,
                1));
            Assert.IsFalse(UnityEnhancedOwnerSessionSource.CanReuseOwnerHudFrame(
                2,
                128,
                7,
                12,
                7,
                12,
                1,
                1,
                320,
                200,
                320,
                320u * 200u,
                320u * 200u,
                validFlags,
                1,
                0));
        }

        [Test]
        public void EnhancedVerticalPresentationUsesOneSharedScaleAcrossOwnerYPaths()
        {
            Assert.That(
                UnityEnhancedOwnerSessionSource.EnhancedVerticalPresentationScale,
                Is.EqualTo(0.5f).Within(0.0001f));
            Assert.That(
                UnityEnhancedOwnerSessionSource.SourceVerticalToUnity(64f),
                Is.EqualTo(0.5f).Within(0.0001f));
            Assert.That(
                UnityEnhancedOwnerSessionSource.SourceHeightToUnity(2),
                Is.EqualTo(UnityEnhancedOwnerSessionSource.SourceVerticalToUnity(64f)).Within(0.0001f));

            string source = File.ReadAllText(Path.Combine(
                Application.dataPath,
                "Scripts",
                "UnityEnhancedOwnerSessionSource.cs"));
            StringAssert.Contains(
                "float y = SourceVerticalToUnity(presentation.cameraZ) + EnhancedCameraLift;",
                source);
            StringAssert.Contains("float y = SourceVerticalToUnity(source.z);", source);
            StringAssert.Contains("float projectedY = SourceVerticalToUnity(sourceZ);", source);
            StringAssert.Contains("Vector3 position = ProjectedPosition(source.x, source.y, source.z);", source);
        }

        [Test]
        public void EnhancedOwnerSourceSkipsUnsupportedFamiliesWithoutGenericFallback()
        {
            string source = File.ReadAllText(Path.Combine(
                Application.dataPath,
                "Scripts",
                "UnityEnhancedOwnerSessionSource.cs"));
            int buildEntitiesStart = source.IndexOf(
                "private UnityEnhancedEntitySnapshot[] BuildEntities()",
                StringComparison.Ordinal);
            int buildLightsStart = source.IndexOf(
                "private UnityEnhancedLocalLightRequest[] BuildLocalLights()",
                StringComparison.Ordinal);
            int presentationFamilyStart = source.IndexOf(
                "private static int PresentationFamily(",
                StringComparison.Ordinal);

            Assert.GreaterOrEqual(buildEntitiesStart, 0);
            Assert.Greater(buildLightsStart, buildEntitiesStart);
            Assert.Greater(presentationFamilyStart, buildLightsStart);

            string buildEntities = source.Substring(
                buildEntitiesStart,
                buildLightsStart - buildEntitiesStart);
            string buildLights = source.Substring(
                buildLightsStart,
                presentationFamilyStart - buildLightsStart);
            string familyMapping = source.Substring(presentationFamilyStart);

            StringAssert.Contains("private const int UnsupportedPresentationFamily = -1;", source);
            StringAssert.Contains(
                "if (family == UnsupportedPresentationFamily)\n" +
                "                {\n" +
                "                    continue;\n" +
                "                }",
                buildEntities);
            StringAssert.DoesNotContain(
                "family = (int)UnityEnhancedPresentationFamily.Generic;",
                buildEntities);
            StringAssert.Contains(
                "if (family == UnsupportedPresentationFamily ||",
                buildLights);
            StringAssert.Contains("default: return UnsupportedPresentationFamily;", familyMapping);
            StringAssert.Contains(
                "return (int)UnityEnhancedPresentationFamily.Player;",
                familyMapping);
            StringAssert.Contains(
                "case 45: return (int)UnityEnhancedPresentationFamily.Town;",
                familyMapping);
            StringAssert.Contains(
                "case 79: return (int)UnityEnhancedPresentationFamily.Castle;",
                familyMapping);
            StringAssert.Contains(
                "? (int)UnityEnhancedPresentationFamily.Fire",
                buildEntities);
            StringAssert.Contains(
                "int family = source.projectileKind == 1",
                buildEntities);
            StringAssert.Contains(
                ": (int)UnityEnhancedPresentationFamily.Impact;",
                buildEntities);
            StringAssert.Contains(
                "case 59: return (int)UnityEnhancedPresentationFamily.Smoke;",
                familyMapping);
            StringAssert.DoesNotContain(
                "return (int)UnityEnhancedPresentationFamily.Generic;",
                familyMapping);
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

            MeshRenderer renderer = gameObject.GetComponent<MeshRenderer>();
            Assert.IsNotNull(renderer);
            Assert.AreEqual("MagicCarpet/Enhanced/Terrain", renderer.sharedMaterial.shader.name);
            Assert.That(renderer.sharedMaterial.GetFloat("_DetailScale"), Is.EqualTo(0.14f).Within(0.0001f));
            Assert.That(renderer.sharedMaterial.GetFloat("_DetailStrength"), Is.EqualTo(0.90f).Within(0.0001f));
            Assert.That(renderer.sharedMaterial.GetFloat("_SlopeStrength"), Is.EqualTo(0.84f).Within(0.0001f));
            Assert.That(renderer.sharedMaterial.GetFloat("_NormalStrength"), Is.EqualTo(0.48f).Within(0.0001f));
            Assert.That(renderer.sharedMaterial.GetFloat("_RidgeStrength"), Is.EqualTo(0.64f).Within(0.0001f));

            Object.DestroyImmediate(gameObject);
        }

        [Test]
        public void WholeHogTerrainUsesInterpolatedPresentationMesh()
        {
            GameObject gameObject = new GameObject("enhanced-wholehog-terrain-test");
            UnityEnhancedWorldRoot root = gameObject.AddComponent<UnityEnhancedWorldRoot>();
            root.OpenDiagnostic(PresentationModeId.UnityEnhanced);
            UnityEnhancedTerrainSnapshot terrain = TerrainSnapshot(
                12,
                0,
                true,
                new UnityEnhancedTerrainDirtyRegion[0]);
            terrain.Heights[0] = 0f;
            terrain.Heights[1] = 2f;
            terrain.Heights[3] = 4f;
            terrain.Heights[4] = 6f;

            Assert.IsTrue(root.ApplySnapshot(new UnityEnhancedWorldSnapshot
            {
                Generation = 12,
                Tick = 0,
                Reset = true,
                CameraPose = new UnityEnhancedCameraPose(Vector3.zero, Quaternion.identity, 70f),
                Terrain = terrain,
                Quality = UnityEnhancedQualitySettings.ForTier(UnityEnhancedQualityTier.WholeHog)
            }));

            Assert.AreEqual(2, root.TerrainPresentation.RenderSubdivision);
            Assert.AreEqual(25, root.TerrainPresentation.VertexCount);
            Assert.AreEqual(IndexFormat.UInt16,
                root.TerrainPresentation.GeneratedMesh.indexFormat);
            Assert.That(root.TerrainPresentation.GeneratedMesh.vertices[1].y,
                Is.EqualTo(1f).Within(0.001f));
            Assert.That(root.TerrainPresentation.GeneratedMesh.vertices[5].y,
                Is.EqualTo(2f).Within(0.001f));

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
        public void AuthoritativeFireballUsesLuminousProjectileAndLocalLight()
        {
            GameObject gameObject = new GameObject("enhanced-fireball-test");
            UnityEnhancedWorldRoot root = gameObject.AddComponent<UnityEnhancedWorldRoot>();
            root.OpenDiagnostic(PresentationModeId.UnityEnhanced);
            ulong fireballIdentity = 0x0000000820000001UL;

            Assert.IsTrue(root.ApplySnapshot(new UnityEnhancedWorldSnapshot
            {
                Generation = 8,
                Tick = 12,
                CameraPose = new UnityEnhancedCameraPose(Vector3.zero, Quaternion.identity, 70f),
                Entities = new[]
                {
                    new UnityEnhancedEntitySnapshot
                    {
                        StableIdentity = fireballIdentity,
                        Generation = 8,
                        Position = new Vector3(1f, 2f, 3f),
                        Rotation = Quaternion.identity,
                        Visible = true,
                        PresentationFamily = (int)UnityEnhancedPresentationFamily.Fire,
                        Scale = 1f,
                        RenderLayer = 2201
                    },
                    new UnityEnhancedEntitySnapshot
                    {
                        StableIdentity = 0x0000000840000002UL,
                        Generation = 8,
                        Position = new Vector3(2f, 2f, 3f),
                        Rotation = Quaternion.identity,
                        Visible = true,
                        PresentationFamily = (int)UnityEnhancedPresentationFamily.Fire,
                        Scale = 1f,
                        RenderLayer = 3002
                    }
                },
                NightLighting = UnityEnhancedNightLightingDescriptor.Level1Night(),
                LocalLights = new[]
                {
                    new UnityEnhancedLocalLightRequest(
                        fireballIdentity,
                        new Vector3(1f, 2f, 3f),
                        new Color(1f, 0.2f, 0.04f, 1f),
                        5.5f,
                        11f,
                        2201)
                },
                VfxEvents = new[]
                {
                    new UnityEnhancedVfxEvent(
                        1,
                        8,
                        12,
                        fireballIdentity,
                        UnityEnhancedVfxKind.Fire,
                        new Vector3(1f, 2f, 3f),
                        1f,
                        64)
                }
            }));

            Transform visual = gameObject.transform.Find(
                "EnhancedWorldObjects/EnhancedEntity_0000000820000001/EnhancedActorVisual");
            Assert.IsNotNull(visual);
            MeshRenderer renderer = visual.GetComponent<MeshRenderer>();
            Assert.AreEqual("MagicCarpet/Enhanced/Fireball", renderer.sharedMaterial.shader.name);
            Assert.Greater(renderer.sharedMaterial.GetFloat("_CoreIntensity"), 3f);
            Assert.Greater(renderer.sharedMaterial.GetFloat("_HaloIntensity"), 0f);
            Assert.Less(
                renderer.sharedMaterial.GetFloat("_CoreRadius"),
                renderer.sharedMaterial.GetFloat("_HaloRadius"));
            Assert.Greater(renderer.sharedMaterial.GetFloat("_ShimmerStrength"), 0f);
            Assert.That(visual.localScale.x, Is.EqualTo(2.8f).Within(0.001f));
            Transform trail = gameObject.transform.Find(
                "EnhancedWorldObjects/EnhancedEntity_0000000820000001/EnhancedFireballTrail");
            Assert.IsNotNull(trail);
            TrailRenderer trailRenderer = trail.GetComponent<TrailRenderer>();
            Assert.IsNotNull(trailRenderer);
            Assert.AreEqual("MagicCarpet/Enhanced/FireballTrail", trailRenderer.sharedMaterial.shader.name);
            Assert.That(trailRenderer.time, Is.EqualTo(0.24f).Within(0.001f));
            Assert.That(trailRenderer.startWidth, Is.EqualTo(0.38f).Within(0.001f));
            Assert.That(trailRenderer.endWidth, Is.EqualTo(0.025f).Within(0.001f));
            Assert.IsTrue(trailRenderer.enabled);
            Assert.IsFalse(trailRenderer.emitting);
            Assert.AreEqual(1, trailRenderer.positionCount);
            Transform plume = gameObject.transform.Find(
                "EnhancedWorldObjects/EnhancedEntity_0000000820000001/EnhancedFireballPlume");
            Assert.IsNotNull(plume);
            MeshRenderer plumeRenderer = plume.GetComponent<MeshRenderer>();
            Assert.AreEqual("MagicCarpet/Enhanced/FireballTrail", plumeRenderer.sharedMaterial.shader.name);
            Assert.IsTrue(plume.gameObject.activeSelf);
            Assert.AreEqual(6, plume.GetComponent<MeshFilter>().sharedMesh.vertexCount);
            Assert.Greater(plume.localScale.y, plume.localScale.x * 2f);
            Assert.That(plume.position.x, Is.EqualTo(visual.position.x).Within(0.001f));
            Assert.That(plume.position.y, Is.EqualTo(visual.position.y).Within(0.001f));
            Assert.That(plume.position.z, Is.EqualTo(visual.position.z).Within(0.001f));
            Transform halo = gameObject.transform.Find(
                "EnhancedWorldObjects/EnhancedEntity_0000000820000001/EnhancedFireballHalo");
            Assert.IsNotNull(halo);
            MeshRenderer haloRenderer = halo.GetComponent<MeshRenderer>();
            Assert.AreEqual("MagicCarpet/Enhanced/Fireball", haloRenderer.sharedMaterial.shader.name);
            Assert.That(halo.localScale.x, Is.EqualTo(8f).Within(0.001f));
            Assert.Greater(
                haloRenderer.sharedMaterial.GetFloat("_HaloIntensity"),
                renderer.sharedMaterial.GetFloat("_HaloIntensity"));
            Assert.That(haloRenderer.sharedMaterial.GetFloat("_CoreIntensity"), Is.EqualTo(0f));
            Assert.That(
                haloRenderer.sharedMaterial.GetFloat("_OuterRadius"),
                Is.GreaterThan(haloRenderer.sharedMaterial.GetFloat("_HaloRadius")));
            Assert.Greater(haloRenderer.sharedMaterial.GetFloat("_OuterIntensity"), 0f);
            Assert.That(renderer.sharedMaterial.GetFloat("_OuterIntensity"), Is.EqualTo(0f));
            Assert.That(haloRenderer.bounds.center.x, Is.EqualTo(renderer.bounds.center.x).Within(0.001f));
            Assert.That(haloRenderer.bounds.center.y, Is.EqualTo(renderer.bounds.center.y).Within(0.001f));
            Assert.That(haloRenderer.bounds.center.z, Is.EqualTo(renderer.bounds.center.z).Within(0.001f));
            Assert.AreEqual(1, root.NightLighting.ActiveLocalLightCount);
            Light fireballLight = gameObject.transform
                .Find("EnhancedNightLights/EnhancedLocalLight_0")
                .GetComponent<Light>();
            Assert.That(fireballLight.intensity, Is.EqualTo(5.5f).Within(0.001f));
            Assert.That(fireballLight.range, Is.EqualTo(11f).Within(0.001f));
            Assert.AreEqual(1, root.VfxPresentation.ActiveEffectCount);

            Transform impactVisual = gameObject.transform.Find(
                "EnhancedWorldObjects/EnhancedEntity_0000000840000002/EnhancedActorVisual");
            Assert.IsNotNull(impactVisual);
            MeshRenderer impactRenderer = impactVisual.GetComponent<MeshRenderer>();
            Assert.AreEqual("MagicCarpet/Enhanced/Vfx", impactRenderer.sharedMaterial.shader.name);
            Assert.That(impactVisual.localScale.x, Is.EqualTo(1f).Within(0.001f));
            Assert.IsNull(gameObject.transform.Find(
                "EnhancedWorldObjects/EnhancedEntity_0000000840000002/EnhancedFireballHalo"));
            Assert.IsNull(impactVisual.GetComponentInChildren<TrailRenderer>(true));
            Assert.IsNull(gameObject.transform.Find(
                "EnhancedWorldObjects/EnhancedEntity_0000000840000002/EnhancedFireballPlume"));

            Assert.IsTrue(root.ApplySnapshot(new UnityEnhancedWorldSnapshot
            {
                Generation = 8,
                Tick = 13,
                CameraPose = new UnityEnhancedCameraPose(Vector3.zero, Quaternion.identity, 70f),
                Entities = new[]
                {
                    new UnityEnhancedEntitySnapshot
                    {
                        StableIdentity = fireballIdentity,
                        Generation = 8,
                        Position = new Vector3(1.35f, 2f, 3f),
                        Rotation = Quaternion.identity,
                        Visible = true,
                        PresentationFamily = (int)UnityEnhancedPresentationFamily.Fire
                    },
                    new UnityEnhancedEntitySnapshot
                    {
                        StableIdentity = 0x0000000840000002UL,
                        Generation = 8,
                        Position = new Vector3(2f, 2f, 3f),
                        Rotation = Quaternion.identity,
                        Visible = true,
                        PresentationFamily = (int)UnityEnhancedPresentationFamily.Fire
                    }
                }
            }));
            Transform movedTrail = gameObject.transform.Find(
                "EnhancedWorldObjects/EnhancedEntity_0000000820000001/EnhancedFireballTrail");
            TrailRenderer movedTrailRenderer = movedTrail.GetComponent<TrailRenderer>();
            Assert.Greater(movedTrailRenderer.positionCount, 1);
            Assert.IsFalse(movedTrailRenderer.emitting);
            Transform movedPlume = gameObject.transform.Find(
                "EnhancedWorldObjects/EnhancedEntity_0000000820000001/EnhancedFireballPlume");
            Assert.IsTrue(movedPlume.gameObject.activeSelf);
            Assert.That(movedPlume.position.x, Is.EqualTo(visual.position.x).Within(0.001f));
            Assert.That(movedPlume.position.y, Is.EqualTo(visual.position.y).Within(0.001f));
            Assert.That(movedPlume.position.z, Is.EqualTo(visual.position.z).Within(0.001f));

            Assert.IsTrue(root.ApplySnapshot(new UnityEnhancedWorldSnapshot
            {
                Generation = 8,
                Tick = 14,
                CameraPose = new UnityEnhancedCameraPose(Vector3.zero, Quaternion.identity, 70f),
                Entities = new[]
                {
                    new UnityEnhancedEntitySnapshot
                    {
                        StableIdentity = fireballIdentity,
                        Generation = 8,
                        Position = new Vector3(1.35f, 2f, 3f),
                        Rotation = Quaternion.identity,
                        Visible = false,
                        PresentationFamily = (int)UnityEnhancedPresentationFamily.Fire
                    },
                    new UnityEnhancedEntitySnapshot
                    {
                        StableIdentity = 0x0000000840000002UL,
                        Generation = 8,
                        Position = new Vector3(2f, 2f, 3f),
                        Rotation = Quaternion.identity,
                        Visible = true,
                        PresentationFamily = (int)UnityEnhancedPresentationFamily.Fire
                    }
                }
            }));
            Assert.IsTrue(root.ApplySnapshot(new UnityEnhancedWorldSnapshot
            {
                Generation = 8,
                Tick = 15,
                CameraPose = new UnityEnhancedCameraPose(Vector3.zero, Quaternion.identity, 70f),
                Entities = new[]
                {
                    new UnityEnhancedEntitySnapshot
                    {
                        StableIdentity = fireballIdentity,
                        Generation = 8,
                        Position = new Vector3(1f, 2f, 3f),
                        Rotation = Quaternion.identity,
                        Visible = true,
                        PresentationFamily = (int)UnityEnhancedPresentationFamily.Generic
                    },
                    new UnityEnhancedEntitySnapshot
                    {
                        StableIdentity = 0x0000000840000002UL,
                        Generation = 8,
                        Position = new Vector3(2f, 2f, 3f),
                        Rotation = Quaternion.identity,
                        Visible = true,
                        PresentationFamily = (int)UnityEnhancedPresentationFamily.Fire
                    }
                }
            }));
            Transform reusedTrail = gameObject.transform.Find(
                "EnhancedWorldObjects/EnhancedEntity_0000000820000001/EnhancedFireballTrail");
            Assert.IsNotNull(reusedTrail);
            TrailRenderer reusedTrailRenderer = reusedTrail.GetComponent<TrailRenderer>();
            Assert.IsFalse(reusedTrailRenderer.enabled);
            Assert.IsFalse(reusedTrailRenderer.emitting);
            Assert.AreEqual(0, reusedTrailRenderer.positionCount);
            Assert.IsFalse(movedPlume.gameObject.activeSelf);

            Object.DestroyImmediate(gameObject);
        }

        [Test]
        public void ReducedMotionSuppressesFireballPlumeButKeepsAuthoritativeProjectile()
        {
            GameObject gameObject = new GameObject("enhanced-fireball-reduced-motion-test");
            UnityEnhancedWorldRoot root = gameObject.AddComponent<UnityEnhancedWorldRoot>();
            root.OpenDiagnostic(PresentationModeId.UnityEnhanced);
            UnityEnhancedQualitySettings quality =
                UnityEnhancedQualitySettings.ForTier(UnityEnhancedQualityTier.WholeHog);
            quality.ReducedMotion = true;
            quality.CameraShake = false;
            quality.SpeedStreaks = false;
            quality.MotionBlur = false;

            Assert.IsTrue(root.ApplySnapshot(new UnityEnhancedWorldSnapshot
            {
                Generation = 9,
                Tick = 1,
                CameraPose = new UnityEnhancedCameraPose(Vector3.zero, Quaternion.identity, 70f),
                Entities = new[]
                {
                    new UnityEnhancedEntitySnapshot
                    {
                        StableIdentity = 0x0000000920000001UL,
                        Generation = 9,
                        Position = new Vector3(0f, 0f, 5f),
                        Rotation = Quaternion.identity,
                        Visible = true,
                        PresentationFamily = (int)UnityEnhancedPresentationFamily.Fire
                    }
                },
                Quality = quality
            }));

            Transform core = gameObject.transform.Find(
                "EnhancedWorldObjects/EnhancedEntity_0000000920000001/EnhancedActorVisual");
            Transform plume = gameObject.transform.Find(
                "EnhancedWorldObjects/EnhancedEntity_0000000920000001/EnhancedFireballPlume");
            Assert.IsTrue(core.gameObject.activeSelf);
            Assert.IsFalse(plume.gameObject.activeSelf);
            Assert.IsTrue(root.VfxPresentation.ReducedMotion);
            Assert.IsTrue(root.ActorPresentation != null);

            Object.DestroyImmediate(gameObject);
        }

        [Test]
        public void FireballCaptureGateRequiresFrontInViewportRenderer()
        {
            GameObject gameObject = new GameObject("enhanced-fireball-visibility-test");
            UnityEnhancedWorldRoot root = gameObject.AddComponent<UnityEnhancedWorldRoot>();
            root.OpenDiagnostic(PresentationModeId.UnityEnhanced);
            ulong fireballIdentity = 0x0000000820000001UL;
            UnityEnhancedWorldSnapshot snapshot = new UnityEnhancedWorldSnapshot
            {
                Generation = 8,
                Tick = 1,
                CameraPose = new UnityEnhancedCameraPose(Vector3.zero, Quaternion.identity, 70f),
                Entities = new[]
                {
                    new UnityEnhancedEntitySnapshot
                    {
                        StableIdentity = fireballIdentity,
                        Generation = 8,
                        Position = new Vector3(0f, 0f, 5f),
                        Rotation = Quaternion.identity,
                        Visible = true,
                        PresentationFamily = (int)UnityEnhancedPresentationFamily.Fire,
                        RenderLayer = 2201
                    }
                }
            };

            Assert.IsTrue(root.ApplySnapshot(snapshot));
            UnityEnhancedFireballViewportEvidence evidence;
            Assert.IsTrue(root.TryGetFireballViewportEvidence(1280, 720, out evidence));
            Assert.IsTrue(evidence.RendererFound);
            Assert.IsTrue(evidence.RendererActive);
            Assert.IsTrue(evidence.CenterInFront);
            Assert.IsTrue(evidence.CenterInViewport);
            Assert.IsTrue(evidence.CaptureReady);
            Assert.That(evidence.ViewportCenter.x, Is.EqualTo(0.5f).Within(0.02f));

            snapshot.Tick = 2;
            snapshot.Entities[0].Position = new Vector3(0f, 0f, -5f);
            Assert.IsTrue(root.ApplySnapshot(snapshot));
            Assert.IsTrue(root.TryGetFireballViewportEvidence(1280, 720, out evidence));
            Assert.IsFalse(evidence.CenterInFront);
            Assert.IsFalse(evidence.CaptureReady);

            snapshot.Tick = 3;
            snapshot.Entities[0].Position = new Vector3(100f, 0f, 5f);
            Assert.IsTrue(root.ApplySnapshot(snapshot));
            Assert.IsTrue(root.TryGetFireballViewportEvidence(1280, 720, out evidence));
            Assert.IsTrue(evidence.CenterInFront);
            Assert.IsFalse(evidence.CenterInViewport);
            Assert.IsFalse(evidence.CaptureReady);

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
            Assert.AreEqual(0, lighting.ShaderVisibleWarmLightCount);
            lighting.ResetForGeneration(6);
            Assert.AreEqual(0, lighting.ActiveLocalLightCount);
            Assert.AreEqual(0, Shader.GetGlobalInt("_EnhancedWarmLightCount"));

            Object.DestroyImmediate(gameObject);
        }

        [Test]
        public void WarmLightShaderContractPublishesFireballAndClearsCoolOrStaleRequests()
        {
            string terrainSource = File.ReadAllText(
                Path.Combine(Application.dataPath, "Resources", "EnhancedTerrain.shader"));
            string waterSource = File.ReadAllText(
                Path.Combine(Application.dataPath, "Resources", "EnhancedWater.shader"));
            StringAssert.Contains("_EnhancedWarmLightPositions", terrainSource);
            StringAssert.Contains("_EnhancedWarmLightPositions", waterSource);
            StringAssert.Contains("_ADDITIONAL_LIGHTS", waterSource);

            GameObject gameObject = new GameObject("enhanced-warm-light-contract-test");
            UnityEnhancedNightLighting lighting = gameObject.AddComponent<UnityEnhancedNightLighting>();
            UnityEnhancedLocalLightRequest fireball = new UnityEnhancedLocalLightRequest(
                0x20000001UL,
                new Vector3(3f, 2f, 1f),
                new Color(1f, 0.2f, 0.04f, 1f),
                5.5f,
                11f,
                2001);
            UnityEnhancedLocalLightRequest cool = new UnityEnhancedLocalLightRequest(
                0x10000001UL,
                new Vector3(-7f, 1f, 2f),
                new Color(0.15f, 0.35f, 1f, 1f),
                5f,
                11f,
                3000);

            lighting.ApplyDescriptor(10, UnityEnhancedNightLightingDescriptor.Level1Night(),
                new[] { cool, fireball }, true);
            Assert.AreEqual(1, lighting.ShaderVisibleWarmLightCount);
            Assert.AreEqual(1, Shader.GetGlobalInt("_EnhancedWarmLightCount"));
            Vector4 publishedPosition = Shader.GetGlobalVectorArray("_EnhancedWarmLightPositions")[0];
            Assert.That(publishedPosition.x, Is.EqualTo(3f).Within(0.001f));
            Assert.That(publishedPosition.y, Is.EqualTo(2f).Within(0.001f));
            Assert.That(publishedPosition.z, Is.EqualTo(1f).Within(0.001f));
            Vector4 publishedRange = Shader.GetGlobalVectorArray("_EnhancedWarmLightRanges")[0];
            Assert.That(publishedRange.x, Is.EqualTo(22f).Within(0.001f));

            lighting.ApplyDescriptor(10, UnityEnhancedNightLightingDescriptor.Level1Night(),
                new[] { cool }, false);
            Assert.AreEqual(0, lighting.ShaderVisibleWarmLightCount);
            Assert.AreEqual(0, Shader.GetGlobalInt("_EnhancedWarmLightCount"));
            Assert.AreEqual(0f, Shader.GetGlobalVectorArray("_EnhancedWarmLightPositions")[0].magnitude);
            Assert.AreEqual(1, lighting.ActiveLocalLightCount);

            lighting.ResetForSession();
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
        public void WaterPlanarReflectionUsesBoundedEnhancedCameraContract()
        {
            string waterPresentationSource = File.ReadAllText(
                Path.Combine(Application.dataPath, "Scripts", "UnityEnhancedWaterPresentation.cs"));
            string worldRootSource = File.ReadAllText(
                Path.Combine(Application.dataPath, "Scripts", "UnityEnhancedWorldRoot.cs"));
            string shaderSource = File.ReadAllText(
                Path.Combine(Application.dataPath, "Resources", "EnhancedWater.shader"));

            StringAssert.Contains("SetPresentationCamera", waterPresentationSource);
            StringAssert.Contains("CameraType.Reflection", waterPresentationSource);
            StringAssert.Contains("CalculateReflectionMatrix", waterPresentationSource);
            StringAssert.Contains("CameraSpacePlane", waterPresentationSource);
            StringAssert.Contains("GL.invertCulling", waterPresentationSource);
            StringAssert.Contains("MaximumReflectionDimension = 512", waterPresentationSource);
            StringAssert.Contains("LayerMask.NameToLayer(WaterLayerName)", waterPresentationSource);
            StringAssert.Contains("LayerMask.NameToLayer(UiLayerName)", waterPresentationSource);
            StringAssert.Contains("waterPresentation.RenderReflection()", worldRootSource);
            StringAssert.Contains("_PlanarReflectionTexture", shaderSource);
            StringAssert.Contains("_PlanarReflectionEnabled", shaderSource);
            StringAssert.Contains("SAMPLE_TEXTURE2D", shaderSource);
            StringAssert.DoesNotContain(
                "half3 reflection = lerp(_ReflectionColor.rgb, sceneReflection, _SceneReflectionStrength);",
                shaderSource);
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
            Assert.IsNotNull(water.WaterMaterial);
            Assert.AreEqual("MagicCarpet/Enhanced/Water", water.WaterMaterial.shader.name);
            Assert.Greater(water.WaterMaterial.GetFloat("_WaveStrength"), 0.20f);
            Assert.Greater(water.WaterMaterial.GetFloat("_FresnelStrength"), 1.0f);
            Assert.Greater(water.WaterMaterial.GetFloat("_SceneReflectionStrength"), 0.5f);

            UnityEnhancedVfxEvent splash = new UnityEnhancedVfxEvent(
                1, 2, 4, 0x40000001UL, UnityEnhancedVfxKind.Weather,
                new Vector3(1f, 1.5f, 1f), 1f, 18);
            Assert.IsTrue(water.ApplyRipple(splash));
            Assert.AreEqual(1, water.ActiveRippleCount);
            Assert.Greater(water.WaterMaterial.GetFloat("_RippleStrength"), 0.4f);
            UnityEnhancedVfxEvent nonWater = new UnityEnhancedVfxEvent(
                2, 2, 4, 0x20000001UL, UnityEnhancedVfxKind.Fire,
                Vector3.zero, 1f, 18);
            Assert.IsFalse(water.ApplyRipple(nonWater));

            Camera camera = gameObject.AddComponent<Camera>();
            UniversalAdditionalCameraData cameraData = gameObject.AddComponent<UniversalAdditionalCameraData>();
            cameraData.requiresColorTexture = false;
            UnityEnhancedQualityAndPost quality = gameObject.AddComponent<UnityEnhancedQualityAndPost>();
            Assert.IsTrue(quality.Apply(UnityEnhancedQualitySettings.ForTier(UnityEnhancedQualityTier.WholeHog), camera));
            Assert.IsTrue(quality.OpaqueTextureRequired);
            Assert.IsNotNull(cameraData);
            Assert.IsTrue(cameraData.requiresColorTexture);
            quality.ResetForSession();
            Assert.IsFalse(quality.OpaqueTextureRequired);
            Assert.IsFalse(cameraData.requiresColorTexture);
            MeshFilter waterSurface = gameObject.transform.Find("EnhancedWaterSurface").GetComponent<MeshFilter>();
            MeshFilter shoreline = gameObject.transform.Find("EnhancedShoreline").GetComponent<MeshFilter>();
            Assert.That(waterSurface.sharedMesh.vertices[0].y, Is.EqualTo(snapshot.Level + 0.08f).Within(0.0001f));
            Assert.That(shoreline.sharedMesh.vertices[0].y, Is.EqualTo(snapshot.Level + 0.092f).Within(0.0001f));
            Assert.Greater(shoreline.sharedMesh.vertices[0].y, waterSurface.sharedMesh.vertices[0].y);
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
            Assert.Less(renderer.sharedMaterial.GetFloat("_GlowIntensity"), 2f);
            Assert.Less(renderer.sharedMaterial.GetFloat("_Opacity"), 0.8f);
            Assert.Less(renderer.GetComponent<MeshFilter>().sharedMesh.bounds.size.x, 1f);
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
            Assert.IsNotNull(Shader.Find("MagicCarpet/Enhanced/OwnerHudIndexed"));
        }

        [Test]
        public void TownAndCastleCarryReadableNightStructureAccents()
        {
            GameObject gameObject = new GameObject("enhanced-structure-accent-test");
            UnityEnhancedActorPresentation actors = gameObject.AddComponent<UnityEnhancedActorPresentation>();
            GameObject view = new GameObject("structure-view");
            view.transform.SetParent(gameObject.transform, false);

            UnityEnhancedEntitySnapshot town = Entity(1, 42, Vector3.zero);
            town.PresentationFamily = (int)UnityEnhancedPresentationFamily.Town;
            actors.Configure(view, town);
            MeshRenderer renderer = view.transform.Find("EnhancedActorVisual").GetComponent<MeshRenderer>();
            Assert.Greater(renderer.sharedMaterial.GetFloat("_AccentStrength"), 0.9f);

            UnityEnhancedEntitySnapshot castle = Entity(1, 42, Vector3.zero);
            castle.PresentationFamily = (int)UnityEnhancedPresentationFamily.Castle;
            actors.Configure(view, castle);
            Assert.Greater(renderer.sharedMaterial.GetFloat("_AccentStrength"), 0.7f);

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

        private static UnityEnhancedWorldSnapshot OwnerHudWorldSnapshot(ulong generation, ulong tick)
        {
            return new UnityEnhancedWorldSnapshot
            {
                Generation = generation,
                Tick = tick,
                CameraPose = new UnityEnhancedCameraPose(Vector3.zero, Quaternion.identity, 70f),
                Entities = new UnityEnhancedEntitySnapshot[0],
                Hud = OwnerHudSnapshot(generation, tick)
            };
        }

        private static UnityEnhancedHudSnapshot OwnerHudSnapshot(ulong generation, ulong tick)
        {
            byte[] palette = new byte[256 * 3];
            palette[7 * 3] = 11;
            palette[7 * 3 + 1] = 22;
            palette[7 * 3 + 2] = 33;
            palette[9 * 3] = 44;
            palette[9 * 3 + 1] = 55;
            palette[9 * 3 + 2] = 66;
            return new UnityEnhancedHudSnapshot
            {
                Generation = generation,
                Tick = tick,
                OwnerFrame = new UnityEnhancedOwnerHudFrame
                {
                    Generation = generation,
                    Tick = tick,
                    Width = 2,
                    Height = 1,
                    Stride = 2,
                    IndexedPixels = new byte[] { 7, 9 },
                    Provenance = new byte[] { 0, 1 << 3 },
                    PaletteRgb8 = palette
                }
            };
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

        private static UnityEnhancedMinimapSnapshot MinimapSnapshot(
            ulong generation,
            ulong tick,
            ushort order)
        {
            short[] markX = new short[UnityEnhancedMinimapSnapshot.MaxMarks];
            short[] markY = new short[UnityEnhancedMinimapSnapshot.MaxMarks];
            byte[] markColour = new byte[UnityEnhancedMinimapSnapshot.MaxMarks];
            byte[] markHelpSelector = new byte[UnityEnhancedMinimapSnapshot.MaxMarks];
            markX[0] = 32;
            markY[0] = 28;
            markColour[0] = 184;
            markHelpSelector[0] = 0x51;
            return new UnityEnhancedMinimapSnapshot
            {
                Generation = generation,
                Tick = tick,
                MarkCount = 1,
                MarkPulse = true,
                ArrowValid = true,
                ArrowVisible = true,
                MarkX = markX,
                MarkY = markY,
                MarkColour = markColour,
                MarkHelpSelector = markHelpSelector,
                ArrowXY = new short[UnityEnhancedMinimapSnapshot.ArrowCoordinateCount],
                TotalEntityCount = 1,
                CrossX = 63,
                CrossY = 64,
                CrossColour = 176,
                CrossArmLength = 6,
                Authority = UnityEnhancedMinimapSnapshot.Level1MissionAuthority,
                CommandHash = 0x1234UL,
                EntityCommands = new[]
                {
                    new UnityEnhancedMinimapEntityCommand
                    {
                        SourceSlot = 4,
                        X = 40,
                        Y = 44,
                        Kind = UnityEnhancedMinimapSnapshot.DotCommand,
                        PaletteIndex = 23,
                        Size = 1,
                        Anchor = 1,
                        Order = order
                    }
                }
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
