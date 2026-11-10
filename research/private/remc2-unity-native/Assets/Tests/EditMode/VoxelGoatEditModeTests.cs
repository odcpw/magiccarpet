using System.Collections.Generic;
using System.Linq;
using MagicCarpet.Native.Voxels;
using NUnit.Framework;
using UnityEngine;

namespace MagicCarpet.Native.Tests
{
    public sealed class VoxelGoatEditModeTests
    {
        private static ModernCharacterAsset Character() => Resources.Load<ModernCharacterAsset>("ModernCharacters/Goat/Character");
        private static VoxelGoatAsset Asset(int resolution) => Resources.Load<VoxelGoatAsset>("ModernCharacters/Goat/Voxels/Goat" + resolution);

        [TestCase("Firefly", 64)]
        [TestCase("Firefly", 96)]
        [TestCase("WormHead", 64)]
        [TestCase("WormHead", 96)]
        [TestCase("WormSegment", 64)]
        [TestCase("WormSegment", 96)]
        public void ConvertedCreatureVoxelsRetainAuthoredPalette(string resource, int resolution)
        {
            var asset = Resources.Load<VoxelGoatAsset>($"ModernCharacters/{resource}/Voxels/{resource}{resolution}");
            Assert.That(asset, Is.Not.Null, "Require the actual imported creature derivative");
            // Firefly has seven authored flat-colour material maps; requiring
            // eight colours would reject its correct palette. Worms use
            // textured surfaces. Both must reject the observed one-grey bake.
            var minimumColours = resource == "Firefly" ? 7 : 9;
            Assert.That(asset.cellColors.Select(c => ((int)c.r << 16) | (c.g << 8) | c.b).Distinct().Count(), Is.GreaterThanOrEqualTo(minimumColours));
            Assert.That(asset.cellColors.Any(c => Mathf.Max(c.r, Mathf.Max(c.g, c.b)) - Mathf.Min(c.r, Mathf.Min(c.g, c.b)) > 32), Is.True,
                "Original olive shells, violet eyes and red lens must survive headless texture sampling");
            if (resource == "Firefly")
            {
                Assert.That(asset.cellColors.Any(c => c.b > c.g * 1.5f && c.r > c.g), Is.True, "Violet eyes");
                Assert.That(asset.cellColors.Any(c => c.r > c.b * 2 && c.g > c.b * 1.5f), Is.True, "Amber underside");
            }
        }

        [Test]
        public void BoundedSnapshotUsesCurrentPoseWithoutChangingTheExactDefault()
        {
            var character = Character(); var asset = Asset(24);
            Assert.That(character, Is.Not.Null); Assert.That(asset, Is.Not.Null);
            using var actor = new ModernCharacterInstance(character, null);
            actor.Root.transform.SetPositionAndRotation(new Vector3(3, 4, -5), Quaternion.Euler(0, 37, 0));
            actor.Play("Walk", 0); actor.Tick(.2f);
            using var skin = new VoxelCharacterSkin(actor.Root, asset);
            var exact = skin.SnapshotFragments();
            var all = skin.SnapshotFragments(int.MaxValue);
            Assert.That(all.Count, Is.EqualTo(exact.Count));
            for (var i = 0; i < exact.Count; i++)
            {
                Assert.That(all[i].Position, Is.EqualTo(exact[i].Position));
                Assert.That(all[i].Size, Is.EqualTo(exact[i].Size));
                Assert.That(all[i].Color, Is.EqualTo(exact[i].Color));
            }
            var positions = new HashSet<Vector3>(exact.Select(c => c.Position));
            var bounded = skin.SnapshotFragments(17);
            Assert.That(bounded.Count, Is.InRange(1, 17));
            Assert.That(bounded.Select(c => c.Position).Distinct().Count(), Is.EqualTo(bounded.Count));
            foreach (var fragment in bounded)
            {
                Assert.That(positions.Contains(fragment.Position), Is.True, "Sample the current posed surface");
                Assert.That(fragment.Size, Is.EqualTo(exact[0].Size));
            }
            Assert.Throws<System.ArgumentOutOfRangeException>(() => skin.SnapshotFragments(0));
        }

