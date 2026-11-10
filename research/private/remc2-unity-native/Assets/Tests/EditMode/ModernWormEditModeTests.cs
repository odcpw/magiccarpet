using System;
using System.Linq;
using MagicCarpet.Native.Voxels;
using NUnit.Framework;
using UnityEngine;

namespace MagicCarpet.Native.Tests
{
    public sealed class ModernWormEditModeTests
    {
        private ModernCharacterAsset head, segment;
        private VoxelGoatAsset headVoxel, segmentVoxel;

        [SetUp]
        public void SetUp()
        {
            head = Resources.Load<ModernCharacterAsset>("ModernCharacters/WormHead/Character");
            segment = Resources.Load<ModernCharacterAsset>("ModernCharacters/WormSegment/Character");
            headVoxel = Resources.Load<VoxelGoatAsset>("ModernCharacters/WormHead/Voxels/WormHead64");
            segmentVoxel = Resources.Load<VoxelGoatAsset>("ModernCharacters/WormSegment/Voxels/WormSegment64");
            Assert.That(head, Is.Not.Null, "Prepare the real imported Worm head");
            Assert.That(segment, Is.Not.Null, "Prepare the real imported Worm segment");
            Assert.That(headVoxel, Is.Not.Null, "Prepare the real Worm head 64-cell skin");
            Assert.That(segmentVoxel, Is.Not.Null, "Prepare the real Worm segment 64-cell skin");
        }

        private ModernWormPresenter Worm(uint generation = 4) => new ModernWormPresenter(head, segment, null, 10, 17, generation);

        private static ModernWormPresenter.SegmentPose Part(byte index, ushort slot, uint generation = 1,
            byte action = 232, int life = 100, bool drawing = true, ushort parent = 10, uint parentGeneration = 4) =>
            new ModernWormPresenter.SegmentPose(slot, 17, generation, parent, parentGeneration, index,
                new Vector3(1, 2, 3), 15, 25, 35, 2, 1, true, drawing, life, action);

        private static void Place(ModernWormPresenter worm, ushort slot, uint generation, Vector3 position) =>
            Assert.That(worm.PlaceSegment(slot, generation, position, Quaternion.Euler(9, 41, 13), 2, 1, 1.5f), Is.True);

        [Test]
        public void SourceRotationUsesEachYawAndNeverTreatsSteeringFieldsAsBank()
        {
            foreach (short yaw in new short[] { 0, 256, 1023, 2047 })
            {
                var expected = Quaternion.Euler(0, 180f-yaw*360f/2048f, 0);
                var rotation = ModernWormPresenter.SourceRotation(yaw);
                Assert.That(Quaternion.Angle(expected, rotation), Is.LessThan(.001f));
                Assert.That(Vector3.Distance(rotation * Vector3.up, Vector3.up), Is.LessThan(.0001f));
            }
            Assert.That(Quaternion.Angle(ModernWormPresenter.SourceRotation(0),
                ModernWormPresenter.SourceRotation(512)), Is.EqualTo(90).Within(.001f),
                "Independent child yaw must remain visible");
            Assert.That(Quaternion.Angle(ModernWormPresenter.SourceRotation(-1),
                ModernWormPresenter.SourceRotation(2047)), Is.LessThan(.01f),
                "The source constructor can represent wrapped heading 2047 as -1");
            var constructorHeading = ModernWormPresenter.SourceRotation(1023);
            var incorrectBanked = Quaternion.Euler(1023f*360f/2048f, 180f-1023f*360f/2048f, -1023f*360f/2048f);
            Assert.That(Quaternion.Angle(constructorHeading, incorrectBanked), Is.GreaterThan(30),
                "Copying constructor pitch and desired heading into visual bank flips the head");
        }

