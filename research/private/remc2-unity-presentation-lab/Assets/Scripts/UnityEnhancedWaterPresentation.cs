using System;
using UnityEngine;
using UnityEngine.Rendering;
using UnityEngine.Rendering.Universal;

namespace MagicCarpet.PresentationLab
{
    public enum UnityEnhancedWaterReflectionMode
    {
        FallbackSky = 0,
        PlanarWhenSupported = 1,
        // Kept as an ABI/source-compatibility alias for the earlier water
        // contract.  Enhanced now fills this slot with a planar reflection.
        ScreenSpaceWhenSupported = PlanarWhenSupported
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
        private const float WaterSurfaceOffset = 0.08f;
        private const float ShorelineSurfaceOffset = 0.012f;
        private const float RippleSurfaceOffset = 0.035f;
        private const float RippleLifetime = 5f;
        private const float ReflectionClipOffset = 0.05f;
        private const int MinimumReflectionDimension = 64;
        private const int MaximumReflectionDimension = 512;
        private const string WaterLayerName = "Water";
        private const string UiLayerName = "UI";
        private MeshFilter waterFilter;
        private MeshRenderer waterRenderer;
        private MeshFilter shorelineFilter;
        private MeshRenderer shorelineRenderer;
        private Mesh waterMesh;
        private Mesh shorelineMesh;
        private MeshFilter rippleFilter;
        private MeshRenderer rippleRenderer;
        private Mesh rippleMesh;
        private GameObject rippleObject;
        private Material waterMaterial;
        private Material shorelineMaterial;
        private Material rippleMaterial;
        private ulong generation;
        private ulong lastTick;
        private bool hasSnapshot;
        private int activeRippleCount;
        private Camera presentationCamera;
        private Camera reflectionCamera;
        private RenderTexture reflectionTexture;
        private float waterSurfaceLevel;
        private Vector3 waterCenter;
        private Vector2 waterSize;
        private float rippleMaxRadius;
        private float rippleStartTime;
        private int waterLayer = -1;
        private int reflectionWidth;
        private int reflectionHeight;
        private bool renderingReflection;

        public bool HasSnapshot { get { return hasSnapshot; } }
        public ulong Generation { get { return generation; } }
        public ulong LastTick { get { return lastTick; } }
        public bool ReflectionEnabled { get; private set; }
        public UnityEnhancedWaterReflectionMode ReflectionMode { get; private set; }
        public int ShorelineCellCount { get; private set; }
        public int ShorelineVertexCount { get { return shorelineMesh == null ? 0 : shorelineMesh.vertexCount; } }
        public Material WaterMaterial { get { return waterMaterial; } }
        public int ActiveRippleCount { get { return activeRippleCount; } }
        public bool PlanarReflectionReady { get; private set; }
        public int PlanarReflectionWidth { get { return reflectionWidth; } }
        public int PlanarReflectionHeight { get { return reflectionHeight; } }

        /// <summary>
        /// Supplies the Enhanced presentation camera used to render the
        /// bounded, owner-world-only planar reflection.  Exact and Classic HD
        /// never instantiate this component.
        /// </summary>
        public void SetPresentationCamera(Camera camera)
        {
            if (presentationCamera == camera)
            {
                if (camera != null)
                {
                    EnsureReflectionCamera();
                }
                return;
            }

            presentationCamera = camera;
            DisablePlanarReflection();
            if (presentationCamera != null)
            {
                EnsureReflectionCamera();
            }
        }