        [Test]
        public void ImportedResolutionsUseApprovedSkinAndSampledColors()
        {
            var character = Character();
            Assert.That(character, Is.Not.Null, "Prepare the approved character first");
            var source = character.model.GetComponentsInChildren<SkinnedMeshRenderer>(true).Single().sharedMesh;
            var counts = new int[3];
            var resolutions = new[] { 24, 40, 64 };
            for (var i = 0; i < resolutions.Length; i++)
            {
                var asset = Asset(resolutions[i]);
                Assert.That(asset, Is.Not.Null, "Run VoxelGoatImport.Prepare first");
                Assert.That(asset.sourceMesh, Is.SameAs(source));
                Assert.That(asset.mesh.vertexCount, Is.EqualTo(asset.cellColors.Length * 24));
                Assert.That(asset.mesh.boneWeights.Length, Is.EqualTo(asset.mesh.vertexCount));
                Assert.That(asset.mesh.bindposes.Length, Is.EqualTo(source.bindposes.Length));
                Assert.That(asset.cellColors.Select(c => ((int)c.r << 16) | (c.g << 8) | c.b).Distinct().Count(), Is.GreaterThan(8));
                Assert.That(asset.resolution, Is.EqualTo(resolutions[i]));
                Assert.That(asset.cellSize, Is.GreaterThan(0));
                counts[i] = asset.cellColors.Length;
            }
            Assert.That(counts[1], Is.GreaterThan(counts[0]));
            Assert.That(counts[2], Is.GreaterThan(counts[1]));
        }

        [Test]
        public void CurrentPoseBecomesWorldSpaceFragmentsAndGaitUsesTravel()
        {
            var source = Character(); var asset = Asset(24);
            Assert.That(source, Is.Not.Null); Assert.That(asset, Is.Not.Null);
            using var goat = new VoxelGoat(source, asset, null, 2.34f);
            goat.Root.transform.position = new Vector3(4, 2, -3);
            goat.Root.transform.rotation = Quaternion.Euler(0, 37, 0);
            goat.Render(0, 0);
            var idle = goat.SnapshotFragments();
            goat.Render(.2f, .2f);
            Assert.That(goat.CurrentClip, Is.EqualTo("Walk"));
            var phase = goat.ClipTime;
            var moving = goat.SnapshotFragments();
            Assert.That(moving.Count, Is.GreaterThan(0).And.LessThanOrEqualTo(asset.cellColors.Length));
            Assert.That(moving[0].Size, Is.EqualTo(2.34f / asset.resolution).Within(.0001f));
            var idleKeys = new HashSet<Vector3Int>(idle.Select(f => GridKey(f.Position, goat.Root.transform.position, f.Size)));
            var movingKeys = new HashSet<Vector3Int>(moving.Select(f => GridKey(f.Position, goat.Root.transform.position, f.Size)));
            Assert.That(movingKeys.Except(idleKeys).Count(), Is.GreaterThan(10),
                "Animated fragments must change with the gait, not use a rest pose");
            AssertPoseGrid(goat.Root, asset, moving);
            var sampleCount = VoxelRubble.SampleCount(moving.Count, 16);
            Assert.That(sampleCount, Is.EqualTo(16));
            var sampled = new HashSet<int>();
            var positions = new HashSet<Vector3>();
            var represented = 0;
            for (var i = 0; i < sampleCount; i++)
            {
                var index = VoxelRubble.SampleIndex(i, moving.Count, sampleCount);
                Assert.That(sampled.Add(index), Is.True, "Rubble must sample distinct goat cells");
                Assert.That(positions.Add(moving[index].Position), Is.True, "Sampled rubble positions must be distinct");
                Assert.That(VoxelRubble.BlockSize(moving[index].Size), Is.LessThanOrEqualTo(VoxelRubble.MaximumBlockSize));
                represented += VoxelRubble.RepresentedCellCount(i, moving.Count, sampleCount);
            }
            Assert.That(represented, Is.EqualTo(moving.Count));

            var fullCount = VoxelRubble.SampleCount(moving.Count, VoxelRubble.MaxBodies);
            var sizes = VoxelRubble.CharacterSizes(moving, fullCount);
            Assert.That(sizes.Length, Is.EqualTo(fullCount));
            Assert.That(sizes.Any(size => size > moving[0].Size * 1.1f), Is.True,
                "Sparse pieces should occupy more visible volume than single fine cells");
            for (var i = 0; i < fullCount; i++) for (var j = i + 1; j < fullCount; j++)
            {
                var a = moving[VoxelRubble.SampleIndex(i, moving.Count, fullCount)];
                var b = moving[VoxelRubble.SampleIndex(j, moving.Count, fullCount)];
                var delta = a.Position - b.Position;
                float separation = Mathf.Max(Mathf.Abs(delta.x), Mathf.Max(Mathf.Abs(delta.y), Mathf.Abs(delta.z)));
                Assert.That(separation, Is.GreaterThanOrEqualTo((sizes[i] + sizes[j]) * .5f),
                    "Visible cubes and their colliders must start without mutual overlap");
            }

            var beforeMove = moving[0].Position;
            goat.Root.transform.position += Vector3.right * 3;
            var translated = goat.SnapshotFragments();
            Assert.That(Vector3.Distance(translated[0].Position - beforeMove, Vector3.right * 3), Is.LessThan(.0001f));
            AssertPoseGrid(goat.Root, asset, translated);
            goat.Render(.2f, 0);
            Assert.That(goat.CurrentClip, Is.EqualTo("Idle"));
            goat.Render(.2f, .2f);
            Assert.That(goat.CurrentClip, Is.EqualTo("Walk"));
            Assert.That(phase, Is.GreaterThan(0));
        }

