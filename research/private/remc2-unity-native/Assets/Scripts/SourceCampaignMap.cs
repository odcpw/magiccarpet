using System;
using System.Collections.Generic;
using System.IO;
using System.Linq;
using UnityEngine;

namespace MagicCarpet.Native
{
    // HSCREEN bank6 and DrawEndGameTable_82C20. Rust supplies the earned
    // statistics; this owner supplies the original bitmap font, frame and
    // palette-index shading over the map and its current portal sprites.
    internal sealed class SourceCampaignMap : IDisposable
    {
        [Serializable] internal sealed class IndexedBitmap
        { public int selector, width, height; public string indices_path, mask_path; }
        [Serializable] internal sealed class Binding
        {
            public SourceMenuText.FontBinding font;
            public string colours_path;
            public bool english_statistics;
            public OwnerCampaignMenuCache.BitmapBinding[] bitmaps;
            public IndexedBitmap[] indexed_portals;
        }
        private sealed class Portal
        { internal int Width, Height; internal byte[] Indices, Mask; }
        private readonly Dictionary<int, Texture2D> bitmaps = new();
        private readonly Dictionary<int, Portal> portals = new();
        private readonly byte[] world, colours, palette;
        private readonly SourceMenuText text;
        private readonly int advance, lineHeight;
        private readonly bool englishStatistics;
        private Texture2D shade;
        private byte[] shadePixels;

        internal SourceCampaignMap(Binding binding, Func<string, int, byte[]> read,
            byte[] world, byte[] palette)
        {
            if (binding?.font?.glyph_mapping != "mc2-hscreen-bank6-byte-plus-one/v1" ||
                binding.bitmaps?.Length != 2 || binding.indexed_portals?.Length != 27 ||
                world?.Length != 1280 * 960 || palette?.Length != 768)
                throw new InvalidDataException("source campaign results artwork is incomplete");
            this.world = world;
            this.palette = palette;
            colours = read(binding.colours_path, 16384);
            advance = binding.font.advance_x;
            lineHeight = binding.font.advance_y;
            englishStatistics = binding.english_statistics;
            try
            {
                foreach (var row in binding.bitmaps)
                {
                    if (row.width <= 0 || row.width > 640 || row.height <= 0 || row.height > 480 ||
                        bitmaps.ContainsKey(row.selector))
                        throw new InvalidDataException("invalid campaign source sprite extent");
                    bitmaps.Add(row.selector, OwnerCampaignMenuCache.Texture(read(row.path,
                        checked(row.width * row.height * 4)), row.width, row.height,
                        "MC2 source campaign sprite " + row.selector));
                }
                foreach (var selector in new[] { 274, 275 })
                    if (!bitmaps.ContainsKey(selector)) throw new InvalidDataException("unbound campaign source sprite");
                foreach (var row in binding.indexed_portals)
                {
                    if (row.width <= 0 || row.width > 256 || row.height <= 0 || row.height > 256 ||
                        portals.ContainsKey(row.selector))
                        throw new InvalidDataException("invalid indexed campaign portal extent");
                    var mask = read(row.mask_path, checked(row.width * row.height));
                    if (mask.Any(value => value > 1)) throw new InvalidDataException("invalid campaign portal mask");
                    portals.Add(row.selector, new Portal { Width = row.width, Height = row.height,
                        Indices = read(row.indices_path, checked(row.width * row.height)), Mask = mask });
                }
                var atlas = OwnerCampaignMenuCache.Texture(read(binding.font.path,
                    checked(binding.font.width * binding.font.height * 4)), binding.font.width,
                    binding.font.height, "MC2 original HSCREEN bank6 coloured font");
                try { text = new SourceMenuText(binding.font, atlas, selector => bitmaps[selector], Color.black); }
                catch
                {
                    Destroy(atlas);
                    throw;
                }
            }
            catch { Dispose(); throw; }
        }

        private void DrawBitmap(int selector, int x, int y, Rect view)
        {
            var bitmap = bitmaps[selector];
            var scale = view.width / 640f;
            GUI.DrawTexture(new Rect(view.x + x * scale, view.y + y * scale,
                bitmap.width * scale, bitmap.height * scale), bitmap);
        }

