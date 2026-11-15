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
        public void TransparentSourceTailEmitsNothingAndSparseFramesFade()
        {
            var palette = new byte[768]; palette[3] = 255; palette[4] = 120;
            Assert.That(ModernEntityPresenter.EffectTint(new byte[16], palette), Is.EqualTo(Color.clear));
            var sparse = new byte[16]; sparse[7] = 1;
            var tint = ModernEntityPresenter.EffectTint(sparse, palette);
            Assert.That(tint.a, Is.EqualTo(3f/16));
            Assert.That(tint.r, Is.EqualTo(1));
            Assert.That(tint.g, Is.EqualTo(120f/255).Within(.0001));
            Array.Fill(sparse, (byte)1);
            Assert.That(ModernEntityPresenter.EffectTint(sparse, palette).a, Is.EqualTo(1));
        }

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
            Assert.That(ModernEntityPresenter.UsesSmokeMaterial(row), Is.True, "cold ash/splash uses the existing soft particle asset");
            row.renderParameterIndex = 340; row.baseTmap = 463;
            Assert.That(ModernEntityPresenter.HotEffectKind(row), Is.Zero, "fireball retains the native projectile mesh");
            Assert.That(ModernEntityPresenter.UsesSmokeMaterial(row), Is.False);
            row.renderParameterIndex = 7;
            Assert.That(ModernEntityPresenter.HotEffectKind(row), Is.Zero, "a parameter without its bound source image is insufficient");
            row.renderParameterIndex = 177; row.baseTmap = 96;
            Assert.That(ModernEntityPresenter.IsSourceHalo(row), Is.True);
            Assert.That(ModernEntityPresenter.HotEffectKind(row), Is.Zero, "source ownership orb must not become a flame");
            row.baseTmap = 97;
            Assert.That(ModernEntityPresenter.IsSourceHalo(row), Is.False);
        }

        [TestCase(false, false, false)]
        [TestCase(false, true, false)]
        [TestCase(true, false, false)]
        [TestCase(true, true, false)]
        [TestCase(false, false, true)]
        [TestCase(true, false, true)]
        public void SourceQuadRendersTransparentFlameOrImpactWithoutOpaqueRectangle(bool voxel, bool impact, bool halo)
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
                material.SetFloat("_Halo", halo ? 1 : 0);
                if (halo) material.SetColor("_Tint", new Color(.2f,.6f,1));
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
                using var output = new FileStream(Path.Combine(directory, $"{(voxel ? "voxel" : "modern")}-{(halo ? "halo" : impact ? "impact" : "fire")}.png"), FileMode.CreateNew);
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
