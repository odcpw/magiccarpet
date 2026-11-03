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

        internal ModernWaterReflection(Transform parent, Material water)
        {
            this.water = water;
            reflectionCamera = new GameObject("Water reflection camera").AddComponent<Camera>();
            reflectionCamera.transform.SetParent(parent, false);
            reflectionCamera.enabled = false;
            var data = reflectionCamera.GetUniversalAdditionalCameraData();
            data.renderPostProcessing = false;
            data.requiresDepthTexture = true;
            data.requiresColorTexture = false;
            data.antialiasing = AntialiasingMode.None;
        }

        internal void Render(Camera source, ModernVegetation vegetation)
        {
            if (disposed) return;
            if (source.transform.position.y <= .25f)
            {
                water.SetFloat("_UsePlanarReflection", 0);
                return;
            }
            var width = Mathf.Clamp(source.pixelWidth / 2, 256, 1024);
            var height = Mathf.Clamp(Mathf.RoundToInt(width / source.aspect), 128, 1024);
            if (target == null || target.width != width || target.height != height)
            {
                ReleaseTarget();
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
            vegetation.Render(reflectionCamera);
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
        }

        private void ReleaseTarget()
        {
            if (target == null) return;
            target.Release();
            UnityEngine.Object.Destroy(target);
            target = null;
        }

        public void Dispose()
        {
            if (disposed) return;
            disposed = true;
            water.SetFloat("_UsePlanarReflection", 0);
            ReleaseTarget();
            UnityEngine.Object.Destroy(reflectionCamera.gameObject);
        }
    }
}
