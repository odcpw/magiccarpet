using NUnit.Framework;
using System.Collections.Generic;
using System.IO;
using UnityEngine;
using UnityEngine.Rendering;
using UnityEngine.Rendering.Universal;

namespace MagicCarpet.Native.Tests
{
    public sealed class ModernStoneStatueEditModeTests
    {
        [Test]
        public void OnlyNightBankUsesTheStatue()
        {
            Assert.That(ModernStoneStatues.SupportsBank("TMAPS1-0"), Is.True);
            Assert.That(ModernStoneStatues.SupportsBank("TMAPS0-0"), Is.False);
            Assert.That(ModernStoneStatues.SupportsBank(null), Is.False);
        }

        [Test]
        public void PresenterMaterialUsesTheImportedStoneAlbedoAndBlueGreyTint()
        {
            var texture = Resources.Load<Texture2D>("ModernScenery/StoneStatueAlbedo");
            Assert.That(texture, Is.Not.Null);
            Assert.That(texture.width, Is.EqualTo(1024));
            Assert.That(texture.height, Is.EqualTo(1024));
            var material = ModernStoneStatues.CreateMaterial();
            try
            {
                Assert.That(material.shader.name, Is.EqualTo("Universal Render Pipeline/Lit"));
                Assert.That(material.GetTexture("_BaseMap"), Is.SameAs(texture));
                var tint = material.GetColor("_BaseColor");
                Assert.That(tint.b, Is.GreaterThan(tint.g));
                Assert.That(tint.g, Is.GreaterThan(tint.r));
                Assert.That(material.GetFloat("_Smoothness"), Is.EqualTo(.08f).Within(.001f));
            }
            finally { Object.DestroyImmediate(material); }
        }

        [Test]
        public void AdmissionRequiresLiveStoneAndMatchingSourceIdentity()
        {
            var entity = new NativeSim.CampaignEntitySnapshot {
                entityIndex = 130, sourceId = 130, active = 1, classId = 2, model = 1, action = 3
            };
            var view = new NativeSim.CampaignEntityPresentation {
                entityIndex = 130, sourceId = 130, renderParameterIndex = 79,
                baseTmap = 78, selectedTmap = 78, logicalHeight = 500
            };
            Assert.That(ModernStoneStatues.Matches(entity, view), Is.True);
            var other = entity; other.sourceId++;
            Assert.That(ModernStoneStatues.Matches(other, view), Is.False);
            other = entity; other.model = 2;
            Assert.That(ModernStoneStatues.Matches(other, view), Is.False);
            other = entity; other.active = 0;
            Assert.That(ModernStoneStatues.Matches(other, view), Is.False);
            var otherView = view; otherView.selectedTmap = 79;
            Assert.That(ModernStoneStatues.Matches(entity, otherView), Is.False);
            IReadOnlyDictionary<ushort, NativeSim.CampaignEntitySnapshot> current =
                new Dictionary<ushort, NativeSim.CampaignEntitySnapshot> { [entity.entityIndex] = entity };
            Assert.That(ModernStoneStatues.MatchesCurrent(current, view), Is.True);
            var stale = view; stale.sourceId++;
            Assert.That(ModernStoneStatues.MatchesCurrent(current, stale), Is.False);
            stale = view; stale.entityIndex++;
            Assert.That(ModernStoneStatues.MatchesCurrent(current, stale), Is.False);
        }

        [TestCase(false, false)]
        [TestCase(true, false)]
        [TestCase(true, true)]
        public void PresenterPreservesImportedTransformsWhenPlacingStone(bool voxel, bool blockSurface)
        {
            var parent = new GameObject("Stone placement test");
            Material material = null;
            try
            {
                var camera = new GameObject("Stone camera").AddComponent<Camera>();
                camera.transform.SetParent(parent.transform, false);
                camera.transform.position = new Vector3(310, 85, 890);
                var entity = new NativeSim.CampaignEntitySnapshot {
                    entityIndex = 53, sourceId = 53, active = 1, classId = 2, model = 1, action = 3
                };
                var view = new NativeSim.CampaignEntityPresentation {
                    entityIndex = 53, sourceId = 53, renderParameterIndex = 79,
                    baseTmap = 78, selectedTmap = 78, logicalHeight = 500,
                    x = 19328, y = 55936, z = 4575
                };
                var presenter = new ModernStoneStatues(parent.transform, voxel, "TMAPS1-0",
                    blockSurface ? (_, _) => 80f : null);
                presenter.Observe(3, 10688, new Dictionary<ushort, NativeSim.CampaignEntitySnapshot> {
                    [53] = entity
                }, new[] { view });
                Assert.That(presenter.Replaces(53), Is.True);
                presenter.Render(camera, true);
                var renderers = parent.GetComponentsInChildren<Renderer>();
                Assert.That(renderers.Length, Is.GreaterThan(0));
                material = renderers[0].sharedMaterial;
                var bounds = renderers[0].bounds;
                foreach (var renderer in renderers) bounds.Encapsulate(renderer.bounds);
                var height = view.logicalHeight / 64f;
                Assert.That(bounds.size.y, Is.InRange(height * .95f, height * 1.05f),
                    "runtime placement must preserve the prefab's imported axis and scale");
                Assert.That(bounds.min.y, Is.EqualTo(blockSurface ? 80f : view.z / 64f).Within(height * .03f));
                Assert.That(view.z, Is.EqualTo(4575), "visual grounding leaves the native pose unchanged");
                Assert.That(bounds.center.x, Is.EqualTo(view.x / 64f).Within(height * .1f));
                Assert.That(bounds.center.z, Is.EqualTo(view.y / 64f).Within(height * .1f));
            }
            finally
            {
                Object.DestroyImmediate(parent);
                if (material != null) Object.DestroyImmediate(material);
            }
        }

