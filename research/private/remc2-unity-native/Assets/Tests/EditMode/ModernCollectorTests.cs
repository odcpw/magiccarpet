using System;
using System.IO;
using System.Linq;
using NUnit.Framework;
using UnityEngine;
using UnityEngine.Rendering;
using UnityEngine.Rendering.Universal;

namespace MagicCarpet.Native.Tests
{
    public sealed class ModernCollectorTests
    {
        private static NativeSim.CampaignCastleChild Child() => new NativeSim.CampaignCastleChild {
            entityIndex = 372, sourceId = 152, slotGeneration = 11,
            renderParameter = 169, renderHeight = 800, ownerPlayerColor = 0,
            relationFlags = 3, drawing = 1
        };

        private static NativeSim.CampaignEntityPresentation Sprite() => new NativeSim.CampaignEntityPresentation {
            entityIndex = 372, sourceId = 152, renderParameterIndex = 169,
            logicalHeight = 800, selectedTmap = 88
        };

        [TestCase(false)]
        [TestCase(true)]
        public void NativeLossReuseAndResetDoNotLeaveAStaleBalloon(bool voxel)
        {
            var parent = new GameObject("collector lifecycle test");
            var presenter = new ModernCollector(parent.transform, voxel);
            try
            {
                var child = Child(); var sprite = Sprite();
                presenter.Present(1, 2304, new[] { child }, new[] { sprite });
                Assert.That(presenter.Replaces(372, 152), Is.True);
                var collection = parent.transform.GetChild(0);
                Assert.That(collection.childCount, Is.EqualTo(1));
                var first = collection.GetChild(0);
                Assert.That(first.gameObject.activeSelf, Is.True);

                child.drawing = 0;
                presenter.Present(1, 2305, new[] { child }, new[] { sprite });
                Assert.That(presenter.Replaces(372, 152), Is.False);
                Assert.That(first.gameObject.activeSelf, Is.False, "hidden source row must hide the model");
                presenter.Present(1, 2306, Array.Empty<NativeSim.CampaignCastleChild>(), Array.Empty<NativeSim.CampaignEntityPresentation>());
                Assert.That(collection.childCount, Is.Zero, "reclaimed source row must destroy the visual");

                child.drawing = 1; child.slotGeneration = 12;
                presenter.Present(1, 2307, new[] { child }, new[] { sprite });
                Assert.That(presenter.Replaces(372, 152), Is.True);
                var reused = collection.GetChild(0);
                child.slotGeneration = 13;
                presenter.Present(1, 2308, new[] { child }, new[] { sprite });
                Assert.That(collection.childCount, Is.EqualTo(1));
                Assert.That(collection.GetChild(0), Is.Not.SameAs(reused), "reused slot gets a fresh instance");
                var beforeReset = collection.GetChild(0);
                presenter.Present(2, 0, Array.Empty<NativeSim.CampaignCastleChild>(), Array.Empty<NativeSim.CampaignEntityPresentation>());
                Assert.That(presenter.Replaces(372, 152), Is.False);
                Assert.That(collection.childCount, Is.Zero, "reset clears the previous generation");
                presenter.Present(2, 2304, new[] { child }, new[] { sprite });
                Assert.That(presenter.Replaces(372, 152), Is.True);
                Assert.That(collection.GetChild(0), Is.Not.SameAs(beforeReset));
                sprite.selectedTmap = 89;
                presenter.Present(2, 2305, new[] { child }, new[] { sprite });
                Assert.That(presenter.Replaces(372, 152), Is.False, "mismatched sprite must not be replaced");
            }
            finally
            {
                presenter.Dispose();
                UnityEngine.Object.DestroyImmediate(parent);
            }
        }

