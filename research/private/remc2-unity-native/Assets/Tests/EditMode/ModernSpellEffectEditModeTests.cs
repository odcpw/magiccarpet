using System;
using System.IO;
using NUnit.Framework;
using UnityEngine;
using UnityEngine.Rendering;
using UnityEngine.Rendering.Universal;

namespace MagicCarpet.Native.Tests
{
    public sealed class ModernSpellEffectEditModeTests
    {
        [Test]
        public void HotSubstitutionRequiresBoundSourceAndPreservesColdSplashAndProjectileOwners()
        {
            var row = new NativeSim.CampaignEntityPresentation { renderParameterIndex = 7, baseTmap = 38 };
            Assert.That(ModernEntityPresenter.HotEffectKind(row), Is.EqualTo(2));
            row.renderParameterIndex = 41; row.baseTmap = 54;
            Assert.That(ModernEntityPresenter.HotEffectKind(row), Is.EqualTo(1));
            row.renderParameterIndex = 228; row.baseTmap = 77;
            Assert.That(ModernEntityPresenter.HotEffectKind(row), Is.EqualTo(1));
            row.renderParameterIndex = 244; row.baseTmap = 186;
            Assert.That(ModernEntityPresenter.HotEffectKind(row), Is.Zero, "cold splash must not turn into fire");
            row.renderParameterIndex = 340; row.baseTmap = 463;
            Assert.That(ModernEntityPresenter.HotEffectKind(row), Is.Zero, "fireball retains the native projectile mesh");
            row.renderParameterIndex = 7;
            Assert.That(ModernEntityPresenter.HotEffectKind(row), Is.Zero, "a parameter without its bound source image is insufficient");
        }

        [TestCase(false, false)]
        [TestCase(false, true)]
        [TestCase(true, false)]
        [TestCase(true, true)]
        public void SourceQuadRendersTransparentFlameOrImpactWithoutOpaqueRectangle(bool voxel, bool impact)
        {
            var directory = Environment.GetEnvironmentVariable("MC_EFFECT_PREVIEW_DIR");
            var shader = Resources.Load<Shader>("ModernSpellEffect");
            Assert.That(shader, Is.Not.Null);
            Assert.That(UnityEditor.ShaderUtil.ShaderHasError(shader), Is.False);
            if (string.IsNullOrEmpty(directory)) return;
            Assert.That(SystemInfo.graphicsDeviceType, Is.Not.EqualTo(GraphicsDeviceType.Null));
            var oldPipeline = GraphicsSettings.defaultRenderPipeline;
            var oldQuality = QualitySettings.renderPipeline;
            var oldTarget = RenderTexture.active;
            var cameraNode = new GameObject("Effect asset camera");
            var quad = new GameObject("Native effect quad");
            var mesh = new Mesh();
            var material = new Material(shader);
            var target = new RenderTexture(320, 320, 24);
            var pixels = new Texture2D(320, 320, TextureFormat.RGB24, false);
            try
            {
                GraphicsSettings.defaultRenderPipeline = Resources.Load<UniversalRenderPipelineAsset>("ModernPipeline");
                QualitySettings.renderPipeline = null;
                mesh.vertices = new[] { new Vector3(-.5f,0,0), new Vector3(.5f,0,0), new Vector3(-.5f,1,0), new Vector3(.5f,1,0) };
                mesh.uv = new[] { new Vector2(0,1), new Vector2(1,1), new Vector2(0,0), new Vector2(1,0) };
                mesh.triangles = new[] { 0,2,1,2,3,1 }; mesh.RecalculateBounds();
                quad.layer = 31;
                quad.AddComponent<MeshFilter>().sharedMesh = mesh;
                quad.AddComponent<MeshRenderer>().sharedMaterial = material;
                material.SetFloat("_Voxel", voxel ? 1 : 0);
                material.SetFloat("_Impact", impact ? 1 : 0);
                var camera = cameraNode.AddComponent<Camera>();
                camera.transform.position = new Vector3(0,.5f,-3);
                camera.orthographic = true; camera.orthographicSize = .65f;
                camera.cullingMask = 1 << 31;
                camera.clearFlags = CameraClearFlags.SolidColor;
                camera.backgroundColor = new Color(.015f,.025f,.04f);
                camera.targetTexture = target;
                camera.Render(); camera.Render();
                RenderTexture.active = target;
                pixels.ReadPixels(new Rect(0,0,320,320),0,0); pixels.Apply();
                var image = pixels.GetPixels32();
                int bright = 0, errors = 0;
                foreach (var pixel in image)
                {
                    if (pixel.r > 120 && pixel.g > 30) bright++;
                    if (pixel.r > 200 && pixel.b > 200 && pixel.g < 50) errors++;
                }
                Assert.That(bright, Is.GreaterThan(400));
                Assert.That(errors, Is.Zero);
                foreach (var corner in new[] { new Vector2Int(45,45), new Vector2Int(275,45), new Vector2Int(45,275), new Vector2Int(275,275) })
                {
                    var pixel = image[corner.y*320+corner.x];
                    Assert.That(Mathf.Abs(pixel.r-image[0].r)+Mathf.Abs(pixel.g-image[0].g)+Mathf.Abs(pixel.b-image[0].b), Is.LessThan(8), "source card corners remain transparent");
                }
                Directory.CreateDirectory(directory);
                using var output = new FileStream(Path.Combine(directory, $"{(voxel ? "voxel" : "modern")}-{(impact ? "impact" : "fire")}.png"), FileMode.CreateNew);
                var bytes = pixels.EncodeToPNG(); output.Write(bytes,0,bytes.Length);
            }
            finally
            {
                RenderTexture.active = oldTarget;
                GraphicsSettings.defaultRenderPipeline = oldPipeline;
                QualitySettings.renderPipeline = oldQuality;
                target.Release();
                UnityEngine.Object.DestroyImmediate(cameraNode);
                UnityEngine.Object.DestroyImmediate(quad);
                UnityEngine.Object.DestroyImmediate(mesh);
                UnityEngine.Object.DestroyImmediate(material);
                UnityEngine.Object.DestroyImmediate(target);
                UnityEngine.Object.DestroyImmediate(pixels);
            }
        }
    }
}
