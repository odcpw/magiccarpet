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
        private static ModernCharacterAsset Asset() => Resources.Load<ModernCharacterAsset>("ModernCharacters/Villager/Character");
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
                    "\",\"commands\":[{\"longitudinal\":\"accelerate\",\"lateral\":\"negative\",\"cast_selected\":true,\"roll_axis\":24,\"pitch_axis\":-127,\"look_yaw\":69,\"look_pitch\":365}]}");
                var args = new[] { "-mc-input-playback", commands, "-mc-runtime-replay-load", replay };
                var stream = new CampaignInputPlayback(args);
                Assert.Throws<InvalidOperationException>(() => stream.At(2, 444));
                var frame = stream.At(2, 443);
                Assert.That(frame.Flags, Is.EqualTo(69));
                Assert.That(frame.look_pitch, Is.EqualTo(365));
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
        public void OnlyReviewedVillagerAppearanceReplacesOriginalSprite()
        {
            var row = new NativeSim.CampaignCharacterState { classId = 5, model = 13, renderParameter = 241 };
            Assert.That(ModernCharacterActors.IsSupportedActor(row), Is.True);
            foreach (var look in new ushort[] { 239, 242, 271, 0, 65535 })
            {
                row.renderParameter = look;
                Assert.That(ModernCharacterActors.IsSupportedActor(row), Is.False);
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

        [Test]
        public void AuthoredGaitAdvancesByRenderedTravelAndMovingHitKeepsRunning()
        {
            var asset = Asset();
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

        [TestCase("Idle")]
        [TestCase("Walk")]
        [TestCase("Run")]
        [TestCase("Death")]
        public void ImportedVillagerHasFiniteGeometryAndAppropriateGroundContact(string clip)
        {
            using var character = new ModernCharacterInstance(Asset(), null);
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

        [Test]
        public void ImportedVillagerKeepsPhysicalMapsAndActualSurfaceNormals()
        {
            var asset = Asset();
            Assert.That(asset, Is.Not.Null);
            var maps = asset.model.GetComponentsInChildren<Renderer>(true).SelectMany(r => r.sharedMaterials).Distinct().ToArray();
            Assert.That(maps.Length, Is.GreaterThanOrEqualTo(1));
            Assert.That(maps.All(m => m.shader.name == "Universal Render Pipeline/Lit"), Is.True);
            Assert.That(maps.All(m => m.GetTexture("_BaseMap") != null && m.GetTexture("_MetallicGlossMap") != null), Is.True,
                "A single atlas is valid; every used character material must retain its physical texture maps");
            Assert.That(maps.All(m => m.IsKeywordEnabled("_METALLICSPECGLOSSMAP") && m.IsKeywordEnabled("_OCCLUSIONMAP") &&
                m.GetTexture("_OcclusionMap") != null), Is.True, "Assigned maps must be enabled in the shader");
            var manifest = JsonUtility.FromJson<MaterialManifest>(File.ReadAllText("Assets/Resources/ModernCharacters/Villager/Villager.clips.json"));
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
                        "Assets/Resources/ModernCharacters/Villager/" + description.normal)));
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
