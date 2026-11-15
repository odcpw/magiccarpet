using System;
using System.IO;
using System.Linq;
using NUnit.Framework;
using UnityEngine;
using UnityEngine.Rendering;
using UnityEngine.Rendering.Universal;

namespace MagicCarpet.Native.Tests
{
    public sealed class ModernExitPortalTests
    {
        private static NativeSim.CampaignEntityPresentation Row(ushort slot = 31) =>
            new NativeSim.CampaignEntityPresentation {
                generation = 4, entityIndex = slot, sourceId = 77,
                renderParameterIndex = 338, baseTmap = 461, selectedTmap = 461,
                logicalHeight = 700, x = 640, y = 1280, z = 192
            };

        [TestCase(false)]
        [TestCase(true)]
        public void SourceRowsControlAdmissionRemovalAndReuse(bool voxel)
        {
            var parent = new GameObject("portal test");
            var camera = new GameObject("portal camera").AddComponent<Camera>();
            try
            {
                using var portal = new ModernExitPortal(parent.transform, voxel);
                var row = Row();
                Assert.That(ModernExitPortal.Matches(row), Is.True);
                var other = row; other.renderParameterIndex++;
                Assert.That(ModernExitPortal.Matches(other), Is.False);
                portal.Present(new[] { other });
                Assert.That(portal.Replaces(row.entityIndex), Is.False);
                Assert.That(parent.transform.childCount, Is.Zero);
                portal.Present(new[] { row });
                Assert.That(portal.Replaces(row.entityIndex), Is.True);
                Assert.That(parent.transform.childCount, Is.EqualTo(1));
                var first = parent.transform.GetChild(0);
                portal.Render(camera, true);
                Assert.That(first.position, Is.EqualTo(new Vector3(10, 3, 20)));
                Assert.That(first.localScale.x, Is.EqualTo(700f / 64f).Within(.0001f));
                var renderers = first.GetComponentsInChildren<MeshRenderer>();
                Assert.That(renderers.Length, Is.GreaterThan(0));
                var bounds = renderers[0].bounds;
                foreach (var renderer in renderers.Skip(1)) bounds.Encapsulate(renderer.bounds);
                var height = row.logicalHeight / 64f;
                var triangles = first.GetComponentsInChildren<MeshFilter>()
                    .Sum(x => x.sharedMesh.triangles.Length / 3);
                TestContext.WriteLine($"voxel={voxel} bounds={bounds} triangles={triangles} renderers={renderers.Length}");
                Assert.That(bounds.size.y, Is.EqualTo(height).Within(height * .001f));
                Assert.That(bounds.min.y, Is.EqualTo(row.z / 64f).Within(height * .001f));
                Assert.That(bounds.size.x / height, Is.InRange(.65f, .85f));
                Assert.That(triangles, Is.LessThan(voxel ? 100000 : 15000));
                Assert.That(renderers.Length, Is.LessThan(32), "Voxel cells must remain combined meshes");
                Assert.That(renderers.All(x => x.sharedMaterial.shader.name == "Universal Render Pipeline/Lit"), Is.True);
                var eyes = renderers.Where(x => x.name.Contains("Red eye")).ToArray();
                var cranium = renderers.Single(x => x.name.Contains("Cranium"));
                Assert.That(eyes.Length, Is.EqualTo(2));
                TestContext.WriteLine($"cranium={cranium.bounds.center} eyes={string.Join(", ", eyes.Select(x => x.bounds.center.ToString()))}");
                Assert.That(eyes.All(x => x.bounds.center.z < cranium.bounds.center.z), Is.True,
                    "eyes face the camera along the billboard's -Z side");
                CapturePreviewIfRequested(camera, first, height, voxel);
                portal.Render(camera, false);
                Assert.That(renderers.All(x => !x.enabled), Is.True);
                camera.transform.rotation = Quaternion.Euler(0, 90, 0);
                portal.Render(camera, true);
                Assert.That(renderers.All(x => x.enabled), Is.True);
                Assert.That(Quaternion.Angle(first.rotation, camera.transform.rotation), Is.LessThan(.01f));
                row.sourceId++;
                portal.Present(new[] { row });
                Assert.That(parent.transform.childCount, Is.EqualTo(1));
                Assert.That(parent.transform.GetChild(0), Is.Not.SameAs(first));
                row.generation++;
                portal.Present(new[] { row });
                Assert.That(parent.transform.childCount, Is.EqualTo(1));
                portal.Present(Array.Empty<NativeSim.CampaignEntityPresentation>());
                Assert.That(portal.Replaces(row.entityIndex), Is.False);
                Assert.That(parent.transform.childCount, Is.Zero);
            }
            finally
            {
                UnityEngine.Object.DestroyImmediate(camera.gameObject);
                UnityEngine.Object.DestroyImmediate(parent);
            }
        }