        [Test]
        public void ImportedHeadMandiblesFaceNativeTravelAtCardinalHeadings()
        {
            // The reviewed authoring mesh points along Blender +Y. The fixed
            // FBX/Unity conversion maps that vector to Unity +Z. Native
            // move_entity(0) travels source -Y = Unity -Z; yaw512 travels +X.
            var modelForward = Quaternion.Euler(0, head.forwardYaw, 0) * Vector3.forward;
            var yawZeroForward = ModernWormPresenter.SourceRotation(0) * modelForward;
            var yawQuarterForward = ModernWormPresenter.SourceRotation(512) * modelForward;
            Assert.That(Vector3.Distance(yawZeroForward, Vector3.back), Is.LessThan(.0001f),
                "The visible mandibles must lead, not trail, native yaw-zero motion");
            Assert.That(Vector3.Distance(yawQuarterForward, Vector3.right), Is.LessThan(.0001f));
            Assert.That(segment.forwardYaw, Is.EqualTo(0).Within(.0001f),
                "The reusable body uses the same asset-axis convention");

            // Check the imported geometry, not only its metadata: both rigid
            // mandible groups must lie ahead of the head pivot in world space.
            using var instance = new ModernCharacterInstance(head, null);
            var renderer = instance.Root.GetComponentsInChildren<SkinnedMeshRenderer>(true).Single();
            var left = Array.FindIndex(renderer.bones, bone => bone.name == "Mandible.L");
            var right = Array.FindIndex(renderer.bones, bone => bone.name == "Mandible.R");
            Assert.That(left, Is.GreaterThanOrEqualTo(0)); Assert.That(right, Is.GreaterThanOrEqualTo(0));
            var mesh = new Mesh();
            try
            {
                instance.Sample("Idle", 0); renderer.BakeMesh(mesh, true);
                var weights = renderer.sharedMesh.boneWeights; var vertices = mesh.vertices;
                Assert.That(weights.Length, Is.EqualTo(vertices.Length));
                void CheckJaw(int index, Vector3 travel)
                {
                    var sum = Vector3.zero; var count = 0;
                    for (var i = 0; i < weights.Length; i++)
                    {
                        if (weights[i].boneIndex0 != index || weights[i].weight0 < .99f) continue;
                        sum += renderer.transform.TransformPoint(vertices[i]); count++;
                    }
                    Assert.That(count, Is.GreaterThan(0), "Imported mandible has no rigid surface");
                    Assert.That(Vector3.Dot(sum/count-instance.Root.transform.position, travel), Is.GreaterThan(.1f),
                        "Imported mandible surface must lead the native head pivot");
                }
                instance.Root.transform.rotation = ModernWormPresenter.SourceRotation(0);
                CheckJaw(left, Vector3.back); CheckJaw(right, Vector3.back);
                instance.Root.transform.rotation = ModernWormPresenter.SourceRotation(512);
                CheckJaw(left, Vector3.right); CheckJaw(right, Vector3.right);
            }
            finally { UnityEngine.Object.DestroyImmediate(mesh); }
        }

        [Test]
        public void NativeRowsOwnPlacementAndRigidPartsHaveNoPerChildAnimator()
        {
            using var worm = Worm();
            var headPosition = new Vector3(12, 3, -5); var headRotation = Quaternion.Euler(12, 70, -10);
            var childPosition = new Vector3(10, 2, -6);
            worm.Observe(0, 25, 1, new[] { Part(0, 20) });
            worm.PlaceHead(headPosition, headRotation, 3, 1, 2);
            Place(worm, 20, 1, childPosition);
            var child = worm.SegmentRoot(20, 1);
            Assert.That(child, Is.Not.Null);
            Assert.That(child.GetComponentsInChildren<SkinnedMeshRenderer>(true).Length, Is.EqualTo(1));
            Assert.That(child.GetComponentsInChildren<Animator>(true).All(a => !a.enabled), Is.True);
            var scale = child.transform.localScale;
            worm.Render(.25f);
            Assert.That(worm.IsDying, Is.False, "Exactly zero life is living");
            Assert.That(worm.Root.transform.position, Is.EqualTo(headPosition));
            Assert.That(Quaternion.Angle(worm.Root.transform.rotation, headRotation), Is.LessThan(.001f));
            Assert.That(child.transform.position, Is.EqualTo(childPosition));
            Assert.That(child.transform.localScale, Is.EqualTo(scale));
            Assert.That(worm.CurrentClip, Is.EqualTo("Idle"));
            Assert.That(worm.PlaceSegment(20, 2, childPosition, Quaternion.identity, 2, 1, 2), Is.False);
            Assert.Throws<ArgumentOutOfRangeException>(() => worm.PlaceHead(Vector3.zero, Quaternion.identity, 1, 1, 0));
        }

