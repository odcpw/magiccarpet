using System;
using UnityEngine;
using UnityEngine.Rendering;

namespace MagicCarpet.PresentationLab
{
    public enum UnityEnhancedTerrainWrapMode
    {
        Clamp = 0,
        Repeat = 1
    }

    public enum UnityEnhancedTerrainDiagnosticOverlay
    {
        None = 0,
        Height = 1,
        TerrainType = 2,
        Shade = 3,
        Angle = 4,
        DirtyRegion = 5
    }

    [Serializable]
    public struct UnityEnhancedTerrainDirtyRegion
    {
        public int X;
        public int Y;
        public int Width;
        public int Height;

        public UnityEnhancedTerrainDirtyRegion(int x, int y, int width, int height)
        {
            X = x;
            Y = y;
            Width = width;
            Height = height;
        }

        public bool IsValid(int width, int height)
        {
            return Width > 0 && Height > 0 && X >= 0 && Y >= 0 &&
                   X <= width - Width && Y <= height - Height;
        }
    }

    /// <summary>
    /// Presentation input for the Enhanced terrain.  Height, type, shade, and
    /// angle remain Rust-owned planes; Unity only turns them into a lit mesh.
    /// </summary>
    [Serializable]
    public sealed class UnityEnhancedTerrainSnapshot
    {
        public ulong Generation;
        public ulong Tick;
        public bool Reset;
        public int Width;
        public int Height;
        public float CellSize = 1f;
        public Vector3 Origin;
        public float[] Heights = Array.Empty<float>();
        public byte[] TerrainTypes = Array.Empty<byte>();
        public byte[] Shade = Array.Empty<byte>();
        public byte[] Angles = Array.Empty<byte>();
        public UnityEnhancedTerrainDirtyRegion[] DirtyRegions = Array.Empty<UnityEnhancedTerrainDirtyRegion>();
        public UnityEnhancedTerrainWrapMode WrapMode = UnityEnhancedTerrainWrapMode.Clamp;
    }

    /// <summary>
    /// Generates a reusable URP-lit terrain mesh from authoritative planes.
    /// No authored normal/detail texture is required: normals are derived from
    /// neighbouring heights and the material receives a stable vertex palette.
    /// </summary>
    [DisallowMultipleComponent]
    public sealed class UnityEnhancedTerrainPresentation : MonoBehaviour
    {
        private MeshFilter meshFilter;
        private MeshRenderer meshRenderer;
        private Mesh terrainMesh;
        private Material terrainMaterial;
        private Vector3[] vertices = Array.Empty<Vector3>();
        private Color[] colors = Array.Empty<Color>();
        private int[] triangles = Array.Empty<int>();
        private int width;
        private int height;
        private float cellSize;
        private Vector3 origin;
        private UnityEnhancedTerrainWrapMode wrapMode;
        private int renderSubdivision = 1;
        private int renderWidth;
        private int renderHeight;
        private bool renderTopologyDirty;
        private ulong generation;
        private ulong lastTick;
        private bool hasSnapshot;

        public bool HasSnapshot { get { return hasSnapshot; } }
        public ulong Generation { get { return generation; } }
        public ulong LastTick { get { return lastTick; } }
        public int Width { get { return width; } }
        public int Height { get { return height; } }
        public int VertexCount { get { return vertices.Length; } }
        public int RenderSubdivision { get { return renderSubdivision; } }
        public int DirtyRegionApplications { get; private set; }
        public UnityEnhancedTerrainWrapMode WrapMode { get { return wrapMode; } }
        public Mesh GeneratedMesh { get { return terrainMesh; } }
        public UnityEnhancedTerrainDiagnosticOverlay DiagnosticOverlay { get; private set; }

        public void SetDiagnosticOverlay(UnityEnhancedTerrainDiagnosticOverlay overlay)
        {
            DiagnosticOverlay = overlay;
        }

        /// <summary>
        /// Selects the presentation mesh density without changing the owner
        /// terrain grid. Whole Hog samples the authoritative height plane at
        /// four times the grid density and bilinearly interpolates between samples;
        /// simulation, collision, and dirty-region identity stay owner-sized.
        /// </summary>
        public void SetRenderSubdivision(int subdivision)
        {
            subdivision = Mathf.Clamp(subdivision, 1, 4);
            if (renderSubdivision == subdivision)
            {
                return;
            }

            renderSubdivision = subdivision;
            renderTopologyDirty = true;
        }

