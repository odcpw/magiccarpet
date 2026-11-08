using NUnit.Framework;
using System.Linq;
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

        [Test]
        public void LiveBuildingsFollowSourceOwnerAndNeverFlattenChangedStamp()
        {
            var root=new GameObject("Live source buildings");
            var layout=ModernBuildingLayoutEditModeTests.Layout(out var source);
            var rows=layout.buildings.Select(entry=>new ModernBuildings.LiveBuilding {
                Slot=entry.source_slot, SlotGeneration=0, BuildingIndex=entry.build_index,
                Action=52, Drawing=true, X=(ushort)entry.source_entity_position.x,
                Y=(ushort)entry.source_entity_position.y, Z=(short)entry.source_entity_position.z }).ToArray();
            try
            {
                using(var buildings=new ModernBuildings(root.transform,null,layout))
                {
                    // An unrelated source height change invalidates the old
                    // global digest, but not any of the five intact stamps.
                    source[0]=7;
                    var visual=buildings.Reconcile(source,rows,0,out var changed);
                    Assert.That(changed,Is.True);Assert.That(visual[0],Is.EqualTo(7));
                    Assert.That(buildings.ChangedFootprints.Count,Is.EqualTo(4));
                    Assert.That(visual[layout.buildings[0].cells[0].index],Is.EqualTo(20));
                    Assert.That(root.transform.Find("Level 1 building 0 BUILD 26").gameObject.activeSelf,Is.False,
                        "voxel house cannot overlap an intact facade");
                    Assert.That(root.transform.Find("Level 1 building 1 BUILD 29").gameObject.activeSelf,Is.True);
                    var perBuilding=ModernBuildingMeshes.WindowCenters(layout.profiles[0]).Length;
                    Assert.That(perBuilding,Is.GreaterThan(0));
                    Assert.That(buildings.WindowCenters.Length,Is.EqualTo(perBuilding*4));
                    Assert.That(buildings.WindowNormals.Length,Is.EqualTo(perBuilding*4));
                    buildings.Reconcile(source,rows,0,out changed);Assert.That(changed,Is.False);
                    var damaged=layout.buildings[1].cells[0];source[damaged.index]--;
                    visual=buildings.Reconcile(source,rows,0,out changed);
                    Assert.That(changed,Is.True);
                    Assert.That(buildings.ChangedFootprints.Count,Is.EqualTo(1));
                    Assert.That(root.transform.Find("Level 1 building 1 BUILD 29").gameObject.activeSelf,Is.False);
                    Assert.That(buildings.WindowCenters.Length,Is.EqualTo(perBuilding*3),"hidden facade has no window lights");
                    foreach(var cell in layout.buildings[1].cells)
                        Assert.That(visual[cell.index],Is.EqualTo(source[cell.index]),"do not flatten any part of a changed stamp");
                    Assert.That(root.transform.Find("Level 1 building 2 BUILD 30").gameObject.activeSelf,Is.True);
                    source[damaged.index]=(byte)damaged.expected;
                    rows[1].SlotGeneration=1;
                    buildings.Reconcile(source,rows,0,out changed);
                    Assert.That(root.transform.Find("Level 1 building 1 BUILD 29").gameObject.activeSelf,Is.False,"reused source slot");
                    rows[1].SlotGeneration=0;rows[1].BuildingIndex=42;
                    buildings.Reconcile(source,rows,0,out changed);
                    Assert.That(root.transform.Find("Level 1 building 1 BUILD 29").gameObject.activeSelf,Is.False,"changed BUILD selector");
                    rows[1].BuildingIndex=29;rows[1].Action=53;
                    buildings.Reconcile(source,rows,0,out changed);
                    Assert.That(root.transform.Find("Level 1 building 1 BUILD 29").gameObject.activeSelf,Is.False,"destruction transition");
                    rows[1].Action=52;
                    buildings.Reconcile(source,rows,0,out changed);
                    Assert.That(root.transform.Find("Level 1 building 1 BUILD 29").gameObject.activeSelf,Is.True);
                    rows[1].Drawing=false;
                    buildings.Reconcile(source,rows,0,out changed);
                    Assert.That(root.transform.Find("Level 1 building 1 BUILD 29").gameObject.activeSelf,Is.False,"source draw disabled");
                    rows[1].Drawing=true;rows[1].X++;
                    buildings.Reconcile(source,rows,0,out changed);
                    Assert.That(root.transform.Find("Level 1 building 1 BUILD 29").gameObject.activeSelf,Is.False,"source moved");
                    rows[1].X--;rows[1].Z++;
                    buildings.Reconcile(source,rows,0,out changed);
                    Assert.That(root.transform.Find("Level 1 building 1 BUILD 29").gameObject.activeSelf,Is.False,"source height changed");
                    rows[1].Z--;
                    buildings.Reconcile(source,rows,0,out changed);
                    Assert.That(buildings.WindowCenters.Length,Is.EqualTo(perBuilding*4),"restored facade restores lights");
                }
            }
            finally { Object.DestroyImmediate(root); }
        }
    }
}
