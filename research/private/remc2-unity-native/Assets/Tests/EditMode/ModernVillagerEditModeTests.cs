using System;
using System.IO;
using System.Linq;
using System.Reflection;
using System.Security.Cryptography;
using NUnit.Framework;
using UnityEditor;
using UnityEngine;

namespace MagicCarpet.Native.Tests
{
    public sealed class ModernVillagerEditModeTests
    {
        private static ModernCharacterAsset Asset(string name = "Villager") => Resources.Load<ModernCharacterAsset>("ModernCharacters/" + name + "/Character");
        [Serializable] private sealed class MaterialSpec { public string name, normal; }
        [Serializable] private sealed class MaterialManifest { public MaterialSpec material; public MaterialSpec[] materials; }

        [Test]
        public void MaterialLoaderDistinguishesAbsentNormalFromMissingDeclaredTexture()
        {
            var importer = AppDomain.CurrentDomain.GetAssemblies().Select(a => a.GetType("MagicCarpet.Native.Editor.ModernCharacterImport"))
                .Single(t => t != null);
            var spec = importer.GetNestedType("MaterialMaps", BindingFlags.NonPublic);
            var load = importer.GetMethod("ImportMaps", BindingFlags.NonPublic | BindingFlags.Static);
            var maps = Activator.CreateInstance(spec, true);
            spec.GetField("name").SetValue(maps, "MaterialLoaderProbe");
            spec.GetField("albedo").SetValue(maps, "textures/body_albedo.jpg");
            spec.GetField("orm").SetValue(maps, "textures/body_orm.jpg");
            const string folder = "Assets/Resources/ModernCharacters/Archer/";
            var prefix = "normal_probe_" + Guid.NewGuid().ToString("N") + "_";
            try
            {
                spec.GetField("normal").SetValue(maps, "textures/absent-declared-normal.png");
                var error = Assert.Throws<TargetInvocationException>(() => load.Invoke(null, new object[] { folder, maps, prefix }));
                Assert.That(error.InnerException, Is.TypeOf<InvalidOperationException>());
                Assert.That(error.InnerException.Message, Does.StartWith("Declared character PBR maps are absent"));
                spec.GetField("normal").SetValue(maps, null);
                var optional = load.Invoke(null, new object[] { folder, maps, prefix });
                Assert.That(optional.GetType().GetField("normal", BindingFlags.NonPublic | BindingFlags.Instance).GetValue(optional), Is.Null);
                spec.GetField("normal").SetValue(maps, "textures/body_normal.png");
                var declared = load.Invoke(null, new object[] { folder, maps, prefix });
                Assert.That(declared.GetType().GetField("normal", BindingFlags.NonPublic | BindingFlags.Instance).GetValue(declared),
                    Is.EqualTo(AssetDatabase.LoadAssetAtPath<Texture2D>(folder + "textures/body_normal.png")));
            }
            finally
            {
                // This exact generated fixture is never referenced by a prefab.
                AssetDatabase.DeleteAsset(folder + prefix + "metallic_occlusion_smoothness.png");
            }
        }

        [Test]
        public void AbsentRecordedInputLeavesOrdinaryControlsInCharge()
        {
            Assert.That(new CampaignInputPlayback(Array.Empty<string>()).At(1, 0), Is.Null);
        }

