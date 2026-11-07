using System;
using System.Linq;
using NUnit.Framework;
using UnityEngine;

namespace MagicCarpet.Native.Tests
{
    public sealed class ModernCharacterEditModeTests
    {
        private static ModernCharacterAsset Asset(string name)
        {
            var asset = Resources.Load<ModernCharacterAsset>("ModernCharacters/" + name + "/Character");
            Assert.That(asset, Is.Not.Null, "Prepare the owner-local character assets before this test");
            return asset;
        }

        [TestCase("Goat", 2)]
        [TestCase("Archer", 6)]
        public void ImportedModelHasNamedClipsAndTexturedUrpMaterials(string name, int count)
        {
            var asset = Asset(name);
            Assert.That(asset.clips.Length, Is.EqualTo(count));
            Assert.That(asset.Clip("Idle").isLooping, Is.True);
            Assert.That(asset.Clip("Walk").length, Is.EqualTo(1).Within(.001));
            var materials = asset.model.GetComponentsInChildren<Renderer>(true).SelectMany(r => r.sharedMaterials).ToArray();
            Assert.That(materials.All(m => m.shader.name == "Universal Render Pipeline/Lit"), Is.True);
            Assert.That(materials.Any(m => m.GetTexture("_BaseMap") != null && m.GetTexture("_BumpMap") != null), Is.True,
                "Embedded FBX textures previously disappeared in Unity; require actual imported maps");
        }

        [TestCase("Goat")]
        [TestCase("Archer")]
        public void NativeImportedWalkRetainsGroundContactAndLoop(string name)
        {
            using var character = new ModernCharacterInstance(Asset(name), null);
            var renderer = character.Root.GetComponentsInChildren<SkinnedMeshRenderer>(true)
                .OrderByDescending(r => r.sharedMesh.vertexCount).First();
            var mesh = new Mesh();
            try
            {
                Vector3[] first = null, last = null;
                var low = float.PositiveInfinity; var high = float.NegativeInfinity;
                for (var sample = 0; sample <= 96; sample++)
                {
                    character.Sample("Walk", sample / 96f);
                    // Compensate importer scale before converting baked local vertices to world.
                    // The goat renderer is scaled 100x; default BakeMesh would apply that twice.
                    renderer.BakeMesh(mesh, true);
                    var vertices = mesh.vertices.Select(v => renderer.transform.TransformPoint(v)).ToArray();
                    Assert.That(vertices.All(v => float.IsFinite(v.x) && float.IsFinite(v.y) && float.IsFinite(v.z)), Is.True);
                    var floor = vertices.Min(v => v.y);
                    Assert.That(vertices.Max(v => v.y) - floor,
                        Is.InRange(character.Asset.height * .8f, character.Asset.height * 1.4f), "Imported character units differ from its authored size");
                    low = Mathf.Min(low, floor); high = Mathf.Max(high, floor);
                    first ??= vertices; last = vertices;
                }
                TestContext.WriteLine($"{name} sole range: {low:R}..{high:R}");
                Assert.That(low, Is.GreaterThan(-.001f), "Imported walk penetrates the floor");
                Assert.That(high, Is.LessThan(.003f), "Imported walk loses supporting-foot contact");
                Assert.That(first.Zip(last, Vector3.Distance).Max(), Is.LessThan(.0001f));
            }
            finally { UnityEngine.Object.DestroyImmediate(mesh); }
        }

        [Test]
        public void GoatIdlePlantsAllFourHooves()
        {
            using var character = new ModernCharacterInstance(Asset("Goat"), null);
            var renderer = character.Root.GetComponentsInChildren<SkinnedMeshRenderer>(true).First();
            var mesh = new Mesh();
            try
            {
                character.Sample("Idle", .5f); renderer.BakeMesh(mesh, true);
                TestContext.WriteLine($"Goat posed mesh bounds={mesh.bounds} renderer_scale={renderer.transform.lossyScale} rootbone={renderer.rootBone?.position} rootbone_scale={renderer.rootBone?.lossyScale}");
                foreach (var bone in renderer.bones.Where(b => b.name == "body" || b.name == "front_L_hoof"))
                    TestContext.WriteLine($"{bone.name} world={bone.position} local={bone.localPosition} scale={bone.lossyScale}");
                var vertices = mesh.vertices;
                var weights = renderer.sharedMesh.boneWeights;
                foreach (var name in new[] { "front_L_hoof", "front_R_hoof", "hind_L_hoof", "hind_R_hoof" })
                {
                    var index = Array.FindIndex(renderer.bones, bone => bone.name == name);
                    Assert.That(index, Is.GreaterThanOrEqualTo(0));
                    var ids = Enumerable.Range(0, weights.Length).Where(i => Weight(weights[i], index) > .5f).ToArray();
                    Assert.That(ids.Length, Is.GreaterThan(0));
                    var floor = ids.Min(i => renderer.transform.TransformPoint(vertices[i]).y);
                    TestContext.WriteLine(name + " sole=" + floor.ToString("R"));
                    Assert.That(floor, Is.InRange(-.001f, .005f), "Idle hoof is floating or penetrating");
                }
            }
            finally { UnityEngine.Object.DestroyImmediate(mesh); }
        }

