using System;
using System.Collections.Generic;
using UnityEngine;

namespace MagicCarpet.Native
{
    internal static class ObjectiveTextboxCompositor
    {
        internal readonly struct Glyph
        {
            internal readonly int Offset, Length, Width, Height;
            internal Glyph(int offset, int length, int width, int height)
            { Offset = offset; Length = length; Width = width; Height = height; }
        }

        internal sealed class Sprite
        {
            internal readonly int Width, Height;
            internal readonly byte[] Indices, Mask;
            internal Sprite(int width, int height, byte[] indices, byte[] mask)
            { Width = width; Height = height; Indices = indices; Mask = mask; }
        }

        internal sealed class Raster
        {
            internal readonly int X, Y, Width, Height, BlendX, BlendY, BlendWidth, BlendHeight;
            internal readonly byte[] IndicesThenMask;
            internal Raster(int x, int y, int width, int height, int blendX, int blendY,
                int blendWidth, int blendHeight, byte[] indicesThenMask)
            {
                X = x; Y = y; Width = width; Height = height;
                BlendX = blendX; BlendY = blendY; BlendWidth = blendWidth; BlendHeight = blendHeight;
                IndicesThenMask = indicesThenMask;
            }
        }

        // GameUI.cpp ComputeTextboxSizesFromTextWords_89420 through
        // DrawTextboxFrame_89690, at the original 640x480 high-resolution
        // boundary. All input text is retained source bytes, not Unicode copy.
        internal static Raster BuildSourceForeground(byte[] text, Glyph[] glyphs, byte[] glyphMasks,
            Sprite corner, Sprite side, Sprite horizontal, byte textColor)
            => BuildSourceForeground(text, glyphs, glyphMasks, corner, side, horizontal,
                textColor, 320, 320, 300, false, 0, 0, 0, 0);

        // DrawPopupTextBox_87610 type 2, using SetTextBoxMinMaxSizes_87090
        // and ComputeTextboxSizes_89830 at the original 640x480 boundary.
        internal static Raster BuildSourceHintForeground(byte[] text, Glyph[] glyphs, byte[] glyphMasks,
            Sprite corner, Sprite side, Sprite horizontal, byte textColor, byte lineColor,
            byte lineHighlight, int anchorX, int anchorY)
        {
            if ((anchorX != 540 && anchorX != 600) || anchorY != 48)
                throw new ArgumentException("source hint 41 anchor is invalid");
            var centerX = 600 < anchorX + 135 ? anchorX - 135 : anchorX + 25;
            var centerY = anchorY - 94 + 28 < 80 ? anchorY + 66 : anchorY - 94;
            return BuildSourceForeground(text, glyphs, glyphMasks, corner, side, horizontal,
                textColor, 220, centerX, centerY, true, anchorX, anchorY, lineColor, lineHighlight);
        }

