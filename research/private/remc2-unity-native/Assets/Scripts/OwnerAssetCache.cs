using System;
using System.Collections.Generic;
using System.IO;
using System.Linq;
using System.Security.Cryptography;
using System.Text;
using UnityEngine;

namespace MagicCarpet.Native
{
    public sealed class OwnerAssetCache
    {
        public const string Schema = "mc2-owner-unity-asset-cache/v1";

        // These are the pinned Day owner-sprite roots consumed by the HUD
        // path. They are membership/root-bound checks only: the carried Rust
        // selectors are never recomputed from model, tier, or activation.
        internal static bool IsQualifiedDayHudSpriteSelector(int selector)
        {
            return selector == 1 ||
                   selector == 2 ||
                   selector == 40 ||
                   selector == 41 ||
                   selector == 42 ||
                   selector == 43 ||
                   selector == 54 ||
                   selector == 55 ||
                   (selector >= 123 && selector <= 148);
        }

        internal static bool IsQualifiedDayHudLevelTextSelector(int selector)
        {
            return selector >= 0 && selector <= 4;
        }
        private const string ExactManifestFileHash =
            "1eeb90e39927cb28e214e017c2603f3200ee2d3899f3f81ce53063f6d64a63d6";
        private const string ExactContentHash =
            "cb58b2190d3f6ed7c676e0180d0dfc909c7fe692e42946a927f887568be2eb8a";
        private const string ExactContractFileHash =
            "170bf5a1b9ee1190f89cf4d34ac9f139ca7ceb28761cd45785026575b112f110";
        private const string ExactContractSelfHash =
            "8007568bcfb4180137a7e0432fe2f094d44817fd771f90d4a98017036d9efcd4";
        private const string ExactSealHash =
            "becd3151922ccde6508ef4a77bf7987981c7fdedad04a3c040a9cdd72c88a96a";
        private const string PinnedRemc2Commit =
            "6806c7204d17220ecc0430759f784aea8ee2ddb2";
        private const string Level2PackageHash =
            "dc8f973ee9a2f53863a2e2e3743b6fa4b9ef947d3ce0b27b79d3ba86de318b80";
        private const string Level2ClosureHash =
            "bee3b7a4fd585d6631523b87217978cbfa58ed08cd494fa7921486053de1dccc";
        private const string Level2OwnerManifestSchema =
            "mc2-owner-level-export/v5";
        private const string Level2PlayersComponentMagic = "MCP1";
        private const int Level2PlayersComponentVersion = 4;
        private const int Level2PlayersComponentRecordBytes = 663;
        private const int Level2PlayersComponentCount = 8;
        private const string Level2CatalogHash =
            "ea9a735c598f2cbed81541366e1fdf8a5fcbc8cef7aecd086bb8a6a9e7c2429e";
        private const string Level2CatalogFileHash =
            "a25488be0933e5b4499665d37440a9860c8614649d9f695080da511c11d75b44";
        private const string Level2AudioDenominatorHash =
            "869c603fdabce9a3a354edaee58944b8e53b42eeb0c29288cb4c4433ba740cac";
        private const string Level2GameplaySchema =
            "mc2-level2-day-gameplay-cache/v1";
        private const string Level2GameplayDescriptorSchema =
            "mc2-level2-day-gameplay/v1";
        private const string Level2MaterialSchema =
            "mc2-level2-day-material-cache/v1";
        private const string Level2AudioSchema =
            "mc2-level2-day-audio-cache/v1";
        private static readonly int[] Level2HudSelectors =
        {
            1, 2, 40, 41, 42, 43, 54, 55,
            123, 124, 125, 126, 127, 128, 129, 130,
            131, 132, 133, 134, 135, 136, 137, 138,
            139, 140, 141, 142, 143, 144, 145, 146,
            147, 148
        };
        private static readonly string[] Level2HudTexts = { "I", "II", "III", "IV", "V" };

        [Serializable]
        private sealed class Manifest
        {
            public string schema;
            public Bindings bindings;
            public Output[] outputs;
            public string cache_content_sha256;
        }

        [Serializable]
        private sealed class Bindings
        {
            public string level_export_package_sha256;
            public string level_asset_closure_sha256;
            public string plan29_contract_file_sha256;
            public string plan29_contract_self_sha256;
        }

        [Serializable]
        private sealed class ConformanceSeal
        {
            public string schema;
            public string contract_file_sha256;
            public string contract_self_sha256;
            public string remc2_commit;
            public string seal_sha256;
        }

        [Serializable]
        private sealed class Output
        {
            public string path;
            public long bytes;
            public string sha256;
            public string component;
        }

        [Serializable]
        internal sealed class Level2DayHudDescriptor
        {
            public string schema;
            public string authority;
            public string bitmap_bank;
            public string font_bank;
            public Level2HudSource source;
            public Level2HudPalette palette;
            public int[] bitmap_selectors;
            public Level2HudBitmap[] bitmap_assets;
            public Level2HudSpellTierText spell_tier_text;
        }

