#if UNITY_EDITOR
using System;
using System.Collections.Generic;
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
        public void HudUsesOwnerIndexedFrameWithoutSyntheticCompanionOverlay()
        {
            GameObject gameObject = new GameObject("enhanced-hud-readable-test");
            UnityEnhancedWorldRoot root = gameObject.AddComponent<UnityEnhancedWorldRoot>();
            root.OpenDiagnostic(PresentationModeId.UnityEnhanced);

            UnityEnhancedWorldSnapshot snapshot = OwnerHudWorldSnapshot(3, 4);
            snapshot.Hud.PlayerLife = 37;
            snapshot.Hud.PlayerMaxLife = 100;
            snapshot.Hud.PlayerMana = 12;
            snapshot.Hud.PlayerMaxMana = 50;
            snapshot.Hud.ActualSpeed = 44;
            snapshot.Hud.TargetSpeed = 80;
            snapshot.Hud.CurrentStage = 2;
            snapshot.Hud.TasksCompleted = 1;
            snapshot.Hud.ObjectiveCount = 3;
            snapshot.Hud.HasFireball = true;
            snapshot.Hud.HasSpeedSpell = true;
            snapshot.Hud.SelectedSpellUiIndex = 2;

            Assert.IsTrue(root.ApplySnapshot(snapshot));
            Assert.IsNull(gameObject.transform.Find(
                "EnhancedHud/EnhancedOwnerHudCanvas/EnhancedHudReadabilityOverlay"));
            Assert.IsNotNull(gameObject.transform.Find(
                "EnhancedHud/EnhancedOwnerHudCanvas/EnhancedOwnerHudFrame"));
            Assert.IsNull(gameObject.transform.Find(
                "EnhancedHud/EnhancedOwnerHudCanvas/EnhancedHudPanel"));
            Assert.IsNotNull(root.HudPresentation.OwnerHudMaterial);

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
            const uint validFlags = 0x4000u | 0x8000u | 0x10000u | 0x400000u;

            Assert.IsTrue(UnityEnhancedOwnerSessionSource.CanReuseOwnerHudFrame(
                2,
                128,
                7,
                12,
                7,
                12,
                1,
                1,
                640,
                480,
                640,
                640u * 480u,
                640u * 480u,
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
                640,
                480,
                640,
                640u * 480u,
                640u * 480u,
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
                640,
                480,
                640,
                640u * 480u,
                640u * 480u,
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
                640,
                480,
                640,
                640u * 480u,
                640u * 480u,
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
                640,
                480,
                640,
                640u * 480u,
                640u * 480u,
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
                640,
                480,
                640,
                640u * 480u,
                640u * 480u,
                validFlags,
                1,
                0));
        }

        [Test]
        public void EnhancedOwnerHudUsesProfileSizedClassicHdOverlay()
        {
            Assert.AreEqual(640, UnityEnhancedOwnerSessionSource.EnhancedOwnerHudWidth);
            Assert.AreEqual(480, UnityEnhancedOwnerSessionSource.EnhancedOwnerHudHeight);
            Assert.AreEqual(1u, UnityEnhancedOwnerSessionSource.EnhancedOwnerHudProfileId);

            string source = File.ReadAllText(Path.Combine(
                Application.dataPath,
                "Scripts",
                "UnityEnhancedOwnerSessionSource.cs"));
            StringAssert.Contains("mc_native_session_classic_hd_owner_overlay_copy", source);
            StringAssert.Contains("0x400000u", source);
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
                "int family = IsLuminousFireballProjectileKind(source.projectileKind)",
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
            Assert.That(renderer.sharedMaterial.GetFloat("_DetailScale"), Is.EqualTo(0.16f).Within(0.0001f));
            Assert.That(renderer.sharedMaterial.GetFloat("_DetailStrength"), Is.EqualTo(0.44f).Within(0.0001f));
            Assert.That(renderer.sharedMaterial.GetFloat("_SlopeStrength"), Is.EqualTo(0.50f).Within(0.0001f));
            Assert.That(renderer.sharedMaterial.GetFloat("_NormalStrength"), Is.EqualTo(0.56f).Within(0.0001f));
            Assert.That(renderer.sharedMaterial.GetFloat("_RidgeStrength"), Is.EqualTo(0.78f).Within(0.0001f));
            Assert.That(renderer.sharedMaterial.GetFloat("_MicroDetailScale"), Is.EqualTo(0.90f).Within(0.0001f));
            Assert.That(renderer.sharedMaterial.GetFloat("_MicroDetailStrength"), Is.EqualTo(0.42f).Within(0.0001f));
            Assert.That(renderer.sharedMaterial.GetFloat("_MicroReliefStrength"), Is.EqualTo(0.30f).Within(0.0001f));
            Assert.That(renderer.sharedMaterial.GetFloat("_MicroDetailNearStart"), Is.EqualTo(4f).Within(0.0001f));
            Assert.That(renderer.sharedMaterial.GetFloat("_MicroDetailNearEnd"), Is.EqualTo(112f).Within(0.0001f));
            string terrainSource = File.ReadAllText(
                Path.Combine(Application.dataPath, "Resources", "EnhancedTerrain.shader"));
            StringAssert.Contains("grainMask", terrainSource);
            StringAssert.Contains("grainTint", terrainSource);

            Object.DestroyImmediate(gameObject);
        }

        [Test]
        public void EnhancedTerrainNearFieldSharpeningIsBoundedAndDistanceFaded()
        {
            GameObject gameObject = new GameObject("enhanced-terrain-near-detail-test");
            UnityEnhancedTerrainPresentation terrain =
                gameObject.AddComponent<UnityEnhancedTerrainPresentation>();

            Assert.IsTrue(terrain.ApplySnapshot(TerrainSnapshot(
                15,
                1,
                true,
                new UnityEnhancedTerrainDirtyRegion[0])));

            MeshRenderer renderer = gameObject.GetComponent<MeshRenderer>();
            Assert.IsNotNull(renderer);
            Assert.AreEqual("MagicCarpet/Enhanced/Terrain", renderer.sharedMaterial.shader.name);
            float sharpenStrength = renderer.sharedMaterial.GetFloat("_NearDetailSharpenStrength");
            Assert.That(sharpenStrength, Is.EqualTo(0.36f).Within(0.0001f));
            Assert.That(sharpenStrength, Is.InRange(0f, 0.5f));

            float maximumFactorDelta = 0.30f * sharpenStrength;
            Assert.That(1f - maximumFactorDelta, Is.GreaterThanOrEqualTo(0.85f));
            Assert.That(1f + maximumFactorDelta, Is.LessThanOrEqualTo(1.15f));

            string terrainSource = File.ReadAllText(
                Path.Combine(Application.dataPath, "Resources", "EnhancedTerrain.shader"));
            StringAssert.Contains(
                "_NearDetailSharpenStrength (\"Near Detail Sharpen Strength\", Range(0, 0.5)) = 0.36",
                terrainSource);
            StringAssert.Contains(
                "half nearDetailHighPass = clamp((half)(grain - microDetail), -0.30h, 0.30h);",
                terrainSource);
            StringAssert.Contains(
                "nearDetailHighPass * _NearDetailSharpenStrength * nearSurfaceMask",
                terrainSource);

            Object.DestroyImmediate(gameObject);
        }

        [Test]
        public void TerrainSelectorMaterialKeepsWaterDistinctFromLand()
        {
            GameObject gameObject = new GameObject("enhanced-terrain-selector-material-test");
            UnityEnhancedTerrainPresentation terrain = gameObject.AddComponent<UnityEnhancedTerrainPresentation>();
            UnityEnhancedTerrainSnapshot snapshot = TerrainSnapshot(
                3,
                0,
                true,
                new UnityEnhancedTerrainDirtyRegion[0]);
            snapshot.TerrainTypes[0] = 0;
            snapshot.TerrainTypes[1] = 2;
            snapshot.TerrainTypes[2] = 22;

            Assert.IsTrue(terrain.ApplySnapshot(snapshot));
            Color water = terrain.GeneratedMesh.colors[0];
            Color land = terrain.GeneratedMesh.colors[1];
            Color rock = terrain.GeneratedMesh.colors[2];

            Assert.That(Vector3.Distance(
                    new Vector3(water.r, water.g, water.b),
                    new Vector3(land.r, land.g, land.b)),
                Is.GreaterThan(0.20f));
            Assert.That(Vector3.Distance(
                    new Vector3(land.r, land.g, land.b),
                    new Vector3(rock.r, rock.g, rock.b)),
                Is.GreaterThan(0.12f));
            Assert.That(land.r, Is.GreaterThan(rock.r));
            Assert.That(land.g, Is.GreaterThan(rock.g));

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

            Assert.AreEqual(4, root.TerrainPresentation.RenderSubdivision);
            Assert.AreEqual(81, root.TerrainPresentation.VertexCount);
            Assert.AreEqual(IndexFormat.UInt16,
                root.TerrainPresentation.GeneratedMesh.indexFormat);
            Assert.That(root.TerrainPresentation.GeneratedMesh.vertices[4].y,
                Is.EqualTo(2f).Within(0.001f));
            Assert.That(root.TerrainPresentation.GeneratedMesh.vertices[36].y,
                Is.EqualTo(4f).Within(0.001f));
            Assert.That(root.TerrainPresentation.GeneratedMesh.vertices[20].y,
                Is.EqualTo(3f).Within(0.001f));

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
            Assert.AreEqual(UnityEnhancedActorPresentation.CoverageCount, 21);
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
                        AnimationPhase = 0.35f,
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
            Transform volume = gameObject.transform.Find(
                "EnhancedWorldObjects/EnhancedEntity_0000000820000001/EnhancedFireballVolume");
            Assert.IsNotNull(volume);
            MeshRenderer volumeRenderer = volume.GetComponent<MeshRenderer>();
            Assert.AreEqual("MagicCarpet/Enhanced/FireballVolume", volumeRenderer.sharedMaterial.shader.name);
            Assert.IsTrue(volume.gameObject.activeSelf);
            Assert.AreEqual(12, volume.GetComponent<MeshFilter>().sharedMesh.vertexCount);
            Assert.Greater(volume.localScale.y, volume.localScale.x * 2f);
            string volumeShaderSource = File.ReadAllText(
                Path.Combine(Application.dataPath, "Resources", "EnhancedFireballVolume.shader"));
            StringAssert.Contains("ValueNoise", volumeShaderSource);
            StringAssert.Contains("_ShimmerStrength", volumeShaderSource);
            Transform halo = gameObject.transform.Find(
                "EnhancedWorldObjects/EnhancedEntity_0000000820000001/EnhancedFireballHalo");
            Assert.IsNotNull(halo);
            MeshRenderer haloRenderer = halo.GetComponent<MeshRenderer>();
            Assert.AreEqual("MagicCarpet/Enhanced/Fireball", haloRenderer.sharedMaterial.shader.name);
            Assert.That(halo.localScale.x, Is.EqualTo(9.5f).Within(0.001f));
            Assert.Greater(
                haloRenderer.sharedMaterial.GetFloat("_HaloIntensity"),
                renderer.sharedMaterial.GetFloat("_HaloIntensity"));
            Assert.That(haloRenderer.sharedMaterial.GetFloat("_CoreIntensity"), Is.EqualTo(0f));
            Assert.That(
                haloRenderer.sharedMaterial.GetFloat("_OuterRadius"),
                Is.GreaterThan(haloRenderer.sharedMaterial.GetFloat("_HaloRadius")));
            Assert.Greater(haloRenderer.sharedMaterial.GetFloat("_OuterIntensity"), 0f);
            Assert.That(renderer.sharedMaterial.GetFloat("_OuterIntensity"), Is.EqualTo(0f));
            string fireballShaderSource = File.ReadAllText(
                Path.Combine(Application.dataPath, "Resources", "EnhancedFireball.shader"));
            StringAssert.Contains("half edgeFade", fireballShaderSource);
            StringAssert.Contains("smoothstep(0.78h, 0.99h, radius)", fireballShaderSource);
            string impactShaderSource = File.ReadAllText(
                Path.Combine(Application.dataPath, "Resources", "EnhancedImpact.shader"));
            StringAssert.Contains("shockwaveBand", impactShaderSource);
            StringAssert.Contains("_ImpactProgress", impactShaderSource);
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
            Assert.AreEqual("MagicCarpet/Enhanced/Impact", impactRenderer.sharedMaterial.shader.name);
            Color ownerShockwaveColor = impactRenderer.sharedMaterial.GetColor("_ShockwaveColor");
            Assert.That(ownerShockwaveColor.r, Is.EqualTo(1f).Within(0.001f));
            Assert.That(ownerShockwaveColor.g, Is.EqualTo(0.50f).Within(0.001f));
            Assert.That(ownerShockwaveColor.b, Is.EqualTo(0.06f).Within(0.001f));
            Assert.That(impactRenderer.sharedMaterial.GetFloat("_ShockwaveStrength"), Is.EqualTo(2.6f).Within(0.001f));
            Assert.That(impactRenderer.sharedMaterial.GetFloat("_SparkStrength"), Is.EqualTo(1.25f).Within(0.001f));
            Assert.That(impactRenderer.sharedMaterial.GetFloat("_RimStrength"), Is.EqualTo(0.65f).Within(0.001f));
            MaterialPropertyBlock impactPropertyBlock = new MaterialPropertyBlock();
            impactRenderer.GetPropertyBlock(impactPropertyBlock);
            Assert.That(impactPropertyBlock.GetFloat("_ImpactProgress"), Is.EqualTo(0.35f).Within(0.001f));
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
            Transform movedVolume = gameObject.transform.Find(
                "EnhancedWorldObjects/EnhancedEntity_0000000820000001/EnhancedFireballVolume");
            Assert.IsTrue(movedVolume.gameObject.activeSelf);
            Assert.That(movedVolume.position.x, Is.EqualTo(visual.position.x).Within(0.001f));
            Assert.That(movedVolume.position.y, Is.EqualTo(visual.position.y).Within(0.001f));
            Assert.That(movedVolume.position.z, Is.EqualTo(visual.position.z).Within(0.001f));

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
        public void AuthoritativeArcherArrowUsesDirectionalMeshWithoutLuminousProxy()
        {
            Assert.AreEqual(19, (int)UnityEnhancedPresentationFamily.Generic);
            Assert.AreEqual(20, (int)UnityEnhancedPresentationFamily.ArcherArrow);
            Assert.AreEqual(21, UnityEnhancedActorPresentation.CoverageCount);

            string ownerSource = File.ReadAllText(Path.Combine(
                Application.dataPath,
                "Scripts",
                "UnityEnhancedOwnerSessionSource.cs"));
            int buildEntitiesStart = ownerSource.IndexOf(
                "private UnityEnhancedEntitySnapshot[] BuildEntities()",
                StringComparison.Ordinal);
            int buildLightsStart = ownerSource.IndexOf(
                "private UnityEnhancedLocalLightRequest[] BuildLocalLights()",
                StringComparison.Ordinal);
            int presentationFamilyStart = ownerSource.IndexOf(
                "private static int PresentationFamily(",
                StringComparison.Ordinal);
            Assert.GreaterOrEqual(buildEntitiesStart, 0);
            Assert.Greater(buildLightsStart, buildEntitiesStart);
            Assert.Greater(presentationFamilyStart, buildLightsStart);

            string buildEntities = ownerSource.Substring(
                buildEntitiesStart,
                buildLightsStart - buildEntitiesStart);
            string buildLights = ownerSource.Substring(
                buildLightsStart,
                presentationFamilyStart - buildLightsStart);
            StringAssert.Contains("source.projectileKind == 2", buildEntities);
            StringAssert.Contains(
                "UnityEnhancedPresentationFamily.ArcherArrow",
                buildEntities);
            StringAssert.Contains(
                "int family = IsLuminousFireballProjectileKind(source.projectileKind)",
                buildEntities);
            StringAssert.Contains(
                "short signedPitch = SignedAngle11(source.pitch);",
                buildEntities);
            StringAssert.Contains(
                "float pitchRadians = signedPitch * (2f * Mathf.PI / 2048f);",
                buildEntities);
            StringAssert.Contains(
                "Mathf.Sin(pitchRadians) * EnhancedVerticalPresentationScale",
                buildEntities);
            StringAssert.Contains(
                "Mathf.Cos(pitchRadians)) * Mathf.Rad2Deg",
                buildEntities);
            float positiveSourceRadians = 256f * (2f * Mathf.PI / 2048f);
            float positivePresentationPitch = Mathf.Atan2(
                Mathf.Sin(positiveSourceRadians) *
                UnityEnhancedOwnerSessionSource.EnhancedVerticalPresentationScale,
                Mathf.Cos(positiveSourceRadians)) * Mathf.Rad2Deg;
            float negativePresentationPitch = Mathf.Atan2(
                Mathf.Sin(-positiveSourceRadians) *
                UnityEnhancedOwnerSessionSource.EnhancedVerticalPresentationScale,
                Mathf.Cos(-positiveSourceRadians)) * Mathf.Rad2Deg;
            Assert.Greater(positivePresentationPitch, 0f);
            Assert.Less(negativePresentationPitch, 0f);
            Assert.That(
                Mathf.Abs(positivePresentationPitch),
                Is.LessThan(256f * (360f / 2048f)));
            Assert.That(
                negativePresentationPitch,
                Is.EqualTo(-positivePresentationPitch).Within(0.0001f));
            StringAssert.Contains(
                "if (source.projectileKind == 2)\n" +
                "                {\n" +
                "                    continue;\n" +
                "                }",
                buildLights);
            StringAssert.Contains(
                "bool fireball = IsLuminousFireballProjectileKind(source.projectileKind);",
                buildLights);

            GameObject gameObject = new GameObject("enhanced-archer-arrow-test");
            try
            {
                UnityEnhancedWorldRoot root = gameObject.AddComponent<UnityEnhancedWorldRoot>();
                root.OpenDiagnostic(PresentationModeId.UnityEnhanced);
                const ulong generation = 31;
                const ulong fireballIdentity = 0x0000000020000001UL;
                const ulong arrowIdentity = 0x0000000020000002UL;
                Vector3 fireballPosition = new Vector3(1f, 2f, 3f);
                Vector3 arrowPosition = new Vector3(4f, 5f, 6f);
                Quaternion arrowRotation = Quaternion.Euler(17f, 43f, 0f);
                UnityEnhancedNightLightingDescriptor night =
                    UnityEnhancedNightLightingDescriptor.Level1Night();

                Assert.IsTrue(root.ApplySnapshot(new UnityEnhancedWorldSnapshot
                {
                    Generation = generation,
                    Tick = 0,
                    Reset = true,
                    CameraPose = new UnityEnhancedCameraPose(Vector3.zero, Quaternion.identity, 70f),
                    Entities = new[]
                    {
                        new UnityEnhancedEntitySnapshot
                        {
                            StableIdentity = fireballIdentity,
                            Generation = generation,
                            Position = fireballPosition,
                            Rotation = Quaternion.identity,
                            Visible = true,
                            PresentationFamily = (int)UnityEnhancedPresentationFamily.Fire,
                            Scale = 1f,
                            RenderLayer = 2201
                        }
                    },
                    NightLighting = night,
                    LocalLights = new[]
                    {
                        new UnityEnhancedLocalLightRequest(
                            fireballIdentity,
                            fireballPosition,
                            new Color(1f, 0.2f, 0.04f, 1f),
                            5.5f,
                            11f,
                            2201)
                    },
                    VfxEvents = Array.Empty<UnityEnhancedVfxEvent>()
                }));

                Transform fireballView = gameObject.transform.Find(
                    "EnhancedWorldObjects/EnhancedEntity_0000000020000001");
                Assert.IsNotNull(fireballView);
                Assert.AreEqual(
                    "MagicCarpet/Enhanced/Fireball",
                    fireballView.Find("EnhancedActorVisual").GetComponent<MeshRenderer>()
                        .sharedMaterial.shader.name);
                Assert.IsNotNull(fireballView.Find("EnhancedFireballHalo"));
                Assert.IsNotNull(fireballView.Find("EnhancedFireballTrail"));
                Assert.IsNotNull(fireballView.Find("EnhancedFireballPlume"));
                Assert.IsNotNull(fireballView.Find("EnhancedFireballVolume"));
                Assert.AreEqual(1, root.NightLighting.ActiveLocalLightCount);

                Assert.IsTrue(root.ApplySnapshot(new UnityEnhancedWorldSnapshot
                {
                    Generation = generation,
                    Tick = 1,
                    CameraPose = new UnityEnhancedCameraPose(Vector3.zero, Quaternion.identity, 70f),
                    Entities = Array.Empty<UnityEnhancedEntitySnapshot>(),
                    NightLighting = night,
                    LocalLights = Array.Empty<UnityEnhancedLocalLightRequest>(),
                    VfxEvents = Array.Empty<UnityEnhancedVfxEvent>()
                }));
                Assert.AreEqual(1, root.PooledEntityCount);
                Assert.AreEqual(1, root.ActorPresentation.PooledVisualCount);
                Assert.AreEqual(0, root.NightLighting.ActiveLocalLightCount);

                Assert.IsTrue(root.ApplySnapshot(new UnityEnhancedWorldSnapshot
                {
                    Generation = generation,
                    Tick = 2,
                    CameraPose = new UnityEnhancedCameraPose(Vector3.zero, Quaternion.identity, 70f),
                    Entities = new[]
                    {
                        new UnityEnhancedEntitySnapshot
                        {
                            StableIdentity = arrowIdentity,
                            Generation = generation,
                            Position = arrowPosition,
                            Rotation = arrowRotation,
                            Visible = true,
                            PresentationFamily = (int)UnityEnhancedPresentationFamily.ArcherArrow,
                            FacingDegrees = 179f,
                            Scale = 1f,
                            RenderLayer = 2202
                        }
                    },
                    NightLighting = night,
                    LocalLights = Array.Empty<UnityEnhancedLocalLightRequest>(),
                    VfxEvents = Array.Empty<UnityEnhancedVfxEvent>()
                }));

                Transform arrowView = gameObject.transform.Find(
                    "EnhancedWorldObjects/EnhancedEntity_0000000020000002");
                Assert.IsNotNull(arrowView);
                Transform arrowVisual = arrowView.Find("EnhancedActorVisual");
                Assert.IsNotNull(arrowVisual);
                MeshFilter arrowFilter = arrowVisual.GetComponent<MeshFilter>();
                MeshRenderer arrowRenderer = arrowVisual.GetComponent<MeshRenderer>();
                Assert.AreEqual("EnhancedGeneratedMesh_ArcherArrow", arrowFilter.sharedMesh.name);
                Assert.Greater(
                    arrowFilter.sharedMesh.bounds.size.z,
                    arrowFilter.sharedMesh.bounds.size.x * 4f);
                Assert.Greater(
                    arrowFilter.sharedMesh.bounds.size.z,
                    arrowFilter.sharedMesh.bounds.size.y * 4f);
                Assert.Less(arrowFilter.sharedMesh.bounds.min.z, -0.7f);
                Assert.Greater(arrowFilter.sharedMesh.bounds.max.z, 0.9f);
                Assert.AreEqual(
                    "MagicCarpet/Enhanced/VertexColorLit",
                    arrowRenderer.sharedMaterial.shader.name);
                Assert.IsFalse(arrowRenderer.sharedMaterial.HasProperty("_MainTex"));
                Assert.That(
                    Quaternion.Angle(arrowRotation, arrowView.rotation),
                    Is.LessThan(0.001f));
                Assert.That(
                    Quaternion.Angle(arrowRotation, arrowVisual.rotation),
                    Is.LessThan(0.001f));
                Assert.AreEqual(0, root.NightLighting.ActiveLocalLightCount);
                Assert.IsFalse(arrowView.Find("EnhancedFireballHalo").gameObject.activeSelf);
                Assert.IsFalse(arrowView.Find("EnhancedFireballTrail").gameObject.activeSelf);
                Assert.IsFalse(arrowView.Find("EnhancedFireballPlume").gameObject.activeSelf);
                Assert.IsFalse(arrowView.Find("EnhancedFireballVolume").gameObject.activeSelf);
                Assert.AreEqual(0, root.PooledEntityCount);
            }
            finally
            {
                Object.DestroyImmediate(gameObject);
            }
        }

        [Test]
        public void AuthoritativePossessionProjectileUsesRadialOrbAndMatchingOwnerLight()
        {
            Assert.AreEqual(18, (int)UnityEnhancedPresentationFamily.Possession);

            string ownerSource = File.ReadAllText(Path.Combine(
                Application.dataPath,
                "Scripts",
                "UnityEnhancedOwnerSessionSource.cs"));
            string actorSource = File.ReadAllText(Path.Combine(
                Application.dataPath,
                "Scripts",
                "UnityEnhancedActorPresentation.cs"));
            int buildEntitiesStart = ownerSource.IndexOf(
                "private UnityEnhancedEntitySnapshot[] BuildEntities()",
                StringComparison.Ordinal);
            int buildLightsStart = ownerSource.IndexOf(
                "private UnityEnhancedLocalLightRequest[] BuildLocalLights()",
                StringComparison.Ordinal);
            int presentationFamilyStart = ownerSource.IndexOf(
                "private static int PresentationFamily(",
                StringComparison.Ordinal);
            Assert.GreaterOrEqual(buildEntitiesStart, 0);
            Assert.Greater(buildLightsStart, buildEntitiesStart);
            Assert.Greater(presentationFamilyStart, buildLightsStart);

            string buildEntities = ownerSource.Substring(
                buildEntitiesStart,
                buildLightsStart - buildEntitiesStart);
            string buildLights = ownerSource.Substring(
                buildLightsStart,
                presentationFamilyStart - buildLightsStart);
            StringAssert.Contains(
                "int family = IsLuminousFireballProjectileKind(source.projectileKind)",
                buildEntities);
            StringAssert.Contains(
                "source.projectileKind == 5",
                buildEntities);
            StringAssert.Contains(
                "UnityEnhancedPresentationFamily.Possession",
                buildEntities);
            StringAssert.Contains(
                "DynamicStableIdentity(0x20000000U, source.projectileId)",
                buildEntities);
            StringAssert.Contains(
                "private static bool IsLuminousFireballProjectileKind(uint projectileKind)",
                ownerSource);
            StringAssert.Contains(
                "return projectileKind == 1 || projectileKind == 3;",
                ownerSource);
            StringAssert.Contains(
                "if (source.projectileKind == 2)\n                {\n                    continue;\n                }",
                buildLights);
            StringAssert.Contains(
                "bool fireball = IsLuminousFireballProjectileKind(source.projectileKind);",
                buildLights);
            StringAssert.Contains(
                "bool possession = source.projectileKind == 5;",
                buildLights);
            StringAssert.Contains(
                "new Color(0.92f, 0.12f, 0.82f, 1f)",
                buildLights);
            StringAssert.Contains(
                ": possession ? 2.2f : 1.8f,",
                buildLights);

            StringAssert.Contains(
                "bool possessionFamily = familyId == UnityEnhancedPresentationFamily.Possession;",
                actorSource);
            StringAssert.Contains(
                "fireFamily || fireflyFamily || possessionFamily",
                actorSource);
            StringAssert.Contains(
                "new Color(0.92f, 0.12f, 0.82f, 1f)",
                actorSource);
            StringAssert.Contains(
                "family == UnityEnhancedPresentationFamily.Possession;",
                actorSource);
            StringAssert.DoesNotContain(
                "case UnityEnhancedPresentationFamily.Possession:\n                    return CreateCreatureMesh",
                actorSource);

            GameObject gameObject = new GameObject("enhanced-possession-orb-test");
            try
            {
                UnityEnhancedWorldRoot root = gameObject.AddComponent<UnityEnhancedWorldRoot>();
                root.OpenDiagnostic(PresentationModeId.UnityEnhanced);
                const ulong generation = 50;
                const ulong fireballIdentity = 0x0000003220000001UL;
                const ulong possessionIdentity = 0x0000003220000005UL;
                Vector3 position = new Vector3(3f, 4f, 5f);
                Quaternion possessionRotation = Quaternion.Euler(-16f, 47f, 0f);
                UnityEnhancedNightLightingDescriptor night =
                    UnityEnhancedNightLightingDescriptor.Level1Night();

                Assert.IsTrue(root.ApplySnapshot(new UnityEnhancedWorldSnapshot
                {
                    Generation = generation,
                    Tick = 0,
                    Reset = true,
                    CameraPose = new UnityEnhancedCameraPose(Vector3.zero, Quaternion.identity, 70f),
                    Entities = new[]
                    {
                        new UnityEnhancedEntitySnapshot
                        {
                            StableIdentity = fireballIdentity,
                            Generation = generation,
                            Position = position,
                            Rotation = Quaternion.identity,
                            Visible = true,
                            PresentationFamily = (int)UnityEnhancedPresentationFamily.Fire,
                            Scale = 1f,
                            RenderLayer = 2001
                        }
                    },
                    NightLighting = night,
                    LocalLights = new[]
                    {
                        new UnityEnhancedLocalLightRequest(
                            fireballIdentity,
                            position,
                            new Color(1f, 0.2f, 0.04f, 1f),
                            5.5f,
                            11f,
                            2001)
                    },
                    VfxEvents = Array.Empty<UnityEnhancedVfxEvent>()
                }));
                Transform fireballView = gameObject.transform.Find(
                    "EnhancedWorldObjects/EnhancedEntity_0000003220000001");
                Assert.IsNotNull(fireballView);
                Assert.IsNotNull(fireballView.Find("EnhancedFireballHalo"));
                Assert.IsNotNull(fireballView.Find("EnhancedFireballTrail"));

                Assert.IsTrue(root.ApplySnapshot(new UnityEnhancedWorldSnapshot
                {
                    Generation = generation,
                    Tick = 1,
                    CameraPose = new UnityEnhancedCameraPose(Vector3.zero, Quaternion.identity, 70f),
                    Entities = Array.Empty<UnityEnhancedEntitySnapshot>(),
                    NightLighting = night,
                    LocalLights = Array.Empty<UnityEnhancedLocalLightRequest>(),
                    VfxEvents = Array.Empty<UnityEnhancedVfxEvent>()
                }));
                Assert.AreEqual(1, root.PooledEntityCount);
                Assert.AreEqual(1, root.ActorPresentation.PooledVisualCount);
                Assert.AreEqual(0, root.NightLighting.ActiveLocalLightCount);

                Assert.IsTrue(root.ApplySnapshot(new UnityEnhancedWorldSnapshot
                {
                    Generation = generation,
                    Tick = 2,
                    CameraPose = new UnityEnhancedCameraPose(Vector3.zero, Quaternion.identity, 70f),
                    Entities = new[]
                    {
                        new UnityEnhancedEntitySnapshot
                        {
                            StableIdentity = possessionIdentity,
                            Generation = generation,
                            Position = position,
                            Rotation = possessionRotation,
                            Visible = true,
                            PresentationFamily = (int)UnityEnhancedPresentationFamily.Possession,
                            Scale = 1f,
                            RenderLayer = 2005
                        }
                    },
                    NightLighting = night,
                    LocalLights = new[]
                    {
                        new UnityEnhancedLocalLightRequest(
                            possessionIdentity,
                            position,
                            new Color(0.92f, 0.12f, 0.82f, 1f),
                            2.2f,
                            6f,
                            2005)
                    },
                    VfxEvents = Array.Empty<UnityEnhancedVfxEvent>()
                }));

                Transform possessionView = gameObject.transform.Find(
                    "EnhancedWorldObjects/EnhancedEntity_0000003220000005");
                Assert.IsNotNull(possessionView);
                Assert.AreEqual(0, root.PooledEntityCount);
                Assert.That(
                    Quaternion.Angle(possessionRotation, possessionView.rotation),
                    Is.LessThan(0.001f));

                Transform possessionVisual = possessionView.Find("EnhancedActorVisual");
                Assert.IsNotNull(possessionVisual);
                MeshFilter possessionFilter = possessionVisual.GetComponent<MeshFilter>();
                MeshRenderer possessionRenderer = possessionVisual.GetComponent<MeshRenderer>();
                Assert.AreEqual("EnhancedGeneratedMesh_Possession", possessionFilter.sharedMesh.name);
                Assert.AreEqual(8, possessionFilter.sharedMesh.vertexCount);
                Assert.AreEqual(8, possessionFilter.sharedMesh.uv.Length);
                Assert.That(
                    possessionFilter.sharedMesh.bounds.size.x,
                    Is.EqualTo(0.7f).Within(0.001f));
                Assert.That(
                    possessionFilter.sharedMesh.bounds.size.y,
                    Is.EqualTo(0.7f).Within(0.001f));
                Assert.That(
                    possessionFilter.sharedMesh.bounds.size.z,
                    Is.EqualTo(0.7f).Within(0.001f));
                Assert.AreEqual(
                    "MagicCarpet/Enhanced/Vfx",
                    possessionRenderer.sharedMaterial.shader.name);
                Color glowColor = possessionRenderer.sharedMaterial.GetColor("_GlowColor");
                Assert.That(glowColor.r, Is.EqualTo(0.92f).Within(0.001f));
                Assert.That(glowColor.g, Is.EqualTo(0.12f).Within(0.001f));
                Assert.That(glowColor.b, Is.EqualTo(0.82f).Within(0.001f));
                Assert.Greater(glowColor.b, 0.6f);
                Assert.That(Mathf.Abs(glowColor.r - glowColor.b), Is.LessThan(0.2f));
                Assert.That(
                    possessionRenderer.sharedMaterial.GetFloat("_GlowIntensity"),
                    Is.EqualTo(1.6f).Within(0.001f));
                Assert.That(
                    possessionRenderer.sharedMaterial.GetFloat("_Opacity"),
                    Is.EqualTo(0.82f).Within(0.001f));
                Assert.That(
                    possessionRenderer.sharedMaterial.GetFloat("_PulseSpeed"),
                    Is.EqualTo(2.4f).Within(0.001f));
                Assert.That(
                    possessionRenderer.sharedMaterial.GetFloat("_RadialMask"),
                    Is.EqualTo(1f).Within(0.001f));
                Assert.IsFalse(possessionView.Find("EnhancedFireballHalo").gameObject.activeSelf);
                Assert.IsFalse(possessionView.Find("EnhancedFireballTrail").gameObject.activeSelf);
                Assert.IsFalse(possessionView.Find("EnhancedFireballPlume").gameObject.activeSelf);
                Assert.IsFalse(possessionView.Find("EnhancedFireballVolume").gameObject.activeSelf);
                Assert.AreEqual(1, root.NightLighting.ActiveLocalLightCount);
                Light possessionLight = gameObject.transform
                    .Find("EnhancedNightLights/EnhancedLocalLight_0")
                    .GetComponent<Light>();
                Assert.That(possessionLight.color.r, Is.EqualTo(0.92f).Within(0.001f));
                Assert.That(possessionLight.color.g, Is.EqualTo(0.12f).Within(0.001f));
                Assert.That(possessionLight.color.b, Is.EqualTo(0.82f).Within(0.001f));
                Assert.Greater(possessionLight.color.b, 0.6f);
                Assert.That(
                    Mathf.Abs(possessionLight.color.r - possessionLight.color.b),
                    Is.LessThan(0.2f));
                Assert.That(possessionLight.intensity, Is.EqualTo(2.2f).Within(0.001f));
                Assert.That(possessionLight.range, Is.EqualTo(6f).Within(0.001f));
            }
            finally
            {
                Object.DestroyImmediate(gameObject);
            }
        }

        [Test]
        public void AuthoritativeFireflyFireballUsesExistingLuminousProjectileStack()
        {
            string ownerSource = File.ReadAllText(Path.Combine(
                Application.dataPath,
                "Scripts",
                "UnityEnhancedOwnerSessionSource.cs"));
            int buildEntitiesStart = ownerSource.IndexOf(
                "private UnityEnhancedEntitySnapshot[] BuildEntities()",
                StringComparison.Ordinal);
            int buildLightsStart = ownerSource.IndexOf(
                "private UnityEnhancedLocalLightRequest[] BuildLocalLights()",
                StringComparison.Ordinal);
            int presentationFamilyStart = ownerSource.IndexOf(
                "private static int PresentationFamily(",
                StringComparison.Ordinal);
            Assert.GreaterOrEqual(buildEntitiesStart, 0);
            Assert.Greater(buildLightsStart, buildEntitiesStart);
            Assert.Greater(presentationFamilyStart, buildLightsStart);

            string buildEntities = ownerSource.Substring(
                buildEntitiesStart,
                buildLightsStart - buildEntitiesStart);
            string buildLights = ownerSource.Substring(
                buildLightsStart,
                presentationFamilyStart - buildLightsStart);
            StringAssert.Contains(
                "private static bool IsLuminousFireballProjectileKind(uint projectileKind)",
                ownerSource);
            StringAssert.Contains(
                "return projectileKind == 1 || projectileKind == 3;",
                ownerSource);
            StringAssert.Contains(
                "int family = IsLuminousFireballProjectileKind(source.projectileKind)",
                buildEntities);
            StringAssert.Contains("source.projectileKind == 2", buildEntities);
            StringAssert.Contains("source.projectileKind == 5", buildEntities);
            StringAssert.Contains(
                "UnityEnhancedPresentationFamily.Possession",
                buildEntities);
            StringAssert.Contains(
                "bool fireball = IsLuminousFireballProjectileKind(source.projectileKind);",
                buildLights);
            StringAssert.Contains(
                "if (source.projectileKind == 2)\n                {\n                    continue;\n                }",
                buildLights);

            GameObject gameObject = new GameObject("enhanced-firefly-fireball-test");
            try
            {
                UnityEnhancedWorldRoot root = gameObject.AddComponent<UnityEnhancedWorldRoot>();
                root.OpenDiagnostic(PresentationModeId.UnityEnhanced);
                const ulong generation = 44;
                const ulong fireflyIdentity = 0x0000002c20000003UL;
                const ulong arrowIdentity = 0x0000002c20000002UL;
                Vector3 fireflyPosition = new Vector3(2f, 3f, 4f);
                Quaternion fireflyRotation = Quaternion.Euler(-9f, 37f, 0f);
                UnityEnhancedNightLightingDescriptor night =
                    UnityEnhancedNightLightingDescriptor.Level1Night();
                float expectedIntensity =
                    5.5f * UnityEnhancedOwnerSessionSource.FireballLightIntensityPulse(0, 3);
                float expectedRange =
                    11f * UnityEnhancedOwnerSessionSource.FireballLightRangePulse(0, 3);

                Assert.IsTrue(root.ApplySnapshot(new UnityEnhancedWorldSnapshot
                {
                    Generation = generation,
                    Tick = 0,
                    Reset = true,
                    CameraPose = new UnityEnhancedCameraPose(Vector3.zero, Quaternion.identity, 70f),
                    Entities = new[]
                    {
                        new UnityEnhancedEntitySnapshot
                        {
                            StableIdentity = fireflyIdentity,
                            Generation = generation,
                            Position = fireflyPosition,
                            Rotation = fireflyRotation,
                            Visible = true,
                            PresentationFamily = (int)UnityEnhancedPresentationFamily.Fire,
                            Scale = 1f,
                            RenderLayer = 2003
                        }
                    },
                    NightLighting = night,
                    LocalLights = new[]
                    {
                        new UnityEnhancedLocalLightRequest(
                            fireflyIdentity,
                            fireflyPosition,
                            new Color(1f, 0.2f, 0.04f, 1f),
                            expectedIntensity,
                            expectedRange,
                            2003)
                    },
                    VfxEvents = Array.Empty<UnityEnhancedVfxEvent>()
                }));

                Transform fireflyView = gameObject.transform.Find(
                    "EnhancedWorldObjects/EnhancedEntity_0000002c20000003");
                Assert.IsNotNull(fireflyView);
                Assert.That(
                    Quaternion.Angle(fireflyRotation, fireflyView.rotation),
                    Is.LessThan(0.001f));
                Assert.AreEqual(
                    "MagicCarpet/Enhanced/Fireball",
                    fireflyView.Find("EnhancedActorVisual").GetComponent<MeshRenderer>()
                        .sharedMaterial.shader.name);
                Assert.AreEqual(
                    "MagicCarpet/Enhanced/Fireball",
                    fireflyView.Find("EnhancedFireballHalo").GetComponent<MeshRenderer>()
                        .sharedMaterial.shader.name);
                Assert.AreEqual(
                    "MagicCarpet/Enhanced/FireballTrail",
                    fireflyView.Find("EnhancedFireballTrail").GetComponent<TrailRenderer>()
                        .sharedMaterial.shader.name);
                Assert.AreEqual(
                    "MagicCarpet/Enhanced/FireballTrail",
                    fireflyView.Find("EnhancedFireballPlume").GetComponent<MeshRenderer>()
                        .sharedMaterial.shader.name);
                Assert.AreEqual(
                    "MagicCarpet/Enhanced/FireballVolume",
                    fireflyView.Find("EnhancedFireballVolume").GetComponent<MeshRenderer>()
                        .sharedMaterial.shader.name);
                Assert.IsTrue(fireflyView.Find("EnhancedFireballHalo").gameObject.activeSelf);
                Assert.IsTrue(fireflyView.Find("EnhancedFireballTrail").gameObject.activeSelf);
                Assert.IsTrue(fireflyView.Find("EnhancedFireballPlume").gameObject.activeSelf);
                Assert.IsTrue(fireflyView.Find("EnhancedFireballVolume").gameObject.activeSelf);
                Assert.AreEqual(1, root.NightLighting.ActiveLocalLightCount);
                Light fireflyLight = gameObject.transform
                    .Find("EnhancedNightLights/EnhancedLocalLight_0")
                    .GetComponent<Light>();
                Assert.That(fireflyLight.intensity, Is.EqualTo(expectedIntensity).Within(0.001f));
                Assert.That(fireflyLight.range, Is.EqualTo(expectedRange).Within(0.001f));

                Assert.IsTrue(root.ApplySnapshot(new UnityEnhancedWorldSnapshot
                {
                    Generation = generation,
                    Tick = 13,
                    CameraPose = new UnityEnhancedCameraPose(Vector3.zero, Quaternion.identity, 70f),
                    Entities = Array.Empty<UnityEnhancedEntitySnapshot>(),
                    NightLighting = night,
                    LocalLights = Array.Empty<UnityEnhancedLocalLightRequest>(),
                    VfxEvents = Array.Empty<UnityEnhancedVfxEvent>()
                }));
                Assert.AreEqual(1, root.PooledEntityCount);
                Assert.AreEqual(1, root.ActorPresentation.PooledVisualCount);
                Assert.AreEqual(0, root.NightLighting.ActiveLocalLightCount);

                Assert.IsTrue(root.ApplySnapshot(new UnityEnhancedWorldSnapshot
                {
                    Generation = generation,
                    Tick = 14,
                    CameraPose = new UnityEnhancedCameraPose(Vector3.zero, Quaternion.identity, 70f),
                    Entities = new[]
                    {
                        new UnityEnhancedEntitySnapshot
                        {
                            StableIdentity = arrowIdentity,
                            Generation = generation,
                            Position = fireflyPosition,
                            Rotation = Quaternion.Euler(11f, 23f, 0f),
                            Visible = true,
                            PresentationFamily = (int)UnityEnhancedPresentationFamily.ArcherArrow,
                            Scale = 1f,
                            RenderLayer = 2002
                        }
                    },
                    NightLighting = night,
                    LocalLights = Array.Empty<UnityEnhancedLocalLightRequest>(),
                    VfxEvents = Array.Empty<UnityEnhancedVfxEvent>()
                }));
                Assert.AreEqual(0, root.PooledEntityCount);
                Assert.AreEqual(0, root.NightLighting.ActiveLocalLightCount);
                Transform reusedView = gameObject.transform.Find(
                    "EnhancedWorldObjects/EnhancedEntity_0000002c20000002");
                Assert.IsNotNull(reusedView);
                Assert.IsFalse(reusedView.Find("EnhancedFireballHalo").gameObject.activeSelf);
                Assert.IsFalse(reusedView.Find("EnhancedFireballTrail").gameObject.activeSelf);
                Assert.IsFalse(reusedView.Find("EnhancedFireballPlume").gameObject.activeSelf);
                Assert.IsFalse(reusedView.Find("EnhancedFireballVolume").gameObject.activeSelf);
            }
            finally
            {
                Object.DestroyImmediate(gameObject);
            }
        }

        [Test]
        public void AuthoritativeFireballNightLightFollowsInterpolatedPoseWithoutDuplicateActorLight()
        {
            GameObject gameObject = new GameObject("enhanced-fireball-interpolated-light-test");
            UnityEnhancedWorldRoot root = gameObject.AddComponent<UnityEnhancedWorldRoot>();
            root.OpenDiagnostic(PresentationModeId.UnityEnhanced);
            const ulong generation = 21;
            const ulong fireballIdentity = 0x0000000020000001UL;
            Vector3 initialPosition = new Vector3(1f, 2f, 3f);
            Vector3 targetPosition = new Vector3(5f, 4f, 7f);
            UnityEnhancedQualitySettings quality =
                UnityEnhancedQualitySettings.ForTier(UnityEnhancedQualityTier.WholeHog);
            UnityEnhancedNightLightingDescriptor night =
                UnityEnhancedNightLightingDescriptor.Level1Night();
            UnityEnhancedLocalLightRequest[] initialLights =
            {
                new UnityEnhancedLocalLightRequest(
                    fireballIdentity,
                    initialPosition,
                    new Color(1f, 0.2f, 0.04f, 1f),
                    5.5f,
                    11f,
                    2001),
                new UnityEnhancedLocalLightRequest(
                    0x9000000000000001UL,
                    Vector3.one,
                    Color.red,
                    1f,
                    4f,
                    1000),
                new UnityEnhancedLocalLightRequest(
                    0x9000000000000002UL,
                    Vector3.up,
                    Color.yellow,
                    1f,
                    4f,
                    999),
                new UnityEnhancedLocalLightRequest(
                    0x9000000000000003UL,
                    Vector3.forward,
                    Color.red,
                    1f,
                    4f,
                    998),
                new UnityEnhancedLocalLightRequest(
                    0x9000000000000004UL,
                    Vector3.right,
                    Color.yellow,
                    1f,
                    4f,
                    997),
                new UnityEnhancedLocalLightRequest(
                    0x9000000000000005UL,
                    Vector3.zero,
                    Color.red,
                    1f,
                    4f,
                    996)
            };
            Assert.IsTrue(root.ApplySnapshot(new UnityEnhancedWorldSnapshot
            {
                Generation = generation,
                Tick = 0,
                CameraPose = new UnityEnhancedCameraPose(Vector3.zero, Quaternion.identity, 70f),
                Quality = quality,
                Entities = new[]
                {
                    FireballEntity(generation, fireballIdentity, initialPosition)
                },
                NightLighting = night,
                LocalLights = initialLights
            }));

            Assert.AreEqual(6, root.NightLighting.ActiveLocalLightCount);
            Assert.AreEqual(4, root.NightLighting.ShaderVisibleWarmLightCount);
            Assert.IsNull(gameObject.transform.Find(
                "EnhancedWorldObjects/EnhancedEntity_0000000020000001/EnhancedFireballLight"));
            Transform lightTransform = gameObject.transform.Find(
                "EnhancedNightLights/EnhancedLocalLight_0");
            Assert.IsNotNull(lightTransform);
            Light fireballLight = lightTransform.GetComponent<Light>();
            Assert.IsNotNull(fireballLight);
            Assert.IsTrue(fireballLight.enabled);
            Assert.AreEqual(LightType.Point, fireballLight.type);
            Assert.That(fireballLight.color.r, Is.EqualTo(1f).Within(0.001f));
            Assert.That(fireballLight.color.g, Is.EqualTo(0.2f).Within(0.001f));
            Assert.That(fireballLight.color.b, Is.EqualTo(0.04f).Within(0.001f));
            Assert.That(fireballLight.range, Is.EqualTo(11f).Within(0.001f));
            Assert.That(fireballLight.intensity, Is.EqualTo(5.5f).Within(0.001f));
            Assert.That(fireballLight.transform.position.x, Is.EqualTo(initialPosition.x).Within(0.001f));
            Assert.That(fireballLight.transform.position.y, Is.EqualTo(initialPosition.y).Within(0.001f));
            Assert.That(fireballLight.transform.position.z, Is.EqualTo(initialPosition.z).Within(0.001f));
            Vector4 initialShaderPosition = Shader.GetGlobalVectorArray(
                "_EnhancedWarmLightPositions")[0];
            Assert.That(initialShaderPosition.x, Is.EqualTo(initialPosition.x).Within(0.001f));
            Assert.That(initialShaderPosition.y, Is.EqualTo(initialPosition.y).Within(0.001f));
            Assert.That(initialShaderPosition.z, Is.EqualTo(initialPosition.z).Within(0.001f));

            UnityEnhancedLocalLightRequest[] targetLights =
                (UnityEnhancedLocalLightRequest[])initialLights.Clone();
            targetLights[0].Position = targetPosition;
            Assert.IsTrue(root.ApplySnapshot(new UnityEnhancedWorldSnapshot
            {
                Generation = generation,
                Tick = 1,
                CameraPose = new UnityEnhancedCameraPose(Vector3.zero, Quaternion.identity, 70f),
                Quality = quality,
                Entities = new[]
                {
                    FireballEntity(generation, fireballIdentity, targetPosition)
                },
                NightLighting = night,
                LocalLights = targetLights
            }));
            root.InterpolatePresentation(0.5f);
            Vector3 midpoint = Vector3.Lerp(initialPosition, targetPosition, 0.5f);
            Assert.That(fireballLight.transform.position.x, Is.EqualTo(midpoint.x).Within(0.001f));
            Assert.That(fireballLight.transform.position.y, Is.EqualTo(midpoint.y).Within(0.001f));
            Assert.That(fireballLight.transform.position.z, Is.EqualTo(midpoint.z).Within(0.001f));
            Vector4 midpointShaderPosition = Shader.GetGlobalVectorArray(
                "_EnhancedWarmLightPositions")[0];
            Assert.That(midpointShaderPosition.x, Is.EqualTo(midpoint.x).Within(0.001f));
            Assert.That(midpointShaderPosition.y, Is.EqualTo(midpoint.y).Within(0.001f));
            Assert.That(midpointShaderPosition.z, Is.EqualTo(midpoint.z).Within(0.001f));

            Assert.IsTrue(root.ApplySnapshot(new UnityEnhancedWorldSnapshot
            {
                Generation = generation,
                Tick = 2,
                CameraPose = new UnityEnhancedCameraPose(Vector3.zero, Quaternion.identity, 70f),
                Quality = quality,
                Entities = Array.Empty<UnityEnhancedEntitySnapshot>(),
                NightLighting = night,
                LocalLights = Array.Empty<UnityEnhancedLocalLightRequest>()
            }));
            Assert.AreEqual(0, root.NightLighting.ActiveLocalLightCount);
            Assert.AreEqual(0, root.NightLighting.ShaderVisibleWarmLightCount);
            Assert.IsFalse(fireballLight.enabled);

            Assert.IsTrue(root.ApplySnapshot(new UnityEnhancedWorldSnapshot
            {
                Generation = generation + 1,
                Tick = 0,
                Reset = true,
                CameraPose = new UnityEnhancedCameraPose(Vector3.zero, Quaternion.identity, 70f),
                Quality = quality,
                Entities = Array.Empty<UnityEnhancedEntitySnapshot>(),
                NightLighting = night,
                LocalLights = Array.Empty<UnityEnhancedLocalLightRequest>()
            }));
            Assert.AreEqual(0, root.NightLighting.ActiveLocalLightCount);
            Assert.AreEqual(0, root.NightLighting.ShaderVisibleWarmLightCount);
            Assert.IsFalse(fireballLight.enabled);

            root.Close();
            Assert.AreEqual(0, root.NightLighting.ActiveLocalLightCount);
            Object.DestroyImmediate(gameObject);
        }

        [Test]
        public void OwnerActualSpeedDrivesBoundedSpeedStreaksAndReducedMotionDisablesThem()
        {
            GameObject gameObject = new GameObject("enhanced-speed-readability-test");
            UnityEnhancedWorldRoot root = gameObject.AddComponent<UnityEnhancedWorldRoot>();
            root.OpenDiagnostic(PresentationModeId.UnityEnhanced);
            UnityEnhancedQualitySettings quality =
                UnityEnhancedQualitySettings.ForTier(UnityEnhancedQualityTier.WholeHog);

            Assert.IsTrue(root.ApplySnapshot(
                SpeedSnapshot(13, 1, 0, 0, quality)));
            Assert.AreEqual(0, root.VfxPresentation.ActiveSpeedStreakCount);

            Assert.IsTrue(root.ApplySnapshot(
                SpeedSnapshot(13, 2, 48, 0, quality)));
            Assert.AreEqual(0, root.VfxPresentation.ActiveSpeedStreakCount);

            Assert.IsTrue(root.ApplySnapshot(
                SpeedSnapshot(13, 3, 80, 0, quality)));
            Assert.AreEqual(8, root.VfxPresentation.ActiveSpeedStreakCount);
            Assert.AreEqual(8, root.VfxPresentation.SpeedStreakPoolCount);
            Transform streakRoot = root.CameraRig.PresentationCamera.transform.Find(
                "EnhancedSpeedStreaks");
            Assert.IsNotNull(streakRoot);
            Vector3 firstPosition = streakRoot.GetChild(0).localPosition;
            MeshRenderer streakRenderer = streakRoot.GetChild(0).GetComponent<MeshRenderer>();
            Assert.IsNotNull(streakRenderer);
            Assert.AreEqual("MagicCarpet/Enhanced/SpeedStreak",
                streakRenderer.sharedMaterial.shader.name);
            Assert.That(streakRenderer.sharedMaterial.GetFloat("_Intensity"),
                Is.EqualTo(1.4f).Within(0.001f));
            Assert.That(streakRenderer.sharedMaterial.GetFloat("_Opacity"),
                Is.EqualTo(0.40f).Within(0.001f));
            Assert.That(streakRenderer.sharedMaterial.GetFloat("_CoreWidth"),
                Is.EqualTo(0.18f).Within(0.001f));
            Assert.That(streakRenderer.sharedMaterial.GetFloat("_EdgeSoftness"),
                Is.EqualTo(0.34f).Within(0.001f));
            Mesh streakMesh = streakRoot.GetChild(0).GetComponent<MeshFilter>().sharedMesh;
            Assert.AreEqual(8, streakMesh.vertexCount);
            Assert.AreEqual(8, streakMesh.uv.Length);
            Assert.Less(Mathf.Abs(streakMesh.vertices[0].x),
                Mathf.Abs(streakMesh.vertices[2].x));
            Assert.Less(Mathf.Abs(streakMesh.vertices[6].x),
                Mathf.Abs(streakMesh.vertices[4].x));
            Assert.Greater(streakRoot.GetChild(0).localScale.y, 0.25f);

            Assert.IsTrue(root.ApplySnapshot(
                SpeedSnapshot(13, 3, 80, 0, quality)));
            Assert.That(
                Vector3.Distance(firstPosition, streakRoot.GetChild(0).localPosition),
                Is.LessThan(0.0001f));

            Assert.IsTrue(root.ApplySnapshot(
                SpeedSnapshot(13, 4, 80, 4, quality)));
            Assert.AreEqual(12, root.VfxPresentation.ActiveSpeedStreakCount);
            Assert.AreEqual(12, root.VfxPresentation.SpeedStreakPoolCount);

            Assert.IsTrue(root.ApplySnapshot(
                SpeedSnapshot(13, 5, 80, byte.MaxValue, quality)));
            Assert.AreEqual(12, root.VfxPresentation.ActiveSpeedStreakCount);

            quality.SpeedStreaks = false;
            Assert.IsTrue(root.ApplySnapshot(
                SpeedSnapshot(13, 6, 80, 4, quality)));
            Assert.AreEqual(0, root.VfxPresentation.ActiveSpeedStreakCount);

            quality.ReducedMotion = true;
            quality.CameraShake = false;
            quality.SpeedStreaks = false;
            quality.MotionBlur = false;
            Assert.IsTrue(root.ApplySnapshot(
                SpeedSnapshot(13, 7, 80, 4, quality)));
            Assert.AreEqual(0, root.VfxPresentation.ActiveSpeedStreakCount);

            root.ResetPresentation(14);
            Assert.AreEqual(0, root.VfxPresentation.ActiveSpeedStreakCount);

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
            Transform volume = gameObject.transform.Find(
                "EnhancedWorldObjects/EnhancedEntity_0000000920000001/EnhancedFireballVolume");
            Assert.IsNotNull(volume);
            Assert.IsFalse(volume.gameObject.activeSelf);
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
        public void TransientWarmLightMergeDeduplicatesStaticRequestsAndCapsShaderField()
        {
            GameObject gameObject = new GameObject("enhanced-transient-warm-light-test");
            UnityEnhancedNightLighting lighting = gameObject.AddComponent<UnityEnhancedNightLighting>();
            UnityEnhancedLocalLightRequest snapshotRequest = new UnityEnhancedLocalLightRequest(
                0x4000002aUL,
                new Vector3(1f, 2f, 3f),
                new Color(1f, 0.18f, 0.03f, 1f),
                2.6f,
                8f,
                3042);

            lighting.ApplyDescriptor(
                11,
                UnityEnhancedNightLightingDescriptor.Level1Night(),
                new[] { snapshotRequest },
                true);
            int staticLightCount = lighting.ActiveLocalLightCount;

            var transientRequests = new List<UnityEnhancedLocalLightRequest>
            {
                new UnityEnhancedLocalLightRequest(
                    snapshotRequest.StableIdentity,
                    new Vector3(90f, 90f, 90f),
                    Color.red,
                    9f,
                    2f,
                    9999),
                new UnityEnhancedLocalLightRequest(
                    0x40000001UL,
                    new Vector3(4f, 2f, 3f),
                    Color.red,
                    1f,
                    4f,
                    3001),
                new UnityEnhancedLocalLightRequest(
                    0x40000002UL,
                    new Vector3(5f, 2f, 3f),
                    Color.red,
                    1f,
                    4f,
                    3002),
                new UnityEnhancedLocalLightRequest(
                    0x40000003UL,
                    new Vector3(6f, 2f, 3f),
                    Color.red,
                    1f,
                    4f,
                    3003),
                new UnityEnhancedLocalLightRequest(
                    0x40000004UL,
                    new Vector3(7f, 2f, 3f),
                    Color.red,
                    1f,
                    4f,
                    3004),
                new UnityEnhancedLocalLightRequest(
                    0x40000005UL,
                    new Vector3(8f, 2f, 3f),
                    Color.red,
                    1f,
                    4f,
                    3005)
            };

            lighting.ApplyTransientWarmLights(transientRequests);
            Assert.AreEqual(staticLightCount, lighting.ActiveLocalLightCount);
            Assert.AreEqual(4, lighting.ShaderVisibleWarmLightCount);
            Assert.AreEqual(4, Shader.GetGlobalInt("_EnhancedWarmLightCount"));
            Vector4 publishedPosition = Shader.GetGlobalVectorArray(
                "_EnhancedWarmLightPositions")[0];
            Assert.That(publishedPosition.x, Is.EqualTo(1f).Within(0.001f));
            Assert.That(publishedPosition.y, Is.EqualTo(2f).Within(0.001f));
            Assert.That(publishedPosition.z, Is.EqualTo(3f).Within(0.001f));
            Vector4 publishedColor = Shader.GetGlobalVectorArray(
                "_EnhancedWarmLightColors")[0];
            Assert.That(publishedColor.w, Is.EqualTo(2.6f).Within(0.001f));

            lighting.ResetForGeneration(12);
            Assert.AreEqual(0, lighting.ShaderVisibleWarmLightCount);
            Assert.AreEqual(0, Shader.GetGlobalInt("_EnhancedWarmLightCount"));
            Object.DestroyImmediate(gameObject);
        }

        [Test]
        public void OwnerImpactWarmLightPersistsAfterSnapshotRequestExpires()
        {
            GameObject gameObject = new GameObject("enhanced-owner-impact-warm-light-test");
            UnityEnhancedWorldRoot root = gameObject.AddComponent<UnityEnhancedWorldRoot>();
            root.OpenDiagnostic(PresentationModeId.UnityEnhanced);
            const ulong generation = 15;
            const ulong stableIdentity = 0x4000002aUL;
            UnityEnhancedVfxEvent impact = new UnityEnhancedVfxEvent(
                1,
                generation,
                10,
                stableIdentity,
                UnityEnhancedVfxKind.Impact,
                new Vector3(8f, 7f, 6f),
                1f,
                18);
            UnityEnhancedLocalLightRequest snapshotRequest = new UnityEnhancedLocalLightRequest(
                stableIdentity,
                new Vector3(1f, 2f, 3f),
                new Color(1f, 0.18f, 0.03f, 1f),
                2.6f,
                8f,
                3042);

            Assert.IsTrue(root.ApplySnapshot(new UnityEnhancedWorldSnapshot
            {
                Generation = generation,
                Tick = 10,
                CameraPose = new UnityEnhancedCameraPose(Vector3.zero, Quaternion.identity, 70f),
                NightLighting = UnityEnhancedNightLightingDescriptor.Level1Night(),
                LocalLights = new[] { snapshotRequest },
                VfxEvents = new[] { impact }
            }));
            Assert.AreEqual(1, root.NightLighting.ShaderVisibleWarmLightCount);
            Vector4 snapshotPosition = Shader.GetGlobalVectorArray(
                "_EnhancedWarmLightPositions")[0];
            Assert.That(snapshotPosition.x, Is.EqualTo(1f).Within(0.001f));
            Assert.That(snapshotPosition.y, Is.EqualTo(2f).Within(0.001f));
            Assert.That(snapshotPosition.z, Is.EqualTo(3f).Within(0.001f));

            Assert.IsTrue(root.ApplySnapshot(new UnityEnhancedWorldSnapshot
            {
                Generation = generation,
                Tick = 19,
                CameraPose = new UnityEnhancedCameraPose(Vector3.zero, Quaternion.identity, 70f),
                NightLighting = UnityEnhancedNightLightingDescriptor.Level1Night(),
                LocalLights = Array.Empty<UnityEnhancedLocalLightRequest>(),
                VfxEvents = Array.Empty<UnityEnhancedVfxEvent>()
            }));
            Assert.AreEqual(1, root.NightLighting.ShaderVisibleWarmLightCount);
            Vector4 transientPosition = Shader.GetGlobalVectorArray(
                "_EnhancedWarmLightPositions")[0];
            Assert.That(transientPosition.x, Is.EqualTo(8f).Within(0.001f));
            Assert.That(transientPosition.y, Is.EqualTo(7f).Within(0.001f));
            Assert.That(transientPosition.z, Is.EqualTo(6f).Within(0.001f));
            Vector4 transientColor = Shader.GetGlobalVectorArray(
                "_EnhancedWarmLightColors")[0];
            Assert.That(transientColor.w, Is.GreaterThan(0f));
            Assert.That(transientColor.w, Is.LessThan(2f));

            Assert.IsTrue(root.ApplySnapshot(new UnityEnhancedWorldSnapshot
            {
                Generation = generation,
                Tick = 28,
                CameraPose = new UnityEnhancedCameraPose(Vector3.zero, Quaternion.identity, 70f),
                NightLighting = UnityEnhancedNightLightingDescriptor.Level1Night(),
                LocalLights = Array.Empty<UnityEnhancedLocalLightRequest>(),
                VfxEvents = Array.Empty<UnityEnhancedVfxEvent>()
            }));
            Assert.AreEqual(0, root.NightLighting.ShaderVisibleWarmLightCount);
            Assert.AreEqual(0, root.VfxPresentation.ActiveImpactLightCount);

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
            Assert.AreEqual(FogMode.Linear, RenderSettings.fogMode);
            Assert.That(lighting.AppliedFogStartDistance, Is.EqualTo(96f).Within(0.001f));
            Assert.That(lighting.AppliedFogEndDistance, Is.EqualTo(720f).Within(0.001f));
            Assert.That(RenderSettings.fogStartDistance, Is.EqualTo(96f).Within(0.001f));
            Assert.That(RenderSettings.fogEndDistance, Is.EqualTo(720f).Within(0.001f));
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
            string rippleShaderSource = File.ReadAllText(
                Path.Combine(Application.dataPath, "Resources", "EnhancedWaterRipple.shader"));

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
            StringAssert.Contains("_PlanarReflectionMix", shaderSource);
            StringAssert.Contains("_CausticColor", shaderSource);
            StringAssert.Contains("CausticPattern", shaderSource);
            StringAssert.Contains("saturate(_PlanarReflectionEnabled) *", shaderSource);
            StringAssert.Contains("SAMPLE_TEXTURE2D", shaderSource);
            StringAssert.Contains("MagicCarpet/Enhanced/WaterRipple", rippleShaderSource);
            StringAssert.Contains("_RippleCenter", rippleShaderSource);
            StringAssert.Contains("radialFade", rippleShaderSource);
            StringAssert.Contains("_RippleLifetime", rippleShaderSource);
            StringAssert.Contains("_ShorelineMode", shaderSource);
            StringAssert.Contains("ShorelineFoam", shaderSource);
            StringAssert.Contains("moonSheen", shaderSource);
            StringAssert.DoesNotContain(
                "half3 reflection = lerp(_ReflectionColor.rgb, sceneReflection, _SceneReflectionStrength);",
                shaderSource);
        }

        [Test]
        public void WaterBuildsReflectionSurfaceAndShorelineMesh()
        {
            Assert.IsNotNull(Shader.Find("MagicCarpet/Enhanced/Water"));
            Assert.IsNotNull(Shader.Find("MagicCarpet/Enhanced/WaterRipple"));
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
            Assert.That(water.WaterMaterial.GetFloat("_PlanarReflectionMix"), Is.EqualTo(0.78f).Within(0.001f));
            Assert.That(water.WaterMaterial.GetFloat("_CausticStrength"), Is.EqualTo(1.15f).Within(0.001f));
            MeshRenderer rippleRenderer = gameObject.transform.Find("EnhancedWaterRipple").GetComponent<MeshRenderer>();
            Assert.IsFalse(rippleRenderer.enabled);
            MeshRenderer shorelineRenderer = gameObject.transform.Find("EnhancedShoreline").GetComponent<MeshRenderer>();
            Assert.AreEqual("MagicCarpet/Enhanced/Water", shorelineRenderer.sharedMaterial.shader.name);
            Assert.That(shorelineRenderer.sharedMaterial.GetFloat("_ShorelineMode"), Is.EqualTo(1f).Within(0.001f));
            Assert.Greater(shorelineRenderer.sharedMaterial.GetFloat("_ShorelineFoamStrength"), 0.5f);

            UnityEnhancedVfxEvent splash = new UnityEnhancedVfxEvent(
                1, 2, 4, 0x40000001UL, UnityEnhancedVfxKind.Weather,
                new Vector3(1f, 1.5f, 1f), 1f, 18);
            Assert.IsTrue(water.ApplyRipple(splash));
            Assert.AreEqual(1, water.ActiveRippleCount);
            Assert.Greater(water.WaterMaterial.GetFloat("_RippleStrength"), 0.4f);
            Assert.IsTrue(rippleRenderer.enabled);
            Assert.AreEqual("MagicCarpet/Enhanced/WaterRipple", rippleRenderer.sharedMaterial.shader.name);
            Assert.Greater(rippleRenderer.sharedMaterial.GetFloat("_RippleMaxRadius"), 2f);
            Assert.Greater(rippleRenderer.transform.localScale.x, 2f);
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
        public void OwnerWaterImpactUsesWorldTickAndWarmReflectionRipple()
        {
            string waterSource = File.ReadAllText(
                Path.Combine(Application.dataPath, "Resources", "EnhancedWater.shader"));
            StringAssert.Contains("_RippleColor", waterSource);
            StringAssert.Contains("_RippleColor.rgb * rippleBand", waterSource);
            GameObject gameObject = new GameObject("enhanced-water-impact-tick-test");
            UnityEnhancedWorldRoot root = gameObject.AddComponent<UnityEnhancedWorldRoot>();
            root.OpenDiagnostic(PresentationModeId.UnityEnhanced);

            Assert.IsTrue(root.ApplySnapshot(new UnityEnhancedWorldSnapshot
            {
                Generation = 12,
                Tick = 0,
                CameraPose = new UnityEnhancedCameraPose(Vector3.zero, Quaternion.identity, 70f),
                Water = new UnityEnhancedWaterSnapshot
                {
                    Generation = 12,
                    Tick = 0,
                    Reset = false,
                    Level = 1.5f,
                    Size = new Vector2(8f, 8f),
                    ShorelineWidth = 0,
                    ShorelineHeight = 0,
                    ShorelineMask = Array.Empty<byte>()
                }
            }));

            Assert.IsTrue(root.ApplySnapshot(new UnityEnhancedWorldSnapshot
            {
                Generation = 12,
                Tick = 6,
                CameraPose = new UnityEnhancedCameraPose(Vector3.zero, Quaternion.identity, 70f),
                VfxEvents = new[]
                {
                    new UnityEnhancedVfxEvent(
                        1,
                        12,
                        6,
                        0x4000002aUL,
                        UnityEnhancedVfxKind.Weather,
                        new Vector3(1f, 1.5f, 1f),
                        1f,
                        18)
                }
            }));
            Assert.AreEqual(1, root.WaterPresentation.ActiveRippleCount);
            Assert.Greater(root.WaterPresentation.WaterMaterial.GetFloat("_RippleStrength"), 0.4f);
            Color warmRipple = root.WaterPresentation.WaterMaterial.GetColor("_RippleColor");
            Assert.That(warmRipple.r, Is.EqualTo(1f).Within(0.001f));
            Assert.That(warmRipple.g, Is.EqualTo(0.30f).Within(0.001f));
            Assert.That(warmRipple.b, Is.EqualTo(0.04f).Within(0.001f));
            Vector4 rippleCenter = root.WaterPresentation.WaterMaterial.GetVector("_RippleCenter");
            Assert.That(rippleCenter.x, Is.EqualTo(1f).Within(0.001f));
            Assert.That(rippleCenter.z, Is.EqualTo(1f).Within(0.001f));
            Assert.IsFalse(root.WaterPresentation.ApplyRipple(
                new UnityEnhancedVfxEvent(
                    2,
                    12,
                    7,
                    0x4000002bUL,
                    UnityEnhancedVfxKind.Weather,
                    new Vector3(2f, 1.5f, 2f),
                    1f,
                    18),
                6));
            Assert.AreEqual(1, root.WaterPresentation.ActiveRippleCount);

            root.ResetPresentation(13);
            Assert.AreEqual(0, root.WaterPresentation.ActiveRippleCount);
            Color restoredRipple = root.WaterPresentation.WaterMaterial.GetColor("_RippleColor");
            Assert.That(restoredRipple.r, Is.EqualTo(0.28f).Within(0.001f));
            Assert.That(restoredRipple.g, Is.EqualTo(0.68f).Within(0.001f));
            Assert.That(restoredRipple.b, Is.EqualTo(1f).Within(0.001f));

            Object.DestroyImmediate(gameObject);
        }

        [Test]
        public void AuthoritativeFireflyUsesRadialGlowWithoutChangingExistingVfx()
        {
            string actorSource = File.ReadAllText(Path.Combine(
                Application.dataPath,
                "Scripts",
                "UnityEnhancedActorPresentation.cs"));
            StringAssert.Contains(
                "bool fireflyFamily = familyId == UnityEnhancedPresentationFamily.Firefly;",
                actorSource);
            StringAssert.Contains("fireFamily || fireflyFamily", actorSource);

            string shaderSource = File.ReadAllText(Path.Combine(
                Application.dataPath,
                "Resources",
                "EnhancedVfx.shader"));
            StringAssert.Contains("_RadialMask (\"Radial Mask\", Range(0, 1)) = 0", shaderSource);
            StringAssert.Contains("float2 uv : TEXCOORD0;", shaderSource);
            StringAssert.Contains("output.uv = input.uv;", shaderSource);
            StringAssert.Contains("half radial = 1.0h - smoothstep(", shaderSource);
            StringAssert.Contains(
                "length(input.uv * 2.0h - 1.0h)",
                shaderSource);
            float centerDistance = (new Vector2(0.5f, 0.5f) * 2f - Vector2.one).magnitude;
            float centerEdgeT = Mathf.Clamp01((centerDistance - 0.45f) / (1f - 0.45f));
            float centerRadial = 1f - centerEdgeT * centerEdgeT * (3f - 2f * centerEdgeT);
            float cornerDistance = (new Vector2(1f, 1f) * 2f - Vector2.one).magnitude;
            float cornerEdgeT = Mathf.Clamp01((cornerDistance - 0.45f) / (1f - 0.45f));
            float cornerRadial = 1f - cornerEdgeT * cornerEdgeT * (3f - 2f * cornerEdgeT);
            Assert.That(centerRadial, Is.EqualTo(1f).Within(0.001f));
            Assert.That(cornerRadial, Is.EqualTo(0f).Within(0.001f));
            Assert.Greater(centerRadial, cornerRadial);

            GameObject gameObject = new GameObject("enhanced-firefly-radial-glow-test");
            GameObject vfxGameObject = new GameObject("enhanced-rectangular-vfx-test");
            try
            {
                UnityEnhancedWorldRoot root = gameObject.AddComponent<UnityEnhancedWorldRoot>();
                root.OpenDiagnostic(PresentationModeId.UnityEnhanced);
                const ulong generation = 9;
                const ulong fireflyIdentity = 0x0000000900000001UL;
                Vector3 fireflyPosition = new Vector3(2f, 3f, 4f);
                Assert.IsTrue(root.ApplySnapshot(new UnityEnhancedWorldSnapshot
                {
                    Generation = generation,
                    Tick = 0,
                    Reset = true,
                    CameraPose = new UnityEnhancedCameraPose(Vector3.zero, Quaternion.identity, 70f),
                    Entities = new[]
                    {
                        new UnityEnhancedEntitySnapshot
                        {
                            StableIdentity = fireflyIdentity,
                            Generation = generation,
                            Position = fireflyPosition,
                            Rotation = Quaternion.identity,
                            Visible = true,
                            PresentationFamily = (int)UnityEnhancedPresentationFamily.Firefly,
                            Scale = 1f,
                            RenderLayer = 17
                        }
                    },
                    NightLighting = UnityEnhancedNightLightingDescriptor.Level1Night(),
                    LocalLights = Array.Empty<UnityEnhancedLocalLightRequest>(),
                    VfxEvents = Array.Empty<UnityEnhancedVfxEvent>()
                }));

                Transform fireflyVisual = gameObject.transform.Find(
                    "EnhancedWorldObjects/EnhancedEntity_0000000900000001/EnhancedActorVisual");
                Assert.IsNotNull(fireflyVisual);
                MeshFilter fireflyFilter = fireflyVisual.GetComponent<MeshFilter>();
                MeshRenderer fireflyRenderer = fireflyVisual.GetComponent<MeshRenderer>();
                Assert.AreEqual("MagicCarpet/Enhanced/Vfx", fireflyRenderer.sharedMaterial.shader.name);
                Assert.AreEqual(fireflyFilter.sharedMesh.vertexCount, fireflyFilter.sharedMesh.uv.Length);
                Color glowColor = fireflyRenderer.sharedMaterial.GetColor("_GlowColor");
                Assert.That(glowColor.r, Is.EqualTo(0.72f).Within(0.001f));
                Assert.That(glowColor.g, Is.EqualTo(1f).Within(0.001f));
                Assert.That(glowColor.b, Is.EqualTo(0.3f).Within(0.001f));
                Assert.That(
                    fireflyRenderer.sharedMaterial.GetFloat("_GlowIntensity"),
                    Is.EqualTo(1.4f).Within(0.001f));
                Assert.That(
                    fireflyRenderer.sharedMaterial.GetFloat("_Opacity"),
                    Is.EqualTo(0.7f).Within(0.001f));
                Assert.That(
                    fireflyRenderer.sharedMaterial.GetFloat("_PulseSpeed"),
                    Is.EqualTo(2.2f).Within(0.001f));
                Assert.That(
                    fireflyRenderer.sharedMaterial.GetFloat("_RadialMask"),
                    Is.EqualTo(1f).Within(0.001f));

                UnityEnhancedVfxPresentation vfx =
                    vfxGameObject.AddComponent<UnityEnhancedVfxPresentation>();
                vfx.ResetForGeneration(4);
                UnityEnhancedVfxEvent spellCast = new UnityEnhancedVfxEvent(
                    1,
                    4,
                    10,
                    17,
                    UnityEnhancedVfxKind.SpellCast,
                    Vector3.zero,
                    1f,
                    4);
                Assert.IsTrue(vfx.ApplyEvent(spellCast));
                MeshRenderer vfxRenderer = vfxGameObject.transform
                    .Find("EnhancedVfxPool/EnhancedVfx")
                    .GetComponent<MeshRenderer>();
                Assert.AreEqual("MagicCarpet/Enhanced/Vfx", vfxRenderer.sharedMaterial.shader.name);
                Assert.That(
                    vfxRenderer.sharedMaterial.GetFloat("_RadialMask"),
                    Is.EqualTo(0f).Within(0.001f));
            }
            finally
            {
                Object.DestroyImmediate(vfxGameObject);
                Object.DestroyImmediate(gameObject);
            }
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
        public void FireballVfxEventUsesRadialFireballShaderAndGeneratedCrossBillboard()
        {
            Assert.IsNotNull(Shader.Find("MagicCarpet/Enhanced/Fireball"));
            GameObject gameObject = new GameObject("enhanced-fireball-vfx-test");
            UnityEnhancedVfxPresentation vfx = gameObject.AddComponent<UnityEnhancedVfxPresentation>();
            vfx.ResetForGeneration(4);

            UnityEnhancedVfxEvent fireball = new UnityEnhancedVfxEvent(
                1,
                4,
                20,
                0x20000001UL,
                UnityEnhancedVfxKind.Fire,
                new Vector3(2f, 3f, 4f),
                1f,
                16);
            Assert.IsTrue(vfx.ApplyEvent(fireball));

            Transform visual = gameObject.transform.Find("EnhancedVfxPool/EnhancedVfx");
            Assert.IsNotNull(visual);
            MeshFilter filter = visual.GetComponent<MeshFilter>();
            MeshRenderer renderer = visual.GetComponent<MeshRenderer>();
            Assert.AreEqual("MagicCarpet/Enhanced/Fireball", renderer.sharedMaterial.shader.name);
            Assert.AreEqual(8, filter.sharedMesh.vertexCount);
            Assert.AreEqual(8, filter.sharedMesh.uv.Length);
            Assert.Greater(filter.sharedMesh.bounds.size.x, 1.4f);
            Assert.Greater(renderer.sharedMaterial.GetFloat("_OuterIntensity"), 0f);
            Assert.Greater(renderer.sharedMaterial.GetFloat("_ShimmerStrength"), 0.1f);
            Assert.That(visual.position.x, Is.EqualTo(2f).Within(0.001f));
            Assert.That(visual.position.y, Is.EqualTo(3f).Within(0.001f));
            Assert.That(visual.position.z, Is.EqualTo(4f).Within(0.001f));

            vfx.SetReducedMotion(true);
            Assert.That(visual.localScale.x, Is.EqualTo(0.6f).Within(0.001f));

            Object.DestroyImmediate(gameObject);
        }

        [Test]
        public void ImpactVfxEventUsesBoundedBurstAndOwnerPosition()
        {
            Assert.IsNotNull(Shader.Find("MagicCarpet/Enhanced/Impact"));
            GameObject gameObject = new GameObject("enhanced-impact-vfx-test");
            GameObject cameraObject = new GameObject("enhanced-impact-vfx-camera");
            Camera camera = cameraObject.AddComponent<Camera>();
            cameraObject.transform.rotation = Quaternion.Euler(17f, 23f, 0f);
            UnityEnhancedVfxPresentation vfx = gameObject.AddComponent<UnityEnhancedVfxPresentation>();
            vfx.SetPresentationCamera(camera);
            vfx.ResetForGeneration(7);

            UnityEnhancedVfxEvent impact = new UnityEnhancedVfxEvent(
                1,
                7,
                24,
                0x4000002aUL,
                UnityEnhancedVfxKind.Impact,
                new Vector3(2f, 3f, 4f),
                1f,
                18);
            Assert.IsTrue(vfx.ApplyEvent(impact));

            Transform visual = gameObject.transform.Find("EnhancedVfxPool/EnhancedVfx");
            Assert.IsNotNull(visual);
            MeshFilter filter = visual.GetComponent<MeshFilter>();
            MeshRenderer renderer = visual.GetComponent<MeshRenderer>();
            Assert.AreEqual("MagicCarpet/Enhanced/Impact", renderer.sharedMaterial.shader.name);
            Assert.GreaterOrEqual(filter.sharedMesh.vertexCount, 36);
            Assert.AreEqual(filter.sharedMesh.vertexCount, filter.sharedMesh.uv.Length);
            Assert.Greater(
                Vector2.Distance(filter.sharedMesh.uv[1], new Vector2(0.5f, 0.5f)),
                0.20f);
            Assert.Greater(filter.sharedMesh.bounds.size.y, 0.40f);
            Assert.LessOrEqual(filter.sharedMesh.bounds.size.x, 0.90f);
            Assert.LessOrEqual(filter.sharedMesh.bounds.size.z, 0.90f);
            Assert.That(visual.position.x, Is.EqualTo(2f).Within(0.001f));
            Assert.That(visual.position.y, Is.EqualTo(3f).Within(0.001f));
            Assert.That(visual.position.z, Is.EqualTo(4f).Within(0.001f));
            Assert.Greater(
                Vector3.Dot(visual.forward, -camera.transform.forward),
                0.99f);
            Color glowColor = renderer.sharedMaterial.GetColor("_GlowColor");
            Assert.That(glowColor.r, Is.EqualTo(1f).Within(0.001f));
            Assert.That(glowColor.g, Is.EqualTo(0.24f).Within(0.001f));
            Assert.That(glowColor.b, Is.EqualTo(0.03f).Within(0.001f));
            Assert.That(renderer.sharedMaterial.GetFloat("_GlowIntensity"), Is.EqualTo(0.55f).Within(0.001f));
            Color shockwaveColor = renderer.sharedMaterial.GetColor("_ShockwaveColor");
            Assert.That(shockwaveColor.r, Is.EqualTo(1f).Within(0.001f));
            Assert.That(shockwaveColor.g, Is.EqualTo(0.50f).Within(0.001f));
            Assert.That(shockwaveColor.b, Is.EqualTo(0.06f).Within(0.001f));
            Assert.That(renderer.sharedMaterial.GetFloat("_ShockwaveStrength"), Is.EqualTo(2.6f).Within(0.001f));
            Assert.That(renderer.sharedMaterial.GetFloat("_SparkStrength"), Is.EqualTo(1.25f).Within(0.001f));
            Assert.That(renderer.sharedMaterial.GetFloat("_RimStrength"), Is.EqualTo(0.65f).Within(0.001f));
            Assert.That(renderer.sharedMaterial.GetFloat("_Opacity"), Is.EqualTo(0.45f).Within(0.001f));
            Assert.That(renderer.sharedMaterial.GetFloat("_PulseSpeed"), Is.EqualTo(5.0f).Within(0.001f));
            Light impactLight = visual.GetComponentInChildren<Light>(true);
            Assert.IsNotNull(impactLight);
            Assert.AreEqual(LightType.Point, impactLight.type);
            Assert.AreEqual(LightShadows.None, impactLight.shadows);
            Assert.That(impactLight.color.r, Is.EqualTo(1f).Within(0.001f));
            Assert.That(impactLight.color.g, Is.EqualTo(0.18f).Within(0.001f));
            Assert.That(impactLight.color.b, Is.EqualTo(0.025f).Within(0.001f));
            Assert.That(impactLight.range, Is.EqualTo(6f).Within(0.001f));
            Assert.Greater(impactLight.intensity, 0f);
            Assert.IsTrue(impactLight.enabled);
            Assert.AreEqual(1, vfx.ActiveImpactLightCount);
            var warmRequests = new List<UnityEnhancedLocalLightRequest>();
            Assert.AreEqual(1, vfx.AppendActiveImpactWarmLightRequests(warmRequests));
            Assert.AreEqual(impact.StableIdentity, warmRequests[0].StableIdentity);
            Assert.AreEqual(3042, warmRequests[0].Priority);
            Assert.That(warmRequests[0].Position.x, Is.EqualTo(2f).Within(0.001f));
            Assert.That(warmRequests[0].Intensity, Is.EqualTo(2f).Within(0.001f));
            Assert.IsFalse(vfx.ApplyEvent(impact));
            Assert.AreEqual(1, vfx.DuplicateEffectCount);

            for (int index = 0; index < 4; index++)
            {
                Assert.IsTrue(vfx.ApplyEvent(new UnityEnhancedVfxEvent(
                    (ulong)(index + 2),
                    7,
                    24,
                    (ulong)(0x4000002b + index),
                    UnityEnhancedVfxKind.Impact,
                    new Vector3(2f + index, 3f, 4f),
                    1f,
                    18)));
            }
            Assert.AreEqual(4, vfx.ActiveImpactLightCount);
            Assert.LessOrEqual(gameObject.GetComponentsInChildren<Light>(true).Length, 4);
            warmRequests.Clear();
            Assert.AreEqual(4, vfx.AppendActiveImpactWarmLightRequests(warmRequests));

            float initialLightIntensity = impactLight.intensity;
            vfx.SetReducedMotion(true);
            Assert.That(visual.localScale.x, Is.EqualTo(0.6f).Within(0.001f));
            Assert.That(impactLight.intensity, Is.LessThan(initialLightIntensity));
            vfx.Advance(33);
            Assert.That(impactLight.intensity, Is.GreaterThan(0f));
            Assert.That(impactLight.intensity, Is.LessThan(initialLightIntensity));
            warmRequests.Clear();
            Assert.AreEqual(4, vfx.AppendActiveImpactWarmLightRequests(warmRequests));
            Assert.That(warmRequests[0].Intensity, Is.LessThan(initialLightIntensity));
            vfx.Advance(43);
            Assert.AreEqual(0, vfx.ActiveEffectCount);
            Assert.AreEqual(0, vfx.ActiveImpactLightCount);
            Assert.IsFalse(impactLight.enabled);
            Object.DestroyImmediate(gameObject);
            Object.DestroyImmediate(cameraObject);
        }

        [Test]
        public void OwnerImpactAppliesBoundedCameraImpulseAndReducedMotionDisablesIt()
        {
            GameObject gameObject = new GameObject("enhanced-impact-camera-impulse-test");
            UnityEnhancedWorldRoot root = gameObject.AddComponent<UnityEnhancedWorldRoot>();
            root.OpenDiagnostic(PresentationModeId.UnityEnhanced);

            UnityEnhancedCameraPose authoritativePose = new UnityEnhancedCameraPose(
                new Vector3(10f, 12f, 14f),
                Quaternion.Euler(7f, 23f, -4f),
                70f);
            UnityEnhancedQualitySettings comfort =
                UnityEnhancedQualitySettings.ForTier(UnityEnhancedQualityTier.WholeHog);
            Assert.IsTrue(root.ApplySnapshot(new UnityEnhancedWorldSnapshot
            {
                Generation = 7,
                Tick = 20,
                CameraPose = authoritativePose,
                Quality = comfort,
                VfxEvents = Array.Empty<UnityEnhancedVfxEvent>()
            }));
            root.InterpolatePresentation(1f);

            Vector3 impactPosition = authoritativePose.Position +
                                     authoritativePose.Rotation * new Vector3(0f, 0f, 6f);
            UnityEnhancedVfxEvent impact = new UnityEnhancedVfxEvent(
                1,
                7,
                24,
                0x4000002aUL,
                UnityEnhancedVfxKind.Impact,
                impactPosition,
                1f,
                18);
            Assert.IsTrue(root.ApplySnapshot(new UnityEnhancedWorldSnapshot
            {
                Generation = 7,
                Tick = 24,
                CameraPose = authoritativePose,
                Quality = comfort,
                VfxEvents = new[] { impact }
            }));
            root.InterpolatePresentation(1f);

            Camera camera = root.CameraRig.PresentationCamera;
            Vector3 firstLocalTranslation = Quaternion.Inverse(authoritativePose.Rotation) *
                                             (camera.transform.position - authoritativePose.Position);
            float firstRotationDegrees = Quaternion.Angle(
                authoritativePose.Rotation,
                camera.transform.rotation);
            Assert.Greater(firstLocalTranslation.magnitude, 0.0001f);
            Assert.LessOrEqual(firstLocalTranslation.magnitude, 0.0351f);
            Assert.Greater(firstRotationDegrees, 0.0001f);
            Assert.LessOrEqual(firstRotationDegrees, 0.4501f);

            Assert.IsTrue(root.ApplySnapshot(new UnityEnhancedWorldSnapshot
            {
                Generation = 7,
                Tick = 24,
                CameraPose = authoritativePose,
                Quality = comfort,
                VfxEvents = new[] { impact }
            }));
            root.InterpolatePresentation(1f);
            Vector3 repeatedLocalTranslation = Quaternion.Inverse(authoritativePose.Rotation) *
                                                (camera.transform.position - authoritativePose.Position);
            float repeatedRotationDegrees = Quaternion.Angle(
                authoritativePose.Rotation,
                camera.transform.rotation);
            Assert.That(Vector3.Distance(firstLocalTranslation, repeatedLocalTranslation),
                Is.LessThan(0.00001f));
            Assert.That(repeatedRotationDegrees, Is.EqualTo(firstRotationDegrees).Within(0.00001f));

            Assert.IsTrue(root.ApplySnapshot(new UnityEnhancedWorldSnapshot
            {
                Generation = 7,
                Tick = 33,
                CameraPose = authoritativePose,
                Quality = comfort,
                VfxEvents = Array.Empty<UnityEnhancedVfxEvent>()
            }));
            root.InterpolatePresentation(1f);
            Vector3 decayedLocalTranslation = Quaternion.Inverse(authoritativePose.Rotation) *
                                               (camera.transform.position - authoritativePose.Position);
            Assert.Greater(decayedLocalTranslation.magnitude, 0.0001f);
            Assert.Less(decayedLocalTranslation.magnitude, firstLocalTranslation.magnitude);

            Assert.IsTrue(root.ApplySnapshot(new UnityEnhancedWorldSnapshot
            {
                Generation = 7,
                Tick = 42,
                CameraPose = authoritativePose,
                Quality = comfort,
                VfxEvents = Array.Empty<UnityEnhancedVfxEvent>()
            }));
            root.InterpolatePresentation(1f);
            Assert.AreEqual(0, root.VfxPresentation.ActiveEffectCount);
            Assert.That(Vector3.Distance(camera.transform.position, authoritativePose.Position),
                Is.LessThan(0.0001f));
            Assert.That(Quaternion.Angle(camera.transform.rotation, authoritativePose.Rotation),
                Is.LessThan(0.001f));

            UnityEnhancedQualitySettings noShake =
                UnityEnhancedQualitySettings.ForTier(UnityEnhancedQualityTier.WholeHog);
            noShake.CameraShake = false;
            Assert.IsTrue(root.ApplySnapshot(new UnityEnhancedWorldSnapshot
            {
                Generation = 8,
                Tick = 0,
                Reset = true,
                CameraPose = authoritativePose,
                Quality = noShake,
                VfxEvents = new[]
                {
                    new UnityEnhancedVfxEvent(
                        1,
                        8,
                        0,
                        0x4000002bUL,
                        UnityEnhancedVfxKind.Impact,
                        impactPosition,
                        1f,
                        18)
                }
            }));
            root.InterpolatePresentation(1f);
            Assert.That(Vector3.Distance(camera.transform.position, authoritativePose.Position),
                Is.LessThan(0.0001f));
            Assert.That(Quaternion.Angle(camera.transform.rotation, authoritativePose.Rotation),
                Is.LessThan(0.001f));

            UnityEnhancedQualitySettings reducedMotion =
                UnityEnhancedQualitySettings.ForTier(UnityEnhancedQualityTier.WholeHog);
            reducedMotion.ReducedMotion = true;
            reducedMotion.CameraShake = false;
            reducedMotion.SpeedStreaks = false;
            reducedMotion.MotionBlur = false;
            Assert.IsTrue(root.ApplySnapshot(new UnityEnhancedWorldSnapshot
            {
                Generation = 9,
                Tick = 0,
                Reset = true,
                CameraPose = authoritativePose,
                Quality = reducedMotion,
                VfxEvents = new[]
                {
                    new UnityEnhancedVfxEvent(
                        1,
                        9,
                        0,
                        0x4000002cUL,
                        UnityEnhancedVfxKind.Impact,
                        impactPosition,
                        1f,
                        18)
                }
            }));
            root.InterpolatePresentation(1f);
            Assert.That(Vector3.Distance(camera.transform.position, authoritativePose.Position),
                Is.LessThan(0.0001f));
            Assert.That(Quaternion.Angle(camera.transform.rotation, authoritativePose.Rotation),
                Is.LessThan(0.001f));

            Object.DestroyImmediate(gameObject);
        }

        [Test]
        public void ImpactVfxProgressesShockwaveAcrossAuthoritativeTicks()
        {
            GameObject gameObject = new GameObject("enhanced-impact-progress-test");
            UnityEnhancedVfxPresentation vfx = gameObject.AddComponent<UnityEnhancedVfxPresentation>();
            vfx.ResetForGeneration(9);

            UnityEnhancedVfxEvent impact = new UnityEnhancedVfxEvent(
                1,
                9,
                40,
                0x4000002bUL,
                UnityEnhancedVfxKind.Impact,
                Vector3.zero,
                1f,
                16);
            Assert.IsTrue(vfx.ApplyEvent(impact));

            Transform visual = gameObject.transform.Find("EnhancedVfxPool/EnhancedVfx");
            Assert.IsNotNull(visual);
            MeshRenderer renderer = visual.GetComponent<MeshRenderer>();
            MaterialPropertyBlock properties = new MaterialPropertyBlock();

            renderer.GetPropertyBlock(properties);
            Assert.That(properties.GetFloat("_ImpactProgress"), Is.EqualTo(0f).Within(0.0001f));

            vfx.Advance(48);
            renderer.GetPropertyBlock(properties);
            Assert.That(properties.GetFloat("_ImpactProgress"), Is.EqualTo(0.5f).Within(0.0001f));
            Assert.AreEqual(1, vfx.ActiveEffectCount);

            vfx.Advance(56);
            Assert.AreEqual(0, vfx.ActiveEffectCount);
            Assert.IsFalse(visual.gameObject.activeSelf);

            Object.DestroyImmediate(gameObject);
        }

        [Test]
        public void OwnerFireballWarmLightPulseAndImpactMappingAreBounded()
        {
            float first = UnityEnhancedOwnerSessionSource.FireballLightIntensityPulse(6, 1);
            float replay = UnityEnhancedOwnerSessionSource.FireballLightIntensityPulse(6, 1);
            float later = UnityEnhancedOwnerSessionSource.FireballLightIntensityPulse(11, 1);
            float alternate = UnityEnhancedOwnerSessionSource.FireballLightIntensityPulse(6, 2);
            float range = UnityEnhancedOwnerSessionSource.FireballLightRangePulse(6, 1);

            Assert.That(replay, Is.EqualTo(first).Within(0.000001f));
            Assert.That(Mathf.Abs(later - first), Is.GreaterThan(0.001f));
            Assert.That(Mathf.Abs(alternate - first), Is.GreaterThan(0.001f));
            Assert.That(first, Is.InRange(0.82f, 1.18f));
            Assert.That(later, Is.InRange(0.82f, 1.18f));
            Assert.That(alternate, Is.InRange(0.82f, 1.18f));
            Assert.That(range, Is.InRange(0.94f, 1.06f));
            Assert.AreEqual(
                UnityEnhancedVfxKind.Impact,
                UnityEnhancedOwnerSessionSource.ClassifyImpactVfxKind(16, 0));
            Assert.AreEqual(
                UnityEnhancedVfxKind.Impact,
                UnityEnhancedOwnerSessionSource.ClassifyImpactVfxKind(16, 6));
            Assert.AreEqual(
                UnityEnhancedVfxKind.Weather,
                UnityEnhancedOwnerSessionSource.ClassifyImpactVfxKind(37, 0));
            Assert.AreEqual(
                UnityEnhancedVfxKind.Smoke,
                UnityEnhancedOwnerSessionSource.ClassifyImpactVfxKind(99, 13));
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
            Assert.AreEqual(1.25f, quality.AppliedRenderScale);
            Assert.IsTrue(quality.UsesSupersampling);
            Assert.IsFalse(quality.UsesSpatialUpscaling);
            Assert.AreEqual(0f, quality.AppliedPostExposure);
            Assert.IsFalse(quality.TemporalUpscalingEnabled);
            Assert.IsTrue(quality.AmbientOcclusionRequested);
            Assert.IsTrue(quality.AmbientOcclusionEnabled);
            Assert.IsNotNull(quality.PostVolume);
            MotionBlur appliedMotionBlur;
            Assert.IsTrue(quality.PostVolume.profile.TryGet(out appliedMotionBlur));
            Assert.IsTrue(quality.MotionBlurEnabled);
            Assert.That(quality.AppliedMotionBlurIntensity, Is.EqualTo(0.28f).Within(0.0001f));
            Assert.IsTrue(appliedMotionBlur.active);
            Assert.AreEqual(MotionBlurMode.CameraOnly, appliedMotionBlur.mode.value);
            Assert.AreEqual(MotionBlurQuality.High, appliedMotionBlur.quality.value);
            Assert.That(appliedMotionBlur.intensity.value, Is.EqualTo(0.28f).Within(0.0001f));
            Assert.That(appliedMotionBlur.clamp.value, Is.EqualTo(0.12f).Within(0.0001f));
            Assert.AreEqual(1, quality.CameraHistoryResetRequestCount);
            Assert.IsTrue(quality.Apply(
                UnityEnhancedQualitySettings.ForTier(UnityEnhancedQualityTier.WholeHog),
                camera));
            Assert.AreEqual(1, quality.CameraHistoryResetRequestCount);
            Assert.IsTrue(quality.Apply(
                UnityEnhancedQualitySettings.ForTier(UnityEnhancedQualityTier.WholeHog),
                camera,
                0.35f));
            Assert.AreEqual(0.35f, quality.AppliedPostExposure, 0.0001f);
            Assert.AreEqual(1, quality.CameraHistoryResetRequestCount);
            UnityEnhancedQualitySettings reducedMotion =
                UnityEnhancedQualitySettings.ForTier(UnityEnhancedQualityTier.WholeHog);
            reducedMotion.ReducedMotion = true;
            reducedMotion.CameraShake = false;
            reducedMotion.SpeedStreaks = false;
            reducedMotion.MotionBlur = false;
            Assert.IsTrue(quality.Apply(reducedMotion, camera));
            Assert.AreEqual(2, quality.CameraHistoryResetRequestCount);
            Assert.IsFalse(quality.MotionBlurEnabled);
            Assert.AreEqual(0f, quality.AppliedMotionBlurIntensity);
            Assert.IsFalse(appliedMotionBlur.active);
            Assert.AreEqual(0f, appliedMotionBlur.intensity.value);
            quality.ResetForSession();

            Object.DestroyImmediate(gameObject);
        }

        [Test]
        public void WholeHogAmbientOcclusionUsesDedicatedRendererAndRestoresDefault()
        {
            UniversalRenderPipelineAsset pipeline = UniversalRenderPipeline.asset;
            Assert.IsNotNull(pipeline);
            int enhancedRendererIndex = -1;
            ReadOnlySpan<ScriptableRendererData> rendererDataList = pipeline.rendererDataList;
            for (int index = 0; index < rendererDataList.Length; index++)
            {
                if (rendererDataList[index] != null &&
                    rendererDataList[index].name == "PresentationLabEnhancedRenderer")
                {
                    enhancedRendererIndex = index;
                    break;
                }
            }

            Assert.GreaterOrEqual(enhancedRendererIndex, 0);
            string enhancedRendererAsset = File.ReadAllText(
                AssetDatabase.GetAssetPath(rendererDataList[enhancedRendererIndex]));
            StringAssert.Contains("m_Active: 1", enhancedRendererAsset);
            StringAssert.Contains("AOMethod: 1", enhancedRendererAsset);
            StringAssert.Contains("AfterOpaque: 1", enhancedRendererAsset);
            StringAssert.Contains("Source: 0", enhancedRendererAsset);

            bool hasActiveSsao = false;
            for (int featureIndex = 0;
                 featureIndex < rendererDataList[enhancedRendererIndex].rendererFeatures.Count;
                 featureIndex++)
            {
                if (rendererDataList[enhancedRendererIndex].rendererFeatures[featureIndex]
                    is ScreenSpaceAmbientOcclusion ambientOcclusion && ambientOcclusion.isActive)
                {
                    hasActiveSsao = true;
                    break;
                }
            }
            Assert.IsTrue(hasActiveSsao);

            GameObject gameObject = new GameObject("enhanced-dedicated-ssao-test");
            Camera camera = gameObject.AddComponent<Camera>();
            UnityEnhancedQualityAndPost quality = gameObject.AddComponent<UnityEnhancedQualityAndPost>();
            Assert.IsTrue(quality.Apply(
                UnityEnhancedQualitySettings.ForTier(UnityEnhancedQualityTier.WholeHog),
                camera));
            UniversalAdditionalCameraData cameraData =
                gameObject.GetComponent<UniversalAdditionalCameraData>();
            Assert.AreEqual(
                pipeline.renderers[enhancedRendererIndex],
                cameraData.scriptableRenderer);

            quality.ResetForSession();
            Assert.AreEqual(pipeline.renderers[0], cameraData.scriptableRenderer);
            Object.DestroyImmediate(gameObject);
        }

        [Test]
        public void EnhancedNightLightingUsesLongRangeCascadedMoonShadows()
        {
            UniversalRenderPipelineAsset pipeline = UniversalRenderPipeline.asset;
            Assert.IsNotNull(pipeline);
            Assert.IsTrue(pipeline.supportsMainLightShadows);
            Assert.AreEqual(128f, pipeline.shadowDistance);
            Assert.AreEqual(2, pipeline.shadowCascadeCount);
            Assert.That(pipeline.mainLightShadowmapResolution, Is.GreaterThanOrEqualTo(2048));

            GameObject gameObject = new GameObject("enhanced-moon-shadow-test");
            UnityEnhancedNightLighting lighting = gameObject.AddComponent<UnityEnhancedNightLighting>();
            lighting.SetQuality(UnityEnhancedLightingQuality.WholeHog);
            lighting.ApplyDescriptor(
                1,
                UnityEnhancedNightLightingDescriptor.Level1Night(),
                Array.Empty<UnityEnhancedLocalLightRequest>(),
                true);

            Transform moonTransform = gameObject.transform.Find("EnhancedMoonLight");
            Assert.IsNotNull(moonTransform);
            Light moon = moonTransform.GetComponent<Light>();
            Assert.IsNotNull(moon);
            Assert.AreEqual(LightShadows.Soft, moon.shadows);
            Assert.That(moon.transform.forward.y, Is.LessThan(-0.5f));
            Assert.That(
                Vector3.Dot(
                    moon.transform.forward,
                    UnityEnhancedNightLightingDescriptor.Level1Night().MoonDirection.normalized),
                Is.GreaterThan(0.999f));
            Assert.AreSame(moon, RenderSettings.sun);

            lighting.ResetForSession();
            Object.DestroyImmediate(gameObject);
        }

        [Test]
        public void SpatialQualityRequestsFsrAndRestoresPipelineSettings()
        {
            GameObject gameObject = new GameObject("enhanced-spatial-upscale-test");
            Camera camera = gameObject.AddComponent<Camera>();
            UnityEnhancedQualityAndPost quality = gameObject.AddComponent<UnityEnhancedQualityAndPost>();

            Assert.IsTrue(quality.Apply(
                UnityEnhancedQualitySettings.ForTier(UnityEnhancedQualityTier.Medium),
                camera));
            Assert.IsTrue(quality.UsesSpatialUpscaling);
            Assert.AreEqual(UpscalingFilterSelection.FSR, UniversalRenderPipeline.asset.upscalingFilter);

            quality.ResetForSession();
            Object.DestroyImmediate(gameObject);
        }

        [Test]
        public void OwnerQualityTierIsSelectedBeforeConfiguration()
        {
            UnityEnhancedOwnerSessionSource source = new UnityEnhancedOwnerSessionSource();
            string reason;

            Assert.IsTrue(source.TrySetQualityTier(UnityEnhancedQualityTier.Medium, out reason));
            Assert.AreEqual(UnityEnhancedQualityTier.Medium, source.QualityTier);
            Assert.IsFalse(source.TrySetQualityTier((UnityEnhancedQualityTier)99, out reason));
            Assert.AreEqual(UnityEnhancedQualityTier.Medium, source.QualityTier);

            source.Dispose();
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
            Assert.That(renderer.sharedMaterial.color.r, Is.EqualTo(1f).Within(0.001f));
            Assert.That(renderer.sharedMaterial.color.g, Is.EqualTo(1f).Within(0.001f));
            Assert.That(renderer.sharedMaterial.color.b, Is.EqualTo(1f).Within(0.001f));
            Assert.AreEqual(ShadowCastingMode.On, renderer.shadowCastingMode);
            Assert.IsTrue(renderer.receiveShadows);

            // Simulate a pooled view that previously displayed an owner
            // billboard, whose renderer contract is shadow-off.
            renderer.shadowCastingMode = ShadowCastingMode.Off;
            renderer.receiveShadows = false;
            UnityEnhancedEntitySnapshot castle = Entity(1, 42, Vector3.zero);
            castle.PresentationFamily = (int)UnityEnhancedPresentationFamily.Castle;
            actors.Configure(view, castle);
            Assert.Greater(renderer.sharedMaterial.GetFloat("_AccentStrength"), 0.7f);
            Assert.AreEqual(ShadowCastingMode.On, renderer.shadowCastingMode);
            Assert.IsTrue(renderer.receiveShadows);

            Object.DestroyImmediate(gameObject);
        }

        [Test]
        public void EnhancedStructureShaderAddsBoundedMoonFacingRelief()
        {
            string shaderSource = File.ReadAllText(
                Path.Combine(Application.dataPath, "Resources", "EnhancedVertexColorLit.shader"));
            StringAssert.Contains("structureMask = smoothstep(0.60h, 0.90h, _AccentStrength)", shaderSource);
            StringAssert.Contains("moonFacing = saturate(dot(normalWS, mainLight.direction))", shaderSource);
            StringAssert.Contains("lerp(0.82h, 1.12h, moonFacing)", shaderSource);
            StringAssert.Contains("structureMask * 0.68h", shaderSource);
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

        private static UnityEnhancedWorldSnapshot SpeedSnapshot(
            ulong generation,
            ulong tick,
            int actualSpeed,
            byte speedTrailCount,
            UnityEnhancedQualitySettings quality)
        {
            UnityEnhancedHudSnapshot hud = OwnerHudSnapshot(generation, tick);
            hud.ActualSpeed = actualSpeed;
            hud.SpeedTrailCount = speedTrailCount;
            return new UnityEnhancedWorldSnapshot
            {
                Generation = generation,
                Tick = tick,
                CameraPose = new UnityEnhancedCameraPose(Vector3.zero, Quaternion.identity, 70f),
                Entities = new UnityEnhancedEntitySnapshot[0],
                Hud = hud,
                Quality = quality
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

        private static UnityEnhancedEntitySnapshot FireballEntity(
            ulong generation,
            ulong identity,
            Vector3 position)
        {
            UnityEnhancedEntitySnapshot entity = Entity(generation, identity, position);
            entity.PresentationFamily = (int)UnityEnhancedPresentationFamily.Fire;
            entity.Scale = 1f;
            entity.RenderLayer = 2201;
            return entity;
        }
    }
}
#endif
