using System;
using UnityEngine;
using UnityEngine.UI;

namespace MagicCarpet.PresentationLab
{
    [Serializable]
    public struct UnityEnhancedMinimapEntityCommand
    {
        public ushort SourceSlot;
        public short X;
        public short Y;
        public ushort Selector;
        public byte Kind;
        public byte PaletteIndex;
        public byte Size;
        public byte Anchor;
        public byte HelpSelector;
        public byte Flags;
        public ushort Order;
    }

    /// <summary>
    /// Immutable Enhanced presentation copy of the authoritative Level 1
    /// minimap ABI. Coordinates remain source 128x128 pixels; the consumer
    /// may beautify their display but may not reinterpret gameplay state.
    /// </summary>
    [Serializable]
    public sealed class UnityEnhancedMinimapSnapshot
    {
        public const int MaxMarks = 16;
        public const int ArrowCoordinateCount = 14;
        public const int MaxEntityCommands = 1152;
        public const byte Level1MissionAuthority = 6;
        public const byte DotCommand = 1;
        public const byte SpriteCommand = 2;
        public const byte BlendPixelCommand = 3;

        public ulong Generation;
        public ulong Tick;
        public byte MarkCount;
        public bool MarkPulse;
        public bool ArrowValid;
        public bool ArrowVisible;
        public short[] MarkX = new short[MaxMarks];
        public short[] MarkY = new short[MaxMarks];
        public byte[] MarkColour = new byte[MaxMarks];
        public byte[] MarkHelpSelector = new byte[MaxMarks];
        public short[] ArrowXY = new short[ArrowCoordinateCount];
        public uint TotalEntityCount;
        public ushort CrossX;
        public ushort CrossY;
        public byte CrossColour;
        public byte CrossArmLength;
        public bool RevealActive;
        public byte Authority;
        public ulong CommandHash;
        public UnityEnhancedMinimapEntityCommand[] EntityCommands =
            Array.Empty<UnityEnhancedMinimapEntityCommand>();

        public bool TryValidate(ulong expectedGeneration, ulong expectedTick, out string reason)
        {
            if (Generation == 0 || Generation != expectedGeneration || Tick != expectedTick)
            {
                reason = "minimap generation or tick does not match the world snapshot";
                return false;
            }

            if (Authority != Level1MissionAuthority || MarkCount > MaxMarks ||
                MarkX == null || MarkX.Length != MaxMarks ||
                MarkY == null || MarkY.Length != MaxMarks ||
                MarkColour == null || MarkColour.Length != MaxMarks ||
                MarkHelpSelector == null || MarkHelpSelector.Length != MaxMarks ||
                ArrowXY == null || ArrowXY.Length != ArrowCoordinateCount ||
                EntityCommands == null || TotalEntityCount > (uint)MaxEntityCommands ||
                EntityCommands.Length != (int)TotalEntityCount)
            {
                reason = "minimap extent, authority, or fixed-array contract is invalid";
                return false;
            }

            ushort previousOrder = 0;
            for (int index = 0; index < EntityCommands.Length; index++)
            {
                UnityEnhancedMinimapEntityCommand command = EntityCommands[index];
                if (command.SourceSlot == 0 || command.Kind < DotCommand ||
                    command.Kind > BlendPixelCommand || command.Order != index ||
                    (index > 0 && command.Order <= previousOrder))
                {
                    reason = "minimap command stream is not source ordered or has an invalid kind";
                    return false;
                }

                previousOrder = command.Order;
            }

            reason = string.Empty;
            return true;
        }
    }

    /// <summary>
    /// Asset-free Enhanced minimap. The source stream is rasterized into a
    /// small point-filtered texture so the modern HUD stays readable at HD
    /// and high-resolution output without introducing an owner asset.
    /// </summary>
    [DisallowMultipleComponent]
    public sealed class UnityEnhancedMinimapPresentation : MonoBehaviour
    {
        private const int MapSize = 128;
        private Canvas canvas;
        private RawImage panel;
        private RawImage mapImage;
        private Text titleText;
        private Text countText;
        private Texture2D mapTexture;
        private Color32[] pixels;
        private ulong generation;
        private ulong lastTick;
        private int renderedMarkCount;
        private int renderedCommandCount;
        private ulong commandHash;

        public ulong Generation { get { return generation; } }
        public ulong LastTick { get { return lastTick; } }
        public int RenderedMarkCount { get { return renderedMarkCount; } }
        public int RenderedCommandCount { get { return renderedCommandCount; } }
        public ulong CommandHash { get { return commandHash; } }
        public bool IsVisible { get { return canvas != null && canvas.enabled; } }
        public Texture2D Texture { get { return mapTexture; } }