        public static int WrapIndex(int value, int extent, UnityEnhancedTerrainWrapMode mode)
        {
            if (extent <= 0)
            {
                return 0;
            }

            if (mode == UnityEnhancedTerrainWrapMode.Clamp)
            {
                return Mathf.Clamp(value, 0, extent - 1);
            }

            int result = value % extent;
            return result < 0 ? result + extent : result;
        }

        public bool ApplySnapshot(UnityEnhancedTerrainSnapshot snapshot)
        {
            if (snapshot == null || snapshot.Width < 2 || snapshot.Height < 2 ||
                snapshot.CellSize <= 0f || float.IsNaN(snapshot.CellSize) || float.IsInfinity(snapshot.CellSize) ||
                !IsFinite(snapshot.Origin) ||
                snapshot.Heights == null || snapshot.TerrainTypes == null || snapshot.Shade == null || snapshot.Angles == null)
            {
                return false;
            }

            long expected = (long)snapshot.Width * snapshot.Height;
            if (expected > int.MaxValue ||
                snapshot.Heights.Length != expected || snapshot.TerrainTypes.Length != expected ||
                snapshot.Shade.Length != expected || snapshot.Angles.Length != expected ||
                (snapshot.WrapMode != UnityEnhancedTerrainWrapMode.Clamp &&
                 snapshot.WrapMode != UnityEnhancedTerrainWrapMode.Repeat))
            {
                return false;
            }

            if (hasSnapshot && snapshot.Generation == generation && snapshot.Tick < lastTick)
            {
                return false;
            }

            bool topologyChanged = !hasSnapshot || snapshot.Reset || snapshot.Generation != generation ||
                                    snapshot.Width != width || snapshot.Height != height ||
                                    !Mathf.Approximately(snapshot.CellSize, cellSize) ||
                                    renderTopologyDirty;
            bool originChanged = !hasSnapshot || !Approximately(snapshot.Origin, origin);
            if (topologyChanged)
            {
                width = snapshot.Width;
                height = snapshot.Height;
                cellSize = snapshot.CellSize;
                origin = snapshot.Origin;
                wrapMode = snapshot.WrapMode;
                EnsureResources();
                BuildTopology();
                UpdateRegion(snapshot, new UnityEnhancedTerrainDirtyRegion(0, 0, width, height));
                terrainMesh.vertices = vertices;
                terrainMesh.colors = colors;
                terrainMesh.RecalculateNormals();
                renderTopologyDirty = false;
            }
            else
            {
                origin = snapshot.Origin;
                wrapMode = snapshot.WrapMode;
                if (originChanged || snapshot.DirtyRegions == null || snapshot.DirtyRegions.Length == 0)
                {
                    UpdateRegion(snapshot, new UnityEnhancedTerrainDirtyRegion(0, 0, width, height));
                }
                else
                {
                    for (int index = 0; index < snapshot.DirtyRegions.Length; index++)
                    {
                        UnityEnhancedTerrainDirtyRegion region = snapshot.DirtyRegions[index];
                        if (region.IsValid(width, height))
                        {
                            UpdateRegion(snapshot, region);
                        }
                    }
                }

                terrainMesh.vertices = vertices;
                terrainMesh.colors = colors;
                terrainMesh.RecalculateNormals();
            }

            UpdateBounds();

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
            DirtyRegionApplications = 0;
            if (terrainMesh != null)
            {
                terrainMesh.Clear();
            }
            renderTopologyDirty = false;
        }