        [TestCase("KneelEnter")]
        [TestCase("KneelExit")]
        public void ImportedKneelTransitionKeepsSupportingContact(string clip)
        {
            using var character = new ModernCharacterInstance(Asset("Archer"), null);
            var renderer = character.Root.GetComponentsInChildren<SkinnedMeshRenderer>(true)
                .OrderByDescending(r => r.sharedMesh.vertexCount).First();
            var mesh = new Mesh();
            try
            {
                var length = character.Asset.Clip(clip).length;
                var samples = Mathf.CeilToInt(length * 96);
                var low = float.PositiveInfinity; var high = float.NegativeInfinity;
                for (var i = 0; i <= samples; i++)
                {
                    character.Sample(clip, length * i / samples); renderer.BakeMesh(mesh, true);
                    var floor = mesh.vertices.Min(v => renderer.transform.TransformPoint(v).y);
                    low = Mathf.Min(low, floor); high = Mathf.Max(high, floor);
                }
                TestContext.WriteLine($"{clip} surface range: {low:R}..{high:R}");
                Assert.That(low, Is.GreaterThan(-.001f), "Transition penetrates the floor between held poses");
                Assert.That(high, Is.LessThan(.005f), "Transition floats above its supporting foot/knee");
            }
            finally { UnityEngine.Object.DestroyImmediate(mesh); }
        }

        [Test]
        public void ArcherIdlePlantsBothShoes()
        {
            using var character = new ModernCharacterInstance(Asset("Archer"), null);
            var body = character.Root.GetComponentsInChildren<SkinnedMeshRenderer>(true).OrderByDescending(r => r.sharedMesh.vertexCount).First();
            var mesh = new Mesh();
            try
            {
                character.Sample("Idle", .5f); body.BakeMesh(mesh, true);
                foreach (var side in new[] { "Left", "Right" })
                {
                    var ids = ShoeIds(body, side);
                    var vertices = mesh.vertices;
                    var floor = ids.Min(i => body.transform.TransformPoint(vertices[i]).y);
                    TestContext.WriteLine($"Idle {side} shoe sole={floor:R}");
                    Assert.That(floor, Is.InRange(-.001f, .005f), "Idle shoe floats above its footprint");
                }
            }
            finally { UnityEngine.Object.DestroyImmediate(mesh); }
        }

        [Test]
        public void RiseKeepsFrontShoePlanted()
        {
            using var character = new ModernCharacterInstance(Asset("Archer"), null);
            var body = character.Root.GetComponentsInChildren<SkinnedMeshRenderer>(true).OrderByDescending(r => r.sharedMesh.vertexCount).First();
            var ids = ShoeIds(body, "Left");
            var mesh = new Mesh();
            try
            {
                var length = character.Asset.Clip("KneelExit").length;
                var samples = Mathf.CeilToInt(length * 96);
                var first = Vector3.zero; var drift = 0f;
                var low = float.PositiveInfinity; var high = float.NegativeInfinity;
                for (var i = 0; i <= samples; i++)
                {
                    character.Sample("KneelExit", length * i / samples); body.BakeMesh(mesh, true);
                    var vertices = mesh.vertices;
                    var points = ids.Select(index => body.transform.TransformPoint(vertices[index])).ToArray();
                    var center = points.Aggregate(Vector3.zero, (sum, p) => sum + p) / points.Length;
                    center.y = 0;
                    if (i == 0) first = center;
                    drift = Mathf.Max(drift, Vector3.Distance(first, center));
                    var floor = points.Min(p => p.y);
                    low = Mathf.Min(low, floor); high = Mathf.Max(high, floor);
                }
                TestContext.WriteLine($"Rise front sole={low:R}..{high:R}, horizontal drift={drift:R}");
                Assert.That(low, Is.GreaterThan(-.001f));
                Assert.That(high, Is.LessThan(.006f), "Front support shoe lifts during the rise");
                Assert.That(drift, Is.LessThan(.02f), "Front support shoe slides during the rise");
            }
            finally { UnityEngine.Object.DestroyImmediate(mesh); }
        }