        [Test]
        public void ImportedBoundsProvideProportionateScaleAndBottomOffset()
        {
            using var worm = Worm();
            Assert.That(worm.HeadWidthToHeight, Is.GreaterThan(0));
            Assert.That(worm.HeadDepthToHeight, Is.GreaterThan(0));
            Assert.That(worm.HeadBottomToHeight, Is.LessThan(0));
            Assert.That(worm.SegmentWidthToHeight, Is.GreaterThan(0));
            Assert.That(worm.SegmentDepthToHeight, Is.GreaterThan(0));
            Assert.That(worm.SegmentBottomToHeight, Is.LessThan(0));
            const float height = 1.5f; const float nativeBottom = 4f;
            worm.Observe(100, 25, 1, new[] { Part(0, 20) });
            var headCenter = new Vector3(1, nativeBottom - height * worm.HeadBottomToHeight, 2);
            worm.PlaceHead(headCenter, Quaternion.identity, height * worm.HeadWidthToHeight, height,
                height * worm.HeadDepthToHeight);
            var headScale = worm.Root.transform.localScale;
            Assert.That(headScale.x, Is.EqualTo(headScale.y).Within(.0001f));
            Assert.That(headScale.z, Is.EqualTo(headScale.y).Within(.0001f));
            var bodyCenter = new Vector3(3, nativeBottom - height * worm.SegmentBottomToHeight, 4);
            Assert.That(worm.PlaceSegment(20, 1, bodyCenter, Quaternion.identity, height * worm.SegmentWidthToHeight,
                height, height * worm.SegmentDepthToHeight), Is.True);
            var bodyScale = worm.SegmentRoot(20, 1).transform.localScale;
            Assert.That(bodyScale.x, Is.EqualTo(bodyScale.y).Within(.0001f));
            Assert.That(bodyScale.z, Is.EqualTo(bodyScale.y).Within(.0001f));
        }

        [Test]
        public void MonotonicObservationRejectsForeignOrReusedChildIdentity()
        {
            using var worm = Worm();
            worm.Observe(100, 25, 4, new[] { Part(0, 20) });
            Place(worm, 20, 1, Vector3.one);
            worm.Observe(-1, 28, 4, Array.Empty<ModernWormPresenter.SegmentPose>());
            Assert.That(worm.IsDying, Is.False, "A duplicate tick cannot change the retained observation");
            Assert.Throws<ArgumentOutOfRangeException>(() => worm.Observe(100, 25, 3, Array.Empty<ModernWormPresenter.SegmentPose>()));
            Assert.Throws<ArgumentException>(() => worm.Observe(100, 25, 5, new[] { Part(0, 20, parentGeneration: 5) }));
            Assert.Throws<ArgumentException>(() => worm.Observe(100, 25, 5, new[] { Part(0, 10) }));
            Assert.Throws<ArgumentException>(() => worm.Observe(100, 25, 5, new[] { Part(0, 20), Part(1, 20) }));
            worm.Observe(100, 25, 5, new[] { Part(1, 20, 2) });
            Assert.That(worm.SegmentRoot(20, 1), Is.Null, "Slot reuse cannot retain the old part generation");
            Assert.That(worm.SegmentCount, Is.EqualTo(1));
            Place(worm, 20, 2, Vector3.right);
            worm.Observe(100, 25, 6, new[] { Part(1, 20, 2, 29, 0) });
            Assert.That(worm.IsDying, Is.False, "A body action 29 is not a second head death");
            Assert.That(worm.PlaceSegment(20, 2, Vector3.zero, Quaternion.identity, 2, 1, 1), Is.False);
        }

