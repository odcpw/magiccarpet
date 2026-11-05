using NUnit.Framework;
using System.IO;
using UnityEditor;
using Newtonsoft.Json.Linq;
using UnityEngine;
using UnityEngine.Rendering.Universal;

namespace MagicCarpet.Native.Tests
{
    public sealed class ModernRenderingEditModeTests
    {
        [System.Runtime.InteropServices.DllImport("libc", SetLastError = true)]
        private static extern int kill(int processId, int signal);

        [TestCase("grass_lod")]
        [TestCase("fern_lod")]
        [TestCase("shrub_lod")]
        [TestCase("tree_leaves_lod")]
        public void BakedViewBoundsMatchActualImportedSource(string asset)
        {
            var record = JObject.Parse(File.ReadAllText("Assets/Resources/ModernNature/LODS_PROVENANCE.json"))["bake_bounds"][asset];
            var prefab = AssetDatabase.LoadAssetAtPath<GameObject>("Assets/Resources/ModernNature/" + (string)record["source_fbx"]);
            Assert.That(prefab, Is.Not.Null);
            MeshFilter selected = null;
            foreach (var filter in prefab.GetComponentsInChildren<MeshFilter>(true))
                if (filter.name == (string)record["part"]) selected = filter;
            Assert.That(selected, Is.Not.Null);
            var vertices = selected.sharedMesh.vertices;
            Assert.That(vertices.Length, Is.GreaterThan(0));
            var bounds = new Bounds(selected.transform.localToWorldMatrix.MultiplyPoint3x4(vertices[0]), Vector3.zero);
            for (var i = 0; i < vertices.Length; i++)
            {
                vertices[i] = selected.transform.localToWorldMatrix.MultiplyPoint3x4(vertices[i]);
                bounds.Encapsulate(vertices[i]);
            }
            var radius = 0f;
            foreach (var vertex in vertices) radius = Mathf.Max(radius, Vector3.Distance(vertex, bounds.center));
            var center = record["expected_unity_import_center"];
            var capturedCenter = new Vector3((float)center[0], (float)center[1], (float)center[2]);
            var diameter = (float)record["expected_unity_import_sphere_diameter"];
            var tolerance = Mathf.Max(.0001f, diameter * .000002f);
            void RequireMatch(Vector3 candidateCenter, float candidateDiameter)
            {
                Assert.That(Vector3.Distance(candidateCenter, bounds.center), Is.LessThanOrEqualTo(tolerance), "bake and imported source must share the tight vertex center");
                Assert.That(candidateDiameter, Is.EqualTo(radius * 2.02f).Within(tolerance), "same padded source sphere in offline and runtime views");
            }
            RequireMatch(capturedCenter, diameter);
            Assert.Throws<AssertionException>(() => RequireMatch(capturedCenter + Vector3.right * diameter * .01f, diameter),
                "reject a displaced capture instead of accepting its image as a new baseline");
            Assert.Throws<AssertionException>(() => RequireMatch(capturedCenter, diameter * 1.01f), "reject a mis-scaled capture");
        }

        [TestCase("rock_moss_set_01_lod", "rock_moss_set_01_rock03")]
        [TestCase("tree_small_02_lod", "tree_trunk")]
        [TestCase("tree_small_02_lod", "tree_branches")]
        public void GeneratedSolidLodsKeepAuthoredMeshNamesAndImportTransforms(string asset, string part)
        {
            var path = "Assets/Resources/ModernNature/" + asset + ".fbx";
            var model = AssetImporter.GetAtPath(path) as ModelImporter;
            Assert.That(model, Is.Not.Null);
            Assert.That(model.preserveHierarchy, Is.True);
            Assert.That(model.isReadable, Is.True);
            var prefab = AssetDatabase.LoadAssetAtPath<GameObject>(path);
            MeshFilter selected = null;
            foreach (var filter in prefab.GetComponentsInChildren<MeshFilter>(true)) if (filter.name == part) selected = filter;
            Assert.That(selected, Is.Not.Null, "authored identity survives the single-object root import");
            Assert.That(selected.sharedMesh.vertexCount, Is.GreaterThan(0));
            Assert.That(Mathf.Abs(selected.transform.localToWorldMatrix.determinant), Is.GreaterThan(.000001f));
        }