        [Serializable]
        internal sealed class Level2GameplayDescriptor
        {
            public string schema;
            public Level2Profile profile;
            public Level2DayHudDescriptor hud;
            public Level2Output[] hud_outputs;
        }

        [Serializable]
        internal sealed class Level2HudSource
        {
            public string bitmap_bank;
            public string font_bank;
            public string font_glyph_index_rule;
            public string palette_bank;
            public string remc2_commit;
        }

        [Serializable]
        internal sealed class Level2HudPalette
        {
            public string asset_id;
            public string bank;
            public int channel_bits;
            public int entries;
            public string rgb6_path;
            public string rgb6_sha256;
            public string rgb8_expansion;
            public string rgb8_path;
            public string rgb8_sha256;
        }

        [Serializable]
        internal sealed class Level2HudBitmap
        {
            public string authority;
            public string bank;
            public string filter;
            public string indexed_pixels_sha256;
            public string indices_path;
            public string indices_sha256;
            public string mask_path;
            public string mask_semantics;
            public string mask_sha256;
            public string name;
            public string origin;
            public string palette_asset_id;
            public string row_order;
            public int runtime_height;
            public int runtime_width;
            public int selector;
            public int source_scale;
            public string source_asset_id;
            public int stored_height;
            public int stored_width;
            public string tab_asset_id;
            public string transparency_mask_sha256;
        }

        [Serializable]
        internal sealed class Level2HudSpellTierText
        {
            public string font_asset_id;
            public string font_bank;
            public string font_tab_asset_id;
            public string glyph_index_rule;
            public string palette_asset_id;
            public int palette_index;
            public string palette_selector_source_asset_id;
            public string[] texts;
            public Level2HudTier[] tiers;
        }

        [Serializable]
        internal sealed class Level2HudTier
        {
            public string authority;
            public string font_bank;
            public int[] glyph_indices;
            public int height;
            public string palette_asset_id;
            public int palette_index;
            public string pixel_format;
            public string raster_path;
            public string raster_sha256;
            public Level2HudDimensions runtime_dimensions;
            public int runtime_height;
            public int runtime_width;
            public int source_scale;
            public int stored_height;
            public int stored_width;
            public string text;
            public int[] text_bytes;
            public int tier;
            public int width;
        }

        [Serializable]
        internal sealed class Level2HudDimensions
        {
            public int height;
            public int width;
        }

        [Serializable]
        internal sealed class Level2Output
        {
            public string path;
            public long bytes;
            public string sha256;
            public string component;
        }

        [Serializable]
        private sealed class Level2AggregateManifest
        {
            public string schema;
            public Level2Profile profile;
            public Level2Bindings bindings;
            public Level2Components components;
            public Level2Output[] outputs;
            public string cache_content_sha256;
            public string manifest_payload_sha256;
        }

        [Serializable]
        private sealed class Level2Components
        {
            public Level2ComponentSummary material;
            public Level2ComponentSummary gameplay;
            public Level2ComponentSummary audio;
        }

        [Serializable]
        private sealed class Level2ComponentSummary
        {
            public string schema;
            public string manifest_path;
            public string manifest_sha256;
            public string cache_content_sha256;
            public int output_count;
        }

        [Serializable]
        private sealed class Level2ComponentManifest
        {
            public string schema;
            public Level2Profile profile;
            public Level2Bindings bindings;
            public Level2Output[] outputs;
            public string cache_content_sha256;
            public string manifest_payload_sha256;
        }

        [Serializable]
        internal sealed class Level2Profile
        {
            public string name;
            public int external_level;
            public int internal_level;
            public Level2MapType map_type;
            public Level2Selectors selectors;
            public string package_sha256;
            public string closure_sha256;
            public string catalog_sha256;
            public string catalog_file_sha256;
            public Level2OwnerPackage owner_package;
            public string spec_commit;
        }

        [Serializable]
        internal sealed class Level2OwnerPackage
        {
            public string manifest_schema;
            public Level2PlayersComponent players_component;
        }

        [Serializable]
        internal sealed class Level2PlayersComponent
        {
            public string magic;
            public int version;
            public int record_bytes;
            public int count;
        }

        [Serializable]
        internal sealed class Level2MapType
        {
            public int id;
            public string name;
        }

        [Serializable]
        internal sealed class Level2Selectors
        {
            public int vga_resolution_selector;
            public int texture_size;
            public int terrain_graphics_flags;
            public bool terrain_graphics_flag_2;
            public string terrain_bank;
            public string sprite_bank;
            public string web_bank;
            public string block_bank;
            public string palette;
            public string sky_bank;
        }

        [Serializable]
        internal sealed class Level2Bindings
        {
            public string level_export_package_sha256;
            public string level_asset_closure_sha256;
            public string owner_catalog_sha256;
            public string owner_catalog_file_sha256;
            public string owner_manifest_schema;
            public string players_component_magic;
            public int players_component_version;
            public int players_component_record_bytes;
            public int players_component_count;
            public string remc2_commit;
            public string audio_denominator_sha256;
        }