        [Test]
        public void SnapshotBoundsFollowVisibleGoatAtSandboxScale()
        {
            var source = Character(); var asset = Asset(40);
            Assert.That(source, Is.Not.Null); Assert.That(asset, Is.Not.Null);
            using var goat = new VoxelGoat(source, asset, null, 1.2f);
            goat.Root.transform.SetPositionAndRotation(new Vector3(15, 4, -3), Quaternion.Euler(0, 37, 0));
            goat.Render(.2f, .15f);
            var renderer = goat.Root.GetComponentsInChildren<SkinnedMeshRenderer>(true).Single(r => r.sharedMesh == asset.mesh);
            var fragments = goat.SnapshotFragments();
            Assert.That(fragments, Is.Not.Empty);
            var actual = new Bounds(fragments[0].Position, Vector3.zero);
            foreach (var fragment in fragments) actual.Encapsulate(fragment.Position);
            actual.Expand(fragments[0].Size);
            var visible = renderer.bounds;
            Assert.That(fragments[0].Size, Is.EqualTo(1.2f / 40).Within(.001f));
            Assert.That(actual.size.y, Is.InRange(.6f, 1.8f), "Fragments must be goat-sized, not hundreds of times larger");
            Assert.That(Vector3.Distance(actual.center, visible.center), Is.LessThan(.6f),
                "Fragment cloud must occupy the visible renderer's world position");
            Assert.That(actual.size.x, Is.LessThan(visible.size.x + .3f));
            Assert.That(actual.size.y, Is.LessThan(visible.size.y + .3f));
            Assert.That(actual.size.z, Is.LessThan(visible.size.z + .3f));
            Assert.That(Vector3.Distance(actual.center, goat.Root.transform.position), Is.LessThan(2f));
        }

        [Test]
        public void GoatVoxelAttachmentPreservesSourceContactAndNativeGait()
        {
            var source = Character(); var skin = Asset(64);
            Assert.That(source, Is.Not.Null); Assert.That(skin, Is.Not.Null);
            using var voxel = new ModernGoatPresenter(source, null, 2.34f);
            using var control = new ModernGoatPresenter(source, null, 2.34f);
            voxel.AttachVoxelSkin(skin);
            foreach (var goat in new[] { voxel, control })
            {
                goat.Observe(Vector3.zero, 0, 600, 9, 1);
                goat.Render(0);
                goat.Root.transform.position = Vector3.forward * .2f;
                goat.Render(.1f);
            }
            Assert.That(voxel.CurrentClip, Is.EqualTo(control.CurrentClip));
            Assert.That(voxel.ClipTime, Is.EqualTo(control.ClipTime).Within(.00001f));
            var visibleSource = voxel.Root.GetComponentsInChildren<SkinnedMeshRenderer>(true).Single(r => r.sharedMesh == skin.sourceMesh);
            Assert.That(visibleSource.forceRenderingOff, Is.True);
            var expected = control.HoofClearances((x, z) => 0);
            var actual = voxel.HoofClearances((x, z) => 0);
            for (var i = 0; i < expected.Length; i++) Assert.That(actual[i], Is.EqualTo(expected[i]).Within(.001f));
            Assert.That(voxel.GroundToTerrain((x, z) => 0), Is.EqualTo(control.GroundToTerrain((x, z) => 0)));
            var timer = System.Diagnostics.Stopwatch.StartNew();
            var fragments = voxel.SnapshotVoxelFragments(); timer.Stop();
            Assert.That(fragments, Is.Not.Empty);
            AssertPoseGrid(voxel.Root, skin, fragments);
            TestContext.WriteLine($"Goat64 snapshot cells={fragments.Count} elapsedMs={timer.Elapsed.TotalMilliseconds:F3}");
        }

