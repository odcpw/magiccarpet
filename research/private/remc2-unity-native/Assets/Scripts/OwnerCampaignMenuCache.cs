using System;
using System.Collections.Generic;
using System.IO;
using System.Linq;
using System.Security.Cryptography;
using System.Text;
using UnityEngine;

namespace MagicCarpet.Native
{
    // Private owner presentation assets. Mission availability and selection
    // always come from Rust; this cache supplies images, text and hit regions.
    internal sealed class OwnerCampaignMenuCache : IDisposable
    {
        [Serializable] internal sealed class Mission
        {
            public string level_id, name, briefing, briefing_text_id, name_source_bytes;
            public int viewport_x, viewport_y, portal_x, portal_y, portal_width, portal_height;
        }
        [Serializable] private sealed class Output { public string path, sha256; public int bytes; }
        [Serializable] private sealed class Portal { public string kind, path; public int width, height; }
        [Serializable] internal sealed class BitmapBinding { public int selector, width, height; public string path; }
        [Serializable] internal sealed class PositionedBitmap { public int selector, x, y; }
        [Serializable] internal sealed class StartupAnimation
        { public int x, y, initial_selector, first_selector, last_selector, step_ticks; }
        [Serializable] internal sealed class StartupButton
        { public int source_action, x, y, width, height, text_selector, normal_selector, hover_selector; }
        [Serializable] internal sealed class StartupBinding
        {
            public int width, height, clock_hz;
            public string background_path, palette_path;
            public PositionedBitmap logo;
            public StartupAnimation[] animations;
            public StartupButton[] buttons;
            public BitmapBinding[] bitmaps;
        }
        [Serializable] internal sealed class PortalSound
        { public int bank, id, volume, pan, rate; public string path; public bool if_not_playing; }
        [Serializable] internal sealed class PortalAnimationBinding
        {
            public int clock_hz, step_ticks, opening_first, opening_last, loop_first, loop_last,
                completed_first, completed_last;
            public BitmapBinding[] frames;
            public PortalSound sound;
        }
        [Serializable] private sealed class LanguageBinding { public string path, binding_sha256, default_locale_id; }
        [Serializable] private sealed class LanguageRecord { public string id, sha256; public int selector, offset, bytes; }
        [Serializable] private sealed class LanguageLocale
        {
            public string id, pack_sha256, records_path, text_sha256;
            public LanguageRecord[] records;
        }
        [Serializable] private sealed class LanguageManifest
        {
            public string schema, source_commit, binding_sha256, default_locale_id, glyph_mapping;
            public int language_prefix_bytes, language_record_count;
            public LanguageLocale[] locales;
            public Output[] outputs;
        }
        [Serializable] private sealed class Manifest
        {
            public string schema, source_commit, source_language_sha256;
            public LanguageBinding language_cache;
            public int width, height, view_width, view_height;
            public Mission[] missions;
            public Portal[] portals;
            public Output[] outputs;
            public StartupBinding startup;
            public PortalAnimationBinding portal_animation;
        }

        private readonly Dictionary<string, Mission> missions;
        private readonly Dictionary<string, Texture2D> portals = new();
        private readonly Dictionary<int, Texture2D> startupBitmaps = new();
        private readonly Dictionary<int, Texture2D> portalFrames = new();
        private byte[] portalSound;
        private string[] languageRecords;
        internal Texture2D World { get; private set; }
        internal Texture2D StartupBackground { get; private set; }
        internal StartupBinding Startup { get; private set; }
        internal PortalAnimationBinding PortalAnimation { get; private set; }