        public string RootPath { get; }
        public string ContentHash { get; }
        public int OutputCount { get; }
        internal Level2DayHudDescriptor DayHud { get; }
        private readonly HashSet<string> approvedPaths;

        private OwnerAssetCache(
            string rootPath,
            string contentHash,
            int outputCount,
            HashSet<string> approvedPaths,
            Level2DayHudDescriptor dayHud = null)
        {
            RootPath = rootPath;
            ContentHash = contentHash;
            OutputCount = outputCount;
            DayHud = dayHud;
            this.approvedPaths = approvedPaths;
        }

        public byte[] ReadOutput(string relative)
        {
            var canonical = relative.Replace(Path.DirectorySeparatorChar, '/');
            if (!approvedPaths.Contains(canonical))
                throw new InvalidDataException($"path is not a validated cache output: {relative}");
            return File.ReadAllBytes(ConfinedPath(RootPath, canonical));
        }

        public string ReadOutputText(string relative)
        {
            return Encoding.UTF8.GetString(ReadOutput(relative));
        }

        internal byte[] ReadLevel2GameplayOutput(string relative)
        {
            var canonical = relative?.Replace(Path.DirectorySeparatorChar, '/');
            ValidateLevel2RelativePath(canonical, "Level 2 gameplay output path");
            var aggregatePath = $"components/gameplay/{canonical}";
            RequireLevel2(
                approvedPaths.Contains(aggregatePath),
                $"Level 2 gameplay output membership {relative}");
            return File.ReadAllBytes(ConfinedPath(RootPath, aggregatePath));
        }

        public static string ResolveOptionalPath(string[] arguments)
        {
            for (var index = 0; index + 1 < arguments.Length; index++)
            {
                if (arguments[index] == "-mc-asset-cache")
                    return Path.GetFullPath(arguments[index + 1]);
            }
            var environment = Environment.GetEnvironmentVariable("MC_ASSET_CACHE_PATH");
            return string.IsNullOrWhiteSpace(environment)
                ? null
                : Path.GetFullPath(environment);
        }

        public static OwnerAssetCache OpenAndValidate(
            string rootPath,
            string expectedPackageHash,
            string expectedClosureHash)
        {
            var root = Path.GetFullPath(rootPath);
            var manifestPath = ConfinedPath(root, "manifest.json");
            if ((File.GetAttributes(root) & FileAttributes.ReparsePoint) != 0)
                throw new InvalidDataException("owner asset cache root is a symlink");
            if (!File.Exists(manifestPath))
                throw new InvalidDataException("owner asset cache manifest is missing");
            if ((File.GetAttributes(manifestPath) & FileAttributes.ReparsePoint) != 0)
                throw new InvalidDataException("owner asset cache manifest is a symlink");
            if (Sha256File(manifestPath) != ExactManifestFileHash)
                throw new InvalidDataException("owner asset cache manifest metadata mismatch");
            var manifest = JsonUtility.FromJson<Manifest>(File.ReadAllText(manifestPath));
            if (manifest == null || manifest.schema != Schema)
                throw new InvalidDataException("unsupported owner asset cache schema");
            if (manifest.bindings == null ||
                manifest.bindings.level_export_package_sha256 != expectedPackageHash)
                throw new InvalidDataException("owner asset cache package binding mismatch");
            if (manifest.bindings.level_asset_closure_sha256 != expectedClosureHash)
                throw new InvalidDataException("owner asset cache closure binding mismatch");
            if (manifest.bindings.plan29_contract_file_sha256 != ExactContractFileHash ||
                manifest.bindings.plan29_contract_self_sha256 != ExactContractSelfHash)
                throw new InvalidDataException("owner asset cache PLAN29 contract binding mismatch");
            if (manifest.outputs == null || manifest.outputs.Length == 0)
                throw new InvalidDataException("owner asset cache has no outputs");

            var paths = new HashSet<string>(StringComparer.Ordinal);
            foreach (var output in manifest.outputs)
            {
                if (output == null || string.IsNullOrWhiteSpace(output.path) ||
                    !paths.Add(output.path))
                    throw new InvalidDataException("invalid or duplicate cache output path");
                ConfinedPath(root, output.path);
            }
            var expectedTree = new HashSet<string>(paths, StringComparer.Ordinal)
                { "manifest.json" };
            var actualTree = new HashSet<string>(StringComparer.Ordinal);
            foreach (var path in Directory.GetFileSystemEntries(
                         root, "*", SearchOption.AllDirectories))
            {
                var attributes = File.GetAttributes(path);
                if ((attributes & FileAttributes.ReparsePoint) != 0)
                    throw new InvalidDataException("owner asset cache contains a symlink");
                if ((attributes & FileAttributes.Directory) != 0) continue;
                actualTree.Add(path.Substring(root.Length)
                    .TrimStart(Path.DirectorySeparatorChar)
                    .Replace(Path.DirectorySeparatorChar, '/'));
            }
            if (!actualTree.SetEquals(expectedTree))
                throw new InvalidDataException("owner asset cache tree mismatch");

            var identity = new StringBuilder();
            foreach (var output in manifest.outputs.OrderBy(value => value.path, StringComparer.Ordinal))
            {
                var extension = Path.GetExtension(output.path);
                if (extension.Equals(".DAT", StringComparison.OrdinalIgnoreCase) ||
                    extension.Equals(".TAB", StringComparison.OrdinalIgnoreCase))
                    throw new InvalidDataException("raw DAT/TAB is forbidden in Unity cache");
                var path = ConfinedPath(root, output.path);
                var info = new FileInfo(path);
                if (!info.Exists)
                    throw new InvalidDataException($"cache output missing: {output.path}");
                if (info.Length != output.bytes)
                    throw new InvalidDataException($"cache output truncated: {output.path}");
                var hash = Sha256File(path);
                if (!hash.Equals(output.sha256, StringComparison.Ordinal))
                    throw new InvalidDataException($"cache output hash mismatch: {output.path}");
                identity.Append(output.path).Append('\0')
                    .Append(hash).Append('\0')
                    .Append(output.bytes).Append('\n');
            }
            var identityHash = Sha256Bytes(Encoding.ASCII.GetBytes(identity.ToString()));
            if (!identityHash.Equals(manifest.cache_content_sha256, StringComparison.Ordinal))
                throw new InvalidDataException("owner asset cache identity mismatch");
            if (identityHash != ExactContentHash)
                throw new InvalidDataException("owner asset cache is not the qualified Level 1 cache");
            var seal = JsonUtility.FromJson<ConformanceSeal>(
                File.ReadAllText(ConfinedPath(root, "conformance-identity.json")));
            if (seal == null || seal.schema != "mc2-plan29-cache-conformance-identity/v1" ||
                seal.contract_file_sha256 != ExactContractFileHash ||
                seal.contract_self_sha256 != ExactContractSelfHash ||
                seal.remc2_commit != PinnedRemc2Commit ||
                seal.seal_sha256 != ExactSealHash)
                throw new InvalidDataException("owner asset cache PLAN29 conformance seal mismatch");
            return new OwnerAssetCache(root, identityHash, manifest.outputs.Length, paths);
        }

