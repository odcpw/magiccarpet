using System;
using System.Linq;
using NUnit.Framework;
using UnityEngine;

namespace MagicCarpet.Native.Tests
{
    public sealed class ModernGoatEditModeTests
    {
        private GameObject model;
        private ModernCharacterAsset asset;

        [SetUp]
        public void SetUp()
        {
            model = new GameObject("test goat");
            asset = ScriptableObject.CreateInstance<ModernCharacterAsset>();
            asset.model = model;
            asset.height = .95917f;
            asset.clips = new[] { Clip("Idle"), Clip("Walk"), Clip("Sprint"), Clip("Hit"), Clip("Death") };
        }

        private static AnimationClip Clip(string name)
        {
            var clip = new AnimationClip { name = name };
            clip.SetCurve("", typeof(Transform), "localPosition.x", AnimationCurve.Linear(0, 0, 1, 0));
            return clip;
        }

        [TearDown]
        public void TearDown()
        {
            foreach (var clip in asset.clips) UnityEngine.Object.DestroyImmediate(clip);
            UnityEngine.Object.DestroyImmediate(asset);
            UnityEngine.Object.DestroyImmediate(model);
        }

        [Test]
        public void GaitAdvancesWithActualRenderedTravelAndStops()
        {
            using var goat = new ModernGoatPresenter(asset, null, 150 / 64f);
            goat.Observe(Vector3.zero, 0, 600, 9, 1);
            goat.Render(.1f);
            Assert.AreEqual("Idle", goat.CurrentClip);
            goat.Root.transform.position = new Vector3(.2f, 0, 0);
            goat.Render(.1f);
            Assert.AreEqual("Walk", goat.CurrentClip);
            Assert.That(goat.ClipTime, Is.EqualTo(.2f / (.16f / .18f * (150 / 64f) / .95917f)).Within(.0001f));
            goat.Render(.1f);
            Assert.AreEqual("Idle", goat.CurrentClip);
            goat.Observe(new Vector3(.2f, 0, 0), 0, 600, 14, 2);
            goat.Root.transform.position += Vector3.right * .3f;
            goat.Render(.1f);
            Assert.AreEqual("Sprint", goat.CurrentClip);
        }

        [Test]
        public void DamageAndRemovalHaveBoundedReactionsAndCannotResurrect()
        {
            using var goat = new ModernGoatPresenter(asset, null, 150 / 64f);
            goat.Observe(Vector3.zero, 0, 600, 9, 1);
            goat.Observe(Vector3.zero, 0, 590, 14, 2);
            goat.Render(.1f);
            Assert.AreEqual("Hit", goat.CurrentClip);
            goat.Render(1f);
            goat.Render(.01f);
            Assert.AreEqual("Idle", goat.CurrentClip);
            goat.Observe(Vector3.zero, 0, 580, 14, 3);
            goat.Render(.1f);
            Assert.AreEqual("Hit", goat.CurrentClip);
            Assert.That(goat.ClipTime, Is.EqualTo(.1f).Within(.0001f), "A later hit must restart the reaction");
            goat.Observe(Vector3.zero, 0, -1, 12, 4);
            goat.MarkRemoved();
            Assert.IsTrue(goat.IsDying);
            Assert.IsFalse(goat.DeathFinished);
            goat.Render(1.3f);
            Assert.IsTrue(goat.DeathFinished);
            Assert.AreEqual("Death", goat.CurrentClip);
            Assert.Throws<InvalidOperationException>(() => goat.Observe(Vector3.zero, 0, 600, 9, 5));
        }

