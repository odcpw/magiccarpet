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
    public sealed class ModernCastleProjectileTests
    {
        private static NativeSim.CampaignEntityPresentation Row() => new NativeSim.CampaignEntityPresentation {
            generation = 4, entityIndex = 31, sourceId = 77, renderParameterIndex = 18,
            baseTmap = 61, selectedTmap = 61, logicalHeight = 150, renderMode = 21, x = 640, y = 1280, z = 192
        };
        private static NativeSim.CampaignEntitySnapshot Entity() => new NativeSim.CampaignEntitySnapshot {
            active = 1, entityIndex = 31, sourceId = 77, classId = 9, model = 10, action = 10
        };

        [TestCase(false)]
        [TestCase(true)]
        public void NativeProjectileOwnsHeadVisibilitySizeFacingAndReuse(bool voxel)
        {
            var parent = new GameObject("castle projectile test");
            var camera = new GameObject("castle projectile camera").AddComponent<Camera>();
            try
            {
                using var presenter = new ModernCastleProjectile(parent.transform, voxel);
                var row = Row(); var entity = Entity();
                var entities = new Dictionary<ushort, NativeSim.CampaignEntitySnapshot> { [31] = entity };
                Assert.That(ModernCastleProjectile.Matches(entity, row), Is.True);
                var wrong = entity; wrong.model = 0;
                Assert.That(ModernCastleProjectile.Matches(wrong, row), Is.False);
                wrong = entity; wrong.action++;
                Assert.That(ModernCastleProjectile.Matches(wrong, row), Is.False);
                var wrongRow = row; wrongRow.baseTmap++;
                Assert.That(ModernCastleProjectile.Matches(entity, wrongRow), Is.False);
                wrong = entity; wrong.sourceId = 0; wrongRow = row; wrongRow.sourceId = 0;
                Assert.That(ModernCastleProjectile.Matches(wrong, wrongRow), Is.True, "a zero source ID is not a visibility decision");
                wrongRow = row; wrongRow.controlFlags = 1;
                Assert.That(ModernCastleProjectile.Matches(entity, wrongRow), Is.False);
                presenter.Present(entities, new[] { row }); presenter.Render(camera, true);
                Assert.That(presenter.Replaces(31), Is.True);
                Assert.That(presenter.ActiveCount, Is.EqualTo(1));
                var node = parent.transform.GetChild(0);
                Assert.That(node.position, Is.EqualTo(new Vector3(10, 3, 20)));
                var renderers = node.GetComponentsInChildren<Renderer>();
                Assert.That(renderers.Length, Is.EqualTo(6));
                Assert.That(renderers.All(r => r.sharedMaterial.shader.name == "Universal Render Pipeline/Lit"), Is.True);
                var wrap = renderers.Single(r => r.name.Contains("Wrap"));
                var face = renderers.Single(r => r.name.Contains("Face"));
                Assert.That(renderers.Any(r => r.name.Contains("Beard")), Is.True);
                Assert.That(renderers.Any(r => r.name.Contains("Pupils")), Is.True);
                Assert.That(wrap.bounds.center.y, Is.GreaterThan(face.bounds.center.y));
                var tint = wrap.sharedMaterial.GetColor("_BaseColor");
                Assert.That(tint.g, Is.GreaterThan(tint.r)); Assert.That(tint.b, Is.GreaterThan(tint.r));
                var bounds = renderers[0].bounds; foreach (var renderer in renderers.Skip(1)) bounds.Encapsulate(renderer.bounds);
                var height = row.logicalHeight / 64f;
                Assert.That(bounds.size.y, Is.EqualTo(height).Within(height * .001f));
                Assert.That(bounds.min.y, Is.EqualTo(3).Within(height * .001f));
                Assert.That(bounds.size.x, Is.GreaterThan(height * .35f));
                Assert.That(bounds.size.z, Is.GreaterThan(height * .2f));
                var triangles = node.GetComponentsInChildren<MeshFilter>().Sum(f => f.sharedMesh.triangles.Length / 3);
                TestContext.WriteLine($"castle projectile voxel={voxel} bounds={bounds} triangles={triangles}");
                CaptureIfRequested(camera, node, height, voxel);
                Assert.That(triangles, Is.InRange(200, voxel ? 90000 : 15000));
                presenter.Render(camera, false); Assert.That(renderers.All(r => !r.enabled), Is.True);
                camera.transform.rotation = Quaternion.Euler(0, 90, 0);
                presenter.Render(camera, true);
                Assert.That(Quaternion.Angle(node.rotation, camera.transform.rotation), Is.LessThan(.01f));
                Assert.That(renderers.All(r => r.enabled), Is.True);
                row.logicalHeight = 100; row.x = 704; row.z = 256;
                presenter.Present(entities, new[] { row }); presenter.Render(camera, true);
                Assert.That(node.localScale.x, Is.EqualTo(100 / 64f).Within(.001f));
                Assert.That(node.position, Is.EqualTo(new Vector3(11, 4, 20)));
                presenter.Present(entities, Array.Empty<NativeSim.CampaignEntityPresentation>());
                Assert.That(presenter.Replaces(31), Is.False); Assert.That(presenter.PoolCount, Is.EqualTo(1));
                Assert.That(node.gameObject.activeSelf, Is.False);
                row.sourceId++; row.generation++; entity.sourceId++; entities[31] = entity;
                presenter.Present(entities, new[] { row }); presenter.Render(camera, true);
                Assert.That(parent.transform.GetChild(0), Is.SameAs(node));
                Assert.That(node.gameObject.activeSelf, Is.True);
                Assert.That(node.localScale.x, Is.EqualTo(100 / 64f).Within(.001f));
                entities[31] = wrong;
                presenter.Present(entities, new[] { row });
                Assert.That(presenter.Replaces(31), Is.False);
            }
            finally { UnityEngine.Object.DestroyImmediate(camera.gameObject); UnityEngine.Object.DestroyImmediate(parent); }
        }

        private static void CaptureIfRequested(Camera camera, Transform node, float height, bool voxel)
        {
            var directory = Environment.GetEnvironmentVariable("MC_CASTLE_PROJECTILE_PREVIEW_DIR");
            if (string.IsNullOrEmpty(directory)) return;
            Assert.That(SystemInfo.graphicsDeviceType, Is.Not.EqualTo(GraphicsDeviceType.Null));
            var previousPipeline = GraphicsSettings.defaultRenderPipeline; var previousQuality = QualitySettings.renderPipeline;
            var previousAmbient = RenderSettings.ambientLight; var previousMode = RenderSettings.ambientMode;
            var previousSun = RenderSettings.sun; var previousTarget = RenderTexture.active;
            var target = new RenderTexture(640, 640, 24, RenderTextureFormat.ARGB32);
            var pixels = new Texture2D(640, 640, TextureFormat.RGB24, false);
            var lightObject = new GameObject("Castle projectile review light");
            try
            {
                var pipeline = Resources.Load<UniversalRenderPipelineAsset>("ModernPipeline"); Assert.That(pipeline, Is.Not.Null);
                GraphicsSettings.defaultRenderPipeline = pipeline; QualitySettings.renderPipeline = null;
                foreach (var child in node.GetComponentsInChildren<Transform>(true)) child.gameObject.layer = 31;
                var light = lightObject.AddComponent<Light>(); light.type = LightType.Directional; light.cullingMask = 1 << 31;
                light.transform.rotation = Quaternion.Euler(35, 155, 0); RenderSettings.sun = light;
                RenderSettings.ambientMode = AmbientMode.Flat; camera.cullingMask = 1 << 31;
                camera.clearFlags = CameraClearFlags.SolidColor; camera.backgroundColor = new Color(.08f, .09f, .10f);
                camera.orthographic = true; camera.orthographicSize = height * .65f; camera.targetTexture = target;
                var center = node.position + Vector3.up * (height * .5f); Directory.CreateDirectory(directory);
                foreach (var view in new[] { "front", "side", "night" })
                {
                    bool night = view == "night";
                    RenderSettings.ambientLight = night ? new Color(.12f, .16f, .20f) : new Color(.28f, .28f, .30f);
                    light.color = night ? new Color(.76f, .86f, 1f) : Color.white; light.intensity = night ? .8f : 1f;
                    camera.transform.position = center + (view == "side" ? new Vector3(height * 2.5f, height * .15f, -height * .1f) : new Vector3(0, 0, -height * 3));
                    camera.transform.LookAt(center); camera.Render(); camera.Render(); RenderTexture.active = target;
                    pixels.ReadPixels(new Rect(0, 0, 640, 640), 0, 0); pixels.Apply();
                    var image = pixels.GetPixels32();
                    Assert.That(image.Count(p => p.r > 200 && p.b > 200 && p.g < 50), Is.LessThan(10), "no shader error");
                    Assert.That(image.Count(p => p.g > 30 && p.g > p.r * 1.15 && p.b > p.r * 1.1), Is.GreaterThan(100), "teal source-like wrap is visible");
                    using var output = new FileStream(Path.Combine(directory, (voxel ? "voxel-" : "modern-") + view + ".png"), FileMode.CreateNew, FileAccess.Write);
                    var bytes = pixels.EncodeToPNG(); output.Write(bytes, 0, bytes.Length);
                }
            }
            finally
            {
                camera.targetTexture = null; RenderTexture.active = previousTarget;
                RenderSettings.ambientLight = previousAmbient; RenderSettings.ambientMode = previousMode; RenderSettings.sun = previousSun;
                GraphicsSettings.defaultRenderPipeline = previousPipeline; QualitySettings.renderPipeline = previousQuality;
                target.Release(); UnityEngine.Object.DestroyImmediate(target); UnityEngine.Object.DestroyImmediate(pixels); UnityEngine.Object.DestroyImmediate(lightObject);
            }
        }
    }
}