        [Test]
        public void FamilyCullingAndBoundsRespectAdmittedPartsAndRetainedDeath()
        {
            using var worm = Worm();
            worm.Observe(100, 25, 1, new[] { Part(0, 20), Part(1, 21) });
            worm.PlaceHead(Vector3.zero, Quaternion.identity, 3, 1, 2);
            Place(worm, 20, 1, new Vector3(30, 0, 0));
            Place(worm, 21, 1, new Vector3(60, 0, 0));
            var first = worm.SegmentRoot(20, 1); var second = worm.SegmentRoot(21, 1);
            Assert.That(worm.FamilyRenderers.Count, Is.EqualTo(3));
            Assert.That(worm.TryGetFamilyBounds(out var full), Is.True);
            Assert.That(full.max.x, Is.GreaterThan(60));
            worm.SetVisible(false);
            Assert.That(worm.Root.activeSelf || first.activeSelf || second.activeSelf, Is.False);
            Place(worm, 21, 1, new Vector3(60, 0, 0));
            Assert.That(second.activeSelf, Is.False, "Interpolated placement must not defeat culling");
            Assert.That(worm.TryGetFamilyBounds(out var culled), Is.True);
            Assert.That(culled.max.x, Is.EqualTo(full.max.x).Within(.0001f), "A culled family must still be testable for re-entry");
            worm.Observe(100, 25, 2, new[] { Part(1, 21) });
            worm.SetVisible(true);
            Assert.That(worm.Root.activeSelf, Is.True);
            Assert.That(first.activeSelf, Is.False, "Culling cannot resurrect a native-hidden part");
            Assert.That(second.activeSelf, Is.True);
            Assert.That(worm.FamilyRenderers.Count, Is.EqualTo(2));
            worm.Observe(-1, 28, 3, new[] { Part(1, 21, action: 29) });
            Assert.That(first.activeSelf, Is.False, "Death must not revive a previously hidden child");
            Assert.That(second.activeSelf, Is.True, "Death retains the admitted child's last live pose");
            worm.SetVisible(false);
            Assert.That(worm.Root.activeSelf || first.activeSelf || second.activeSelf, Is.False,
                "A successful death burst must hide the whole family");
        }

        [Test]
        public void AttackHitAndDeathFollowHeadStateWithoutRestartingOnRemoval()
        {
            using var worm = Worm();
            worm.Observe(100, 25, 1, Array.Empty<ModernWormPresenter.SegmentPose>());
            worm.Observe(100, 26, 2, Array.Empty<ModernWormPresenter.SegmentPose>());
            worm.Render(.1f);
            Assert.That(worm.CurrentClip, Is.EqualTo("Attack"));
            worm.Observe(90, 26, 3, Array.Empty<ModernWormPresenter.SegmentPose>());
            worm.Render(.1f);
            Assert.That(worm.CurrentClip, Is.EqualTo("Hit"));
            worm.Observe(0, 25, 4, Array.Empty<ModernWormPresenter.SegmentPose>());
            Assert.That(worm.IsDying, Is.False);
            worm.Observe(0, 28, 5, Array.Empty<ModernWormPresenter.SegmentPose>());
            worm.Render(.2f);
            Assert.That(worm.IsDying, Is.True);
            Assert.That(worm.CurrentClip, Is.EqualTo("Death"));
            var time = worm.ClipTime;
            worm.ConfirmDeath(); worm.MarkRemoved(); worm.MarkRemoved();
            Assert.That(worm.ClipTime, Is.EqualTo(time));
            worm.Render(head.Clip("Death").length + .3f);
            Assert.That(worm.DeathFinished, Is.True);
            Assert.Throws<InvalidOperationException>(() => worm.Observe(100, 25, 6, Array.Empty<ModernWormPresenter.SegmentPose>()));
        }

