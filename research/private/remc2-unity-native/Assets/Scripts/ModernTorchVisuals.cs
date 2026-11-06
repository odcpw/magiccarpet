using System;
using System.Collections.Generic;
using UnityEngine;
using UnityEngine.Rendering;

namespace MagicCarpet.Native
{
    // Authored modern exterior lamps, selected from actual house observations by
    // the lighting owner. These are cosmetic fixtures, not source game entities.
    internal sealed class ModernTorchVisuals : IDisposable
    {
        internal const int Capacity = 8;
        private static readonly int SeedId = Shader.PropertyToID("_FlameSeed");
        private readonly GameObject root;
        private readonly GameObject[] slots = new GameObject[Capacity];
        private readonly MeshRenderer[] flames = new MeshRenderer[Capacity];
        private readonly MaterialPropertyBlock properties = new MaterialPropertyBlock();
        private readonly Mesh poleMesh, flameMesh;
        private readonly Material poleMaterial, flameMaterial;
        private int used;
        private bool disposed;

        internal ModernTorchVisuals(Transform parent)
        {
            if (parent == null) throw new ArgumentNullException(nameof(parent));
            var poleShader = Resources.Load<Shader>("ModernEntity");
            var flameShader = Resources.Load<Shader>("ModernTorchFlame");
            if (poleShader == null || flameShader == null)
                throw new InvalidOperationException("Missing modern torch shaders");
            try
            {
                root = new GameObject("Modern exterior torch fixtures");
                root.transform.SetParent(parent, false);
                poleMaterial = new Material(poleShader) { name = "Dark torch iron and timber" };
                poleMaterial.SetColor("_BaseColor", new Color(.065f, .040f, .023f, 1));
                poleMaterial.SetFloat("_EmissionStrength", 0);
                flameMaterial = new Material(flameShader) { name = "Wind bent torch flame" };
                poleMesh = BuildPole(); flameMesh = BuildFlame();
                for (var i = 0; i < Capacity; i++)
                {
                    var slot = new GameObject("Exterior torch " + i);
                    slot.transform.SetParent(root.transform, false);
                    CreateRenderer(slot.transform, "Post and brazier", poleMesh, poleMaterial);
                    flames[i] = CreateRenderer(slot.transform, "Flame and embers", flameMesh, flameMaterial);
                    slot.SetActive(false); slots[i] = slot;
                }
            }
            catch { Dispose(); throw; }
        }

        internal void BeginFrame()
        {
            ThrowIfDisposed();
            used = 0;
        }

        internal void Add(Vector3 flameCenter, float seed)
        {
            ThrowIfDisposed();
            if (used == Capacity) return;
            if (!Finite(flameCenter.x) || !Finite(flameCenter.y) || !Finite(flameCenter.z) || !Finite(seed)) return;
            var slot = slots[used];
            slot.transform.position = flameCenter;
            properties.SetFloat(SeedId, Mathf.Repeat(seed, 1024f));
            flames[used].SetPropertyBlock(properties);
            if (!slot.activeSelf) slot.SetActive(true);
            used++;
        }

        internal void EndFrame()
        {
            ThrowIfDisposed();
            for (var i = used; i < Capacity; i++)
                if (slots[i].activeSelf) slots[i].SetActive(false);
        }

        private static MeshRenderer CreateRenderer(Transform parent, string name, Mesh mesh, Material material)
        {
            var node = new GameObject(name); node.transform.SetParent(parent, false);
            node.AddComponent<MeshFilter>().sharedMesh = mesh;
            var renderer = node.AddComponent<MeshRenderer>();
            renderer.sharedMaterial = material;
            renderer.shadowCastingMode = ShadowCastingMode.Off;
            renderer.receiveShadows = false;
            renderer.lightProbeUsage = LightProbeUsage.Off;
            renderer.reflectionProbeUsage = ReflectionProbeUsage.Off;
            renderer.motionVectorGenerationMode = MotionVectorGenerationMode.ForceNoMotion;
            return renderer;
        }

        private static Mesh BuildPole()
        {
            var vertices = new List<Vector3>(); var indices = new List<int>();
            Frustum(vertices, indices, -1.6f, -.20f, .035f, .045f);
            Frustum(vertices, indices, -.20f, -.045f, .060f, .125f);
            Frustum(vertices, indices, -.06f, -.012f, .135f, .135f);
            var mesh = new Mesh { name = "Shared eight-sided torch post and brazier" };
            mesh.SetVertices(vertices); mesh.SetTriangles(indices, 0); mesh.RecalculateNormals(); mesh.RecalculateBounds();
            mesh.UploadMeshData(true);
            return mesh;
        }

