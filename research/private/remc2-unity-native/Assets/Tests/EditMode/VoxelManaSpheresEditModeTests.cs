using System;
using System.Collections.Generic;
using System.IO;
using MagicCarpet.Native.Voxels;
using NUnit.Framework;
using UnityEngine;
using UnityEngine.Rendering;
using UnityEngine.Rendering.Universal;

namespace MagicCarpet.Native.Tests
{
    public sealed class VoxelManaSpheresEditModeTests
    {
        private static NativeSim.ManaSphereSnapshot Sphere(ushort slot, ushort tmap, ushort height, ushort x = 0, ushort y = 0, short z = 0)
            => new NativeSim.ManaSphereSnapshot { active = 1, sphereId = slot, sourceEntityIndex = slot,
                baseTmap = tmap, logicalHeight = height, x = x, y = y, z = z };
        private static NativeSim.CampaignEntityPresentation Visible(ushort slot)
            => new NativeSim.CampaignEntityPresentation { entityIndex = slot };
        private static Dictionary<int, Color> Colors() => new() { [58] = Color.white, [67] = Color.red };

        [Test]
        public void SphereMeshIsClosedVoxelSurfaceWithOutwardWindingAndBoundedGeometry()
        {
            var mesh = VoxelManaSpheres.BuildSphereMesh();
            try
            {
                Assert.That(mesh.vertexCount, Is.GreaterThan(1000));
                Assert.That(mesh.vertexCount, Is.LessThan(10000));
                Assert.That(mesh.bounds.size.x, Is.EqualTo(1).Within(.001));
                Assert.That(mesh.bounds.size.y, Is.EqualTo(1).Within(.001));
                Assert.That(mesh.bounds.size.z, Is.EqualTo(1).Within(.001));
                var vertices = mesh.vertices; var normals = mesh.normals; var triangles = mesh.triangles;
                for (var i = 0; i < triangles.Length; i += 3)
                {
                    var a = triangles[i]; var b = triangles[i + 1]; var c = triangles[i + 2];
                    Assert.That(Vector3.Dot(Vector3.Cross(vertices[b] - vertices[a], vertices[c] - vertices[a]), normals[a]), Is.GreaterThan(0));
                }
            }
            finally { UnityEngine.Object.DestroyImmediate(mesh); }
        }

        [Test]
        public void SmoothSphereHasUnitBoundsOutwardTrianglesAndUsableNormalsAndUvs()
        {
            var mesh = VoxelManaSpheres.BuildSmoothSphereMesh();
            try
            {
                Assert.That(mesh.triangles.Length / 3, Is.EqualTo(528));
                Assert.That(mesh.bounds.size.x, Is.EqualTo(1).Within(.001));
                Assert.That(mesh.bounds.size.y, Is.EqualTo(1).Within(.001));
                Assert.That(mesh.bounds.size.z, Is.EqualTo(1).Within(.001));
                var vertices = mesh.vertices; var normals = mesh.normals; var triangles = mesh.triangles;
                Assert.That(normals.Length, Is.EqualTo(vertices.Length));
                Assert.That(mesh.uv.Length, Is.EqualTo(vertices.Length));
                for (var i = 0; i < triangles.Length; i += 3)
                {
                    var a = triangles[i]; var b = triangles[i + 1]; var c = triangles[i + 2];
                    Assert.That(Vector3.Dot(Vector3.Cross(vertices[b] - vertices[a], vertices[c] - vertices[a]), normals[a]), Is.GreaterThan(0));
                }
            }
            finally { UnityEngine.Object.DestroyImmediate(mesh); }
        }

        [Test]
        public void IndexedSpriteColorUsesActualNontransparentPalettePixels()
        {
            var palette = new byte[768];
            palette[3] = 255; palette[4] = 20; palette[5] = 10;
            palette[6] = 30; palette[7] = 180; palette[8] = 20;
            var color = VoxelManaSpheres.ColorFromIndexedSprite(new byte[] { 0, 1, 1, 2 }, palette);
            Assert.That(color.r, Is.GreaterThan(color.g));
            Assert.That(color.g, Is.GreaterThan(color.b));
            Assert.That(color.a, Is.EqualTo(1));
            Assert.Throws<InvalidOperationException>(() => VoxelManaSpheres.ColorFromIndexedSprite(new byte[] { 0 }, palette));
        }