        [Test]
        public void LivingRemovalNeverAuthorizesFractureAndFreshGenerationStartsEmpty()
        {
            using var old = Worm();
            old.Observe(0, 25, 1, new[] { Part(0, 20) });
            Place(old, 20, 1, Vector3.one);
            old.MarkRemoved();
            Assert.That(old.IsDying, Is.False);
            Assert.That(old.Root.activeSelf, Is.False);
            Assert.Throws<InvalidOperationException>(() => old.SnapshotVoxelFragments(128));
            using var fresh = Worm(5);
            Assert.That(fresh.HeadGeneration, Is.EqualTo(5));
            Assert.That(fresh.SegmentCount, Is.Zero);
            Assert.Throws<ArgumentException>(() => fresh.Observe(1, 25, 1, new[] { Part(0, 20, parentGeneration: 4) }));
            fresh.Observe(1, 25, 1, new[] { Part(0, 20, 2, parentGeneration: 5) });
            Assert.That(fresh.SegmentCount, Is.EqualTo(1));
        }

        [Test]
        public void FirstObservationAlreadyDeadDoesNotInventALastLivePosition()
        {
            using var worm = Worm();
            worm.AttachVoxelSkins(headVoxel, segmentVoxel);
            worm.Observe(-1, 29, 1, Array.Empty<ModernWormPresenter.SegmentPose>());
            worm.PlaceHead(new Vector3(7, 8, 9), Quaternion.identity, 3, 1, 2);
            Assert.That(worm.IsDying, Is.True);
            Assert.That(worm.HasRetainedLivePose, Is.False);
            Assert.Throws<InvalidOperationException>(() => worm.SnapshotVoxelFragments(128));
        }

        private void PrepareLastLive(ModernWormPresenter worm)
        {
            worm.AttachVoxelSkins(headVoxel, segmentVoxel);
            worm.Observe(100, 25, 1, new[] { Part(0, 20), Part(1, 21) });
            worm.PlaceHead(new Vector3(3, 4, 5), Quaternion.Euler(3, 27, 5), 3, 1.2f, 2.5f);
            Place(worm, 20, 1, new Vector3(2, 4, 4));
            Place(worm, 21, 1, new Vector3(1, 4, 3));
            worm.Render(.12f);
            worm.Observe(-1, 28, 2, new[] { Part(0, 20, action: 29), Part(1, 21, action: 29) });
        }

        [Test]
        public void ConfirmedDeathSnapshotsBoundedLastLiveFamilyOnceWithoutChangingCurrentPose()
        {
            using var reference = Worm(); using var changed = Worm();
            PrepareLastLive(reference); PrepareLastLive(changed);
            var expected = reference.SnapshotVoxelFragments(256);
            Assert.That(expected.Count, Is.GreaterThan(0).And.LessThanOrEqualTo(256));
            changed.Render(.3f);
            changed.PlaceHead(new Vector3(100, 90, 80), Quaternion.Euler(0, 90, 0), 3, 1.2f, 2.5f);
            var part = changed.SegmentRoot(20, 1);
            part.transform.position = new Vector3(90, 80, 70);
            var currentHead = changed.Root.transform.position; var currentPart = part.transform.position; var time = changed.ClipTime;
            var actual = changed.SnapshotVoxelFragments(256);
            Assert.That(actual.Count, Is.EqualTo(expected.Count));
            Assert.That(expected.Zip(actual, (a,b) => Vector3.Distance(a.Position,b.Position)).Max(), Is.LessThan(.0001f));
            Assert.That(changed.Root.transform.position, Is.EqualTo(currentHead));
            Assert.That(part.transform.position, Is.EqualTo(currentPart));
            Assert.That(changed.ClipTime, Is.EqualTo(time));
            Assert.That(changed.SnapshotVoxelFragments(256), Is.SameAs(actual), "Retries reuse one retained family snapshot");
            Assert.Throws<InvalidOperationException>(() => changed.SnapshotVoxelFragments(128));
            Assert.Throws<ArgumentOutOfRangeException>(() => changed.SnapshotVoxelFragments(ModernWormPresenter.MaximumSnapshotFragments + 1));
        }