        [Test]
        public void MovingHitKeepsDistanceDrivenGaitAndAddsTheImportedHeadReaction()
        {
            var imported = Resources.Load<ModernCharacterAsset>("ModernCharacters/Goat/Character");
            Assert.That(imported, Is.Not.Null);
            using var hit = new ModernGoatPresenter(imported, null, 150 / 64f);
            using var control = new ModernGoatPresenter(imported, null, 150 / 64f);
            foreach (var goat in new[] { hit, control })
            {
                goat.Observe(Vector3.zero, 0, 600, 9, 1);
                goat.Render(0);
                goat.Root.transform.position = Vector3.forward * .2f;
                goat.Render(.1f);
            }
            hit.Observe(Vector3.forward * .2f, 0, 350, 14, 2);
            control.Observe(Vector3.forward * .2f, 0, 600, 14, 2);
            foreach (var goat in new[] { hit, control })
            {
                goat.Root.transform.position = Vector3.forward * .4f;
                goat.Render(.1f);
            }
            Assert.AreEqual("Sprint", hit.CurrentClip, "A moving hit must retain the native flee gait");
            Assert.IsTrue(hit.MovingHitActive);
            Assert.IsFalse(control.MovingHitActive);
            Assert.That(hit.ClipTime, Is.EqualTo(control.ClipTime).Within(.00001f), "Hit changed distance-driven gait phase");
            Assert.AreEqual(control.HoofStanceMask(), hit.HoofStanceMask());
            var hitBones = hit.Root.GetComponentsInChildren<Transform>(true);
            var controlBones = control.Root.GetComponentsInChildren<Transform>(true);
            var hitHead = hitBones.Single(b => b.name == "head");
            var controlHead = controlBones.Single(b => b.name == "head");
            Assert.That(Quaternion.Angle(hitHead.localRotation, controlHead.localRotation), Is.GreaterThan(2f),
                "The actual imported hit track did not produce head recoil");
            foreach (var name in new[] { "hind_L_hoof", "hind_R_hoof", "front_L_hoof", "front_R_hoof" })
            {
                var a = hitBones.Single(b => b.name == name).position;
                var b = controlBones.Single(bone => bone.name == name).position;
                Assert.That(Vector3.Distance(a, b), Is.LessThan(.0001f), "Hit changed the planted gait at " + name);
            }
            var heldPhase = hit.ClipTime;
            hit.Render(1f / 30f);
            Assert.AreEqual("Sprint", hit.CurrentClip, "An interpolation boundary must not switch to the stationary hit");
            Assert.IsTrue(hit.MovingHitActive);
            Assert.That(hit.ClipTime, Is.EqualTo(heldPhase).Within(.00001f), "Gait advanced without travel");
            hit.Render(.09f);
            Assert.AreEqual("Hit", hit.CurrentClip, "A genuinely stopped goat must use its stationary reaction");
            Assert.IsFalse(hit.MovingHitActive);
            hit.Observe(Vector3.forward * .4f, 0, 250, 14, 3);
            hit.Root.transform.position = Vector3.forward * .5f;
            hit.Render(.05f);
            Assert.AreEqual("Sprint", hit.CurrentClip, "Repeated moving hits must not freeze the gait");
            Assert.IsTrue(hit.MovingHitActive);
        }

        [Test]
        public void WorldSeamUsesShortTravelAndStaleTicksAreRejected()
        {
            Assert.That(ModernGoatPresenter.HorizontalTravel(new Vector3(1023.9f, 0, 0), new Vector3(.1f, 0, 0)), Is.EqualTo(.2f).Within(.0001f));
            using var goat = new ModernGoatPresenter(asset, null, 150 / 64f);
            goat.Observe(Vector3.zero, 0, 600, 9, 3);
            Assert.Throws<ArgumentOutOfRangeException>(() => goat.Observe(Vector3.zero, 0, 600, 9, 2));
        }

        [Test]
        public void ZeroLifeWithoutDeathActionDoesNotInventDeath()
        {
            using var goat = new ModernGoatPresenter(asset, null, 150 / 64f);
            goat.Observe(Vector3.zero, 0, 0, 9, 1);
            goat.Render(.1f);
            Assert.IsFalse(goat.IsDying);
            Assert.AreEqual("Idle", goat.CurrentClip);
        }