        [Test]
        public void RecordedInputRequiresMatchingReplayTickAndOwner()
        {
            var folder = Path.Combine(Path.GetTempPath(), "mc-input-playback-test-" + Guid.NewGuid().ToString("N"));
            Directory.CreateDirectory(folder);
            var replay = Path.Combine(folder, "replay.json");
            var commands = Path.Combine(folder, "commands.json");
            try
            {
                File.WriteAllText(replay, "unit-test input identity; not a native replay");
                string hash;
                using (var sha = SHA256.Create()) hash = BitConverter.ToString(sha.ComputeHash(File.ReadAllBytes(replay))).Replace("-", "").ToLowerInvariant();
                File.WriteAllText(commands, "{\"schema\":\"mc-campaign-input-playback/v1\",\"startingTick\":\"443\",\"replaySha256\":\"" + hash +
                    "\",\"commands\":[{\"longitudinal\":\"accelerate\",\"lateral\":\"negative\",\"cast_selected\":true,\"roll_axis\":24,\"pitch_axis\":-127,\"look_yaw\":69,\"look_pitch\":365,\"select_spell\":null}]}");
                var args = new[] { "-mc-input-playback", commands, "-mc-runtime-replay-load", replay };
                var stream = new CampaignInputPlayback(args);
                Assert.Throws<InvalidOperationException>(() => stream.At(2, 444));
                var frame = stream.At(2, 443);
                Assert.That(frame.Flags, Is.EqualTo(69));
                Assert.That(frame.look_pitch, Is.EqualTo(365));
                Assert.That(frame.select_spell, Is.Null, "A JSON null selection must not become an empty semantic command");
                Assert.That(stream.At(2, 444).Flags, Is.Zero, "Completed stream must coast rather than repeat its last cast");
                Assert.Throws<InvalidOperationException>(() => stream.At(3, 444));
                Assert.Throws<InvalidOperationException>(() => stream.At(2, 442));
                File.WriteAllText(replay, "a different starting replay");
                Assert.Throws<ArgumentException>(() => new CampaignInputPlayback(args));
            }
            finally
            {
                // These three paths are disposable fixtures created by this test.
                File.Delete(commands);
                File.Delete(replay);
                Directory.Delete(folder);
            }
        }

        [Test]
        public void AllFourVillagerLooksUseDistinctAssetsAndSourceHeights()
        {
            var row = new NativeSim.CampaignCharacterState { classId = 5, model = 13 };
            foreach (var (look, resource, height) in new[] {
                (239, "VillagerBrown", 200), (241, "Villager", 250),
                (242, "VillagerStriped", 250), (271, "VillagerCarrier", 300) })
            {
                row.renderParameter = (ushort)look;
                Assert.That(ModernCharacterActors.IsSupportedActor(row), Is.True);
                var binding = ModernVillagerAppearances.Get((ushort)look);
                Assert.That(binding.Resource, Is.EqualTo(resource));
                Assert.That(binding.SourceHeight, Is.EqualTo(height / 64f));
            }
            Assert.That(ModernVillagerAppearances.All.Select(b => b.Resource).Distinct().Count(), Is.EqualTo(4));
            foreach (var look in new ushort[] { 0, 221, 219, 65535 })
            {
                row.renderParameter = look;
                Assert.That(ModernCharacterActors.IsSupportedActor(row), Is.False);
                Assert.Throws<ArgumentException>(() => ModernVillagerAppearances.Get(look));
            }
            row.renderParameter = 241;
            row.classId = 9;
            Assert.That(ModernCharacterActors.IsSupportedActor(row), Is.False);
        }

        [Test]
        public void NativeVillagerDeathKeepsZeroLifeAliveAndAdmitsSourceDeathActions()
        {
            Assert.That(ModernVillagerPresenter.SourceDeath(111, 0), Is.False);
            Assert.That(ModernVillagerPresenter.SourceDeath(110, 1), Is.False);
            Assert.That(ModernVillagerPresenter.SourceDeath(111, -1), Is.True);
            Assert.That(ModernCharacterActors.SourceVoxelDeath(13, 108, 5), Is.True);
            Assert.That(ModernCharacterActors.SourceVoxelDeath(13, 109, 5), Is.True);
        }

