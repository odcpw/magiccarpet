using System;
using UnityEngine;
using UnityEngine.Rendering;
using UnityEngine.UI;

namespace MagicCarpet.PresentationLab
{
    /// <summary>
    /// Owner-backed indexed HUD for Unity Enhanced. The Rust Classic HD graph
    /// supplies the frame; Unity only uploads its indexed pixels, provenance,
    /// and owner palette to a transparent overlay over the modern camera.
    /// </summary>
    [DisallowMultipleComponent]
    public sealed class UnityEnhancedHudPresentation : MonoBehaviour
    {
        private const string ShaderResourceName = "EnhancedOwnerHudIndexed";

        private Canvas canvas;
        private RawImage frameImage;
        private AspectRatioFitter aspectFitter;
        private Texture2D indexTexture;
        private Texture2D provenanceTexture;
        private Texture2D paletteTexture;
        private Material ownerHudMaterial;
        private byte[] indexUpload;
        private byte[] provenanceUpload;
        private byte[] paletteUpload;
        private ulong generation;
        private ulong lastTick;
        private ulong frameGeneration;
        private ulong frameTick;
        private int frameWidth;
        private int frameHeight;

        public ulong Generation { get { return generation; } }
        public ulong LastTick { get { return lastTick; } }
        public ulong FrameGeneration { get { return frameGeneration; } }
        public ulong FrameTick { get { return frameTick; } }
        public int FrameWidth { get { return frameWidth; } }
        public int FrameHeight { get { return frameHeight; } }
        public bool IsVisible { get { return canvas != null && canvas.enabled; } }
        public Texture2D IndexTexture { get { return indexTexture; } }
        public Texture2D ProvenanceTexture { get { return provenanceTexture; } }
        public Texture2D PaletteTexture { get { return paletteTexture; } }
        public Material OwnerHudMaterial { get { return ownerHudMaterial; } }
        public RawImage FrameImage { get { return frameImage; } }

        public bool CompositeOwnerHudTo(RenderTexture target, out string reason)
        {
            reason = string.Empty;
            if (target == null || !target.IsCreated())
            {
                reason = "Unity Enhanced owner HUD capture target is not created";
                return false;
            }

            if (ownerHudMaterial == null || indexTexture == null || provenanceTexture == null ||
                paletteTexture == null || frameWidth <= 0 || frameHeight <= 0)
            {
                reason = "Unity Enhanced owner HUD material is not uploaded for capture";
                return false;
            }

            Rect viewport = AspectFitViewport(target.width, target.height);
            CommandBuffer command = new CommandBuffer
            {
                name = "UnityEnhanced_OwnerHudCaptureComposite"
            };
            RenderTexture previousActive = RenderTexture.active;
            try
            {
                command.SetRenderTarget(target);
                command.SetViewport(viewport);
                command.Blit(
                    Texture2D.whiteTexture,
                    BuiltinRenderTextureType.CurrentActive,
                    ownerHudMaterial);
                Graphics.ExecuteCommandBuffer(command);
                return true;
            }
            catch (Exception exception)
            {
                reason = "Unity Enhanced owner HUD capture composite failed: " + exception.Message;
                return false;
            }
            finally
            {
                RenderTexture.active = previousActive;
                command.Release();
            }
        }

        public bool ApplySnapshot(UnityEnhancedHudSnapshot snapshot, Camera targetCamera)
        {
            if (snapshot == null || targetCamera == null || snapshot.Generation == 0 ||
                snapshot.OwnerFrame == null)
            {
                return false;
            }

            string frameReason;
            if (!snapshot.OwnerFrame.TryValidate(snapshot.Generation, snapshot.Tick, out frameReason))
            {
                return false;
            }

            if (generation != 0 && snapshot.Generation < generation)
            {
                return false;
            }

            if (generation == snapshot.Generation && snapshot.Tick < lastTick)
            {
                return false;
            }

            if (!EnsureSurface(snapshot.OwnerFrame, targetCamera, out frameReason))
            {
                return false;
            }

            UploadFrame(snapshot.OwnerFrame);
            generation = snapshot.Generation;
            lastTick = snapshot.Tick;
            frameGeneration = snapshot.OwnerFrame.Generation;
            frameTick = snapshot.OwnerFrame.Tick;
            canvas.enabled = true;
            return true;
        }

