using NUnit.Framework;
using UnityEngine;
using System.Linq;

namespace MagicCarpet.Native.Tests
{
    public sealed class ModernWaterBedEditModeTests
    {
        [Test]
        public void VoxelReceiptCountsCurrentBaseWhileRetainingHistoricalIntent()
        {
            var terrain = new NativeCampaignSession.VoxelVolumeProjection {
                kind = 0, slot = 0, dimensions = new[] { 1, 1, 8 }, terrain_columns = new[] { 4 }
            };
            var house = new NativeCampaignSession.VoxelVolumeProjection {
                kind = 1, slot = 38, dimensions = new[] { 1, 1, 8 }, building_bytes = new[] { 2 }
            };
            var removed = new System.Collections.Generic.HashSet<(int kind,int slot,int x,int y,int z)> {
                (0,0,0,0,1), (0,0,0,0,3), (1,38,0,0,1)
            };
            Assert.That(Voxels.VoxelModernWorld.EffectiveRemovalCounts(removed, terrain, house), Is.EqualTo((2, 1)));
            terrain.terrain_columns[0] = 2;
            Assert.That(Voxels.VoxelModernWorld.EffectiveRemovalCounts(removed, terrain, house), Is.EqualTo((1, 1)));
            Assert.That(removed.Count, Is.EqualTo(3), "source lowering must retain the historical hole");
            terrain.terrain_columns[0] = 4;
            Assert.That(Voxels.VoxelModernWorld.EffectiveRemovalCounts(removed, terrain, house), Is.EqualTo((2, 1)));
        }

        [Test]
        public void LocalBedRefreshMatchesFreshGeometryAcrossLandCoastSeamAndReset()
        {
            var root = new GameObject("Incremental lake bed test");
            var material = new Material(Shader.Find("Hidden/InternalErrorShader"));
            var heights = new byte[65536]; var types = new byte[65536]; var flags = new byte[65536];
            System.Array.Fill(types, (byte)1);
            for (int z = 0; z < 256; z++) for (int x = 0; x < 256; x++)
                if (x < 32 || (x >= 64 && x < 96)) flags[x + z * 256] = 8;
            try
            {
                using var terrain = new ModernTerrainMesh(root.transform, material, material);
                using var coast = new ModernWaterCoast();
                using var bed = new ModernWaterBed(root.transform, material);
                terrain.Update(heights, types, flags, 1, 0); coast.Update(terrain); bed.Update(terrain, coast, Vector3.zero);
                Assert.That(bed.LastRebuiltTileCount, Is.EqualTo(64));
                var changes = new System.Action[] {
                    () => heights[80 + 80 * 256] = 12,
                    () => heights[100 + 100 * 256] = 12,
                    () => heights[31 + 31 * 256] = 8,
                    () => heights[255 + 255 * 256] = 10,
                    () => types[2 + 2 * 256] = 7,
                    () => heights[25 + 25 * 256] = 9,
                    () => heights[38 + 38 * 256] = 11,
                    () => flags[10 + 10 * 256] = 0,
                    () => { }
                };
                for (int turn = 0; turn < changes.Length; turn++)
                {
                    changes[turn]();
                    var shapeBefore = coast.ShapeRevision;
                    terrain.Update(heights, types, flags, turn == changes.Length - 1 ? 2UL : 1UL, (ulong)turn + 1);
                    if (turn == 3)
                    {
                        // Several simulation snapshots may arrive before the bed is presented.
                        types[70 + 180 * 256] = 7;
                        terrain.Update(heights, types, flags, 1, 100);
                    }
                    coast.Update(terrain); bed.Update(terrain, coast, Vector3.zero);
                    if (turn < changes.Length - 2)
                    {
                        Assert.That(coast.ShapeRevision, Is.EqualTo(shapeBefore));
                        Assert.That(bed.LastRebuiltTileCount, Is.InRange(1, 63));
                        if (turn == 0)
                        {
                            Assert.That(bed.LastRebuiltTileCount, Is.EqualTo(1),
                                "an interior terrain edit should only invalidate its own bed tile");
                            Assert.That(bed.LastUpdatedVertexCount, Is.InRange(1, 625),
                                "a local edit should resample its vertex halo, not the whole tile");
                        }
                    }
                    else Assert.That(bed.LastRebuiltTileCount, Is.EqualTo(64));
                    AssertMatchesFresh(root, material, terrain, coast, turn);
                }
            }
            finally { Object.DestroyImmediate(root); Object.DestroyImmediate(material); }
        }

