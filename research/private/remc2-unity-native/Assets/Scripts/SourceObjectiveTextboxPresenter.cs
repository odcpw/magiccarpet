using System;
using UnityEngine;

namespace MagicCarpet.Native
{
    // Owns presentation resources only; presenting a frame never advances the session.
    internal sealed class SourceObjectiveTextboxPresenter : IDisposable
    {
        private readonly OwnerCampaignMenuCache.ObjectiveAssets assets;
        private readonly Material material;
        private readonly Texture2D blendRow;
        private readonly Texture2D palette;
        private Texture2D foreground;
        private RenderTexture output;
        private Texture2D priorOverlay;
        private RenderTexture priorOutput;
        private RectInt priorRect;
        private ulong priorGeneration;
        private ObjectiveTextboxCompositor.Raster raster;
        private ulong generation;
        private string currentText;
        private int currentAnchor = -1;

        internal SourceObjectiveTextboxPresenter(OwnerCampaignMenuCache.ObjectiveAssets assets)
        {
            this.assets = assets ?? throw new ArgumentNullException(nameof(assets));
            var row = checked(assets.TableOffset + (assets.BlendSelector << 8));
            if (assets.Tables == null || row < 0 || row + 256 > assets.Tables.Length)
                throw new ArgumentException("objective blend table extent");
            var shader = Resources.Load<Shader>("SourceObjectiveTextbox");
            if (shader == null) throw new InvalidOperationException("missing source objective textbox shader");
            material = new Material(shader);
            blendRow = new Texture2D(256, 1, TextureFormat.R8, false, true)
                { filterMode = FilterMode.Point, wrapMode = TextureWrapMode.Clamp };
            var bytes = new byte[256];
            Buffer.BlockCopy(assets.Tables, row, bytes, 0, bytes.Length);
            blendRow.LoadRawTextureData(bytes);
            blendRow.Apply(false, false);
            palette = new Texture2D(256, 1, TextureFormat.RGB24, false, false)
                { filterMode = FilterMode.Point, wrapMode = TextureWrapMode.Clamp };
            material.SetTexture("_BlendRow", blendRow);
            material.SetTexture("_Palette", palette);
            material.SetFloat("_PriorEnabled", 0);
            material.SetFloat("_RenderPrior", 0);
        }

        internal void Present(string sourceText, ulong sessionGeneration)
        {
            if (raster != null && generation == sessionGeneration && currentText == sourceText && currentAnchor == -1) return;
            var bytes = SourceBytes(sourceText);
            var next = ObjectiveTextboxCompositor.BuildSourceForeground(bytes, assets.Glyphs,
                assets.GlyphMasks, assets.Corner, assets.Side, assets.Horizontal, assets.TextColor);
            PresentRaster(next, sourceText, sessionGeneration, -1);
        }

        internal void PresentHint(string sourceText, ulong sessionGeneration, int anchorX)
        {
            if (raster != null && generation == sessionGeneration && currentText == sourceText && currentAnchor == anchorX) return;
            var bytes = SourceBytes(sourceText);
            var next = ObjectiveTextboxCompositor.BuildSourceHintForeground(bytes, assets.Glyphs,
                assets.GlyphMasks, assets.Corner, assets.Side, assets.Horizontal, assets.HelpTextColor,
                assets.BlendSelector, assets.TextColor, anchorX, 48);
            PresentRaster(next, sourceText, sessionGeneration, anchorX);
        }

        internal void PresentContextHint(string sourceText, ulong sessionGeneration, int centerX, int centerY)
        {
            if (raster != null && generation == sessionGeneration && currentText == sourceText && currentAnchor == -2) return;
            var bytes = SourceBytes(sourceText);
            var next = ObjectiveTextboxCompositor.BuildSourceContextHintForeground(bytes, assets.Glyphs,
                assets.GlyphMasks, assets.Corner, assets.Side, assets.Horizontal, assets.HelpTextColor, centerX, centerY);
            PresentRaster(next, sourceText, sessionGeneration, -2);
        }