        [TestCase("ModernScenery/StoneStatue")]
        [TestCase("ModernScenery/StoneStatueVoxel")]
        public void ImportedStatueIsGroundedUprightAndBounded(string resource)
        {
            var prefab = Resources.Load<GameObject>(resource);
            Assert.That(prefab, Is.Not.Null, resource);
            var instance = Object.Instantiate(prefab);
            try
            {
                var renderers = instance.GetComponentsInChildren<MeshRenderer>();
                Assert.That(renderers.Length, Is.GreaterThan(0));
                var bounds = renderers[0].bounds;
                foreach (var renderer in renderers) bounds.Encapsulate(renderer.bounds);
                Assert.That(bounds.min.y, Is.EqualTo(0).Within(.03));
                Assert.That(bounds.size.y, Is.InRange(.95f, 1.05f));
                Assert.That(bounds.size.x / bounds.size.y, Is.InRange(.32f, .5f));
                Assert.That(bounds.size.z, Is.InRange(.20f, .35f));
                var triangles = 0;
                foreach (var filter in instance.GetComponentsInChildren<MeshFilter>())
                    triangles += filter.sharedMesh.triangles.Length / 3;
                Assert.That(triangles, Is.InRange(100, 30000));
                CaptureIfRequested(instance, resource);
            }
            finally { Object.DestroyImmediate(instance); }
        }

        private static void CaptureIfRequested(GameObject instance, string resource)
        {
            var directory = System.Environment.GetEnvironmentVariable("MC_STONE_PREVIEW_DIR");
            if (string.IsNullOrEmpty(directory)) return;
            Assert.That(SystemInfo.graphicsDeviceType,
                Is.Not.EqualTo(UnityEngine.Rendering.GraphicsDeviceType.Null));
            var root = new GameObject("Stone material review");
            var target = new RenderTexture(480, 720, 24, RenderTextureFormat.ARGB32);
            var pixels = new Texture2D(480, 720, TextureFormat.RGB24, false);
            var material = ModernStoneStatues.CreateMaterial();
            var previousTarget = RenderTexture.active;
            var previousAmbient = RenderSettings.ambientLight;
            var previousAmbientMode = RenderSettings.ambientMode;
            var previousPipeline = GraphicsSettings.defaultRenderPipeline;
            var previousQuality = QualitySettings.renderPipeline;
            try
            {
                var pipeline = Resources.Load<UniversalRenderPipelineAsset>("ModernPipeline");
                Assert.That(pipeline, Is.Not.Null);
                GraphicsSettings.defaultRenderPipeline = pipeline;
                QualitySettings.renderPipeline = null;
                instance.transform.SetParent(root.transform, false);
                foreach (var renderer in instance.GetComponentsInChildren<Renderer>()) renderer.sharedMaterial = material;
                var camera = new GameObject("Review camera").AddComponent<Camera>();
                camera.transform.SetParent(root.transform, false);
                camera.transform.position = new Vector3(.25f, .65f, 3.5f);
                camera.transform.LookAt(new Vector3(0, .5f, 0));
                camera.orthographic = true;
                camera.orthographicSize = .62f;
                camera.clearFlags = CameraClearFlags.SolidColor;
                camera.backgroundColor = new Color(.12f, .12f, .12f);
                camera.targetTexture = target;
                var light = new GameObject("Review light").AddComponent<Light>();
                light.transform.SetParent(root.transform, false);
                light.type = LightType.Directional;
                light.transform.rotation = Quaternion.Euler(35, 155, 0);
                light.intensity = 1;
                RenderSettings.ambientMode = AmbientMode.Flat;
                RenderSettings.ambientLight = new Color(.28f, .28f, .3f);
                // The first render initializes URP and its lighting resources.
                camera.Render();
                camera.Render();
                RenderTexture.active = target;
                pixels.ReadPixels(new Rect(0, 0, 480, 720), 0, 0);
                pixels.Apply();
                var image = pixels.GetPixels32();
                var background = image[0];
                var changed = 0;
                var errorPixels = 0;
                var litPixels = 0;
                foreach (var pixel in image)
                {
                    if (Mathf.Abs(pixel.r - background.r) + Mathf.Abs(pixel.g - background.g) +
                        Mathf.Abs(pixel.b - background.b) > 12) changed++;
                    if (pixel.r > 200 && pixel.b > 200 && pixel.g < 50) errorPixels++;
                    if (pixel.r + pixel.g + pixel.b > background.r + background.g + background.b + 60) litPixels++;
                }
                Directory.CreateDirectory(directory);
                var path = Path.Combine(directory, Path.GetFileName(resource) + ".png");
                var encoded = pixels.EncodeToPNG();
                using (var output = new FileStream(path, FileMode.CreateNew, FileAccess.Write))
                    output.Write(encoded, 0, encoded.Length);
                Assert.That(changed, Is.GreaterThan(1000), "review render contains visible geometry");
                Assert.That(errorPixels, Is.LessThan(10), "review render must not contain the magenta shader error");
                Assert.That(litPixels, Is.GreaterThan(5000), "review render contains visibly lit stone");
            }
            finally
            {
                RenderTexture.active = previousTarget;
                RenderSettings.ambientLight = previousAmbient;
                RenderSettings.ambientMode = previousAmbientMode;
                GraphicsSettings.defaultRenderPipeline = previousPipeline;
                QualitySettings.renderPipeline = previousQuality;
                instance.transform.SetParent(null, false);
                Object.DestroyImmediate(root);
                target.Release();
                Object.DestroyImmediate(target);
                Object.DestroyImmediate(pixels);
                Object.DestroyImmediate(material);
            }
        }
    }
}