        private static void Frustum(List<Vector3> vertices, List<int> indices, float bottom, float top, float lower, float upper)
        {
            const int sides = 8;
            for (var i = 0; i < sides; i++)
            {
                var a = i * (2f * Mathf.PI / sides); var b = (i + 1) * (2f * Mathf.PI / sides);
                var first = vertices.Count;
                vertices.Add(new Vector3(Mathf.Cos(a) * lower, bottom, Mathf.Sin(a) * lower));
                vertices.Add(new Vector3(Mathf.Cos(a) * upper, top, Mathf.Sin(a) * upper));
                vertices.Add(new Vector3(Mathf.Cos(b) * upper, top, Mathf.Sin(b) * upper));
                vertices.Add(new Vector3(Mathf.Cos(b) * lower, bottom, Mathf.Sin(b) * lower));
                indices.Add(first); indices.Add(first + 1); indices.Add(first + 2);
                indices.Add(first); indices.Add(first + 2); indices.Add(first + 3);
                first = vertices.Count;
                vertices.Add(new Vector3(0, top, 0));
                vertices.Add(new Vector3(Mathf.Cos(b) * upper, top, Mathf.Sin(b) * upper));
                vertices.Add(new Vector3(Mathf.Cos(a) * upper, top, Mathf.Sin(a) * upper));
                indices.Add(first); indices.Add(first + 1); indices.Add(first + 2);
            }
        }

        private static Mesh BuildFlame()
        {
            var vertices = new List<Vector3>(); var normals = new List<Vector3>();
            var uv = new List<Vector2>(); var colors = new List<Color>(); var indices = new List<int>();
            const int segments = 4;
            for (var plane = 0; plane < 3; plane++)
            {
                var angle = plane * Mathf.PI / 3;
                var across = new Vector3(Mathf.Cos(angle), 0, Mathf.Sin(angle));
                var normal = new Vector3(-across.z, 0, across.x);
                var first = vertices.Count;
                for (var row = 0; row <= segments; row++)
                for (var side = 0; side < 2; side++)
                {
                    var y = row / (float)segments;
                    // Start inside the brazier, so the flame emerges from the fuel
                    // instead of hovering above the opaque cap at close range.
                    vertices.Add(across * ((side * 2 - 1) * .22f) + Vector3.up * (y * .75f - .025f));
                    normals.Add(normal); uv.Add(new Vector2(side, y)); colors.Add(Color.black);
                }
                for (var row = 0; row < segments; row++)
                {
                    var p = first + row * 2;
                    indices.Add(p); indices.Add(p + 2); indices.Add(p + 1);
                    indices.Add(p + 1); indices.Add(p + 2); indices.Add(p + 3);
                }
            }
            // Small hot fuel disc shares the flame draw; vertex colour selects the
            // ember branch. It sits just above the opaque brazier cap.
            for (var i = 0; i < 8; i++)
            {
                var first = vertices.Count;
                vertices.Add(new Vector3(0, -.01f, 0)); normals.Add(Vector3.up); uv.Add(new Vector2(.5f, .5f)); colors.Add(Color.red);
                for (var j = 1; j >= 0; j--)
                {
                    var angle = (i + j) * Mathf.PI / 4;
                    var disc = new Vector2(Mathf.Cos(angle), Mathf.Sin(angle));
                    vertices.Add(new Vector3(disc.x * .115f, -.01f, disc.y * .115f));
                    normals.Add(Vector3.up); uv.Add(disc * .5f + Vector2.one * .5f); colors.Add(Color.red);
                }
                indices.Add(first); indices.Add(first + 1); indices.Add(first + 2);
            }
            var mesh = new Mesh { name = "Shared torch crossed flame sheets and ember disc" };
            mesh.SetVertices(vertices); mesh.SetNormals(normals); mesh.SetUVs(0, uv); mesh.SetColors(colors); mesh.SetTriangles(indices, 0);
            // Includes worst-case shader wind bend, not just the rest mesh.
            mesh.bounds = new Bounds(new Vector3(0, .35f, 0), new Vector3(2.2f, 1.2f, 2.2f));
            mesh.UploadMeshData(true);
            return mesh;
        }

        private static bool Finite(float value) => !float.IsNaN(value) && !float.IsInfinity(value);
        private void ThrowIfDisposed()
        { if (disposed) throw new ObjectDisposedException(nameof(ModernTorchVisuals)); }

        public void Dispose()
        {
            if (disposed) return;
            disposed = true;
            Destroy(root); Destroy(poleMesh); Destroy(flameMesh); Destroy(poleMaterial); Destroy(flameMaterial);
        }

        private static void Destroy(UnityEngine.Object value)
        {
            if (value == null) return;
            if (Application.isPlaying) UnityEngine.Object.Destroy(value);
            else UnityEngine.Object.DestroyImmediate(value);
        }
    }
}
