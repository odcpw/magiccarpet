using System;
using NUnit.Framework;
using UnityEngine;

namespace MagicCarpet.Native.Tests
{
    public sealed class ModernBuildingMeshesEditModeTests
    {
        private static ModernBuildingMeshes.Profile Sample()
        {
            return new ModernBuildingMeshes.Profile
            {
                build_index = 999, columns = 5, rows = 5,
                heights = new float[] {
                    0, 0, 0, 0, 0,
                    0, 8, 8, 8, 0,
                    0, 8, 10, 8, 0,
                    0, 8, 8, 8, 0,
                    0, 0, 0, 0, 0 }
            };
        }

        [Test]
        public void EveryLodKeepsEverySampleAndTheSameSourceFacets()
        {
            var profile = Sample();
            var meshes = new Mesh[3];
            try
            {
                int bodyVertices = (profile.columns - 1) * (profile.rows - 1) * 6;
                for (int lod = 0; lod < 3; lod++) meshes[lod] = ModernBuildingMeshes.Create(profile, lod);
                Assert.That(ModernBuildingMeshes.Footprint(profile), Is.EqualTo(new Vector2(16, 16)));
                for (int z = 0; z < profile.rows; z++)
                for (int x = 0; x < profile.columns; x++)
                {
                    var sample = new Vector3(x * 4 - 8, profile.heights[x + z * profile.columns], z * 4 - 8);
                    Assert.That(Array.IndexOf(meshes[0].vertices, sample), Is.InRange(0, bodyVertices - 1));
                }
                var body = meshes[0].vertices;
                for (int lod = 0; lod < 3; lod++)
                {
                    var vertices = meshes[lod].vertices;
                    var normals = meshes[lod].normals;
                    var palette = meshes[lod].uv2;
                    var triangles = meshes[lod].triangles;
                    Assert.That(meshes[lod].subMeshCount, Is.EqualTo(1));
                    Assert.That(normals.Length, Is.EqualTo(vertices.Length));
                    Assert.That(palette.Length, Is.EqualTo(vertices.Length));
                    Assert.That(meshes[lod].uv.Length, Is.EqualTo(vertices.Length));
                    Assert.That(meshes[lod].colors.Length, Is.EqualTo(vertices.Length));
                    for (int i = 0; i < bodyVertices; i++) Assert.That(vertices[i], Is.EqualTo(body[i]));
                    Assert.That(meshes[lod].bounds.min.x, Is.GreaterThanOrEqualTo(-8.001f));
                    Assert.That(meshes[lod].bounds.max.x, Is.LessThanOrEqualTo(8.001f));
                    Assert.That(meshes[lod].bounds.min.z, Is.GreaterThanOrEqualTo(-8.001f));
                    Assert.That(meshes[lod].bounds.max.z, Is.LessThanOrEqualTo(8.001f));
                    Assert.That(meshes[lod].bounds.min.y, Is.GreaterThanOrEqualTo(-.001f));
                    Assert.That(meshes[lod].bounds.max.y, Is.LessThanOrEqualTo(10.001f));
                    for (int i = 0; i < triangles.Length; i += 3)
                    {
                        int a = triangles[i], b = triangles[i + 1], c = triangles[i + 2];
                        var normal = Vector3.Cross(vertices[b] - vertices[a], vertices[c] - vertices[a]);
                        Assert.That(normal.sqrMagnitude, Is.GreaterThan(1e-12f));
                        Assert.That(Vector3.Dot(normal, normals[a]), Is.GreaterThan(0f));
                        Assert.That(normals[a].sqrMagnitude, Is.EqualTo(1f).Within(.001f));
                        Assert.That(palette[a].x, Is.EqualTo(Mathf.Round(palette[a].x)).And.InRange(0f, 6f));
                        if (i < bodyVertices) Assert.That(normals[a].y, Is.GreaterThan(0f));
                    }
                }
                Assert.That(meshes[0].triangles.Length, Is.GreaterThan(meshes[1].triangles.Length));
                Assert.That(meshes[1].triangles.Length, Is.GreaterThan(meshes[2].triangles.Length));
            }
            finally { foreach (var mesh in meshes) if (mesh != null) UnityEngine.Object.DestroyImmediate(mesh); }
        }

