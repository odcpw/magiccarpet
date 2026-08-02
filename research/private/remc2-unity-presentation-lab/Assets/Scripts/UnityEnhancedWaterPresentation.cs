using System;
using UnityEngine;

namespace MagicCarpet.PresentationLab
{
    public enum UnityEnhancedWaterReflectionMode
    {
        FallbackSky = 0,
        ScreenSpaceWhenSupported = 1
    }

    [Serializable]
    public sealed class UnityEnhancedWaterSnapshot
    {
        public ulong Generation;
        public ulong Tick;
        public bool Reset;
        public float Level;
        public Vector3 Center;
        public Vector2 Size = new Vector2(16f, 16f);
        public bool ReflectionEnabled = true;
        public byte[] ShorelineMask = Array.Empty<byte>();
        public int ShorelineWidth;
        public int ShorelineHeight;
        public float ShorelineCellSize = 1f;
    }

    /// <summary>
    /// Procedural water and shoreline presentation.  The simulation supplies
    /// the level and shore mask; this component owns only mesh/material state.
    /// </summary>
    [DisallowMultipleComponent]
    public sealed class UnityEnhancedWaterPresentation : MonoBehaviour
    {
        private MeshFilter waterFilter;
        private MeshRenderer waterRenderer;
        private MeshFilter shorelineFilter;
        private MeshRenderer shorelineRenderer;
        private Mesh waterMesh;
        private Mesh shorelineMesh;
        private Material waterMaterial;
        private Material shorelineMaterial;
        private ulong generation;
        private ulong lastTick;
        private bool hasSnapshot;

        public bool HasSnapshot { get { return hasSnapshot; } }
        public ulong Generation { get { return generation; } }
        public ulong LastTick { get { return lastTick; } }
        public bool ReflectionEnabled { get; private set; }
        public UnityEnhancedWaterReflectionMode ReflectionMode { get; private set; }
        public int ShorelineCellCount { get; private set; }
        public int ShorelineVertexCount { get { return shorelineMesh == null ? 0 : shorelineMesh.vertexCount; } }

        public bool ApplySnapshot(UnityEnhancedWaterSnapshot snapshot)
        {
            if (snapshot == null || snapshot.Size.x <= 0f || snapshot.Size.y <= 0f ||
                !IsFinite(snapshot.Center) ||
                !IsFinite(snapshot.Size.x) || !IsFinite(snapshot.Size.y) ||
                !IsFinite(snapshot.Level) ||
                !IsFinite(snapshot.ShorelineCellSize) || snapshot.ShorelineCellSize <= 0f ||
                snapshot.ShorelineWidth < 0 || snapshot.ShorelineHeight < 0)
            {
                return false;
            }

            long expectedShorelineCells = (long)snapshot.ShorelineWidth * snapshot.ShorelineHeight;
            if (expectedShorelineCells > int.MaxValue || snapshot.ShorelineMask == null ||
                snapshot.ShorelineMask.Length != expectedShorelineCells)
            {
                return false;
            }

            if (hasSnapshot && snapshot.Generation == generation && snapshot.Tick < lastTick)
            {
                return false;
            }

            EnsureResources();
            BuildWater(snapshot);
            BuildShoreline(snapshot);
            ReflectionEnabled = snapshot.ReflectionEnabled;
            ReflectionMode = ReflectionEnabled
                ? UnityEnhancedWaterReflectionMode.ScreenSpaceWhenSupported
                : UnityEnhancedWaterReflectionMode.FallbackSky;
            generation = snapshot.Generation;
            lastTick = snapshot.Tick;
            hasSnapshot = true;
            return true;
        }

        public void ResetPresentation(ulong nextGeneration)
        {
            generation = nextGeneration;
            lastTick = 0;
            hasSnapshot = false;
            ReflectionEnabled = false;
            ShorelineCellCount = 0;
            if (waterMesh != null) waterMesh.Clear();
            if (shorelineMesh != null) shorelineMesh.Clear();
        }

        private void EnsureResources()
        {
            if (waterFilter == null)
            {
                GameObject water = new GameObject("EnhancedWaterSurface");
                water.transform.SetParent(transform, false);
                water.hideFlags = HideFlags.DontSave;
                waterFilter = water.AddComponent<MeshFilter>();
                waterRenderer = water.AddComponent<MeshRenderer>();
                waterMesh = new Mesh { name = "EnhancedWaterMesh", hideFlags = HideFlags.DontSave };
                waterFilter.sharedMesh = waterMesh;
                waterMaterial = CreateMaterial("EnhancedWaterMaterial", new Color(0.035f, 0.18f, 0.31f, 0.72f));
                waterRenderer.sharedMaterial = waterMaterial;
            }

            if (shorelineFilter == null)
            {
                GameObject shoreline = new GameObject("EnhancedShoreline");
                shoreline.transform.SetParent(transform, false);
                shoreline.hideFlags = HideFlags.DontSave;
                shorelineFilter = shoreline.AddComponent<MeshFilter>();
                shorelineRenderer = shoreline.AddComponent<MeshRenderer>();
                shorelineMesh = new Mesh { name = "EnhancedShorelineMesh", hideFlags = HideFlags.DontSave };
                shorelineFilter.sharedMesh = shorelineMesh;
                shorelineMaterial = CreateMaterial("EnhancedShorelineMaterial", new Color(0.055f, 0.16f, 0.23f, 1f));
                shorelineRenderer.sharedMaterial = shorelineMaterial;
            }
        }

