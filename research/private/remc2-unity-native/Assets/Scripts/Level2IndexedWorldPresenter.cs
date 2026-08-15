using UnityEngine;

namespace MagicCarpet.Native
{
    public sealed partial class MagicCarpetRuntime
    {
        private sealed class Level2IndexedWorldPresenter
        {
            private readonly MagicCarpetRuntime owner;

            internal Level2IndexedWorldPresenter(MagicCarpetRuntime owner)
            {
                this.owner = owner;
            }

            internal void Ensure()
            {
                MagicCarpetRuntime.Require(
                    owner.level2Mode && owner.campaignSession != null,
                    "Level 2 indexed frame presentation has a campaign owner");
                if (owner.level2IndexedFrameRenderer != null)
                {
                    return;
                }

                MagicCarpetRuntime.Require(
                    SystemInfo.SupportsTextureFormat(TextureFormat.R8),
                    "Level 2 indexed frame R8 texture support");
                MagicCarpetRuntime.Require(
                    SystemInfo.SupportsTextureFormat(TextureFormat.RGB24),
                    "Level 2 indexed frame RGB8 palette texture support");
                var shader = Resources.Load<Shader>("TerrainIndexedFrame");
                MagicCarpetRuntime.Require(
                    shader != null,
                    "Level 2 indexed frame palette shader");

                owner.level2IndexedFrameObject = new GameObject(
                    "Level 2 Day Rust indexed frame presentation");
                owner.level2IndexedFrameObject.transform.SetParent(
                    owner.transform,
                    false);
                owner.level2IndexedFrameObject.transform.localPosition =
                    Vector3.zero;
                owner.level2IndexedFrameObject.transform.localScale =
                    new Vector3(3.2f, 2f, 1f);
                var filter = owner.level2IndexedFrameObject.AddComponent<MeshFilter>();
                owner.level2IndexedFrameMesh = CreateQuad();
                filter.sharedMesh = owner.level2IndexedFrameMesh;
                owner.level2IndexedFrameRenderer =
                    owner.level2IndexedFrameObject.AddComponent<MeshRenderer>();
                owner.level2IndexedFrameRenderer.shadowCastingMode =
                    UnityEngine.Rendering.ShadowCastingMode.Off;
                owner.level2IndexedFrameRenderer.receiveShadows = false;

                owner.level2IndexedFrameIndexTexture = new Texture2D(
                    NativeSim.DayIndexedFrameWidth,
                    NativeSim.DayIndexedFrameHeight,
                    TextureFormat.R8,
                    false,
                    true)
                {
                    name = "Level 2 Day Rust indexed frame indices",
                    filterMode = FilterMode.Point,
                    wrapMode = TextureWrapMode.Clamp
                };
                owner.level2IndexedFramePaletteTexture = new Texture2D(
                    256,
                    1,
                    TextureFormat.RGB24,
                    false,
                    false)
                {
                    name = "Level 2 Day Rust indexed frame RGB8 palette",
                    filterMode = FilterMode.Point,
                    wrapMode = TextureWrapMode.Clamp
                };
                owner.level2IndexedFrameMaterial = new Material(shader)
                {
                    name = "Level 2 Day Rust indexed frame palette material"
                };
                owner.level2IndexedFrameMaterial.SetTexture(
                    "_Indices",
                    owner.level2IndexedFrameIndexTexture);
                owner.level2IndexedFrameMaterial.SetTexture(
                    "_Coverage",
                    Texture2D.whiteTexture);
                owner.level2IndexedFrameMaterial.SetTexture(
                    "_Palette",
                    owner.level2IndexedFramePaletteTexture);
                owner.level2IndexedFrameRenderer.sharedMaterial =
                    owner.level2IndexedFrameMaterial;
                owner.level2IndexedFrameRenderer.enabled = false;

                owner.level2IndexedFrameCamera = new GameObject(
                        "Level 2 Day indexed frame presentation camera")
                    .AddComponent<Camera>();
                owner.level2IndexedFrameCamera.transform.SetParent(
                    owner.transform,
                    false);
                owner.level2IndexedFrameCamera.transform.localPosition =
                    new Vector3(0f, 0f, -1f);
                owner.level2IndexedFrameCamera.transform.localRotation =
                    Quaternion.identity;
                owner.level2IndexedFrameCamera.clearFlags =
                    CameraClearFlags.SolidColor;
                owner.level2IndexedFrameCamera.backgroundColor = Color.black;
                owner.level2IndexedFrameCamera.orthographic = true;
                owner.level2IndexedFrameCamera.orthographicSize = 1f;
                owner.level2IndexedFrameCamera.aspect = 1.6f;
                owner.level2IndexedFrameCamera.nearClipPlane = 0.01f;
                owner.level2IndexedFrameCamera.farClipPlane = 10f;
                owner.level2IndexedFrameCamera.cullingMask =
                    1 << owner.level2IndexedFrameObject.layer;
                owner.level2IndexedFrameCamera.depth = -100f;
            }