        [Test]
        public void SourceParitySelectsTheOtherDiagonalForNonPlanarCells()
        {
            var profile = new ModernBuildingMeshes.Profile { columns = 2, rows = 2, heights = new[] { 0f, 1f, 2f, 8f } };
            var even = ModernBuildingMeshes.Create(profile, 2);
            profile.diagonal_parity = 1;
            var odd = ModernBuildingMeshes.Create(profile, 2);
            try
            {
                CollectionAssert.AreEqual(new[] { new Vector3(-2, 0, -2), new Vector3(-2, 2, 2), new Vector3(2, 8, 2) },
                    new[] { even.vertices[0], even.vertices[1], even.vertices[2] });
                CollectionAssert.AreEqual(new[] { new Vector3(-2, 0, -2), new Vector3(-2, 2, 2), new Vector3(2, 1, -2) },
                    new[] { odd.vertices[0], odd.vertices[1], odd.vertices[2] });
            }
            finally { UnityEngine.Object.DestroyImmediate(even); UnityEngine.Object.DestroyImmediate(odd); }
        }

        [Test]
        public void WindowAnchorsMeetVisibleGlassOnTheirHostFacetsAtEveryLod()
        {
            var profile = Sample();
            var centers = ModernBuildingMeshes.WindowCenters(profile);
            var normals = ModernBuildingMeshes.WindowNormals(profile);
            Assert.That(centers.Length, Is.InRange(1, 6));
            Assert.That(normals.Length, Is.EqualTo(centers.Length));
            for (int lod = 0; lod < 3; lod++)
            {
                var mesh = ModernBuildingMeshes.Create(profile, lod);
                try
                {
                    for (int i = 0; i < centers.Length; i++)
                    {
                        Assert.That(normals[i].sqrMagnitude, Is.EqualTo(1f).Within(.001f));
                        float distance;
                        int material = FirstMaterial(mesh, centers[i] + normals[i], -normals[i], out distance);
                        Assert.That(material, Is.EqualTo(5), "anchor " + i + " LOD " + lod);
                        Assert.That(distance, Is.EqualTo(1f).Within(.001f));
                    }
                }
                finally { UnityEngine.Object.DestroyImmediate(mesh); }
            }
        }

        [Test]
        public void InvalidProfilesAndLodsAreRejected()
        {
            Assert.Throws<ArgumentNullException>(() => ModernBuildingMeshes.Create(null, 0));
            var p = Sample(); p.columns = 1;
            Assert.Throws<ArgumentException>(() => ModernBuildingMeshes.Footprint(p));
            p = Sample(); p.heights = new float[1];
            Assert.Throws<ArgumentException>(() => ModernBuildingMeshes.WindowCenters(p));
            p = Sample(); p.heights[2] = float.NaN;
            Assert.Throws<ArgumentException>(() => ModernBuildingMeshes.Create(p, 0));
            p = Sample(); p.heights[2] = float.PositiveInfinity;
            Assert.Throws<ArgumentException>(() => ModernBuildingMeshes.Create(p, 0));
            p = Sample(); p.diagonal_parity = 2;
            Assert.Throws<ArgumentException>(() => ModernBuildingMeshes.Create(p, 0));
            p = Sample();
            Assert.Throws<ArgumentOutOfRangeException>(() => ModernBuildingMeshes.Create(p, 3));
        }

        private static int FirstMaterial(Mesh mesh, Vector3 origin, Vector3 direction, out float nearest)
        {
            var vertices = mesh.vertices; var indices = mesh.triangles; var palette = mesh.uv2;
            nearest = float.PositiveInfinity; int material = -1;
            for (int i = 0; i < indices.Length; i += 3)
            {
                var a = vertices[indices[i]]; var e1 = vertices[indices[i + 1]] - a; var e2 = vertices[indices[i + 2]] - a;
                var p = Vector3.Cross(direction, e2); float det = Vector3.Dot(e1, p);
                if (Mathf.Abs(det) < 1e-7f) continue;
                var t = origin - a; float u = Vector3.Dot(t, p) / det; var q = Vector3.Cross(t, e1);
                float v = Vector3.Dot(direction, q) / det; float distance = Vector3.Dot(e2, q) / det;
                if (u >= 0 && v >= 0 && u + v <= 1 && distance >= 0 && distance < nearest)
                { nearest = distance; material = (int)palette[indices[i]].x; }
            }
            return material;
        }
    }
}
