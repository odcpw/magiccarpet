using System;
using UnityEngine;

namespace MagicCarpet.PresentationLab
{
    public static class ClassicHdDisplayComposition
    {
        public static Rect CalculateViewport(int outputWidth, int outputHeight, int internalWidth, int internalHeight)
        {
            if (outputWidth <= 0 || outputHeight <= 0 || internalWidth <= 0 || internalHeight <= 0)
            {
                return new Rect(0f, 0f, 0f, 0f);
            }

            float scale = Mathf.Min((float)outputWidth / internalWidth, (float)outputHeight / internalHeight);
            int width = Mathf.Max(1, Mathf.FloorToInt(internalWidth * scale));
            int height = Mathf.Max(1, Mathf.FloorToInt(internalHeight * scale));
            int x = (outputWidth - width) / 2;
            int y = (outputHeight - height) / 2;
            return new Rect(x, y, width, height);
        }

        public static void ApplyTo(RectTransform rectTransform, Rect viewport)
        {
            if (rectTransform == null)
            {
                throw new ArgumentNullException("rectTransform");
            }

            rectTransform.anchorMin = Vector2.zero;
            rectTransform.anchorMax = Vector2.zero;
            rectTransform.pivot = Vector2.zero;
            rectTransform.anchoredPosition = Vector2.zero;
            rectTransform.offsetMin = new Vector2(viewport.xMin, viewport.yMin);
            rectTransform.offsetMax = new Vector2(viewport.xMax, viewport.yMax);
        }

        public static string Describe(Rect viewport, int internalWidth, int internalHeight)
        {
            return string.Format(
                "Classic HD display {0}x{1} at ({2},{3}) from {4}x{5} internal",
                Mathf.RoundToInt(viewport.width),
                Mathf.RoundToInt(viewport.height),
                Mathf.RoundToInt(viewport.x),
                Mathf.RoundToInt(viewport.y),
                internalWidth,
                internalHeight);
        }
    }
}