        private void EnsureResources()
        {
            if (meshFilter == null)
            {
                meshFilter = GetComponent<MeshFilter>();
                if (meshFilter == null)
                {
                    meshFilter = gameObject.AddComponent<MeshFilter>();
                }
            }

            if (meshRenderer == null)
            {
                meshRenderer = GetComponent<MeshRenderer>();
                if (meshRenderer == null)
                {
                    meshRenderer = gameObject.AddComponent<MeshRenderer>();
                }
            }

            if (terrainMesh == null)
            {
                terrainMesh = new Mesh { name = "EnhancedTerrainMesh", hideFlags = HideFlags.DontSave };
                terrainMesh.MarkDynamic();
                meshFilter.sharedMesh = terrainMesh;
            }

            if (terrainMaterial == null)
            {
                Shader customTerrainShader = Shader.Find("MagicCarpet/Enhanced/Terrain");
                Shader shader = customTerrainShader ??
                                Shader.Find("MagicCarpet/Enhanced/VertexColorLit") ??
                                Shader.Find("Universal Render Pipeline/Lit") ??
                                Shader.Find("Standard");
                if (shader != null)
                {
                    terrainMaterial = new Material(shader)
                    {
                        name = "EnhancedTerrainMaterial",
                        hideFlags = HideFlags.DontSave,
                        color = Color.white
                    };
                    terrainMaterial.enableInstancing = true;
                    if (shader == customTerrainShader)
                    {
                        terrainMaterial.SetFloat("_AmbientBoost", 0.74f);
                        terrainMaterial.SetFloat("_DetailScale", 0.14f);
                        terrainMaterial.SetFloat("_DetailStrength", 0.35f);
                        terrainMaterial.SetFloat("_MicroDetailScale", 0.62f);
                        terrainMaterial.SetFloat("_MicroDetailStrength", 0.28f);
                        terrainMaterial.SetFloat("_MicroReliefStrength", 0.20f);
                        terrainMaterial.SetFloat("_MicroDetailNearStart", 8f);
                        terrainMaterial.SetFloat("_MicroDetailNearEnd", 128f);
                        terrainMaterial.SetColor("_SlopeTint", new Color(0.72f, 0.84f, 1.08f, 1f));
                        terrainMaterial.SetFloat("_SlopeStrength", 0.50f);
                        terrainMaterial.SetFloat("_SpecularStrength", 0.22f);
                        terrainMaterial.SetFloat("_NormalStrength", 0.48f);
                        terrainMaterial.SetFloat("_RidgeStrength", 0.64f);
                        terrainMaterial.SetColor("_RockTint", new Color(0.72f, 0.80f, 0.88f, 1f));
                        terrainMaterial.SetColor("_SoilTint", new Color(0.78f, 0.62f, 0.42f, 1f));
                    }
                    else
                    {
                        terrainMaterial.SetFloat("_AmbientBoost", 0.65f);
                        terrainMaterial.SetFloat("_DetailScale", 0.075f);
                        terrainMaterial.SetFloat("_DetailStrength", 0.42f);
                        terrainMaterial.SetFloat("_SpecularStrength", 0.08f);
                        terrainMaterial.SetColor("_RimColor", new Color(0.08f, 0.16f, 0.52f, 1f));
                        terrainMaterial.SetFloat("_RimStrength", 0.18f);
                        terrainMaterial.SetColor("_EmissionColor", new Color(0.02f, 0.04f, 0.12f, 1f));
                        terrainMaterial.SetFloat("_EmissionStrength", 0.25f);
                    }
                    meshRenderer.sharedMaterial = terrainMaterial;
                }
            }
        }

        private void BuildTopology()
        {
            renderWidth = (width - 1) * renderSubdivision + 1;
            renderHeight = (height - 1) * renderSubdivision + 1;
            int vertexCount = renderWidth * renderHeight;
            vertices = new Vector3[vertexCount];
            colors = new Color[vertexCount];
            triangles = new int[(renderWidth - 1) * (renderHeight - 1) * 6];
            int triangleIndex = 0;
            for (int y = 0; y < renderHeight - 1; y++)
            {
                for (int x = 0; x < renderWidth - 1; x++)
                {
                    int topLeft = y * renderWidth + x;
                    int topRight = topLeft + 1;
                    int bottomLeft = topLeft + renderWidth;
                    int bottomRight = bottomLeft + 1;
                    triangles[triangleIndex++] = topLeft;
                    triangles[triangleIndex++] = bottomLeft;
                    triangles[triangleIndex++] = topRight;
                    triangles[triangleIndex++] = topRight;
                    triangles[triangleIndex++] = bottomLeft;
                    triangles[triangleIndex++] = bottomRight;
                }
            }

            terrainMesh.Clear();
            terrainMesh.indexFormat = vertexCount > 65535
                ? IndexFormat.UInt32
                : IndexFormat.UInt16;
            terrainMesh.vertices = vertices;
            terrainMesh.colors = colors;
            terrainMesh.triangles = triangles;
            UpdateBounds();
        }

