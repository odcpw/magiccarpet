using System;
using System.IO;
using System.Linq;
using System.Runtime.InteropServices;
using NUnit.Framework;
using UnityEngine;
using UnityEngine.Rendering;
using UnityEngine.Rendering.Universal;
using Object = UnityEngine.Object;

namespace MagicCarpet.Native.Tests
{
    public sealed class ModernAuthoredTreesEditModeTests
    {
        private static ModernAuthoredTrees.TreeObservation Row(ushort id, byte action = 0, ushort parameter = 83) => new()
        {
            entityIndex = id, sourceId = id, classId = 2, model = 0, action = action,
            renderParameter = parameter, renderHeight = 400, drawing = 1,
            x = (ushort)(id * 64), y = 128, z = 64
        };

        [Test]
        public void ExactNativeFamilyAndLifecycleAreAdmitted()
        {
            Assert.That(Marshal.SizeOf<ModernAuthoredTrees.TreeObservation>(), Is.EqualTo(56));
            Assert.That(ModernAuthoredTrees.IsTree(Row(1, 0, 83)), Is.True);
            Assert.That(ModernAuthoredTrees.IsTree(Row(1, 1, 84)), Is.True);
            Assert.That(ModernAuthoredTrees.IsTree(Row(1, 2, 226)), Is.True);
            Assert.That(ModernAuthoredTrees.IsTree(Row(1, 2, 227)), Is.True);
            Assert.That(ModernAuthoredTrees.IsTree(Row(1, 2, 83)), Is.False);
            Assert.That(ModernAuthoredTrees.IsTree(Row(1, 0, 226)), Is.False);
            var wrong = Row(1); wrong.classId = 5;
            Assert.That(ModernAuthoredTrees.IsTree(wrong), Is.False);
        }