        [Test]
        public void MovingHitRequiresExplicitUnambiguousAuthoredBones()
        {
            var asset = ScriptableObject.CreateInstance<ModernCharacterAsset>();
            var model = new GameObject("AnatomicalRigBindingProbe");
            try
            {
                var chest = new GameObject("DEF-spine.003");
                chest.transform.SetParent(model.transform, false);
                Assert.Throws<InvalidOperationException>(() => asset.ResolveHitOverlayBones(model.transform));
                asset.hitOverlayBones = new[] { "Spine" };
                Assert.Throws<InvalidOperationException>(() => asset.ResolveHitOverlayBones(model.transform));
                asset.hitOverlayBones = new[] { "DEF-spine.003" };
                Assert.That(asset.ResolveHitOverlayBones(model.transform), Is.EqualTo(new[] { chest.transform }));
                asset.hitOverlayBones = new[] { "DEF-spine.003", "DEF-spine.003" };
                Assert.Throws<InvalidOperationException>(() => asset.ResolveHitOverlayBones(model.transform));
                asset.hitOverlayBones = new[] { "DEF-spine.003" };
                new GameObject("DEF-spine.003").transform.SetParent(chest.transform, false);
                Assert.Throws<InvalidOperationException>(() => asset.ResolveHitOverlayBones(model.transform));
            }
            finally
            {
                UnityEngine.Object.DestroyImmediate(model);
                UnityEngine.Object.DestroyImmediate(asset);
            }
        }

        [Test]
        public void UprightPropBindingRejectsMissingAndAmbiguousTargets()
        {
            var asset = ScriptableObject.CreateInstance<ModernCharacterAsset>();
            var root = new GameObject("Carried prop binding");
            try
            {
                Assert.That(asset.ResolveUprightBone(root.transform), Is.Null);
                asset.uprightBone = "Jar";
                Assert.Throws<InvalidOperationException>(() => asset.ResolveUprightBone(root.transform));
                var jar = new GameObject("Jar"); jar.transform.SetParent(root.transform, false);
                Assert.That(asset.ResolveUprightBone(root.transform), Is.SameAs(jar.transform));
                new GameObject("Jar").transform.SetParent(root.transform, false);
                Assert.Throws<InvalidOperationException>(() => asset.ResolveUprightBone(root.transform));
            }
            finally { UnityEngine.Object.DestroyImmediate(root); UnityEngine.Object.DestroyImmediate(asset); }
        }

        [TestCase(false)]
        [TestCase(true)]
        public void CarriedJarStaysLevelDuringSlopeTravelAndHitsButReleasesDuringDeath(bool voxel)
        {
            var asset = Asset("VillagerCarrier");
            using var villager = new ModernVillagerPresenter(asset, null, asset.height);
            var skin = villager.Root.GetComponentsInChildren<SkinnedMeshRenderer>(true).Single();
            var support = asset.ResolveUprightSupport(villager.Root.transform);
            var supportIndex = Array.IndexOf(skin.bones, support);
            var weights = skin.sharedMesh.boneWeights;
            var headVertices = Enumerable.Range(0, weights.Length).Where(i => {
                var w = weights[i];
                return (w.boneIndex0 == supportIndex ? w.weight0 : 0) + (w.boneIndex1 == supportIndex ? w.weight1 : 0) +
                    (w.boneIndex2 == supportIndex ? w.weight2 : 0) + (w.boneIndex3 == supportIndex ? w.weight3 : 0) >= .9999f;
            }).ToArray();
            Assert.That(headVertices.Length, Is.GreaterThan(0));
            if (voxel) villager.AttachVoxelSkin(Resources.Load<MagicCarpet.Native.Voxels.VoxelGoatAsset>(
                "ModernCharacters/VillagerCarrier/Voxels/VillagerCarrier64"));
            var bone = asset.ResolveUprightBone(villager.Root.transform);
            Assert.That(bone, Is.Not.Null, "Carrier must declare its actual held vessel");
            var localUp = bone.InverseTransformDirection(Vector3.up);
            var mesh = new Mesh();
            void CheckCrownContact()
            {
                skin.BakeMesh(mesh, true);
                var vertices = mesh.vertices;
                var radius = asset.uprightFootRadius * villager.Root.transform.lossyScale.x;
                var under = headVertices.Select(i => skin.transform.TransformPoint(vertices[i])).Where(p =>
                    new Vector2(p.x - bone.position.x, p.z - bone.position.z).sqrMagnitude <= radius * radius).ToArray();
                Assert.That(under.Length, Is.GreaterThan(0));
                Assert.That(bone.position.y - under.Max(p => p.y), Is.EqualTo(.0005f).Within(.0001f),
                    "Actual baked crown must touch the horizontal vessel foot, on either slope sign and during recoil");
            }
            try
            {
            villager.Observe(Vector3.zero, 0, 1000, 111, 1);
            villager.Render(0);
            for (var i = 0; i < 20; i++)
            {
                villager.Root.transform.rotation = Quaternion.Euler(i % 2 == 0 ? -28 : 28, i * 3, i % 2 == 0 ? 15 : -15);
                villager.Root.transform.position += Vector3.forward * asset.runSpeed * .02f;
                // One impact then a progressing overlay; do not restart it on every sample.
                villager.Observe(villager.Root.transform.position, 0, 990, 110, (ulong)i + 2);
                villager.Render(.02f);
                Assert.That(villager.MovingHit, Is.True);
                Assert.That(Vector3.Angle(bone.TransformDirection(localUp), Vector3.up), Is.LessThan(.05f),
                    "Terrain tilt and moving recoil must not tip a balanced carried jar");
                CheckCrownContact();
            }
            villager.Observe(villager.Root.transform.position, 0, -1, 108, 22);
            if (voxel)
            {
                var time = villager.ClipTime;
                villager.Root.transform.rotation = Quaternion.Euler(-28, 95, 15);
                Assert.That(villager.SnapshotVoxelFragments().Count, Is.GreaterThan(0));
                Assert.That(villager.ClipTime, Is.EqualTo(time), "Fracture samples the last held pose without advancing Death");
                Assert.That(Vector3.Angle(bone.TransformDirection(localUp), Vector3.up), Is.LessThan(.05f));
                CheckCrownContact();
            }
            villager.Render(asset.Clip("Death").length);
            Assert.That(Vector3.Angle(bone.TransformDirection(localUp), Vector3.up), Is.GreaterThan(30),
                "The released jar must keep its authored fall rather than remain gravity-locked");
            }
            finally { UnityEngine.Object.DestroyImmediate(mesh); }
        }

