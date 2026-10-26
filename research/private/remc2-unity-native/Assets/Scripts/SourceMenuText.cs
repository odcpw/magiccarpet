using System;
using System.Collections.Generic;
using System.IO;
using UnityEngine;

namespace MagicCarpet.Native
{
    // HSCREEN bank 4 owns a coloured, fixed-advance font. Basic.cpp's
    // DrawText_7FB90 uses byte indices directly, unlike the gameplay fonts.
    internal sealed class SourceMenuText : IDisposable
    {
        [Serializable] internal sealed class Glyph
        { public int index, x, y, width, height; }
        [Serializable] internal sealed class FontBinding
        {
            public string path, glyph_mapping, decoded_dat_sha256, decoded_tab_sha256;
            public int width, height, advance_x, advance_y;
            public Glyph[] glyphs;
        }
        [Serializable] internal sealed class HelpBinding
        {
            public int text_selector, x, y, line_x, line_y, line_end_x, line_end_y,
                help_key, border_selector, line_selector;
        }

        private readonly Texture2D atlas;
        private readonly FontBinding font;
        private readonly Func<int, Texture2D> bitmap;
        private readonly Color background;
        private readonly bool gameplayFont;
        private readonly bool campaignFont;

        internal SourceMenuText(FontBinding font, Texture2D atlas,
            Func<int, Texture2D> bitmap, Color background)
        {
            gameplayFont = font?.glyph_mapping == "mc2-byte-plus-one-controls/v1";
            campaignFont = font?.glyph_mapping == "mc2-hscreen-bank6-byte-plus-one/v1";
            var tall = gameplayFont || campaignFont;
            var cellWidth = tall ? 10 : 8;
            var cellHeight = tall ? 14 : 7;
            if (font == null || !tall && font.glyph_mapping != "mc2-hscreen-bank4-byte-controls/v1" ||
                font.width != 16 * cellWidth || font.height != (tall ? 252 : 119) ||
                font.advance_x != (gameplayFont ? 10 : campaignFont ? 8 : 7) || font.advance_y != cellHeight ||
                font.glyphs?.Length != (tall ? 273 : 272) || atlas == null || bitmap == null)
                throw new InvalidDataException("source title font is incomplete; refresh the campaign menu cache");
            for (var index = 0; index < font.glyphs.Length; index++)
            {
                var glyph = font.glyphs[index];
                if (glyph == null || glyph.index != index || glyph.x != index % 16 * cellWidth || glyph.y != index / 16 * cellHeight ||
                    glyph.width < 0 || glyph.width > cellWidth || glyph.height < 0 || glyph.height > cellHeight ||
                    (glyph.width == 0) != (glyph.height == 0))
                    throw new InvalidDataException("source title font glyph extent is invalid");
            }
            this.font = font;
            this.atlas = atlas;
            this.bitmap = bitmap;
            this.background = background;
        }

        internal void DrawText(string text, int x, int y, Rect view, Color? color = null)
        {
            var start = x;
            var scale = view.width / 640f;
            var oldColor = GUI.color;
            if (color.HasValue) GUI.color = color.Value;
            try
            {
                foreach (var value in text)
                {
                    if (value == 0 || x >= 640) break;
                    if (value > 255) throw new InvalidDataException("source menu text is not an owner byte string");
                    if (value == 13) continue;
                    if (value == 10) { x = start; y += font.advance_y; continue; }
                    var glyph = font.glyphs[gameplayFont || campaignFont ? value + 1 : value];
                    if (value != 9 && value != 32 && glyph.width != 0)
                        GUI.DrawTextureWithTexCoords(new Rect(view.x + x * scale, view.y + y * scale,
                                glyph.width * scale, glyph.height * scale), atlas,
                                new Rect(glyph.x / (float)font.width,
                                    (font.height - glyph.y - glyph.height) / (float)font.height,
                                    glyph.width / (float)font.width, glyph.height / (float)font.height));
                    x += gameplayFont ? (value == 9 || value == 32 ? font.glyphs[33].width : glyph.width) : font.advance_x;
                }
            }
            finally { GUI.color = oldColor; }
        }

