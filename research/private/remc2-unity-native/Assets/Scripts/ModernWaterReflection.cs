using System;
using UnityEngine;
using UnityEngine.Rendering;
using UnityEngine.Rendering.Universal;

namespace MagicCarpet.Native
{
    // NightSpire's authoritative outdoor water corners all lie at height zero.
    // This camera reflects the live scene; it never changes simulation state.
    internal sealed class ModernWaterReflection : IDisposable
    {
        private readonly Camera reflectionCamera;
        private readonly Material water;
        private readonly UniversalRenderPipeline.SingleCameraRequest request = new();
        private RenderTexture target;
        private bool disposed;
        private bool hasFrame;
        private Vector3 lastPosition;
        private Quaternion lastRotation;
        private Matrix4x4 lastProjection;
        private ulong lastTerrainRevision;
        private float lastRenderTime;
        internal int RenderedFrames { get; private set; }
        internal int ReusedFrames { get; private set; }
        internal int InvisibleFrames { get; private set; }

        internal ModernWaterReflection(Transform parent, Material water)
        {
            this.water = water;
            reflectionCamera = new GameObject("Water reflection camera").AddComponent<Camera>();
            reflectionCamera.transform.SetParent(parent, false);
            reflectionCamera.enabled = false;
            var data = reflectionCamera.GetUniversalAdditionalCameraData();
            data.renderPostProcessing = false;
            // Reflected smoke uses scene depth for soft intersections. There is
            // no opaque-color consumer; shadows stay on the main flight camera.
            data.requiresDepthTexture = true;
            data.requiresColorTexture = false;
            data.renderShadows = false;
            data.antialiasing = AntialiasingMode.None;
        }

        internal void Render(Camera source, ModernVegetation vegetation, ModernTerrainMesh terrain)
        {
            if (disposed) return;
            if (source.transform.position.y <= .25f || !terrain.WaterVisible(source))
            {
                water.SetFloat("_UsePlanarReflection", 0);
                hasFrame = false;
                InvisibleFrames++;
                return;
            }
            var width = Mathf.Clamp(source.pixelWidth / 2, 256, 1024);
            var height = Mathf.Clamp(Mathf.RoundToInt(width / source.aspect), 128, 1024);
            if (target == null || target.width != width || target.height != height)
            {
                ReleaseTarget();
                hasFrame = false;
                target = new RenderTexture(width, height, 24, RenderTextureFormat.ARGBHalf) {
                    name = "NightSpire reflected world", filterMode = FilterMode.Trilinear,
                    wrapMode = TextureWrapMode.Clamp, useMipMap = true, autoGenerateMips = false
                };
                request.destination = target;
                water.SetTexture("_ReflectionTexture", target);
            }
            if (!target.IsCreated() && !target.Create())
            {
                water.SetFloat("_UsePlanarReflection", 0);
                return;
            }
            // Static/small camera motion can reuse a frame for at most 1/30s.
            // Keep the matching VP matrix with the retained texture; a camera
            // cut, projection change or terrain mutation always refreshes it.
            if (hasFrame && terrain.Revision == lastTerrainRevision &&
                Time.unscaledTime - lastRenderTime < 1f / 30f &&
                (source.transform.position - lastPosition).sqrMagnitude < .0625f &&
                Quaternion.Angle(source.transform.rotation, lastRotation) < .5f &&
                source.projectionMatrix == lastProjection)
            {
                ReusedFrames++;
                return;
            }
            reflectionCamera.CopyFrom(source);
            reflectionCamera.enabled = false;
            reflectionCamera.allowMSAA = false;
            reflectionCamera.targetTexture = target;
            var mirror = Matrix4x4.Scale(new Vector3(1, -1, 1));
            var position = source.transform.position;
            position.y = -position.y;
            reflectionCamera.transform.SetPositionAndRotation(position,
                Quaternion.LookRotation(Vector3.Reflect(source.transform.forward, Vector3.up),
                    Vector3.Reflect(source.transform.up, Vector3.up)));
            reflectionCamera.worldToCameraMatrix = source.worldToCameraMatrix * mirror;
            var view = reflectionCamera.worldToCameraMatrix;
            var point = view.MultiplyPoint(new Vector3(0, .03f, 0));
            var normal = view.MultiplyVector(Vector3.up).normalized;
            reflectionCamera.projectionMatrix = reflectionCamera.CalculateObliqueMatrix(
                new Vector4(normal.x, normal.y, normal.z, -Vector3.Dot(point, normal)));
            if (!RenderPipeline.SupportsRenderRequest(reflectionCamera, request))
            {
                water.SetFloat("_UsePlanarReflection", 0);
                return;
            }
            // Instanced foliage is explicitly submitted per camera.
            vegetation.Render(reflectionCamera, true, (float)target.height / Mathf.Max(1, source.pixelHeight));
            var previousCulling = GL.invertCulling;
            var previousPass = Shader.GetGlobalFloat("_ModernReflectionPass");
            try
            {
                GL.invertCulling = !previousCulling;
                Shader.SetGlobalFloat("_ModernReflectionPass", 1);
                RenderPipeline.SubmitRenderRequest(reflectionCamera, request);
            }
            finally
            {
                GL.invertCulling = previousCulling;
                Shader.SetGlobalFloat("_ModernReflectionPass", previousPass);
            }
            target.GenerateMips();
            water.SetMatrix("_ReflectionVP", GL.GetGPUProjectionMatrix(reflectionCamera.projectionMatrix, true) * view);
            water.SetFloat("_UsePlanarReflection", 1);
            hasFrame = true;
            lastPosition = source.transform.position;
            lastRotation = source.transform.rotation;
            lastProjection = source.projectionMatrix;
            lastTerrainRevision = terrain.Revision;
            lastRenderTime = Time.unscaledTime;
            RenderedFrames++;
        }

        private void ReleaseTarget()
        {
            if (target == null) return;
            target.Release();
            Destroy(target);
            target = null;
        }

        public void Dispose()
        {
            if (disposed) return;
            disposed = true;
            water.SetFloat("_UsePlanarReflection", 0);
            ReleaseTarget();
            Destroy(reflectionCamera.gameObject);
        }

        private static void Destroy(UnityEngine.Object value)
        {
            if (Application.isPlaying) UnityEngine.Object.Destroy(value);
            else UnityEngine.Object.DestroyImmediate(value);
        }
    }
}