        internal void DrawStatistics(GameRuntimeTransport.CampaignMenuSnapshot menu,
            OwnerCampaignMenuCache.Mission focus, OwnerCampaignMenuCache.Mission mission,
            GameRuntimeTransport.CompletionStatistics stats, Rect view,
            Func<string, OwnerCampaignMenuCache.Mission> lookup,
            Func<string, bool, int> selector, Func<int, string> language)
        {
            if (stats == null) throw new InvalidDataException("completed mission has no earned source statistics");
            var labels = new[] { mission.name, language(386), language(385), language(384), language(377), language(394) };
            // The source's selected language2 branch replaces these two
            // English records inside the results table itself.
            if (englishStatistics) { labels[2] = "Hit Accuracy"; labels[4] = "Mana Collected"; }
            var originalWidth = 2 * advance * labels.Max(value => value.Length);
            var beginX = 320 - originalWidth / 2;
            var top = bitmaps[275];
            var side = bitmaps[274];
            var width = ((originalWidth + top.width - 1) / top.width) * top.width;
            var bodyY = 50 + top.height;
            var bodyHeight = 12 * side.height;
            var innerX = beginX + side.width;
            var innerWidth = width - 2 * side.width;
            DrawShade(menu, focus, lookup, selector, innerX, bodyY, innerWidth, bodyHeight, view);
            for (var x = 0; x < width; x += top.width)
            {
                DrawBitmap(275, beginX + x, 50, view);
                DrawBitmap(275, beginX + x, bodyY + bodyHeight, view);
            }
            for (var row = 0; row < 12; row++)
            {
                DrawBitmap(274, beginX, bodyY + row * side.height, view);
                DrawBitmap(274, beginX + width - side.width, bodyY + row * side.height, view);
            }
            var textX = beginX + advance;
            var textY = 50 + 2 * side.width;
            if (labels[0].Length != 1)
                text.DrawText(labels[0], textX + (width - advance) / 2 - advance * labels[0].Length / 2,
                    textY, view);
            textY += 7;
            var values = new[] { stats.spells_found_percent, stats.hit_accuracy_percent,
                stats.creatures_killed_percent, stats.mana_collected_percent };
            for (var row = 1; row <= 4; row++)
            {
                var y = textY + row * lineHeight;
                text.DrawText(labels[row], textX + 2 * advance, y, view);
                text.DrawText(values[row - 1].ToString().PadLeft(3) + "%", textX + width - 8 * advance, y, view);
            }
            var timeY = textY + 6 * lineHeight;
            text.DrawText(labels[5], textX + 2 * advance, timeY, view);
            var seconds = stats.time_taken_seconds;
            text.DrawText($"{seconds / 3600:D2}:{seconds % 3600 / 60:D2}:{seconds % 60:D2}",
                textX + width - 14 * advance, timeY, view);
        }

        private void DrawShade(GameRuntimeTransport.CampaignMenuSnapshot menu,
            OwnerCampaignMenuCache.Mission focus, Func<string, OwnerCampaignMenuCache.Mission> lookup,
            Func<string, bool, int> selector, int x, int y, int width, int height, Rect view)
        {
            if (Event.current.type != EventType.Repaint) return;
            if (width <= 0 || x < 0 || x + width > 640 || y < 0 || y + height > 480)
                throw new InvalidDataException("source results sheet exceeds its 640x480 view");
            var pixels = new byte[checked(width * height)];
            for (var row = 0; row < height; row++)
                Buffer.BlockCopy(world, (focus.viewport_y + y + row) * 1280 + focus.viewport_x + x,
                    pixels, row * width, width);
            foreach (var level in menu.available_levels)
            {
                var completed = menu.completed_levels.Contains(level);
                if (menu.phase == "results" && !completed) continue;
                var chosen = selector(level, completed);
                if (chosen == -1) continue;
                if (chosen < 0) chosen = completed ? 37 : 70;
                if (!portals.TryGetValue(chosen, out var portal)) throw new InvalidDataException("unbound source portal frame");
                var mission = lookup(level);
                var px = mission.portal_x - focus.viewport_x - x;
                var py = mission.portal_y - focus.viewport_y - y;
                for (var row = Math.Max(0, -py); row < portal.Height && py + row < height; row++)
                    for (var column = Math.Max(0, -px); column < portal.Width && px + column < width; column++)
                    {
                        var source = row * portal.Width + column;
                        if (portal.Mask[source] != 0) pixels[(py + row) * width + px + column] = portal.Indices[source];
                    }
            }
            if (shade == null || shade.width != width || shade.height != height)
            {
                if (shade != null) Destroy(shade);
                shade = new Texture2D(width, height, TextureFormat.RGBA32, false)
                { name = "MC2 source palette-shaded results sheet", filterMode = FilterMode.Point };
                shadePixels = new byte[pixels.Length * 4];
            }
            for (var row = 0; row < height; row++)
                for (var column = 0; column < width; column++)
                {
                    var source = colours[0xF00 + pixels[row * width + column]] * 3;
                    var target = ((height - 1 - row) * width + column) * 4;
                    shadePixels[target] = (byte)(palette[source] * 4);
                    shadePixels[target + 1] = (byte)(palette[source + 1] * 4);
                    shadePixels[target + 2] = (byte)(palette[source + 2] * 4);
                    shadePixels[target + 3] = 255;
                }
            shade.LoadRawTextureData(shadePixels);
            shade.Apply(false, false);
            var scale = view.width / 640f;
            GUI.DrawTexture(new Rect(view.x + x * scale, view.y + y * scale, width * scale, height * scale), shade);
        }

        public void Dispose()
        {
            text?.Dispose();
            foreach (var bitmap in bitmaps.Values) Destroy(bitmap);
            bitmaps.Clear();
            if (shade != null) Destroy(shade);
            shade = null;
        }
        private static void Destroy(Texture2D texture)
        {
            if (Application.isPlaying) UnityEngine.Object.Destroy(texture);
            else UnityEngine.Object.DestroyImmediate(texture);
        }
    }
}