        internal void DrawHelp(HelpBinding help, string text, Rect view)
        {
            var scale = view.width / 640f;
            void DrawBitmap(int selector, int x, int y)
            {
                var texture = bitmap(selector);
                GUI.DrawTexture(new Rect(view.x + x * scale, view.y + y * scale,
                    texture.width * scale, texture.height * scale), texture);
            }
            // sub_7E840 draws the pointer first, then the bordered textbox.
            var line = bitmap(help.line_selector);
            foreach (var part in LineBitmaps(help, line.width, line.height))
                DrawBitmap(part.selector, part.x, part.y);
            var top = bitmap(help.border_selector);
            var side = bitmap(help.border_selector - 1);
            var width = ((180 + top.width - 1) / top.width) * top.width;
            var right = help.x + width - side.width;
            var lines = WrapFramedText(text, font.advance_x, right - help.x);
            for (var x = 0; x < width; x += top.width)
                DrawBitmap(help.border_selector, help.x + x, help.y);
            var rowY = help.y + top.height;
            for (var row = 0; row < lines.Length + 2; row++)
            {
                var oldColor = GUI.color;
                GUI.color = background;
                GUI.DrawTexture(new Rect(view.x + help.x * scale, view.y + rowY * scale,
                    width * scale, side.height * scale), Texture2D.whiteTexture);
                GUI.color = oldColor;
                DrawBitmap(help.border_selector - 1, help.x, rowY);
                DrawBitmap(help.border_selector - 1, right, rowY);
                rowY += side.height;
            }
            for (var x = 0; x < width; x += top.width)
                DrawBitmap(help.border_selector, help.x + x, rowY);
            var textY = help.y + 12;
            foreach (var row in lines)
            {
                // DrawText_7FAE0 centres by fixed character advance, keeping
                // the source's one-character no-draw branch intact.
                if (row.Length != 1)
                    DrawText(row, help.x + (right - help.x) / 2 - font.advance_x * row.Length / 2, textY, view);
                textY += font.advance_y;
            }
        }

        // Basic.cpp sub_7FCB0: delimiters are retained on their preceding
        // word, and fitting reserves three fixed-width characters at right.
        internal static string[] WrapFramedText(string text, int advance, int width)
        {
            var lines = new List<string>();
            var current = "";
            var previous = 0;
            var lineBoundary = 0;
            for (var index = 0; index <= text.Length; index++)
            {
                var end = index == text.Length;
                if (!end && text[index] != ' ' && text[index] != ',' && text[index] != '-' && text[index] != '.') continue;
                var segmentEnd = end ? index : index + 1;
                if (previous == 0) current = text.Substring(0, segmentEnd);
                else
                {
                    var segment = text.Substring(previous + 1, segmentEnd - previous - 1);
                    if (advance * (index - lineBoundary) > width - 3 * advance)
                    {
                        lines.Add(current);
                        current = segment;
                        lineBoundary = previous;
                    }
                    else current += segment;
                }
                previous = index;
            }
            lines.Add(current);
            return lines.ToArray();
        }

        // EventsFunctions.cpp sub_81360 walks in whole sprite steps, then
        // draws the direction-specific cap at the exact endpoint.
        internal static IEnumerable<OwnerCampaignMenuCache.PositionedBitmap> LineBitmaps(
            HelpBinding help, int width, int height)
        {
            var dx = help.line_end_x - help.line_x;
            var dy = help.line_end_y - help.line_y;
            var cap = dx == 0 ? (dy < 0 ? -5 : -1) : dy == 0 ? (dx < 0 ? -3 : 3) :
                dx < 0 ? (dy < 0 ? -2 : -4) : (dy < 0 ? 4 : 2);
            var countX = 2 * Math.Abs(dx / width);
            var countY = 2 * Math.Abs(dy / height);
            var stepX = Math.Sign(dx) * width;
            var stepY = Math.Sign(dy) * height;
            var error = Math.Min(countX, countY) - (Math.Max(countX, countY) >> 1);
            var x = help.line_x;
            var y = help.line_y;
            while (true)
            {
                if (countX <= countY)
                {
                    if (y == help.line_end_y) break;
                    if (error >= 0) { x += stepX; error -= countY; }
                    y += stepY;
                    error += countX;
                }
                else
                {
                    if (x == help.line_end_x) break;
                    if (error >= 0) { y += stepY; error -= countX; }
                    x += stepX;
                    error += countY;
                }
                if (dx < 0 && x < help.line_end_x || dx > 0 && x > help.line_end_x ||
                    dy < 0 && y < help.line_end_y || dy > 0 && y > help.line_end_y) break;
                yield return new OwnerCampaignMenuCache.PositionedBitmap { selector = help.line_selector, x = x, y = y };
            }
            yield return new OwnerCampaignMenuCache.PositionedBitmap {
                selector = help.line_selector + cap, x = help.line_end_x, y = help.line_end_y };
        }

        public void Dispose()
        {
            if (Application.isPlaying) UnityEngine.Object.Destroy(atlas);
            else UnityEngine.Object.DestroyImmediate(atlas);
        }
    }
}