        public static OwnerAssetCache OpenAndValidateLevel2(string rootPath)
        {
            var root = Path.GetFullPath(rootPath);
            RequireLevel2Directory(root, "aggregate root");
            var manifestPath = ConfinedPath(root, "manifest.json");
            RequireLevel2File(manifestPath, "aggregate manifest");
            var manifestText = File.ReadAllText(manifestPath);
            var manifest = JsonUtility.FromJson<Level2AggregateManifest>(manifestText);
            RequireLevel2(manifest != null, "Level 2 aggregate manifest parses");
            RequireLevel2(
                manifest.schema == "mc2-level2-day-owner-cache/v1",
                "Level 2 aggregate schema");
            ValidateLevel2Profile(manifest.profile, "aggregate profile");
            RequireLevel2(
                manifest.bindings != null &&
                manifest.components != null &&
                manifest.components.material != null &&
                manifest.components.gameplay != null &&
                manifest.components.audio != null,
                "exact Level 2 aggregate component set");
            ValidateLevel2Bindings(manifest.bindings, false, "aggregate bindings");

            var aggregateRows = ValidateLevel2OutputRows(
                root,
                manifest.outputs,
                manifest.cache_content_sha256,
                "aggregate",
                true);
            ValidateLevel2Component(
                root,
                "material",
                manifest.components?.material,
                Level2MaterialSchema,
                aggregateRows);
            var gameplayRows = ValidateLevel2Component(
                root,
                "gameplay",
                manifest.components?.gameplay,
                Level2GameplaySchema,
                aggregateRows);
            ValidateLevel2Component(
                root,
                "audio",
                manifest.components?.audio,
                Level2AudioSchema,
                aggregateRows);
            var hud = ValidateLevel2GameplayHud(root, gameplayRows);
            return new OwnerAssetCache(
                root,
                manifest.cache_content_sha256,
                aggregateRows.Count,
                new HashSet<string>(aggregateRows.Keys, StringComparer.Ordinal),
                hud);
        }

