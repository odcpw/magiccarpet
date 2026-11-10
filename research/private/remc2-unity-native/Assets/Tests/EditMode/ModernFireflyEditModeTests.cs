using System;
using System.Linq;
using MagicCarpet.Native.Voxels;
using NUnit.Framework;
using UnityEditor;
using UnityEngine;

namespace MagicCarpet.Native.Tests
{
    public sealed class ModernFireflyEditModeTests
    {
        private GameObject model;
        private ModernCharacterAsset asset;

        [SetUp]
        public void SetUp()
        {
            model = new GameObject("Firefly test rig");
            asset = ScriptableObject.CreateInstance<ModernCharacterAsset>();
            asset.model = model;
            asset.height = 1;
            asset.forwardYaw = 0;
            asset.clips = new[] { Clip("Idle", true), Clip("Flight", true), Clip("Attack", false),
                Clip("Hit", false), Clip("Death", false) };
        }

        private static AnimationClip Clip(string name, bool loop)
        {
            var clip = new AnimationClip { name = name };
            clip.SetCurve("", typeof(Transform), "localPosition.x", AnimationCurve.Linear(0, 0, 1, 0));
            var settings = AnimationUtility.GetAnimationClipSettings(clip);
            settings.loopTime = loop;
            AnimationUtility.SetAnimationClipSettings(clip, settings);
            return clip;
        }

        [TearDown]
        public void TearDown()
        {
            foreach (var clip in asset.clips) UnityEngine.Object.DestroyImmediate(clip);
            UnityEngine.Object.DestroyImmediate(asset);
            UnityEngine.Object.DestroyImmediate(model);
        }

        private static void Observe(ModernFireflyPresenter actor, int life, byte action, ulong tick) =>
            actor.Observe(Vector3.zero, 13, 27, 39, life, action, 0, tick);

        [Test]
        public void LivingFlightUsesSourcePlacementOnlyAndAlwaysAdvancesWings()
        {
            using var fly = new ModernFireflyPresenter(asset, null, 1);
            var position = new Vector3(4, 8, -2);
            var rotation = Quaternion.Euler(22, 38, -14);
            fly.Root.transform.SetPositionAndRotation(position, rotation);
            fly.Observe(new Vector3(99, 25, 80), 10, 20, 30, 100, 153, 2, 1);
            fly.Render(.2f);
            Assert.That(fly.CurrentClip, Is.EqualTo("Idle"));
            Assert.That(fly.ClipTime, Is.EqualTo(.2f).Within(.0001f));
            Assert.That(fly.Root.transform.position, Is.EqualTo(position));
            Assert.That(Quaternion.Angle(fly.Root.transform.rotation, rotation), Is.LessThan(.001f));
            fly.Root.transform.position += new Vector3(.2f, .1f, .1f);
            fly.Render(.1f);
            Assert.That(fly.CurrentClip, Is.EqualTo("Flight"));
            Assert.That(fly.ClipTime, Is.GreaterThan(0));
            fly.Render(.2f);
            Assert.That(fly.CurrentClip, Is.EqualTo("Idle"));
            var time = fly.ClipTime;
            fly.Render(.1f);
            Assert.That(fly.ClipTime, Is.GreaterThan(time));
        }

        [Test]
        public void HitWinsOverAttackAndRepeatedDamageRestartsIt()
        {
            using var fly = new ModernFireflyPresenter(asset, null, 1);
            Observe(fly, 100, 153, 1);
            fly.Render(.1f);
            Observe(fly, 90, 154, 2);
            fly.Render(.2f);
            Assert.That(fly.CurrentClip, Is.EqualTo("Hit"));
            Observe(fly, 80, 154, 3);
            fly.Render(.1f);
            Assert.That(fly.CurrentClip, Is.EqualTo("Hit"));
            Assert.That(fly.ClipTime, Is.EqualTo(.1f).Within(.0001f));
            fly.Render(1f);
            fly.Render(.1f);
            Assert.That(fly.CurrentClip, Is.EqualTo("Attack"), "Attack entry waits for the damage reaction");
            fly.Render(1f);
            fly.Render(.1f);
            Assert.That(fly.CurrentClip, Is.EqualTo("Idle"), "Action 154 must not loop a firing gesture");
        }

        [Test]
        public void DamageDuringAttackResumesThePendingAttackThenReturnsToFlight()
        {
            using var fly = new ModernFireflyPresenter(asset, null, 1);
            Observe(fly, 100, 153, 1);
            Observe(fly, 100, 154, 2);
            fly.Render(.2f);
            Assert.That(fly.CurrentClip, Is.EqualTo("Attack"));
            Observe(fly, 90, 154, 3);
            fly.Render(.2f);
            Assert.That(fly.CurrentClip, Is.EqualTo("Hit"));
            fly.Render(1f);
            fly.Render(.1f);
            Assert.That(fly.CurrentClip, Is.EqualTo("Attack"));
        }

        [Test]
        public void NativeZeroLifeAndLivingRemovalDoNotInventDeath()
        {
            foreach (byte action in new byte[] { 153, 154, 155, 159 })
                Assert.That(ModernFireflyPresenter.SourceDeath(action, 0), Is.False);
            Assert.That(ModernFireflyPresenter.SourceDeath(153, -1), Is.True);
            Assert.That(ModernFireflyPresenter.SourceDeath(156, 10), Is.True);
            Assert.That(ModernFireflyPresenter.SourceDeath(157, 10), Is.True);
            using var fly = new ModernFireflyPresenter(asset, null, 1);
            Observe(fly, 0, 153, 1);
            fly.MarkRemoved();
            fly.MarkRemoved();
            Assert.That(fly.IsDying, Is.False);
            Assert.That(fly.DeathFinished, Is.False);
            Assert.Throws<InvalidOperationException>(() => Observe(fly, 1, 153, 2));
        }