        internal void PresentPauseHint(string sourceText, ulong sessionGeneration, int anchorX, int anchorY)
        {
            // The Y coordinate also participates in the cache key.
            var key = checked(anchorY * 640 + anchorX + 1);
            if (raster != null && generation == sessionGeneration && currentText == sourceText && currentAnchor == -key - 2) return;
            var next = ObjectiveTextboxCompositor.BuildSourcePauseHintForeground(SourceBytes(sourceText), assets.Glyphs,
                assets.GlyphMasks, assets.Corner, assets.Side, assets.Horizontal, assets.HelpTextColor,
                assets.BlendSelector, assets.TextColor, anchorX, anchorY);
            PresentRaster(next, sourceText, sessionGeneration, -key - 2);
        }

        // Red is the source sprite index, alpha its opacity, and green marks
        // the source's twice-shaded rectangle independently of sprite opacity.
        internal void SetPausePrior(RectInt rect, Color32[] pixels, ulong sessionGeneration)
        {
            if (rect.x < 0 || rect.y < 0 || rect.width <= 0 || rect.height <= 0 ||
                rect.xMax > 640 || rect.yMax > 480 || pixels == null || pixels.Length != rect.width * rect.height)
                throw new ArgumentException("source pause prior extent");
            var next = new Texture2D(rect.width, rect.height, TextureFormat.RGBA32, false, true)
                { filterMode = FilterMode.Point, wrapMode = TextureWrapMode.Clamp };
            var bottomUp = new Color32[pixels.Length];
            for (var y = 0; y < rect.height; y++)
                Array.Copy(pixels, y * rect.width, bottomUp, (rect.height - 1 - y) * rect.width, rect.width);
            next.SetPixels32(bottomUp);
            next.Apply(false, false);
            ReleaseTexture(priorOverlay); ReleaseTexture(priorOutput);
            priorOutput = null; priorOverlay = next; priorRect = rect; priorGeneration = sessionGeneration;
            material.SetTexture("_PriorOverlay", priorOverlay);
            material.SetVector("_PriorRect", new Vector4(rect.x, rect.y, rect.width, rect.height));
        }

        internal RenderTexture RenderPausePrior(Texture sceneIndices, ulong sceneGeneration,
            byte[] currentPaletteRgb8, bool sceneTopDown = false)
        {
            if (priorOverlay == null || sceneIndices == null || priorGeneration != sceneGeneration) return null;
            if (currentPaletteRgb8 == null || currentPaletteRgb8.Length != 256 * 3)
                throw new ArgumentException("source pause palette extent");
            palette.LoadRawTextureData(currentPaletteRgb8); palette.Apply(false, false);
            if (priorOutput == null)
            {
                priorOutput = new RenderTexture(priorRect.width, priorRect.height, 0, RenderTextureFormat.ARGB32,
                    RenderTextureReadWrite.Linear) { filterMode = FilterMode.Point, wrapMode = TextureWrapMode.Clamp };
                priorOutput.Create();
            }
            material.SetTexture("_SceneIndices", sceneIndices);
            material.SetFloat("_SceneTopDown", sceneTopDown ? 1f : 0f);
            material.SetVector("_Raster", new Vector4(priorRect.x, priorRect.y, priorRect.width, priorRect.height));
            material.SetFloat("_PriorEnabled", 1); material.SetFloat("_RenderPrior", 1);
            var callerTarget = RenderTexture.active;
            try { Graphics.Blit(null, priorOutput, material); }
            finally
            {
                RenderTexture.active = callerTarget;
                material.SetFloat("_RenderPrior", 0);
                if (raster != null) material.SetVector("_Raster", new Vector4(raster.X, raster.Y, raster.Width, raster.Height));
            }
            return priorOutput;
        }

        internal void DrawPausePrior(Texture sceneIndices, ulong sceneGeneration, Rect view,
            byte[] currentPaletteRgb8, bool sceneTopDown = false)
        {
            if (Event.current == null || Event.current.type != EventType.Repaint) return;
            var rendered = RenderPausePrior(sceneIndices, sceneGeneration, currentPaletteRgb8, sceneTopDown);
            if (rendered == null) return;
            GUI.DrawTexture(new Rect(view.x + view.width * priorRect.x / 640f,
                view.y + view.height * priorRect.y / 480f, view.width * priorRect.width / 640f,
                view.height * priorRect.height / 480f), rendered, ScaleMode.StretchToFill, true);
        }

        private static byte[] SourceBytes(string sourceText)
        {
            if (sourceText == null) throw new ArgumentNullException(nameof(sourceText));
            var bytes = new byte[sourceText.Length];
            for (var i = 0; i < bytes.Length; i++)
            {
                if (sourceText[i] > byte.MaxValue) throw new ArgumentException("objective text is not source bytes");
                bytes[i] = (byte)sourceText[i];
            }
            return bytes;
        }