        private static Dictionary<string, Level2Output> ValidateLevel2Component(
            string root,
            string name,
            Level2ComponentSummary summary,
            string expectedSchema,
            Dictionary<string, Level2Output> aggregateRows)
        {
            RequireLevel2(summary != null, $"Level 2 {name} component summary");
            var manifestPath = $"components/{name}/manifest.json";
            RequireLevel2(
                summary.manifest_path == manifestPath &&
                summary.schema == expectedSchema,
                $"Level 2 {name} component summary identity");
            var childRoot = ConfinedPath(root, $"components/{name}");
            RequireLevel2Directory(childRoot, $"Level 2 {name} component root");
            var childManifestPath = ConfinedPath(root, summary.manifest_path);
            RequireLevel2File(childManifestPath, $"Level 2 {name} child manifest");
            var childText = File.ReadAllText(childManifestPath);
            RequireLevel2(
                Sha256File(childManifestPath) == summary.manifest_sha256,
                $"Level 2 {name} child manifest hash");
            var child = JsonUtility.FromJson<Level2ComponentManifest>(childText);
            RequireLevel2(child != null, $"Level 2 {name} child manifest parses");
            RequireLevel2(child.schema == expectedSchema, $"Level 2 {name} child schema");
            ValidateLevel2Profile(child.profile, $"Level 2 {name} child profile");
            ValidateLevel2Bindings(child.bindings, name == "audio", $"Level 2 {name} child bindings");
            var childRows = ValidateLevel2OutputRows(
                childRoot,
                child.outputs,
                child.cache_content_sha256,
                $"Level 2 {name} child",
                false);
            RequireLevel2(
                childRows.Count == summary.output_count &&
                child.cache_content_sha256 == summary.cache_content_sha256,
                $"Level 2 {name} child summary receipt");
            foreach (var pair in childRows)
            {
                var aggregatePath = $"components/{name}/{pair.Key}";
                RequireLevel2(
                    aggregateRows.TryGetValue(aggregatePath, out var aggregateRow) &&
                    aggregateRow.bytes == pair.Value.bytes &&
                    aggregateRow.sha256 == pair.Value.sha256 &&
                    aggregateRow.component == name,
                    $"Level 2 {name} aggregate output binding {pair.Key}");
            }
            return childRows;
        }