        private static int[] ShoeIds(SkinnedMeshRenderer body, string side)
        {
            var foot = Array.FindIndex(body.bones, bone => bone.name == side + "Foot");
            var toe = Array.FindIndex(body.bones, bone => bone.name == side + "ToeBase");
            var weights = body.sharedMesh.boneWeights;
            var ids = Enumerable.Range(0, weights.Length).Where(i => Weight(weights[i], foot) + Weight(weights[i], toe) > .5f).ToArray();
            Assert.That(ids.Length, Is.GreaterThan(0));
            return ids;
        }

        [TestCase("StandingShot", 1.75f)]
        [TestCase("StandingShot", 2.35f)]
        [TestCase("KneelingShot", .6f)]
        public void ImportedNockAndGripMeetTheVisibleHands(string clip, float time)
        {
            using var character = new ModernCharacterInstance(Asset("Archer"), null);
            var body = character.Root.GetComponentsInChildren<SkinnedMeshRenderer>(true)
                .OrderByDescending(r => r.sharedMesh.vertexCount).First();
            var left = FingerIds(body, "Left", .10f, .14f);
            var right = FingerIds(body, "Right", .13f, .17f);
            var bow = character.Root.GetComponentsInChildren<Transform>(true).Single(t => t.name == "BowControl");
            var bowstring = character.Root.GetComponentsInChildren<SkinnedMeshRenderer>(true)
                .Single(r => r.name == "Bowstring");
            var basis = bowstring.sharedMesh.vertices;
            var extent = bowstring.sharedMesh.bounds.size;
            var axis = extent.x > extent.y && extent.x > extent.z ? 0 : extent.y > extent.z ? 1 : 2;
            var center = (basis.Max(v => v[axis]) + basis.Min(v => v[axis])) * .5f;
            var distance = basis.Min(v => Mathf.Abs(v[axis] - center));
            var middle = Enumerable.Range(0, basis.Length).Where(i => Mathf.Abs(basis[i][axis] - center) < distance + .0001f).ToArray();
            var bodyMesh = new Mesh(); var stringMesh = new Mesh();
            try
            {
                character.Sample(clip, time);
                body.BakeMesh(bodyMesh, true); bowstring.BakeMesh(stringMesh, true);
                var vertices = bodyMesh.vertices;
                var l = left.Aggregate(Vector3.zero, (sum, i) => sum + body.transform.TransformPoint(vertices[i])) / left.Length;
                var r = right.Aggregate(Vector3.zero, (sum, i) => sum + body.transform.TransformPoint(vertices[i])) / right.Length;
                var stringVertices = stringMesh.vertices;
                var nock = middle.Aggregate(Vector3.zero, (sum, i) => sum + bowstring.transform.TransformPoint(stringVertices[i])) / middle.Length;
                TestContext.WriteLine($"{clip}@{time}: grip={Vector3.Distance(l, bow.position):R} nock={Vector3.Distance(r, nock):R}");
                Assert.That(Vector3.Distance(l, bow.position), Is.LessThan(.004f));
                Assert.That(Vector3.Distance(r, nock), Is.LessThan(.004f), "FBX morph or clip timing lost the drawing-hand attachment");
            }
            finally { UnityEngine.Object.DestroyImmediate(bodyMesh); UnityEngine.Object.DestroyImmediate(stringMesh); }
        }

        private static int[] FingerIds(SkinnedMeshRenderer body, string side, float low, float high)
        {
            var bones = body.bones; var bind = body.sharedMesh.bindposes;
            var hand = Array.FindIndex(bones, b => b.name == side + "Hand");
            var fore = Array.FindIndex(bones, b => b.name == side + "ForeArm");
            var wrist = body.transform.localToWorldMatrix.MultiplyPoint3x4(bind[hand].inverse.MultiplyPoint3x4(Vector3.zero));
            var elbow = body.transform.localToWorldMatrix.MultiplyPoint3x4(bind[fore].inverse.MultiplyPoint3x4(Vector3.zero));
            var axis = (wrist - elbow).normalized; var scale = Vector3.Distance(wrist, elbow) / .237f;
            var vertices = body.sharedMesh.vertices; var weights = body.sharedMesh.boneWeights;
            var ids = Enumerable.Range(0, vertices.Length).Where(i => {
                var along = Vector3.Dot(body.transform.TransformPoint(vertices[i]) - wrist, axis);
                return Weight(weights[i], hand) > .5f && along > low * scale && along < high * scale;
            }).ToArray();
            Assert.That(ids.Length, Is.GreaterThan(4), "No visible finger region in imported skin");
            return ids;
        }

        private static float Weight(BoneWeight w, int index) =>
            (w.boneIndex0 == index ? w.weight0 : 0) + (w.boneIndex1 == index ? w.weight1 : 0) +
            (w.boneIndex2 == index ? w.weight2 : 0) + (w.boneIndex3 == index ? w.weight3 : 0);
    }
}