        internal OwnerCampaignMenuCache(string path)
        {
            var root = Path.GetFullPath(path);
            var manifestPath = Path.Combine(root, "manifest.json");
            if ((File.GetAttributes(root) & FileAttributes.ReparsePoint) != 0 ||
                (File.GetAttributes(manifestPath) & FileAttributes.ReparsePoint) != 0)
                throw new InvalidDataException("campaign menu cache must use regular owner files");
            var manifest = JsonUtility.FromJson<Manifest>(File.ReadAllText(manifestPath));
            if (manifest == null ||
                (manifest.schema != "mc2-owner-campaign-menu-cache/v1" && manifest.schema != "mc2-owner-campaign-menu-cache/v2") ||
                manifest.source_commit != "6806c7204d17220ecc0430759f784aea8ee2ddb2" ||
                manifest.width != 1280 || manifest.height != 960 || manifest.view_width != 640 || manifest.view_height != 480 ||
                manifest.missions?.Length != 4 || manifest.portals?.Length != 2 || manifest.outputs == null ||
                manifest.outputs.Length < 4 ||
                (manifest.schema == "mc2-owner-campaign-menu-cache/v1" && manifest.outputs.Length != 4))
                throw new InvalidDataException("campaign menu cache has an unsupported source binding");
            if (manifest.schema == "mc2-owner-campaign-menu-cache/v2") BindLanguage(root, manifest);
            missions = manifest.missions.ToDictionary(mission => mission.level_id, StringComparer.Ordinal);
            foreach (var mission in missions.Values)
                if (string.IsNullOrWhiteSpace(mission.name) || string.IsNullOrWhiteSpace(mission.briefing) ||
                    !mission.level_id.StartsWith("level.", StringComparison.Ordinal) ||
                    mission.viewport_x < 0 || mission.viewport_x > 640 || mission.viewport_y < 0 || mission.viewport_y > 480 ||
                    mission.portal_x < 0 || mission.portal_y < 0 || mission.portal_width <= 0 || mission.portal_height <= 0)
                    throw new InvalidDataException("campaign mission presentation is incomplete");
            var consumedOutputs = new HashSet<string>(StringComparer.Ordinal);
            if (manifest.outputs.Select(row => row.path).Distinct(StringComparer.Ordinal).Count() != manifest.outputs.Length)
                throw new InvalidDataException("duplicate campaign output identity");
            byte[] Read(string relative, int expected)
            {
                var row = manifest.outputs.Single(output => output.path == relative);
                if (relative != Path.GetFileName(relative)) throw new InvalidDataException("campaign cache path is not confined");
                var file = Path.Combine(root, relative);
                if ((File.GetAttributes(file) & FileAttributes.ReparsePoint) != 0) throw new InvalidDataException("campaign cache output is a symlink");
                var bytes = File.ReadAllBytes(file);
                using var sha = SHA256.Create();
                var digest = BitConverter.ToString(sha.ComputeHash(bytes)).Replace("-", "").ToLowerInvariant();
                if (bytes.Length != expected || bytes.Length != row.bytes || digest != row.sha256)
                    throw new InvalidDataException("campaign cache output receipt mismatch: " + relative);
                consumedOutputs.Add(relative);
                return bytes;
            }
            try
            {
                var indices = Read("world-map.indices.u8", 1280 * 960);
                var palette = Read("palette.rgb6", 768);
                if (palette.Any(channel => channel > 63)) throw new InvalidDataException("campaign palette is not RGB6");
                var rgba = new byte[indices.Length * 4];
                for (var i = 0; i < indices.Length; i++)
                {
                    var source = indices[i] * 3;
                    rgba[i * 4] = (byte)(palette[source] * 4);
                    rgba[i * 4 + 1] = (byte)(palette[source + 1] * 4);
                    rgba[i * 4 + 2] = (byte)(palette[source + 2] * 4);
                    rgba[i * 4 + 3] = 255;
                }
                World = Texture(rgba, 1280, 960, "MC2 owner campaign world map");
                foreach (var portal in manifest.portals)
                {
                    if ((portal.kind != "completed" && portal.kind != "available") ||
                        portal.width <= 0 || portal.width > 256 || portal.height <= 0 || portal.height > 256)
                        throw new InvalidDataException("campaign portal presentation is invalid");
                    portals.Add(portal.kind, Texture(Read(portal.path, checked(portal.width * portal.height * 4)),
                        portal.width, portal.height, "MC2 owner " + portal.kind + " portal"));
                }
                Startup = manifest.startup;
                if (Startup != null)
                {
                    if (Startup.width != 640 || Startup.height != 480 || Startup.clock_hz != 100 ||
                        Startup.logo == null || Startup.animations == null || Startup.bitmaps == null || Startup.buttons == null)
                        throw new InvalidDataException("unsupported source title artwork binding");
                    var title = Read(Startup.background_path, 640 * 480);
                    var titlePalette = Read(Startup.palette_path, 768);
                    if (titlePalette.Any(channel => channel > 63))
                        throw new InvalidDataException("title palette is not RGB6");
                    var titleRgba = new byte[title.Length * 4];
                    for (var i = 0; i < title.Length; i++)
                    {
                        titleRgba[i * 4] = (byte)(titlePalette[title[i] * 3] * 4);
                        titleRgba[i * 4 + 1] = (byte)(titlePalette[title[i] * 3 + 1] * 4);
                        titleRgba[i * 4 + 2] = (byte)(titlePalette[title[i] * 3 + 2] * 4);
                        titleRgba[i * 4 + 3] = 255;
                    }
                    StartupBackground = Texture(titleRgba, 640, 480, "MC2 source main menu bank4");
                    LoadBitmaps(Startup.bitmaps, startupBitmaps, Read);
                    if (!startupBitmaps.ContainsKey(Startup.logo.selector))
                        throw new InvalidDataException("source title logo is unbound");
                    foreach (var button in Startup.buttons)
                        if (button.x < 0 || button.y < 0 || button.width <= 0 || button.height <= 0 ||
                            button.x + button.width > 640 || button.y + button.height > 480 ||
                            !startupBitmaps.ContainsKey(button.normal_selector) || !startupBitmaps.ContainsKey(button.hover_selector))
                            throw new InvalidDataException("source title button is unbound");
                    foreach (var animation in Startup.animations)
                    {
                        if (animation.step_ticks != 4 || animation.first_selector > animation.last_selector ||
                            !startupBitmaps.ContainsKey(animation.initial_selector))
                            throw new InvalidDataException("source title animation is incomplete");
                        for (var selector = animation.first_selector; selector <= animation.last_selector; selector++)
                            if (!startupBitmaps.ContainsKey(selector))
                                throw new InvalidDataException("source title animation frame is unbound");
                    }
                }
                PortalAnimation = manifest.portal_animation;
                if (PortalAnimation != null)
                {
                    if (PortalAnimation.clock_hz != 100 || PortalAnimation.step_ticks != 8 ||
                        PortalAnimation.opening_first != 70 || PortalAnimation.opening_last != 83 ||
                        PortalAnimation.loop_first != 33 || PortalAnimation.loop_last != 35 ||
                        PortalAnimation.completed_first != 37 || PortalAnimation.completed_last != 43 ||
                        PortalAnimation.frames == null || PortalAnimation.sound == null)
                        throw new InvalidDataException("source portal animation binding is invalid");
                    LoadBitmaps(PortalAnimation.frames, portalFrames, Read);
                    // DrawFrameAnim increments before drawing and wraps only
                    // on the following call, so each last+1 sprite is visible.
                    foreach (var range in new[] { (70, 84), (33, 36), (37, 44) })
                        for (var selector = range.Item1; selector <= range.Item2; selector++)
                            if (!portalFrames.ContainsKey(selector))
                                throw new InvalidDataException("source portal animation frame is unbound");
                    var sound = PortalAnimation.sound;
                    if (sound.bank != 0 || sound.id != 41 || sound.volume != 127 || sound.pan != 64 ||
                        sound.rate != 100 || !sound.if_not_playing)
                        throw new InvalidDataException("source portal opening sound is unbound");
                    portalSound = Read(sound.path, manifest.outputs.Single(row => row.path == sound.path).bytes);
                }
                if (consumedOutputs.Count != manifest.outputs.Length)
                    throw new InvalidDataException("campaign cache contains unbound output receipts");
            }
            catch { Dispose(); throw; }
        }