        private static Raster BuildSourceForeground(byte[] text, Glyph[] glyphs, byte[] glyphMasks,
            Sprite corner, Sprite side, Sprite horizontal, byte textColor, int maxWidth,
            int centerX, int centerY, bool pointer, int anchorX, int anchorY,
            byte lineColor, byte lineHighlight)
        {
            if (text == null || glyphs == null || glyphMasks == null || corner == null || side == null || horizontal == null)
                throw new ArgumentNullException("source objective resources");
            if (text.Length == 0 || text.Length > 4096 || Array.IndexOf(text, (byte)0) >= 0 || glyphs.Length < 257)
                throw new ArgumentException("source objective text/glyph extent");
            ValidateSprite(corner); ValidateSprite(side); ValidateSprite(horizontal);
            const int charWidth = 8, charHeight = 14;
            var textWidth = checked(charWidth * text.Length);
            var boxWidth = maxWidth;
            if (textWidth <= boxWidth) boxWidth = textWidth;
            else if (textWidth % boxWidth < boxWidth / 2)
            {
                while (boxWidth > 0 && textWidth % boxWidth < boxWidth / 2)
                    boxWidth -= charWidth;
                if (boxWidth <= 0) throw new ArgumentException("source objective width collapsed");
            }
            var lines = SplitSourceLines(text, boxWidth / charWidth);
            if (lines.Count == 0) throw new ArgumentException("source objective has no drawable text");
            // ComputeTextboxSizesFromTextWords uses ceil(width/charWidth)
            // while DrawTextboxText uses floor(width/charWidth).
            var layoutLines = textWidth <= maxWidth ? 1 :
                SplitSourceLines(text, (boxWidth + charWidth - 1) / charWidth).Count;
            var boxHeight = checked(charHeight * layoutLines);
            var textX = centerX - boxWidth / 2;
            var textY = centerY - boxHeight / 2;
            if (textX < 40 + corner.Width) textX = 40 + corner.Width;
            if (textY < 80 + corner.Height) textY = 80 + corner.Height;
            if (textX + boxWidth >= 600) textX = 600 - boxWidth;
            if (textY + boxHeight >= 260) textY = 260 - boxHeight;
            var frameWidth = horizontal.Width * ((horizontal.Width - 1 + boxWidth + 8) / horizontal.Width);
            var frameX = textX - (frameWidth - boxWidth) / 2;
            var frameHeight = (side.Height - 1 + boxHeight) / side.Height * side.Height +
                2 * (corner.Height - horizontal.Height);
            var frameY = textY - (frameHeight - boxHeight) / 2;
            var x = frameX - side.Width;
            var y = frameY - horizontal.Height;
            var right = checked(frameX + frameWidth + side.Width);
            var bottom = checked(frameY + frameHeight + horizontal.Height);
            var lineX = 0; var lineY = 0;
            if (pointer)
            {
                var constrainedCenterX = textX + boxWidth / 2;
                var constrainedCenterY = textY + boxHeight / 2;
                lineX = anchorX < frameX - 30 ? frameX - 2 :
                    anchorX > frameX + frameWidth + 30 ? frameX + frameWidth : constrainedCenterX;
                lineY = anchorY < frameY - 30 ? frameY - 2 :
                    anchorY > frameY + frameHeight + 30 ? frameY + frameHeight : constrainedCenterY;
                if (lineX != constrainedCenterX || lineY != constrainedCenterY)
                {
                    x = Math.Min(x, Math.Min(anchorX, lineX));
                    y = Math.Min(y, Math.Min(anchorY, lineY));
                    right = Math.Max(right, Math.Max(anchorX, lineX) + 2);
                    bottom = Math.Max(bottom, Math.Max(anchorY, lineY) + 1);
                }
            }
            var width = checked(right - x);
            var height = checked(bottom - y);
            if (x < 0 || y < 0 || width <= 0 || height <= 0 || x + width > 640 || y + height > 480)
                throw new ArgumentException("source objective frame outside reference viewport");
            var pixels = checked(width * height);
            var foreground = new byte[checked(pixels * 2)];
            var textLocalX = textX - x;
            var textLocalY = textY - y;
            for (var line = 0; line < lines.Count; line++)
            {
                var cursorX = textLocalX;
                var cursorY = textLocalY + line * charHeight;
                foreach (var value in lines[line])
                {
                    if (value == 13) continue;
                    if (value == 10) { cursorX = textLocalX; cursorY += charHeight; continue; }
                    var glyph = glyphs[value == 32 || value == 9 ? 33 : value + 1];
                    ValidateGlyph(glyph, glyphMasks);
                    if (value != 32 && value != 9)
                        DrawGlyph(foreground, width, height, glyph, glyphMasks, cursorX, cursorY, textColor);
                    cursorX += glyph.Width;
                }
            }
            if (pointer && (lineX != textX + boxWidth / 2 || lineY != textY + boxHeight / 2))
            {
                DrawSourceLine(foreground, width, height, anchorX - x, anchorY - y, lineX - x, lineY - y, lineColor);
                DrawSourceLine(foreground, width, height, anchorX + 1 - x, anchorY - y, lineX + 1 - x, lineY - y, lineHighlight);
            }
            var localBlendX = frameX - x;
            var localBlendY = frameY - y;
            var frameEndX = localBlendX + frameWidth;
            var frameTopY = localBlendY - horizontal.Height;
            var frameBottomY = localBlendY + frameHeight;
            foreach (var sideX in new[] { localBlendX - side.Width, frameEndX })
                DrawSprite(foreground, width, height, corner, sideX, frameTopY);
            var sideY = frameTopY + corner.Height;
            for (var line = 0; line < boxHeight / charHeight; line++)
            {
                foreach (var sideX in new[] { localBlendX - side.Width, frameEndX })
                    DrawSprite(foreground, width, height, side, sideX, sideY);
                sideY += side.Height;
            }
            foreach (var sideX in new[] { localBlendX - side.Width, frameEndX })
                DrawSprite(foreground, width, height, corner, sideX, sideY);
            for (var tileX = localBlendX; tileX < frameEndX; tileX += horizontal.Width)
            {
                DrawSprite(foreground, width, height, horizontal, tileX, frameTopY);
                DrawSprite(foreground, width, height, horizontal, tileX, frameBottomY);
            }
            return new Raster(x, y, width, height, frameX, frameY, frameWidth, frameHeight, foreground);
        }