        [Test]
        public void ImportedFrontFacesTheNativeMovementDirectionAtCardinalYaws()
        {
            var imported = Resources.Load<ModernCharacterAsset>("ModernCharacters/Goat/Character");
            Assert.That(imported, Is.Not.Null, "Prepare the owner-local goat before this test");
            using var goat = new ModernCharacterInstance(imported, null);
            goat.Sample("Idle", 0);
            var bones = goat.Root.GetComponentsInChildren<Transform>(true);
            var front = bones.Where(b => b.name == "front_L_hoof" || b.name == "front_R_hoof").ToArray();
            var hind = bones.Where(b => b.name == "hind_L_hoof" || b.name == "hind_R_hoof").ToArray();
            Assert.That(front.Length, Is.EqualTo(2));
            Assert.That(hind.Length, Is.EqualTo(2));
            foreach (var yaw in new[] { 0, 512, 1024, 1536 })
            {
                goat.Root.transform.rotation = Quaternion.Euler(0, 180f - yaw * 360f / 2048f, 0);
                var fore = (front[0].position + front[1].position) * .5f;
                var rear = (hind[0].position + hind[1].position) * .5f;
                var direction = fore - rear;
                direction.y = 0;
                var radians = yaw * Mathf.PI / 1024f;
                var nativeMovement = new Vector3(Mathf.Sin(radians), 0, -Mathf.Cos(radians));
                Assert.That(direction.magnitude, Is.GreaterThan(.2f));
                Assert.That(Vector3.Dot(direction.normalized, nativeMovement), Is.GreaterThan(.98f),
                    "Imported forequarters point away from source travel at yaw " + yaw);
            }
        }

        [Test]
        public void CaptureContactMeasuresAllFourSolesAgainstActualTerrainHeight()
        {
            var imported = Resources.Load<ModernCharacterAsset>("ModernCharacters/Goat/Character");
            Assert.That(imported, Is.Not.Null, "Prepare the owner-local goat before this test");
            using var goat = new ModernGoatPresenter(imported, null, 150 / 64f);
            goat.Observe(Vector3.zero, 0, 600, 8, 1);
            goat.Render(0);
            Assert.AreEqual(15, goat.HoofStanceMask());
            var flat = goat.HoofClearances((x, z) => 0);
            var raised = goat.HoofClearances((x, z) => 1);
            Assert.That(flat.Length, Is.EqualTo(4));
            for (var leg = 0; leg < 4; leg++)
            {
                Assert.That(flat[leg], Is.InRange(-.003f, .01f), "Idle hoof " + leg);
                Assert.That(raised[leg], Is.EqualTo(flat[leg] - 1).Within(.0001f),
                    "Contact measurement must query the supplied terrain");
            }
        }

        [TestCase(0f, 0f)]
        [TestCase(.035f, .025f)]
        public void GroundCorrectionPlantsIdleHoovesWithoutStretchingOrMovingRoot(float slopeX, float slopeZ)
        {
            var imported = Resources.Load<ModernCharacterAsset>("ModernCharacters/Goat/Character");
            Assert.That(imported, Is.Not.Null);
            using var goat = new ModernGoatPresenter(imported, null, 150 / 64f);
            goat.Observe(Vector3.zero, 0, 600, 8, 1);
            goat.Root.transform.position = new Vector3(0, -.08f, 0);
            goat.Render(0);
            float Terrain(float x, float z) => slopeX * x + slopeZ * z;
            var before = goat.HoofClearances(Terrain);
            Assert.That(before.Min(), Is.LessThan(-.04f), "Negative control must start below terrain");
            var lengths = LegLengths(goat.Root);
            var root = goat.Root.transform.position;
            Assert.IsTrue(goat.GroundToTerrain(Terrain), "Modest terrain must be reachable");
            Assert.That(Vector3.Distance(root, goat.Root.transform.position), Is.LessThan(.000001f));
            foreach (var clearance in goat.HoofClearances(Terrain))
                Assert.That(clearance, Is.InRange(-.005f, .01f), "Corrected idle hoof misses terrain");
            var after = LegLengths(goat.Root);
            for (var i = 0; i < lengths.Length; i++)
                Assert.That(after[i], Is.EqualTo(lengths[i]).Within(.0001f), "IK stretched a leg bone");
        }

