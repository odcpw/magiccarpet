using System;
using MagicCarpet.Native.Voxels;
using NUnit.Framework;
using UnityEngine;
using UnityEngine.Rendering;

namespace MagicCarpet.Native.Tests
{
    public sealed class VoxelTerrainSeamEditModeTests
    {
        // Changes slope at the same two-unit lattice that bounds Modern detail triangles.
        private static float Height(float x, float z) => 1 + .13f * x + .17f * z + .7f * Mathf.Abs(x - 2) + .3f * Mathf.Abs(z + 2);

        [Test]
        public void SmallMarbleCannotEscapeBelowTheUnquantizedTerrainLip()
        {
            var parent=new GameObject("Physical terrain seam regression");
            var material=new Material(Shader.Find("Hidden/InternalErrorShader"));
            var marble=new GameObject("Marble below outer terrain height");var mode=Physics.simulationMode;
            try{using(var seam=new VoxelTerrainSeam(parent.transform,new Bounds(Vector3.zero,new Vector3(8,100,12)),(_,_)=>0,material)){
                Physics.simulationMode=SimulationMode.Script;
                marble.transform.position=new Vector3(3.8f,-.06f,.13f);
                marble.AddComponent<BoxCollider>().size=Vector3.one*.065f;
                var body=marble.AddComponent<Rigidbody>();body.useGravity=false;body.collisionDetectionMode=CollisionDetectionMode.Continuous;body.linearVelocity=Vector3.right*10;
                Physics.SyncTransforms();
                for(int i=0;i<8;i++)Physics.Simulate(1f/120);
                Assert.That(body.position.x,Is.LessThan(4),"The visible vertical lip must stop a fragment below the outside height; without it the body passes under the outer terrain");
                Assert.That(Physics.Raycast(new Vector3(0,10,0),Vector3.down,20,1<<8),Is.False,"The skirt must not install a floor across editable ground");
            }}finally{Physics.simulationMode=mode;UnityEngine.Object.DestroyImmediate(marble);UnityEngine.Object.DestroyImmediate(parent);UnityEngine.Object.DestroyImmediate(material);}
        }

        [Test]
        public void NonflatBoundaryIsContinuousVerticalAndNeverCoversInterior()
        {
            var parent = new GameObject("Voxel seam test");
            var shader = Shader.Find("Hidden/InternalErrorShader");
            Assert.That(shader, Is.Not.Null);
            var material = new Material(shader);
            var patch = new Bounds(Vector3.zero, new Vector3(8, 100, 12));
            VoxelTerrainSeam seam = null;
            try
            {
                seam = new VoxelTerrainSeam(parent.transform, patch, Height, material);
                var filters = parent.GetComponentsInChildren<MeshFilter>();
                Assert.That(filters.Length, Is.EqualTo(1));
                Assert.That(parent.GetComponentsInChildren<Collider>().Length, Is.EqualTo(1));
                Assert.That(parent.GetComponentInChildren<MeshCollider>().sharedMesh,Is.SameAs(filters[0].sharedMesh));
                var renderer = parent.GetComponentInChildren<MeshRenderer>();
                Assert.That(renderer.sharedMaterial, Is.SameAs(material));
                Assert.That(renderer.shadowCastingMode, Is.EqualTo(ShadowCastingMode.Off));
                var mesh = filters[0].sharedMesh;
                var vertices = mesh.vertices;
                var normals = mesh.normals;
                var indices = mesh.triangles;
                int segments = (int)((patch.size.x + patch.size.z) * 2 / .25f);
                Assert.That(vertices.Length, Is.EqualTo(segments * 8));
                Assert.That(indices.Length, Is.EqualTo(segments * 12));
                var lengths = new float[4];
                for (int i = 0; i < vertices.Length; i += 8)
                {
                    var a = vertices[i]; var lowerA = vertices[i + 1];
                    var b = vertices[i + 2]; var lowerB = vertices[i + 3];
                    Assert.That(a.y, Is.EqualTo(Height(a.x, a.z)).Within(.00001f));
                    Assert.That(b.y, Is.EqualTo(Height(b.x, b.z)).Within(.00001f));
                    Assert.That(lowerA, Is.EqualTo(a + Vector3.down * 2));
                    Assert.That(lowerB, Is.EqualTo(b + Vector3.down * 2));
                    var midpoint = (a + b) * .5f;
                    Assert.That(midpoint.y, Is.EqualTo(Height(midpoint.x, midpoint.z)).Within(.00001f));
                    Assert.That(Vector2.Distance(new Vector2(a.x, a.z), new Vector2(b.x, b.z)), Is.EqualTo(.25f).Within(.00001f));
                    if (a.z == patch.min.z && b.z == patch.min.z) lengths[0] += Mathf.Abs(b.x - a.x);
                    else if (a.x == patch.max.x && b.x == patch.max.x) lengths[1] += Mathf.Abs(b.z - a.z);
                    else if (a.z == patch.max.z && b.z == patch.max.z) lengths[2] += Mathf.Abs(b.x - a.x);
                    else if (a.x == patch.min.x && b.x == patch.min.x) lengths[3] += Mathf.Abs(b.z - a.z);
                    else Assert.Fail("A seam segment leaves the patch perimeter.");
                    for (int j = 0; j < 4; j++) Assert.That(vertices[i + j + 4], Is.EqualTo(vertices[i + j]));
                    Assert.That(Vector3.Dot(normals[i], normals[i + 4]), Is.EqualTo(-1).Within(.00001f));
                }
                Assert.That(lengths[0], Is.EqualTo(patch.size.x).Within(.00001f));
                Assert.That(lengths[1], Is.EqualTo(patch.size.z).Within(.00001f));
                Assert.That(lengths[2], Is.EqualTo(patch.size.x).Within(.00001f));
                Assert.That(lengths[3], Is.EqualTo(patch.size.z).Within(.00001f));
                for (int i = 0; i < indices.Length; i += 3)
                {
                    var a = vertices[indices[i]]; var b = vertices[indices[i + 1]]; var c = vertices[indices[i + 2]];
                    Assert.That(a.x == b.x && b.x == c.x || a.z == b.z && b.z == c.z, Is.True,
                        "Every triangle must be vertical on one boundary plane.");
                    Assert.That(Vector3.Dot(Vector3.Cross(b - a, c - a), normals[indices[i]]), Is.GreaterThan(0));
                }
                seam.Dispose(); seam.Dispose(); seam = null;
                Assert.That(parent.GetComponentsInChildren<MeshFilter>(), Is.Empty);
                Assert.That(material, Is.Not.Null, "The caller retains its terrain material.");
            }
            finally
            {
                seam?.Dispose();
                UnityEngine.Object.DestroyImmediate(parent);
                UnityEngine.Object.DestroyImmediate(material);
            }
        }

        [Test]
        public void RejectsBoundsThatCannotFollowExactTerrainLattice()
        {
            var parent = new GameObject("Voxel seam alignment test");
            var material = new Material(Shader.Find("Hidden/InternalErrorShader"));
            try
            {
                var patch = new Bounds(new Vector3(.5f, 0, 0), new Vector3(8, 1, 8));
                Assert.Throws<ArgumentException>(() => new VoxelTerrainSeam(parent.transform, patch, Height, material));
                Assert.That(parent.GetComponentsInChildren<MeshFilter>(), Is.Empty);
            }
            finally { UnityEngine.Object.DestroyImmediate(parent); UnityEngine.Object.DestroyImmediate(material); }
        }
    }
}