        /// <summary>
        /// Renders the current Enhanced world from the mirrored camera.  The
        /// caller invokes this after presentation interpolation and before
        /// the main camera render; owner simulation state is never changed.
        /// </summary>
        public bool RenderReflection()
        {
            if (!ReflectionEnabled || !hasSnapshot || presentationCamera == null ||
                waterMaterial == null || renderingReflection)
            {
                if (!ReflectionEnabled || !hasSnapshot || presentationCamera == null)
                {
                    DisablePlanarReflection();
                }
                return PlanarReflectionReady;
            }

            if (!EnsureReflectionCamera() || !EnsureReflectionTexture())
            {
                DisablePlanarReflection();
                return false;
            }

            ConfigureReflectionCamera();
            Vector3 planePosition = new Vector3(
                waterCenter.x,
                waterSurfaceLevel,
                waterCenter.z);
            Vector3 planeNormal = Vector3.up;
            Vector4 reflectionPlane = new Vector4(
                planeNormal.x,
                planeNormal.y,
                planeNormal.z,
                -Vector3.Dot(planeNormal, planePosition));
            Matrix4x4 reflectionMatrix = CalculateReflectionMatrix(reflectionPlane);
            Vector3 reflectedPosition = reflectionMatrix.MultiplyPoint(
                presentationCamera.transform.position);
            Vector3 reflectedForward = reflectionMatrix.MultiplyVector(
                presentationCamera.transform.forward);
            Vector3 reflectedUp = reflectionMatrix.MultiplyVector(
                presentationCamera.transform.up);
            if (reflectedForward.sqrMagnitude < 0.000001f ||
                reflectedUp.sqrMagnitude < 0.000001f)
            {
                DisablePlanarReflection();
                return false;
            }

            reflectionCamera.transform.SetPositionAndRotation(
                reflectedPosition,
                Quaternion.LookRotation(reflectedForward, reflectedUp));
            reflectionCamera.worldToCameraMatrix =
                presentationCamera.worldToCameraMatrix * reflectionMatrix;
            Vector4 clipPlane = CameraSpacePlane(
                reflectionCamera,
                planePosition,
                planeNormal,
                -1f);
            reflectionCamera.projectionMatrix = presentationCamera.CalculateObliqueMatrix(clipPlane);
            reflectionCamera.targetTexture = reflectionTexture;

            bool priorInvertCulling = GL.invertCulling;
            bool priorReflectionCameraEnabled = reflectionCamera.enabled;
            RenderTexture priorActive = RenderTexture.active;
            bool priorWaterEnabled = waterRenderer != null && waterRenderer.enabled;
            bool priorShorelineEnabled = shorelineRenderer != null && shorelineRenderer.enabled;
            renderingReflection = true;
            try
            {
                // The Water layer is excluded when available.  The fallback
                // renderer toggle also prevents recursion in projects with a
                // stripped TagManager that has no Water layer.
                if (waterLayer < 0)
                {
                    if (waterRenderer != null) waterRenderer.enabled = false;
                    if (shorelineRenderer != null) shorelineRenderer.enabled = false;
                }

                reflectionCamera.enabled = false;
                GL.invertCulling = !priorInvertCulling;
                reflectionCamera.Render();
                waterMaterial.SetTexture("_PlanarReflectionTexture", reflectionTexture);
                waterMaterial.SetFloat("_PlanarReflectionEnabled", 1f);
                PlanarReflectionReady = true;
                ReflectionMode = UnityEnhancedWaterReflectionMode.PlanarWhenSupported;
                return true;
            }
            catch (Exception)
            {
                DisablePlanarReflection();
                return false;
            }
            finally
            {
                GL.invertCulling = priorInvertCulling;
                RenderTexture.active = priorActive;
                reflectionCamera.enabled = priorReflectionCameraEnabled;
                if (waterRenderer != null) waterRenderer.enabled = priorWaterEnabled;
                if (shorelineRenderer != null) shorelineRenderer.enabled = priorShorelineEnabled;
                renderingReflection = false;
            }
        }

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