        private void UpdateBounds()
        {
            terrainMesh.bounds = new Bounds(
                origin + new Vector3((width - 1) * cellSize * 0.5f, 0f, (height - 1) * cellSize * 0.5f),
                new Vector3(width * cellSize, 512f, height * cellSize));
        }

        private void UpdateRegion(UnityEnhancedTerrainSnapshot snapshot, UnityEnhancedTerrainDirtyRegion region)
        {
            DirtyRegionApplications++;
            int minX = Mathf.Clamp(region.X, 0, width - 1);
            int minY = Mathf.Clamp(region.Y, 0, height - 1);
            int maxX = Mathf.Clamp(region.X + region.Width, minX + 1, width);
            int maxY = Mathf.Clamp(region.Y + region.Height, minY + 1, height);
            int minRenderX = Mathf.Max(0, minX * renderSubdivision - 1);
            int minRenderY = Mathf.Max(0, minY * renderSubdivision - 1);
            int maxRenderX = Mathf.Min(renderWidth - 1, maxX * renderSubdivision + 1);
            int maxRenderY = Mathf.Min(renderHeight - 1, maxY * renderSubdivision + 1);
            float renderCellSize = cellSize / renderSubdivision;
            for (int y = minRenderY; y <= maxRenderY; y++)
            {
                for (int x = minRenderX; x <= maxRenderX; x++)
                {
                    int sampleX = x / renderSubdivision;
                    int sampleY = y / renderSubdivision;
                    float fractionX = (x % renderSubdivision) / (float)renderSubdivision;
                    float fractionY = (y % renderSubdivision) / (float)renderSubdivision;
                    int x0 = WrapIndex(sampleX, width, wrapMode);
                    int x1 = WrapIndex(sampleX + 1, width, wrapMode);
                    int y0 = WrapIndex(sampleY, height, wrapMode);
                    int y1 = WrapIndex(sampleY + 1, height, wrapMode);
                    float h00 = snapshot.Heights[y0 * width + x0];
                    float h10 = snapshot.Heights[y0 * width + x1];
                    float h01 = snapshot.Heights[y1 * width + x0];
                    float h11 = snapshot.Heights[y1 * width + x1];
                    float lower = Mathf.Lerp(h00, h10, fractionX);
                    float upper = Mathf.Lerp(h01, h11, fractionX);
                    float heightValue = Mathf.Lerp(lower, upper, fractionY);
                    int renderIndex = y * renderWidth + x;
                    int colorIndex = y0 * width + x0;
                    vertices[renderIndex] = origin + new Vector3(
                        x * renderCellSize,
                        heightValue,
                        y * renderCellSize);
                    colors[renderIndex] = VertexColor(snapshot, colorIndex);
                }
            }
        }

        private static Color VertexColor(UnityEnhancedTerrainSnapshot snapshot, int index)
        {
            byte terrainType = snapshot.TerrainTypes[index];
            Color baseColor = NightPaletteForSelector(terrainType);

            // The owner shade/angle planes remain in the snapshot for
            // Classic HD and diagnostics. Enhanced derives smooth lighting
            // from mesh normals instead of exposing their indexed raster
            // quantization as a checkerboard over the procedural mesh.
            float shadeFactor = Mathf.Lerp(0.95f, 1.08f, Mathf.Clamp01(snapshot.Heights[index] / 8f));
            float angleFactor = 1f;
            float heightFactor = Mathf.Lerp(
                0.94f,
                1.10f,
                Mathf.Clamp01(snapshot.Heights[index] / 4f));
            return new Color(
                Mathf.Clamp01(baseColor.r * shadeFactor * angleFactor * heightFactor),
                Mathf.Clamp01(baseColor.g * shadeFactor * angleFactor * heightFactor),
                Mathf.Clamp01(baseColor.b * shadeFactor * angleFactor * heightFactor),
                1f);
        }