        // DrawLine_BD542: source swaps endpoints to ascend Y and uses a
        // doubled integer error, with both endpoints included.
        private static void DrawSourceLine(byte[] output, int width, int height,
            int x0, int y0, int x1, int y1, byte color)
        {
            if (y0 > y1) { (x0, x1) = (x1, x0); (y0, y1) = (y1, y0); }
            var dx = Math.Abs(x1 - x0); var dy = y1 - y0;
            var direction = x1 < x0 ? -1 : 1;
            var major = Math.Max(dx, dy); var minor = Math.Min(dx, dy);
            var error = 2 * minor - major;
            void Plot()
            {
                if (x0 < 0 || x0 >= width || y0 < 0 || y0 >= height)
                    throw new ArgumentException("source hint line outside raster");
                var index = y0 * width + x0;
                output[index] = color; output[width * height + index] = 1;
            }
            Plot();
            for (var index = 0; index < major; index++)
            {
                if (dx >= dy) x0 += direction; else y0++;
                if (error >= 0)
                {
                    if (dx >= dy) y0++; else x0 += direction;
                    error += 2 * (minor - major);
                }
                else error += 2 * minor;
                Plot();
            }
        }

        private static List<byte[]> SplitSourceLines(byte[] text, int width)
        {
            var lines = new List<byte[]>();
            var line = new List<byte>();
            var start = 0;
            while (start < text.Length)
            {
                var end = start;
                while (end < text.Length && text[end] != 32) end++;
                var length = end - start;
                if (line.Count != 0 && line.Count + length + 1 > width)
                { lines.Add(line.ToArray()); line.Clear(); }
                else if (line.Count != 0) line.Add(32);
                for (var index = start; index < end; index++) line.Add(text[index]);
                start = end + 1;
            }
            if (line.Count != 0) lines.Add(line.ToArray());
            return lines;
        }

        private static void ValidateGlyph(Glyph glyph, byte[] masks)
        {
            if (glyph.Offset < 0 || glyph.Length < 0 || glyph.Width < 0 || glyph.Height < 0 ||
                glyph.Length != checked(glyph.Width * glyph.Height) ||
                (long)glyph.Offset + glyph.Length > masks.Length)
                throw new ArgumentException("source objective glyph extent");
        }

        private static void ValidateSprite(Sprite sprite)
        {
            if (sprite.Width <= 0 || sprite.Height <= 0 || sprite.Indices == null || sprite.Mask == null ||
                sprite.Indices.Length != checked(sprite.Width * sprite.Height) || sprite.Mask.Length != sprite.Indices.Length)
                throw new ArgumentException("source objective frame sprite extent");
        }

        private static void DrawGlyph(byte[] output, int width, int height, Glyph glyph, byte[] mask,
            int x, int y, byte color)
        {
            for (var row = 0; row < glyph.Height; row++)
            for (var column = 0; column < glyph.Width; column++)
            {
                var value = mask[glyph.Offset + row * glyph.Width + column];
                if (value > 1) throw new ArgumentException("source objective glyph mask is not binary");
                if (value == 0) continue;
                var targetX = x + column; var targetY = y + row;
                if (targetX < 0 || targetX >= width || targetY < 0 || targetY >= height) continue;
                var target = targetY * width + targetX;
                output[target] = color; output[width * height + target] = 1;
            }
        }

        private static void DrawSprite(byte[] output, int width, int height, Sprite sprite, int x, int y)
        {
            for (var row = 0; row < sprite.Height; row++)
            for (var column = 0; column < sprite.Width; column++)
            {
                var source = row * sprite.Width + column;
                var value = sprite.Mask[source];
                if (value > 1) throw new ArgumentException("source objective sprite mask is not binary");
                if (value == 0) continue;
                var targetX = x + column; var targetY = y + row;
                if (targetX < 0 || targetX >= width || targetY < 0 || targetY >= height) continue;
                var target = targetY * width + targetX;
                output[target] = sprite.Indices[source]; output[width * height + target] = 1;
            }
        }

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