            internal void Update(ulong expectedTick)
            {
                owner.RecordLevel2Presentation("indexed-world");
                Ensure();
                owner.level2IndexedFrameRenderer.enabled = false;
                owner.level2IndexedFrameLive = false;
                owner.level2IndexedFrameInfo = default;

                // The copy is generation/tick-bound in the native carrier. Do
                // not touch either Unity texture until the complete copy
                // succeeds; stale requests therefore cannot re-present prior
                // bytes.
                var info = owner.campaignSession.CopyDayIndexedFrame(expectedTick);
                var indices = owner.campaignSession.DayIndexedFrameIndices;
                var palette = owner.campaignSession.DayIndexedFramePalette;
                MagicCarpetRuntime.Require(
                    info.generation == owner.campaignSession.Generation &&
                    info.tick == expectedTick &&
                    info.indexBytes == (uint)indices.Length &&
                    info.paletteBytes == (uint)palette.Length,
                    "Level 2 indexed frame generation/tick and byte binding");

                owner.level2IndexedFrameIndexTexture.LoadRawTextureData(indices);
                owner.level2IndexedFrameIndexTexture.Apply(false, false);
                owner.level2IndexedFramePaletteTexture.LoadRawTextureData(palette);
                owner.level2IndexedFramePaletteTexture.Apply(false, false);
                owner.level2IndexedFrameInfo = info;
                owner.level2IndexedFrameRenderer.enabled = true;
                owner.level2IndexedFrameLive = true;
                owner.level2IndexedFrameUploadCount++;
            }

            internal void Dispose()
            {
                if (owner.level2IndexedFrameRenderer != null)
                {
                    owner.level2IndexedFrameRenderer.enabled = false;
                }

                DestroyObject(owner.level2IndexedFrameObject);
                DestroyObject(owner.level2IndexedFrameCamera?.gameObject);
                DestroyObject(owner.level2IndexedFrameMaterial);
                DestroyObject(owner.level2IndexedFrameIndexTexture);
                DestroyObject(owner.level2IndexedFramePaletteTexture);
                DestroyObject(owner.level2IndexedFrameMesh);

                owner.level2IndexedFrameObject = null;
                owner.level2IndexedFrameCamera = null;
                owner.level2IndexedFrameMesh = null;
                owner.level2IndexedFrameRenderer = null;
                owner.level2IndexedFrameIndexTexture = null;
                owner.level2IndexedFramePaletteTexture = null;
                owner.level2IndexedFrameMaterial = null;
                owner.level2IndexedFrameInfo = default;
                owner.level2IndexedFrameLive = false;
            }

            private static void DestroyObject(Object value)
            {
                if (value == null) return;
                if (Application.isPlaying)
                    Object.Destroy(value);
                else
                    Object.DestroyImmediate(value);
            }

            private static Mesh CreateQuad()
            {
                var mesh = new Mesh
                {
                    name = "Level 2 Day Rust indexed frame quad"
                };
                mesh.vertices = new[]
                {
                    new Vector3(-0.5f, -0.5f, 0f),
                    new Vector3(0.5f, -0.5f, 0f),
                    new Vector3(-0.5f, 0.5f, 0f),
                    new Vector3(0.5f, 0.5f, 0f)
                };
                // The native frame is top-to-bottom, while Unity samples raw
                // texture row zero at v=0. Keep the native bytes intact and
                // map the top vertices to v=0 so source order is preserved.
                mesh.uv = new[]
                {
                    new Vector2(0f, 1f),
                    new Vector2(1f, 1f),
                    new Vector2(0f, 0f),
                    new Vector2(1f, 0f)
                };
                mesh.triangles = new[] { 0, 2, 1, 2, 3, 1 };
                mesh.RecalculateBounds();
                return mesh;
            }
        }

        private void EnsureLevel2IndexedFramePresentation()
        {
            Level2IndexedWorld.Ensure();
        }

        private void UpdateLevel2IndexedFrame(ulong expectedTick)
        {
            Level2IndexedWorld.Update(expectedTick);
        }

        private void DestroyLevel2IndexedFramePresentation()
        {
            level2IndexedWorldPresenter?.Dispose();
        }
    }
}
