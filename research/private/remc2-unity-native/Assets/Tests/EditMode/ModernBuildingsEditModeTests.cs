using NUnit.Framework;
using UnityEngine;

namespace MagicCarpet.Native.Tests
{
    public sealed class ModernBuildingsEditModeTests
    {
        [Test]
        public void FailedPlacementRemovesAlreadyCreatedBuildings()
        {
            var root=new GameObject("Building rollback test");
            var layout=ModernBuildingLayoutEditModeTests.Layout(out var source);
            try
            {
                using(var buildings=new ModernBuildings(root.transform,null,layout))
                {
                    Assert.Throws<System.InvalidOperationException>(()=>buildings.Stage(Vector3.zero));
                    buildings.PrepareTerrain(source);
                    // Force a post-validation failure after the first instance exists.
                    layout.buildings[1].build_index=999;
                    Assert.Throws<System.IO.InvalidDataException>(()=>buildings.Stage(Vector3.zero));
                    Assert.That(buildings.Ready,Is.False);Assert.That(buildings.Count,Is.Zero);
                    Assert.That(buildings.WindowCenters.Length,Is.Zero);
                    Assert.That(root.GetComponentsInChildren<LODGroup>().Length,Is.Zero);
                }
            }
            finally { Object.DestroyImmediate(root); }
        }

        [Test]
        public void StagedSourceBuildingsShareMeshesHaveLodsAndExcludeWrappedFootprints()
        {
            var root=new GameObject("Source building placement test");
            var layout=ModernBuildingLayoutEditModeTests.Layout(out var source);
            try
            {
                using(var buildings=new ModernBuildings(root.transform,null,layout))
                {
                    buildings.PrepareTerrain(source);buildings.Stage(new Vector3(100,10,100));
                    Assert.That(buildings.Count,Is.EqualTo(5));
                    Assert.That(buildings.ExcludesVegetation(buildings.House.x,buildings.House.z),Is.True);
                    Assert.That(buildings.ExcludesVegetation(buildings.House.x+1024,buildings.House.z-1024),Is.True);
                    Assert.That(buildings.ExcludesVegetation(buildings.House.x+300,buildings.House.z),Is.False);
                    var groups=root.GetComponentsInChildren<LODGroup>();Assert.That(groups.Length,Is.EqualTo(5));
                    foreach(var group in groups)
                    {
                        var levels=group.GetLODs();Assert.That(levels.Length,Is.EqualTo(3));
                        Assert.That(levels[0].screenRelativeTransitionHeight,Is.GreaterThan(levels[1].screenRelativeTransitionHeight));
                        foreach(var level in levels) Assert.That(level.renderers.Length,Is.EqualTo(1));
                        Assert.That(group.transform.localScale,Is.EqualTo(Vector3.one),"Source geometry must not be stretched");
                    }
                    Assert.That(groups[0].GetComponentInChildren<MeshFilter>().sharedMesh,
                        Is.SameAs(groups[4].GetComponentInChildren<MeshFilter>().sharedMesh));
                    buildings.Render(new Vector3(1124,10,100));
                    foreach(var group in groups) Assert.That(Mathf.Abs(group.transform.position.x-1124),Is.LessThan(512));
                    buildings.Stage(Vector3.zero);Assert.That(buildings.Count,Is.EqualTo(5));
                }
                Assert.That(root.GetComponentsInChildren<LODGroup>().Length,Is.Zero);
            }
            finally { Object.DestroyImmediate(root); }
        }
    }
}