        internal Mission ForMission(string level) => missions.TryGetValue(level, out var mission) ? mission :
            throw new InvalidDataException("campaign menu has no owner presentation for " + level);

        private static void LoadBitmaps(BitmapBinding[] bindings, Dictionary<int, Texture2D> destination,
            Func<string, int, byte[]> read)
        {
            foreach (var bitmap in bindings)
            {
                if (bitmap.selector < 0 || bitmap.width <= 0 || bitmap.height <= 0 ||
                    bitmap.width > 640 || bitmap.height > 480 || destination.ContainsKey(bitmap.selector))
                    throw new InvalidDataException("invalid source menu bitmap extent/selector");
                destination.Add(bitmap.selector, Texture(read(bitmap.path, checked(bitmap.width * bitmap.height * 4)),
                    bitmap.width, bitmap.height, "MC2 menu sprite " + bitmap.selector));
            }
        }

        internal Texture2D StartupBitmap(int selector) => startupBitmaps.TryGetValue(selector, out var bitmap) ? bitmap :
            throw new InvalidDataException("unbound source title sprite " + selector);
        internal string StartupHoverText(Vector2 sourcePosition)
        {
            if (Startup == null) return null;
            foreach (var entry in Startup.buttons)
                if (new Rect(entry.x, entry.y, entry.width, entry.height).Contains(sourcePosition))
                    return LanguageText(entry.text_selector);
            return null;
        }
        internal Texture2D PortalBitmap(int selector) => portalFrames.TryGetValue(selector, out var bitmap) ? bitmap :
            throw new InvalidDataException("unbound source portal sprite " + selector);
        internal byte[] ReadPortalOpeningSound() => portalSound ??
            throw new InvalidDataException("unbound source portal opening sound");
        internal string LanguageText(int selector) => languageRecords != null &&
            selector >= 0 && selector < languageRecords.Length ? languageRecords[selector] :
            throw new InvalidDataException("unbound source menu language record " + selector);
        internal bool HasLanguage => languageRecords != null;

