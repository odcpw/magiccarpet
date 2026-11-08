using System;
using System.Linq;
using MagicCarpet.Native.Voxels;
using NUnit.Framework;
using UnityEngine;

namespace MagicCarpet.Native.Tests
{
    public sealed class VoxelTerrainContactEditModeTests
    {
        private static bool Ray(Ray ray,out RaycastHit hit,float length=200)=>Physics.Raycast(ray,out hit,length,1<<8);
        [Test]
        public void FirstNativeImpactHasContactBeforeItsFragmentsAreEmitted()
        {
            var parent=new GameObject("First native impact contact");
            var previousSimulation=Physics.simulationMode;
            try
            {
                Physics.simulationMode=SimulationMode.Script;
                using(var contact=new VoxelTerrainContact(parent.transform,(_,_)=>0))
                {
                    var camera=Vector3.zero;
                    var existing=new[]{new Vector3(1024,1,1024)};
                    var impact=new Vector3(4096,1,4096);
                    contact.Update(camera,default,existing);
                    Physics.SyncTransforms();
                    Assert.That(Ray(new Ray(impact+Vector3.up*10,Vector3.down),out _,20),Is.False);

                    // This is the same call made immediately before EmitImpact, without a Render in between.
                    VoxelModernWorld.PrepareEmissionContact(contact,camera,default,existing,0,impact);
                    Assert.That(Ray(new Ray(impact+Vector3.up*10,Vector3.down),out var hit,20),Is.True);
                    Assert.That(hit.point.y,Is.EqualTo(0).Within(.001f));
                    Assert.That(Ray(new Ray(camera+Vector3.up*10,Vector3.down),out _,20),Is.True);
                    Assert.That(Ray(new Ray(existing[0]+Vector3.up*10,Vector3.down),out _,20),Is.True);
                    Assert.That(contact.TileCount,Is.EqualTo(34),"Coverage remains sparse across distant locations");

                    var piece=new GameObject("First impact fragment");
                    piece.transform.SetParent(parent.transform);
                    piece.transform.position=new Vector3(impact.x,.8f,impact.z);
                    var collider=piece.AddComponent<BoxCollider>();
                    collider.size=Vector3.one*.0644f;
                    collider.contactOffset=VoxelRubble.ContactOffset(.07f);
                    var body=piece.AddComponent<Rigidbody>();
                    body.collisionDetectionMode=CollisionDetectionMode.Continuous;
                    body.linearVelocity=new Vector3(173,-117,-28);
                    Physics.SyncTransforms();
                    Physics.Simulate(1f/120);
                    Assert.That(body.position.y,Is.GreaterThan(0),"The first fast step must hit terrain before it can pass below");
                    Assert.That(body.linearVelocity.y,Is.GreaterThan(-117),"Contact must change the incoming vertical motion");
                }
            }
            finally { Physics.simulationMode=previousSimulation;UnityEngine.Object.DestroyImmediate(parent); }
        }
        [Test]
        public void SourceTerrainRevisionRefreshesOnlyChangedContactTiles()
        {
            var parent=new GameObject("Source terrain update regression");float elevation=0;
            try{using(var contact=new VoxelTerrainContact(parent.transform,(x,z)=>x>=0&&x<=32&&z>=0&&z<=32?elevation:0)){
                contact.Update(Vector3.zero,default);
                var retained=parent.GetComponentsInChildren<MeshCollider>().First(c=>c.sharedMesh.bounds.Contains(new Vector3(-48,0,-48))).sharedMesh;
                elevation=3;Assert.That(contact.Update(Vector3.zero,default,null,1),Is.True);Physics.SyncTransforms();
                Assert.That(Ray(new Ray(new Vector3(10,50,10),Vector3.down),out var hit),Is.True);Assert.That(hit.point.y,Is.EqualTo(3).Within(.001f));
                Assert.That(parent.GetComponentsInChildren<MeshCollider>().Any(c=>c.sharedMesh==retained),Is.True);
                Assert.That(contact.Update(Vector3.zero,default,null,2),Is.False,"A source revision with equal sampled heights does not recook tiles");
            }}finally{UnityEngine.Object.DestroyImmediate(parent);}
        }
        [Test]
        public void CameraCutRetainsDebrisTilesWithoutFillingEmptySpace()
        {
            var parent=new GameObject("Sparse contact regression");
            try {using(var contact=new VoxelTerrainContact(parent.transform,(_,_)=>0)){
                var debris=new[]{new Vector3(10,2,10)};
                contact.Update(Vector3.zero,default,debris);
                var retained=parent.GetComponentsInChildren<MeshCollider>().Where(c=>c.sharedMesh.bounds.Contains(new Vector3(10,0,10))).Single().sharedMesh;
                contact.Update(new Vector3(4096,0,4096),default,debris);Physics.SyncTransforms();
                Assert.That(contact.TileCount,Is.EqualTo(25),"Sixteen camera tiles and nine debris tiles, independent of separation");
                Assert.That(Ray(new Ray(new Vector3(10,50,10),Vector3.down),out _),Is.True);
                Assert.That(Ray(new Ray(new Vector3(2048,50,2048),Vector3.down),out _),Is.False);
                Assert.That(parent.GetComponentsInChildren<MeshCollider>().Any(c=>c.sharedMesh==retained),Is.True,"Unchanged debris contacts must remain installed");
                Assert.That(parent.GetComponentsInChildren<MeshCollider>().All(c=>c.sharedMesh.vertexCount==289),Is.True);
            }}finally{UnityEngine.Object.DestroyImmediate(parent);}
        }
        [Test]
        public void RollingDebrisKeepsContactAfterLeavingTheCameraWindow()
        {
            var parent=new GameObject("Rolling debris terrain");
            try { using(var contact=new VoxelTerrainContact(parent.transform,(x,z)=>z*.1f)){
                var camera=new Vector3(244,54,129);var hole=new Bounds(new Vector3(212,0,768),new Vector3(32,200,40));
                contact.Update(camera,hole);Physics.SyncTransforms();
                var ray=new Ray(new Vector3(251,100,213),Vector3.down);
                Assert.That(Ray(ray,out _),Is.False,"Original camera coverage ends at z192");
                var debris=new[]{new Vector3(251,23,213)};
                Assert.That(contact.Update(camera,hole,debris),Is.True);Physics.SyncTransforms();
                Assert.That(Ray(ray,out var hit),Is.True);Assert.That(hit.point.y,Is.EqualTo(21.3f).Within(.001f));
                Assert.That(contact.Update(camera,hole,debris),Is.False,"Unchanged coverage must not cook another mesh");
                Assert.That(Ray(new Ray(camera+Vector3.up*100,Vector3.down),out _),Is.True);
            }} finally {UnityEngine.Object.DestroyImmediate(parent);}
        }
        [Test]
        public void ContactFollowsCameraAndNeverFillsTheVoxelPatch()
        {
            var parent = new GameObject("Terrain contact test");
            try
            {
                using (var contact = new VoxelTerrainContact(parent.transform, (x, z) => x * .1f + z * .2f))
                {
                    var hole = new Bounds(Vector3.zero, new Vector3(32, 10, 32));
                    Assert.That(contact.Update(Vector3.zero, hole), Is.True);
                    Assert.That(contact.Update(new Vector3(31, 0, 0), hole), Is.False);
                    var collider = parent.GetComponentInChildren<MeshCollider>();
                    Assert.That(collider, Is.Not.Null);
                    Assert.That(collider.gameObject.layer, Is.EqualTo(8));
                    Assert.That(parent.GetComponentsInChildren<MeshRenderer>(), Is.Empty);
                    Physics.SyncTransforms();
                    Assert.That(Ray(new Ray(new Vector3(0, 50, 0), Vector3.down), out _, 100), Is.False);
                    Assert.That(Ray(new Ray(new Vector3(20, 50, 0), Vector3.down), out var hit, 100), Is.True);
                    Assert.That(hit.point.y, Is.EqualTo(2).Within(.001f));
                    var previous = parent.GetComponentsInChildren<MeshCollider>().Where(c=>c.sharedMesh.bounds.Contains(new Vector3(20,2,0))).First().sharedMesh;
                    Assert.That(contact.Update(new Vector3(32, 0, 0), hole), Is.True);
                    Assert.That(parent.GetComponentsInChildren<MeshCollider>().Any(c=>c.sharedMesh==previous),Is.True,"Retained terrain is not recooked on camera movement");
                    Physics.SyncTransforms();
                    Assert.That(Ray(new Ray(new Vector3(-60, 50, 0), Vector3.down), out _, 100), Is.False);
                    Assert.That(Ray(new Ray(new Vector3(90, 50, 0), Vector3.down), out _, 100), Is.True);
                    Assert.That(Ray(new Ray(new Vector3(0, 50, 0), Vector3.down), out _, 100), Is.False);
                    Assert.That(contact.Update(new Vector3(32, 0, 0), new Bounds(new Vector3(8, 0, 0), new Vector3(32, 10, 32))), Is.True);
                }
            }
            finally { UnityEngine.Object.DestroyImmediate(parent); }
        }

        [Test]
        public void NonAlignedExclusionOmitsAllIntersectingQuads()
        {
            var parent = new GameObject("Terrain exclusion test");
            try
            {
                using (var contact = new VoxelTerrainContact(parent.transform, (_, _) => 0))
                {
                    contact.Update(Vector3.zero, new Bounds(new Vector3(.3f, 0, .3f), new Vector3(2, 1, 2)));
                    foreach(var collider in parent.GetComponentsInChildren<MeshCollider>()){
                    var mesh = collider.sharedMesh;
                    var vertices = mesh.vertices;
                    var indices = mesh.triangles;
                    for (int i = 0; i < indices.Length; i += 3)
                    {
                        var points = new[] { vertices[indices[i]], vertices[indices[i + 1]], vertices[indices[i + 2]] };
                        float minX = points.Min(p => p.x), maxX = points.Max(p => p.x);
                        float minZ = points.Min(p => p.z), maxZ = points.Max(p => p.z);
                        Assert.That(minX < 1.3f && maxX > -.7f && minZ < 1.3f && maxZ > -.7f, Is.False);
                    }
                    }
                }
            }
            finally { UnityEngine.Object.DestroyImmediate(parent); }
        }
    }
}
