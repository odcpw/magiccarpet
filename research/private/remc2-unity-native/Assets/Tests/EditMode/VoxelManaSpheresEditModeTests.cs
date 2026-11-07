using System;
using System.Collections.Generic;
using MagicCarpet.Native.Voxels;
using NUnit.Framework;
using UnityEngine;

namespace MagicCarpet.Native.Tests
{
    public sealed class VoxelManaSpheresEditModeTests
    {
        private static NativeSim.ManaSphereSnapshot Sphere(ushort slot, ushort tmap, ushort height, ushort x = 0, ushort y = 0, short z = 0)
            => new NativeSim.ManaSphereSnapshot { active = 1, sphereId = slot, sourceEntityIndex = slot,
                baseTmap = tmap, logicalHeight = height, x = x, y = y, z = z };
        private static NativeSim.CampaignEntityPresentation Visible(ushort slot)
            => new NativeSim.CampaignEntityPresentation { entityIndex = slot };
        private static Dictionary<int, Color> Colors() => new() { [58] = Color.white, [67] = Color.red };

        [Test]
        public void SphereMeshIsClosedVoxelSurfaceWithOutwardWindingAndBoundedGeometry()
        {
            var mesh = VoxelManaSpheres.BuildSphereMesh();
            try
            {
                Assert.That(mesh.vertexCount, Is.GreaterThan(1000));
                Assert.That(mesh.vertexCount, Is.LessThan(10000));
                Assert.That(mesh.bounds.size.x, Is.EqualTo(1).Within(.001));
                Assert.That(mesh.bounds.size.y, Is.EqualTo(1).Within(.001));
                Assert.That(mesh.bounds.size.z, Is.EqualTo(1).Within(.001));
                var vertices = mesh.vertices; var normals = mesh.normals; var triangles = mesh.triangles;
                for (var i = 0; i < triangles.Length; i += 3)
                {
                    var a = triangles[i]; var b = triangles[i + 1]; var c = triangles[i + 2];
                    Assert.That(Vector3.Dot(Vector3.Cross(vertices[b] - vertices[a], vertices[c] - vertices[a]), normals[a]), Is.GreaterThan(0));
                }
            }
            finally { UnityEngine.Object.DestroyImmediate(mesh); }
        }

        [Test]
        public void IndexedSpriteColorUsesActualNontransparentPalettePixels()
        {
            var palette = new byte[768];
            palette[3] = 255; palette[4] = 20; palette[5] = 10;
            palette[6] = 30; palette[7] = 180; palette[8] = 20;
            var color = VoxelManaSpheres.ColorFromIndexedSprite(new byte[] { 0, 1, 1, 2 }, palette);
            Assert.That(color.r, Is.GreaterThan(color.g));
            Assert.That(color.g, Is.GreaterThan(color.b));
            Assert.That(color.a, Is.EqualTo(1));
            Assert.Throws<InvalidOperationException>(() => VoxelManaSpheres.ColorFromIndexedSprite(new byte[] { 0 }, palette));
        }

        [Test]
        public void NativeVisibilityClaimCollectionSlotReuseAndGenerationDrivePooledVisuals()
        {
            var parent = new GameObject("Mana lifecycle test");
            var cameraObject = new GameObject("Camera");
            try
            {
                var camera = cameraObject.AddComponent<Camera>();
                camera.transform.position = new Vector3(1023, 0, 1023);
                using var presenter = new VoxelManaSpheres(parent.transform, Colors());
                var neutral = Sphere(7, 58, 100, 64, 128, 192);
                presenter.Present(new[] { neutral }, new[] { Visible(7) }, 1);
                presenter.Render(camera);
                Assert.That(presenter.ActiveCount, Is.EqualTo(1));
                Assert.That(presenter.Replaces(7), Is.True);
                var node = parent.transform.GetChild(0).GetChild(0);
                Assert.That(node.position.x, Is.EqualTo(1025).Within(.001));
                Assert.That(node.position.z, Is.EqualTo(1026).Within(.001));
                Assert.That(node.position.y, Is.EqualTo(3 + 100 / 128f).Within(.001));
                Assert.That(node.localScale.x, Is.EqualTo(100 / 64f).Within(.001));
                Assert.That(node.GetComponent<Collider>(), Is.Null);
                var renderer = node.GetComponent<MeshRenderer>();
                var block = new MaterialPropertyBlock(); renderer.GetPropertyBlock(block);
                Assert.That(block.GetColor("_BaseColor"), Is.EqualTo(Color.white));
                var claimed = Sphere(7, 67, 200);
                presenter.Present(new[] { claimed }, new[] { Visible(7) }, 1);
                renderer.GetPropertyBlock(block);
                Assert.That(block.GetColor("_BaseColor"), Is.EqualTo(Color.red));
                Assert.That(node.localScale.x, Is.EqualTo(200 / 64f).Within(.001));
                presenter.Present(Array.Empty<NativeSim.ManaSphereSnapshot>(), new[] { Visible(7) }, 1);
                Assert.That(presenter.ActiveCount, Is.Zero);
                Assert.That(presenter.PoolCount, Is.EqualTo(1));
                Assert.That(node.gameObject.activeSelf, Is.False);
                presenter.Present(new[] { neutral }, Array.Empty<NativeSim.CampaignEntityPresentation>(), 1);
                Assert.That(presenter.Replaces(7), Is.False, "Native-hidden spheres must not be drawn or suppress other actors");
                presenter.Present(new[] { neutral }, new[] { Visible(7) }, 2);
                Assert.That(presenter.ActiveCount, Is.EqualTo(1));
                Assert.That(parent.transform.GetChild(0).GetChild(0), Is.EqualTo(node), "Reused slot must reuse only the visual, never stale state");
                renderer.GetPropertyBlock(block);
                Assert.That(block.GetColor("_BaseColor"), Is.EqualTo(Color.white));
                presenter.Present(new[] { neutral, Sphere(8, 67, 150) }, new[] { Visible(7), Visible(8) }, 2);
                Assert.That(presenter.ActiveCount, Is.EqualTo(2));
                presenter.Present(new[] { Sphere(8, 58, 50) }, new[] { Visible(8) }, 3);
                Assert.That(presenter.ActiveCount, Is.EqualTo(1));
                Assert.That(presenter.Replaces(7), Is.False, "A new world must not keep a prior world's sphere");
                Assert.That(presenter.Replaces(8), Is.True);
                presenter.Present(new[] { new NativeSim.ManaSphereSnapshot { sourceEntityIndex = 7, active = 0 } }, new[] { Visible(7) }, 2);
                Assert.That(presenter.ActiveCount, Is.Zero);
            }
            finally { UnityEngine.Object.DestroyImmediate(cameraObject); UnityEngine.Object.DestroyImmediate(parent); }
        }
    }
}