        private static Level2DayHudDescriptor ValidateLevel2GameplayHud(
            string root,
            Dictionary<string, Level2Output> gameplayRows)
        {
            var descriptorRelative = "gameplay/descriptor.json";
            RequireLevel2(
                gameplayRows.TryGetValue(descriptorRelative, out var descriptorRow),
                "Level 2 gameplay descriptor output");
            var descriptorPath = ConfinedPath(
                root,
                $"components/gameplay/{descriptorRelative}");
            RequireLevel2File(descriptorPath, "Level 2 gameplay descriptor");
            RequireLevel2(
                Sha256File(descriptorPath) == descriptorRow.sha256,
                "Level 2 gameplay descriptor receipt");
            var descriptorText = File.ReadAllText(descriptorPath);
            var descriptor = JsonUtility.FromJson<Level2GameplayDescriptor>(descriptorText);
            RequireLevel2(descriptor != null, "Level 2 gameplay descriptor parses");
            RequireLevel2(
                descriptor.schema == Level2GameplayDescriptorSchema,
                "Level 2 gameplay descriptor schema");
            ValidateLevel2Profile(descriptor.profile, "Level 2 gameplay descriptor profile");
            var hud = descriptor.hud;
            RequireLevel2(hud != null, "Level 2 Day HUD descriptor");
            RequireLevel2(
                hud.schema == "mc2-level2-day-hud/v1" &&
                hud.authority == "SourceMappedLive" &&
                hud.bitmap_bank == "MSPRD0-0" &&
                hud.font_bank == "FONT1" &&
                hud.source != null &&
                hud.source.bitmap_bank == "MSPRD0-0" &&
                hud.source.font_bank == "FONT1" &&
                hud.source.font_glyph_index_rule == "ASCII byte + 1" &&
                hud.source.palette_bank == "PALD-0" &&
                hud.source.remc2_commit == PinnedRemc2Commit,
                "Level 2 Day HUD source identity");
            RequireLevel2(
                hud.bitmap_selectors != null &&
                hud.bitmap_selectors.Length == Level2HudSelectors.Length &&
                new HashSet<int>(hud.bitmap_selectors).SetEquals(Level2HudSelectors),
                "exact Level 2 Day HUD selector set");
            RequireLevel2(
                hud.bitmap_assets != null &&
                hud.bitmap_assets.Length == Level2HudSelectors.Length,
                "complete Level 2 Day HUD bitmap descriptors");
            var seenSelectors = new HashSet<int>();
            var expectedHudOutputPaths = new HashSet<string>(StringComparer.Ordinal);
            foreach (var bitmap in hud.bitmap_assets)
            {
                RequireLevel2(bitmap != null, "non-null Level 2 Day HUD bitmap descriptor");
                RequireLevel2(
                    seenSelectors.Add(bitmap.selector) &&
                    new HashSet<int>(Level2HudSelectors).Contains(bitmap.selector) &&
                    bitmap.authority == "SourceMappedLive" &&
                    bitmap.bank == "MSPRD0-0" &&
                    bitmap.source_asset_id ==
                        $"owner_data:DATA/MSPRD0-0.DAT#{bitmap.selector}" &&
                    bitmap.tab_asset_id == "owner_data:DATA/MSPRD0-0.TAB" &&
                    bitmap.palette_asset_id == "owner_data:DATA/PALD-0.DAT" &&
                    bitmap.filter == "nearest" &&
                    bitmap.origin == "top_left" &&
                    bitmap.row_order == "top_to_bottom" &&
                    bitmap.mask_semantics == "one_visible_zero_transparent" &&
                    bitmap.source_scale == 2 &&
                    bitmap.stored_width > 0 &&
                    bitmap.stored_height > 0 &&
                    bitmap.runtime_width == bitmap.stored_width * 2 &&
                    bitmap.runtime_height == bitmap.stored_height * 2 &&
                    bitmap.indices_path.StartsWith("gameplay/hud/", StringComparison.Ordinal) &&
                    bitmap.mask_path.StartsWith("gameplay/hud/", StringComparison.Ordinal) &&
                    bitmap.indices_sha256 == bitmap.indexed_pixels_sha256 &&
                    bitmap.mask_sha256 == bitmap.transparency_mask_sha256,
                    $"Level 2 Day HUD bitmap selector {bitmap.selector}");
                RequireLevel2HudOutput(
                    descriptor,
                    gameplayRows,
                    bitmap.indices_path,
                    bitmap.indices_sha256,
                    bitmap.stored_width * bitmap.stored_height,
                    expectedHudOutputPaths,
                    $"bitmap indices {bitmap.selector}");
                RequireLevel2HudOutput(
                    descriptor,
                    gameplayRows,
                    bitmap.mask_path,
                    bitmap.mask_sha256,
                    bitmap.stored_width * bitmap.stored_height,
                    expectedHudOutputPaths,
                    $"bitmap mask {bitmap.selector}");
            }
            RequireLevel2(
                seenSelectors.SetEquals(Level2HudSelectors),
                "Level 2 Day HUD bitmap selector census");
            RequireLevel2(
                hud.palette != null &&
                hud.palette.asset_id == "owner_data:DATA/PALD-0.DAT" &&
                hud.palette.bank == "PALD-0" &&
                hud.palette.channel_bits == 6 &&
                hud.palette.entries == 256 &&
                hud.palette.rgb8_expansion == "rgb6_value * 4",
                "Level 2 Day HUD PALD descriptor");
            RequireLevel2HudOutput(
                descriptor,
                gameplayRows,
                hud.palette.rgb6_path,
                hud.palette.rgb6_sha256,
                256 * 3,
                expectedHudOutputPaths,
                "PALD rgb6");
            RequireLevel2HudOutput(
                descriptor,
                gameplayRows,
                hud.palette.rgb8_path,
                hud.palette.rgb8_sha256,
                256 * 3,
                expectedHudOutputPaths,
                "PALD rgb8");

            var tiers = hud.spell_tier_text;
            RequireLevel2(
                tiers != null &&
                tiers.font_asset_id == "owner_data:DATA/FONT1.DAT" &&
                tiers.font_bank == "FONT1" &&
                tiers.font_tab_asset_id == "owner_data:DATA/FONT1.TAB" &&
                tiers.glyph_index_rule == "ASCII byte + 1" &&
                tiers.palette_asset_id == "owner_data:DATA/PALD-0.DAT" &&
                tiers.palette_index == 12 &&
                tiers.palette_selector_source_asset_id == "owner_data:DATA/CLRD-0.DAT" &&
                tiers.texts != null &&
                tiers.texts.SequenceEqual(Level2HudTexts) &&
                tiers.tiers != null &&
                tiers.tiers.Length == Level2HudTexts.Length,
                "Level 2 Day HUD Roman tier descriptor");
            for (var index = 0; index < tiers.tiers.Length; index++)
            {
                var tier = tiers.tiers[index];
                RequireLevel2(
                    tier != null &&
                    tier.tier == index &&
                    tier.text == Level2HudTexts[index] &&
                    tier.authority == "SourceMappedLive" &&
                    tier.font_bank == "FONT1" &&
                    tier.palette_asset_id == "owner_data:DATA/PALD-0.DAT" &&
                    tier.palette_index == 12 &&
                    tier.pixel_format == "RGBA8" &&
                    tier.text_bytes != null &&
                    tier.text_bytes.SequenceEqual(
                        tier.text.Select(character => (int)(byte)character)) &&
                    tier.glyph_indices != null &&
                    tier.glyph_indices.Length == tier.text_bytes.Length &&
                    tier.glyph_indices.SequenceEqual(
                        tier.text_bytes.Select(value => value + 1)) &&
                    tier.source_scale == 2 &&
                    tier.width > 0 && tier.height > 0 &&
                    tier.stored_width == tier.width &&
                    tier.stored_height == tier.height &&
                    tier.runtime_width == tier.stored_width * 2 &&
                    tier.runtime_height == tier.stored_height * 2 &&
                    tier.runtime_dimensions != null &&
                    tier.runtime_dimensions.width == tier.runtime_width &&
                    tier.runtime_dimensions.height == tier.runtime_height,
                    $"Level 2 Day HUD Roman tier {index}");
                RequireLevel2HudOutput(
                    descriptor,
                    gameplayRows,
                    tier.raster_path,
                    tier.raster_sha256,
                    tier.stored_width * tier.stored_height * 4,
                    expectedHudOutputPaths,
                    $"Roman tier {index}");
            }

            RequireLevel2(
                descriptor.hud_outputs != null &&
                descriptor.hud_outputs.Length == 75,
                "exact 75-row Level 2 Day HUD output ledger");
            var actualHudOutputPaths = new HashSet<string>(StringComparer.Ordinal);
            foreach (var row in descriptor.hud_outputs)
            {
                RequireLevel2(
                    row != null && actualHudOutputPaths.Add(row.path),
                    "unique Level 2 Day HUD output ledger path");
                RequireLevel2HudOutput(
                    descriptor,
                    gameplayRows,
                    row.path,
                    row.sha256,
                    row.bytes,
                    null,
                    "HUD output ledger row");
            }
            RequireLevel2(
                actualHudOutputPaths.SetEquals(expectedHudOutputPaths),
                "Level 2 Day HUD ledger matches palette/selector/Roman outputs");
            return hud;
        }