        [Test]
        public void DeathTakesPriorityAndRemovalCannotRestartIt()
        {
            using var fly = new ModernFireflyPresenter(asset, null, 1);
            Observe(fly, 100, 153, 1);
            Observe(fly, 90, 154, 2);
            Observe(fly, -1, 156, 3);
            fly.Render(.3f);
            Assert.That(fly.IsDying, Is.True);
            Assert.That(fly.CurrentClip, Is.EqualTo("Death"));
            var time = fly.ClipTime;
            Observe(fly, -1, 157, 4);
            fly.MarkRemoved();
            fly.MarkRemoved();
            Assert.That(fly.ClipTime, Is.EqualTo(time));
            fly.Render(1f);
            Assert.That(fly.DeathFinished, Is.True);
            Assert.Throws<InvalidOperationException>(() => Observe(fly, 100, 153, 5));
        }

        [Test]
        public void ConfirmedJournalDeathAfterRemovalStartsOnce()
        {
            using var fly = new ModernFireflyPresenter(asset, null, 1);
            Observe(fly, 0, 153, 1);
            fly.MarkRemoved();
            Assert.That(fly.IsDying, Is.False);
            fly.ConfirmDeath();
            fly.Render(.2f);
            var time = fly.ClipTime;
            fly.ConfirmDeath();
            fly.MarkRemoved();
            Assert.That(fly.IsDying, Is.True);
            Assert.That(fly.CurrentClip, Is.EqualTo("Death"));
            Assert.That(fly.ClipTime, Is.EqualTo(time), "Repeated journal confirmation must not restart Death");
        }

        [Test]
        public void DuplicateTicksDoNotCreateHitsAndOlderObservationsFail()
        {
            using var fly = new ModernFireflyPresenter(asset, null, 1);
            Observe(fly, 100, 153, 4);
            Observe(fly, 10, 154, 4);
            fly.Render(.1f);
            Assert.That(fly.CurrentClip, Is.EqualTo("Idle"));
            Assert.Throws<ArgumentOutOfRangeException>(() => Observe(fly, 100, 153, 3));
            Assert.Throws<ArgumentOutOfRangeException>(() => fly.Render(float.NaN));
        }

        [Test]
        public void ImportedFlightLoopsAreContinuousAndContainWingMotion()
        {
            var imported = Resources.Load<ModernCharacterAsset>("ModernCharacters/Firefly/Character");
            Assert.That(imported, Is.Not.Null, "Prepare the Firefly owner-local import first");
            using var fly = new ModernCharacterInstance(imported, null);
            var renderer = fly.Root.GetComponentsInChildren<SkinnedMeshRenderer>(true).Single();
            var mesh = new Mesh();
            try
            {
                foreach (var clipName in new[] { "Idle", "Flight" })
                {
                    var clip = imported.Clip(clipName);
                    Assert.That(clip.isLooping, Is.True);
                    Vector3[] Sample(float time)
                    {
                        fly.Sample(clipName, time);
                        renderer.BakeMesh(mesh, true);
                        return mesh.vertices.Select(v => renderer.transform.TransformPoint(v)).ToArray();
                    }
                    var start = Sample(0);
                    var middle = Sample(clip.length * .25f);
                    // Exact length wraps to zero in ModernCharacterInstance;
                    // sample just before the boundary to test continuity.
                    var end = Sample(Mathf.Max(0, clip.length - .000001f));
                    Assert.That(start.Length, Is.GreaterThan(0));
                    Assert.That(start.Zip(end, Vector3.Distance).Max(), Is.LessThan(.0001f));
                    Assert.That(start.Zip(middle, Vector3.Distance).Max(), Is.GreaterThan(.05f), "Actual wing surfaces must move");
                }
            }
            finally { UnityEngine.Object.DestroyImmediate(mesh); }
        }

        [Test]
        public void ImportedVoxelDeathSnapshotsLastLivePoseEvenAfterDeathPlayback()
        {
            var imported = Resources.Load<ModernCharacterAsset>("ModernCharacters/Firefly/Character");
            var skin = Resources.Load<VoxelGoatAsset>("ModernCharacters/Firefly/Voxels/Firefly64");
            Assert.That(imported, Is.Not.Null, "Prepare Firefly owner-local import first");
            Assert.That(skin, Is.Not.Null, "Prepare the Firefly 64-cell voxel asset first");
            using var fly = new ModernFireflyPresenter(imported, null, imported.height);
            fly.AttachVoxelSkin(skin);
            Observe(fly, 100, 153, 1);
            fly.Render(.12f);
            var live = fly.SnapshotVoxelFragments();
            Assert.That(live, Is.Not.Empty);
            Observe(fly, -1, 156, 2);
            fly.Render(.3f);
            var deathClipTime = fly.ClipTime;
            var fragments = fly.SnapshotVoxelFragments();
            Assert.That(fragments.Count, Is.EqualTo(live.Count));
            Assert.That(live.Zip(fragments, (a, b) => Vector3.Distance(a.Position, b.Position)).Max(), Is.LessThan(.0001f));
            Assert.That(fly.ClipTime, Is.EqualTo(deathClipTime), "Snapshot must not advance or restart Death");
        }
    }
}