        public void ResetForGeneration(ulong nextGeneration)
        {
            generation = nextGeneration;
            lastTick = 0;
            frameGeneration = 0;
            frameTick = 0;
            if (canvas != null)
            {
                canvas.enabled = false;
            }
        }

        public void ResetForSession()
        {
            ResetForGeneration(0);
        }

        public void HideForSnapshot()
        {
            if (canvas != null)
            {
                canvas.enabled = false;
            }
        }

        private bool EnsureSurface(
            UnityEnhancedOwnerHudFrame frame,
            Camera targetCamera,
            out string reason)
        {
            reason = string.Empty;
            EnsureHierarchy(targetCamera);
            if (indexTexture != null &&
                frameWidth == frame.Width &&
                frameHeight == frame.Height &&
                ownerHudMaterial != null)
            {
                canvas.worldCamera = targetCamera;
                aspectFitter.aspectRatio = (float)frame.Width / frame.Height;
                return true;
            }

            ReleaseSurface();
            try
            {
                frameWidth = frame.Width;
                frameHeight = frame.Height;
                indexUpload = new byte[checked(frame.Stride * frame.Height)];
                provenanceUpload = new byte[indexUpload.Length];
                paletteUpload = new byte[256 * 4];

                indexTexture = new Texture2D(frame.Width, frame.Height, TextureFormat.R8, false, true);
                indexTexture.name = "EnhancedOwnerHud_R8_Index";
                indexTexture.filterMode = FilterMode.Point;
                indexTexture.wrapMode = TextureWrapMode.Clamp;
                indexTexture.anisoLevel = 0;

                provenanceTexture = new Texture2D(frame.Width, frame.Height, TextureFormat.R8, false, true);
                provenanceTexture.name = "EnhancedOwnerHud_R8_Provenance";
                provenanceTexture.filterMode = FilterMode.Point;
                provenanceTexture.wrapMode = TextureWrapMode.Clamp;
                provenanceTexture.anisoLevel = 0;

                paletteTexture = new Texture2D(256, 1, TextureFormat.RGBA32, false, true);
                paletteTexture.name = "EnhancedOwnerHud_OwnerPalette";
                paletteTexture.filterMode = FilterMode.Point;
                paletteTexture.wrapMode = TextureWrapMode.Clamp;
                paletteTexture.anisoLevel = 0;

                Shader shader = Resources.Load<Shader>(ShaderResourceName);
                if (shader == null)
                {
                    shader = Shader.Find("MagicCarpet/Enhanced/OwnerHudIndexed");
                }

                if (shader == null)
                {
                    reason = "Unity Enhanced owner HUD indexed shader is unavailable";
                    ReleaseSurface();
                    return false;
                }

                ownerHudMaterial = new Material(shader);
                ownerHudMaterial.name = "EnhancedOwnerHud_IndexedPalette_Material";
                ownerHudMaterial.hideFlags = HideFlags.DontSave;
                ownerHudMaterial.SetTexture("_IndexTex", indexTexture);
                ownerHudMaterial.SetTexture("_ProvenanceTex", provenanceTexture);
                ownerHudMaterial.SetTexture("_PaletteTex", paletteTexture);
                frameImage.texture = indexTexture;
                frameImage.material = ownerHudMaterial;
                aspectFitter.aspectRatio = (float)frame.Width / frame.Height;
                return true;
            }
            catch (Exception exception)
            {
                reason = "Unity Enhanced owner HUD surface setup failed: " + exception.Message;
                ReleaseSurface();
                return false;
            }
        }

        private void UploadFrame(UnityEnhancedOwnerHudFrame frame)
        {
            for (int sourceRow = 0; sourceRow < frame.Height; sourceRow++)
            {
                int destinationRow = frame.Height - sourceRow - 1;
                Buffer.BlockCopy(
                    frame.IndexedPixels,
                    sourceRow * frame.Stride,
                    indexUpload,
                    destinationRow * frame.Width,
                    frame.Width);
                Buffer.BlockCopy(
                    frame.Provenance,
                    sourceRow * frame.Stride,
                    provenanceUpload,
                    destinationRow * frame.Width,
                    frame.Width);
            }

            for (int index = 0; index < 256; index++)
            {
                int source = index * 3;
                int destination = index * 4;
                paletteUpload[destination] = frame.PaletteRgb8[source];
                paletteUpload[destination + 1] = frame.PaletteRgb8[source + 1];
                paletteUpload[destination + 2] = frame.PaletteRgb8[source + 2];
                paletteUpload[destination + 3] = 255;
            }

            indexTexture.SetPixelData(indexUpload, 0);
            indexTexture.Apply(false, false);
            provenanceTexture.SetPixelData(provenanceUpload, 0);
            provenanceTexture.Apply(false, false);
            paletteTexture.SetPixelData(paletteUpload, 0);
            paletteTexture.Apply(false, false);
        }