        [Test]
        public void ConsecutiveStationaryHitsRestartTheAuthoredReaction()
        {
            var asset = Asset();
            Assert.That(asset, Is.Not.Null);
            using var villager = new ModernVillagerPresenter(asset, null, asset.height);
            villager.Observe(Vector3.zero, 0, 100, 111, 1);
            villager.Render(0);
            villager.Observe(Vector3.zero, 0, 90, 110, 2);
            villager.Render(.05f);
            Assert.That(villager.CurrentClip, Is.EqualTo("Hit"));
            Assert.That(villager.ClipTime, Is.EqualTo(.05f).Within(.001f));
            villager.Observe(Vector3.zero, 0, 80, 110, 3);
            villager.Render(.01f);
            Assert.That(villager.ClipTime, Is.EqualTo(.01f).Within(.001f), "A new native impact must restart an active stationary reaction");
        }

        [TestCase("Villager")]
        [TestCase("VillagerBrown")]
        [TestCase("VillagerStriped")]
        [TestCase("VillagerCarrier")]
        public void AuthoredGaitAdvancesByRenderedTravelAndMovingHitKeepsRunning(string name)
        {
            var asset = Asset(name);
            Assert.That(asset, Is.Not.Null);
            using var villager = new ModernVillagerPresenter(asset, null, asset.height);
            villager.Observe(Vector3.zero, 0, 100, 111, 1);
            villager.Render(0);
            villager.Root.transform.position = new Vector3(0, 0, asset.runSpeed * .1f);
            villager.Render(.1f);
            Assert.That(villager.CurrentClip, Is.EqualTo("Run"));
            Assert.That(villager.ClipTime, Is.EqualTo(.1f).Within(.001f));
            villager.Observe(villager.Root.transform.position, 0, 90, 110, 2);
            villager.Root.transform.position += Vector3.forward * asset.runSpeed * .1f;
            villager.Render(.1f);
            Assert.That(villager.CurrentClip, Is.EqualTo("Run"));
            Assert.That(villager.ClipTime, Is.EqualTo(.2f).Within(.001f));
            Assert.That(villager.MovingHit, Is.True);
            villager.Render(.02f);
            Assert.That(villager.CurrentClip, Is.EqualTo("Run"), "A held interpolation endpoint must not flash Idle");
            Assert.That(villager.ClipTime, Is.EqualTo(.2f).Within(.001f), "Feet cannot advance without source travel");
            villager.Observe(villager.Root.transform.position, 0, -1, 108, 3);
            Assert.That(villager.IsDying, Is.True);
            Assert.That(villager.CurrentClip, Is.EqualTo("Death"));
        }