        public bool ApplySnapshot(UnityEnhancedMinimapSnapshot snapshot, Camera targetCamera)
        {
            string reason;
            if (snapshot == null || targetCamera == null ||
                !snapshot.TryValidate(snapshot.Generation, snapshot.Tick, out reason))
            {
                return false;
            }

            if (generation == snapshot.Generation && snapshot.Tick < lastTick)
            {
                return false;
            }

            EnsureHierarchy(targetCamera);
            generation = snapshot.Generation;
            lastTick = snapshot.Tick;
            commandHash = snapshot.CommandHash;
            renderedMarkCount = snapshot.MarkCount;
            renderedCommandCount = snapshot.EntityCommands.Length;
            canvas.enabled = true;
            titleText.text = "MINIMAP  //  LEVEL 1";
            countText.text = "MARKS " + renderedMarkCount +
                             "   ENTITIES " + renderedCommandCount;
            RenderSnapshot(snapshot);
            return true;
        }

        public void ResetForGeneration(ulong nextGeneration)
        {
            generation = nextGeneration;
            lastTick = 0;
            renderedMarkCount = 0;
            renderedCommandCount = 0;
            commandHash = 0;
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

        private void OnDestroy()
        {
            if (mapTexture == null)
            {
                return;
            }

            if (Application.isPlaying)
            {
                Destroy(mapTexture);
            }
            else
            {
                DestroyImmediate(mapTexture);
            }

            mapTexture = null;
        }

        private void EnsureHierarchy(Camera targetCamera)
        {
            if (canvas != null)
            {
                canvas.worldCamera = targetCamera;
                return;
            }

            GameObject canvasObject = new GameObject("EnhancedMinimapCanvas");
            canvasObject.transform.SetParent(transform, false);
            canvasObject.hideFlags = HideFlags.DontSave;
            canvas = canvasObject.AddComponent<Canvas>();
            canvas.renderMode = RenderMode.ScreenSpaceCamera;
            canvas.worldCamera = targetCamera;
            canvas.planeDistance = 1f;
            canvas.sortingOrder = 101;

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

            panel = CreateImage(root, "EnhancedMinimapPanel", new Color(0.015f, 0.04f, 0.10f, 0.90f));
            ConfigureBox(panel.rectTransform, new Vector2(1f, 1f), new Vector2(1f, 1f),
                new Vector2(-26f, -58f), new Vector2(220f, 220f));

            titleText = CreateText(panel.rectTransform, "EnhancedMinimapTitle", 13, FontStyle.Bold);
            ConfigureBox(titleText.rectTransform, new Vector2(0f, 1f), new Vector2(0f, 1f),
                new Vector2(12f, -10f), new Vector2(196f, 22f));

            mapImage = CreateImage(panel.rectTransform, "EnhancedMinimapMap", Color.white);
            ConfigureBox(mapImage.rectTransform, new Vector2(0f, 0f), new Vector2(0f, 0f),
                new Vector2(16f, 24f), new Vector2(188f, 168f));
            countText = CreateText(panel.rectTransform, "EnhancedMinimapCount", 10, FontStyle.Normal);
            ConfigureBox(countText.rectTransform, new Vector2(0f, 0f), new Vector2(0f, 0f),
                new Vector2(12f, 5f), new Vector2(196f, 16f));

            mapTexture = new Texture2D(MapSize, MapSize, TextureFormat.RGBA32, false, true);
            mapTexture.name = "EnhancedGeneratedMinimap";
            mapTexture.filterMode = FilterMode.Point;
            mapTexture.wrapMode = TextureWrapMode.Clamp;
            mapTexture.hideFlags = HideFlags.DontSave;
            pixels = new Color32[MapSize * MapSize];
            mapImage.texture = mapTexture;
        }

        private void RenderSnapshot(UnityEnhancedMinimapSnapshot snapshot)
        {
            Color32 background = new Color32(4, 11, 25, 238);
            for (int index = 0; index < pixels.Length; index++)
            {
                pixels[index] = background;
            }

            DrawBorder(new Color32(44, 86, 124, 255));
            for (int index = 0; index < snapshot.EntityCommands.Length; index++)
            {
                DrawCommand(snapshot.EntityCommands[index]);
            }

            if (snapshot.ArrowValid && snapshot.ArrowVisible)
            {
                for (int index = 0; index + 3 < snapshot.ArrowXY.Length; index += 2)
                {
                    DrawLine(
                        snapshot.ArrowXY[index],
                        snapshot.ArrowXY[index + 1],
                        snapshot.ArrowXY[index + 2],
                        snapshot.ArrowXY[index + 3],
                        new Color32(116, 224, 255, 255));
                }
            }

            for (int index = 0; index < snapshot.MarkCount; index++)
            {
                int radius = snapshot.MarkPulse ? 3 : 2;
                DrawDiamond(snapshot.MarkX[index], snapshot.MarkY[index], radius,
                    SourcePaletteColor(snapshot.MarkColour[index]));
            }

            DrawCross(snapshot.CrossX, snapshot.CrossY, snapshot.CrossArmLength,
                SourcePaletteColor(snapshot.CrossColour));
            mapTexture.SetPixels32(pixels);
            mapTexture.Apply(false, false);
        }

        private void DrawCommand(UnityEnhancedMinimapEntityCommand command)
        {
            Color32 color = SourcePaletteColor(command.PaletteIndex);
            if (command.Kind == UnityEnhancedMinimapSnapshot.BlendPixelCommand)
            {
                DrawPixel(command.X, command.Y, color);
                return;
            }

            int radius = command.Kind == UnityEnhancedMinimapSnapshot.SpriteCommand
                ? Mathf.Clamp(command.Size, 1, 3)
                : Mathf.Clamp(command.Size, 1, 2);
            DrawDiamond(command.X, command.Y, radius, color);
        }

        private void DrawCross(int sourceX, int sourceY, int armLength, Color32 color)
        {
            if (armLength <= 0)
            {
                armLength = 4;
            }

            DrawLine(sourceX - armLength, sourceY, sourceX + armLength, sourceY, color);
            DrawLine(sourceX, sourceY - armLength, sourceX, sourceY + armLength, color);
        }

        private void DrawDiamond(int sourceX, int sourceY, int radius, Color32 color)
        {
            for (int deltaY = -radius; deltaY <= radius; deltaY++)
            {
                int width = radius - Mathf.Abs(deltaY);
                for (int deltaX = -width; deltaX <= width; deltaX++)
                {
                    DrawPixel(sourceX + deltaX, sourceY + deltaY, color);
                }
            }
        }

        private void DrawBorder(Color32 color)
        {
            for (int index = 0; index < MapSize; index++)
            {
                DrawPixel(index, 0, color);
                DrawPixel(index, MapSize - 1, color);
                DrawPixel(0, index, color);
                DrawPixel(MapSize - 1, index, color);
            }
        }

        private void DrawLine(int sourceX0, int sourceY0, int sourceX1, int sourceY1, Color32 color)
        {
            int deltaX = Mathf.Abs(sourceX1 - sourceX0);
            int stepX = sourceX0 < sourceX1 ? 1 : -1;
            int deltaY = -Mathf.Abs(sourceY1 - sourceY0);
            int stepY = sourceY0 < sourceY1 ? 1 : -1;
            int error = deltaX + deltaY;
            int sourceX = sourceX0;
            int sourceY = sourceY0;
            while (true)
            {
                DrawPixel(sourceX, sourceY, color);
                if (sourceX == sourceX1 && sourceY == sourceY1)
                {
                    return;
                }

                int doubledError = 2 * error;
                if (doubledError >= deltaY)
                {
                    error += deltaY;
                    sourceX += stepX;
                }

                if (doubledError <= deltaX)
                {
                    error += deltaX;
                    sourceY += stepY;
                }
            }
        }

        private void DrawPixel(int sourceX, int sourceY, Color32 color)
        {
            if (sourceX < 0 || sourceX >= MapSize || sourceY < 0 || sourceY >= MapSize)
            {
                return;
            }

            int textureY = MapSize - 1 - sourceY;
            pixels[textureY * MapSize + sourceX] = color;
        }

        private static Color32 SourcePaletteColor(byte paletteIndex)
        {
            switch (paletteIndex)
            {
                case 23: return new Color32(100, 224, 124, 255);
                case 58: return new Color32(104, 196, 255, 255);
                case 119: return new Color32(255, 92, 92, 255);
                case 164: return new Color32(86, 174, 255, 255);
                case 170: return new Color32(164, 116, 255, 255);
                case 176: return new Color32(255, 224, 116, 255);
                case 177: return new Color32(255, 168, 104, 255);
                case 184: return new Color32(255, 238, 136, 255);
                case 215: return new Color32(255, 132, 92, 255);
                case 232: return new Color32(196, 154, 255, 255);
                default:
                    float hue = paletteIndex / 255f;
                    Color color = Color.HSVToRGB(hue, 0.62f, 1f);
                    return new Color32(
                        (byte)Mathf.RoundToInt(color.r * 255f),
                        (byte)Mathf.RoundToInt(color.g * 255f),
                        (byte)Mathf.RoundToInt(color.b * 255f),
                        255);
            }
        }

        private static RawImage CreateImage(Transform parent, string name, Color color)
        {
            GameObject imageObject = new GameObject(name);
            imageObject.transform.SetParent(parent, false);
            imageObject.hideFlags = HideFlags.DontSave;
            RawImage image = imageObject.AddComponent<RawImage>();
            image.texture = Texture2D.whiteTexture;
            image.color = color;
            image.raycastTarget = false;
            return image;
        }

        private static Text CreateText(Transform parent, string name, int size, FontStyle style)
        {
            GameObject textObject = new GameObject(name);
            textObject.transform.SetParent(parent, false);
            textObject.hideFlags = HideFlags.DontSave;
            Text text = textObject.AddComponent<Text>();
            text.font = Resources.GetBuiltinResource<Font>("LegacyRuntime.ttf");
            text.fontSize = size;
            text.fontStyle = style;
            text.color = new Color(0.88f, 0.95f, 1f, 1f);
            text.alignment = TextAnchor.MiddleLeft;
            text.raycastTarget = false;
            return text;
        }

        private static void ConfigureBox(
            RectTransform rect,
            Vector2 anchor,
            Vector2 pivot,
            Vector2 position,
            Vector2 size)
        {
            rect.anchorMin = anchor;
            rect.anchorMax = anchor;
            rect.pivot = pivot;
            rect.anchoredPosition = position;
            rect.sizeDelta = size;
        }
    }
}
