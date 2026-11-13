using System;
using System.Collections.Generic;
using System.IO;
using System.Linq;
using NUnit.Framework;
using UnityEngine;
using UnityEngine.Rendering;
using UnityEngine.Rendering.Universal;

namespace MagicCarpet.Native.Tests
{
    public sealed class ModernExperienceScrollTests
    {
        private static NativeSim.CampaignEntityPresentation View() => new NativeSim.CampaignEntityPresentation {
            generation = 4, entityIndex = 31, sourceId = 77, renderParameterIndex = 280,
            baseTmap = 259, selectedTmap = 259, logicalHeight = 75, renderMode = 1,
            x = 640, y = 1280, z = 192
        };
        private static NativeSim.CampaignEntitySnapshot Entity() => new NativeSim.CampaignEntitySnapshot {
            active = 1, entityIndex = 31, sourceId = 77, classId = 14, model = 5, action = 10
        };

        [TestCase(false)]
        [TestCase(true)]
        public void SourceOccupantControlsAdmissionRemovalAndSlotReuse(bool voxel)
        {
            var parent = new GameObject("scroll test");
            var camera = new GameObject("scroll camera").AddComponent<Camera>();
            try
            {
                using var presenter = new ModernExperienceScroll(parent.transform, voxel);
                var view = View(); var entity = Entity();
                var entities = new Dictionary<ushort, NativeSim.CampaignEntitySnapshot> { [31] = entity };
                Assert.That(ModernExperienceScroll.Matches(entity, view), Is.True);
                var other = entity; other.model = 6;
                Assert.That(ModernExperienceScroll.Matches(other, view), Is.False);
                other = entity; other.active = 0;
                Assert.That(ModernExperienceScroll.Matches(other, view), Is.False);
                var wrong = view; wrong.selectedTmap++;
                Assert.That(ModernExperienceScroll.Matches(entity, wrong), Is.False);
                wrong = view; wrong.logicalHeight++;
                Assert.That(ModernExperienceScroll.Matches(entity, wrong), Is.False);
                presenter.Present(entities, new[] { view });
                Assert.That(presenter.Replaces(31), Is.True);
                Assert.That(parent.transform.childCount, Is.EqualTo(1));
                var first = parent.transform.GetChild(0);
                presenter.Render(camera, true);
                Assert.That(first.position, Is.EqualTo(new Vector3(10, 3, 20)));
                Assert.That(first.localScale.x, Is.EqualTo(75f / 64f).Within(.0001f));
                var renderers = first.GetComponentsInChildren<MeshRenderer>();
                Assert.That(renderers.Length, Is.GreaterThan(0));
                var materials = renderers.SelectMany(r => r.sharedMaterials).Select(m => m.name).Distinct().ToArray();
                Assert.That(materials, Does.Contain("Scroll parchment"));
                Assert.That(materials, Does.Contain("Scroll parchment overlay"));
                Assert.That(materials, Does.Contain("Scroll parchment lip"));
                Assert.That(materials, Does.Contain("Scroll rolled edge"));
                Assert.That(materials, Does.Contain("Scroll violet binding"));
                var paper = renderers.SelectMany(r => r.sharedMaterials)
                    .First(m => m.name == "Scroll parchment");
                Assert.That(paper.GetColor("_BaseColor").r, Is.GreaterThan(.8f));
                Assert.That(paper.GetTexture("_BaseMap").name,
                    Is.EqualTo(voxel ? "ExperienceScrollAlbedo" : "ExperienceScrollPaper"));
                var bounds = renderers[0].bounds;
                foreach (var renderer in renderers.Skip(1)) bounds.Encapsulate(renderer.bounds);
                Assert.That(bounds.size.x / bounds.size.y, Is.InRange(2.3f, 3.2f));
                Assert.That(bounds.size.y, Is.InRange(.8f, 1.4f));
                Assert.That(Quaternion.Angle(first.rotation,
                    Quaternion.Euler(0, ModernExperienceScroll.FacingOffsetDegrees, 0)), Is.LessThan(.01f));
                if (!voxel)
                {
                    var ends = renderers.Where(r => r.sharedMaterials.Any(m => m.name == "Scroll rolled edge"))
                        .OrderBy(r => first.InverseTransformPoint(r.bounds.center).x).ToArray();
                    Assert.That(ends.Length, Is.EqualTo(2));
                    Assert.That(first.InverseTransformPoint(ends[0].bounds.center).x, Is.LessThan(-1f));
                    Assert.That(first.InverseTransformPoint(ends[1].bounds.center).x, Is.GreaterThan(1f));
                    Assert.That(camera.transform.InverseTransformPoint(ends[1].bounds.center).z,
                        Is.LessThan(camera.transform.InverseTransformPoint(ends[0].bounds.center).z));
                }
                camera.transform.rotation = Quaternion.Euler(0, 37, 0);
                presenter.Render(camera, true);
                Assert.That(Quaternion.Angle(first.rotation,
                    Quaternion.Euler(0, 37 + ModernExperienceScroll.FacingOffsetDegrees, 0)), Is.LessThan(.01f));
                CaptureNightPreviewIfRequested(presenter, camera, parent.transform, voxel);
                // The next native presentation omits the collected occupant.
                presenter.Present(entities, Array.Empty<NativeSim.CampaignEntityPresentation>());
                Assert.That(presenter.Replaces(31), Is.False);
                Assert.That(parent.transform.childCount, Is.Zero);
                presenter.Present(entities, new[] { view });
                first = parent.transform.GetChild(0);
                view.sourceId++; entity.sourceId++; entities[31] = entity;
                presenter.Present(entities, new[] { view });
                Assert.That(parent.transform.childCount, Is.EqualTo(1));
                Assert.That(parent.transform.GetChild(0), Is.Not.SameAs(first));
                entities[31] = other;
                presenter.Present(entities, new[] { view });
                Assert.That(presenter.Replaces(31), Is.False);
                Assert.That(parent.transform.childCount, Is.Zero);
            }
            finally
            {
                UnityEngine.Object.DestroyImmediate(camera.gameObject);
                UnityEngine.Object.DestroyImmediate(parent);
            }
        }