        /// <summary>
        /// Selectors are direct owner texture indices, not a repeating biome
        /// number.  The Enhanced pass keeps the owner water selectors on the
        /// reflective path, then uses a bounded Level 1 albedo table for the
        /// observed land and rock families until its optional HD material
        /// atlas is bound.  This is presentation-only color grouping; the
        /// authoritative selector plane remains unchanged.
        /// </summary>
        internal static Color NightPaletteForSelector(byte terrainType)
        {
            Color baseColor;
            if (terrainType == 0)
            {
                baseColor = new Color(0.028f, 0.115f, 0.20f, 1f);
            }
            else if (IsReflectiveSelector(terrainType))
            {
                baseColor = new Color(0.10f, 0.30f, 0.42f, 1f);
            }
            else
            {
                switch (terrainType)
                {
                    // Dark earth and shadowed rock selectors from the
                    // owner-observed Level 1 BL32N closure.
                    case 1:
                    case 22:
                    case 52:
                    case 69:
                    case 72:
                    case 73:
                    case 75:
                    case 76:
                    case 84:
                    case 97:
                    case 124:
                        baseColor = new Color(0.16f, 0.25f, 0.30f, 1f);
                        break;

                    // Mid-value stone and exposed shelf selectors remain
                    // distinct from the greener land family at night.
                    case 4:
                    case 23:
                    case 24:
                    case 25:
                    case 26:
                    case 27:
                    case 53:
                    case 54:
                    case 65:
                    case 71:
                    case 74:
                    case 85:
                    case 120:
                    case 121:
                    case 122:
                    case 125:
                        baseColor = new Color(0.23f, 0.35f, 0.42f, 1f);
                        break;

                    // The brighter owner-observed selectors form the
                    // moonlit grass/soil family used by the Level 1 ground.
                    case 2:
                    case 5:
                    case 9:
                    case 55:
                    case 56:
                    case 57:
                    case 58:
                    case 59:
                    case 64:
                    case 66:
                    case 67:
                    case 70:
                    case 86:
                    case 87:
                    case 88:
                    case 89:
                    case 99:
                    case 101:
                    case 123:
                    case 128:
                    case 131:
                    case 136:
                    case 139:
                        baseColor = new Color(0.28f, 0.46f, 0.35f, 1f);
                        break;

                    // Preserve a useful cool land fallback for selectors
                    // outside the currently observed Level 1 closure.
                    default:
                        baseColor = new Color(0.20f, 0.36f, 0.34f, 1f);
                        break;
                }
            }

            float variation = 0.97f + ((terrainType * 17) & 7) * 0.008f;
            return new Color(
                Mathf.Clamp01(baseColor.r * variation),
                Mathf.Clamp01(baseColor.g * variation),
                Mathf.Clamp01(baseColor.b * variation),
                1f);
        }

        private static bool IsReflectiveSelector(byte terrainType)
        {
            switch (terrainType)
            {
                case 0:
                case 35:
                case 36:
                case 37:
                case 38:
                case 39:
                case 44:
                case 45:
                case 46:
                case 47:
                case 80:
                case 81:
                case 82:
                case 83:
                case 102:
                case 103:
                case 104:
                case 105:
                case 106:
                case 107:
                case 108:
                case 109:
                case 110:
                case 111:
                case 112:
                case 113:
                case 114:
                case 115:
                case 116:
                case 117:
                case 118:
                case 119:
                case 126:
                case 127:
                    return true;
                default:
                    return false;
            }
        }

        private static bool IsFinite(Vector3 value)
        {
            return IsFinite(value.x) && IsFinite(value.y) && IsFinite(value.z);
        }

        private static bool IsFinite(float value)
        {
            return !float.IsNaN(value) && !float.IsInfinity(value);
        }

        private static bool Approximately(Vector3 left, Vector3 right)
        {
            return Mathf.Abs(left.x - right.x) < 0.001f &&
                   Mathf.Abs(left.y - right.y) < 0.001f &&
                   Mathf.Abs(left.z - right.z) < 0.001f;
        }

        private void OnDestroy()
        {
            if (terrainMesh != null)
            {
                DestroyImmediate(terrainMesh);
            }

            if (terrainMaterial != null)
            {
                DestroyImmediate(terrainMaterial);
            }
        }
    }
}