        [TestCase(true)]
        [TestCase(false)]
        public void NativeVisibilityClaimCollectionSlotReuseAndGenerationDrivePooledVisuals(bool voxel)
        {
            var parent = new GameObject("Mana lifecycle test");
            var cameraObject = new GameObject("Camera");
            try
            {
                var camera = cameraObject.AddComponent<Camera>();
                camera.transform.position = new Vector3(1023, 0, 1023);
                using var presenter = new VoxelManaSpheres(parent.transform, Colors(), (_,_)=>0, voxel);
                var neutral = Sphere(7, 58, 100, 64, 128, 192);
                presenter.Present(new[] { neutral }, new[] { Visible(7) }, 1);
                presenter.Render(camera);
                Assert.That(presenter.ActiveCount, Is.EqualTo(1));
                Assert.That(presenter.Replaces(7), Is.True);
                var node = parent.transform.GetChild(0).GetChild(0);
                Assert.That(node.position.x, Is.EqualTo(1025).Within(.001));
                Assert.That(node.position.z, Is.EqualTo(1026).Within(.001));
                Assert.That(node.position.y, Is.EqualTo(3 + 100 / 128f).Within(.001));
                Assert.That(node.localScale.x, Is.EqualTo(100 / 64f).Within(.001));
                Assert.That(node.GetComponent<Collider>(), Is.Null);
                var renderer = node.GetComponent<MeshRenderer>();
                var block = new MaterialPropertyBlock(); renderer.GetPropertyBlock(block);
                Assert.That(block.GetColor("_BaseColor"), Is.EqualTo(Color.white));
                var claimed = Sphere(7, 67, 200);
                presenter.Present(new[] { claimed }, new[] { Visible(7) }, 1);
                renderer.GetPropertyBlock(block);
                Assert.That(block.GetColor("_BaseColor"), Is.EqualTo(Color.red));
                Assert.That(node.localScale.x, Is.EqualTo(200 / 64f).Within(.001));
                presenter.Present(Array.Empty<NativeSim.ManaSphereSnapshot>(), new[] { Visible(7) }, 1);
                Assert.That(presenter.ActiveCount, Is.Zero);
                Assert.That(presenter.PoolCount, Is.EqualTo(1));
                Assert.That(node.gameObject.activeSelf, Is.False);
                presenter.Present(new[] { neutral }, Array.Empty<NativeSim.CampaignEntityPresentation>(), 1);
                Assert.That(presenter.Replaces(7), Is.False, "Native-hidden spheres must not be drawn or suppress other actors");
                presenter.Present(new[] { neutral }, new[] { Visible(7) }, 2);
                Assert.That(presenter.ActiveCount, Is.EqualTo(1));
                Assert.That(parent.transform.GetChild(0).GetChild(0), Is.EqualTo(node), "Reused slot must reuse only the visual, never stale state");
                renderer.GetPropertyBlock(block);
                Assert.That(block.GetColor("_BaseColor"), Is.EqualTo(Color.white));
                presenter.Present(new[] { neutral, Sphere(8, 67, 150) }, new[] { Visible(7), Visible(8) }, 2);
                Assert.That(presenter.ActiveCount, Is.EqualTo(2));
                var other = parent.transform.GetChild(0).GetChild(1);
                Assert.That(other.GetComponent<MeshFilter>().sharedMesh, Is.SameAs(node.GetComponent<MeshFilter>().sharedMesh));
                Assert.That(other.GetComponent<MeshRenderer>().sharedMaterial, Is.SameAs(renderer.sharedMaterial));
                presenter.Render(camera, false);
                Assert.That(renderer.enabled, Is.False);
                presenter.Render(camera);
                Assert.That(renderer.enabled, Is.True);
                presenter.Present(new[] { Sphere(8, 58, 50) }, new[] { Visible(8) }, 3);
                Assert.That(presenter.ActiveCount, Is.EqualTo(1));
                Assert.That(presenter.Replaces(7), Is.False, "A new world must not keep a prior world's sphere");
                Assert.That(presenter.Replaces(8), Is.True);
                presenter.Present(new[] { new NativeSim.ManaSphereSnapshot { sourceEntityIndex = 7, active = 0 } }, new[] { Visible(7) }, 2);
                Assert.That(presenter.ActiveCount, Is.Zero);
            }
            finally { UnityEngine.Object.DestroyImmediate(cameraObject); UnityEngine.Object.DestroyImmediate(parent); }
        }