        private static void CaptureNightPreviewIfRequested(ModernExperienceScroll presenter, Camera camera,
            Transform parent, bool voxel)
        {
            var directory = Environment.GetEnvironmentVariable("MC_SCROLL_NIGHT_PREVIEW_DIR");
            if (string.IsNullOrEmpty(directory)) return;
            Assert.That(SystemInfo.graphicsDeviceType, Is.Not.EqualTo(GraphicsDeviceType.Null));
            var previousPipeline = GraphicsSettings.defaultRenderPipeline;
            var previousQuality = QualitySettings.renderPipeline;
            var previousMode = RenderSettings.ambientMode;
            var previousSky = RenderSettings.ambientSkyColor;
            var previousEquator = RenderSettings.ambientEquatorColor;
            var previousGround = RenderSettings.ambientGroundColor;
            var previousSun = RenderSettings.sun;
            var previousFog = RenderSettings.fog;
            var previousFogMode = RenderSettings.fogMode;
            var previousFogColor = RenderSettings.fogColor;
            var previousFogDensity = RenderSettings.fogDensity;
            var previousTarget = RenderTexture.active;
            var target = new RenderTexture(960, 720, 24, RenderTextureFormat.ARGB32);
            var pixels = new Texture2D(960, 720, TextureFormat.RGB24, false);
            var moonObject = new GameObject("Scroll review moon");
            try
            {
                var pipeline = Resources.Load<UniversalRenderPipelineAsset>("ModernPipeline");
                Assert.That(pipeline, Is.Not.Null);
                GraphicsSettings.defaultRenderPipeline = pipeline;
                QualitySettings.renderPipeline = null;
                RenderSettings.ambientMode = AmbientMode.Trilight;
                RenderSettings.ambientSkyColor = new Color(.18f, .24f, .34f);
                RenderSettings.ambientEquatorColor = new Color(.12f, .16f, .20f);
                RenderSettings.ambientGroundColor = new Color(.07f, .09f, .085f);
                RenderSettings.fog = true;
                RenderSettings.fogMode = FogMode.ExponentialSquared;
                RenderSettings.fogColor = new Color(.24f, .29f, .32f);
                RenderSettings.fogDensity = .0038f;
                foreach (var node in parent.GetComponentsInChildren<Transform>(true)) node.gameObject.layer = 31;
                var moon = moonObject.AddComponent<Light>();
                moon.type = LightType.Directional;
                moon.cullingMask = 1 << 31;
                moon.color = new Color(.76f, .86f, 1f);
                moon.intensity = .8f;
                moon.transform.rotation = Quaternion.LookRotation(-new Vector3(-.38f, .48f, .79f).normalized);
                RenderSettings.sun = moon;
                camera.transform.position = new Vector3(14, 7.58f, 28);
                camera.transform.LookAt(new Vector3(10, 3.58f, 20));
                camera.fieldOfView = 48;
                camera.cullingMask = 1 << 31;
                camera.allowHDR = true;
                camera.clearFlags = CameraClearFlags.SolidColor;
                camera.backgroundColor = new Color(.04f, .05f, .06f);
                camera.targetTexture = target;
                presenter.Render(camera, true);
                camera.Render(); camera.Render();
                RenderTexture.active = target;
                pixels.ReadPixels(new Rect(0, 0, 960, 720), 0, 0);
                pixels.Apply();
                var image = pixels.GetPixels32();
                var background = image[0];
                var changed = 0; var errors = 0;
                foreach (var pixel in image)
                {
                    if (Mathf.Abs(pixel.r - background.r) + Mathf.Abs(pixel.g - background.g) +
                        Mathf.Abs(pixel.b - background.b) > 12) changed++;
                    if (pixel.r > 200 && pixel.b > 200 && pixel.g < 50) errors++;
                }
                Assert.That(changed, Is.GreaterThan(1000), "night review contains visible geometry");
                Assert.That(errors, Is.LessThan(10), "night review contains no shader error");
                if (voxel)
                {
                    // Fixed close-side crop of the paper body. The original
                    // axis-normal voxel surface measured >330 here, while the
                    // same cells with source-shaped normals measure ~27.
                    var means = new float[41];
                    for (var y = 310; y <= 350; y++)
                    {
                        var total = 0f;
                        for (var x = 375; x < 435; x++)
                        {
                            var p = image[(719 - y) * 960 + x];
                            total += (p.r + p.g + p.b) / 3f;
                        }
                        means[y - 310] = total / 60f;
                    }
                    var oscillation = 0f;
                    for (var i = 1; i < means.Length - 1; i++)
                        oscillation += Mathf.Abs(means[i + 1] - 2 * means[i] + means[i - 1]);
                    Assert.That(oscillation, Is.LessThan(120f), "voxel paper must not form high-contrast horizontal bands");
                }
                Directory.CreateDirectory(directory);
                var path = Path.Combine(directory, voxel ? "voxel-night.png" : "modern-night.png");
                using (var output = new FileStream(path, FileMode.CreateNew, FileAccess.Write))
                {
                    var data = pixels.EncodeToPNG();
                    output.Write(data, 0, data.Length);
                }
                if (!voxel)
                {
                    // A second isolated view removes direct moonlight to
                    // expose material readability in a cast-shadow case.
                    // It is a preview, not a replay of the level's lighting.
                    moon.enabled = false;
                    camera.Render(); camera.Render();
                    pixels.ReadPixels(new Rect(0, 0, 960, 720), 0, 0);
                    pixels.Apply();
                    using var shadow = new FileStream(Path.Combine(directory, "modern-shadow.png"),
                        FileMode.CreateNew, FileAccess.Write);
                    var data = pixels.EncodeToPNG();
                    shadow.Write(data, 0, data.Length);
                }
            }
            finally
            {
                camera.targetTexture = null;
                RenderTexture.active = previousTarget;
                RenderSettings.sun = previousSun;
                RenderSettings.ambientMode = previousMode;
                RenderSettings.ambientSkyColor = previousSky;
                RenderSettings.ambientEquatorColor = previousEquator;
                RenderSettings.ambientGroundColor = previousGround;
                RenderSettings.fog = previousFog;
                RenderSettings.fogMode = previousFogMode;
                RenderSettings.fogColor = previousFogColor;
                RenderSettings.fogDensity = previousFogDensity;
                GraphicsSettings.defaultRenderPipeline = previousPipeline;
                QualitySettings.renderPipeline = previousQuality;
                target.Release();
                UnityEngine.Object.DestroyImmediate(target);
                UnityEngine.Object.DestroyImmediate(pixels);
                UnityEngine.Object.DestroyImmediate(moonObject);
            }
        }
    }
}