        [Test]
        public void BothStylesFollowLiveBurnedHiddenAndResetRows()
        {
            var root = new GameObject("Tree test root");
            var cameraObject = new GameObject("Tree test camera");
            var camera = cameraObject.AddComponent<Camera>();
            ModernAuthoredTrees trees = null;
            try
            {
                trees = new ModernAuthoredTrees(root.transform);
                trees.BeginFrame();
                for (ushort i = 1; i <= 18; i++)
                    Assert.That(trees.Observe(Row(i, 0, i % 2 == 0 ? (ushort)84 : (ushort)83), false), Is.True);
                trees.EndFrame(); trees.Render(camera, true);
                Assert.That(root.transform.childCount, Is.EqualTo(18));
                var first = root.transform.GetChild(0);
                Assert.That(first.position, Is.EqualTo(new Vector3(1, 1, 2)));
                var sourceTrunk = Resources.Load<Mesh>("ModernNature/tree_trunk_mid");
                var sourceLeaves = Resources.Load<Mesh>("ModernNature/tree_leaves_mid");
                var trunk = first.GetChild(0).GetComponent<MeshFilter>().sharedMesh;
                var nearLeaves = first.GetChild(2).GetComponent<MeshFilter>().sharedMesh;
                var farLeaves = first.GetChild(5).GetComponent<MeshFilter>().sharedMesh;
                var originalVertices = sourceTrunk.vertices;
                var trunkVertices = trunk.vertices;
                var originalRoot = Vector3.zero;
                var thickRoot = Vector3.zero;
                var rootCount = 0;
                for (int i = 0; i < originalVertices.Length; i++)
                {
                    if (originalVertices[i].y >= sourceTrunk.bounds.min.y + .2f) continue;
                    originalRoot += new Vector3(originalVertices[i].x, 0f, originalVertices[i].z);
                    thickRoot += new Vector3(trunkVertices[i].x, 0f, trunkVertices[i].z);
                    rootCount++;
                }
                Assert.That(rootCount, Is.GreaterThan(0));
                originalRoot /= rootCount;
                thickRoot /= rootCount;
                Assert.That(thickRoot.magnitude, Is.LessThan(.0001f), "the bole contacts the native anchor");
                for (int i = 0; i < originalVertices.Length; i++)
                {
                    if (originalVertices[i].y >= sourceTrunk.bounds.min.y + .2f) continue;
                    var relative = new Vector3(originalVertices[i].x, 0f, originalVertices[i].z) - originalRoot;
                    Assert.That(Vector3.Distance(new Vector3(trunkVertices[i].x, 0f, trunkVertices[i].z), relative * 3.2f),
                        Is.LessThan(.0001f));
                }
                Assert.That(nearLeaves.GetIndexCount(0), Is.EqualTo(sourceLeaves.GetIndexCount(0) * 10));
                Assert.That(farLeaves.GetIndexCount(0), Is.EqualTo(sourceLeaves.GetIndexCount(0) * 2));
                var sourceColor = sourceLeaves.colors[0];
                var leafColor = nearLeaves.colors[0];
                Assert.That(leafColor.r, Is.EqualTo(sourceColor.r * .5f).Within(.0001f));
                Assert.That(leafColor.g, Is.EqualTo(sourceColor.g * .95f).Within(.0001f));
                Assert.That(leafColor.b, Is.EqualTo(sourceColor.b * 1.4f).Within(.0001f));
                Assert.That(first.GetChild(2).GetComponent<MeshRenderer>().sharedMaterial.GetFloat("_Cutoff"), Is.EqualTo(.15f).Within(.0001f));
                Assert.That(root.transform.GetChild(1).GetChild(2).GetComponent<MeshFilter>().sharedMesh,
                    Is.SameAs(nearLeaves), "all authored trees share one canopy mesh");
                Assert.That(trees.Replaces(1, 1), Is.True);
                Assert.That(trees.Replaces(1, 2), Is.False);
                Assert.That(first.GetChild(2).GetComponent<MeshRenderer>().enabled, Is.True);
                trees.BeginFrame();
                Assert.That(trees.Observe(Row(1, 2, 226), true), Is.True);
                var hidden = Row(2); hidden.drawing = 0;
                Assert.That(trees.Observe(hidden, true), Is.True);
                trees.EndFrame();
                Assert.That(root.transform.childCount, Is.EqualTo(2));
                Assert.That(first.GetChild(2).GetComponent<MeshRenderer>().enabled, Is.False);
                Assert.That(first.GetChild(0).GetComponent<MeshFilter>().sharedMesh.name, Does.Contain("voxel"));
                Assert.That(first.GetChild(2).GetComponent<MeshFilter>().sharedMesh.GetIndexCount(0),
                    Is.GreaterThan(first.GetChild(5).GetComponent<MeshFilter>().sharedMesh.GetIndexCount(0)),
                    "the voxel canopy has a smaller distant mesh");
                var voxelNear = first.GetChild(2).GetComponent<MeshFilter>().sharedMesh;
                var voxelFar = first.GetChild(5).GetComponent<MeshFilter>().sharedMesh;
                if (SystemInfo.graphicsDeviceType == UnityEngine.Rendering.GraphicsDeviceType.Null)
                    Assert.That(voxelNear.colors32[0], Is.EqualTo(new Color32(32, 84, 103, 255)));
                Assert.That(voxelNear.GetIndexCount(0) / 3, Is.LessThanOrEqualTo(80_000),
                    "the shared near voxel canopy remains bounded");
                TestContext.WriteLine($"Tree canopy triangles: Modern near={nearLeaves.GetIndexCount(0) / 3}, far={farLeaves.GetIndexCount(0) / 3}; " +
                    $"Voxel near={voxelNear.GetIndexCount(0) / 3}, far={voxelFar.GetIndexCount(0) / 3} (graphics device: {SystemInfo.graphicsDeviceType})");
                Assert.That(root.transform.GetChild(1).gameObject.activeSelf, Is.False);
                Assert.That(trees.Replaces(2, 2), Is.False);
                trees.BeginFrame();
                trees.Observe(Row(1), false);
                trees.EndFrame();
                Assert.That(root.transform.childCount, Is.EqualTo(1));
                Assert.That(first.GetChild(2).GetComponent<MeshRenderer>().enabled, Is.True);
                Assert.That(first.GetChild(0).GetComponent<MeshFilter>().sharedMesh.name, Does.Not.Contain("voxel"));
            }
            finally { trees?.Dispose(); Object.DestroyImmediate(root); Object.DestroyImmediate(cameraObject); }
        }