        [Test]
        public void SteepTerrainNormalKeepsBothEndsReachableUnlikeTheOldTiltCap()
        {
            var imported = Resources.Load<ModernCharacterAsset>("ModernCharacters/Goat/Character");
            Assert.That(imported, Is.Not.Null);
            var rise = Mathf.Tan(51f * Mathf.Deg2Rad);
            float Terrain(float x, float z) => rise * z;
            using var matched = new ModernGoatPresenter(imported, null, 150 / 64f);
            matched.Root.transform.rotation = Quaternion.FromToRotation(Vector3.up, new Vector3(0, 1, -rise).normalized);
            matched.Observe(Vector3.zero, 0, 600, 8, 1);
            matched.Render(0);
            var lengths = LegLengths(matched.Root);
            Assert.IsTrue(matched.GroundToTerrain(Terrain), "Real 51-degree ground normal must remain reachable");
            foreach (var clearance in matched.HoofClearances(Terrain))
                Assert.That(clearance, Is.InRange(-.005f, .01f), "Matched slope misses support");
            var after = LegLengths(matched.Root);
            for (var i = 0; i < lengths.Length; i++)
                Assert.That(after[i], Is.EqualTo(lengths[i]).Within(.0001f), "Steep slope stretched a bone");

            using var capped = new ModernGoatPresenter(imported, null, 150 / 64f);
            capped.Root.transform.rotation = Quaternion.FromToRotation(Vector3.up,
                new Vector3(0, 1, -Mathf.Tan(35f * Mathf.Deg2Rad)).normalized);
            capped.Observe(Vector3.zero, 0, 600, 8, 1);
            capped.Render(0);
            capped.GroundToTerrain(Terrain);
            Assert.That(capped.HoofClearances(Terrain).Max(Mathf.Abs), Is.GreaterThan(.03f),
                "The previous 35-degree cap must fail this steep terrain control");
        }

        [Test]
        public void GroundCorrectionPreservesAirborneGaitAndReportsUnreachableTerrain()
        {
            var imported = Resources.Load<ModernCharacterAsset>("ModernCharacters/Goat/Character");
            Assert.That(imported, Is.Not.Null);
            using var goat = new ModernGoatPresenter(imported, null, 150 / 64f);
            goat.Observe(Vector3.zero, 0, 600, 9, 1);
            goat.Render(0);
            var cycle = .16f / .18f * (150 / 64f) / .95917f;
            goat.Root.transform.position = Vector3.forward * (cycle * .3f);
            goat.Render(.1f);
            Assert.AreEqual(0, goat.HoofStanceMask(), "Control must sample the airborne phase");
            var bones = goat.Root.GetComponentsInChildren<Transform>(true)
                .Where(t => t.name.EndsWith("_hoof", StringComparison.Ordinal)).ToArray();
            var before = bones.Select(b => b.position).ToArray();
            Assert.IsTrue(goat.GroundToTerrain((x, z) => -1));
            for (var i = 0; i < bones.Length; i++)
                Assert.That(Vector3.Distance(before[i], bones[i].position), Is.LessThan(.00001f), "Airborne hoof moved without a collision");
            var hind = bones.Single(b => b.name == "hind_L_hoof");
            var center = hind.position;
            float Bump(float x, float z) => (new Vector2(x - center.x, z - center.z)).sqrMagnitude < .16f * .16f
                ? center.y - .08f : -1;
            var bumpBefore = goat.HoofClearances(Bump);
            Assert.That(bumpBefore[0], Is.LessThan(-.02f), "Uneven negative control must penetrate one airborne hoof");
            var otherBefore = bones.Where(b => b != hind).Select(b => b.position).ToArray();
            Assert.IsTrue(goat.GroundToTerrain(Bump));
            Assert.That(goat.HoofClearances(Bump)[0], Is.GreaterThan(-.005f), "Airborne hoof did not clear the bump");
            var others = bones.Where(b => b != hind).ToArray();
            for (var i = 0; i < others.Length; i++)
                Assert.That(Vector3.Distance(otherBefore[i], others[i].position), Is.LessThan(.00001f), "Unobstructed airborne hoof moved");
            float HighBump(float x, float z) => (new Vector2(x - center.x, z - center.z)).sqrMagnitude < .16f * .16f
                ? center.y + .23f : -1;
            Assert.That(goat.HoofClearances(HighBump)[0], Is.LessThan(-.25f), "Sharp-step control must exceed the old cap");
            Assert.IsTrue(goat.GroundToTerrain(HighBump), "Reachable high step must not be rejected by the correction bound");
            Assert.That(goat.HoofClearances(HighBump)[0], Is.GreaterThan(-.005f), "Airborne hoof still intersects the high step");
            var lengths = LegLengths(goat.Root);
            Assert.IsFalse(goat.GroundToTerrain((x, z) => 10), "Unreachable terrain must not report complete grounding");
            var after = LegLengths(goat.Root);
            for (var i = 0; i < lengths.Length; i++)
                Assert.That(after[i], Is.EqualTo(lengths[i]).Within(.0001f), "Unreachable target stretched a bone");
        }