        [TestCase(false)]
        [TestCase(true)]
        public void SourcePlacementMaterialsAndVoxelDetailAreBound(bool voxel)
        {
            var parent = new GameObject("Collector geometry test");
            var cameraObject = new GameObject("Collector camera");
            var camera = cameraObject.AddComponent<Camera>();
            try
            {
                using var presenter = new ModernCollector(parent.transform, voxel);
                var child = Child(); child.x = 640; child.y = 1280; child.z = 192; child.yaw = 1024;
                var sprite = Sprite();
                presenter.Present(1, 1, new[] { child }, new[] { sprite });
                presenter.Render(camera, true);
                var node = parent.transform.GetChild(0).GetChild(0);
                Assert.That(node.position, Is.EqualTo(new Vector3(10,3,20)));
                Assert.That(Quaternion.Angle(node.rotation, Quaternion.identity), Is.LessThan(.01f));
                var source = node.GetComponentsInChildren<SkinnedMeshRenderer>(true).Single();
                var near = node.Find("Collector voxel near"); var far = node.Find("Collector voxel far");
                if (voxel)
                {
                    Assert.That(source.forceRenderingOff, Is.True);
                    Assert.That(near, Is.Not.Null); Assert.That(far, Is.Not.Null);
                    Assert.That(near.gameObject.activeSelf, Is.True); Assert.That(far.gameObject.activeSelf, Is.False);
                    var nearRenderers = near.GetComponentsInChildren<MeshRenderer>(true);
                    var farRenderers = far.GetComponentsInChildren<MeshRenderer>(true);
                    Assert.That(nearRenderers.Length, Is.EqualTo(1)); Assert.That(farRenderers.Length, Is.EqualTo(1));
                    var nearMesh = near.GetComponentInChildren<MeshFilter>(true).sharedMesh;
                    var farMesh = far.GetComponentInChildren<MeshFilter>(true).sharedMesh;
                    int Triangles(Mesh mesh) { int n = 0; for (int i = 0; i < mesh.subMeshCount; i++) n += (int)mesh.GetIndexCount(i) / 3; return n; }
                    int nearTriangles = Triangles(nearMesh), farTriangles = Triangles(farMesh);
                    // A hidden skinned renderer's cached bounds may be stale or
                    // carry FBX axes; measure the actual posed vertices instead.
                    var baked = new Mesh();
                    Bounds sourceBounds;
                    try
                    {
                        source.BakeMesh(baked, true);
                        var positions = baked.vertices;
                        Assert.That(positions.Length, Is.GreaterThan(0));
                        sourceBounds = new Bounds(source.transform.TransformPoint(positions[0]), Vector3.zero);
                        foreach (var position in positions.Skip(1)) sourceBounds.Encapsulate(source.transform.TransformPoint(position));
                    }
                    finally { UnityEngine.Object.DestroyImmediate(baked); }
                    var nearBounds = nearRenderers[0].bounds;
                    TestContext.WriteLine($"Collector source={sourceBounds} near={nearBounds} nearTriangles={nearTriangles} farTriangles={farTriangles} nearDraws={nearMesh.subMeshCount} farDraws={farMesh.subMeshCount}");
                    Assert.That(nearBounds.size.y, Is.EqualTo(child.renderHeight / 64f).Within(.3f));
                    Assert.That(nearBounds.min.y, Is.EqualTo(sourceBounds.min.y).Within(.3f));
                    Assert.That(nearBounds.size.x, Is.EqualTo(sourceBounds.size.x).Within(.4f));
                    Assert.That(nearTriangles, Is.LessThanOrEqualTo(150000));
                    Assert.That(farTriangles, Is.LessThan(nearTriangles / 2));
                    Assert.That(nearMesh.normals.All(n => Mathf.Abs(n.magnitude - 1) < .01f), Is.True);
                    Assert.That(nearRenderers[0].sharedMaterials.Length, Is.EqualTo(9));
                    camera.transform.position = new Vector3(10, 9, -180);
                    presenter.Render(camera, true);
                    Assert.That(near.gameObject.activeSelf, Is.False); Assert.That(far.gameObject.activeSelf, Is.True);
                    presenter.Render(camera, false);
                    Assert.That(node.gameObject.activeSelf, Is.False);
                }
                else Assert.That(source.forceRenderingOff, Is.False);
                CapturePreviewIfRequested(presenter, camera, node, voxel);
            }
            finally { UnityEngine.Object.DestroyImmediate(parent); UnityEngine.Object.DestroyImmediate(cameraObject); }
        }

        [Test]
        public void VoxelMaterialsFollowAllNativeOwnerVariants()
        {
            var parent = new GameObject("Collector owner test");
            try
            {
                using var presenter = new ModernCollector(parent.transform, true);
                byte[] selectors = { 0, 1, 4, 3, 2, 5, 7, 6 };
                var distinct = new System.Collections.Generic.HashSet<Color>();
                for (byte owner = 0; owner < selectors.Length; owner++)
                {
                    var child = Child(); child.ownerPlayerColor = owner;
                    var sprite = Sprite(); sprite.selectedTmap = (ushort)(88 + selectors[owner]);
                    presenter.Present(1, owner + 1UL, new[] { child }, new[] { sprite });
                    Assert.That(presenter.Replaces(child.entityIndex, child.sourceId), Is.True);
                    var node = parent.transform.GetChild(0).GetChild(0);
                    var source = node.GetComponentsInChildren<SkinnedMeshRenderer>(true).Single();
                    var near = node.Find("Collector voxel near").GetComponentInChildren<MeshRenderer>(true);
                    var far = node.Find("Collector voxel far").GetComponentInChildren<MeshRenderer>(true);
                    Color Tint(Renderer renderer, string role)
                    {
                        int index = Array.FindIndex(renderer.sharedMaterials, m => m.name.Replace('_',' ').ToLowerInvariant().Contains(role));
                        Assert.That(index, Is.GreaterThanOrEqualTo(0), role);
                        var block = new MaterialPropertyBlock(); renderer.GetPropertyBlock(block, index);
                        return block.GetColor("_BaseColor");
                    }
                    var sourceFabric = Tint(source, "owner fabric");
                    distinct.Add(sourceFabric);
                    foreach (string role in new[] { "owner crown", "owner band", "owner fabric", "fabric seams", "owner embroidery" })
                    {
                        Assert.That(Tint(near, role), Is.EqualTo(Tint(source, role)), $"near {role}, owner {owner}");
                        Assert.That(Tint(far, role), Is.EqualTo(Tint(source, role)), $"far {role}, owner {owner}");
                    }
                }
                Assert.That(distinct.Count, Is.EqualTo(8));
            }
            finally { UnityEngine.Object.DestroyImmediate(parent); }
        }