        [TestCase(true)]
        [TestCase(false)]
        public void WorldCensusPresentsOffSourceCameraManaButHonorsNativeHide(bool voxel)
        {
            var parent=new GameObject("World-view mana test");
            var cameraObject=new GameObject("Observer camera");
            try
            {
                var camera=cameraObject.AddComponent<Camera>();
                using var presenter=new VoxelManaSpheres(parent.transform,Colors(),(_,_)=>0,voxel);
                var sphere=Sphere(364,58,100,64,128,0);
                // The source camera census may omit a valid sphere even when
                // the independent 3D observer is looking at it.
                presenter.Present(new[]{sphere},Array.Empty<NativeSim.CampaignEntityPresentation>(),1);
                Assert.That(presenter.Replaces(364),Is.False);
                presenter.Present(new[]{sphere},new[]{Visible(364)},1);
                presenter.Render(camera);
                var renderer=parent.GetComponentInChildren<MeshRenderer>(true);
                Assert.That(renderer.enabled && renderer.gameObject.activeInHierarchy,Is.True);
                // World census remains authoritative for hide/pending-delete.
                presenter.Present(new[]{sphere},Array.Empty<NativeSim.CampaignEntityPresentation>(),1);
                Assert.That(renderer.gameObject.activeInHierarchy,Is.False);
                Assert.That(presenter.Replaces(364),Is.False);
            }
            finally {UnityEngine.Object.DestroyImmediate(cameraObject);UnityEngine.Object.DestroyImmediate(parent);}
        }

        [TestCase(true)]
        [TestCase(false)]
        public void VisualBottomClearsModernGroundWithoutChangingNativeAltitudeAboveIt(bool voxel)
        {
            var parent=new GameObject("Mana terrain test");
            var cameraObject=new GameObject("Camera");
            try
            {
                var camera=cameraObject.AddComponent<Camera>();
                camera.transform.position=new Vector3(1023,0,1023);
                var sampleX=0f;var sampleZ=0f;
                using var presenter=new VoxelManaSpheres(parent.transform,Colors(),(x,z)=>{sampleX=x;sampleZ=z;return 5;},voxel);
                var row=Sphere(7,58,100,64,128,0);
                presenter.Present(new[]{row},new[]{Visible(7)},1);
                presenter.Render(camera);
                var node=parent.transform.GetChild(0).GetChild(0);
                Assert.That(sampleX,Is.EqualTo(1025).Within(.001),"Sample the same wrapped X used to render");
                Assert.That(sampleZ,Is.EqualTo(1026).Within(.001),"Sample the same wrapped Z used to render");
                Assert.That(node.position.y,Is.EqualTo(5+100/128f).Within(.001),"A buried native bottom is raised to the visible surface");

                row.z=640;
                presenter.Present(new[]{row},new[]{Visible(7)},1);
                presenter.Render(camera);
                Assert.That(node.position.y,Is.EqualTo(10+100/128f).Within(.001),"An airborne native bottom stays at its native altitude");
                presenter.Present(new[]{row},Array.Empty<NativeSim.CampaignEntityPresentation>(),1);
                Assert.That(presenter.Replaces(7),Is.False,"Ground correction cannot bypass native visibility");
            }
            finally {UnityEngine.Object.DestroyImmediate(cameraObject);UnityEngine.Object.DestroyImmediate(parent);}
        }