            if (hasSnapshot && snapshot.Generation != generation)
            {
                ClearRipple();
            }
            EnsureResources();
            BuildWater(snapshot);
            BuildShoreline(snapshot);
            waterCenter = snapshot.Center;
            waterSurfaceLevel = snapshot.Level + WaterSurfaceOffset;
            waterSize = snapshot.Size;
            ReflectionEnabled = snapshot.ReflectionEnabled;
            ReflectionMode = ReflectionEnabled
                ? UnityEnhancedWaterReflectionMode.PlanarWhenSupported
                : UnityEnhancedWaterReflectionMode.FallbackSky;
            if (!ReflectionEnabled)
            {
                DisablePlanarReflection();
            }
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
            ReflectionMode = UnityEnhancedWaterReflectionMode.FallbackSky;
            DisablePlanarReflection();
            ShorelineCellCount = 0;
            waterSurfaceLevel = 0f;
            waterCenter = Vector3.zero;
            ClearRipple();
            if (waterMesh != null) waterMesh.Clear();
            if (shorelineMesh != null) shorelineMesh.Clear();
        }

        public bool ApplyRipple(UnityEnhancedVfxEvent splash)
        {
            return ApplyRipple(splash, lastTick);
        }

        /// <summary>
        /// Applies an owner-authored water impact against the currently
        /// retained water surface. Water geometry is published less often
        /// than the complete world snapshot, so the event horizon must be
        /// the containing world tick rather than the geometry tick stored by
        /// this component.
        /// </summary>
        public bool ApplyRipple(UnityEnhancedVfxEvent splash, ulong latestPresentationTick)
        {
            if (!hasSnapshot || splash.Kind != UnityEnhancedVfxKind.Weather ||
                splash.Generation != generation || splash.Tick > latestPresentationTick || splash.Sequence == 0 ||
                !IsFinite(splash.Position) || !IsFinite(splash.Intensity) || splash.Intensity <= 0f ||
                waterMaterial == null)
            {
                return false;
            }

            waterMaterial.SetVector("_RippleCenter", splash.Position);
            waterMaterial.SetFloat("_RippleStartTime", Time.time);
            waterMaterial.SetFloat("_RippleStrength", Mathf.Clamp(splash.Intensity, 0.45f, 1.35f));
            if (rippleMaterial != null && rippleObject != null)
            {
                rippleStartTime = Time.time;
                rippleMaxRadius = Mathf.Clamp(
                    Mathf.Max(waterSize.x, waterSize.y) * 0.5f,
                    2f,
                    8f);
                rippleObject.transform.position = new Vector3(
                    splash.Position.x,
                    waterSurfaceLevel + RippleSurfaceOffset,
                    splash.Position.z);
                rippleObject.transform.localScale = new Vector3(
                    rippleMaxRadius,
                    1f,
                    rippleMaxRadius);
                rippleMaterial.SetVector("_RippleCenter", splash.Position);
                rippleMaterial.SetFloat("_RippleStartTime", rippleStartTime);
                rippleMaterial.SetFloat("_RippleStrength", Mathf.Clamp(splash.Intensity, 0.55f, 1.5f));
                rippleMaterial.SetFloat("_RippleMaxRadius", rippleMaxRadius);
                rippleRenderer.enabled = true;
            }
            activeRippleCount = 1;
            return true;
        }

        private void Update()
        {
            if (activeRippleCount != 0 && Time.time - rippleStartTime >= RippleLifetime)
            {
                ClearRipple();
            }
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
                ConfigureWaterLayer(water);
                waterMaterial = CreateMaterial(
                    "EnhancedWaterMaterial",
                    new Color(0.035f, 0.18f, 0.31f, 0.72f),
                    false);
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
                ConfigureWaterLayer(shoreline);
                shorelineMaterial = CreateMaterial(
                    "EnhancedShorelineMaterial",
                    new Color(0.055f, 0.16f, 0.23f, 1f),
                    true);
                shorelineRenderer.sharedMaterial = shorelineMaterial;
            }

