#if UNITY_EDITOR
using System;
using System.IO;
using NUnit.Framework;
using UnityEngine;
using Object = UnityEngine.Object;

namespace MagicCarpet.PresentationLab.Tests
{
    public sealed class UnityEnhancedOwnerActorPresentationEditModeTests
    {
        [Test]
        public void QualifiedOwnerActorUsesIndexedBillboardAndYawSelection()
        {
            string cachePath = OwnerCachePath();
            Assert.That(
                Directory.Exists(cachePath),
                Is.True,
                "The private Level 1 owner actor cache is required for this test: " + cachePath);

            GameObject gameObject = new GameObject("enhanced-owner-actor-test");
            try
            {
                UnityEnhancedWorldRoot root = gameObject.AddComponent<UnityEnhancedWorldRoot>();
                root.OpenDiagnostic(PresentationModeId.UnityEnhanced);

                string reason;
                Assert.IsTrue(root.ConfigureOwnerActorCache(cachePath, out reason), reason);

                UnityEnhancedEntitySnapshot entity = OwnerEntity(
                    1,
                    0x1000000000000001UL,
                    50,
                    0,
                    0);
                Assert.IsTrue(root.ApplySnapshot(OwnerSnapshot(1, 0, entity)));

                Transform view = gameObject.transform.Find(
                    "EnhancedWorldObjects/EnhancedEntity_1000000000000001");
                Assert.IsNotNull(view);
                Transform visual = view.Find("EnhancedActorVisual");
                Assert.IsNotNull(visual);
                MeshRenderer renderer = visual.GetComponent<MeshRenderer>();
                MeshFilter filter = visual.GetComponent<MeshFilter>();
                Assert.IsNotNull(renderer);
                Assert.IsNotNull(filter);
                Assert.IsTrue(renderer.enabled);
                Assert.IsTrue(visual.gameObject.activeSelf);
                Assert.AreEqual(
                    "MagicCarpet/Enhanced/OwnerEntitySprite",
                    renderer.sharedMaterial.shader.name);
                StringAssert.StartsWith("Owner Indexed Actor Billboard Quad", filter.sharedMesh.name);
                StringAssert.StartsWith("Owner Indexed Actor Material", renderer.sharedMaterial.name);
                StringAssert.StartsWith("Owner Indexed Actor", filter.sharedMesh.name);
                Assert.That(visual.localScale.y, Is.EqualTo(150f / 64f * 0.5f).Within(0.0001f));
                Assert.That(
                    visual.localPosition.y,
                    Is.EqualTo((150f / 64f * 0.5f) * 0.5f + 0.12f).Within(0.0001f));
                Assert.IsFalse(filter.sharedMesh.name.StartsWith("EnhancedGenerated", StringComparison.Ordinal));
                Assert.IsFalse(renderer.sharedMaterial.name.StartsWith("EnhancedGenerated", StringComparison.Ordinal));

                UnityEnhancedEntitySnapshot rearEntity = OwnerEntity(
                    1,
                    0x1000000000000001UL,
                    50,
                    1024,
                    0);
                Assert.IsTrue(root.ApplySnapshot(OwnerSnapshot(1, 1, rearEntity)));
                StringAssert.Contains("Mirrored", filter.sharedMesh.name);
                StringAssert.Contains("154", renderer.sharedMaterial.name);
            }
            finally
            {
                Object.DestroyImmediate(gameObject);
            }
        }

        [Test]
        public void ConfiguredOwnerRouteFailsClosedForUnqualifiedActor()
        {
            string cachePath = OwnerCachePath();
            Assert.That(
                Directory.Exists(cachePath),
                Is.True,
                "The private Level 1 owner actor cache is required for this test: " + cachePath);

            GameObject gameObject = new GameObject("enhanced-owner-actor-fail-closed-test");
            try
            {
                UnityEnhancedWorldRoot root = gameObject.AddComponent<UnityEnhancedWorldRoot>();
                root.OpenDiagnostic(PresentationModeId.UnityEnhanced);

                string reason;
                Assert.IsTrue(root.ConfigureOwnerActorCache(cachePath, out reason), reason);

                UnityEnhancedEntitySnapshot entity = OwnerEntity(
                    1,
                    0x1000000000000002UL,
                    999,
                    0,
                    0);
                Assert.IsTrue(root.ApplySnapshot(OwnerSnapshot(1, 0, entity)));

                Transform visual = gameObject.transform.Find(
                    "EnhancedWorldObjects/EnhancedEntity_1000000000000002/EnhancedActorVisual");
                Assert.IsNotNull(visual);
                MeshRenderer renderer = visual.GetComponent<MeshRenderer>();
                MeshFilter filter = visual.GetComponent<MeshFilter>();
                Assert.IsNotNull(renderer);
                Assert.IsNotNull(filter);
                Assert.IsFalse(renderer.enabled);
                Assert.IsFalse(visual.gameObject.activeSelf);
                Assert.IsNull(filter.sharedMesh);
                Assert.IsNull(renderer.sharedMaterial);
            }
            finally
            {
                Object.DestroyImmediate(gameObject);
            }
        }

        private static string OwnerCachePath()
        {
            string projectRoot = Directory.GetParent(Application.dataPath).FullName;
            return Path.GetFullPath(Path.Combine(
                projectRoot,
                "..",
                "generated",
                "level1-unity-asset-cache-plan29-final"));
        }

        private static UnityEnhancedWorldSnapshot OwnerSnapshot(
            ulong generation,
            ulong tick,
            UnityEnhancedEntitySnapshot entity)
        {
            return new UnityEnhancedWorldSnapshot
            {
                Generation = generation,
                Tick = tick,
                Reset = tick == 0,
                CameraPose = new UnityEnhancedCameraPose(Vector3.zero, Quaternion.identity, 70f),
                Entities = new[] { entity },
                LocalLights = Array.Empty<UnityEnhancedLocalLightRequest>(),
                VfxEvents = Array.Empty<UnityEnhancedVfxEvent>()
            };
        }

        private static UnityEnhancedEntitySnapshot OwnerEntity(
            ulong generation,
            ulong stableIdentity,
            ushort sourceEntityIndex,
            short sourceYaw,
            short cameraYaw)
        {
            return new UnityEnhancedEntitySnapshot
            {
                StableIdentity = stableIdentity,
                Generation = generation,
                SourceEntityIndex = sourceEntityIndex,
                SourceYaw = sourceYaw,
                CameraYaw = cameraYaw,
                Position = new Vector3(0f, 0f, 4f),
                Rotation = Quaternion.identity,
                Visible = true,
                PresentationFamily = (int)UnityEnhancedPresentationFamily.Goat,
                FacingDegrees = 0f,
                AnimationPhase = 0f,
                Scale = 1f,
                RenderLayer = 7
            };
        }
    }
}
#endif