        // Opt-in review of the actual imported presenter geometry. The side
        // view holds the billboard rotation fixed to expose its physical depth.
        private static void CapturePreviewIfRequested(Camera camera, Transform node, float height, bool voxel)
        {
            var directory = Environment.GetEnvironmentVariable("MC_PORTAL_PREVIEW_DIR");
            if (string.IsNullOrEmpty(directory)) return;
            Assert.That(SystemInfo.graphicsDeviceType, Is.Not.EqualTo(GraphicsDeviceType.Null));
            var previousPipeline = GraphicsSettings.defaultRenderPipeline;
            var previousQuality = QualitySettings.renderPipeline;
            var previousAmbient = RenderSettings.ambientLight;
            var previousMode = RenderSettings.ambientMode;
            var previousSun = RenderSettings.sun;
            var previousTarget = RenderTexture.active;
            var target = new RenderTexture(640, 640, 24, RenderTextureFormat.ARGB32);
            var pixels = new Texture2D(640, 640, TextureFormat.RGB24, false);
            var lightObject = new GameObject("Portal preview light");
            try
            {
                var pipeline = Resources.Load<UniversalRenderPipelineAsset>("ModernPipeline");
                Assert.That(pipeline, Is.Not.Null);
                GraphicsSettings.defaultRenderPipeline = pipeline;
                QualitySettings.renderPipeline = null;
                foreach (var child in node.GetComponentsInChildren<Transform>(true)) child.gameObject.layer = 31;
                var light = lightObject.AddComponent<Light>();
                light.type = LightType.Directional;
                light.cullingMask = 1 << 31;
                light.transform.rotation = Quaternion.Euler(35, 155, 0);
                RenderSettings.sun = light;
                RenderSettings.ambientMode = AmbientMode.Flat;
                camera.cullingMask = 1 << 31;
                camera.clearFlags = CameraClearFlags.SolidColor;
                camera.backgroundColor = new Color(.08f, .09f, .10f);
                camera.orthographic = true;
                camera.orthographicSize = height * .65f;
                camera.targetTexture = target;
                var center = node.position + Vector3.up * (height * .5f);
                Directory.CreateDirectory(directory);
                foreach (var view in new[] { "front", "side", "night" })
                {
                    var night = view == "night";
                    RenderSettings.ambientLight = night ? new Color(.12f, .16f, .20f) : new Color(.28f, .28f, .30f);
                    light.color = night ? new Color(.76f, .86f, 1f) : Color.white;
                    light.intensity = night ? .8f : 1f;
                    camera.transform.position = center + (view == "side" ? new Vector3(height * 1.5f, height * .15f, -height * 2.5f) : new Vector3(0, 0, -height * 3));
                    camera.transform.LookAt(center);
                    camera.Render(); camera.Render();
                    RenderTexture.active = target;
                    pixels.ReadPixels(new Rect(0, 0, 640, 640), 0, 0);
                    pixels.Apply();
                    var path = Path.Combine(directory, (voxel ? "voxel-" : "modern-") + view + ".png");
                    using var output = new FileStream(path, FileMode.CreateNew, FileAccess.Write);
                    var bytes = pixels.EncodeToPNG();
                    output.Write(bytes, 0, bytes.Length);
                    var image = pixels.GetPixels32();
                    Assert.That(image.Count(p => p.r > 200 && p.b > 200 && p.g < 50), Is.LessThan(10),
                        "preview must not contain the magenta shader error");
                    if (view != "side")
                    {
                        Assert.That(image.Count(p => p.r > 100 && p.r > p.g * 2 && p.r > p.b * 2), Is.GreaterThan(20),
                            "front view must show the red eyes");
                        // Interior crop avoids the stone rim and the preview
                        // background. The source has a broad black mouth.
                        var black = 0;
                        for (var y = 140; y < 370; y++)
                            for (var x = 240; x < 400; x++)
                            {
                                var p = image[y * 640 + x];
                                if (p.r < 13 && p.g < 13 && p.b < 13) black++;
                            }
                        TestContext.WriteLine($"voxel={voxel} view={view} mouth-black={black}/36800");
                        Assert.That(black, Is.GreaterThan(31280), "the mouth backing must cover the broad opening");
                    }
                }
            }
            finally
            {
                camera.targetTexture = null;
                RenderTexture.active = previousTarget;
                RenderSettings.ambientLight = previousAmbient;
                RenderSettings.ambientMode = previousMode;
                RenderSettings.sun = previousSun;
                GraphicsSettings.defaultRenderPipeline = previousPipeline;
                QualitySettings.renderPipeline = previousQuality;
                target.Release();
                UnityEngine.Object.DestroyImmediate(target);
                UnityEngine.Object.DestroyImmediate(pixels);
                UnityEngine.Object.DestroyImmediate(lightObject);
            }
        }
    }
}