        [Test]
        public void ArcherVoxelAttachmentKeepsAuthoredBowAndNativeAim()
        {
            var source = Resources.Load<ModernCharacterAsset>("ModernCharacters/Archer/Character");
            var skin = Resources.Load<VoxelGoatAsset>("ModernCharacters/Archer/Voxels/Archer64");
            Assert.That(source, Is.Not.Null); Assert.That(skin, Is.Not.Null);
            using var voxel = new ModernArcherPresenter(source, null, source.height);
            using var control = new ModernArcherPresenter(source, null, source.height);
            voxel.AttachVoxelSkin(skin);
            foreach (var archer in new[] { voxel, control })
            {
                archer.Observe(Vector3.zero, 0, 100, 34, 24, 1, Vector3.forward, false, false);
                archer.Render(0);
            }
            Assert.That(voxel.CurrentClip, Is.EqualTo(control.CurrentClip));
            Assert.That(Vector3.Distance(voxel.HeldNockPosition, control.HeldNockPosition), Is.LessThan(.001f));
            Assert.That(Vector3.Angle(voxel.HeldArrowDirection, control.HeldArrowDirection), Is.LessThan(.01f));
            var renderers = voxel.Root.GetComponentsInChildren<Renderer>(true);
            var original = renderers.OfType<SkinnedMeshRenderer>().Single(r => r.sharedMesh == skin.sourceMesh);
            Assert.That(original.forceRenderingOff, Is.True);
            var bowstring = renderers.Single(r => r.name == "Bowstring");
            Assert.That(bowstring.forceRenderingOff, Is.False);
            Assert.That(renderers.Count(r => r is MeshRenderer && !r.forceRenderingOff), Is.GreaterThan(1),
                "Bow and held arrow parts must remain visible");
            var timer = System.Diagnostics.Stopwatch.StartNew();
            var fragments = voxel.SnapshotVoxelFragments(); timer.Stop();
            Assert.That(fragments, Is.Not.Empty);
            AssertPoseGrid(voxel.Root, skin, fragments);
            TestContext.WriteLine($"Archer64 snapshot cells={fragments.Count} elapsedMs={timer.Elapsed.TotalMilliseconds:F3}");
        }

        [TestCase("Goat", 40)]
        [TestCase("Archer", 64)]
        [TestCase("Villager", 64)]
        public void SwitchingDetailKeepsRigAndSnapshotsCurrentPose(string name, int coarseResolution)
        {
            var source = Resources.Load<ModernCharacterAsset>("ModernCharacters/" + name + "/Character");
            var prefix = "ModernCharacters/" + name + "/Voxels/" + name;
            var fine = Resources.Load<VoxelGoatAsset>(prefix + "96");
            var coarse = Resources.Load<VoxelGoatAsset>(prefix + coarseResolution);
            var other = name == "Goat" ? Resources.Load<VoxelGoatAsset>("ModernCharacters/Archer/Voxels/Archer64") : Asset(40);
            Assert.That(source, Is.Not.Null); Assert.That(fine, Is.Not.Null); Assert.That(coarse, Is.Not.Null); Assert.That(other, Is.Not.Null);
            VoxelCharacterSkin.PrepareDetail(fine); VoxelCharacterSkin.PrepareDetail(coarse);
            using var character = new ModernCharacterInstance(source, null);
            using var skin = new VoxelCharacterSkin(character.Root, fine);
            character.Root.transform.SetPositionAndRotation(new Vector3(4, 2, -3), Quaternion.Euler(0, 37, 0));
            character.Sample("Walk", Mathf.Min(.2f, source.Clip("Walk").length * .5f));
            var renderer = skin.Renderer; var material = renderer.sharedMaterial; var bones = renderer.bones;
            var clip = character.CurrentClip; var time = character.ClipTime;
            skin.SetDetail(coarse);
            Assert.That(skin.Renderer, Is.SameAs(renderer));
            Assert.That(renderer.sharedMesh, Is.SameAs(coarse.mesh));
            Assert.That(renderer.sharedMaterial, Is.SameAs(material));
            Assert.That(renderer.bones, Is.EqualTo(bones));
            Assert.That(skin.Source.forceRenderingOff, Is.True);
            Assert.That(skin.Resolution, Is.EqualTo(coarseResolution));
            Assert.That(character.CurrentClip, Is.EqualTo(clip)); Assert.That(character.ClipTime, Is.EqualTo(time));
            var snapshot = skin.SnapshotFragments();
            Assert.That(snapshot, Is.Not.Empty);
            Assert.That(snapshot[0].Size, Is.EqualTo(coarse.cellSize * Mathf.Abs(skin.Source.transform.lossyScale.x)).Within(.00001f));
            AssertPoseGrid(character.Root, coarse, snapshot);
            Assert.Throws<System.InvalidOperationException>(() => skin.SetDetail(other));
            Assert.That(renderer.sharedMesh, Is.SameAs(coarse.mesh), "Rejected detail must not change the current render mesh");
            skin.SetDetail(fine);
            Assert.That(renderer.sharedMesh, Is.SameAs(fine.mesh));
            Assert.That(renderer.sharedMaterial, Is.SameAs(material));
            Assert.That(skin.Resolution, Is.EqualTo(96));
            AssertPoseGrid(character.Root, fine, skin.SnapshotFragments());
        }