        private static void RequireLevel2HudOutput(
            Level2GameplayDescriptor descriptor,
            Dictionary<string, Level2Output> gameplayRows,
            string relativePath,
            string sha256,
            long bytes,
            HashSet<string> expectedPaths,
            string description)
        {
            RequireLevel2(
                !string.IsNullOrEmpty(relativePath) &&
                relativePath.StartsWith("gameplay/hud/", StringComparison.Ordinal),
                $"{description} path");
            if (expectedPaths != null) expectedPaths.Add(relativePath);
            RequireLevel2(
                gameplayRows.TryGetValue(relativePath, out var gameplayRow) &&
                gameplayRow.bytes == bytes &&
                gameplayRow.sha256 == sha256,
                $"{description} gameplay child receipt");
            var ledgerRow = descriptor.hud_outputs
                .FirstOrDefault(row => row != null && row.path == relativePath);
            RequireLevel2(
                ledgerRow != null &&
                ledgerRow.bytes == bytes &&
                ledgerRow.sha256 == sha256,
                $"{description} HUD ledger receipt");
        }

        private static Dictionary<string, Level2Output> ValidateLevel2OutputRows(
            string root,
            Level2Output[] rows,
            string expectedContentHash,
            string description,
            bool requireSorted)
        {
            RequireLevel2(rows != null && rows.Length > 0, $"{description} outputs");
            var outputByPath = new Dictionary<string, Level2Output>(StringComparer.Ordinal);
            var identity = new StringBuilder();
            var previousPath = string.Empty;
            foreach (var row in rows)
            {
                RequireLevel2(row != null, $"{description} output row");
                ValidateLevel2RelativePath(row.path, $"{description} output path");
                if (requireSorted)
                {
                    RequireLevel2(
                        previousPath.Length == 0 ||
                        string.CompareOrdinal(previousPath, row.path) < 0,
                        $"{description} outputs sorted and unique");
                }
                previousPath = row.path;
                RequireLevel2(
                    outputByPath.TryAdd(row.path, row),
                    $"{description} output uniqueness");
                var path = ConfinedPath(root, row.path);
                RequireLevel2File(path, $"{description} output {row.path}");
                var info = new FileInfo(path);
                RequireLevel2(info.Length == row.bytes, $"{description} output size {row.path}");
                var hash = Sha256File(path);
                RequireLevel2(hash == row.sha256, $"{description} output hash {row.path}");
                identity.Append(row.path).Append('\0')
                    .Append(row.sha256).Append('\0')
                    .Append(row.bytes).Append('\n');
            }
            var identityHash = Sha256Bytes(Encoding.ASCII.GetBytes(identity.ToString()));
            RequireLevel2(
                identityHash == expectedContentHash,
                $"{description} output identity");
            var expectedTree = new HashSet<string>(outputByPath.Keys, StringComparer.Ordinal)
                { "manifest.json" };
            ValidateLevel2Tree(root, expectedTree, description);
            return outputByPath;
        }

        private static void ValidateLevel2Profile(Level2Profile profile, string description)
        {
            RequireLevel2(
                profile != null &&
                profile.name == "level2-day" &&
                profile.external_level == 2 &&
                profile.internal_level == 1 &&
                profile.map_type != null &&
                profile.map_type.id == 0 &&
                profile.map_type.name == "Day" &&
                profile.package_sha256 == Level2PackageHash &&
                profile.closure_sha256 == Level2ClosureHash &&
                profile.catalog_sha256 == Level2CatalogHash &&
                profile.catalog_file_sha256 == Level2CatalogFileHash &&
                profile.owner_package != null &&
                profile.owner_package.manifest_schema == Level2OwnerManifestSchema &&
                profile.owner_package.players_component != null &&
                profile.owner_package.players_component.magic ==
                    Level2PlayersComponentMagic &&
                profile.owner_package.players_component.version ==
                    Level2PlayersComponentVersion &&
                profile.owner_package.players_component.record_bytes ==
                    Level2PlayersComponentRecordBytes &&
                profile.owner_package.players_component.count ==
                    Level2PlayersComponentCount &&
                profile.spec_commit == PinnedRemc2Commit &&
                profile.selectors != null &&
                profile.selectors.vga_resolution_selector == 1 &&
                profile.selectors.texture_size == 32 &&
                profile.selectors.terrain_graphics_flags == 0 &&
                !profile.selectors.terrain_graphics_flag_2 &&
                profile.selectors.terrain_bank == "TMAPS0-0" &&
                profile.selectors.sprite_bank == "MSPRD0-0" &&
                profile.selectors.web_bank == "MWEBD0-0" &&
                profile.selectors.block_bank == "BL32N0-0" &&
                profile.selectors.palette == "PALD-0" &&
                profile.selectors.sky_bank == "SKYD0-0",
                description);
        }

