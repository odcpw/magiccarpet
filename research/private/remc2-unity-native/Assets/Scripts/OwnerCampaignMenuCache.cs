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
            public string level_id, name, briefing, briefing_text_id, name_source_bytes, pause_profile;
            public int viewport_x, viewport_y, portal_x, portal_y, portal_width, portal_height;
            public int source_ordinary_index, source_internal_index;
            public bool is_secret;
        }
        [Serializable] private sealed class Output { public string path, sha256; public int bytes; }
        [Serializable] private sealed class Portal { public string kind, path; public int width, height; }
        [Serializable] internal sealed class BitmapBinding { public int selector, width, height; public string path, disabled_path; }
        [Serializable] internal sealed class PositionedBitmap { public int selector, x, y; }
        [Serializable] internal sealed class StartupAnimation
        { public int x, y, initial_selector, first_selector, last_selector, step_ticks; }
        [Serializable] internal sealed class StartupButton
        { public int source_action, x, y, width, height, text_selector, normal_selector, hover_selector, help_key; }
        [Serializable] internal sealed class StartupBinding
        {
            public int width, height, clock_hz;
            public string background_path, palette_path;
            public PositionedBitmap logo;
            public StartupAnimation[] animations;
            public StartupButton[] buttons;
            public BitmapBinding[] bitmaps;
            public SourceMenuText.FontBinding font;
            public SourceMenuText.HelpBinding[] help;
        }
        [Serializable] internal sealed class PortalSound
        { public int bank, id, volume, pan, rate; public string path; public bool if_not_playing; }
        [Serializable] private sealed class SettingsBinding
        {
            public int width, height;
            public string background_path;
            public BitmapBinding[] bitmaps;
        }

        internal sealed class SettingsAssets : IDisposable
        {
            internal Texture2D Background { get; private set; }
            private readonly Dictionary<int, Texture2D> bitmaps = new();
            internal SettingsAssets(int width, int height, string backgroundPath,
                BitmapBinding[] bindings, Func<string, int, byte[]> read)
            {
                if (width != 640 || height != 480 || bindings?.Length != 4)
                    throw new InvalidDataException("source settings artwork binding is incomplete");
                try
                {
                    Background = Texture(read(backgroundPath, 640 * 480 * 4), 640, 480, "MC2 source settings screen15");
                    LoadBitmaps(bindings, bitmaps, read);
                    if (!bitmaps.ContainsKey(107) || !bitmaps.ContainsKey(108) ||
                        !bitmaps.ContainsKey(109) || !bitmaps.ContainsKey(110))
                        throw new InvalidDataException("source settings focus artwork is unbound");
                }
                catch { Dispose(); throw; }
            }
            internal Texture2D Bitmap(int selector) => bitmaps.TryGetValue(selector, out var value) ? value :
                throw new InvalidDataException("unbound source settings sprite " + selector);
            public void Dispose()
            {
                if (Background != null) Destroy(Background);
                Background = null;
                foreach (var texture in bitmaps.Values) Destroy(texture);
                bitmaps.Clear();
            }
        }
        [Serializable] internal sealed class PortalAnimationBinding
        {
            public int clock_hz, step_ticks, opening_first, opening_last, loop_first, loop_last,
                completed_first, completed_last;
            public BitmapBinding[] frames;
            public PortalSound sound;
        }
        [Serializable] private sealed class LanguageBinding { public string path, binding_sha256, default_locale_id; }
        [Serializable] private sealed class LanguageScreenFlag
        { public string locale_id, indices_path, mask_path; public int selector, width, height; }
        [Serializable] private sealed class LanguageScreenBinding
        { public int width, height; public string background_path, palette_path; public BitmapBinding cursor; public LanguageScreenFlag[] flags; }
        [Serializable] private sealed class LanguageRecord { public string id, sha256; public int selector, offset, bytes; }
        [Serializable] private sealed class ObjectiveGlyph
        {
            public int index, offset, length, width, height;
            public string sha256;
        }
        [Serializable] private sealed class LanguageFont
        { public string id, masks_path; public ObjectiveGlyph[] glyphs; }
        [Serializable] private sealed class ObjectiveSpriteBinding
        { public int selector, width, height; public string indices_path, mask_path; }
        [Serializable] private sealed class ObjectiveProfileBinding
        { public string id, palette_path, colours_path, tables_path; public ObjectiveSpriteBinding[] sprites, pause_sprites; }
        [Serializable] private sealed class ObjectiveBinding
        { public string font_id; public int table_offset; public ObjectiveProfileBinding[] profiles; }
        [Serializable] private sealed class GameplayHelpRecord
        {
            public int row, kind, text_record, duration, flags, x, y, age_threshold, priority;
        }
        [Serializable] private sealed class GameplayHelpDocument
        { public string schema, source_sha256; public GameplayHelpRecord[] rows; }
        internal sealed class GameplayHelpBinding
        {
            internal readonly int Row, TextRecord, Duration, Flags, X, Y, AgeThreshold, Priority;
            internal GameplayHelpBinding(int row, int textRecord, int duration, int flags,
                int x, int y, int ageThreshold, int priority)
            {
                Row = row; TextRecord = textRecord; Duration = duration; Flags = flags;
                X = x; Y = y; AgeThreshold = ageThreshold; Priority = priority;
            }
        }
        internal sealed class ObjectiveAssets
        {
            internal readonly ObjectiveTextboxCompositor.Glyph[] Glyphs;
            internal readonly byte[] GlyphMasks, Tables, PaletteRgb8;
            internal readonly ObjectiveTextboxCompositor.Sprite Corner, Side, Horizontal;
            private readonly ObjectiveTextboxCompositor.Sprite[] pauseSprites;
            internal ObjectiveTextboxCompositor.Sprite[] PauseSprites => pauseSprites ??
                throw new InvalidDataException("source indexed pause sprites are not bound");
            internal readonly int TableOffset;
            internal readonly byte BlendSelector, TextColor, HelpTextColor;
            internal ObjectiveAssets(ObjectiveTextboxCompositor.Glyph[] glyphs, byte[] glyphMasks,
                ObjectiveTextboxCompositor.Sprite[] sprites,
                byte[] tables, int tableOffset, byte blendSelector, byte textColor, byte[] paletteRgb8,
                byte helpTextColor = 0, ObjectiveTextboxCompositor.Sprite[] pauseSprites = null)
            {
                Glyphs = glyphs; GlyphMasks = glyphMasks; Corner = sprites[0]; Side = sprites[1];
                Horizontal = sprites[2]; Tables = tables;
                TableOffset = tableOffset; BlendSelector = blendSelector; TextColor = textColor;
                HelpTextColor = helpTextColor;
                this.pauseSprites = pauseSprites;
                PaletteRgb8 = paletteRgb8;
            }
        }
        [Serializable] private sealed class LanguageLocale
        {
            public string id, pack_sha256, records_path, text_sha256;
            public string[] font_ids;
            public LanguageRecord[] records;
        }
        internal sealed class LocaleTextBinding
        {
            internal readonly string CachePath, BindingSha256, Id, PackSha256, TextSha256;
            private readonly string[] fontIds;
            private readonly string[] records;
            internal LocaleTextBinding(string path, string binding, string id, string pack, string text,
                string[] fonts, string[] values)
            {
                CachePath = path; BindingSha256 = binding; Id = id;
                PackSha256 = pack; TextSha256 = text;
                fontIds = (string[])fonts.Clone(); records = values;
            }
            internal string Record(int index) => index >= 0 && index < records.Length ? records[index] :
                throw new InvalidDataException("unbound source menu language record " + index);
            internal bool MatchesFonts(string[] values) => values != null && fontIds.SequenceEqual(values);
            internal PlayerLocaleBinding Identity => new PlayerLocaleBinding {
                cache_path = CachePath, binding_sha256 = BindingSha256, id = Id,
                pack_sha256 = PackSha256, text_sha256 = TextSha256, font_ids = (string[])fontIds.Clone()
            };
        }
        [Serializable] private sealed class LanguageManifest
        {
            public string schema, source_commit, binding_sha256, default_locale_id, glyph_mapping;
            public int language_prefix_bytes, language_record_count;
            public LanguageLocale[] locales;
            public LanguageFont[] fonts;
            public Output[] outputs;
        }
        [Serializable] private sealed class Manifest
        {
            public string schema, source_commit, source_language_sha256, source_catalog_path, gameplay_help_path;
            public LanguageBinding language_cache;
            public LanguageScreenBinding language_screen;
            public int width, height, view_width, view_height;
            public Mission[] missions;
            public Portal[] portals;
            public Output[] outputs;
            public StartupBinding startup;
            public SettingsBinding settings;
            public PortalAnimationBinding portal_animation;
            public SourcePauseMenu.Binding pause;
            public ObjectiveBinding objective;
            public SourceCampaignMap.Binding campaign;
            public PortalSound click_sound;
        }

        private readonly Dictionary<string, Mission> missions;
        private readonly Dictionary<string, Texture2D> portals = new();
        private readonly Dictionary<int, Texture2D> startupBitmaps = new();
        private readonly Dictionary<int, Texture2D> portalFrames = new();
        private byte[] portalSound;
        private byte[] clickSound;
        private readonly Dictionary<string, LocaleTextBinding> localeBindings = new(StringComparer.Ordinal);
        private readonly Dictionary<int, GameplayHelpBinding> gameplayHelp = new();
        private readonly Dictionary<string, int> briefingSelectors = new(StringComparer.Ordinal);
        private string languageCachePath, defaultLocaleId;
        private LocaleTextBinding currentLocale;
        private readonly Dictionary<string, Texture2D> languageFlags = new(StringComparer.Ordinal);
        internal Texture2D LanguageBackground { get; private set; }
        internal Texture2D LanguageCursor { get; private set; }
        private byte[] startupPalette;
        private ObjectiveTextboxCompositor.Glyph[] objectiveGlyphs;
        private byte[] objectiveGlyphMasks;
        private readonly Dictionary<string, ObjectiveAssets> objectiveProfiles = new(StringComparer.Ordinal);
        internal Texture2D World { get; private set; }
        internal Texture2D StartupBackground { get; private set; }
        internal StartupBinding Startup { get; private set; }
        internal PortalAnimationBinding PortalAnimation { get; private set; }
        internal SourceMenuText StartupText { get; private set; }
        internal SettingsAssets Settings { get; private set; }
        internal SourcePauseMenu PauseMenu { get; private set; }
        internal SourceCampaignMap Campaign { get; private set; }
        internal PortalSound ClickSound { get; private set; }

        internal OwnerCampaignMenuCache(string path)
        {
            var root = Path.GetFullPath(path);
            var manifestPath = Path.Combine(root, "manifest.json");
            if ((File.GetAttributes(root) & FileAttributes.ReparsePoint) != 0 ||
                (File.GetAttributes(manifestPath) & FileAttributes.ReparsePoint) != 0)
                throw new InvalidDataException("campaign menu cache must use regular owner files");
            var manifest = JsonUtility.FromJson<Manifest>(File.ReadAllText(manifestPath));
            if (manifest == null ||
                (manifest.schema != "mc2-owner-campaign-menu-cache/v1" && manifest.schema != "mc2-owner-campaign-menu-cache/v2" &&
                 manifest.schema != "mc2-owner-campaign-menu-cache/v3") ||
                manifest.source_commit != "6806c7204d17220ecc0430759f784aea8ee2ddb2" ||
                manifest.width != 1280 || manifest.height != 960 || manifest.view_width != 640 || manifest.view_height != 480 ||
                manifest.missions?.Length != (manifest.schema == "mc2-owner-campaign-menu-cache/v3" ? 30 : 4) ||
                manifest.portals?.Length != 2 || manifest.outputs == null ||
                manifest.outputs.Length < 4 ||
                (manifest.schema == "mc2-owner-campaign-menu-cache/v1" && manifest.outputs.Length != 4))
                throw new InvalidDataException("campaign menu cache has an unsupported source binding");
            if (manifest.schema == "mc2-owner-campaign-menu-cache/v3" &&
                manifest.missions.Any(mission => mission == null || mission.source_ordinary_index < 0 ||
                    mission.source_ordinary_index >= 25))
                throw new InvalidDataException("campaign mission has an invalid source briefing origin");
            if (manifest.schema != "mc2-owner-campaign-menu-cache/v1") BindLanguage(root, manifest);
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
                if (manifest.gameplay_help_path != null)
                {
                    if (manifest.gameplay_help_path != "gameplay-help.json")
                        throw new InvalidDataException("source gameplay help path is unbound");
                    var helpReceipt = manifest.outputs.SingleOrDefault(row => row.path == manifest.gameplay_help_path);
                    var helpHash = helpReceipt?.bytes switch {
                        2638 => "50cbdaec44a5ca55ef9451301c76f304bc6e18e32cdfb2c3c869b1ce52cabc33",
                        2746 => "914179b7e5e064cd41ab694defd0163d349b23738bf6a76fb8f6c878ce4be052",
                        2965 => "e6c0c41882fc529d239f2815a6b9e19e4cbdb295b5ce79f1384d25c01578aea5",
                        3075 => "b950880c717595525010f9752b712be3acd357c567e295325f3ddd28ca92dae2",
                        3186 => "76b2093efe79e811c5ba510286232f896c7f589046fcf432219c52328edc17d2",
                        3406 => "022637d862cd209bd56b5e66f96bfd976318a7e3da3bcc2f8da76d28ce369fca",
                        3630 => "6340326111c40b7272065af8bcf411ef49ff016791e61479bdb0ec51492b8702",
                        3856 => "d4cc8b1203b2ca4f9da0c2488c27966c2aaa3c099425ff7b78f2400f560896ce",
                        4077 => "8a108cc216d64884e51df5dd17f6419a4e31551af972c2d0f735f62fd64eb12f",
                        4303 => "472d70de35f78b3545fcfeaa864a29ea4a8706583b6eb400a33e5b2451a2efa1",
                        4525 => "5b18ce4a633f19ebc951d387cc2669553767c316d18f623cf60ac3f44e7a01db",
                        4857 => "6f4cedce621770af8f0a1ea92cf3cc9e79d6375ebe9057899be1ea2516a2a1be",
                        4967 => "a1262ada4fb8bf14f8d0e7077748636ea4190b4773d3dcee810fff36eee585c2",
                        5080 => "6dfd1ca61db643b2ef2ae94b9dda41c3de03bcd5acb1c861e6417426ffdb2f75",
                        5193 => "f8805e4d672a70d40509f0af76f629e5ee0cec8e6aa24c502f6b1aee6e7585fd",
                        5407 => "1e93cf05abd3e3af63451030736017f4c6d74bc244de48f8b57f0aa3e5ccbf8d",
                        _ => throw new InvalidDataException("source gameplay help receipt is incomplete")
                    };
                    var helpBytes = Read(manifest.gameplay_help_path, helpReceipt.bytes);
                    if (Digest(helpBytes) != helpHash)
                        throw new InvalidDataException("source gameplay help table identity differs");
                    var help = JsonUtility.FromJson<GameplayHelpDocument>(Encoding.ASCII.GetString(helpBytes));
                    var sourceRows = new List<int> { 1, 3, 40, 41, 42, 69, 70, 71,
                        72, 73, 74, 75, 76, 77, 78, 79, 80, 85, 86, 87, 88, 89, 90 };
                    if (helpReceipt.bytes >= 2746) sourceRows.Insert(1, 2);
                    if (helpReceipt.bytes >= 2965)
                    {
                        sourceRows.Insert(sourceRows.IndexOf(85), 81);
                        sourceRows.Insert(sourceRows.IndexOf(85), 82);
                    }
                    if (helpReceipt.bytes >= 3075) sourceRows.Insert(sourceRows.IndexOf(40), 4);
                    if (helpReceipt.bytes >= 3186) sourceRows.Insert(sourceRows.IndexOf(40), 5);
                    if (helpReceipt.bytes >= 4077) sourceRows.InsertRange(sourceRows.IndexOf(40), new[] { 6, 8 });
                    if (helpReceipt.bytes >= 4303) sourceRows.InsertRange(sourceRows.IndexOf(40), new[] { 12, 13 });
                    if (helpReceipt.bytes >= 4525)
                    {
                        sourceRows.Insert(sourceRows.IndexOf(8), 7);
                        sourceRows.Insert(sourceRows.IndexOf(12), 10);
                    }
                    if (helpReceipt.bytes >= 3406) sourceRows.InsertRange(sourceRows.IndexOf(40), new[] { 15, 16 });
                    if (helpReceipt.bytes >= 3630) sourceRows.InsertRange(sourceRows.IndexOf(40), new[] { 36, 37 });
                    if (helpReceipt.bytes >= 4857)
                    {
                        sourceRows.Insert(sourceRows.IndexOf(10), 9);
                        sourceRows.Insert(sourceRows.IndexOf(12), 11);
                        sourceRows.Insert(sourceRows.IndexOf(36), 21);
                    }
                    if (helpReceipt.bytes >= 4967) sourceRows.Insert(sourceRows.IndexOf(21), 18);
                    if (helpReceipt.bytes >= 5080) sourceRows.Insert(sourceRows.IndexOf(15), 14);
                    if (helpReceipt.bytes >= 5193) sourceRows.Insert(sourceRows.IndexOf(18), 17);
                    if (helpReceipt.bytes == 5407) sourceRows.InsertRange(sourceRows.IndexOf(36), new[] { 23, 24 });
                    if (helpReceipt.bytes >= 3856) sourceRows.InsertRange(sourceRows.IndexOf(40), new[] { 38, 39 });
                    if (help?.schema != "mc2-gameplay-help/v1" ||
                        help.source_sha256 != "a5d1812f796c690cd0fc7ea7f46cdb64bd9b73d999c632b04f9ea1e2806f4386" ||
                        help.rows?.Length != sourceRows.Count)
                        throw new InvalidDataException("source gameplay help table is incomplete");
                    for (var index = 0; index < help.rows.Length; index++)
                    {
                        var row = help.rows[index];
                        var expectedRow = sourceRows[index];
                        var expectedKind = expectedRow <= 17 ? 1 : expectedRow is 18 or 21 or 23 or 24 ? 2 : expectedRow <= 37 ? 4 : expectedRow <= 42 ? 5 : expectedRow <= 71 ? 6 :
                            expectedRow <= 80 ? 7 : expectedRow <= 82 ? 8 : expectedRow == 85 ? 10 : 11;
                        if (row == null || row.row != expectedRow || row.kind != expectedKind ||
                            row.text_record < 0 || row.text_record >= 471 ||
                            (expectedKind == 11 ? row.duration != 0 : row.duration <= 0) ||
                            row.flags < 0 || row.x < 0 || row.y < 0 || row.age_threshold < 0 ||
                            row.priority < 0 || row.priority > 6 || localeBindings.Count == 0 ||
                            localeBindings.Values.Any(locale => string.IsNullOrEmpty(locale.Record(row.text_record))))
                            throw new InvalidDataException("source gameplay help row or locale record is invalid");
                        gameplayHelp.Add(row.row, new GameplayHelpBinding(row.row, row.text_record,
                            row.duration, row.flags, row.x, row.y, row.age_threshold, row.priority));
                    }
                }
                if (manifest.schema == "mc2-owner-campaign-menu-cache/v3")
                {
                    if (manifest.source_catalog_path != "source-runtime-catalog.tsv")
                        throw new InvalidDataException("source mission catalog is unbound");
                    var receipt = manifest.outputs.Single(row => row.path == manifest.source_catalog_path);
                    if (receipt.bytes <= 0 || receipt.bytes > 16384)
                        throw new InvalidDataException("source mission catalog extent is invalid");
                    ValidateSourceCatalog(Read(manifest.source_catalog_path, receipt.bytes), manifest.missions);
                    var selectors = manifest.campaign?.indexed_portals?.Select(row => row.selector).ToArray();
                    if (selectors == null || Enumerable.Range(270, 4).Concat(Enumerable.Range(305, 8))
                            .Any(selector => !selectors.Contains(selector)))
                        throw new InvalidDataException("source secret portal art is incomplete");
                }
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
                if (manifest.language_screen != null)
                {
                    var screen = manifest.language_screen;
                    if (screen.width != 640 || screen.height != 480 || screen.cursor == null ||
                        screen.cursor.selector != 37 || screen.cursor.width <= 0 || screen.cursor.height <= 0 ||
                        screen.cursor.width > 256 || screen.cursor.height > 256 || screen.flags == null ||
                        screen.flags.Length != localeBindings.Count)
                        throw new InvalidDataException("source language screen is incomplete");
                    var screenIndices = Read(screen.background_path, 640 * 480);
                    var screenPalette = Read(screen.palette_path, 768);
                    if (screenPalette.Any(channel => channel > 63))
                        throw new InvalidDataException("source language screen palette is not RGB6");
                    byte[] Pixels(byte[] pixelIndices, byte[] mask)
                    {
                        var pixels = new byte[checked(pixelIndices.Length * 4)];
                        for (var i = 0; i < pixelIndices.Length; i++)
                        {
                            if (mask != null && mask[i] > 1) throw new InvalidDataException("source language flag mask is invalid");
                            var p = pixelIndices[i] * 3;
                            pixels[i * 4] = (byte)(screenPalette[p] * 4);
                            pixels[i * 4 + 1] = (byte)(screenPalette[p + 1] * 4);
                            pixels[i * 4 + 2] = (byte)(screenPalette[p + 2] * 4);
                            pixels[i * 4 + 3] = mask == null || mask[i] != 0 ? (byte)255 : (byte)0;
                        }
                        return pixels;
                    }
                    LanguageBackground = Texture(Pixels(screenIndices, null), 640, 480, "MC2 source language screen 12");
                    LanguageCursor = Texture(Read(screen.cursor.path, checked(screen.cursor.width * screen.cursor.height * 4)),
                        screen.cursor.width, screen.cursor.height, "MC2 source language cursor 37");
                    foreach (var flag in screen.flags)
                    {
                        if (flag == null || !localeBindings.ContainsKey(flag.locale_id) || flag.selector != 1 ||
                            flag.width <= 0 || flag.height <= 0 || flag.width > 640 || flag.height > 480 ||
                            languageFlags.ContainsKey(flag.locale_id))
                            throw new InvalidDataException("source language flag binding is invalid");
                        var size = checked(flag.width * flag.height);
                        var flagIndices = Read(flag.indices_path, size); var flagMask = Read(flag.mask_path, size);
                        languageFlags.Add(flag.locale_id, Texture(Pixels(flagIndices, flagMask), flag.width, flag.height,
                            "MC2 source language flag"));
                    }
                }
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
                    startupPalette = titlePalette;
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
                    if (Startup.font == null || Startup.help?.Length != 9)
                        throw new InvalidDataException("source title text is unbound; refresh the campaign menu cache");
                    var fontTexture = Texture(Read(Startup.font.path,
                        checked(Startup.font.width * Startup.font.height * 4)),
                        Startup.font.width, Startup.font.height, "MC2 HSCREEN bank4 original coloured font");
                    var blackIndex = Enumerable.Range(0, 256).OrderBy(index =>
                        titlePalette[index * 3] * titlePalette[index * 3] +
                        titlePalette[index * 3 + 1] * titlePalette[index * 3 + 1] +
                        titlePalette[index * 3 + 2] * titlePalette[index * 3 + 2]).First() * 3;
                    try
                    {
                        StartupText = new SourceMenuText(Startup.font, fontTexture, StartupBitmap,
                            new Color32((byte)(titlePalette[blackIndex] * 4),
                                (byte)(titlePalette[blackIndex + 1] * 4), (byte)(titlePalette[blackIndex + 2] * 4), 255));
                    }
                    catch { Destroy(fontTexture); throw; }
                    if (Startup.help.Select(help => help.help_key).Distinct().Count() != Startup.help.Length ||
                        Startup.help.Any(help => help.border_selector != 80 || help.line_selector != 89 ||
                            help.x < 0 || help.x + 192 > 640 || help.y < 0 || help.y > 440 ||
                            help.text_selector < 0 || help.text_selector >= 471 ||
                            !startupBitmaps.ContainsKey(help.border_selector) ||
                            !startupBitmaps.ContainsKey(help.border_selector - 1)))
                        throw new InvalidDataException("source title help layout is invalid");
                    if (!startupBitmaps.ContainsKey(Startup.logo.selector))
                        throw new InvalidDataException("source title logo is unbound");
                    foreach (var button in Startup.buttons)
                        if (button.x < 0 || button.y < 0 || button.width <= 0 || button.height <= 0 ||
                            button.x + button.width > 640 || button.y + button.height > 480 ||
                            !startupBitmaps.ContainsKey(button.normal_selector) || !startupBitmaps.ContainsKey(button.hover_selector))
                            throw new InvalidDataException("source title button is unbound");
                    foreach (var entry in Startup.buttons)
                        if (!Startup.help.Any(help => help.help_key == entry.help_key))
                            throw new InvalidDataException("source title button help is unbound");
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
                if (manifest.settings != null)
                    Settings = new SettingsAssets(manifest.settings.width, manifest.settings.height,
                        manifest.settings.background_path, manifest.settings.bitmaps, Read);
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
                if (manifest.pause != null) PauseMenu = new SourcePauseMenu(manifest.pause, Read, LanguageText);
                if (manifest.objective != null)
                {
                    var binding = manifest.objective;
                    if (binding.font_id != "hfont3" || binding.table_offset != 0x4000 ||
                        binding.profiles?.Length != 3 || objectiveGlyphs == null || objectiveGlyphMasks == null)
                        throw new InvalidDataException("source objective artwork binding is incomplete");
                    foreach (var profile in binding.profiles)
                    {
                        if (profile == null || (profile.id != "day" && profile.id != "night" && profile.id != "cave") ||
                            profile.palette_path != "pause-" + profile.id + "-palette.rgb6" ||
                            profile.colours_path != "objective-" + profile.id + "-colours.u8" ||
                            profile.tables_path != "objective-" + profile.id + "-tables.u8" ||
                            profile.sprites?.Length != 3 || objectiveProfiles.ContainsKey(profile.id))
                            throw new InvalidDataException("source objective profile is invalid");
                        var objectivePalette = Read(profile.palette_path, 768);
                        if (objectivePalette.Any(channel => channel > 63))
                            throw new InvalidDataException("source objective palette is not RGB6");
                        var colours = Read(profile.colours_path, 4096);
                        var tables = Read(profile.tables_path, 83456);
                        var sprites = new ObjectiveTextboxCompositor.Sprite[3];
                        for (var i = 0; i < sprites.Length; i++)
                        {
                            var row = profile.sprites[i];
                            var selector = 171 + i;
                            var prefix = "objective-" + profile.id + "-" + selector.ToString("D3");
                            if (row == null || row.selector != selector || row.width <= 0 || row.height <= 0 ||
                                row.width > 256 || row.height > 256 || row.indices_path != prefix + ".indices.u8" ||
                                row.mask_path != prefix + ".mask.u8")
                                throw new InvalidDataException("source objective frame sprite is invalid");
                            var objectiveIndices = Read(row.indices_path, checked(row.width * row.height));
                            var mask = Read(row.mask_path, checked(row.width * row.height));
                            if (mask.Any(value => value > 1))
                                throw new InvalidDataException("source objective frame mask is not binary");
                            sprites[i] = new ObjectiveTextboxCompositor.Sprite(row.width, row.height, objectiveIndices, mask);
                        }
                        ObjectiveTextboxCompositor.Sprite[] pauseSprites = null;
                        if (profile.pause_sprites != null)
                        {
                            if (profile.pause_sprites.Length != 5)
                                throw new InvalidDataException("source indexed pause sprites are incomplete");
                            pauseSprites = new ObjectiveTextboxCompositor.Sprite[5];
                            for (var i = 0; i < pauseSprites.Length; i++)
                            {
                                var row = profile.pause_sprites[i];
                                var selector = 174 + i;
                                var prefix = "pause-" + profile.id + "-" + selector.ToString("D3");
                                if (row == null || row.selector != selector || row.width <= 0 || row.height <= 0 ||
                                    row.width > 256 || row.height > 256 || row.indices_path != prefix + ".indices.u8" ||
                                    row.mask_path != prefix + ".mask.u8")
                                    throw new InvalidDataException("source indexed pause sprite is invalid");
                                var pauseIndices = Read(row.indices_path, checked(row.width * row.height));
                                var pauseMask = Read(row.mask_path, checked(row.width * row.height));
                                if (pauseMask.Any(value => value > 1))
                                    throw new InvalidDataException("source indexed pause mask is not binary");
                                pauseSprites[i] = new ObjectiveTextboxCompositor.Sprite(row.width, row.height, pauseIndices, pauseMask);
                            }
                        }
                        objectiveProfiles.Add(profile.id, new ObjectiveAssets(objectiveGlyphs, objectiveGlyphMasks,
                            sprites, tables, binding.table_offset, colours[0], colours[3840],
                            objectivePalette.Select(channel => checked((byte)(channel * 4))).ToArray(), colours[4095], pauseSprites));
                    }
                }
                if (manifest.campaign != null)
                {
                    Campaign = new SourceCampaignMap(manifest.campaign, Read, indices, palette);
                    foreach (var sample in manifest.campaign.motion?.sounds ?? Array.Empty<PortalSound>())
                        Campaign.BindMotionSound(sample.id,
                            Read(sample.path, manifest.outputs.Single(row => row.path == sample.path).bytes));
                }
                ClickSound = manifest.click_sound;
                if (ClickSound != null)
                {
                    if (ClickSound.bank != 0 || ClickSound.id != 14 || ClickSound.volume != 127 || ClickSound.pan != 64 ||
                        ClickSound.rate != 100 || ClickSound.if_not_playing)
                        throw new InvalidDataException("source menu click sound is unbound");
                    clickSound = Read(ClickSound.path, manifest.outputs.Single(row => row.path == ClickSound.path).bytes);
                }
                if (consumedOutputs.Count != manifest.outputs.Length)
                    throw new InvalidDataException("campaign cache contains unbound output receipts");
            }
            catch { Dispose(); throw; }
        }

        internal Mission ForMission(string level) => missions.TryGetValue(level, out var mission) ? mission :
            throw new InvalidDataException("campaign menu has no owner presentation for " + level);

        internal ObjectiveAssets ObjectiveForProfile(string profile) =>
            objectiveProfiles.TryGetValue(profile, out var assets) ? assets :
                throw new InvalidDataException("source objective assets are not bound for " + profile);

        internal GameplayHelpBinding GameplayHelpForRow(int row) =>
            gameplayHelp.TryGetValue(row, out var binding) ? binding :
                throw new InvalidDataException("source gameplay help row is unbound: " + row);

        internal bool HasMinimapHelp => gameplayHelp.ContainsKey(2);
        internal bool HasLargeMapHelp => gameplayHelp.ContainsKey(4);
        internal bool HasLowLifeHelp => gameplayHelp.ContainsKey(5);
        internal bool HasIdleHelp => gameplayHelp.ContainsKey(15) && gameplayHelp.ContainsKey(16);
        internal bool HasVitalsHelp => gameplayHelp.ContainsKey(36) && gameplayHelp.ContainsKey(37);
        internal bool HasCastHelp => gameplayHelp.ContainsKey(38) && gameplayHelp.ContainsKey(39);
        internal bool HasSpellSearchHelp => gameplayHelp.ContainsKey(6) && gameplayHelp.ContainsKey(8);
        internal bool HasCastleSpellHelp => gameplayHelp.ContainsKey(12) && gameplayHelp.ContainsKey(13);
        internal bool HasFireballHelp => gameplayHelp.ContainsKey(7) && gameplayHelp.ContainsKey(10);
        internal bool HasPossessionHelp => gameplayHelp.ContainsKey(9) && gameplayHelp.ContainsKey(11) &&
            gameplayHelp.ContainsKey(21);
        internal bool HasLooseManaHelp => gameplayHelp.ContainsKey(18);
        internal bool HasCastleReturnReminder => gameplayHelp.ContainsKey(14);
        internal bool HasCastleAttackHelp => gameplayHelp.ContainsKey(17);
        internal bool HasOwnedCastleMinimapHelp => gameplayHelp.ContainsKey(23) && gameplayHelp.ContainsKey(24);
        internal bool HasObjectiveMarkHelp => gameplayHelp.ContainsKey(81) && gameplayHelp.ContainsKey(82);

        internal IReadOnlyList<Mission> MissionOrder => missions.Values.Where(row => !row.is_secret)
            .OrderBy(row => row.source_ordinary_index).ToArray();
        internal SourceCampaignTerminalEffects.FinalMissionBinding FinalMissionBinding
        {
            get
            {
                var final = missions.Values.SingleOrDefault(row => row.source_ordinary_index == 24 && !row.is_secret);
                return final == null ? default : new SourceCampaignTerminalEffects.FinalMissionBinding(
                    final.level_id, final.source_ordinary_index, final.is_secret);
            }
        }

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
        internal Color StartupColor(int red, int green, int blue) =>
            SourceCampaignMap.PaletteColor(startupPalette, red, green, blue);
        internal string StartupHoverText(Vector2 sourcePosition)
        {
            var help = StartupHoverHelp(sourcePosition);
            return help == null ? null : LanguageText(help.text_selector);
        }
        internal SourceMenuText.HelpBinding StartupHoverHelp(Vector2 sourcePosition)
        {
            if (Startup == null) return null;
            foreach (var entry in Startup.buttons)
                if (new Rect(entry.x, entry.y, entry.width, entry.height).Contains(sourcePosition))
                    return Array.Find(Startup.help, help => help.help_key == entry.help_key);
            return null;
        }
        internal Texture2D PortalBitmap(int selector) => portalFrames.TryGetValue(selector, out var bitmap) ? bitmap :
            throw new InvalidDataException("unbound source portal sprite " + selector);
        internal byte[] ReadPortalOpeningSound() => portalSound ??
            throw new InvalidDataException("unbound source portal opening sound");
        internal byte[] ReadClickSound() => clickSound ?? throw new InvalidDataException("source menu click sound is unbound");
        internal string LanguageText(int selector) => currentLocale != null ? currentLocale.Record(selector) :
            throw new InvalidDataException("unbound source menu language record " + selector);
        internal bool HasLanguage => currentLocale != null;
        internal string LanguageCachePath => languageCachePath;
        internal string DefaultLocaleId => defaultLocaleId;
        internal Texture2D LanguageFlag(string localeId) => localeId != null && languageFlags.TryGetValue(localeId, out var flag)
            ? flag : throw new InvalidDataException("source locale flag is unbound");
        internal string BriefingText(Mission mission) => briefingSelectors.TryGetValue(mission.level_id, out var selector)
            ? LanguageText(selector) : mission.briefing;

        internal LocaleTextBinding PrepareLocale(string cachePath, string requestedLocale)
        {
            if (string.IsNullOrEmpty(cachePath) || languageCachePath == null ||
                !string.Equals(Path.GetFullPath(cachePath), languageCachePath, StringComparison.Ordinal))
                throw new InvalidDataException("selected language cache differs from the campaign binding");
            var id = requestedLocale ?? defaultLocaleId;
            if (!localeBindings.TryGetValue(id, out var binding))
                throw new ArgumentOutOfRangeException(nameof(requestedLocale), "selected source locale is not installed");
            _ = new Mc2ObjectiveText(binding);
            return binding;
        }

        internal LocaleTextBinding PrepareLocale(PlayerLocaleBinding accepted)
        {
            if (accepted == null) throw new InvalidDataException("native selected locale is unbound");
            var binding = PrepareLocale(accepted.cache_path, accepted.id);
            if (binding.Id != accepted.id || binding.BindingSha256 != accepted.binding_sha256 || binding.PackSha256 != accepted.pack_sha256 ||
                binding.TextSha256 != accepted.text_sha256 || !binding.MatchesFonts(accepted.font_ids))
                throw new InvalidDataException("native and campaign locale identities differ");
            return binding;
        }

        // Publish only a prepared, native-accepted candidate. All consumers
        // read this one retained reference, so no partial record table appears.
        internal void PublishLocale(LocaleTextBinding binding) => currentLocale = binding;

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

        private static void ValidateSourceCatalog(byte[] bytes, Mission[] bindings)
        {
            if (bytes.Any(value => value > 127))
                throw new InvalidDataException("source mission catalog is not ASCII");
            var rows = Encoding.ASCII.GetString(bytes).Split('\n')
                .Where(line => line.Length != 0 && !line.StartsWith("#", StringComparison.Ordinal)).ToArray();
            if (rows.Length != 30)
                throw new InvalidDataException("source mission catalog must contain the thirty original missions");
            var origins = new[] { 4, 7, 11, 17, 19 };
            var profiles = new[] { "day", "night", "cave" };
            for (var row = 0; row < rows.Length; row++)
            {
                var fields = rows[row].Split('\t');
                var index = row < 25 ? row : row + 5;
                var mission = bindings[row];
                if (fields.Length != 3 || fields[0] != index.ToString(System.Globalization.CultureInfo.InvariantCulture) ||
                    !int.TryParse(fields[2], out var mapType) || mapType < 0 || mapType > 2 ||
                    fields[2] != mapType.ToString(System.Globalization.CultureInfo.InvariantCulture) ||
                    fields[1] != mission.level_id || mission.source_internal_index != index ||
                    mission.is_secret != (row >= 25) || mission.pause_profile != profiles[mapType] ||
                    mission.source_ordinary_index != (row < 25 ? row : origins[row - 25]))
                    throw new InvalidDataException("campaign mission differs from its exact source catalog binding");
            }
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
            if (menu.objective != null)
            {
                var font = language.fonts?.SingleOrDefault(row => row.id == "hfont3");
                if (font?.glyphs?.Length != 273 || font.masks_path != "fonts/hfont3.masks.u8" ||
                    !payloads.TryGetValue(font.masks_path, out var masks))
                    throw new InvalidDataException("source objective HFONT3 is unbound");
                var glyphs = new ObjectiveTextboxCompositor.Glyph[font.glyphs.Length];
                var maskCursor = 0;
                for (var i = 0; i < font.glyphs.Length; i++)
                {
                    var glyph = font.glyphs[i];
                    if (glyph == null || glyph.index != i || glyph.offset != maskCursor ||
                        glyph.width < 0 || glyph.width > 10 || glyph.height < 0 || glyph.height > 14 ||
                        glyph.length != glyph.width * glyph.height || (long)maskCursor + glyph.length > masks.Length)
                        throw new InvalidDataException("source objective HFONT3 glyph extent is invalid");
                    var glyphMask = new byte[glyph.length];
                    Buffer.BlockCopy(masks, maskCursor, glyphMask, 0, glyphMask.Length);
                    if (glyphMask.Any(value => value > 1) || Digest(glyphMask) != glyph.sha256)
                        throw new InvalidDataException("source objective HFONT3 glyph mask identity mismatch");
                    glyphs[i] = new ObjectiveTextboxCompositor.Glyph(glyph.offset, glyph.length, glyph.width, glyph.height);
                    maskCursor += glyph.length;
                }
                if (maskCursor != masks.Length)
                    throw new InvalidDataException("source objective HFONT3 has unbound mask bytes");
                objectiveGlyphs = glyphs;
                objectiveGlyphMasks = masks;
            }
            languageCachePath = Path.GetFullPath(Path.Combine(root, binding.path));
            defaultLocaleId = binding.default_locale_id;
            foreach (var locale in language.locales)
            {
                if (locale?.records?.Length != 471 || locale.font_ids == null ||
                    !locale.font_ids.SequenceEqual(new[] { "font0", "font1", "font2", "hfont3" }) ||
                    !payloads.TryGetValue(locale.records_path, out var text) || Digest(text) != locale.text_sha256 ||
                    string.IsNullOrEmpty(locale.id) || string.IsNullOrEmpty(locale.pack_sha256))
                    throw new InvalidDataException("campaign source language is unbound");
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
                var values = locale.records.Select(record =>
                    Encoding.GetEncoding(28591).GetString(text, record.offset, record.bytes)).ToArray();
                if (!localeBindings.TryAdd(locale.id, new LocaleTextBinding(languageCachePath,
                    binding.binding_sha256, locale.id, locale.pack_sha256, locale.text_sha256, locale.font_ids, values)))
                    throw new InvalidDataException("duplicate source locale identity");
            }
            var defaultLocale = language.locales.SingleOrDefault(row => row.id == binding.default_locale_id);
            if (defaultLocale == null || defaultLocale.pack_sha256 != menu.source_language_sha256)
                throw new InvalidDataException("campaign default language is unbound");
            currentLocale = localeBindings[defaultLocale.id];
            for (var index = 0; index < menu.missions.Length; index++)
            {
                var mission = menu.missions[index];
                var selector = 23 + (menu.schema == "mc2-owner-campaign-menu-cache/v3"
                    ? mission.source_ordinary_index : index);
                var record = defaultLocale.records[selector];
                if (record.selector != selector || mission.briefing_text_id != record.id)
                    throw new InvalidDataException("campaign briefing is bound to the wrong source record");
                briefingSelectors.Add(mission.level_id, selector);
                mission.briefing = currentLocale.Record(selector);
            }
        }

        internal Mission MapFocus(GameRuntimeTransport.CampaignMenuSnapshot menu) =>
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
            var viewportX = Campaign?.Motion?.X ?? focus.viewport_x;
            var viewportY = Campaign?.Motion?.Y ?? focus.viewport_y;
            var view = SourceView(menu);
            if (!view.Contains(screen)) return null;
            var source = (screen - view.position) * (640f / view.width);
            foreach (var level in menu.available_levels)
            {
                var mission = ForMission(level);
                if (new Rect(mission.portal_x - viewportX, mission.portal_y - viewportY,
                    mission.portal_width, mission.portal_height).Contains(source)) return level;
            }
            return null;
        }

        internal void DrawMap(GameRuntimeTransport.CampaignMenuSnapshot menu) => DrawMap(menu, null);

        internal void DrawMap(GameRuntimeTransport.CampaignMenuSnapshot menu, Func<string, bool, int> selector)
        {
            var focus = MapFocus(menu);
            var view = SourceView(menu);
            if (Campaign?.Motion != null)
            {
                Campaign.DrawMap(menu, view, ForMission, selector);
                return;
            }
            var scale = view.width / 640f;
            GUI.DrawTextureWithTexCoords(view, World,
                new Rect(focus.viewport_x / 1280f, (960 - focus.viewport_y - 480) / 960f, .5f, .5f));
            foreach (var level in menu.available_levels)
            {
                var mission = ForMission(level);
                var completed = menu.completed_levels.Contains(level);
                // NewGameDraw state4 shows the earned result before opening
                // the next portal. State3/5 admits the available portal.
                if (menu.phase == "results" && !completed) continue;
                var selected = selector != null && PortalAnimation != null ? selector(level, completed) : -2;
                if (selected == -1) continue; // Source first pass opens sound before drawing.
                var texture = selected >= 0 ? PortalBitmap(selected) : portals[completed ? "completed" : "available"];
                var x = mission.portal_x - focus.viewport_x;
                var y = mission.portal_y - focus.viewport_y;
                if (x < 0 || y < 0 || x + texture.width > 640 || y + texture.height > 480) continue;
                GUI.DrawTexture(new Rect(view.x + x * scale, view.y + y * scale, texture.width * scale, texture.height * scale), texture);
            }
        }

        internal static Texture2D Texture(byte[] topDown, int width, int height, string name)
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
            currentLocale = null;
            localeBindings.Clear();
            gameplayHelp.Clear();
            briefingSelectors.Clear();
            foreach (var flag in languageFlags.Values) Destroy(flag);
            languageFlags.Clear();
            Destroy(LanguageBackground);
            LanguageBackground = null;
            Destroy(LanguageCursor);
            LanguageCursor = null;
            objectiveProfiles.Clear();
            objectiveGlyphs = null;
            objectiveGlyphMasks = null;
            Settings?.Dispose();
            Settings = null;
            StartupText?.Dispose();
            StartupText = null;
            PauseMenu?.Dispose();
            PauseMenu = null;
            Campaign?.Dispose();
            Campaign = null;
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