        [TestCase(false)]
        [TestCase(true)]
        public void TreeGeometryAndPreview(bool voxel)
        {
            var parent = new GameObject("Tree geometry test");
            var cameraObject = new GameObject("Tree geometry camera");
            var camera = cameraObject.AddComponent<Camera>();
            try
            {
                using var trees = new ModernAuthoredTrees(parent.transform);
                var row = Row(1);
                trees.BeginFrame(); Assert.That(trees.Observe(row, voxel), Is.True); trees.EndFrame();
                trees.Render(camera, true);
                var node = parent.transform.GetChild(0);
                var near = Enumerable.Range(0, 3).Select(i => node.GetChild(i).GetComponent<MeshRenderer>()).ToArray();
                var far = Enumerable.Range(3, 3).Select(i => node.GetChild(i).GetComponent<MeshRenderer>()).ToArray();
                var bounds = near[0].bounds;
                foreach (var part in near.Skip(1)) bounds.Encapsulate(part.bounds);
                int Triangles(MeshRenderer[] parts) => parts.Where(p => p.enabled)
                    .Sum(p => (int)p.GetComponent<MeshFilter>().sharedMesh.GetIndexCount(0) / 3);
                TestContext.WriteLine($"voxel={voxel} bounds={bounds} nearTriangles={Triangles(near)} farTriangles={Triangles(far)} nearDraws={near.Count(p => p.enabled)} farDraws={far.Count(p => p.enabled)} rotation={node.rotation.eulerAngles} nearWoodCellWorld={ModernAuthoredTrees.VoxelNearWoodCellSize * node.localScale.x} nearFoliageCellWorld={ModernAuthoredTrees.VoxelNearFoliageCellSize * node.localScale.x} farCellWorld={ModernAuthoredTrees.VoxelFarCellSize * node.localScale.x}");
                var height = row.renderHeight / 64f;
                Assert.That(bounds.max.y, Is.EqualTo(row.z / 64f + height).Within(height * .05f));
                Assert.That(near[0].bounds.min.y, Is.EqualTo(row.z / 64f).Within(height * .05f));
                Assert.That(Quaternion.Angle(node.rotation, Quaternion.Euler(0f, row.entityIndex * 137.50776f, 0f)), Is.LessThan(.01f));
                Assert.That(Triangles(far), Is.LessThan(Triangles(near) * .75f));
                Assert.That(near.Length + far.Length, Is.EqualTo(6), "tree uses shared meshes, not per-cell objects");
                CapturePreviewIfRequested(trees, camera, node, row, voxel);
                row.action = 1; trees.BeginFrame(); trees.Observe(row, voxel); trees.EndFrame();
                Assert.That(near[2].enabled, Is.True, "burning keeps the live canopy");
                row.action = 2; row.renderParameter = 226;
                trees.BeginFrame(); trees.Observe(row, voxel); trees.EndFrame();
                Assert.That(near[2].enabled, Is.False, "the burned tree is leafless");
                Assert.That(near[0].enabled && near[1].enabled, Is.True);
            }
            finally { Object.DestroyImmediate(parent); Object.DestroyImmediate(cameraObject); }
        }