        [TestCaseSource(nameof(ImportedActions))]
        public void ImportedVillagerHasFiniteGeometryAndAppropriateGroundContact(string name, string clip)
        {
            using var character = new ModernCharacterInstance(Asset(name), null);
            var skin = character.Root.GetComponentsInChildren<SkinnedMeshRenderer>(true).Single();
            var mesh = new Mesh();
            try
            {
                var animation = character.Asset.Clip(clip);
                var maximumFloor = float.NegativeInfinity;
                for (var i = 0; i <= 48; i++)
                {
                    character.Sample(clip, animation.length * i / 48f);
                    skin.BakeMesh(mesh, true);
                    var vertices = mesh.vertices.Select(skin.transform.TransformPoint).ToArray();
                    Assert.That(vertices.All(p => float.IsFinite(p.x) && float.IsFinite(p.y) && float.IsFinite(p.z)), Is.True);
                    var floor = vertices.Min(p => p.y);
                    Assert.That(floor, Is.GreaterThan(-.003f), "Imported geometry penetrates flat ground");
                    maximumFloor = Mathf.Max(maximumFloor, floor);
                    if (clip == "Walk" || clip == "Idle")
                        Assert.That(floor, Is.LessThan(.006f), "Supporting sole lost contact");
                    if (clip == "Death" && i == 48)
                        Assert.That(vertices.Max(p => p.y), Is.LessThan(character.Asset.height * .45f), "Death leaves the body suspended upright");
                }
                if (clip == "Run") Assert.That(maximumFloor, Is.GreaterThan(.01f), "Running requires a genuine airborne phase");
            }
            finally { UnityEngine.Object.DestroyImmediate(mesh); }
        }

        private static System.Collections.Generic.IEnumerable<TestCaseData> ImportedActions() =>
            ModernVillagerAppearances.All.SelectMany(binding => new[] { "Idle", "Walk", "Run", "Hit", "Death" }
                .Select(clip => new TestCaseData(binding.Resource, clip)));

        [TestCase(false)]
        [TestCase(true)]
        public void CaptureClearanceMeasuresActualPosedWorldSurface(bool voxel)
        {
            var asset = Asset();
            using var villager = new ModernVillagerPresenter(asset, null, asset.height);
            if (voxel) villager.AttachVoxelSkin(Resources.Load<MagicCarpet.Native.Voxels.VoxelGoatAsset>(
                "ModernCharacters/Villager/Voxels/Villager64"));
            villager.Observe(Vector3.zero, 0, 100, 111, 1);
            villager.Render(0);
            villager.Root.transform.position = new Vector3(20, 4, 30);
            Assert.That(villager.SurfaceGroundClearance((x, z) => 4), Is.EqualTo(.003f).Within(.0001f));
            villager.Root.transform.position += Vector3.up;
            Assert.That(villager.SurfaceGroundClearance((x, z) => 4), Is.EqualTo(1.003f).Within(.0001f));
            Assert.Throws<InvalidOperationException>(() => villager.SurfaceGroundClearance((x, z) => float.NaN));
        }