        [Test]
        public void VoxelSamplerChangesAndRemovalRefreshBedWithoutTerrainRevision()
        {
            var root = new GameObject("Voxel sampler lake bed test");
            var material = new Material(Shader.Find("Hidden/InternalErrorShader"));
            var heights = new byte[65536]; var types = new byte[65536]; var flags = new byte[65536];
            System.Array.Fill(types, (byte)1);
            for (var z = 64; z < 96; z++) for (var x = 64; x < 96; x++) flags[x + z * 256] = 8;
            float externalHeight = 1;
            try
            {
                using var terrain = new ModernTerrainMesh(root.transform, material, material);
                var footprint = new Bounds(new Vector3(320, 0, 320), new Vector3(2, 0, 2));
                float? Sample(float x, float z) => x >= 319 && x < 321 && z >= 319 && z < 321 ? externalHeight : null;
                terrain.SetVoxelHeightSampler(Sample, footprint);
                using var coast = new ModernWaterCoast();
                using var bed = new ModernWaterBed(root.transform, material);
                terrain.Update(heights, types, flags, 1, 0); coast.Update(terrain); bed.Update(terrain, coast, Vector3.zero);
                var revision = terrain.Revision;
                var mesh = root.transform.Find("Visual lake bed/Lake bed 18").GetComponent<MeshFilter>().sharedMesh;
                var originalHeight = mesh.vertices[32 + 32 * 65].y;
                externalHeight = 2;
                terrain.InvalidateVoxelHeight(footprint);
                bed.Update(terrain, coast, Vector3.zero);
                Assert.That(terrain.Revision, Is.EqualTo(revision));
                Assert.That(bed.LastRebuiltTileCount, Is.EqualTo(1));
                Assert.That(bed.LastUpdatedVertexCount, Is.InRange(1, 9));
                Assert.That(mesh.vertices[32 + 32 * 65].y, Is.EqualTo(originalHeight + 1));
                AssertMatchesFresh(root, material, terrain, coast, 0);
                var replacement = new Bounds(new Vector3(336, 0, 336), new Vector3(2, 0, 2));
                float? Other(float x, float z) => x >= 335 && x < 337 && z >= 335 && z < 337 ? (float?)3 : null;
                terrain.SetVoxelHeightSampler(Other, replacement);
                bed.Update(terrain, coast, Vector3.zero);
                Assert.That(terrain.Revision, Is.EqualTo(revision));
                Assert.That(mesh.vertices[32 + 32 * 65].y, Is.Not.EqualTo(originalHeight + 1));
                AssertMatchesFresh(root, material, terrain, coast, 1);
                terrain.SetVoxelHeightSampler(null, null);
                bed.Update(terrain, coast, Vector3.zero);
                Assert.That(terrain.Revision, Is.EqualTo(revision));
                AssertMatchesFresh(root, material, terrain, coast, 2);
                terrain.SetVoxelHeightSampler(Other, replacement);
                bed.Update(terrain, coast, Vector3.zero);
                heights[70 + 70 * 256] = 12;
                terrain.Update(heights, types, flags, 1, 1); coast.Update(terrain); bed.Update(terrain, coast, Vector3.zero);
                Assert.That(bed.LastUpdatedVertexCount, Is.InRange(1, 625));
                AssertMatchesFresh(root, material, terrain, coast, 3);
            }
            finally { Object.DestroyImmediate(root); Object.DestroyImmediate(material); }
        }

        [Test]
        public void SourceShoreOrientationCoverageAndResetMatchFreshBed()
        {
            var root = new GameObject("Source shore lake bed test");
            var material = new Material(Shader.Find("Hidden/InternalErrorShader"));
            var atlas = new byte[256 * 608]; System.Array.Fill(atlas, (byte)12);
            for (var z = 0; z < 32; z++) for (var x = 0; x < 16; x++)
                atlas[(44 / 8 * 32 + z) * 256 + 44 % 8 * 32 + x] = 0;
            var heights = new byte[65536]; var types = new byte[65536]; var flags = new byte[65536];
            System.Array.Fill(types, (byte)1);
            for (var z = 0; z < 256; z++) for (var x = 0; x < 32; x++) flags[x + z * 256] = 8;
            var cell = 32 + 32 * 256; types[cell] = 44;
            try
            {
                using var mask = new ModernShoreMask(atlas);
                using var terrain = new ModernTerrainMesh(root.transform, material, material, mask);
                using var coast = new ModernWaterCoast();
                using var bed = new ModernWaterBed(root.transform, material);
                terrain.Update(heights, types, flags, 1, 0); coast.Update(terrain); bed.Update(terrain, coast, Vector3.zero);
                Assert.That(mask.SampleWorld(128.5f, 129.5f), Is.EqualTo(1));
                Assert.That(mask.SampleWorld(131.5f, 129.5f), Is.Zero);
                for (var turn = 0; turn < 4; turn++)
                {
                    var shapeBefore = coast.ShapeRevision;
                    if (turn == 0) flags[cell] = 0x10; // Flip the partial atlas coverage.
                    if (turn == 1) types[cell] = 1; // Remove coverage and its water cell.
                    if (turn == 2) types[cell] = 44;
                    terrain.Update(heights, types, flags, turn == 3 ? 2UL : 1UL, (ulong)turn + 1);
                    coast.Update(terrain); bed.Update(terrain, coast, Vector3.zero);
                    if (turn == 0)
                    {
                        Assert.That(mask.SampleWorld(128.5f, 129.5f), Is.Zero);
                        Assert.That(mask.SampleWorld(131.5f, 129.5f), Is.EqualTo(1));
                        Assert.That(coast.ShapeRevision, Is.EqualTo(shapeBefore));
                        Assert.That(bed.LastRebuiltTileCount, Is.InRange(1, 63));
                    }
                    else Assert.That(bed.LastRebuiltTileCount, Is.EqualTo(64));
                    AssertMatchesFresh(root, material, terrain, coast, turn);
                }
            }
            finally { Object.DestroyImmediate(root); Object.DestroyImmediate(material); }
        }