        [TestCase("Goat", 96)]
        [TestCase("Archer", 96)]
        [TestCase("Villager", 96)]
        [TestCase("Firefly", 64)]
        [TestCase("WormHead", 64)]
        [TestCase("WormSegment", 64)]
        public void FineSkinCullsOnlyCoincidentFacesAndKeepsPosedFragments(string name, int resolution)
        {
            var source = Resources.Load<ModernCharacterAsset>("ModernCharacters/" + name + "/Character");
            var skin = Resources.Load<VoxelGoatAsset>("ModernCharacters/" + name + "/Voxels/" + name + resolution);
            Assert.That(source, Is.Not.Null); Assert.That(skin, Is.Not.Null);
            Assert.That(skin.meshFormat, Is.EqualTo(1), "Run VoxelGoatImport.Prepare to optimize the preserved derived asset");
            Assert.That(skin.cellFaceMasks.Length, Is.EqualTo(skin.cellColors.Length));
            Assert.That(skin.cellCenters.Length, Is.EqualTo(skin.cellColors.Length));
            Assert.That(skin.cellWeights.Length, Is.EqualTo(skin.cellColors.Length));
            var inconsistent = Object.Instantiate(skin);
            try
            {
                inconsistent.cellFaceMasks = (byte[])skin.cellFaceMasks.Clone();
                inconsistent.cellFaceMasks[0] ^= 1;
                Assert.Throws<System.InvalidOperationException>(() => VoxelCharacterSkin.PrepareDetail(inconsistent),
                    "A face mask that does not match mesh vertices must fail before actor construction");
            }
            finally { Object.DestroyImmediate(inconsistent); }
            var grid = new Dictionary<Vector3Int, int>();
            Vector3Int LocalKey(Vector3 p) => new Vector3Int(Mathf.RoundToInt(p.x / skin.cellSize - .5f),
                Mathf.RoundToInt(p.y / skin.cellSize - .5f), Mathf.RoundToInt(p.z / skin.cellSize - .5f));
            for (var i = 0; i < skin.cellCenters.Length; i++) grid.Add(LocalKey(skin.cellCenters[i]), i);
            var directions = new[] { Vector3Int.right, Vector3Int.left, Vector3Int.up, Vector3Int.down,
                new Vector3Int(0, 0, 1), new Vector3Int(0, 0, -1) };
            var normals = new[] { Vector3.right, Vector3.left, Vector3.up, Vector3.down, Vector3.forward, Vector3.back };
            var axesU = new[] { Vector3.back, Vector3.forward, Vector3.right, Vector3.right, Vector3.right, Vector3.left };
            var axesV = new[] { Vector3.up, Vector3.up, Vector3.back, Vector3.forward, Vector3.up, Vector3.up };
            var vertices = skin.mesh.vertices; var colors = skin.mesh.colors32; var vertexWeights = skin.mesh.boneWeights;
            var shared = new List<(int cell, int neighbor, int face)>();
            var faceCount = 0;
            for (var i = 0; i < skin.cellCenters.Length; i++)
            for (var face = 0; face < 6; face++)
            {
                var neighborExists = grid.TryGetValue(LocalKey(skin.cellCenters[i]) + directions[face], out var neighbor);
                var shouldCull = neighborExists && skin.cellWeights[i].Equals(skin.cellWeights[neighbor]);
                var retained = (skin.cellFaceMasks[i] & (1 << face)) != 0;
                if (retained == shouldCull) Assert.Fail($"Unsafe or missing face: cell={i} face={face}");
                if (!retained)
                {
                    if (shared.Count < 256 && (face % 2) == 0) shared.Add((i, neighbor, face));
                    continue;
                }
                var first = faceCount * 4;
                var expected = skin.cellCenters[i] + (normals[face] - axesU[face] - axesV[face]) * (skin.cellSize * .5f);
                if (Vector3.Distance(vertices[first], expected) > skin.cellSize * .0001f || !colors[first].Equals(skin.cellColors[i]))
                    Assert.Fail($"Rendered face geometry or color differs at cell={i} face={face}");
                var w = vertexWeights[first]; var expectedWeight = skin.cellWeights[i];
                if (w.boneIndex0 != expectedWeight.boneIndex0 || Mathf.Abs(w.weight0 - expectedWeight.weight0) > .0001f)
                    Assert.Fail($"Rendered face skin differs at cell={i} face={face}");
                faceCount++;
            }
            Assert.That(shared.Count, Is.GreaterThan(0));
            Assert.That(skin.mesh.vertexCount, Is.EqualTo(faceCount * 4));
            Assert.That(skin.mesh.vertexCount, Is.LessThan(skin.cellColors.Length * 24));
            TestContext.WriteLine($"{name}{resolution} sourceCells={skin.cellColors.Length} fullVertices={skin.cellColors.Length * 24} optimizedVertices={skin.mesh.vertexCount}");
            using var character = new ModernCharacterInstance(source, null);
            using var voxel = new VoxelCharacterSkin(character.Root, skin);
            var clips = ModernCreatureAssets.TryGet(name, out var creature) ? creature.Clips
                : name == "Goat" ? new[] { "Idle", "Walk", "Sprint" }
                : name == "Villager" ? new[] { "Idle", "Walk", "Run", "Hit", "Death" }
                : new[] { "Idle", "StandingShot", "KneelingShot" };
            foreach (var clip in clips)
            {
                character.Sample(clip, Mathf.Min(.35f, source.Clip(clip).length * .5f));
                var poses = skin.mesh.bindposes;
                var matrices = voxel.Renderer.bones.Select((bone, index) => bone.localToWorldMatrix * poses[index]).ToArray();
                Vector3 SkinPoint(Vector3 point, BoneWeight w)
                {
                    var world = matrices[w.boneIndex0].MultiplyPoint3x4(point) * w.weight0;
                    if (w.weight1 > 0) world += matrices[w.boneIndex1].MultiplyPoint3x4(point) * w.weight1;
                    if (w.weight2 > 0) world += matrices[w.boneIndex2].MultiplyPoint3x4(point) * w.weight2;
                    if (w.weight3 > 0) world += matrices[w.boneIndex3].MultiplyPoint3x4(point) * w.weight3;
                    return world;
                }
                foreach (var pair in shared)
                {
                    var f = pair.face;
                    for (var u = -1; u <= 1; u += 2)
                    for (var v = -1; v <= 1; v += 2)
                    {
                        var side = (axesU[f] * u + axesV[f] * v) * (skin.cellSize * .5f);
                        var a = skin.cellCenters[pair.cell] + normals[f] * (skin.cellSize * .5f) + side;
                        var b = skin.cellCenters[pair.neighbor] - normals[f] * (skin.cellSize * .5f) + side;
                        var delta = Vector3.Distance(SkinPoint(a, skin.cellWeights[pair.cell]), SkinPoint(b, skin.cellWeights[pair.neighbor]));
                        if (delta > skin.cellSize * Mathf.Abs(voxel.Source.transform.lossyScale.x) * .001f)
                            Assert.Fail($"A culled face opened in {clip}: {delta}");
                    }
                }
            }
            AssertPoseGrid(character.Root, skin, voxel.SnapshotFragments());
        }

