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
        internal const string NightSchema = "mc2-owner-unity-asset-cache/v2";
        private const string QualifiedNightRuntimeHash =
            "b078dbed443e2cc3bf3bfd08a0f519492546d9f939db74a3d9c185d58f9c559f";
        private const string CurrentNightClosureHash =
            "ae375e8257b4d4aa701d71e77d37a165ae58de26a3a2415571024f3f3122435e";
        // The current import includes all 21 source BUILD rows and records its
        // actual v2 capture in the census. Qualified frame/palette bytes stay exact.
        private const string CurrentNightContentHash =
            "d63e748f0fa6550e8c0e451d81163ff71cfb3c8cdad1f516d97ea34644985569";
        // The source-closed v6 cache retains the same media products. Only
        // its frame census records updated exporter provenance; this pin
        // qualifies that complete cache independently of the retained import.
        private const string SourceClosedNightContentHash =
            "d4dec7b55d9ac6a68c3bc4ac03ac0b9b2ceded8e6b142cb1086ceffa00cd6b00";

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
        // GOLDEN-CHANGE: restore the already sealed PLAN29 identity used by the
        // current cache validator and Rust loader. This repairs a stale managed
        // admission pin; it does not regenerate or change any cache output.
        private const string ExactManifestFileHash =
            "1050b4f1e3f0e63fa292fe7798d19f4b390c063aa5afd5a350fba9e300298082";
        private const string ExactContentHash =
            "b664ad00c94e3f9393bc92ecac91a821673222cd365ed47617c4fab63db733bf";
        private const string ExactContractFileHash =
            "e8b2f760f24e8de2003e856027420fce4609c47c481d7ad5cf67b51f668f3f0a";
        private const string ExactContractSelfHash =
            "0d59fdb5f43ef794308bc45614a6b2edbffa46f23e29286695406838185b08a7";
        private const string ExactSealHash =
            "988058c2d08a3ebf7eed089de0e4ecd5a7739e7278689462d67a65856854342b";
        private const string PinnedRemc2Commit =
            "6806c7204d17220ecc0430759f784aea8ee2ddb2";
        private const string Level2PackageHash =
            "dc8f973ee9a2f53863a2e2e3743b6fa4b9ef947d3ce0b27b79d3ba86de318b80";
        // GOLDEN-CHANGE: source-backed rooted class-5 constructor TMAP choices
        // were added; simulation/event/audio identity remains unchanged.
        private const string Level2ClosureHash =
            "5f1935977111a911f8c0b954214fe89d3e9e6fa949a37df21be7668800ee101e";
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
        private const string Level3AggregateSchema = "mc2-day-owner-cache/v1";
        private const string Level3MaterialSchema = "mc2-day-material-cache/v1";
        private const string Level3GameplaySchema = "mc2-day-gameplay-cache/v1";
        private const string Level3GameplayDescriptorSchema = "mc2-day-gameplay/v1";
        private const string Level3MaterialDescriptorSchema = "mc2-day-materials/v1";
        private const string Level3AudioSchema = "mc2-day-audio-cache/v1";
        private const string Level3AggregateManifestHash =
            "7b9e05306f9dc6777c82928bbcb5231e853125c97cc75265f85e844fb9e1a2b1";
        private const string Level3AggregateContentHash =
            "a6f7d5c63d8d2612df166ca75034fa0ceb325439ae489245cf9122864765fbf9";
        private const string Level3MaterialManifestHash =
            "d82a3f077f34d61a14296239d5847da20d42ec01a4eb2ba20b5ef5d8359f8af5";
        private const string Level3MaterialContentHash =
            "a77943b57429c00993331b53d8397cf146ac8fb68a1df26d1be3d7eb4dfda52e";
        private const string Level3GameplayManifestHash =
            "d1d20c8d08349f9187bba7337b7730ec08098d36301abb295ac87bbe72ad9950";
        private const string Level3GameplayContentHash =
            "ef3a90f368c4b2fc86e604da8b674cea3c97119520ed523971a50f4a588e6b95";
        private const string Level3AudioManifestHash =
            "838dd5365d2484443f5f685535da7a6249614e923712d1ffd43d7545d0428cc3";
        private const string Level3AudioContentHash =
            "ec30b1d61cbbf206f89716c9f981906fda839eef5efc4f2e1588cb6b3222a335";
        private const string Level3AudioDenominatorHash =
            "a885675828a35203e1941fc24d8d060d8b732753f7ac295988b36a2ec327512c";
        private const string Level3MaterialDescriptorHash =
            "a76360e28bbd7ec66ac738ff40a9f1f568f7327e16cb4e759435bfc2ac9f9230";
        private const string Level3GameplayDescriptorHash =
            "794e3c4714319deb9d566d3832a39ead0c50f6fa116e91b8151232648ac57435";
        private const string Level3OwnerExportPackageHash =
            "0d10845690d7c9753246c633be771e54f3d6a481940af3f5fb33b909e0241a73";
        private const string Level3CampaignPackageHash =
            "6f17fe1c33c5852796b08acba2c44e5073939b947975b4f3b330d2c45b62d18d";
        private const string Level3ClosureHash =
            "d6e909d65437a37fed699429efa968175f39d6253c350537a1c82ce9729d5871";

        // GOLDEN-CHANGE: owner-approved BLOCK32 correction. Retain the exact v1
        // receipts; v2 is a separately qualified package, never a relaxed hash check.
        private static string DayReceipt(string legacy, bool correctedBlocks, bool minimapSymbols = false)
        {
            if (!correctedBlocks) return legacy;
            switch (legacy)
            {
                case Level3AggregateSchema: return minimapSymbols ? "mc2-day-owner-cache/v3" : "mc2-day-owner-cache/v2";
                case Level3MaterialSchema: return minimapSymbols ? "mc2-day-material-cache/v3" : "mc2-day-material-cache/v2";
                case Level3MaterialDescriptorSchema: return minimapSymbols ? "mc2-day-materials/v3" : "mc2-day-materials/v2";
                case Level3AggregateManifestHash: return minimapSymbols ? "3a4f6e113ecd8bf489f5f1ca8ba636f7e065b6166253b11c38444fac5617539a" : "f3ea4eaea129be68d83964994c39689a01173b815a0bdaafa5960f11ed1a3f3b";
                case Level3AggregateContentHash: return minimapSymbols ? "baf1873a409a2a27460810027dfd181eb85bcfd53e8fad40e0b1207f690462a8" : "d6bbe896a44340cfedbcdd1c910d291afbe111678c18a10971b93c8eb9907205";
                case Level3MaterialManifestHash: return minimapSymbols ? "7f52802eab3ac2eaf2197eef0781c42781cac1befc16508bc7eacd154bd2d17e" : "dfdd8bfb83dfd6d2fc07fcb345a857d7e03db0cf82227a5d935342eee9642f4c";
                case Level3MaterialContentHash: return minimapSymbols ? "916c2ad2fdbd10eff88e94a78a8c694554c88dda64d69fa84d6c766627e4edb4" : "9dbcdd2f0a2ee9a6c2826b3bd0442d79c921accd06d5ab6f1d903ac34eb9f858";
                case Level3GameplayManifestHash: return "a53a9c874d4d16e7a1fe7944db3f4f4e3d9c12bdf71bf1f30fe5f543c7599aef";
                case Level3GameplayContentHash: return "a71f75d185702471866dfb0db24c89bd6e766786e10a62556273171ec4d60f5b";
                case Level3AudioManifestHash: return "6f566e814a9be2d96639c8fc2e50b183289783ec6f0cc8ba400bca5a824d66b0";
                case Level3AudioContentHash: return "ef4536670fa76cbf4a5dde7c6244255266a020dc02a4c4fe4f62ac62da17f633";
                case Level3MaterialDescriptorHash: return minimapSymbols ? "c161eee61a31e6ab5f418dd1de92ea1c8fbc1f80790b7657ecbe25cb3e4fc894" : "e521aefffba8e1a82a7b74e929f1dcd1ed869cdb771622faccc13fa36b7575d7";
                case Level3GameplayDescriptorHash: return "076730c183f3dd4af9177085512b95bad98481c897f0daf37de8db5cf42b533f";
                case Level3OwnerExportPackageHash: return "10ce9fa050f915829507b4daa571f01f7e3f9b88512e3ec37f4e35791755bf8a";
                case Level3CampaignPackageHash: return "57b030e7feccadb834142229d0c6d267234214bad94b40d74655adca2718d6ea";
                case Level3ClosureHash: return "db2ba138f36bd2cec82c5ef1d7f2c8e6ce7207815ab8ec8c1e4fb6067029b885";
                default: throw new InvalidOperationException("Unknown Day qualification receipt");
            }
        }

        [Serializable]
        private sealed class Manifest
        {
            public string schema;
            public string scope;
            public Bindings bindings;
            public Output[] outputs;
            public string cache_content_sha256;
            public string night_metadata_sha256;
        }

        [Serializable]
        private sealed class Bindings
        {
            public string level_export_package_sha256;
            public string level_asset_closure_sha256;
            public string plan29_contract_file_sha256;
            public string plan29_contract_self_sha256;
            public string campaign_package_sha256;
            public string owner_catalog_sha256;
            public string owner_catalog_file_sha256;
            public string qualified_runtime_capture_sha256;
            public int qualified_runtime_capture_count;
            public string qualification_level_export_package_sha256;
            public string qualification_runtime_capture_sha256;
            public string runtime_visual_projection_sha256;
            public string runtime_visual_projection_schema;
            public string goat_tmap_golden_file_sha256;
            public string goat_tmap_oracle_manifest_sha256;
        }

        [Serializable]
        private sealed class NightPackageManifest
        {
            public string schema;
            public string package_sha256;
            public string owner_export_package_sha256;
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
            public string[] source_asset_ids;
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
        internal sealed class Level3MaterialDescriptor
        {
            public string schema;
            public Level2Profile profile;
            public Level3Palette palette;
            public Level3TextureBank tmaps0;
            public MinimapSymbolBank minimap_symbols;
        }

        [Serializable]
        internal sealed class MinimapSymbolBank
        {
            public string schema;
            public string bank;
            public string palette_asset_id;
            public int[] selectors;
            public MinimapSymbol[] symbols;
        }

        [Serializable]
        internal sealed class MinimapSymbol
        {
            public int selector;
            public int width;
            public int height;
            public string source_asset_id;
            public string indices_path;
            public string mask_path;
            public string indices_sha256;
            public string mask_sha256;
        }

        [Serializable]
        internal sealed class Level3Palette
        {
            public int entries;
            public string rgb8_path;
        }

        [Serializable]
        internal sealed class Level3TextureBank
        {
            public string bank;
            public string palette_asset_id;
            public int[] selectors;
            public Level3Texture[] textures;
        }

        [Serializable]
        internal sealed class Level3Texture
        {
            public int index;
            public int width;
            public int height;
            public int frame_count;
            public string indices_path;
            public string decoded_payload_path;
            public bool source_empty;
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
            public string owner_export_package_sha256;
            public string campaign_package_sha256;
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
            public string campaign_package_sha256;
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
        internal Level3MaterialDescriptor Level3Materials { get; }
        internal bool IsDayAggregate { get; }
        internal EnvironmentDescriptor EnvironmentMaterials { get; private set; }
        internal NativeCampaignSession.CampaignPackageIdentity EnvironmentPackage { get; private set; }
        internal bool IsEnvironment => EnvironmentMaterials != null;

        [Serializable]
        internal sealed class EnvironmentProfile
        {
            public int external_level, internal_level, vga_resolution_selector, texture_size;
            public int terrain_graphics_flags, key_color_1, key_color_2;
            public Level2MapType map_type;
            public string palette, tables, colors, blocks, dormant_blocks16, sky, sprites, web, tmaps;
            public string campaign_package_sha256, owner_export_package_sha256, spec_commit;
        }

        [Serializable]
        internal sealed class EnvironmentDescriptor
        {
            public string schema;
            public EnvironmentProfile profile;
            public EnvironmentProduct palette, tables, colors, blocks, sky, source16, day_exit_tables;
            public Level3TextureBank tmaps;
            public EnvironmentBitmapBank[] bitmap_banks;
            public EnvironmentBitmap cursor;
        }

        [Serializable]
        internal sealed class EnvironmentProduct
        {
            public string path, rgb8_path, atlas_path;
            public int width, height, tile_size, tile_count, key_color_1, key_color_2;
        }

        [Serializable]
        internal sealed class EnvironmentBitmapBank
        {
            public string bank;
            public EnvironmentBitmap[] bitmaps;
        }

        [Serializable]
        internal sealed class EnvironmentBitmap
        {
            public int selector, width, height;
            public bool source_empty;
            public string indices_path, mask_path;
        }

        [Serializable]
        private sealed class EnvironmentManifest
        {
            public string schema, manifest_payload_sha256, cache_content_sha256;
            public EnvironmentProfile profile;
            public Level2Output[] outputs;
            public EnvironmentSource[] source_files;
        }

        [Serializable]
        private sealed class EnvironmentSource
        {
            public string asset_id, content_address;
            public long size;
        }

        internal static bool HasEnvironmentMaterials(string path)
        {
            var text = File.ReadAllText(ConfinedPath(Path.GetFullPath(path), "manifest.json"));
            return JsonUtility.FromJson<EnvironmentManifest>(text)?.schema == "mc2-environment-material-cache/v1";
        }

        /// Open against a real campaign package, before replacing a visible
        /// generation. Content identity is derived from every immutable output.
        internal static OwnerAssetCache OpenAndValidateEnvironment(string path, string packagePath)
        {
            var root = Path.GetFullPath(path);
            RequireLevel2Directory(root, "environment cache root");
            RequireLevel2File(ConfinedPath(root, "manifest.json"), "environment manifest");
            var text = File.ReadAllText(ConfinedPath(root, "manifest.json"));
            var manifest = JsonUtility.FromJson<EnvironmentManifest>(text);
            RequireLevel2(manifest?.schema == "mc2-environment-material-cache/v1" && manifest.profile != null,
                "environment manifest schema/profile");
            // This dialect emits canonical compact JSON. Removing the parsed
            // self-hash member recovers its exact unsigned byte encoding.
            var member = "\"manifest_payload_sha256\":\"" + manifest.manifest_payload_sha256 + "\",";
            var offset = text.IndexOf(member, StringComparison.Ordinal);
            RequireLevel2(offset >= 0 && offset == text.LastIndexOf(member, StringComparison.Ordinal) &&
                Sha256Bytes(Encoding.UTF8.GetBytes(text.Remove(offset, member.Length))) == manifest.manifest_payload_sha256,
                "environment metadata hash");
            var p = manifest.profile;
            var map = p.map_type?.id ?? -1;
            RequireLevel2(map >= 0 && map <= 2 && p.external_level > 0 &&
                p.vga_resolution_selector >= 0 && p.vga_resolution_selector <= 1 && p.texture_size == 32,
                "environment map/resolution");
            var letter = new[] { "D", "N", "C" }[map];
            var display = map == 1 && (p.terrain_graphics_flags & 2) != 0 ? "F" : letter;
            var prefix = p.vga_resolution_selector == 1 ? "M" : "H";
            RequireLevel2(p.map_type.name == new[] { "Day", "Night", "Cave" }[map] &&
                IsLowerSha256(p.campaign_package_sha256) && IsLowerSha256(p.owner_export_package_sha256) &&
                p.palette == $"PAL{display}-0" && p.tables == $"TABLES{letter}" &&
                p.colors == $"CLR{letter}-0" &&
                p.blocks == (map == 0 ? "BLOCK32" : $"BL32{display}0-0") &&
                p.dormant_blocks16 == (map == 0 ? "BLOCK16" : $"BL16{display}0-0") &&
                (map == 2 ? string.IsNullOrEmpty(p.sky) : p.sky == $"SKY{letter}0-0") &&
                p.sprites == $"{prefix}SPR{letter}0-0" && p.web == $"{prefix}WEB{letter}0-0" &&
                p.tmaps == $"TMAPS{map}-0" && p.key_color_1 == (map == 1 ? 0 : 254) &&
                p.key_color_2 == (map == 0 ? 0 : 255) && p.spec_commit == PinnedRemc2Commit,
                "environment original source selectors");
            var selection = NativeSim.CampaignSelection.Create((ushort)p.external_level, (ushort)p.internal_level, (byte)map);
            var package = NativeCampaignSession.ReadPackageIdentity(packagePath, selection);
            var packageManifest = JsonUtility.FromJson<NightPackageManifest>(File.ReadAllText(Path.Combine(package.PackagePath, "manifest.json")));
            RequireLevel2(packageManifest?.package_sha256 == p.campaign_package_sha256,
                "environment campaign package binding");
            var authored = File.ReadAllBytes(Path.Combine(package.PackagePath, "authored-level.bin"));
            RequireLevel2(authored.Length == 0x6604 && authored[4] == p.terrain_graphics_flags,
                "environment actual package graphics flags");
            var rows = ValidateLevel2OutputRows(root, manifest.outputs.OrderBy(row => row.path, StringComparer.Ordinal).ToArray(),
                manifest.cache_content_sha256, "environment", true);
            RequireLevel2(manifest.source_files != null && manifest.source_files.All(source => source != null &&
                IsLowerSha256(source.content_address?.Replace("sha256:", ""))), "environment source identities");
            var sources = manifest.source_files.ToDictionary(source => source.asset_id, StringComparer.Ordinal);
            foreach (var row in rows.Values)
                RequireLevel2(row.source_asset_ids?.Length > 0 && row.source_asset_ids.All(id =>
                    sources.ContainsKey(id.Split('#')[0])), "environment product source references");
            var raw = new Dictionary<string, string> {
                ["palette/source.rgb6"] = p.palette, ["palette/tables.u8"] = p.tables,
                ["palette/colors.u8"] = p.colors, ["terrain/blocks.atlas.u8"] = p.blocks,
                ["terrain/source16.dat"] = p.dormant_blocks16
            };
            if (map != 2) raw.Add("sky/source.u8", p.sky);
            if (map == 0) raw.Add("palette/day-exit-gtd2.dat", "GTD2");
            foreach (var pair in raw)
            {
                var sourceId = $"owner_data:DATA/{pair.Value}.DAT";
                RequireLevel2(rows.TryGetValue(pair.Key, out var row) && sources.TryGetValue(sourceId, out var source) &&
                    row.source_asset_ids.SequenceEqual(new[] { sourceId }) &&
                    source.content_address == "sha256:" + row.sha256 && source.size == row.bytes,
                    "environment selected owner bank " + pair.Value);
            }
            var cache = new OwnerAssetCache(root, manifest.cache_content_sha256, rows.Count,
                new HashSet<string>(rows.Keys, StringComparer.Ordinal),
                environmentOutputHashes: rows.ToDictionary(pair => pair.Key, pair => pair.Value.sha256, StringComparer.Ordinal));
            var descriptor = JsonUtility.FromJson<EnvironmentDescriptor>(cache.ReadOutputText("materials/descriptor.json"));
            RequireLevel2(descriptor?.schema == "mc2-environment-materials/v1" &&
                JsonUtility.ToJson(descriptor.profile) == JsonUtility.ToJson(p), "environment descriptor/profile");
            RequireLevel2(descriptor.palette?.path == "palette/source.rgb6" && descriptor.palette.rgb8_path == "palette/source.rgb8" &&
                descriptor.tables?.path == "palette/tables.u8" && descriptor.colors?.path == "palette/colors.u8" &&
                descriptor.blocks?.atlas_path == "terrain/blocks.atlas.u8" && descriptor.blocks.width == 256 &&
                descriptor.blocks.height == 608 && descriptor.blocks.tile_count == 152 && descriptor.blocks.tile_size == 32 &&
                descriptor.tables.key_color_1 == p.key_color_1 && descriptor.tables.key_color_2 == p.key_color_2,
                "environment descriptor resources");
            var rgb6 = cache.ReadOutput(descriptor.palette.path);
            var rgb8 = cache.ReadOutput(descriptor.palette.rgb8_path);
            RequireLevel2(rgb6.Length == 768 && rgb8.Length == 768 &&
                rgb6.Select((value, index) => value <= 63 && rgb8[index] == value * 4).All(value => value) &&
                cache.ReadOutput(descriptor.tables.path).Length == 83456 &&
                cache.ReadOutput(descriptor.colors.path).Length == 4096 &&
                cache.ReadOutput(descriptor.blocks.atlas_path).Length == 155648,
                "environment resource extents and palette");
            if (map == 2)
            {
                // Unity's inline serializer materializes null classes as
                // empty objects and null strings as empty strings. Validate
                // resource absence before normalizing to the domain's no-sky.
                RequireLevel2((descriptor.sky == null ||
                    string.IsNullOrEmpty(descriptor.sky.path) &&
                    string.IsNullOrEmpty(descriptor.sky.rgb8_path) &&
                    string.IsNullOrEmpty(descriptor.sky.atlas_path) &&
                    descriptor.sky.width == 0 && descriptor.sky.height == 0 &&
                    descriptor.sky.tile_size == 0 && descriptor.sky.tile_count == 0 &&
                    descriptor.sky.key_color_1 == 0 && descriptor.sky.key_color_2 == 0) &&
                    !rows.Keys.Any(relative => relative.StartsWith("sky/", StringComparison.Ordinal)),
                    "Cave environment has no sky resource");
                descriptor.sky = null;
                descriptor.profile.sky = null;
            }
            else
                RequireLevel2(descriptor.sky?.path == "sky/source.u8" && descriptor.sky.width == 256 &&
                    descriptor.sky.height == 256 && cache.ReadOutput(descriptor.sky.path).Length == 65536,
                    "environment sky policy");
            RequireLevel2(descriptor.tmaps?.bank == p.tmaps && descriptor.tmaps.textures?.Length == 504,
                "environment complete TMAP bank");
            for (var index = 0; index < descriptor.tmaps.textures.Length; index++)
            {
                var row = descriptor.tmaps.textures[index];
                RequireLevel2(row.index == index, "environment TMAP order");
                var decoded = cache.ReadOutput(row.decoded_payload_path);
                if (row.source_empty)
                    RequireLevel2(decoded.SequenceEqual(new byte[] { 48 }) && row.indices_path == null,
                        "environment empty TMAP");
                else
                    RequireLevel2(row.width > 0 && row.height > 0 && decoded.Length >= 6 + checked(row.width * row.height) &&
                        BitConverter.ToUInt16(decoded, 2) == row.width && BitConverter.ToUInt16(decoded, 4) == row.height &&
                        decoded.Skip(6).Take(row.width * row.height).SequenceEqual(cache.ReadOutput(row.indices_path)),
                        "environment TMAP pixels/bounds");
            }
            RequireLevel2(descriptor.bitmap_banks?.Length == 3 &&
                new HashSet<string>(descriptor.bitmap_banks.Select(bank => bank.bank)).SetEquals(new[] { p.sprites, p.web, "FONT1" }),
                "environment bitmap banks");
            foreach (var bank in descriptor.bitmap_banks)
                for (var index = 0; index < bank.bitmaps.Length; index++)
                {
                    var row = bank.bitmaps[index];
                    RequireLevel2(row.selector == index, "environment bitmap order");
                    if (row.source_empty) { RequireLevel2(row.width == 0 || row.height == 0, "environment empty bitmap"); continue; }
                    var extent = checked(row.width * row.height);
                    var mask = cache.ReadOutput(row.mask_path);
                    RequireLevel2(extent > 0 && cache.ReadOutput(row.indices_path).Length == extent &&
                        mask.Length == extent && mask.All(value => value <= 1), "environment bitmap extent/coverage");
                }
            var cursor = descriptor.cursor;
            var hasPointerSource = sources.ContainsKey("owner_data:DATA/POINTERS.DAT");
            // Legacy caches have no POINTERS source. New generations must
            // retain and bind the selected hardware cursor as a unit.
            if (hasPointerSource)
            {
                var selector = new[] { 1, 9, 10 }[map];
                var asset = $"owner_data:DATA/POINTERS.DAT#{selector}";
                var indicesPath = $"cursor/pointers/{selector:000}.indices.u8";
                var maskPath = $"cursor/pointers/{selector:000}.mask.u8";
                RequireLevel2(cursor != null && cursor.selector == selector && cursor.width > 0 && cursor.height > 0 &&
                    cursor.indices_path == indicesPath && cursor.mask_path == maskPath &&
                    rows.TryGetValue(indicesPath, out var indicesRow) && indicesRow.source_asset_ids.SequenceEqual(new[] { asset }) &&
                    rows.TryGetValue(maskPath, out var maskRow) && maskRow.source_asset_ids.SequenceEqual(new[] { asset }),
                    "environment original pointer binding");
                var extent = checked(cursor.width * cursor.height);
                var mask = cache.ReadOutput(maskPath);
                RequireLevel2(cache.ReadOutput(indicesPath).Length == extent && mask.Length == extent &&
                    mask.All(value => value <= 1), "environment original pointer extent/coverage");
            }
            else
                RequireLevel2(cursor == null || cursor.width == 0 && cursor.height == 0 &&
                    string.IsNullOrEmpty(cursor.indices_path) && string.IsNullOrEmpty(cursor.mask_path),
                    "legacy environment has no pointer binding");
            cache.EnvironmentMaterials = descriptor;
            cache.EnvironmentPackage = package;
            return cache;
        }
        private readonly HashSet<string> approvedPaths;
        private readonly Dictionary<string, string> environmentOutputHashes;

        private OwnerAssetCache(
            string rootPath,
            string contentHash,
            int outputCount,
            HashSet<string> approvedPaths,
            Level2DayHudDescriptor dayHud = null,
            Level3MaterialDescriptor level3Materials = null,
            bool isLevel3Aggregate = false,
            Dictionary<string, string> environmentOutputHashes = null)
        {
            RootPath = rootPath;
            ContentHash = contentHash;
            OutputCount = outputCount;
            DayHud = dayHud;
            Level3Materials = level3Materials;
            IsDayAggregate = isLevel3Aggregate;
            this.approvedPaths = approvedPaths;
            this.environmentOutputHashes = environmentOutputHashes;
        }

        internal bool HasOutput(string relative)
        {
            return relative != null &&
                approvedPaths.Contains(relative.Replace(Path.DirectorySeparatorChar, '/'));
        }

        public byte[] ReadOutput(string relative)
        {
            var canonical = relative.Replace(Path.DirectorySeparatorChar, '/');
            if (!approvedPaths.Contains(canonical))
                throw new InvalidDataException($"path is not a validated cache output: {relative}");
            var bytes = File.ReadAllBytes(ConfinedPath(RootPath, canonical));
            if (environmentOutputHashes != null && Sha256Bytes(bytes) != environmentOutputHashes[canonical])
                throw new InvalidDataException($"environment output changed after validation: {relative}");
            return bytes;
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

        internal byte[] ReadLevel3GameplayOutput(string relative)
        {
            if (!IsDayAggregate)
                throw new InvalidDataException("cache is not the qualified Level 3 aggregate");
            var canonical = relative?.Replace(Path.DirectorySeparatorChar, '/');
            ValidateLevel2RelativePath(canonical, "Level 3 gameplay output path");
            var aggregatePath = $"components/gameplay/{canonical}";
            RequireLevel2(
                approvedPaths.Contains(aggregatePath),
                $"Level 3 gameplay output membership {relative}");
            return File.ReadAllBytes(ConfinedPath(RootPath, aggregatePath));
        }

        internal byte[] ReadLevel3MaterialOutput(string relative)
        {
            if (!IsDayAggregate)
                throw new InvalidDataException("cache is not the qualified Level 3 aggregate");
            var canonical = relative?.Replace(Path.DirectorySeparatorChar, '/');
            ValidateLevel2RelativePath(canonical, "Level 3 material output path");
            var aggregatePath = $"components/material/{canonical}";
            RequireLevel2(
                approvedPaths.Contains(aggregatePath),
                $"Level 3 material output membership {relative}");
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
            string expectedClosureHash,
            string campaignPackagePath = null)
        {
            var root = Path.GetFullPath(rootPath);
            var manifestPath = ConfinedPath(root, "manifest.json");
            if ((File.GetAttributes(root) & FileAttributes.ReparsePoint) != 0)
                throw new InvalidDataException("owner asset cache root is a symlink");
            if (!File.Exists(manifestPath))
                throw new InvalidDataException("owner asset cache manifest is missing");
            if ((File.GetAttributes(manifestPath) & FileAttributes.ReparsePoint) != 0)
                throw new InvalidDataException("owner asset cache manifest is a symlink");
            if (new FileInfo(manifestPath).Length > 16 * 1024 * 1024)
                throw new InvalidDataException("owner asset cache manifest exceeds its bounded extent");
            Manifest manifest;
            try { manifest = JsonUtility.FromJson<Manifest>(File.ReadAllText(manifestPath)); }
            catch (ArgumentException error)
            { throw new InvalidDataException("owner asset cache manifest is malformed", error); }
            var currentNight = manifest?.schema == NightSchema;
            if (manifest == null || !currentNight && manifest.schema != Schema)
                throw new InvalidDataException("unsupported owner asset cache schema");
            if (currentNight)
                ValidateNightMetadata(manifest, expectedPackageHash);
            else if (Sha256File(manifestPath) != ExactManifestFileHash)
                throw new InvalidDataException("owner asset cache manifest metadata mismatch");
            if (manifest.bindings == null ||
                (currentNight ? manifest.bindings.qualification_level_export_package_sha256 :
                    manifest.bindings.level_export_package_sha256) != expectedPackageHash)
                throw new InvalidDataException("owner asset cache package binding mismatch");
            if (manifest.bindings.level_asset_closure_sha256 !=
                (currentNight ? CurrentNightClosureHash : expectedClosureHash))
                throw new InvalidDataException("owner asset cache closure binding mismatch");
            if (manifest.bindings.plan29_contract_file_sha256 != ExactContractFileHash ||
                manifest.bindings.plan29_contract_self_sha256 != ExactContractSelfHash)
                throw new InvalidDataException("owner asset cache PLAN29 contract binding mismatch");
            string selectedNightContentHash = null;
            if (campaignPackagePath != null)
            {
                var packageRoot = Path.GetFullPath(campaignPackagePath);
                var package = JsonUtility.FromJson<NightPackageManifest>(
                    File.ReadAllText(ConfinedPath(packageRoot, "manifest.json")));
                if (package == null ||
                    currentNight && (package.package_sha256 != manifest.bindings.campaign_package_sha256 ||
                        package.owner_export_package_sha256 != manifest.bindings.level_export_package_sha256) ||
                    Sha256File(ConfinedPath(packageRoot, "runtime.bin")) != manifest.bindings.qualified_runtime_capture_sha256)
                    throw new InvalidDataException("Night cache does not bind the selected campaign package");
                if (currentNight)
                    selectedNightContentHash = package.schema switch
                    {
                        "mc2-owner-campaign-package/v5" => CurrentNightContentHash,
                        "mc2-owner-campaign-package/v6" => SourceClosedNightContentHash,
                        _ => throw new InvalidDataException("Night cache selected campaign profile is unsupported")
                    };
            }
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
            if (currentNight && selectedNightContentHash != null)
            {
                if (identityHash != selectedNightContentHash)
                    throw new InvalidDataException("Night cache content does not match the selected campaign profile");
            }
            // Package-free callers inspect either independently qualified
            // cache; executing consumers above bind one exact variant.
            else if (currentNight
                ? identityHash != CurrentNightContentHash && identityHash != SourceClosedNightContentHash
                : identityHash != ExactContentHash)
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

        private static void ValidateNightMetadata(Manifest manifest, string qualifiedExport)
        {
            var b = manifest.bindings;
            if (manifest.scope != "PRIVATE_TRACK_P_OWNER_DATA" || b == null ||
                b.qualification_level_export_package_sha256 != qualifiedExport ||
                b.qualification_runtime_capture_sha256 != QualifiedNightRuntimeHash ||
                b.runtime_visual_projection_sha256 != QualifiedNightRuntimeHash ||
                b.runtime_visual_projection_schema != "mc2-runtime-state/v1" ||
                b.qualified_runtime_capture_count != 8 ||
                b.owner_catalog_sha256 != Level2CatalogHash ||
                b.owner_catalog_file_sha256 != Level2CatalogFileHash ||
                b.goat_tmap_golden_file_sha256 != "a5c3d430ba22c3503453124d76119ed7b93ff6ea8445e23cd616faa1015b5f64" ||
                b.goat_tmap_oracle_manifest_sha256 != "cad16d830ece1f6422ec4f3f6a0a5ab4954fe6c20344e2e03c6ce1f0bc41e73b")
                throw new InvalidDataException("Night cache visual qualification mismatch");

            // The raw capture belongs to the current package. Its independently
            // qualified visual projection is a different identity, never an alias.
            var fields = new SortedDictionary<string, string>(StringComparer.Ordinal) {
                { "campaign_package_sha256", b.campaign_package_sha256 },
                { "goat_tmap_golden_file_sha256", b.goat_tmap_golden_file_sha256 },
                { "goat_tmap_oracle_manifest_sha256", b.goat_tmap_oracle_manifest_sha256 },
                { "level_asset_closure_sha256", b.level_asset_closure_sha256 },
                { "level_export_package_sha256", b.level_export_package_sha256 },
                { "owner_catalog_file_sha256", b.owner_catalog_file_sha256 },
                { "owner_catalog_sha256", b.owner_catalog_sha256 },
                { "plan29_contract_file_sha256", b.plan29_contract_file_sha256 },
                { "plan29_contract_self_sha256", b.plan29_contract_self_sha256 },
                { "qualification_level_export_package_sha256", b.qualification_level_export_package_sha256 },
                { "qualification_runtime_capture_sha256", b.qualification_runtime_capture_sha256 },
                { "qualified_runtime_capture_count", "8" },
                { "qualified_runtime_capture_sha256", b.qualified_runtime_capture_sha256 },
                { "runtime_visual_projection_schema", b.runtime_visual_projection_schema },
                { "runtime_visual_projection_sha256", b.runtime_visual_projection_sha256 }
            };
            var metadata = new StringBuilder(NightSchema).Append('\n');
            foreach (var field in fields)
            {
                if (field.Key.EndsWith("_sha256", StringComparison.Ordinal) && !IsLowerSha256(field.Value))
                    throw new InvalidDataException("Night cache binding has an invalid digest: " + field.Key);
                metadata.Append(field.Key).Append('\0').Append(field.Value).Append('\n');
            }
            if (!IsLowerSha256(manifest.cache_content_sha256))
                throw new InvalidDataException("Night cache content digest is invalid");
            metadata.Append("cache_content_sha256\0").Append(manifest.cache_content_sha256).Append('\n');
            if (Sha256Bytes(Encoding.ASCII.GetBytes(metadata.ToString())) != manifest.night_metadata_sha256)
                throw new InvalidDataException("Night cache metadata identity mismatch");
        }

        private static bool IsLowerSha256(string value) => value?.Length == 64 &&
            value.All(c => c is >= '0' and <= '9' or >= 'a' and <= 'f');

        public static OwnerAssetCache OpenAndValidateLevel2(string rootPath)
        {
            var root = Path.GetFullPath(rootPath);
            RequireLevel2Directory(root, "aggregate root");
            var manifestPath = ConfinedPath(root, "manifest.json");
            RequireLevel2File(manifestPath, "aggregate manifest");
            var manifestText = File.ReadAllText(manifestPath);
            var manifest = JsonUtility.FromJson<Level2AggregateManifest>(manifestText);
            if (manifest?.schema == "mc2-day-owner-cache/v3")
                return OpenAndValidateDayV3(root, 2);
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

        public static OwnerAssetCache OpenAndValidateLevel3(string rootPath)
        {
            var root = Path.GetFullPath(rootPath);
            RequireLevel2Directory(root, "Level 3 aggregate root");
            var manifestPath = ConfinedPath(root, "manifest.json");
            RequireLevel2File(manifestPath, "Level 3 aggregate manifest");
            var manifest = JsonUtility.FromJson<Level2AggregateManifest>(
                File.ReadAllText(manifestPath));
            if (manifest?.schema == "mc2-day-owner-cache/v3")
                return OpenAndValidateDayV3(root, 3);
            var minimapSymbols = manifest != null && manifest.schema == "mc2-day-owner-cache/v3";
            var correctedBlocks = minimapSymbols || manifest != null && manifest.schema == "mc2-day-owner-cache/v2";
            RequireLevel2(
                Sha256File(manifestPath) == DayReceipt(Level3AggregateManifestHash, correctedBlocks, minimapSymbols),
                "exact Level 3 aggregate manifest identity");
            RequireLevel2(
                manifest != null && manifest.schema == DayReceipt(Level3AggregateSchema, correctedBlocks, minimapSymbols),
                "Level 3 aggregate schema");
            ValidateLevel3Profile(manifest.profile, "Level 3 aggregate profile", correctedBlocks);
            ValidateLevel3Bindings(manifest.bindings, "Level 3 aggregate bindings", correctedBlocks: correctedBlocks);
            RequireLevel2(
                manifest.components != null &&
                manifest.components.material != null &&
                manifest.components.gameplay != null &&
                manifest.components.audio != null,
                "exact Level 3 aggregate component set");
            RequireLevel2(
                manifest.cache_content_sha256 == DayReceipt(Level3AggregateContentHash, correctedBlocks, minimapSymbols) &&
                manifest.outputs != null && manifest.outputs.Length == (minimapSymbols ? 694 : 662),
                "exact Level 3 aggregate content identity and output count");

            var aggregateRows = ValidateLevel2OutputRows(
                root,
                manifest.outputs,
                DayReceipt(Level3AggregateContentHash, correctedBlocks, minimapSymbols),
                "Level 3 aggregate",
                true);
            ValidateLevel3Component(
                root,
                "material",
                manifest.components.material,
                DayReceipt(Level3MaterialSchema, correctedBlocks, minimapSymbols),
                DayReceipt(Level3MaterialManifestHash, correctedBlocks, minimapSymbols),
                DayReceipt(Level3MaterialContentHash, correctedBlocks, minimapSymbols),
                minimapSymbols ? 365 : 333,
                aggregateRows, correctedBlocks);
            var gameplayRows = ValidateLevel3Component(
                root,
                "gameplay",
                manifest.components.gameplay,
                Level3GameplaySchema,
                DayReceipt(Level3GameplayManifestHash, correctedBlocks),
                DayReceipt(Level3GameplayContentHash, correctedBlocks),
                291,
                aggregateRows, correctedBlocks);
            ValidateLevel3Component(
                root,
                "audio",
                manifest.components.audio,
                Level3AudioSchema,
                DayReceipt(Level3AudioManifestHash, correctedBlocks),
                DayReceipt(Level3AudioContentHash, correctedBlocks),
                35,
                aggregateRows, correctedBlocks);
            var materials = ValidateLevel3MaterialDescriptor(root, aggregateRows, correctedBlocks, minimapSymbols);
            var hud = ValidateLevel3GameplayDescriptor(root, gameplayRows, correctedBlocks);
            return new OwnerAssetCache(
                root,
                DayReceipt(Level3AggregateContentHash, correctedBlocks, minimapSymbols),
                aggregateRows.Count,
                new HashSet<string>(aggregateRows.Keys, StringComparer.Ordinal),
                hud,
                materials,
                true);
        }

        // One v3 reader for both qualified Day packages. The exact root receipt
        // authenticates child receipts; all child files and ledger links are
        // still checked. No client-derived gameplay or level-specific decoding.
        private static OwnerAssetCache OpenAndValidateDayV3(string root, int externalLevel)
        {
            var expectedHash = externalLevel == 2
                ? "37aae643c40dbf41d51d79778b04ce6db398bb84e6a95f5fd9b4a6a307eaf5ff"
                : externalLevel == 3
                    ? "3a4f6e113ecd8bf489f5f1ca8ba636f7e065b6166253b11c38444fac5617539a"
                    : throw new InvalidOperationException("Unqualified Day package");
            var path = ConfinedPath(root, "manifest.json");
            // GOLDEN-CHANGE (owner approved): source-rooted spell/creature
            // constructors add TMAP 463. Keep the previous qualified receipts;
            // neither gameplay/replay goldens nor child-file checks change.
            var spellCompleteHash = externalLevel == 2
                ? "698567ad73184b44db05c85ce72238b951f294590b591168d179c2758da7c91b"
                : "0d19967b7964f0f97705ae4b69fd160de7586605f0d15e795197bc30ea1b4be6";
            var actualHash = Sha256File(path);
            // Same gameplay/material assets, corrected source Day bank0/music2.
            var sourceAudioHash = externalLevel == 2
                ? "30bfc6254eeb540f90f64ef68a75efe5c48d3c76694efac288fa36d26330e979"
                : "fcada666ffca239996e78a6589188bfca8ec15491540fc701dd56b4b36f5711e";
            // The source-closed Level2 export adds house successors and their
            // collision records. Its fresh aggregate retains the same source
            // audio and TMAP census, with exact new owner/package receipts.
            var successorClosure = externalLevel == 2 && actualHash ==
                "5ce86a1a3fef270aab20e51a6de3e2cc805d8efbdf9c96c3a2a2e6e29a4fc5db";
            var spellComplete = actualHash == spellCompleteHash || actualHash == sourceAudioHash || successorClosure;
            RequireLevel2(actualHash == expectedHash || spellComplete, "exact Day v3 aggregate identity");
            var manifest = JsonUtility.FromJson<Level2AggregateManifest>(File.ReadAllText(path));
            RequireLevel2(manifest.schema == "mc2-day-owner-cache/v3" &&
                manifest.profile.external_level == externalLevel &&
                manifest.profile.internal_level == externalLevel - 1 &&
                manifest.profile.map_type.id == 0 && manifest.profile.selectors.block_bank == "BLOCK32",
                "Day v3 package and corrected bank binding");
            var rows = ValidateLevel2OutputRows(root, manifest.outputs,
                manifest.cache_content_sha256, "Day v3 aggregate", true);
            Dictionary<string, Level2Output> gameplayRows = null;
            foreach (var name in new[] { "material", "gameplay", "audio" })
            {
                var summary = name == "material" ? manifest.components.material
                    : name == "gameplay" ? manifest.components.gameplay : manifest.components.audio;
                var relative = $"components/{name}/manifest.json";
                RequireLevel2(summary != null && summary.manifest_path == relative &&
                    Sha256File(ConfinedPath(root, relative)) == summary.manifest_sha256,
                    $"Day v3 {name} manifest receipt");
                var child = JsonUtility.FromJson<Level2ComponentManifest>(File.ReadAllText(ConfinedPath(root, relative)));
                RequireLevel2(child.schema == summary.schema &&
                    child.cache_content_sha256 == summary.cache_content_sha256 &&
                    child.outputs.Length == summary.output_count &&
                    JsonUtility.ToJson(child.profile) == JsonUtility.ToJson(manifest.profile),
                    $"Day v3 {name} profile and content binding");
                var childRows = ValidateLevel2OutputRows(ConfinedPath(root, $"components/{name}"),
                    child.outputs, summary.cache_content_sha256, $"Day v3 {name}", false);
                foreach (var pair in childRows)
                    RequireLevel2(rows.TryGetValue($"components/{name}/{pair.Key}", out var aggregate) &&
                        aggregate.component == name && aggregate.bytes == pair.Value.bytes &&
                        aggregate.sha256 == pair.Value.sha256, $"Day v3 {name} output membership");
                if (name == "gameplay") gameplayRows = childRows;
            }
            var materialPath = "components/material/materials/descriptor.json";
            RequireLevel2(rows.ContainsKey(materialPath), "Day v3 material descriptor membership");
            var material = JsonUtility.FromJson<Level3MaterialDescriptor>(File.ReadAllText(ConfinedPath(root, materialPath)));
            RequireLevel2(material.schema == "mc2-day-materials/v3" &&
                JsonUtility.ToJson(material.profile) == JsonUtility.ToJson(manifest.profile),
                "Day v3 material descriptor identity");
            ValidateDayMaterialContents(material, rows, true,
                (externalLevel == 2 ? 156 : 162) + (spellComplete ? 1 : 0));
            const string gameplayPath = "gameplay/descriptor.json";
            RequireLevel2(gameplayRows.ContainsKey(gameplayPath), "Day v3 gameplay descriptor membership");
            var gameplay = JsonUtility.FromJson<Level2GameplayDescriptor>(File.ReadAllText(
                ConfinedPath(root, $"components/gameplay/{gameplayPath}")));
            RequireLevel2(gameplay.schema == Level3GameplayDescriptorSchema &&
                JsonUtility.ToJson(gameplay.profile) == JsonUtility.ToJson(manifest.profile),
                "Day v3 gameplay descriptor identity");
            var hud = ValidateDayGameplayContents(gameplay, gameplayRows);
            return new OwnerAssetCache(root, manifest.cache_content_sha256, rows.Count,
                new HashSet<string>(rows.Keys, StringComparer.Ordinal), hud, material, true);
        }

        private static Dictionary<string, Level2Output> ValidateLevel3Component(
            string root,
            string name,
            Level2ComponentSummary summary,
            string expectedSchema,
            string expectedManifestHash,
            string expectedContentHash,
            int expectedOutputCount,
            Dictionary<string, Level2Output> aggregateRows,
            bool correctedBlocks)
        {
            var manifestRelative = $"components/{name}/manifest.json";
            RequireLevel2(
                summary != null &&
                summary.schema == expectedSchema &&
                summary.manifest_path == manifestRelative &&
                summary.manifest_sha256 == expectedManifestHash &&
                summary.cache_content_sha256 == expectedContentHash &&
                summary.output_count == expectedOutputCount,
                $"exact Level 3 {name} component receipt");
            var childRoot = ConfinedPath(root, $"components/{name}");
            RequireLevel2Directory(childRoot, $"Level 3 {name} component root");
            var childManifestPath = ConfinedPath(root, manifestRelative);
            RequireLevel2File(childManifestPath, $"Level 3 {name} manifest");
            RequireLevel2(
                Sha256File(childManifestPath) == expectedManifestHash,
                $"Level 3 {name} manifest hash");
            var child = JsonUtility.FromJson<Level2ComponentManifest>(
                File.ReadAllText(childManifestPath));
            RequireLevel2(
                child != null && child.schema == expectedSchema,
                $"Level 3 {name} child schema");
            ValidateLevel3Profile(child.profile, $"Level 3 {name} child profile", correctedBlocks);
            ValidateLevel3Bindings(
                child.bindings,
                $"Level 3 {name} child bindings",
                name == "audio", correctedBlocks);
            RequireLevel2(
                child.cache_content_sha256 == expectedContentHash &&
                child.outputs != null && child.outputs.Length == expectedOutputCount,
                $"Level 3 {name} child content identity");
            var childRows = ValidateLevel2OutputRows(
                childRoot,
                child.outputs,
                expectedContentHash,
                $"Level 3 {name} child",
                false);
            foreach (var pair in childRows)
            {
                var aggregatePath = $"components/{name}/{pair.Key}";
                RequireLevel2(
                    aggregateRows.TryGetValue(aggregatePath, out var aggregateRow) &&
                    aggregateRow.bytes == pair.Value.bytes &&
                    aggregateRow.sha256 == pair.Value.sha256 &&
                    aggregateRow.component == name,
                    $"Level 3 {name} aggregate membership {pair.Key}");
            }
            return childRows;
        }

        private static Level3MaterialDescriptor ValidateLevel3MaterialDescriptor(
            string root,
            Dictionary<string, Level2Output> aggregateRows,
            bool correctedBlocks, bool minimapSymbols)
        {
            const string relative = "materials/descriptor.json";
            var aggregateRelative = $"components/material/{relative}";
            RequireLevel2(
                aggregateRows.TryGetValue(aggregateRelative, out var row) &&
                row.sha256 == DayReceipt(Level3MaterialDescriptorHash, correctedBlocks, minimapSymbols),
                "exact Level 3 material descriptor receipt");
            var descriptor = JsonUtility.FromJson<Level3MaterialDescriptor>(
                File.ReadAllText(ConfinedPath(root, aggregateRelative)));
            RequireLevel2(
                descriptor != null &&
                descriptor.schema == DayReceipt(Level3MaterialDescriptorSchema, correctedBlocks, minimapSymbols),
                "Level 3 material descriptor schema");
            ValidateLevel3Profile(descriptor.profile, "Level 3 material descriptor profile", correctedBlocks);
            return ValidateDayMaterialContents(descriptor, aggregateRows, minimapSymbols, 162);
        }

        private static Level3MaterialDescriptor ValidateDayMaterialContents(
            Level3MaterialDescriptor descriptor, Dictionary<string, Level2Output> aggregateRows,
            bool minimapSymbols, int textureCount)
        {
            RequireLevel2(
                descriptor.palette != null &&
                descriptor.palette.entries == 256 &&
                descriptor.palette.rgb8_path == "palette/pald.rgb8" &&
                descriptor.tmaps0 != null &&
                descriptor.tmaps0.bank == "TMAPS0-0" &&
                descriptor.tmaps0.palette_asset_id == "owner_data:DATA/PALD-0.DAT" &&
                descriptor.tmaps0.selectors != null &&
                descriptor.tmaps0.textures != null &&
                descriptor.tmaps0.selectors.Length == textureCount &&
                descriptor.tmaps0.textures.Length == textureCount,
                "Level 3 material selector census");
            var selectors = new HashSet<int>();
            foreach (var texture in descriptor.tmaps0.textures)
            {
                RequireLevel2(
                    texture != null &&
                    selectors.Add(texture.index) &&
                    texture.index >= 0 && texture.index <= ushort.MaxValue &&
                    texture.width > 0 && texture.height > 0 &&
                    texture.frame_count > 0 &&
                    !string.IsNullOrEmpty(texture.indices_path) &&
                    aggregateRows.TryGetValue(
                        $"components/material/{texture.indices_path}",
                        out var textureRow) &&
                    textureRow.bytes == (long)texture.width * texture.height,
                    "Level 3 material texture membership and bounds");
            }
            RequireLevel2(
                selectors.SetEquals(descriptor.tmaps0.selectors) &&
                new[] { 110, 111, 112, 113, 114, 115 }.All(selectors.Contains),
                "Level 3 material texture/selector equality");
            if (minimapSymbols)
            {
                var bank = descriptor.minimap_symbols;
                RequireLevel2(bank != null && bank.schema == "mc2-minimap-symbols/v1" &&
                    bank.bank == "MSPRD0-0" && bank.palette_asset_id == "owner_data:DATA/PALD-0.DAT" &&
                    bank.selectors != null && bank.selectors.SequenceEqual(Enumerable.Range(58, 16)) &&
                    bank.symbols != null && bank.symbols.Length == 16,
                    "original Day minimap symbol census");
                for (var index = 0; index < bank.symbols.Length; index++)
                {
                    var symbol = bank.symbols[index];
                    RequireLevel2(symbol != null && symbol.selector == 58 + index &&
                        symbol.width > 0 && symbol.height > 0 &&
                        symbol.source_asset_id == $"owner_data:DATA/MSPRD0-0.DAT#{symbol.selector}",
                        "Day minimap symbol source and bounds");
                    foreach (var field in new[] { "indices", "mask" })
                    {
                        var relativePath = $"minimap/msprd0-0/{symbol.selector:000}.{field}.u8";
                        RequireLevel2((field == "indices" ? symbol.indices_path : symbol.mask_path) == relativePath &&
                            aggregateRows.TryGetValue($"components/material/{relativePath}", out var output) &&
                            output.bytes == symbol.width * symbol.height &&
                            output.sha256 == (field == "indices" ? symbol.indices_sha256 : symbol.mask_sha256),
                            "Day minimap symbol output binding");
                    }
                }
            }
            return descriptor;
        }

        private static Level2DayHudDescriptor ValidateLevel3GameplayDescriptor(
            string root,
            Dictionary<string, Level2Output> gameplayRows,
            bool correctedBlocks)
        {
            const string relative = "gameplay/descriptor.json";
            RequireLevel2(
                gameplayRows.TryGetValue(relative, out var row) &&
                row.sha256 == DayReceipt(Level3GameplayDescriptorHash, correctedBlocks),
                "exact Level 3 gameplay descriptor receipt");
            var descriptor = JsonUtility.FromJson<Level2GameplayDescriptor>(
                File.ReadAllText(ConfinedPath(root, $"components/gameplay/{relative}")));
            RequireLevel2(
                descriptor != null && descriptor.schema == Level3GameplayDescriptorSchema,
                "Level 3 gameplay descriptor schema");
            ValidateLevel3Profile(descriptor.profile, "Level 3 gameplay descriptor profile", correctedBlocks);
            return ValidateDayGameplayContents(descriptor, gameplayRows);
        }

        private static Level2DayHudDescriptor ValidateDayGameplayContents(
            Level2GameplayDescriptor descriptor, Dictionary<string, Level2Output> gameplayRows)
        {
            var hud = descriptor.hud;
            RequireLevel2(
                hud != null &&
                hud.schema == "mc2-level2-day-hud/v1" &&
                hud.authority == "SourceMappedLive" &&
                hud.bitmap_selectors != null &&
                hud.bitmap_selectors.Length == Level2HudSelectors.Length &&
                new HashSet<int>(hud.bitmap_selectors).SetEquals(Level2HudSelectors) &&
                hud.bitmap_assets != null &&
                hud.bitmap_assets.Length == Level2HudSelectors.Length &&
                hud.spell_tier_text != null &&
                hud.spell_tier_text.tiers != null &&
                hud.spell_tier_text.tiers.Length == Level2HudTexts.Length &&
                descriptor.hud_outputs != null &&
                descriptor.hud_outputs.Length == 75,
                "exact Level 3 Day HUD, spell, and minimap surface census");
            var ledger = new HashSet<string>(StringComparer.Ordinal);
            foreach (var output in descriptor.hud_outputs)
            {
                RequireLevel2(
                    output != null &&
                    ledger.Add(output.path) &&
                    output.path.StartsWith("gameplay/hud/", StringComparison.Ordinal) &&
                    gameplayRows.TryGetValue(output.path, out var gameplayRow) &&
                    gameplayRow.bytes == output.bytes &&
                    gameplayRow.sha256 == output.sha256,
                    "Level 3 HUD output ledger membership");
            }
            return hud;
        }

        private static void ValidateLevel3Profile(Level2Profile profile, string description, bool correctedBlocks = false)
        {
            RequireLevel2(
                profile != null &&
                profile.name == "day-material" &&
                profile.external_level == 3 &&
                profile.internal_level == 2 &&
                profile.map_type != null &&
                profile.map_type.id == 0 &&
                profile.map_type.name == "Day" &&
                profile.package_sha256 == DayReceipt(Level3OwnerExportPackageHash, correctedBlocks) &&
                profile.owner_export_package_sha256 == DayReceipt(Level3OwnerExportPackageHash, correctedBlocks) &&
                profile.campaign_package_sha256 == DayReceipt(Level3CampaignPackageHash, correctedBlocks) &&
                profile.closure_sha256 == DayReceipt(Level3ClosureHash, correctedBlocks) &&
                profile.catalog_sha256 == Level2CatalogHash &&
                profile.catalog_file_sha256 == Level2CatalogFileHash &&
                profile.spec_commit == PinnedRemc2Commit &&
                profile.owner_package != null &&
                profile.owner_package.manifest_schema == Level2OwnerManifestSchema &&
                profile.owner_package.players_component != null &&
                profile.owner_package.players_component.magic == Level2PlayersComponentMagic &&
                profile.owner_package.players_component.version == Level2PlayersComponentVersion &&
                profile.owner_package.players_component.record_bytes == Level2PlayersComponentRecordBytes &&
                profile.owner_package.players_component.count == Level2PlayersComponentCount &&
                profile.selectors != null &&
                profile.selectors.vga_resolution_selector == 1 &&
                profile.selectors.texture_size == 32 &&
                profile.selectors.terrain_graphics_flags == 0 &&
                !profile.selectors.terrain_graphics_flag_2 &&
                profile.selectors.terrain_bank == "TMAPS0-0" &&
                profile.selectors.sprite_bank == "MSPRD0-0" &&
                profile.selectors.web_bank == "MWEBD0-0" &&
                profile.selectors.block_bank == (correctedBlocks ? "BLOCK32" : "BL32N0-0") &&
                profile.selectors.palette == "PALD-0" &&
                profile.selectors.sky_bank == "SKYD0-0",
                description);
        }

        private static void ValidateLevel3Bindings(
            Level2Bindings bindings,
            string description,
            bool audio = false,
            bool correctedBlocks = false)
        {
            RequireLevel2(
                bindings != null &&
                bindings.level_export_package_sha256 == DayReceipt(Level3OwnerExportPackageHash, correctedBlocks) &&
                bindings.campaign_package_sha256 == DayReceipt(Level3CampaignPackageHash, correctedBlocks) &&
                bindings.level_asset_closure_sha256 == DayReceipt(Level3ClosureHash, correctedBlocks) &&
                bindings.owner_catalog_sha256 == Level2CatalogHash &&
                bindings.owner_catalog_file_sha256 == Level2CatalogFileHash &&
                bindings.owner_manifest_schema == Level2OwnerManifestSchema &&
                bindings.players_component_magic == Level2PlayersComponentMagic &&
                bindings.players_component_version == Level2PlayersComponentVersion &&
                bindings.players_component_record_bytes == Level2PlayersComponentRecordBytes &&
                bindings.players_component_count == Level2PlayersComponentCount &&
                bindings.remc2_commit == PinnedRemc2Commit &&
                (audio
                    ? bindings.audio_denominator_sha256 == Level3AudioDenominatorHash
                    : string.IsNullOrEmpty(bindings.audio_denominator_sha256)),
                description);
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