        [Test]
        public void FullFamilySnapshotCoversEveryRetainedPartAndRestoresCurrentPoses()
        {
            using var worm = Worm();
            worm.AttachVoxelSkins(headVoxel, segmentVoxel);
            var poses = Enumerable.Range(0, 16).Select(i => Part((byte)i, (ushort)(20+i))).ToArray();
            worm.Observe(9000, 25, 1, poses);
            worm.PlaceHead(new Vector3(0, 4, 0), Quaternion.identity, 3, 1, 2);
            for (var i = 0; i < 16; i++) Place(worm, (ushort)(20+i), 1, new Vector3((i+1)*12, 4, 0));
            worm.Render(.12f);
            worm.Observe(-1, 28, 2, Array.Empty<ModernWormPresenter.SegmentPose>());

            // Move the current roots after retaining the living poses. The
            // snapshot must sample each retained pose and restore these roots.
            var headCurrent = new Vector3(2, 20, 3);
            worm.PlaceHead(headCurrent, Quaternion.Euler(0, 72, 0), 3, 1, 2);
            var current = new Vector3[16];
            for (var i = 0; i < 16; i++)
            {
                current[i] = new Vector3((i+1)*12, 25, 100);
                worm.SegmentRoot((ushort)(20+i), 1).transform.position = current[i];
            }
            Assert.Throws<ArgumentOutOfRangeException>(() => worm.SnapshotVoxelFragments(16));
            var fragments = worm.SnapshotVoxelFragments(8192);
            Assert.That(fragments.Count, Is.GreaterThanOrEqualTo(17).And.LessThanOrEqualTo(8192));
            var partCounts = new int[17];
            foreach (var fragment in fragments)
            {
                Assert.That(fragment.Position.z, Is.LessThan(10), "A current pose leaked into the retained snapshot");
                var index = Mathf.RoundToInt(fragment.Position.x / 12f);
                Assert.That(index, Is.InRange(0, 16));
                partCounts[index]++;
            }
            Assert.That(partCounts.All(count => count > 0), Is.True, "Every retained head and body part contributes");
            Assert.That(worm.Root.transform.position, Is.EqualTo(headCurrent));
            for (var i = 0; i < 16; i++)
                Assert.That(worm.SegmentRoot((ushort)(20+i), 1).transform.position, Is.EqualTo(current[i]));
            Assert.That(worm.SnapshotVoxelFragments(8192), Is.SameAs(fragments));
        }

        [Test]
        public void DeathSnapshotRebasesTheWholeFamilyAcrossTheMapSeam()
        {
            using var reference = Worm(); using var wrapped = Worm();
            PrepareLastLive(reference); PrepareLastLive(wrapped);
            var expected = reference.SnapshotVoxelFragments(256);
            var shift = new Vector3(1024, 0, -1024);
            wrapped.Render(.2f);
            var currentHead = new Vector3(3, 4, 5) + shift;
            wrapped.PlaceHead(currentHead, Quaternion.Euler(5, 140, 4), 3, 1.2f, 2.5f);
            var first = wrapped.SegmentRoot(20, 1); var second = wrapped.SegmentRoot(21, 1);
            first.transform.position = new Vector3(600, 30, 60);
            second.transform.position = new Vector3(-600, -20, -60);
            var firstCurrent = first.transform.position; var secondCurrent = second.transform.position;
            var currentRotation = wrapped.Root.transform.rotation; var clipTime = wrapped.ClipTime;
            var actual = wrapped.SnapshotVoxelFragments(256);
            Assert.That(actual.Count, Is.EqualTo(expected.Count));
            Assert.That(expected.Zip(actual, (a,b) => Vector3.Distance(a.Position + shift, b.Position)).Max(), Is.LessThan(.0001f));
            Assert.That(actual.Max(cell => Vector3.Distance(cell.Position, currentHead)), Is.LessThan(10),
                "All retained parts must use the same nearby periodic image");
            Assert.That(wrapped.Root.transform.position, Is.EqualTo(currentHead));
            Assert.That(Quaternion.Angle(wrapped.Root.transform.rotation, currentRotation), Is.LessThan(.001f));
            Assert.That(first.transform.position, Is.EqualTo(firstCurrent));
            Assert.That(second.transform.position, Is.EqualTo(secondCurrent));
            Assert.That(wrapped.ClipTime, Is.EqualTo(clipTime));
            Assert.That(wrapped.SnapshotVoxelFragments(256), Is.SameAs(actual));
        }
    }
}