        private static Vector3Int GridKey(Vector3 position, Vector3 origin, float size)
        {
            var p = (position - origin) / size;
            return new Vector3Int(Mathf.RoundToInt(p.x), Mathf.RoundToInt(p.y), Mathf.RoundToInt(p.z));
        }

        // BakeMesh evaluates every cube vertex, while the snapshot skins a
        // cached center. Their equivalent arithmetic can differ by a few ULPs
        // exactly at a cell face. Only accept an adjacent key when the two
        // independently computed positions straddle that face within the same
        // numerical tolerance as the half-cell geometry check.
        // Both TransformPoint and skinning use floats. World translation can
        // dominate the error budget as cells get smaller. Bound the allowance
        // by eight float epsilons at the represented world magnitude, with the
        // original 0.0001-cell floor and a strict 0.001-cell ceiling.
        private static float AxisTolerance(float a, float b, float origin, float size)
        {
            const float floatEpsilon = 1.1920929e-7f;
            var magnitude = Mathf.Max(1f, Mathf.Max(Mathf.Abs(origin), Mathf.Max(Mathf.Abs(a), Mathf.Abs(b))));
            return Mathf.Min(size * .001f, Mathf.Max(size * .0001f, 8f * floatEpsilon * magnitude));
        }

        private static Vector3Int ResolveOracleKey(Vector3 baked, Vector3 manual, Vector3 origin, float size)
        {
            var bakedKey = GridKey(baked, origin, size);
            var manualKey = GridKey(manual, origin, size);
            for (var axis = 0; axis < 3; axis++)
            {
                var tolerance = AxisTolerance(baked[axis], manual[axis], origin[axis], size);
                Assert.That(Mathf.Abs(baked[axis] - manual[axis]), Is.LessThanOrEqualTo(tolerance), "CPU and baked centers differ beyond floating-point tolerance");
                if (bakedKey[axis] == manualKey[axis]) continue;
                Assert.That(Mathf.Abs(bakedKey[axis] - manualKey[axis]), Is.EqualTo(1), "Only adjacent cells can meet at a face");
                var distance = Mathf.Abs(baked[axis] - (origin[axis] + manualKey[axis] * size));
                Assert.That(distance, Is.InRange(size * .5f - tolerance, size * .5f + tolerance), "An alternate cell must touch the baked center within numeric tolerance");
            }
            return manualKey;
        }

