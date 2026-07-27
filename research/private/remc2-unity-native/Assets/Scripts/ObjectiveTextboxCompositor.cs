using System;
using UnityEngine;

namespace MagicCarpet.Native
{
    internal static class ObjectiveTextboxCompositor
    {
        internal static void ComposeOverlayIndices(
            int visibleTicks,
            byte[] worldTopDown,
            int worldWidth,
            int worldHeight,
            int referenceWidth,
            int referenceHeight,
            int referenceX,
            int referenceY,
            int width,
            int height,
            int blendX,
            int blendY,
            int blendWidth,
            int blendHeight,
            byte[] tables,
            int tableOffset,
            byte blendSelector,
            byte[] foregroundIndicesThenMask,
            byte[] destinationIndices,
            byte[] destinationMask)
        {
            if (
                worldTopDown == null ||
                tables == null ||
                foregroundIndicesThenMask == null ||
                destinationIndices == null ||
                destinationMask == null)
                throw new ArgumentNullException("objective indexed planes");
            if (
                worldWidth <= 0 ||
                worldHeight <= 0 ||
                referenceWidth <= 0 ||
                referenceHeight <= 0 ||
                width <= 0 ||
                height <= 0 ||
                worldTopDown.Length != worldWidth * worldHeight)
                throw new ArgumentException("objective world/reference extents");
            var planeBytes = checked(width * height);
            if (
                foregroundIndicesThenMask.Length != planeBytes * 2 ||
                destinationIndices.Length != planeBytes ||
                destinationMask.Length != planeBytes)
                throw new ArgumentException("objective foreground/output extents");
            if (
                referenceX < 0 ||
                referenceY < 0 ||
                referenceX + width > referenceWidth ||
                referenceY + height > referenceHeight ||
                blendX < referenceX ||
                blendY < referenceY ||
                blendWidth <= 0 ||
                blendHeight <= 0 ||
                blendX + blendWidth > referenceX + width ||
                blendY + blendHeight > referenceY + height)
                throw new ArgumentException("objective reference/blend geometry");
            var blendTableStart = checked(tableOffset + (blendSelector << 8));
            if (tableOffset < 0 || blendTableStart + 256 > tables.Length)
                throw new ArgumentException("objective TABLESN blend row");

            Array.Clear(destinationIndices, 0, destinationIndices.Length);
            Array.Clear(destinationMask, 0, destinationMask.Length);
            if (visibleTicks <= 0) return;

            for (var globalY = blendY; globalY < blendY + blendHeight; globalY++)
            {
                var sourceY = (int)(
                    ((2L * globalY + 1L) * worldHeight) /
                    (2L * referenceHeight));
                for (var globalX = blendX; globalX < blendX + blendWidth; globalX++)
                {
                    var sourceX = (int)(
                        ((2L * globalX + 1L) * worldWidth) /
                        (2L * referenceWidth));
                    var destination =
                        (globalY - referenceY) * width +
                        globalX - referenceX;
                    var source = sourceY * worldWidth + sourceX;
                    destinationIndices[destination] =
                        tables[blendTableStart + worldTopDown[source]];
                    destinationMask[destination] = 1;
                }
            }

            for (var pixel = 0; pixel < planeBytes; pixel++)
            {
                var foregroundMask = foregroundIndicesThenMask[planeBytes + pixel];
                if (foregroundMask == 0) continue;
                if (foregroundMask != 1)
                    throw new ArgumentException("objective foreground mask is not binary");
                destinationIndices[pixel] = foregroundIndicesThenMask[pixel];
                destinationMask[pixel] = 1;
            }
        }

        internal static void MaterializeBottomUpRgba(
            byte[] topDownIndices,
            byte[] topDownMask,
            int width,
            int height,
            byte[] paletteRgb8,
            Color32[] destinationBottomUp)
        {
            if (
                topDownIndices == null ||
                topDownMask == null ||
                paletteRgb8 == null ||
                destinationBottomUp == null)
                throw new ArgumentNullException("objective palette materialization");
            var pixels = checked(width * height);
            if (
                width <= 0 ||
                height <= 0 ||
                topDownIndices.Length != pixels ||
                topDownMask.Length != pixels ||
                paletteRgb8.Length != 256 * 3 ||
                destinationBottomUp.Length != pixels)
                throw new ArgumentException("objective palette materialization extents");

            for (var topDownY = 0; topDownY < height; topDownY++)
            {
                var bottomUpY = height - 1 - topDownY;
                for (var x = 0; x < width; x++)
                {
                    var source = topDownY * width + x;
                    var destination = bottomUpY * width + x;
                    if (topDownMask[source] == 0)
                    {
                        destinationBottomUp[destination] =
                            new Color32(0, 0, 0, 0);
                        continue;
                    }
                    var palette = topDownIndices[source] * 3;
                    destinationBottomUp[destination] = new Color32(
                        paletteRgb8[palette],
                        paletteRgb8[palette + 1],
                        paletteRgb8[palette + 2],
                        255);
                }
            }
        }
    }
}