        private static float[] LegLengths(GameObject root)
        {
            var bones = root.GetComponentsInChildren<Transform>(true);
            return new[] { "hind_L", "hind_R", "front_L", "front_R" }.SelectMany(name => {
                var upper = bones.Single(b => b.name == name + "_upper");
                var lower = bones.Single(b => b.name == name + "_lower");
                var hoof = bones.Single(b => b.name == name + "_hoof");
                return new[] { Vector3.Distance(upper.position, lower.position), Vector3.Distance(lower.position, hoof.position) };
            }).ToArray();
        }

        [TestCase("Walk", .18f)]
        [TestCase("Sprint", .075f)]
        public void ImportedGallopHasPlantedStancesAndAerialPhase(string clip, float stance)
        {
            var imported = Resources.Load<ModernCharacterAsset>("ModernCharacters/Goat/Character");
            Assert.That(imported, Is.Not.Null, "Prepare the owner-local goat before this test");
            using var goat = new ModernCharacterInstance(imported, null);
            var renderer = goat.Root.GetComponentsInChildren<SkinnedMeshRenderer>(true)
                .OrderByDescending(r => r.sharedMesh.vertexCount).First();
            var mesh = new Mesh();
            try
            {
                var names = new[] { "hind_L_hoof", "hind_R_hoof", "front_L_hoof", "front_R_hoof" };
                var phases = new[] { 0f, .035f, .48f, .515f };
                var weights = renderer.sharedMesh.boneWeights;
                var groups = names.Select(name => {
                    var bone = Array.FindIndex(renderer.bones, b => b.name == name);
                    Assert.That(bone, Is.GreaterThanOrEqualTo(0));
                    var ids = Enumerable.Range(0, weights.Length).Where(i => WeightOfBone(weights[i], bone) > .999f).ToArray();
                    Assert.That(ids.Length, Is.GreaterThan(10));
                    return ids;
                }).ToArray();
                var startPlus = new Vector3?[4]; var startMinus = new Vector3?[4];
                var driftPlus = 0f; var driftMinus = 0f; var minimum = float.PositiveInfinity;
                var maximum = float.NegativeInfinity; var counts = new int[4];
                var lastSample = new[] { -2, -2, -2, -2 }; var completed = new bool[4];
                Vector3[] first = null, last = null;
                for (var sample = 0; sample <= 96; sample++)
                {
                    var t = sample / 96f;
                    goat.Sample(clip, imported.Clip(clip).length * t);
                    renderer.BakeMesh(mesh, true);
                    var points = mesh.vertices.Select(v => renderer.transform.TransformPoint(v)).ToArray();
                    Assert.That(points.All(v => float.IsFinite(v.x) && float.IsFinite(v.y) && float.IsFinite(v.z)), Is.True);
                    var floor = points.Min(v => v.y);
                    minimum = Mathf.Min(minimum, floor); maximum = Mathf.Max(maximum, floor);
                    first ??= points; last = points;
                    for (var leg = 0; leg < names.Length; leg++)
                    {
                        if (Mathf.Repeat(t + phases[leg], 1) >= stance || t >= 1) continue;
                        if (counts[leg] > 0 && sample != lastSample[leg] + 1) completed[leg] = true;
                        if (completed[leg]) continue;
                        var centroid = groups[leg].Aggregate(Vector3.zero, (sum, id) => sum + points[id]) / groups[leg].Length;
                        var sole = groups[leg].Min(id => points[id].y);
                        Assert.That(sole, Is.InRange(-.003f, .008f), names[leg] + " stance height");
                        // Import axis conversion may reverse Z; all four feet must
                        // agree on one treadmill direction and remain stationary.
                        var travel = Vector3.forward * (t * .16f / stance);
                        var plus = centroid + travel; var minus = centroid - travel;
                        if (!startPlus[leg].HasValue) { startPlus[leg] = plus; startMinus[leg] = minus; }
                        driftPlus = Mathf.Max(driftPlus, Vector3.Distance(startPlus[leg].Value, plus));
                        driftMinus = Mathf.Max(driftMinus, Vector3.Distance(startMinus[leg].Value, minus));
                        counts[leg]++;
                        lastSample[leg] = sample;
                    }
                }
                Assert.That(counts.All(c => c >= 4), Is.True, "Every hoof must have sampled stance");
                Assert.That(Mathf.Min(driftPlus, driftMinus), Is.LessThan(.005f), "Hoof slides on the moving ground");
                Assert.That(minimum, Is.GreaterThan(-.003f), "Gallop penetrates the ground");
                Assert.That(maximum, Is.GreaterThan(.01f), "Gallop has no airborne phase");
                Assert.That(first.Zip(last, Vector3.Distance).Max(), Is.LessThan(.0001f), "Gallop does not loop");
            }
            finally { UnityEngine.Object.DestroyImmediate(mesh); }
        }