        private void BuildWater(UnityEnhancedWaterSnapshot snapshot)
        {
            float halfX = snapshot.Size.x * 0.5f;
            float halfZ = snapshot.Size.y * 0.5f;
            waterMesh.Clear();
            waterMesh.vertices = new[]
            {
                snapshot.Center + new Vector3(-halfX, snapshot.Level, -halfZ),
                snapshot.Center + new Vector3(halfX, snapshot.Level, -halfZ),
                snapshot.Center + new Vector3(halfX, snapshot.Level, halfZ),
                snapshot.Center + new Vector3(-halfX, snapshot.Level, halfZ)
            };
            waterMesh.uv = new[]
            {
                new Vector2(0f, 0f), new Vector2(1f, 0f), new Vector2(1f, 1f), new Vector2(0f, 1f)
            };
            waterMesh.triangles = new[] { 0, 2, 1, 0, 3, 2 };
            waterMesh.RecalculateNormals();
        }

        private void BuildShoreline(UnityEnhancedWaterSnapshot snapshot)
        {
            shorelineMesh.Clear();
            ShorelineCellCount = 0;
            if (snapshot.ShorelineMask == null || snapshot.ShorelineWidth <= 0 || snapshot.ShorelineHeight <= 0)
            {
                return;
            }

            int expected = snapshot.ShorelineWidth * snapshot.ShorelineHeight;
            int count = Mathf.Min(expected, snapshot.ShorelineMask.Length);
            Vector3[] vertices = new Vector3[count * 4];
            int[] triangles = new int[count * 6];
            int vertexIndex = 0;
            int triangleIndex = 0;
            for (int index = 0; index < count; index++)
            {
                if (snapshot.ShorelineMask[index] == 0)
                {
                    continue;
                }

                int x = index % snapshot.ShorelineWidth;
                int z = index / snapshot.ShorelineWidth;
                float cell = Mathf.Max(0.01f, snapshot.ShorelineCellSize);
                Vector3 shorelineOrigin = snapshot.Center -
                    new Vector3(snapshot.Size.x * 0.5f, 0f, snapshot.Size.y * 0.5f);
                Vector3 center = shorelineOrigin +
                    new Vector3((x + 0.5f) * cell, snapshot.Level + 0.012f, (z + 0.5f) * cell);
                float half = cell * 0.5f;
                int baseVertex = vertexIndex;
                vertices[vertexIndex++] = center + new Vector3(-half, 0f, -half);
                vertices[vertexIndex++] = center + new Vector3(half, 0f, -half);
                vertices[vertexIndex++] = center + new Vector3(half, 0f, half);
                vertices[vertexIndex++] = center + new Vector3(-half, 0f, half);
                triangles[triangleIndex++] = baseVertex;
                triangles[triangleIndex++] = baseVertex + 2;
                triangles[triangleIndex++] = baseVertex + 1;
                triangles[triangleIndex++] = baseVertex;
                triangles[triangleIndex++] = baseVertex + 3;
                triangles[triangleIndex++] = baseVertex + 2;
                ShorelineCellCount++;
            }

            Array.Resize(ref vertices, vertexIndex);
            Array.Resize(ref triangles, triangleIndex);
            shorelineMesh.vertices = vertices;
            shorelineMesh.triangles = triangles;
            shorelineMesh.RecalculateNormals();
        }

        private static Material CreateMaterial(string name, Color color)
        {
            Shader customWaterShader = Shader.Find("MagicCarpet/Enhanced/Water");
            Shader shader = customWaterShader ??
                            Shader.Find("MagicCarpet/Enhanced/VertexColorLit") ??
                            Shader.Find("Universal Render Pipeline/Lit") ??
                            Shader.Find("Standard");
            if (shader == null)
            {
                return null;
            }

            Material material = new Material(shader)
            {
                name = name,
                hideFlags = HideFlags.DontSave,
                color = color
            };
            material.enableInstancing = true;
            if (shader == customWaterShader)
            {
                material.SetColor("_BaseColor", new Color(0.008f, 0.04f, 0.12f, 1f));
                material.SetColor("_ReflectionColor", new Color(0.025f, 0.15f, 0.42f, 1f));
                material.SetColor("_FoamColor", new Color(0.08f, 0.25f, 0.60f, 1f));
                material.SetFloat("_WaveScale", 0.055f);
                material.SetFloat("_WaveSpeed", 0.22f);
                material.SetFloat("_WaveStrength", 0.10f);
                material.SetFloat("_FresnelStrength", 0.55f);
                material.SetFloat("_SpecularStrength", 0.8f);
                material.SetFloat("_Alpha", 0.84f);
            }
            else
            {
                material.SetFloat("_AmbientBoost", 0.55f);
                material.SetFloat("_SpecularStrength", 0.20f);
                material.SetColor("_RimColor", new Color(0.08f, 0.20f, 0.48f, 1f));
                material.SetFloat("_RimStrength", 0.06f);
                material.SetColor("_EmissionColor", color * 0.2f);
                material.SetFloat("_EmissionStrength", 0.08f);
            }
            return material;
        }

        private static bool IsFinite(float value)
        {
            return !float.IsNaN(value) && !float.IsInfinity(value);
        }

        private static bool IsFinite(Vector3 value)
        {
            return IsFinite(value.x) && IsFinite(value.y) && IsFinite(value.z);
        }

        private void OnDestroy()
        {
            if (waterMesh != null) DestroyImmediate(waterMesh);
            if (shorelineMesh != null) DestroyImmediate(shorelineMesh);
            if (waterMaterial != null) DestroyImmediate(waterMaterial);
            if (shorelineMaterial != null) DestroyImmediate(shorelineMaterial);
        }
    }
}