        private static void CapturePreviewIfRequested(ModernAuthoredTrees trees, Camera camera,
            Transform node, ModernAuthoredTrees.TreeObservation row, bool voxel)
        {
            var directory = Environment.GetEnvironmentVariable("MC_TREE_PREVIEW_DIR");
            if (string.IsNullOrEmpty(directory)) return;
            Assert.That(SystemInfo.graphicsDeviceType, Is.Not.EqualTo(GraphicsDeviceType.Null));
            var previousPipeline = GraphicsSettings.defaultRenderPipeline;
            var previousQuality = QualitySettings.renderPipeline;
            var previousAmbient = RenderSettings.ambientLight;
            var previousMode = RenderSettings.ambientMode;
            var previousSun = RenderSettings.sun;
            var previousFog = RenderSettings.fog;
            var previousTarget = RenderTexture.active;
            var target = new RenderTexture(640, 640, 24, RenderTextureFormat.ARGB32);
            var pixels = new Texture2D(640, 640, TextureFormat.RGB24, false);
            var lightObject = new GameObject("Tree preview light");
            try
            {
                var pipeline = Resources.Load<UniversalRenderPipelineAsset>("ModernPipeline");
                Assert.That(pipeline, Is.Not.Null);
                GraphicsSettings.defaultRenderPipeline = pipeline; QualitySettings.renderPipeline = null;
                foreach (var part in node.GetComponentsInChildren<Transform>(true)) part.gameObject.layer = 31;
                var light = lightObject.AddComponent<Light>();
                light.type = LightType.Directional; light.cullingMask = 1 << 31;
                light.transform.rotation = Quaternion.Euler(35, 155, 0); RenderSettings.sun = light;
                RenderSettings.ambientMode = AmbientMode.Flat; RenderSettings.fog = false;
                camera.cullingMask = 1 << 31; camera.clearFlags = CameraClearFlags.SolidColor;
                camera.backgroundColor = new Color(.12f, .12f, .12f);
                camera.orthographic = true; camera.orthographicSize = row.renderHeight / 64f * .76f;
                camera.targetTexture = target;
                var center = new Vector3(row.x / 64f, row.z / 64f + row.renderHeight / 128f, row.y / 64f);
                camera.transform.position = center + new Vector3(9, 4, -16);
                camera.transform.LookAt(center);
                var lod = node.GetComponent<LODGroup>();
                Directory.CreateDirectory(directory);
                foreach (var view in new[] { "day", "night", "bare", "far" })
                {
                    bool night = view != "day";
                    RenderSettings.ambientLight = night ? new Color(.16f, .20f, .25f) : new Color(.36f, .36f, .36f);
                    light.color = night ? new Color(.76f, .86f, 1f) : Color.white;
                    light.intensity = night ? .8f : 1.2f;
                    var current = row;
                    if (view == "bare") { current.action = 2; current.renderParameter = 226; }
                    trees.BeginFrame(); trees.Observe(current, voxel); trees.EndFrame(); trees.Render(camera, true);
                    lod.ForceLOD(view == "far" ? 1 : 0);
                    camera.Render(); camera.Render();
                    RenderTexture.active = target;
                    pixels.ReadPixels(new Rect(0, 0, 640, 640), 0, 0); pixels.Apply();
                    var path = Path.Combine(directory, (voxel ? "voxel-" : "modern-") + view + ".png");
                    using var output = new FileStream(path, FileMode.CreateNew, FileAccess.Write);
                    var bytes = pixels.EncodeToPNG(); output.Write(bytes, 0, bytes.Length);
                    var image = pixels.GetPixels32();
                    Assert.That(image.Count(p => p.r > 200 && p.b > 200 && p.g < 50), Is.LessThan(10), "tree shader must render");
                }
                lod.ForceLOD(-1);
            }
            finally
            {
                camera.targetTexture = null; RenderTexture.active = previousTarget;
                RenderSettings.ambientLight = previousAmbient; RenderSettings.ambientMode = previousMode;
                RenderSettings.sun = previousSun; RenderSettings.fog = previousFog;
                GraphicsSettings.defaultRenderPipeline = previousPipeline; QualitySettings.renderPipeline = previousQuality;
                target.Release(); Object.DestroyImmediate(target); Object.DestroyImmediate(pixels); Object.DestroyImmediate(lightObject);
            }
        }
    }
}
