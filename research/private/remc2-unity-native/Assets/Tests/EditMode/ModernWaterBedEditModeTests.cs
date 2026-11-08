using NUnit.Framework;
using UnityEngine;
using System.Linq;

namespace MagicCarpet.Native.Tests
{
    public sealed class ModernWaterBedEditModeTests
    {
        [Test]
        public void LocalBedRefreshMatchesFreshGeometryAcrossLandCoastSeamAndReset()
        {
            var root = new GameObject("Incremental lake bed test");
            var material = new Material(Shader.Find("Hidden/InternalErrorShader"));
            var heights = new byte[65536]; var types = new byte[65536]; var flags = new byte[65536];
            System.Array.Fill(types, (byte)1);
            for (int z = 0; z < 256; z++) for (int x = 0; x < 32; x++) flags[x + z * 256] = 8;
            try
            {
                using var terrain = new ModernTerrainMesh(root.transform, material, material);
                using var coast = new ModernWaterCoast();
                using var bed = new ModernWaterBed(root.transform, material);
                terrain.Update(heights, types, flags, 1, 0); coast.Update(terrain); bed.Update(terrain, coast, Vector3.zero);
                Assert.That(bed.LastRebuiltTileCount, Is.EqualTo(64));
                var changes = new System.Action[] {
                    () => heights[100 + 100 * 256] = 12,
                    () => heights[31 + 31 * 256] = 8,
                    () => heights[255 + 255 * 256] = 10,
                    () => types[2 + 2 * 256] = 7,
                    () => flags[10 + 10 * 256] = 0,
                    () => { }
                };
                for (int turn = 0; turn < changes.Length; turn++)
                {
                    changes[turn]();
                    var shapeBefore = coast.ShapeRevision;
                    terrain.Update(heights, types, flags, turn == 5 ? 2UL : 1UL, (ulong)turn + 1);
                    coast.Update(terrain); bed.Update(terrain, coast, Vector3.zero);
                    if (turn < 4)
                    {
                        Assert.That(coast.ShapeRevision, Is.EqualTo(shapeBefore));
                        Assert.That(bed.LastRebuiltTileCount, Is.InRange(1, 63));
                    }
                    else Assert.That(bed.LastRebuiltTileCount, Is.EqualTo(64));
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
            }
            finally { Object.DestroyImmediate(root); Object.DestroyImmediate(material); }
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