        private static void AssertMatchesFresh(GameObject root, Material material, ModernTerrainMesh terrain,
            ModernWaterCoast coast, int turn)
        {
            var freshRoot = new GameObject("Fresh full lake bed");
            try
            {
                using var fresh = new ModernWaterBed(freshRoot.transform, material);
                fresh.Update(terrain, coast, Vector3.zero);
                for (int tile = 0; tile < 64; tile++)
                {
                    var existing = root.transform.Find($"Visual lake bed/Lake bed {tile}").GetComponent<MeshFilter>().sharedMesh;
                    var expected = freshRoot.transform.Find($"Visual lake bed/Lake bed {tile}").GetComponent<MeshFilter>().sharedMesh;
                    Assert.That(existing.vertices.SequenceEqual(expected.vertices) && existing.normals.SequenceEqual(expected.normals) &&
                        existing.colors.SequenceEqual(expected.colors) && existing.triangles.SequenceEqual(expected.triangles), Is.True,
                        $"Every lake-bed vertex, normal, color and index must match a full rebuild: turn {turn}, tile {tile}");
                }
            }
            finally { Object.DestroyImmediate(freshRoot); }
        }

        [Test]
        public void BedMatchesVisualCoastWrapsAndRemovesGeometryAfterDryMutation()
        {
            var root=new GameObject("Lake bed test");
            var material=new Material(Shader.Find("Hidden/InternalErrorShader"));
            var heights=new byte[65536];var types=new byte[65536];var flags=new byte[65536];
            System.Array.Fill(types,(byte)1);
            for(var z=0;z<256;z++) for(var x=0;x<32;x++) flags[x+z*256]=8;
            using(var terrain=new ModernTerrainMesh(root.transform,material,material))
            using(var coast=new ModernWaterCoast())
            using(var bed=new ModernWaterBed(root.transform,material))
            {
                terrain.Update(heights,types,flags,1,0);coast.Update(terrain);
                Assert.That(ModernWaterBed.Height(terrain,coast,0,12),Is.EqualTo(terrain.SampleHeight(0,12)),"no gap between land and transparent water bed");
                Assert.That(ModernWaterBed.Height(terrain,coast,16,12),Is.LessThan(-5));
                Assert.That(ModernWaterBed.Height(terrain,coast,16,12),
                    Is.EqualTo(ModernWaterBed.Height(terrain,coast,1040,1036)).Within(.001));
                bed.Update(terrain,coast,new Vector3(1030,4,12));
                var tile=root.transform.Find("Visual lake bed/Lake bed 0");
                Assert.That(tile.gameObject.activeSelf,Is.True);
                Assert.That(tile.position.x,Is.EqualTo(1024));
                var mesh=tile.GetComponent<MeshFilter>().sharedMesh;
                Assert.That(mesh.GetIndexCount(0),Is.EqualTo(64*64*6));
                Assert.That(mesh.vertices[8+6*65].y,Is.EqualTo(ModernWaterBed.Height(terrain,coast,16,12)).Within(.001));
                Assert.That(mesh.colors[8+6*65],Is.EqualTo(terrain.DetailColor(8,6)),"land and bed retain identical source tint");
                System.Array.Clear(flags,0,flags.Length);
                terrain.Update(heights,types,flags,1,1);
                Assert.Throws<System.InvalidOperationException>(()=>bed.Update(terrain,coast,Vector3.zero));
                coast.Update(terrain);bed.Update(terrain,coast,Vector3.zero);
                Assert.That(tile.gameObject.activeSelf,Is.False);
                Assert.That(mesh.vertexCount,Is.Zero);
            }
            Object.DestroyImmediate(material);Object.DestroyImmediate(root);
        }
    }
}
