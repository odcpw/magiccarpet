using System;
using System.IO;
using NUnit.Framework;
using UnityEngine;
using UnityEngine.Rendering;
using UnityEngine.Rendering.Universal;
using MagicCarpet.Native.Voxels;

namespace MagicCarpet.Native.Tests
{
    public sealed class VoxelNativeProjectilesEditModeTests
    {
        [TestCase(false)]
        [TestCase(true)]
        public void BothStylesUseSourceExtentAndReleaseHiddenOrRecycledLifetimes(bool voxel)
        {
            var parent = new GameObject("Fireball art check");
            var cameraNode = new GameObject("Fireball art camera");
            try
            {
                var camera = cameraNode.AddComponent<Camera>();
                using var presenter = new VoxelNativeProjectiles(parent.transform, voxel);
                var fire = new NativeSim.CampaignCharacterState {
                    entityIndex = 7, active = 1, classId = 9, model = 0,
                    x = 640, y = 1280, z = 192, slotGeneration = 1
                };
                var view = new NativeSim.CampaignEntityPresentation {
                    entityIndex = 7, x = 640, y = 1280, z = 192, logicalHeight = 128
                };
                presenter.Present(new[] { fire }, new[] { view }, 1);
                presenter.Render(camera);
                var visual = parent.transform.GetChild(0).GetChild(0);
                Assert.That(visual.position, Is.EqualTo(new Vector3(10, 4, 20)), "source bottom plus half native height");
                Assert.That(visual.localScale, Is.EqualTo(Vector3.one * 2));
                Assert.That(visual.GetComponent<Collider>(), Is.Null);
                Assert.That(visual.GetComponent<Rigidbody>(), Is.Null, "visual fireballs cannot bounce independently of Rust");
                var renderer = visual.GetComponent<MeshRenderer>();
                Assert.That(renderer.sharedMaterial.shader.name, Is.EqualTo(voxel ? "MagicCarpet/VoxelSurface" : "MagicCarpet/ModernFireball"));
                var triangles = visual.GetComponent<MeshFilter>().sharedMesh.triangles.Length / 3;
                Assert.That(triangles, Is.LessThan(voxel ? 10000 : 1000));
                CaptureIfRequested(camera, parent.transform, voxel);
                view.x += 64;
                presenter.Present(new[] { fire }, new[] { view }, 1);
                Assert.That(voxel || Quaternion.Angle(visual.rotation, Quaternion.LookRotation(Vector3.right)) < .001f, Is.True);
                presenter.Present(new[] { fire }, Array.Empty<NativeSim.CampaignEntityPresentation>(), 1);
                Assert.That(presenter.Replaces(7), Is.False);
                Assert.That(visual.gameObject.activeSelf, Is.False);
                fire.slotGeneration++;
                view.x = 1280;
                presenter.Present(new[] { fire }, new[] { view }, 1);
                presenter.Render(camera);
                Assert.That(visual.position, Is.EqualTo(new Vector3(20, 4, 20)), "reused slot must not interpolate from prior occupant");
                Assert.That(visual.rotation, Is.EqualTo(Quaternion.identity));
                fire.model = 13;
                presenter.Present(new[] { fire }, new[] { view }, 1);
                Assert.That(presenter.Replaces(7), Is.False, "arrows retain their own asset");
                fire.model = 28;
                presenter.Present(new[] { fire }, new[] { view }, 2);
                Assert.That(presenter.Replaces(7), Is.True);
                view.logicalHeight = 0;
                presenter.Present(new[] { fire }, new[] { view }, 2);
                Assert.That(presenter.Replaces(7), Is.False, "a zero-height source presentation cannot become an invented ball");
                presenter.Present(Array.Empty<NativeSim.CampaignCharacterState>(), new[] { view }, 3);
                Assert.That(presenter.Replaces(7), Is.False);
                Debug.Log($"MC_FIREBALL_ART style={(voxel ? "voxel" : "modern")} triangles={triangles} rigidbodies=0 colliders=0");
            }
            finally
            {
                UnityEngine.Object.DestroyImmediate(cameraNode);
                UnityEngine.Object.DestroyImmediate(parent);
            }
        }

        private static void CaptureIfRequested(Camera camera, Transform root, bool voxel)
        {
            var directory = Environment.GetEnvironmentVariable("MC_FIREBALL_PREVIEW_DIR");
            if (string.IsNullOrEmpty(directory)) return;
            Assert.That(SystemInfo.graphicsDeviceType, Is.Not.EqualTo(GraphicsDeviceType.Null));
            var oldPipeline = GraphicsSettings.defaultRenderPipeline;
            var oldQuality = QualitySettings.renderPipeline;
            var oldTarget = RenderTexture.active;
            var target = new RenderTexture(640, 480, 24);
            var pixels = new Texture2D(640, 480, TextureFormat.RGB24, false);
            try
            {
                GraphicsSettings.defaultRenderPipeline = Resources.Load<UniversalRenderPipelineAsset>("ModernPipeline");
                QualitySettings.renderPipeline = null;
                foreach (var node in root.GetComponentsInChildren<Transform>(true)) node.gameObject.layer = 31;
                camera.cullingMask = 1 << 31;
                camera.clearFlags = CameraClearFlags.SolidColor;
                camera.backgroundColor = new Color(.015f, .025f, .04f);
                camera.transform.position = new Vector3(13, 5, 14);
                camera.transform.LookAt(new Vector3(10, 4, 20));
                camera.fieldOfView = 36;
                camera.targetTexture = target;
                camera.Render(); camera.Render();
                RenderTexture.active = target;
                pixels.ReadPixels(new Rect(0, 0, 640, 480), 0, 0);
                pixels.Apply();
                int bright = 0, errors = 0;
                foreach (var p in pixels.GetPixels32())
                {
                    if (p.r > 180 && p.g > 70) bright++;
                    if (p.r > 200 && p.b > 200 && p.g < 50) errors++;
                }
                Assert.That(bright, Is.GreaterThan(1000), "fireball must remain luminous in the night view");
                Assert.That(errors, Is.Zero, "no shader-error pixels");
                Directory.CreateDirectory(directory);
                using var output = new FileStream(Path.Combine(directory, voxel ? "voxel-fireball.png" : "modern-fireball.png"), FileMode.CreateNew);
                var data = pixels.EncodeToPNG();
                output.Write(data, 0, data.Length);
            }
            finally
            {
                camera.targetTexture = null;
                RenderTexture.active = oldTarget;
                GraphicsSettings.defaultRenderPipeline = oldPipeline;
                QualitySettings.renderPipeline = oldQuality;
                target.Release();
                UnityEngine.Object.DestroyImmediate(target);
                UnityEngine.Object.DestroyImmediate(pixels);
            }
        }
    }
}