        [Test]
        public void PoseOracleAllowsOnlyNumericalAmbiguityAtSharedGridFaces()
        {
            var origin = Vector3.zero;
            var below = new Vector3(.49998f, .2f, -.1f);
            var above = new Vector3(.50002f, .2f, -.1f);
            Assert.That(ResolveOracleKey(below, above, origin, 1f), Is.EqualTo(new Vector3Int(1, 0, 0)));
            Assert.That(ResolveOracleKey(above, below, origin, 1f), Is.EqualTo(Vector3Int.zero));
            Assert.That(ResolveOracleKey(new Vector3(.1f, .2f, -.1f), new Vector3(.10001f, .2f, -.1f), origin, 1f), Is.EqualTo(Vector3Int.zero));
            Assert.Throws<AssertionException>(() => ResolveOracleKey(new Vector3(.49f, 0, 0), new Vector3(.51f, 0, 0), origin, 1f));
            // Reproduced 96-cell goat check: three ULPs at x=4 exceed the
            // old cell-only allowance but are below eight world-coordinate ULPs.
            const float fineSize = .009991353f;
            var worldOrigin = new Vector3(4, 0, 0);
            var left = new Vector3(4 + fineSize * .5f - .0000007f, 0, 0);
            var right = new Vector3(4 + fineSize * .5f + .0000007f, 0, 0);
            Assert.That(ResolveOracleKey(left, right, worldOrigin, fineSize), Is.EqualTo(new Vector3Int(1, 0, 0)));
            Assert.Throws<AssertionException>(() => ResolveOracleKey(new Vector3(4 + fineSize * .49f, 0, 0), new Vector3(4 + fineSize * .51f, 0, 0), worldOrigin, fineSize));
        }