        private static void CapturePreviewIfRequested(ModernCollector presenter, Camera camera, Transform node, bool voxel)
        {
            var directory = Environment.GetEnvironmentVariable("MC_COLLECTOR_PREVIEW_DIR");
            if (string.IsNullOrEmpty(directory)) return;
            Assert.That(SystemInfo.graphicsDeviceType, Is.Not.EqualTo(GraphicsDeviceType.Null));
            var previousPipeline = GraphicsSettings.defaultRenderPipeline; var previousQuality = QualitySettings.renderPipeline;
            var previousTarget = RenderTexture.active; var previousSun = RenderSettings.sun;
            var previousMode = RenderSettings.ambientMode; var previousAmbient = RenderSettings.ambientLight;
            var previousFog = RenderSettings.fog;
            var target = new RenderTexture(640, 720, 24, RenderTextureFormat.ARGB32);
            var pixels = new Texture2D(640, 720, TextureFormat.RGB24, false);
            var lightObject = new GameObject("Collector preview light");
            try
            {
                var pipeline = Resources.Load<UniversalRenderPipelineAsset>("ModernPipeline"); Assert.That(pipeline, Is.Not.Null);
                GraphicsSettings.defaultRenderPipeline = pipeline; QualitySettings.renderPipeline = null;
                foreach (var part in node.GetComponentsInChildren<Transform>(true)) part.gameObject.layer = 31;
                var light = lightObject.AddComponent<Light>(); light.type = LightType.Directional; light.cullingMask = 1 << 31;
                light.transform.rotation = Quaternion.Euler(35,155,0); RenderSettings.sun = light;
                RenderSettings.ambientMode = AmbientMode.Flat; RenderSettings.fog = false;
                camera.cullingMask = 1 << 31; camera.clearFlags = CameraClearFlags.SolidColor;
                camera.backgroundColor = new Color(.08f,.09f,.10f); camera.orthographic = true;
                camera.orthographicSize = 8; camera.targetTexture = target;
                var center = new Vector3(10,9.25f,20);
                Directory.CreateDirectory(directory);
                foreach (var view in new[] { "front", "side", "night", "far" })
                {
                    bool night = view == "night" || view == "far";
                    RenderSettings.ambientLight = night ? new Color(.16f,.20f,.25f) : new Color(.32f,.32f,.32f);
                    light.color = night ? new Color(.76f,.86f,1) : Color.white; light.intensity = night ? .8f : 1.1f;
                    camera.transform.position = center + (view == "side" ? new Vector3(28,8,-28) : new Vector3(0,0,view == "far" ? -180 : -40));
                    camera.transform.LookAt(center); presenter.Render(camera, true);
                    camera.Render(); camera.Render(); RenderTexture.active = target;
                    pixels.ReadPixels(new Rect(0,0,640,720),0,0); pixels.Apply();
                    var path = Path.Combine(directory, (voxel ? "voxel-" : "modern-") + view + ".png");
                    using var output = new FileStream(path, FileMode.CreateNew, FileAccess.Write);
                    var data = pixels.EncodeToPNG(); output.Write(data,0,data.Length);
                    Assert.That(pixels.GetPixels32().Count(p => p.r > 200 && p.b > 200 && p.g < 50), Is.LessThan(10), "no shader errors");
                }
            }
            finally
            {
                camera.targetTexture = null; RenderTexture.active = previousTarget; RenderSettings.sun = previousSun;
                RenderSettings.ambientMode = previousMode; RenderSettings.ambientLight = previousAmbient; RenderSettings.fog = previousFog;
                GraphicsSettings.defaultRenderPipeline = previousPipeline; QualitySettings.renderPipeline = previousQuality;
                target.Release(); UnityEngine.Object.DestroyImmediate(target); UnityEngine.Object.DestroyImmediate(pixels); UnityEngine.Object.DestroyImmediate(lightObject);
            }
        }
    }
}