        private static float WeightOfBone(BoneWeight value, int bone)
        {
            return (value.boneIndex0 == bone ? value.weight0 : 0) +
                   (value.boneIndex1 == bone ? value.weight1 : 0) +
                   (value.boneIndex2 == bone ? value.weight2 : 0) +
                   (value.boneIndex3 == bone ? value.weight3 : 0);
        }

        [TestCase("Hit", .02f)]
        [TestCase("Death", .1f)]
        public void ImportedReactionMovesWithoutPenetratingGround(string clip, float minimumMotion)
        {
            var imported = Resources.Load<ModernCharacterAsset>("ModernCharacters/Goat/Character");
            Assert.That(imported, Is.Not.Null, "Prepare the owner-local goat before this test");
            using var goat = new ModernCharacterInstance(imported, null);
            var renderer = goat.Root.GetComponentsInChildren<SkinnedMeshRenderer>(true)
                .OrderByDescending(r => r.sharedMesh.vertexCount).First();
            var mesh = new Mesh();
            try
            {
                var length = imported.Clip(clip).length;
                Assert.That(length, Is.GreaterThan(0));
                var samples = Mathf.CeilToInt(length * 96);
                Vector3[] first = null;
                var minimum = float.PositiveInfinity; var motion = 0f;
                for (var sample = 0; sample <= samples; sample++)
                {
                    goat.Sample(clip, length * sample / samples);
                    renderer.BakeMesh(mesh, true);
                    var points = mesh.vertices.Select(v => renderer.transform.TransformPoint(v)).ToArray();
                    minimum = Mathf.Min(minimum, points.Min(v => v.y));
                    first ??= points;
                    motion = Mathf.Max(motion, first.Zip(points, Vector3.Distance).Max());
                }
                Assert.That(minimum, Is.GreaterThan(-.003f), clip + " penetrates the ground");
                Assert.That(motion, Is.GreaterThan(minimumMotion), clip + " contains no meaningful reaction");
            }
            finally { UnityEngine.Object.DestroyImmediate(mesh); }
        }
    }
}