        [TestCase("grass", false)]
        [TestCase("fern", false)]
        [TestCase("shrub", false)]
        [TestCase("tree_leaves", false)]
        [TestCase("grass", true)]
        [TestCase("fern", true)]
        [TestCase("shrub", true)]
        [TestCase("tree_leaves", true)]
        public void FoliageAtlasReimportRetainsColorSpaceCoverageAndProjectionLayout(string name, bool normalDepth)
        {
            var path = "Assets/Resources/ModernNature/" + name + (normalDepth ? "_lod_normal_depth.png" : "_lod_color.png");
            AssetDatabase.ImportAsset(path, ImportAssetOptions.ForceUpdate | ImportAssetOptions.ForceSynchronousImport);
            var importer = AssetImporter.GetAtPath(path) as TextureImporter;
            var texture = AssetDatabase.LoadAssetAtPath<Texture2D>(path);
            Assert.That(texture, Is.Not.Null);
            Assert.That(texture.width, Is.EqualTo(1024));
            Assert.That(texture.height, Is.EqualTo(640));
            Assert.That(importer.sRGBTexture, Is.EqualTo(!normalDepth), "source albedo is sRGB; encoded object normals and depth are linear");
            Assert.That(importer.alphaIsTransparency, Is.EqualTo(!normalDepth), "depth alpha must not trigger color dilation");
            Assert.That(importer.mipMapsPreserveCoverage, Is.False, "match source per-surface cutout mip behavior");
            Assert.That(importer.alphaTestReferenceValue, Is.EqualTo(.4f));
            Assert.That(importer.npotScale, Is.EqualTo(TextureImporterNPOTScale.None));
            Assert.That(importer.wrapMode, Is.EqualTo(TextureWrapMode.Clamp));
            Assert.That(importer.filterMode, Is.EqualTo(FilterMode.Bilinear));
            Assert.That(importer.alphaSource, Is.EqualTo(TextureImporterAlphaSource.FromInput));
            Assert.That(importer.mipmapEnabled, Is.True);
            if (normalDepth)
            {
                Assert.That(importer.textureCompression, Is.EqualTo(TextureImporterCompression.Uncompressed));
                Assert.That(texture.format, Is.EqualTo(TextureFormat.RGBA32));
            }
            else Assert.That(importer.GetPlatformTextureSettings("Standalone").format, Is.EqualTo(TextureImporterFormat.BC7));
        }

        [Test]
        public void PackedTerrainRetainsEverySourceChannelWithAnIndependentDecoder()
        {
            // Native GetPixels32 comparisons were intermittently inconsistent
            // for unchanged images. Keep the exact-byte gate, using an
            // independent pinned decoder plus deliberately corrupted controls.
            using var process = new System.Diagnostics.Process
            {
                // setsid gives this Linux editor check its own process group,
                // including uv's Python child, for bounded timeout cleanup.
                StartInfo = new System.Diagnostics.ProcessStartInfo("setsid")
                {
                    WorkingDirectory = Directory.GetParent(Application.dataPath).FullName,
                    Arguments = "uv run --offline --no-project --with pillow==13.0.0 python Tools/check_modern_terrain.py --self-test",
                    UseShellExecute = false, RedirectStandardOutput = true, RedirectStandardError = true
                }
            };
            Assert.That(process.Start(), Is.True);
            var output = process.StandardOutput.ReadToEndAsync();
            var error = process.StandardError.ReadToEndAsync();
            if (!process.WaitForExit(60000))
            {
                kill(-process.Id, 9);
                Assert.That(process.WaitForExit(5000), Is.True, "checker process group did not stop");
                Assert.Fail("Independent terrain checker timed out");
            }
            Assert.That(System.Threading.Tasks.Task.WaitAll(new System.Threading.Tasks.Task[] { output, error }, 5000),
                Is.True, "checker output pipes did not close");
            Assert.That(process.ExitCode, Is.Zero, error.Result + output.Result);
            var result = JObject.Parse(output.Result);
            Assert.That((string)result["schema"], Is.EqualTo("mc-modern-terrain-pack-check/v1"));
            Assert.That((string)result["status"], Is.EqualTo("passed"));
            Assert.That(((JArray)result["layers"]).Count, Is.EqualTo(2));
            foreach (var layer in result["layers"]) Assert.That(((JArray)layer["errors"]).Count, Is.Zero);
            Assert.That((bool)result["negative_controls"]["one_rgb_byte_rejected"], Is.True);
            Assert.That((bool)result["negative_controls"]["one_alpha_byte_rejected"], Is.True);
        }