            if (rippleFilter == null)
            {
                rippleObject = new GameObject("EnhancedWaterRipple");
                rippleObject.transform.SetParent(transform, false);
                rippleObject.hideFlags = HideFlags.DontSave;
                rippleFilter = rippleObject.AddComponent<MeshFilter>();
                rippleRenderer = rippleObject.AddComponent<MeshRenderer>();
                rippleMesh = CreateRippleQuadMesh();
                rippleFilter.sharedMesh = rippleMesh;
                ConfigureWaterLayer(rippleObject);
                rippleMaterial = CreateRippleMaterial();
                rippleRenderer.sharedMaterial = rippleMaterial;
                rippleRenderer.enabled = false;
            }
        }

        private void BuildWater(UnityEnhancedWaterSnapshot snapshot)
        {
            float halfX = snapshot.Size.x * 0.5f;
            float halfZ = snapshot.Size.y * 0.5f;
            waterMesh.Clear();
            waterMesh.vertices = new[]
            {
                snapshot.Center + new Vector3(-halfX, snapshot.Level + WaterSurfaceOffset, -halfZ),
                snapshot.Center + new Vector3(halfX, snapshot.Level + WaterSurfaceOffset, -halfZ),
                snapshot.Center + new Vector3(halfX, snapshot.Level + WaterSurfaceOffset, halfZ),
                snapshot.Center + new Vector3(-halfX, snapshot.Level + WaterSurfaceOffset, halfZ)
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
                    new Vector3((x + 0.5f) * cell,
                        snapshot.Level + WaterSurfaceOffset + ShorelineSurfaceOffset,
                        (z + 0.5f) * cell);
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

        private static Material CreateMaterial(string name, Color color, bool shoreline)
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
                material.SetColor("_BaseColor", new Color(0.008f, 0.045f, 0.14f, 1f));
                material.SetColor("_ReflectionColor", new Color(0.06f, 0.30f, 0.76f, 1f));
                material.SetColor("_FoamColor", new Color(0.16f, 0.52f, 1.0f, 1f));
                material.SetColor("_MoonColor", new Color(0.44f, 0.74f, 1.0f, 1f));
                material.SetFloat("_WaveScale", 0.085f);
                material.SetFloat("_WaveSpeed", 0.32f);
                material.SetFloat("_WaveStrength", 0.42f);
                material.SetFloat("_FresnelStrength", 1.85f);
                material.SetFloat("_SpecularStrength", 2.80f);
                material.SetFloat("_ReflectionStrength", 1.50f);
                material.SetFloat("_SceneReflectionStrength", 0.94f);
                material.SetFloat("_ReflectionDistortion", 0.045f);
                material.SetTexture("_PlanarReflectionTexture", null);
                material.SetFloat("_PlanarReflectionEnabled", 0f);
                // Let the real bounded reflection camera carry more of the
                // water read at game distance while retaining the screen-space
                // fallback for OpenGL paths where the reflected horizon is
                // sparse.
                material.SetFloat("_PlanarReflectionMix", 0.78f);
                material.SetColor("_CausticColor", new Color(0.18f, 0.65f, 1.0f, 1f));
                material.SetFloat("_CausticScale", 0.22f);
                material.SetFloat("_CausticStrength", 1.15f);
                material.SetFloat("_RippleSpeed", 2.8f);
                material.SetFloat("_RippleWidth", 0.65f);
                material.SetVector("_RippleCenter", Vector4.zero);
                material.SetFloat("_RippleStartTime", 0f);
                material.SetFloat("_RippleStrength", 0f);
                material.SetFloat("_ShorelineMode", shoreline ? 1f : 0f);
                material.SetFloat("_ShorelineFoamStrength", shoreline ? 0.82f : 0f);
                material.SetFloat("_ShorelineFoamScale", shoreline ? 0.34f : 0f);
                material.SetFloat("_Alpha", 0.94f);
                if (shoreline)
                {
                    material.SetColor("_BaseColor", new Color(0.012f, 0.06f, 0.16f, 1f));
                    material.SetColor("_ReflectionColor", new Color(0.025f, 0.13f, 0.34f, 1f));
                    material.SetColor("_FoamColor", new Color(0.32f, 0.74f, 1.0f, 1f));
                    material.SetColor("_MoonColor", new Color(0.48f, 0.72f, 1.0f, 1f));
                    material.SetFloat("_WaveStrength", 0.20f);
                    material.SetFloat("_FresnelStrength", 1.20f);
                    material.SetFloat("_SpecularStrength", 1.45f);
                    material.SetFloat("_ReflectionStrength", 0.72f);
                    material.SetFloat("_SceneReflectionStrength", 0.38f);
                    material.SetFloat("_CausticStrength", 0.55f);
                    material.SetFloat("_Alpha", 0.88f);
                }
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

        private static Mesh CreateRippleQuadMesh()
        {
            Mesh mesh = new Mesh
            {
                name = "EnhancedWaterRippleMesh",
                hideFlags = HideFlags.DontSave
            };
            mesh.vertices = new[]
            {
                new Vector3(-1f, 0f, -1f),
                new Vector3(1f, 0f, -1f),
                new Vector3(1f, 0f, 1f),
                new Vector3(-1f, 0f, 1f)
            };
            mesh.uv = new[]
            {
                new Vector2(0f, 0f),
                new Vector2(1f, 0f),
                new Vector2(1f, 1f),
                new Vector2(0f, 1f)
            };
            mesh.triangles = new[] { 0, 2, 1, 0, 3, 2 };
            mesh.RecalculateNormals();
            mesh.RecalculateBounds();
            return mesh;
        }

        private static Material CreateRippleMaterial()
        {
            Shader shader = Shader.Find("MagicCarpet/Enhanced/WaterRipple");
            if (shader == null)
            {
                return null;
            }

            Material material = new Material(shader)
            {
                name = "EnhancedWaterRippleMaterial",
                hideFlags = HideFlags.DontSave
            };
            material.enableInstancing = true;
            material.SetColor("_RippleColor", new Color(0.28f, 0.68f, 1f, 1f));
            material.SetFloat("_RippleStrength", 0f);
            material.SetFloat("_RippleStartTime", 0f);
            material.SetFloat("_RippleSpeed", 2.8f);
            material.SetFloat("_RippleWidth", 0.12f);
            material.SetFloat("_RippleMaxRadius", 4f);
            material.SetFloat("_RippleLifetime", RippleLifetime);
            material.SetVector("_RippleCenter", Vector4.zero);
            return material;
        }

        private void ClearRipple()
        {
            activeRippleCount = 0;
            if (waterMaterial != null)
            {
                waterMaterial.SetFloat("_RippleStrength", 0f);
            }
            if (rippleMaterial != null)
            {
                rippleMaterial.SetFloat("_RippleStrength", 0f);
            }
            if (rippleRenderer != null)
            {
                rippleRenderer.enabled = false;
            }
        }

        private static bool IsFinite(float value)
        {
            return !float.IsNaN(value) && !float.IsInfinity(value);
        }

        private static bool IsFinite(Vector3 value)
        {
            return IsFinite(value.x) && IsFinite(value.y) && IsFinite(value.z);
        }

        private bool EnsureReflectionCamera()
        {
            if (reflectionCamera != null)
            {
                return true;
            }

            if (!SystemInfo.supportsRenderTextures)
            {
                return false;
            }

            GameObject reflectionObject = new GameObject("EnhancedWaterReflectionCamera");
            reflectionObject.transform.SetParent(transform, false);
            reflectionObject.hideFlags = HideFlags.DontSave;
            reflectionCamera = reflectionObject.AddComponent<Camera>();
            reflectionCamera.enabled = false;
            reflectionCamera.cameraType = CameraType.Reflection;
            reflectionCamera.useOcclusionCulling = false;
            reflectionCamera.stereoTargetEye = StereoTargetEyeMask.None;

            UniversalAdditionalCameraData cameraData =
                reflectionObject.AddComponent<UniversalAdditionalCameraData>();
            cameraData.renderType = CameraRenderType.Base;
            cameraData.renderPostProcessing = false;
            cameraData.requiresColorTexture = false;
            cameraData.requiresDepthTexture = false;
            return true;
        }

        private bool EnsureReflectionTexture()
        {
            int sourceWidth = presentationCamera.targetTexture != null
                ? presentationCamera.targetTexture.width
                : Screen.width;
            int sourceHeight = presentationCamera.targetTexture != null
                ? presentationCamera.targetTexture.height
                : Screen.height;
            if (sourceWidth <= 0 || sourceHeight <= 0)
            {
                sourceWidth = 256;
                sourceHeight = 256;
            }

            float scale = Mathf.Min(
                1f,
                (float)MaximumReflectionDimension / Mathf.Max(sourceWidth, sourceHeight));
            int width = Mathf.Clamp(
                Mathf.RoundToInt(sourceWidth * scale),
                MinimumReflectionDimension,
                MaximumReflectionDimension);
            int height = Mathf.Clamp(
                Mathf.RoundToInt(sourceHeight * scale),
                MinimumReflectionDimension,
                MaximumReflectionDimension);
            if (reflectionTexture != null &&
                reflectionWidth == width && reflectionHeight == height &&
                reflectionTexture.IsCreated())
            {
                return true;
            }

            ReleaseReflectionTexture();
            RenderTextureFormat format =
                SystemInfo.SupportsRenderTextureFormat(RenderTextureFormat.ARGBHalf)
                    ? RenderTextureFormat.ARGBHalf
                    : RenderTextureFormat.ARGB32;
            try
            {
                reflectionTexture = new RenderTexture(
                    width,
                    height,
                    24,
                    format,
                    RenderTextureReadWrite.sRGB);
                reflectionTexture.name = "EnhancedWaterPlanarReflection";
                reflectionTexture.hideFlags = HideFlags.DontSave;
                reflectionTexture.filterMode = FilterMode.Bilinear;
                reflectionTexture.wrapMode = TextureWrapMode.Clamp;
                reflectionTexture.useMipMap = false;
                reflectionTexture.autoGenerateMips = false;
                reflectionTexture.Create();
                reflectionWidth = width;
                reflectionHeight = height;
                return reflectionTexture.IsCreated();
            }
            catch (Exception)
            {
                ReleaseReflectionTexture();
                return false;
            }
        }

        private void ConfigureReflectionCamera()
        {
            reflectionCamera.clearFlags = presentationCamera.clearFlags;
            reflectionCamera.backgroundColor = presentationCamera.backgroundColor;
            reflectionCamera.fieldOfView = presentationCamera.fieldOfView;
            reflectionCamera.nearClipPlane = presentationCamera.nearClipPlane;
            reflectionCamera.farClipPlane = presentationCamera.farClipPlane;
            reflectionCamera.orthographic = presentationCamera.orthographic;
            reflectionCamera.orthographicSize = presentationCamera.orthographicSize;
            reflectionCamera.allowHDR = presentationCamera.allowHDR;
            reflectionCamera.cullingMask = ReflectionCullingMask();
            reflectionCamera.aspect = (float)reflectionWidth / Mathf.Max(1, reflectionHeight);
            reflectionCamera.depthTextureMode = DepthTextureMode.None;
            reflectionCamera.useOcclusionCulling = false;
        }

        private int ReflectionCullingMask()
        {
            int mask = presentationCamera == null ? 0 : presentationCamera.cullingMask;
            if (waterLayer >= 0)
            {
                mask &= ~(1 << waterLayer);
            }

            int uiLayer = LayerMask.NameToLayer(UiLayerName);
            if (uiLayer >= 0)
            {
                mask &= ~(1 << uiLayer);
            }

            return mask;
        }

        private void ConfigureWaterLayer(GameObject waterObject)
        {
            waterLayer = LayerMask.NameToLayer(WaterLayerName);
            if (waterLayer >= 0 && waterObject != null)
            {
                waterObject.layer = waterLayer;
            }
        }

        private void DisablePlanarReflection()
        {
            PlanarReflectionReady = false;
            if (waterMaterial != null)
            {
                waterMaterial.SetFloat("_PlanarReflectionEnabled", 0f);
                waterMaterial.SetTexture("_PlanarReflectionTexture", null);
            }

            ReflectionMode = UnityEnhancedWaterReflectionMode.FallbackSky;
        }

        private void ReleaseReflectionTexture()
        {
            if (reflectionTexture != null)
            {
                reflectionTexture.Release();
                DestroyUnityObject(reflectionTexture);
                reflectionTexture = null;
            }

            reflectionWidth = 0;
            reflectionHeight = 0;
            PlanarReflectionReady = false;
        }

        private static Vector4 CameraSpacePlane(
            Camera camera,
            Vector3 position,
            Vector3 normal,
            float sideSign)
        {
            Vector3 offsetPosition = position + normal * ReflectionClipOffset;
            Matrix4x4 worldToCamera = camera.worldToCameraMatrix;
            Vector3 cameraSpacePosition = worldToCamera.MultiplyPoint(offsetPosition);
            Vector3 cameraSpaceNormal = worldToCamera.MultiplyVector(normal).normalized * sideSign;
            return new Vector4(
                cameraSpaceNormal.x,
                cameraSpaceNormal.y,
                cameraSpaceNormal.z,
                -Vector3.Dot(cameraSpacePosition, cameraSpaceNormal));
        }

        private static Matrix4x4 CalculateReflectionMatrix(Vector4 plane)
        {
            Matrix4x4 reflection = Matrix4x4.zero;
            reflection.m00 = 1f - 2f * plane.x * plane.x;
            reflection.m01 = -2f * plane.x * plane.y;
            reflection.m02 = -2f * plane.x * plane.z;
            reflection.m03 = -2f * plane.x * plane.w;
            reflection.m10 = -2f * plane.y * plane.x;
            reflection.m11 = 1f - 2f * plane.y * plane.y;
            reflection.m12 = -2f * plane.y * plane.z;
            reflection.m13 = -2f * plane.y * plane.w;
            reflection.m20 = -2f * plane.z * plane.x;
            reflection.m21 = -2f * plane.z * plane.y;
            reflection.m22 = 1f - 2f * plane.z * plane.z;
            reflection.m23 = -2f * plane.z * plane.w;
            reflection.m30 = 0f;
            reflection.m31 = 0f;
            reflection.m32 = 0f;
            reflection.m33 = 1f;
            return reflection;
        }

        private static void DestroyUnityObject(UnityEngine.Object value)
        {
            if (value == null)
            {
                return;
            }

            if (Application.isPlaying)
            {
                Destroy(value);
            }
            else
            {
                DestroyImmediate(value);
            }
        }

        private void OnDestroy()
        {
            ReleaseReflectionTexture();
            if (reflectionCamera != null)
            {
                DestroyUnityObject(reflectionCamera.gameObject);
                reflectionCamera = null;
            }

            if (waterMesh != null) DestroyUnityObject(waterMesh);
            if (shorelineMesh != null) DestroyUnityObject(shorelineMesh);
            if (rippleMesh != null) DestroyUnityObject(rippleMesh);
            if (waterMaterial != null) DestroyUnityObject(waterMaterial);
            if (shorelineMaterial != null) DestroyUnityObject(shorelineMaterial);
            if (rippleMaterial != null) DestroyUnityObject(rippleMaterial);
        }
    }
}