        [Test]
        public void OptInSourceColorNightPreview()
        {
            var directory = Environment.GetEnvironmentVariable("MC_MANA_PREVIEW_DIR");
            if (string.IsNullOrEmpty(directory)) return;
            var source = Environment.GetEnvironmentVariable("MC_MANA_SOURCE_DIR");
            Assert.That(source, Is.Not.Null.And.Not.Empty, "Set MC_MANA_SOURCE_DIR to a Level 1 environment cache");
            Assert.That(SystemInfo.graphicsDeviceType, Is.Not.EqualTo(GraphicsDeviceType.Null));
            var palette = File.ReadAllBytes(Path.Combine(source, "palette/source.rgb8"));
            var colors = new Dictionary<int, Color>();
            foreach (var index in new[] { 58, 68 })
                colors[index] = VoxelManaSpheres.ColorFromIndexedSprite(
                    File.ReadAllBytes(Path.Combine(source, $"textures/{index:000}.indices.u8")), palette);

            var previousPipeline = GraphicsSettings.defaultRenderPipeline;
            var previousQuality = QualitySettings.renderPipeline;
            var previousMode = RenderSettings.ambientMode;
            var previousSky = RenderSettings.ambientSkyColor;
            var previousEquator = RenderSettings.ambientEquatorColor;
            var previousGround = RenderSettings.ambientGroundColor;
            var previousSun = RenderSettings.sun;
            var previousFog = RenderSettings.fog;
            var previousTarget = RenderTexture.active;
            var root = new GameObject("Mana night review");
            var cameraObject = new GameObject("Mana review camera");
            var moonObject = new GameObject("Mana review moon");
            var target = new RenderTexture(960, 600, 24, RenderTextureFormat.ARGB32);
            var pixels = new Texture2D(960, 600, TextureFormat.RGB24, false);
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
                RenderSettings.fog = false;
                var moon = moonObject.AddComponent<Light>();
                moon.type = LightType.Directional; moon.cullingMask = 1 << 31;
                moon.color = new Color(.76f, .86f, 1f); moon.intensity = .8f;
                moon.transform.rotation = Quaternion.LookRotation(-new Vector3(-.38f, .48f, .79f).normalized);
                RenderSettings.sun = moon;
                var camera = cameraObject.AddComponent<Camera>();
                camera.transform.position = new Vector3(8, 4, -19);
                camera.transform.LookAt(new Vector3(8, 1, 0));
                camera.fieldOfView = 45; camera.cullingMask = 1 << 31; camera.allowHDR = true;
                camera.clearFlags = CameraClearFlags.SolidColor;
                camera.backgroundColor = new Color(.04f, .05f, .06f); camera.targetTexture = target;
                using var modern = new VoxelManaSpheres(root.transform, colors, (_, _) => 0, false);
                using var voxel = new VoxelManaSpheres(root.transform, colors, (_, _) => 0, true);
                modern.Present(new[] { Sphere(1, 58, 128, 128), Sphere(2, 68, 128, 384) },
                    new[] { Visible(1), Visible(2) }, 1);
                voxel.Present(new[] { Sphere(3, 58, 128, 640), Sphere(4, 68, 128, 896) },
                    new[] { Visible(3), Visible(4) }, 1);
                foreach (var node in root.GetComponentsInChildren<Transform>(true)) node.gameObject.layer = 31;
                modern.Render(camera); voxel.Render(camera);
                camera.Render(); camera.Render();
                RenderTexture.active = target;
                pixels.ReadPixels(new Rect(0, 0, 960, 600), 0, 0); pixels.Apply();
                var image = pixels.GetPixels32(); var background = image[0];
                var changed = 0; var errors = 0;
                foreach (var pixel in image)
                {
                    if (Mathf.Abs(pixel.r - background.r) + Mathf.Abs(pixel.g - background.g) + Mathf.Abs(pixel.b - background.b) > 12) changed++;
                    if (pixel.r > 200 && pixel.b > 200 && pixel.g < 50) errors++;
                }
                Assert.That(changed, Is.GreaterThan(1000), "night review contains visible geometry");
                Assert.That(errors, Is.LessThan(10), "night review contains no shader error");
                Directory.CreateDirectory(directory);
                using var output = new FileStream(Path.Combine(directory, "mana-night.png"), FileMode.CreateNew, FileAccess.Write);
                var data = pixels.EncodeToPNG(); output.Write(data, 0, data.Length);
            }
            finally
            {
                var camera = cameraObject.GetComponent<Camera>();
                if (camera != null) camera.targetTexture = null;
                RenderTexture.active = previousTarget;
                RenderSettings.sun = previousSun; RenderSettings.ambientMode = previousMode;
                RenderSettings.ambientSkyColor = previousSky; RenderSettings.ambientEquatorColor = previousEquator;
                RenderSettings.ambientGroundColor = previousGround; RenderSettings.fog = previousFog;
                GraphicsSettings.defaultRenderPipeline = previousPipeline; QualitySettings.renderPipeline = previousQuality;
                target.Release(); UnityEngine.Object.DestroyImmediate(target); UnityEngine.Object.DestroyImmediate(pixels);
                UnityEngine.Object.DestroyImmediate(root); UnityEngine.Object.DestroyImmediate(cameraObject); UnityEngine.Object.DestroyImmediate(moonObject);
            }
        }
    }
}