        [Test]
        public void FallenCostumeRestsOnUnevenTerrainWithoutChangingItsAuthoredPoseOrHorizontalPosition()
        {
            var asset = Asset();
            using var villager = new ModernVillagerPresenter(asset, null, asset.height);
            villager.Observe(Vector3.zero, 0, -1, 108, 1);
            villager.Render(asset.Clip("Death").length);
            float Ground(float x, float z) => .2f * (x * x + z * z);
            var before = villager.SurfaceGroundClearance(Ground);
            Assert.That(before, Is.LessThan(-.005f), "The curved surface must expose a real support defect");
            var time = villager.ClipTime;
            villager.GroundCorpseToTerrain(Ground, 1);
            Assert.That(villager.SurfaceGroundClearance(Ground), Is.EqualTo(.002f).Within(.0001f));
            var supported = villager.Root.transform.position;
            villager.GroundCorpseToTerrain(Ground, 1);
            Assert.That(villager.Root.transform.position, Is.EqualTo(supported), "Repeated camera/render visits cannot accumulate lift");
            Assert.That(supported.x, Is.Zero); Assert.That(supported.z, Is.Zero);
            Assert.That(villager.ClipTime, Is.EqualTo(time));
            villager.GroundCorpseToTerrain((x, z) => Ground(x, z) * 2, 2);
            Assert.That(villager.SurfaceGroundClearance((x, z) => Ground(x, z) * 2), Is.EqualTo(.002f).Within(.0001f),
                "A later terrain revision must refresh the settled corpse support");
        }

        [TestCase("Villager")]
        [TestCase("VillagerBrown")]
        [TestCase("VillagerStriped")]
        [TestCase("VillagerCarrier")]
        public void ImportedVillagerKeepsPhysicalMapsAndActualSurfaceNormals(string name)
        {
            var asset = Asset(name);
            Assert.That(asset, Is.Not.Null);
            var maps = asset.model.GetComponentsInChildren<Renderer>(true).SelectMany(r => r.sharedMaterials).Distinct().ToArray();
            Assert.That(maps.Length, Is.GreaterThanOrEqualTo(1));
            Assert.That(maps.All(m => m.shader.name == "Universal Render Pipeline/Lit"), Is.True);
            Assert.That(maps.All(m => m.GetTexture("_BaseMap") != null && m.GetTexture("_MetallicGlossMap") != null), Is.True,
                "A single atlas is valid; every used character material must retain its physical texture maps");
            Assert.That(maps.All(m => m.IsKeywordEnabled("_METALLICSPECGLOSSMAP") && m.IsKeywordEnabled("_OCCLUSIONMAP") &&
                m.GetTexture("_OcclusionMap") != null), Is.True, "Assigned maps must be enabled in the shader");
            var folder = "Assets/Resources/ModernCharacters/" + name + "/";
            var manifest = JsonUtility.FromJson<MaterialManifest>(File.ReadAllText(folder + name + ".clips.json"));
            foreach (var description in manifest.materials != null && manifest.materials.Length > 0 ? manifest.materials : new[] { manifest.material })
            {
                var fileName = string.Concat(description.name.Select(c => char.IsLetterOrDigit(c) ? c : '_')) + ".mat";
                var material = maps.Single(m => AssetDatabase.GetAssetPath(m).EndsWith("/" + fileName, StringComparison.Ordinal));
                if (string.IsNullOrWhiteSpace(description.normal))
                    Assert.That(material.IsKeywordEnabled("_NORMALMAP"), Is.False);
                else
                {
                    Assert.That(material.IsKeywordEnabled("_NORMALMAP"), Is.True);
                    Assert.That(material.GetTexture("_BumpMap"), Is.EqualTo(AssetDatabase.LoadAssetAtPath<Texture2D>(
                        folder + description.normal)));
                }
            }
            var skins = asset.model.GetComponentsInChildren<SkinnedMeshRenderer>(true);
            Assert.That(skins.Length, Is.GreaterThan(0));
            foreach (var skin in skins)
            {
                Assert.That(skin.sharedMesh.vertexCount, Is.GreaterThan(0));
                Assert.That(skin.sharedMesh.normals.Length, Is.EqualTo(skin.sharedMesh.vertexCount), "Geometry normals are required even when no normal texture is authored");
                Assert.That(skin.sharedMesh.normals.All(n => float.IsFinite(n.x) && float.IsFinite(n.y) && float.IsFinite(n.z) && n.sqrMagnitude > .000001f), Is.True);
            }
        }
    }
}