        private void EnsureHierarchy(Camera targetCamera)
        {
            if (canvas != null)
            {
                canvas.worldCamera = targetCamera;
                return;
            }

            GameObject canvasObject = new GameObject("EnhancedOwnerHudCanvas");
            canvasObject.transform.SetParent(transform, false);
            canvasObject.hideFlags = HideFlags.DontSave;
            canvas = canvasObject.AddComponent<Canvas>();
            canvas.renderMode = RenderMode.ScreenSpaceCamera;
            canvas.worldCamera = targetCamera;
            canvas.planeDistance = 1f;
            canvas.sortingOrder = 100;

            CanvasScaler scaler = canvasObject.AddComponent<CanvasScaler>();
            scaler.uiScaleMode = CanvasScaler.ScaleMode.ScaleWithScreenSize;
            scaler.referenceResolution = new Vector2(1280f, 720f);
            scaler.screenMatchMode = CanvasScaler.ScreenMatchMode.MatchWidthOrHeight;
            scaler.matchWidthOrHeight = 0.5f;

            RectTransform root = canvasObject.GetComponent<RectTransform>();
            if (root == null)
            {
                root = canvasObject.AddComponent<RectTransform>();
            }
            root.anchorMin = Vector2.zero;
            root.anchorMax = Vector2.one;
            root.offsetMin = Vector2.zero;
            root.offsetMax = Vector2.zero;

            GameObject frameObject = new GameObject("EnhancedOwnerHudFrame");
            frameObject.transform.SetParent(root, false);
            frameObject.hideFlags = HideFlags.DontSave;
            frameImage = frameObject.AddComponent<RawImage>();
            frameImage.raycastTarget = false;
            frameImage.color = Color.white;

            RectTransform frameRect = frameImage.rectTransform;
            frameRect.anchorMin = Vector2.zero;
            frameRect.anchorMax = Vector2.one;
            frameRect.offsetMin = Vector2.zero;
            frameRect.offsetMax = Vector2.zero;
            aspectFitter = frameObject.AddComponent<AspectRatioFitter>();
            aspectFitter.aspectMode = AspectRatioFitter.AspectMode.FitInParent;
            aspectFitter.aspectRatio = 4f / 3f;
        }

        private Rect AspectFitViewport(int targetWidth, int targetHeight)
        {
            float frameAspect = (float)frameWidth / frameHeight;
            int viewportWidth = targetWidth;
            int viewportHeight = Mathf.RoundToInt(viewportWidth / frameAspect);
            if (viewportHeight > targetHeight)
            {
                viewportHeight = targetHeight;
                viewportWidth = Mathf.RoundToInt(viewportHeight * frameAspect);
            }

            return new Rect(
                (targetWidth - viewportWidth) / 2,
                (targetHeight - viewportHeight) / 2,
                viewportWidth,
                viewportHeight);
        }

        private void OnDestroy()
        {
            ReleaseSurface();
        }

        private void ReleaseSurface()
        {
            DestroyUnityObject(ownerHudMaterial);
            DestroyUnityObject(paletteTexture);
            DestroyUnityObject(provenanceTexture);
            DestroyUnityObject(indexTexture);
            ownerHudMaterial = null;
            paletteTexture = null;
            provenanceTexture = null;
            indexTexture = null;
            indexUpload = null;
            provenanceUpload = null;
            paletteUpload = null;
            frameWidth = 0;
            frameHeight = 0;
            if (frameImage != null)
            {
                frameImage.texture = null;
                frameImage.material = null;
            }
        }

        private static void DestroyUnityObject(UnityEngine.Object unityObject)
        {
            if (unityObject == null)
            {
                return;
            }

            if (Application.isPlaying)
            {
                UnityEngine.Object.Destroy(unityObject);
            }
            else
            {
                UnityEngine.Object.DestroyImmediate(unityObject);
            }
        }
    }
}