        [TestCase("Rock")]
        [TestCase("Ground")]
        public void PackedTerrainImportsFullResolutionMipsAndIndependentAlpha(string layer)
        {
            var resource = Resources.Load<Texture2D>("ModernTextures/" + layer + "AlbedoRoughness");
            var path = AssetDatabase.GetAssetPath(resource);
            var importer = AssetImporter.GetAtPath(path) as TextureImporter;
            Assert.That(resource, Is.Not.Null);
            Assert.That(resource.width, Is.EqualTo(2048));
            Assert.That(resource.height, Is.EqualTo(2048));
            Assert.That(resource.mipmapCount, Is.EqualTo(12));
            Assert.That(importer.sRGBTexture, Is.True);
            Assert.That(importer.alphaIsTransparency, Is.False, "roughness alpha must not dilate color RGB");
            Assert.That(importer.alphaSource, Is.EqualTo(TextureImporterAlphaSource.FromInput));
            Assert.That(importer.anisoLevel, Is.EqualTo(8));
            Assert.That(importer.GetPlatformTextureSettings("Standalone").format, Is.EqualTo(TextureImporterFormat.BC7));
        }

        [Test]
        public void CloudLatticeRetainsSignedShaderCornersWithinSixteenBitQuantization()
        {
            var lattice = ModernCloudNoise.Lattice();
            const int side = ModernCloudNoise.Size;
            Assert.That(lattice.Length, Is.EqualTo(side * side * side));
            // The shader samples (integer + smooth fraction + 64.5) / 128.
            // Verify all lattice corners including negative coordinates and
            // the x-fastest texture upload order, not just a constant fixture.
            double maximumError = 0;
            for (var z = 0; z < side; z++)
            for (var y = 0; y < side; y++)
            for (var x = 0; x < side; x++)
            {
                var stored = lattice[x + side * (y + side * z)] / 65535.0;
                var expected = ModernCloudNoise.Corner(x - side / 2, y - side / 2, z - side / 2);
                maximumError = System.Math.Max(maximumError, System.Math.Abs(stored - expected));
            }
            Assert.That(maximumError, Is.LessThanOrEqualTo(.5 / 65535.0 + 1e-10));
            Assert.That(lattice[0], Is.Not.EqualTo(lattice[side * side / 2]));
        }

        [Test]
        public void ReflectionCameraRetainsSmokeDepthButOmitsUnusedColorAndShadowPass()
        {
            var root = new GameObject("Reflection budget test");
            var water = new Material(Shader.Find("MagicCarpet/ModernWater"));
            var reflection = new ModernWaterReflection(root.transform, water);
            try
            {
                var camera = root.GetComponentInChildren<Camera>();
                var data = camera.GetUniversalAdditionalCameraData();
                Assert.That(camera.enabled, Is.False);
                Assert.That(data.requiresDepthTexture, Is.True, "reflected soft smoke consumes scene depth");
                Assert.That(data.requiresColorTexture, Is.False);
                Assert.That(data.renderShadows, Is.False);
                Assert.That(data.renderPostProcessing, Is.False);
            }
            finally
            {
                reflection.Dispose();
                Object.DestroyImmediate(root);
                Object.DestroyImmediate(water);
            }
        }
    }
}
