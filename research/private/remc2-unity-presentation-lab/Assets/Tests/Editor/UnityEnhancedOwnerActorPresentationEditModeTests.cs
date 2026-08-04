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
                Assert.That(
                    renderer.sharedMaterial.GetFloat("_EmissionStrength"),
                    Is.EqualTo(0.10f).Within(0.0001f));
                StringAssert.StartsWith("Owner Indexed Actor Billboard Quad", filter.sharedMesh.name);
                StringAssert.StartsWith("Owner Indexed Actor Material", renderer.sharedMaterial.name);
                StringAssert.StartsWith("Owner Indexed Actor", filter.sharedMesh.name);
                Assert.That(
                    visual.localScale.y,
                    Is.EqualTo(150f / 64f * 0.5f * 1.5f).Within(0.0001f));
                Assert.That(
                    visual.localPosition.y,
                    Is.EqualTo((150f / 64f * 0.5f * 1.5f) * 0.5f + 0.12f).Within(0.0001f));
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
        public void QualifiedOwnerActorUsesBoundedViewportReadabilityWithoutReplacingOwnerPixels()
        {
            string cachePath = OwnerCachePath();
            Assert.That(
                Directory.Exists(cachePath),
                Is.True,
                "The private Level 1 owner actor cache is required for this test: " + cachePath);

            GameObject gameObject = new GameObject("enhanced-owner-actor-depth-scale-test");
            try
            {
                UnityEnhancedWorldRoot root = gameObject.AddComponent<UnityEnhancedWorldRoot>();
                root.OpenDiagnostic(PresentationModeId.UnityEnhanced);

                string reason;
                Assert.IsTrue(root.ConfigureOwnerActorCache(cachePath, out reason), reason);

                UnityEnhancedEntitySnapshot entity = OwnerEntity(
                    1,
                    0x1000000000000003UL,
                    50,
                    0,
                    0,
                    4f);
                Assert.IsTrue(root.ApplySnapshot(OwnerSnapshot(1, 0, entity)));

                Transform visual = gameObject.transform.Find(
                    "EnhancedWorldObjects/EnhancedEntity_1000000000000003/EnhancedActorVisual");
                Assert.IsNotNull(visual);
                MeshRenderer renderer = visual.GetComponent<MeshRenderer>();
                MeshFilter filter = visual.GetComponent<MeshFilter>();
                Assert.IsNotNull(renderer);
                Assert.IsNotNull(filter);
                string materialName = renderer.sharedMaterial.name;
                string meshName = filter.sharedMesh.name;
                float sourceHeight = 150f / 64f * 0.5f;
                float nearScale = visual.localScale.y / sourceHeight;
                Assert.That(nearScale, Is.EqualTo(1.5f).Within(0.0001f));

                entity.Position = new Vector3(0f, 0f, 96f);
                entity.Generation = 1;
                Assert.IsTrue(root.ApplySnapshot(OwnerSnapshot(1, 1, entity)));
                float midScale = visual.localScale.y / sourceHeight;
                Assert.Greater(midScale, nearScale);
                Assert.That(midScale, Is.EqualTo(3.44f).Within(0.02f));

                entity.Position = new Vector3(0f, 0f, 160f);
                entity.Generation = 1;
                Assert.IsTrue(root.ApplySnapshot(OwnerSnapshot(1, 2, entity)));
                float farScale = visual.localScale.y / sourceHeight;
                Assert.That(farScale, Is.EqualTo(4f).Within(0.0001f));
                Assert.AreEqual(materialName, renderer.sharedMaterial.name);
                Assert.AreEqual(meshName, filter.sharedMesh.name);
                Assert.IsFalse(meshName.StartsWith("EnhancedGenerated", StringComparison.Ordinal));
                Assert.IsFalse(materialName.StartsWith("EnhancedGenerated", StringComparison.Ordinal));
            }
            finally
            {
                Object.DestroyImmediate(gameObject);
            }
        }

        [Test]
        public void QualifiedOwnerActorAddsBoundedContactShadowAndClearsOnReuse()
        {
            string cachePath = OwnerCachePath();
            Assert.That(
                Directory.Exists(cachePath),
                Is.True,
                "The private Level 1 owner actor cache is required for this test: " + cachePath);

            GameObject gameObject = new GameObject("enhanced-owner-contact-shadow-test");
            try
            {
                UnityEnhancedWorldRoot root = gameObject.AddComponent<UnityEnhancedWorldRoot>();
                root.OpenDiagnostic(PresentationModeId.UnityEnhanced);

                string reason;
                Assert.IsTrue(root.ConfigureOwnerActorCache(cachePath, out reason), reason);

                UnityEnhancedEntitySnapshot entity = OwnerEntity(
                    1,
                    0x1000000000000005UL,
                    50,
                    0,
                    0);
                Assert.IsTrue(root.ApplySnapshot(OwnerSnapshot(1, 0, entity)));

                Transform view = gameObject.transform.Find(
                    "EnhancedWorldObjects/EnhancedEntity_1000000000000005");
                Assert.IsNotNull(view);
                Transform visual = view.Find("EnhancedActorVisual");
                Transform shadow = view.Find("EnhancedOwnerContactShadow");
                Assert.IsNotNull(visual);
                Assert.IsNotNull(shadow);

                MeshRenderer ownerRenderer = visual.GetComponent<MeshRenderer>();
                MeshFilter ownerFilter = visual.GetComponent<MeshFilter>();
                MeshRenderer shadowRenderer = shadow.GetComponent<MeshRenderer>();
                MeshFilter shadowFilter = shadow.GetComponent<MeshFilter>();
                Assert.IsNotNull(ownerRenderer);
                Assert.IsNotNull(ownerFilter);
                Assert.IsNotNull(shadowRenderer);
                Assert.IsNotNull(shadowFilter);
                Material ownerMaterial = ownerRenderer.sharedMaterial;
                Mesh ownerMesh = ownerFilter.sharedMesh;
                Assert.AreEqual(
                    "MagicCarpet/Enhanced/OwnerEntitySprite",
                    ownerMaterial.shader.name);
                Assert.IsTrue(shadow.gameObject.activeSelf);
                Assert.IsTrue(shadowRenderer.enabled);
                Assert.AreEqual(
                    "MagicCarpet/Enhanced/ActorContactShadow",
                    shadowRenderer.sharedMaterial.shader.name);
                Assert.IsFalse(shadowRenderer.sharedMaterial.HasProperty("_MainTex"));
                Assert.That(
                    shadowRenderer.sharedMaterial.GetColor("_ShadowColor").a,
                    Is.EqualTo(1f).Within(0.0001f));
                Assert.That(
                    shadowRenderer.sharedMaterial.GetFloat("_Opacity"),
                    Is.EqualTo(0.28f).Within(0.0001f));
                Assert.That(
                    shadowRenderer.sharedMaterial.GetFloat("_Softness"),
                    Is.EqualTo(0.72f).Within(0.0001f));
                Assert.AreEqual(view, shadow.parent);
                Assert.That(shadow.localPosition, Is.EqualTo(new Vector3(0f, 0.02f, 0f)));
                Assert.That(shadowFilter.sharedMesh.bounds.size.y, Is.EqualTo(0f).Within(0.0001f));
                Assert.Greater(shadowFilter.sharedMesh.bounds.size.x, 0f);
                Assert.Greater(shadowFilter.sharedMesh.bounds.size.z, 0f);
                Assert.That(
                    shadow.localScale.x,
                    Is.EqualTo(visual.localScale.x * 1.35f).Within(0.0001f));
                Assert.That(shadow.localScale.x, Is.GreaterThan(0.2f));
                Assert.That(shadow.localScale.x, Is.LessThanOrEqualTo(3.5f));
                Assert.AreEqual(1, CountNamedChildren(view, "EnhancedOwnerContactShadow"));

                UnityEnhancedEntitySnapshot generated = OwnerEntity(
                    1,
                    entity.StableIdentity,
                    50,
                    0,
                    0);
                generated.PresentationFamily = (int)UnityEnhancedPresentationFamily.Town;
                Assert.IsTrue(root.ApplySnapshot(OwnerSnapshot(1, 1, generated)));
                Assert.IsFalse(shadow.gameObject.activeSelf);
                Assert.IsFalse(shadowRenderer.enabled);
                Assert.AreEqual(1, CountNamedChildren(view, "EnhancedOwnerContactShadow"));

                Assert.IsTrue(root.ApplySnapshot(OwnerSnapshot(1, 2, entity)));
                Assert.IsTrue(shadow.gameObject.activeSelf);
                Assert.IsTrue(shadowRenderer.enabled);
                Assert.AreSame(ownerMaterial, ownerRenderer.sharedMaterial);
                Assert.AreSame(ownerMesh, ownerFilter.sharedMesh);
                Assert.AreEqual(1, CountNamedChildren(view, "EnhancedOwnerContactShadow"));

                UnityEnhancedEntitySnapshot unresolved = OwnerEntity(
                    1,
                    entity.StableIdentity,
                    999,
                    0,
                    0);
                Assert.IsTrue(root.ApplySnapshot(OwnerSnapshot(1, 3, unresolved)));
                Assert.IsFalse(shadow.gameObject.activeSelf);
                Assert.IsFalse(shadowRenderer.enabled);
                Assert.AreEqual(1, CountNamedChildren(view, "EnhancedOwnerContactShadow"));
                Assert.AreSame(ownerMaterial, ownerRenderer.sharedMaterial);

                UnityEnhancedWorldSnapshot empty = OwnerSnapshot(1, 4, unresolved);
                empty.Entities = Array.Empty<UnityEnhancedEntitySnapshot>();
                Assert.IsTrue(root.ApplySnapshot(empty));
                Assert.IsFalse(view.gameObject.activeSelf);
                Assert.IsFalse(shadow.gameObject.activeSelf);

                UnityEnhancedEntitySnapshot pooledUnresolved = OwnerEntity(
                    1,
                    0x1000000000000006UL,
                    999,
                    0,
                    0);
                Assert.IsTrue(root.ApplySnapshot(OwnerSnapshot(1, 5, pooledUnresolved)));
                Assert.IsFalse(shadow.gameObject.activeSelf);
                Assert.IsFalse(shadowRenderer.enabled);
                Assert.AreEqual(1, CountNamedChildren(view, "EnhancedOwnerContactShadow"));

                root.ResetPresentation(2);
                Assert.IsFalse(view.gameObject.activeSelf);
                Assert.IsFalse(shadow.gameObject.activeSelf);
                Assert.IsFalse(shadowRenderer.enabled);
            }
            finally
            {
                Object.DestroyImmediate(gameObject);
            }
        }

        [Test]
        public void OwnerEntitySpriteAddsOneTexelOutlineWithoutReplacingOwnerPixels()
        {
            Shader shader = Shader.Find("MagicCarpet/Enhanced/OwnerEntitySprite");
            Assert.IsNotNull(shader);

            Texture2D indices = new Texture2D(5, 5, TextureFormat.RGBA32, false, true)
            {
                filterMode = FilterMode.Point,
                wrapMode = TextureWrapMode.Clamp
            };
            Color32[] indexPixels = new Color32[25];
            indexPixels[2 + 2 * 5] = new Color32(1, 1, 1, 255);
            indices.SetPixels32(indexPixels);
            indices.Apply(false, true);

            Texture2D palette = new Texture2D(256, 1, TextureFormat.RGBA32, false, true)
            {
                filterMode = FilterMode.Point,
                wrapMode = TextureWrapMode.Clamp
            };
            Color32[] palettePixels = new Color32[256];
            palettePixels[1] = new Color32(255, 255, 255, 255);
            palette.SetPixels32(palettePixels);
            palette.Apply(false, true);

            Material material = new Material(shader);
            RenderTexture target = new RenderTexture(
                5,
                5,
                0,
                RenderTextureFormat.ARGB32,
                RenderTextureReadWrite.Linear);
            Texture2D readback = new Texture2D(5, 5, TextureFormat.RGBA32, false, true);
            RenderTexture previousActive = RenderTexture.active;
            try
            {
                material.SetTexture("_Indices", indices);
                material.SetTexture("_Palette", palette);
                material.SetColor("_Tint", Color.white);
                material.SetFloat("_EmissionStrength", 0f);
                material.SetColor("_OutlineColor", Color.red);
                material.SetFloat("_OutlineStrength", 1f);

                RenderIndexedSprite(material, target, readback);
                Color center = readback.GetPixel(2, 2);
                Assert.That(center.a, Is.GreaterThan(0.9f));
                Assert.That(center.g, Is.GreaterThan(0.65f));
                Assert.That(center.b, Is.GreaterThan(0.75f));
                Assert.That(readback.GetPixel(1, 2).r, Is.GreaterThan(0.65f));
                Assert.That(readback.GetPixel(1, 2).g, Is.LessThan(0.20f));
                Assert.That(readback.GetPixel(3, 2).r, Is.GreaterThan(0.65f));
                Assert.That(readback.GetPixel(2, 1).r, Is.GreaterThan(0.65f));
                Assert.That(readback.GetPixel(2, 3).r, Is.GreaterThan(0.65f));
                Assert.That(readback.GetPixel(0, 2).a, Is.LessThan(0.05f));
                Assert.That(readback.GetPixel(1, 1).a, Is.LessThan(0.05f));

                Color centerWithOutline = center;
                material.SetFloat("_OutlineStrength", 0f);
                RenderIndexedSprite(material, target, readback);
                Color centerWithoutOutline = readback.GetPixel(2, 2);
                Assert.That(centerWithoutOutline.a, Is.GreaterThan(0.9f));
                Assert.That(centerWithoutOutline.r, Is.EqualTo(centerWithOutline.r).Within(0.03f));
                Assert.That(centerWithoutOutline.g, Is.EqualTo(centerWithOutline.g).Within(0.03f));
                Assert.That(centerWithoutOutline.b, Is.EqualTo(centerWithOutline.b).Within(0.03f));
                Assert.That(readback.GetPixel(1, 2).a, Is.LessThan(0.05f));
            }
            finally
            {
                RenderTexture.active = previousActive;
                Object.DestroyImmediate(readback);
                Object.DestroyImmediate(target);
                Object.DestroyImmediate(material);
                Object.DestroyImmediate(palette);
                Object.DestroyImmediate(indices);
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

        private static void RenderIndexedSprite(
            Material material,
            RenderTexture target,
            Texture2D readback)
        {
            RenderTexture.active = target;
            GL.Clear(true, true, Color.clear);
            Graphics.Blit(Texture2D.whiteTexture, target, material);
            readback.ReadPixels(new Rect(0f, 0f, 5f, 5f), 0, 0, false);
            readback.Apply(false, false);
        }

        private static int CountNamedChildren(Transform root, string name)
        {
            Transform[] descendants = root.GetComponentsInChildren<Transform>(true);
            int count = 0;
            for (int index = 0; index < descendants.Length; index++)
            {
                if (descendants[index].name == name)
                {
                    count++;
                }
            }

            return count;
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
            return OwnerEntity(
                generation,
                stableIdentity,
                sourceEntityIndex,
                sourceYaw,
                cameraYaw,
                4f);
        }

        private static UnityEnhancedEntitySnapshot OwnerEntity(
            ulong generation,
            ulong stableIdentity,
            ushort sourceEntityIndex,
            short sourceYaw,
            short cameraYaw,
            float forwardDepth)
        {
            return new UnityEnhancedEntitySnapshot
            {
                StableIdentity = stableIdentity,
                Generation = generation,
                SourceEntityIndex = sourceEntityIndex,
                SourceYaw = sourceYaw,
                CameraYaw = cameraYaw,
                Position = new Vector3(0f, 0f, forwardDepth),
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