        // Independently bake the visible skin. Every original centre maps to a
        // unique returned grid cell within half a cell per axis, and collisions
        // carry the arithmetic mean of all contributing original colors.
        private static void AssertPoseGrid(GameObject root, VoxelGoatAsset asset, List<VoxelFragment> fragments)
        {
            var renderer = root.GetComponentsInChildren<SkinnedMeshRenderer>(true).Single(r => r.sharedMesh == asset.mesh);
            var mesh = new Mesh();
            GameObject probeObject = null;
            Mesh probeMesh = null;
            try
            {
                var sampledRenderer = renderer;
                if (asset.meshFormat == 1)
                {
                    // Independently ask Unity to skin the saved source centers.
                    // Optimized display meshes do not contain 24 vertices per cell.
                    probeMesh = new Mesh { indexFormat = UnityEngine.Rendering.IndexFormat.UInt32 };
                    probeMesh.vertices = asset.cellCenters; probeMesh.boneWeights = asset.cellWeights;
                    probeMesh.bindposes = asset.mesh.bindposes;
                    probeMesh.SetIndices(Enumerable.Range(0, asset.cellCenters.Length).ToArray(), MeshTopology.Points, 0);
                    probeObject = new GameObject("Voxel pose reference");
                    probeObject.transform.SetParent(renderer.transform, false);
                    sampledRenderer = probeObject.AddComponent<SkinnedMeshRenderer>();
                    sampledRenderer.sharedMesh = probeMesh; sampledRenderer.bones = renderer.bones;
                    sampledRenderer.rootBone = renderer.rootBone; sampledRenderer.quality = SkinQuality.Bone4;
                }
                sampledRenderer.BakeMesh(mesh, true);
                var vertices = mesh.vertices;
                var origin = root.transform.position;
                var size = fragments[0].Size;
                var result = new Dictionary<Vector3Int, VoxelFragment>();
                foreach (var fragment in fragments)
                {
                    Assert.That(fragment.Size, Is.EqualTo(size));
                    var key = GridKey(fragment.Position, origin, size);
                    Assert.That(result.ContainsKey(key), Is.False, "Grid cells must not overlap or repeat");
                    result.Add(key, fragment);
                    Assert.That(Vector3.Distance(fragment.Position, origin + (Vector3)key * size), Is.LessThan(size * .0001f));
                }
                var expected = new Dictionary<Vector3Int, List<Color32>>();
                var hasCenters = asset.cellCenters != null && asset.cellCenters.Length == asset.cellColors.Length &&
                    asset.cellWeights != null && asset.cellWeights.Length == asset.cellColors.Length;
                var restVertices = hasCenters ? null : asset.mesh.vertices;
                var restWeights = hasCenters ? null : asset.mesh.boneWeights;
                var bindposes = asset.mesh.bindposes;
                var matrices = renderer.bones.Select((bone, i) => bone.localToWorldMatrix * bindposes[i]).ToArray();
                for (var i = 0; i < asset.cellColors.Length; i++)
                {
                    var center = Vector3.zero;
                    if (asset.meshFormat == 1) center = vertices[i];
                    else { for (var j = 0; j < 24; j++) center += vertices[i * 24 + j]; center /= 24; }
                    var world = sampledRenderer.transform.TransformPoint(center);
                    var rest = hasCenters ? asset.cellCenters[i] : Vector3.zero;
                    if (!hasCenters)
                    {
                        for (var j = 0; j < 24; j++) rest += restVertices[i * 24 + j];
                        rest /= 24;
                    }
                    var weights = hasCenters ? asset.cellWeights[i] : restWeights[i * 24];
                    var manual = matrices[weights.boneIndex0].MultiplyPoint3x4(rest) * weights.weight0;
                    if (weights.weight1 > 0) manual += matrices[weights.boneIndex1].MultiplyPoint3x4(rest) * weights.weight1;
                    if (weights.weight2 > 0) manual += matrices[weights.boneIndex2].MultiplyPoint3x4(rest) * weights.weight2;
                    if (weights.weight3 > 0) manual += matrices[weights.boneIndex3].MultiplyPoint3x4(rest) * weights.weight3;
                    var key = ResolveOracleKey(world, manual, origin, size);
                    Assert.That(result.TryGetValue(key, out var fragment), Is.True, "Every baked centre has a fragment");
                    var delta = world - fragment.Position;
                    for (var axis = 0; axis < 3; axis++)
                    {
                        var tolerance = AxisTolerance(world[axis], fragment.Position[axis], origin[axis], size);
                        Assert.That(Mathf.Abs(delta[axis]), Is.LessThanOrEqualTo(size * .5f + tolerance));
                    }
                    if (!expected.TryGetValue(key, out var colors)) expected.Add(key, colors = new List<Color32>());
                    colors.Add(asset.cellColors[i]);
                }
                Assert.That(expected.Values.Sum(colors => colors.Count), Is.EqualTo(asset.cellColors.Length));
                Assert.That(result.Count, Is.EqualTo(expected.Count));
                foreach (var pair in expected)
                {
                    var colors = pair.Value;
                    var expectedColor = new Color32((byte)(colors.Sum(c => (long)c.r) / colors.Count),
                        (byte)(colors.Sum(c => (long)c.g) / colors.Count), (byte)(colors.Sum(c => (long)c.b) / colors.Count),
                        (byte)(colors.Sum(c => (long)c.a) / colors.Count));
                    Assert.That(result[pair.Key].Color, Is.EqualTo(expectedColor));
                }
            }
            finally
            {
                Object.DestroyImmediate(mesh);
                if (probeObject != null) Object.DestroyImmediate(probeObject);
                if (probeMesh != null) Object.DestroyImmediate(probeMesh);
            }
        }
    }
}
