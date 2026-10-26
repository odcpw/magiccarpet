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
            public SourceMenuText.FontBinding dialog_font;
            public OwnerCampaignMenuCache.StartupButton[] controls;
            public string colours_path;
            public bool english_statistics;
            public OwnerCampaignMenuCache.BitmapBinding[] bitmaps;
            public IndexedBitmap[] indexed_portals;
            public MotionBinding motion;
        }
        [Serializable] internal sealed class MotionBinding
        {
            public int presentation_hz;
            public string border_indices_path, border_mask_path;
            public SourceMapMotion.BackgroundBinding[] background;
            public SourceMapMotion.DecorationBinding[] decorations;
            public OwnerCampaignMenuCache.PortalSound[] sounds;
        }
        private sealed class Portal
        { internal int Width, Height; internal byte[] Indices, Mask; }
        private readonly Dictionary<int, Texture2D> bitmaps = new();
        private readonly Dictionary<int, Portal> portals = new();
        private readonly byte[] world, colours, palette;
        private readonly byte[] pristineWorld, borderIndices, borderMask;
        private readonly Dictionary<int, byte[]> motionSounds = new();
        private readonly Dictionary<int, OwnerCampaignMenuCache.PortalSound> soundBindings = new();
        private readonly List<(int x, int y)> pendingStamps = new();
        private readonly byte[] frameIndices = new byte[640 * 480], framePixels = new byte[640 * 480 * 4];
        private readonly SourceCampaignMapHover hover = new();
        private readonly SourceCampaignTerminalEffects terminalEffects = new();
        private Texture2D frame;
        private uint lastPresentationPass = uint.MaxValue;
        private bool frameComposed;
        internal bool SourcePassAdvanced { get; private set; }
        internal SourceMapMotion Motion { get; private set; }
        private readonly SourceMenuText text;
        private readonly int advance, lineHeight;
        private readonly bool englishStatistics;
        private Texture2D shade;
        private byte[] shadePixels;
        internal SourceMenuText DialogText { get; private set; }
        internal OwnerCampaignMenuCache.StartupButton[] Controls { get; private set; }

        internal SourceCampaignMap(Binding binding, Func<string, int, byte[]> read,
            byte[] world, byte[] palette)
        {
            if (binding?.font?.glyph_mapping != "mc2-hscreen-bank6-byte-plus-one/v1" ||
                binding.bitmaps == null || binding.indexed_portals == null || binding.indexed_portals.Length < 27 ||
                world?.Length != 1280 * 960 || palette?.Length != 768)
                throw new InvalidDataException("source campaign results artwork is incomplete");
            this.world = (byte[])world.Clone();
            pristineWorld = world;
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
                if (binding.dialog_font != null)
                {
                    if (binding.controls?.Length != 4) throw new InvalidDataException("source campaign controls are incomplete");
                    Controls = binding.controls;
                    foreach (var selector in Enumerable.Range(246, 13))
                        if (!bitmaps.ContainsKey(selector)) throw new InvalidDataException("unbound source campaign control");
                    var dialogAtlas = OwnerCampaignMenuCache.Texture(read(binding.dialog_font.path,
                        checked(binding.dialog_font.width * binding.dialog_font.height * 4)), binding.dialog_font.width,
                        binding.dialog_font.height, "MC2 original FONT1 dialog font");
                    try { DialogText = new SourceMenuText(binding.dialog_font, dialogAtlas, Bitmap, Color.black); }
                    catch { Destroy(dialogAtlas); throw; }
                }
                if (binding.motion != null)
                {
                    var row = binding.motion;
                    if (row.presentation_hz != 30 || row.sounds == null)
                        throw new InvalidDataException("source map animation clock is unbound");
                    Motion = new SourceMapMotion(row.background, row.decorations);
                    borderIndices = read(row.border_indices_path, 640 * 480);
                    borderMask = read(row.border_mask_path, 640 * 480);
                    if (borderMask.Any(value => value > 1)) throw new InvalidDataException("invalid map border mask");
                    foreach (var sound in row.sounds)
                    {
                        if (sound.bank != 0 || sound.id <= 0 || sound.volume != 127 || sound.pan != 64 || sound.rate != 100)
                            throw new InvalidDataException("source map sample binding is invalid");
                        // WAV data is receipt-checked by the cache owner. Its
                        // bounded length is supplied by the decoded manifest.
                        soundBindings.Add(sound.id, sound);
                    }
                    frame = new Texture2D(640, 480, TextureFormat.RGBA32, false)
                    { name = "MC2 source campaign indexed composition", filterMode = FilterMode.Point };
                }
            }
            catch { Dispose(); throw; }
        }

        internal Texture2D Bitmap(int selector) => bitmaps.TryGetValue(selector, out var bitmap) ? bitmap :
            throw new InvalidDataException("campaign source sprite is unbound; refresh the menu cache");

        internal void BindMotionSound(int id, byte[] bytes)
        {
            if (!soundBindings.ContainsKey(id)) throw new InvalidDataException("unbound source map sample");
            motionSounds.Add(id, bytes);
        }
        internal (int width, int height) Extent(int selector)
        {
            if (!portals.TryGetValue(selector, out var sprite))
                throw new InvalidDataException("source campaign indexed sprite is unbound: " + selector);
            return (sprite.Width, sprite.Height);
        }
        internal void Enter(GameRuntimeTransport.CampaignMenuSnapshot menu,
            Func<string, OwnerCampaignMenuCache.Mission> lookup, string lastLaunched)
        {
            if (Motion == null) throw new InvalidDataException("live source campaign map requires a refreshed artwork cache");
            Buffer.BlockCopy(pristineWorld, 0, world, 0, world.Length);
            pendingStamps.Clear();
            hover.Reset();
            terminalEffects.Reset();
            var center = Extent(37);
            Motion.Enter(menu, lookup, lastLaunched, Stamp, center.width / 2, center.height / 2);
            lastPresentationPass = uint.MaxValue;
            frameComposed = false;
        }
        internal void BeginTravel(string target, GameRuntimeTransport.CampaignMenuSnapshot menu,
            Func<string, OwnerCampaignMenuCache.Mission> lookup, FrontendMediaPresenter media)
        {
            var center = Extent(37);
            Motion.BeginTravel(target, menu, lookup, center.width / 2, center.height / 2,
                (id, kind, loops) => PlaySound(media, id, kind, loops));
        }
        internal string Step(GameRuntimeTransport.CampaignMenuSnapshot menu,
            Func<string, OwnerCampaignMenuCache.Mission> lookup, uint now, uint presentationPass,
            Vector2 mouse, bool leftHeld, bool allowPan, FrontendMediaPresenter media,
            SourceCampaignTerminalEffects.FinalMissionBinding finalMission = default)
        {
            SourcePassAdvanced = presentationPass != lastPresentationPass;
            if (!SourcePassAdvanced) return null;
            lastPresentationPass = presentationPass;
            terminalEffects.Step(menu, finalMission, presentationPass);
            var center = Extent(37);
            return Motion.Step(menu, lookup, now, (int)mouse.x, (int)mouse.y, leftHeld, allowPan,
                center.width / 2, center.height / 2, Extent, (x, y) => pendingStamps.Add((x, y)),
                (id, kind, loops) => PlaySound(media, id, kind, loops), terminalEffects.VolcanoEnabled);
        }
        internal void DrawOverlay(GameRuntimeTransport.CampaignMenuSnapshot menu, Rect view,
            Vector2 logicalPointer, bool showHelp, uint sourceTick100Hz, bool allowHelp,
            IReadOnlyList<OwnerCampaignMenuCache.Mission> missionOrder, Func<int, string> language)
        {
            hover.Draw(menu, Motion, view, logicalPointer, showHelp, sourceTick100Hz, allowHelp,
                missionOrder, language, Bitmap, DialogText);
        }
        private void PlaySound(FrontendMediaPresenter media, int id, int kind, int loops)
        {
            if (id == 0) return;
            if (!motionSounds.TryGetValue(id, out var bytes) || media == null)
                throw new InvalidDataException("source campaign animation sound is unbound");
            var row = soundBindings[id];
            media.PlayMenuSample(id, bytes, row.volume, row.pan, row.rate, kind == 2, loops);
        }
        private void Stamp(int x, int y) => Composite(world, 1280, 960, 139, x, y);
        private void Composite(byte[] destination, int width, int height, int selector, int x, int y)
        {
            if (!portals.TryGetValue(selector, out var sprite))
                throw new InvalidDataException("source campaign animation sprite is unbound: " + selector);
            for (var row = Math.Max(0, -y); row < sprite.Height && y + row < height; row++)
                for (var column = Math.Max(0, -x); column < sprite.Width && x + column < width; column++)
                {
                    var source = row * sprite.Width + column;
                    if (sprite.Mask[source] != 0) destination[(y + row) * width + x + column] = sprite.Indices[source];
                }
        }
        internal void DrawMap(GameRuntimeTransport.CampaignMenuSnapshot menu, Rect view,
            Func<string, OwnerCampaignMenuCache.Mission> lookup, Func<string, bool, int> selector)
        {
            DrawMap(menu, view, lookup, selector, null, null);
        }
        internal void DrawMap(GameRuntimeTransport.CampaignMenuSnapshot menu, Rect view,
            Func<string, OwnerCampaignMenuCache.Mission> lookup, Func<string, bool, int> selector,
            Func<string, bool> secretCompleted,
            Func<string, SourceSecretPortalAnimation.Presentation> secretPresentation)
        {
            if (Event.current.type == EventType.Repaint && (!frameComposed || SourcePassAdvanced))
            {
                for (var y = 0; y < 480; y++)
                    Buffer.BlockCopy(world, (Motion.Y + y) * 1280 + Motion.X, frameIndices, y * 640, 640);
                // A travelling carpet stamps the backing map after this pass's
                // background copy; the mark becomes visible next source pass.
                foreach (var point in pendingStamps) Stamp(point.x, point.y);
                pendingStamps.Clear();
                void Draw(SourceMapMotion.Sprite sprite) => Composite(frameIndices, 640, 480,
                    sprite.Selector, sprite.X - Motion.X, sprite.Y - Motion.Y);
                foreach (var sprite in Motion.BackgroundSprites) Draw(sprite);
                foreach (var level in menu.available_levels)
                {
                    var mission = lookup(level);
                    if (mission.is_secret) continue;
                    var complete = menu.completed_levels.Contains(level);
                    if (!complete && (menu.phase == "results" || !Motion.Ready)) continue;
                    var chosen = selector?.Invoke(level, complete) ?? (complete ? 37 : 70);
                    if (chosen == -1) continue;
                    Composite(frameIndices, 640, 480, chosen < 0 ? complete ? 37 : 70 : chosen,
                        mission.portal_x - Motion.X, mission.portal_y - Motion.Y);
                }
                foreach (var level in menu.available_levels)
                {
                    var mission = lookup(level);
                    if (!mission.is_secret) continue;
                    var complete = secretCompleted?.Invoke(level) == true;
                    if (!complete && (menu.phase == "results" || !Motion.Ready)) continue;
                    var secret = secretPresentation?.Invoke(level) ??
                        SourceSecretPortalAnimation.Presentation.Hidden;
                    if (!secret.Visible) continue;
                    Composite(frameIndices, 640, 480, secret.Selector,
                        mission.portal_x - Motion.X, mission.portal_y - Motion.Y);
                }
                if (!Motion.ActorIsTravelling && Motion.Actor.HasValue) Draw(Motion.Actor.Value);
                foreach (var sprite in Motion.DecorationSprites) Draw(sprite);
                if (Motion.ActorIsTravelling && Motion.Actor.HasValue) Draw(Motion.Actor.Value);
                for (var pixel = 0; pixel < frameIndices.Length; pixel++)
                    if (borderMask[pixel] != 0) frameIndices[pixel] = borderIndices[pixel];
                for (var y = 0; y < 480; y++)
                    for (var x = 0; x < 640; x++)
                    {
                        var source = frameIndices[y * 640 + x] * 3;
                        var target = ((479 - y) * 640 + x) * 4;
                        framePixels[target] = (byte)(palette[source] * 4);
                        framePixels[target + 1] = (byte)(palette[source + 1] * 4);
                        framePixels[target + 2] = (byte)(palette[source + 2] * 4);
                        framePixels[target + 3] = 255;
                    }
                frame.LoadRawTextureData(framePixels);
                frame.Apply(false, false);
                frameComposed = true;
            }
            GUI.DrawTexture(view, frame);
        }

        internal Color NearestColor(int red, int green, int blue) => PaletteColor(palette, red, green, blue);

        internal static Color PaletteColor(byte[] palette, int red, int green, int blue)
        {
            var closest = 0;
            var distance = 9999;
            for (var index = 0; index < 256; index++)
            {
                var r = red - palette[index * 3];
                var g = green - palette[index * 3 + 1];
                var b = blue - palette[index * 3 + 2];
                var value = r * r + g * g + b * b;
                if (value < distance) { distance = value; closest = index; }
            }
            return new Color32((byte)(palette[closest * 3] * 4), (byte)(palette[closest * 3 + 1] * 4),
                (byte)(palette[closest * 3 + 2] * 4), 255);
        }

        internal void DrawBitmap(int selector, int x, int y, Rect view)
        {
            var bitmap = bitmaps[selector];
            var scale = view.width / 640f;
            var left = Math.Max(0, x);
            var top = Math.Max(0, y);
            var right = Math.Min(640, x + bitmap.width);
            var bottom = Math.Min(480, y + bitmap.height);
            if (left >= right || top >= bottom) return;
            GUI.DrawTextureWithTexCoords(new Rect(view.x + left * scale, view.y + top * scale,
                (right - left) * scale, (bottom - top) * scale), bitmap,
                new Rect((left - x) / (float)bitmap.width, 1 - (bottom - y) / (float)bitmap.height,
                    (right - left) / (float)bitmap.width, (bottom - top) / (float)bitmap.height));
        }

        internal void DrawBriefing(GameRuntimeTransport.CampaignMenuSnapshot menu,
            OwnerCampaignMenuCache.Mission focus, string briefing, Rect view,
            Func<string, OwnerCampaignMenuCache.Mission> lookup, Func<string, bool, int> selector)
        {
            // PresentLevelDescription_80C30: FONT1 contributes the margins;
            // Basic's border style5 uses the retained coloured bank6 font.
            const int x = 146, width = 352;
            var y = (Motion?.DescriptionViewportY ?? focus.viewport_y) < 478 ? 280 : 60;
            var lines = SourceMenuText.WrapFramedText(briefing, advance, width, false);
            var top = bitmaps[275];
            var side = bitmaps[274];
            var bodyY = y + top.height;
            var rows = lines.Length + 2;
            DrawShade(menu, focus, lookup, selector, x, bodyY, width - side.width,
                rows * side.height, view);
            for (var column = 0; column < width; column += top.width)
            {
                DrawBitmap(275, x + column, y, view);
                DrawBitmap(275, x + column, bodyY + rows * side.height, view);
            }
            for (var row = 0; row < rows; row++)
            {
                DrawBitmap(274, x, bodyY + row * side.height, view);
                DrawBitmap(274, x + width - side.width, bodyY + row * side.height, view);
            }
            for (var row = 0; row < lines.Length; row++)
            {
                var left = x + (row == lines.Length - 1 ? top.width : 0);
                if (lines[row].Length != 1)
                    text.DrawText(lines[row], left + (x + width - left) / 2 - advance * lines[row].Length / 2,
                        y + side.height + row * lineHeight, view);
            }
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
                if (Motion != null) Buffer.BlockCopy(frameIndices, (y + row) * 640 + x, pixels, row * width, width);
                else Buffer.BlockCopy(world, (focus.viewport_y + y + row) * 1280 + focus.viewport_x + x,
                    pixels, row * width, width);
            foreach (var level in Motion == null ? menu.available_levels : Array.Empty<string>())
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
            DialogText?.Dispose();
            DialogText = null;
            foreach (var bitmap in bitmaps.Values) Destroy(bitmap);
            bitmaps.Clear();
            if (shade != null) Destroy(shade);
            shade = null;
            if (frame != null) Destroy(frame);
            frame = null;
        }
        private static void Destroy(Texture2D texture)
        {
            if (Application.isPlaying) UnityEngine.Object.Destroy(texture);
            else UnityEngine.Object.DestroyImmediate(texture);
        }
    }
}