        private static string Digest(byte[] bytes)
        {
            using var sha = SHA256.Create();
            return BitConverter.ToString(sha.ComputeHash(bytes)).Replace("-", "").ToLowerInvariant();
        }

        private static byte[] ReadLanguageFile(string root, string relative)
        {
            if (string.IsNullOrEmpty(relative) || Path.IsPathRooted(relative) || relative.Contains('\\'))
                throw new InvalidDataException("language output path is not confined");
            var path = root;
            foreach (var part in relative.Split('/'))
            {
                if (part is "" or "." or "..") throw new InvalidDataException("language output path is not confined");
                path = Path.Combine(path, part);
                if ((File.GetAttributes(path) & FileAttributes.ReparsePoint) != 0)
                    throw new InvalidDataException("language output must use regular owner files");
            }
            return File.ReadAllBytes(path);
        }

        private void BindLanguage(string root, Manifest menu)
        {
            var binding = menu.language_cache;
            if (binding == null || binding.path != "language" || binding.binding_sha256?.Length != 64)
                throw new InvalidDataException("campaign language binding is missing");
            var raw = Encoding.UTF8.GetString(ReadLanguageFile(root, "language/manifest.json")).TrimEnd('\n');
            var language = JsonUtility.FromJson<LanguageManifest>(raw);
            // The importer writes sorted, compact canonical JSON. Its binding
            // is the same exact byte stream before adding this first property.
            var prefix = "{\"binding_sha256\":\"" + binding.binding_sha256 + "\",";
            if (language?.schema != "mc2-installed-language-cache/v1" ||
                language.source_commit != menu.source_commit || language.binding_sha256 != binding.binding_sha256 ||
                language.default_locale_id != binding.default_locale_id ||
                language.glyph_mapping != "mc2-byte-plus-one-controls/v1" ||
                language.language_prefix_bytes != 4785 || language.language_record_count != 471 ||
                !raw.StartsWith(prefix, StringComparison.Ordinal) ||
                Digest(Encoding.UTF8.GetBytes("{" + raw.Substring(prefix.Length))) != binding.binding_sha256 ||
                language.outputs == null || language.locales == null)
                throw new InvalidDataException("campaign language binding identity mismatch");
            var payloads = new Dictionary<string, byte[]>(StringComparer.Ordinal);
            foreach (var output in language.outputs)
            {
                var bytes = ReadLanguageFile(root, "language/" + output.path);
                if (bytes.Length != output.bytes || Digest(bytes) != output.sha256 || !payloads.TryAdd(output.path, bytes))
                    throw new InvalidDataException("campaign language output identity mismatch");
            }
            var locale = language.locales.SingleOrDefault(row => row.id == binding.default_locale_id);
            if (locale?.records?.Length != 471 || locale.pack_sha256 != menu.source_language_sha256 ||
                !payloads.TryGetValue(locale.records_path, out var text) || Digest(text) != locale.text_sha256)
                throw new InvalidDataException("campaign default language is unbound");
            var cursor = 0;
            for (var selector = 0; selector < locale.records.Length; selector++)
            {
                var record = locale.records[selector];
                if (record.selector != selector || record.id != "text.mc2.record." + selector ||
                    record.offset != cursor || record.bytes < 0 || (long)cursor + record.bytes >= text.Length ||
                    text[cursor + record.bytes] != 0)
                    throw new InvalidDataException("campaign language record extent is invalid");
                var bytes = new byte[record.bytes];
                Buffer.BlockCopy(text, cursor, bytes, 0, bytes.Length);
                if (Array.IndexOf(bytes, (byte)0) >= 0 || Digest(bytes) != record.sha256)
                    throw new InvalidDataException("campaign language record identity mismatch");
                cursor += bytes.Length + 1;
            }
            if (cursor != text.Length) throw new InvalidDataException("campaign language contains unbound bytes");
            languageRecords = locale.records.Select(record =>
                Encoding.GetEncoding(28591).GetString(text, record.offset, record.bytes)).ToArray();
            for (var index = 0; index < menu.missions.Length; index++)
            {
                var mission = menu.missions[index];
                var record = locale.records[23 + index];
                if (record.selector != 23 + index || mission.briefing_text_id != record.id)
                    throw new InvalidDataException("campaign briefing is bound to the wrong source record");
                mission.briefing = Encoding.GetEncoding(28591).GetString(text, record.offset, record.bytes);
            }
        }