        private void PresentRaster(ObjectiveTextboxCompositor.Raster next, string sourceText,
            ulong sessionGeneration, int anchorX)
        {
            var nextForeground = new Texture2D(next.Width, next.Height, TextureFormat.RGBA32, false, true)
                { filterMode = FilterMode.Point, wrapMode = TextureWrapMode.Clamp };
            var colors = new Color32[next.Width * next.Height];
            var count = colors.Length;
            for (var y = 0; y < next.Height; y++)
            for (var x = 0; x < next.Width; x++)
            {
                var source = y * next.Width + x;
                colors[(next.Height - 1 - y) * next.Width + x] = new Color32(
                    next.IndicesThenMask[source], 0, 0, next.IndicesThenMask[count + source] == 0 ? (byte)0 : (byte)255);
            }
            nextForeground.SetPixels32(colors);
            nextForeground.Apply(false, false);
            ReleaseTexture(foreground);
            ReleaseTexture(output);
            output = null;
            foreground = nextForeground;
            raster = next;
            generation = sessionGeneration;
            currentText = sourceText;
            currentAnchor = anchorX;
            material.SetTexture("_Foreground", foreground);
            material.SetVector("_Raster", new Vector4(next.X, next.Y, next.Width, next.Height));
            material.SetVector("_BlendRect", new Vector4(next.BlendX - next.X, next.BlendY - next.Y,
                next.BlendWidth, next.BlendHeight));
        }

        internal RenderTexture Render(int visibleTicks, Texture sceneIndices, ulong sceneGeneration,
            ulong sceneRevision, byte[] currentPaletteRgb8, bool sceneTopDown = false)
        {
            if (visibleTicks <= 0 || raster == null || sceneIndices == null || generation != sceneGeneration)
                return null;
            if (currentPaletteRgb8 == null || currentPaletteRgb8.Length != 256 * 3)
                throw new ArgumentException("objective palette extent");
            palette.LoadRawTextureData(currentPaletteRgb8);
            palette.Apply(false, false);
            if (output == null)
            {
                output = new RenderTexture(raster.Width, raster.Height, 0, RenderTextureFormat.ARGB32,
                    RenderTextureReadWrite.Linear) { filterMode = FilterMode.Point, wrapMode = TextureWrapMode.Clamp };
                output.Create();
            }
            material.SetTexture("_SceneIndices", sceneIndices);
            material.SetFloat("_SceneTopDown", sceneTopDown ? 1f : 0f);
            material.SetFloat("_PriorEnabled", priorOverlay != null && priorGeneration == sceneGeneration ? 1f : 0f);
            material.SetFloat("_RenderPrior", 0);
            // Render from the current source even for a valid map revision of zero.
            var callerTarget = RenderTexture.active;
            try { Graphics.Blit(null, output, material); }
            finally { RenderTexture.active = callerTarget; }
            return output;
        }

        internal void Draw(int visibleTicks, Texture sceneIndices, ulong sceneGeneration,
            ulong sceneRevision, Rect view, byte[] currentPaletteRgb8, bool sceneTopDown = false)
        {
            if (Event.current == null || Event.current.type != EventType.Repaint) return;
            var rendered = Render(visibleTicks, sceneIndices, sceneGeneration, sceneRevision, currentPaletteRgb8, sceneTopDown);
            if (rendered == null) return;
            GUI.DrawTexture(new Rect(view.x + view.width * raster.X / 640f,
                view.y + view.height * raster.Y / 480f, view.width * raster.Width / 640f,
                view.height * raster.Height / 480f), rendered, ScaleMode.StretchToFill, true);
        }

        public void Dispose()
        {
            ReleaseTexture(output); ReleaseTexture(foreground); ReleaseTexture(blendRow);
            ReleaseTexture(priorOutput); ReleaseTexture(priorOverlay);
            ReleaseTexture(palette); ReleaseTexture(material);
            output = null; foreground = null; raster = null; currentText = null; currentAnchor = -1;
        }

        private static void ReleaseTexture(UnityEngine.Object value)
        {
            if (value == null) return;
            if (Application.isPlaying) UnityEngine.Object.Destroy(value);
            else UnityEngine.Object.DestroyImmediate(value);
        }
    }
}