        private static void ValidateLevel2Bindings(
            Level2Bindings bindings,
            bool audio,
            string description)
        {
            RequireLevel2(
                bindings != null &&
                bindings.level_asset_closure_sha256 == Level2ClosureHash &&
                bindings.level_export_package_sha256 == Level2PackageHash &&
                bindings.owner_catalog_file_sha256 == Level2CatalogFileHash &&
                bindings.owner_catalog_sha256 == Level2CatalogHash &&
                bindings.owner_manifest_schema == Level2OwnerManifestSchema &&
                bindings.players_component_magic == Level2PlayersComponentMagic &&
                bindings.players_component_version == Level2PlayersComponentVersion &&
                bindings.players_component_record_bytes == Level2PlayersComponentRecordBytes &&
                bindings.players_component_count == Level2PlayersComponentCount &&
                bindings.remc2_commit == PinnedRemc2Commit &&
                (!audio || bindings.audio_denominator_sha256 == Level2AudioDenominatorHash),
                description);
        }

        private static void ValidateLevel2Tree(
            string root,
            HashSet<string> expected,
            string description)
        {
            var actual = new HashSet<string>(StringComparer.Ordinal);
            foreach (var path in Directory.GetFileSystemEntries(
                         root, "*", SearchOption.AllDirectories))
            {
                var attributes = File.GetAttributes(path);
                RequireLevel2(
                    (attributes & FileAttributes.ReparsePoint) == 0,
                    $"{description} tree symlink");
                if ((attributes & FileAttributes.Directory) != 0) continue;
                var relative = path.Substring(root.Length)
                    .TrimStart(Path.DirectorySeparatorChar)
                    .Replace(Path.DirectorySeparatorChar, '/');
                actual.Add(relative);
            }
            RequireLevel2(actual.SetEquals(expected), $"{description} exact tree");
        }

        private static void ValidateLevel2RelativePath(string relative, string description)
        {
            RequireLevel2(
                !string.IsNullOrWhiteSpace(relative) &&
                !Path.IsPathRooted(relative) &&
                relative.IndexOf('\\') < 0 &&
                relative.IndexOf("//", StringComparison.Ordinal) < 0 &&
                relative[0] != '/' &&
                relative[relative.Length - 1] != '/' &&
                relative.Split('/').All(part => part.Length > 0 && part != "." && part != ".."),
                description);
        }

        private static void RequireLevel2File(string path, string description)
        {
            RequireLevel2(File.Exists(path), $"{description} exists");
            RequireLevel2(
                (File.GetAttributes(path) & FileAttributes.ReparsePoint) == 0,
                $"{description} is not a symlink");
        }

        private static void RequireLevel2Directory(string path, string description)
        {
            RequireLevel2(Directory.Exists(path), $"{description} exists");
            RequireLevel2(
                (File.GetAttributes(path) & FileAttributes.ReparsePoint) == 0,
                $"{description} is not a symlink");
        }

        private static void RequireLevel2(bool condition, string description)
        {
            if (!condition)
                throw new InvalidDataException("Level 2 owner cache invariant failed: " + description);
        }

        private static string ConfinedPath(string root, string relative)
        {
            if (Path.IsPathRooted(relative))
                throw new InvalidDataException("absolute cache output path is forbidden");
            var path = Path.GetFullPath(Path.Combine(root, relative));
            var prefix = root.EndsWith(Path.DirectorySeparatorChar.ToString(), StringComparison.Ordinal)
                ? root
                : root + Path.DirectorySeparatorChar;
            if (!path.StartsWith(prefix, StringComparison.Ordinal))
                throw new InvalidDataException("cache output path escapes cache root");
            return path;
        }

        private static string Sha256File(string path)
        {
            using var stream = File.OpenRead(path);
            using var sha = SHA256.Create();
            return Hex(sha.ComputeHash(stream));
        }

        private static string Sha256Bytes(byte[] bytes)
        {
            using var sha = SHA256.Create();
            return Hex(sha.ComputeHash(bytes));
        }

        private static string Hex(byte[] bytes)
        {
            var builder = new StringBuilder(bytes.Length * 2);
            foreach (var value in bytes) builder.Append(value.ToString("x2"));
            return builder.ToString();
        }
    }
}