        private Mission MapFocus(GameRuntimeTransport.CampaignMenuSnapshot menu) =>
            ForMission(menu.phase == "results" ? menu.completed_level_id :
                menu.selected_level_id ?? menu.briefing_level_id ?? menu.completed_level_id);

        internal Rect SourceView(GameRuntimeTransport.CampaignMenuSnapshot menu)
        {
            var scale = Mathf.Min(Screen.width / 640f, Screen.height / 480f);
            return new Rect((Screen.width - 640 * scale) / 2, (Screen.height - 480 * scale) / 2, 640 * scale, 480 * scale);
        }

        internal string HitTestPortal(GameRuntimeTransport.CampaignMenuSnapshot menu, Vector2 screen)
        {
            if (menu.phase == "results") return null;
            var focus = MapFocus(menu);
            var view = SourceView(menu);
            if (!view.Contains(screen)) return null;
            var source = (screen - view.position) * (640f / view.width);
            foreach (var level in menu.available_levels)
            {
                var mission = ForMission(level);
                if (new Rect(mission.portal_x - focus.viewport_x, mission.portal_y - focus.viewport_y,
                    mission.portal_width, mission.portal_height).Contains(source)) return level;
            }
            return null;
        }

        internal void DrawMap(GameRuntimeTransport.CampaignMenuSnapshot menu) => DrawMap(menu, null);

        internal void DrawMap(GameRuntimeTransport.CampaignMenuSnapshot menu, Func<string, bool, int> selector)
        {
            var focus = MapFocus(menu);
            var view = SourceView(menu);
            var scale = view.width / 640f;
            GUI.DrawTextureWithTexCoords(view, World,
                new Rect(focus.viewport_x / 1280f, (960 - focus.viewport_y - 480) / 960f, .5f, .5f));
            foreach (var level in menu.available_levels)
            {
                var mission = ForMission(level);
                var completed = menu.completed_levels.Contains(level);
                var selected = selector != null && PortalAnimation != null ? selector(level, completed) : -2;
                if (selected == -1) continue; // Source first pass opens sound before drawing.
                var texture = selected >= 0 ? PortalBitmap(selected) : portals[completed ? "completed" : "available"];
                var x = mission.portal_x - focus.viewport_x;
                var y = mission.portal_y - focus.viewport_y;
                if (x < 0 || y < 0 || x + texture.width > 640 || y + texture.height > 480) continue;
                GUI.DrawTexture(new Rect(view.x + x * scale, view.y + y * scale, texture.width * scale, texture.height * scale), texture);
            }
        }

        private static Texture2D Texture(byte[] topDown, int width, int height, string name)
        {
            var bottomUp = new byte[topDown.Length];
            for (var y = 0; y < height; y++) Buffer.BlockCopy(topDown, y * width * 4, bottomUp, (height - 1 - y) * width * 4, width * 4);
            var texture = new Texture2D(width, height, TextureFormat.RGBA32, false) { name = name, filterMode = FilterMode.Point };
            texture.LoadRawTextureData(bottomUp);
            texture.Apply(false, true);
            return texture;
        }

        public void Dispose()
        {
            foreach (var texture in portals.Values) Destroy(texture);
            portals.Clear();
            foreach (var texture in startupBitmaps.Values) Destroy(texture);
            startupBitmaps.Clear();
            foreach (var texture in portalFrames.Values) Destroy(texture);
            portalFrames.Clear();
            if (StartupBackground != null) Destroy(StartupBackground);
            StartupBackground = null;
            portalSound = null;
            if (World != null) Destroy(World);
            World = null;
        }

        private static void Destroy(Texture2D texture)
        {
            if (Application.isPlaying) UnityEngine.Object.Destroy(texture);
            else UnityEngine.Object.DestroyImmediate(texture);
        }
    }
}
