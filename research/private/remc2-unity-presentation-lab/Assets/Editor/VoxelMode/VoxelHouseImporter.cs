using System;
using System.Collections.Generic;
using System.Globalization;
using System.IO;
using System.Security.Cryptography;
using System.Text;

namespace MagicCarpet.PresentationLab.VoxelMode
{
    /// <summary>
    /// Editor-only request for importing the qualified private BUILD-29 package.
    /// The owner root is deliberately external to the Unity project.
    /// </summary>
    public sealed class VoxelHouseImportRequest
    {
        public string OwnerRoot { get; set; }
        public string ManifestRelativePath { get; set; } = "exports/build29/voxel-house-manifest-v1.json";
        public string SchemaRelativePath { get; set; } = "contract/voxel-house-manifest-v1.schema.json";
        public string RecipeRelativePath { get; set; } = "exports/build29/surface-recipe-v1.json";
        public string ReceiptRelativePath { get; set; } = "exports/build29/provenance-receipt.json";
        public string QualificationReportRelativePath { get; set; } = "exports/build29/qualification-report.json";
        public string OutputDirectory { get; set; }
    }

    /// <summary>
    /// Result of a deterministic editor import. No Unity GameObjects are made;
    /// cells remain records for later batched surface generation.
    /// </summary>
    public sealed class VoxelHouseImportResult
    {
        internal VoxelHouseImportResult(
            string mappingPath,
            string receiptPath,
            string mappingSha256,
            int cellCount,
            int roomCount,
            int furnitureCount,
            int openingCount)
        {
            MappingPath = mappingPath;
            ReceiptPath = receiptPath;
            MappingSha256 = mappingSha256;
            CellCount = cellCount;
            RoomCount = roomCount;
            FurnitureCount = furnitureCount;
            OpeningCount = openingCount;
        }

        public string MappingPath { get; }
        public string ReceiptPath { get; }
        public string MappingSha256 { get; }
        public int CellCount { get; }
        public int RoomCount { get; }
        public int FurnitureCount { get; }
        public int OpeningCount { get; }
    }

    /// <summary>
    /// Imports one qualified owner-derived private package into a deterministic
    /// editor mapping. This class has no UnityEngine dependency and must remain
    /// under Assets/Editor/VoxelMode so it cannot become runtime gameplay.
    /// </summary>
    public static class VoxelHouseImporter
    {
        private const string ExpectedManifestId = "private.build29.house.v1";
        private const string ExpectedQualification = "private-build29-voxel-package-v1";
        private const string ExpectedSchemaId = "voxel-house-manifest";
        private const string ExpectedSchemaTitle = "Private VoxelHouseManifest v1";
        private const string ExpectedSchemaUri = "https://json-schema.org/draft/2020-12/schema";
        private const string ExpectedSourceEntity = "level1-build29-profile-house";
        private const int MaxManifestBytes = 16 * 1024 * 1024;
        private const int MaxSchemaBytes = 2 * 1024 * 1024;
        private const int MaxRecipeBytes = 4 * 1024 * 1024;
        private const int MaxReceiptBytes = 2 * 1024 * 1024;
        private const int MaxQualificationBytes = 4 * 1024 * 1024;
        private const long MaxProfileEnvelopeCells = 1000000;

        private static readonly string[] BudgetNames =
        {
            "cells",
            "imported_triangles",
            "persistent_game_objects",
            "renderers",
            "materials",
            "collider_proxies",
            "major_clusters",
            "active_cosmetic_debris",
            "sidecar_snapshot_bytes",
            "added_memory_bytes",
            "p95_dirty_rebuild_ms",
            "worst_impact_frame_ms",
            "managed_allocation_per_impact_bytes",
            "steady_frame_allocation_bytes",
            "monotonic_memory_growth_percent",
        };

        private static readonly Dictionary<string, double> FrozenCaps =
            new Dictionary<string, double>(StringComparer.Ordinal)
            {
                ["cells"] = 4096,
                ["imported_triangles"] = 100000,
                ["persistent_game_objects"] = 48,
                ["renderers"] = 24,
                ["materials"] = 16,
                ["collider_proxies"] = 48,
                ["major_clusters"] = 12,
                ["active_cosmetic_debris"] = 256,
                ["sidecar_snapshot_bytes"] = 524288,
                ["added_memory_bytes"] = 134217728,
                ["p95_dirty_rebuild_ms"] = 8,
                ["worst_impact_frame_ms"] = 33,
                ["managed_allocation_per_impact_bytes"] = 262144,
                ["steady_frame_allocation_bytes"] = 1024,
                ["monotonic_memory_growth_percent"] = 5,
            };

        private static readonly HashSet<string> IntegerBudgets =
            new HashSet<string>(StringComparer.Ordinal)
            {
                "cells",
                "imported_triangles",
                "persistent_game_objects",
                "renderers",
                "materials",
                "collider_proxies",
                "major_clusters",
                "active_cosmetic_debris",
                "sidecar_snapshot_bytes",
                "added_memory_bytes",
                "managed_allocation_per_impact_bytes",
                "steady_frame_allocation_bytes",
            };

        // The BUILD-29 contract freezes one-house bounds. These guards keep an
        // accidentally broad or hostile package from becoming a large editor
        // import even when a new field is added to the schema.
        private static readonly Dictionary<string, int> ArrayCaps =
            new Dictionary<string, int>(StringComparer.Ordinal)
            {
                ["materials"] = 16,
                ["parts"] = 64,
                ["cells"] = 4096,
                ["supports"] = 64,
                ["rooms"] = 64,
                ["furniture"] = 256,
                ["openings"] = 128,
                ["render_groups"] = 64,
            };

        private static readonly string[] RequiredEntityArrays =
        {
            "parts",
            "cells",
            "supports",
            "rooms",
            "furniture",
            "openings",
            "render_groups",
        };

        public static VoxelHouseImportResult ImportQualifiedBuild29(VoxelHouseImportRequest request)
        {
            if (request == null) throw new ArgumentNullException(nameof(request));
            var root = RequireAbsoluteDirectory(request.OwnerRoot, "owner_root");
            var manifestPath = ResolveUnderRoot(root, request.ManifestRelativePath, "manifest");
            var schemaPath = ResolveUnderRoot(root, request.SchemaRelativePath, "schema");
            var recipePath = ResolveUnderRoot(root, request.RecipeRelativePath, "recipe");
            var receiptPath = ResolveUnderRoot(root, request.ReceiptRelativePath, "receipt");
            var qualificationPath = ResolveUnderRoot(root, request.QualificationReportRelativePath, "qualification_report");

            var manifestBytes = ReadPackageBytes(manifestPath, "manifest", MaxManifestBytes);
            var schemaBytes = ReadPackageBytes(schemaPath, "schema", MaxSchemaBytes);
            var recipeBytes = ReadPackageBytes(recipePath, "recipe", MaxRecipeBytes);
            var receiptBytes = ReadPackageBytes(receiptPath, "receipt", MaxReceiptBytes);
            var qualificationBytes = ReadPackageBytes(qualificationPath, "qualification_report", MaxQualificationBytes);

            var manifest = VoxelJsonParser.Parse(manifestBytes, "manifest");
            var schema = VoxelJsonParser.Parse(schemaBytes, "schema");
            var recipe = VoxelJsonParser.Parse(recipeBytes, "recipe");
            var receipt = VoxelJsonParser.Parse(receiptBytes, "receipt");
            var qualification = VoxelJsonParser.Parse(qualificationBytes, "qualification_report");

            ValidateSchema(schema, schemaBytes);
            ValidatePackagePaths(root, request, receipt);
            ValidateManifest(manifest, schemaBytes, recipe, receipt, qualification, manifestBytes, recipeBytes);

            ValidateOutputDirectory(request.OutputDirectory);
            var outputDirectory = Path.GetFullPath(request.OutputDirectory);
            Directory.CreateDirectory(outputDirectory);

            var mapping = BuildMapping(manifest);
            var mappingText = VoxelJsonCanonical.Write(mapping) + "\n";
            var mappingBytes = new UTF8Encoding(false).GetBytes(mappingText);
            var mappingSha256 = Sha256(mappingBytes);
            var generatedMappingPath = Path.Combine(outputDirectory, "voxel-house-build29.mapping.json");
            var generatedReceiptPath = Path.Combine(outputDirectory, "voxel-house-build29.mapping.receipt.json");
            File.WriteAllBytes(generatedMappingPath, mappingBytes);

            var generatedReceipt = VoxelJsonValue.Object(new Dictionary<string, VoxelJsonValue>(StringComparer.Ordinal)
            {
                ["mapping_sha256"] = VoxelJsonValue.String(mappingSha256),
                ["mapping"] = VoxelJsonValue.String("voxel-house-build29.mapping.json"),
                ["manifest_sha256"] = VoxelJsonValue.String(Sha256(manifestBytes)),
                ["semantic_sha256"] = VoxelJsonValue.String(StringAt(manifest, "hashes", "semantic_sha256")),
                ["schema_sha256"] = VoxelJsonValue.String(StringAt(manifest, "hashes", "schema_sha256")),
                ["source_entity_id"] = VoxelJsonValue.String(ExpectedSourceEntity),
                ["building_index"] = VoxelJsonValue.Number("29"),
                ["owner_derived_private"] = VoxelJsonValue.Boolean(true),
                ["track_s_resources"] = VoxelJsonValue.Boolean(false),
            });
            File.WriteAllText(generatedReceiptPath, VoxelJsonCanonical.Write(generatedReceipt) + "\n", new UTF8Encoding(false));

            return new VoxelHouseImportResult(
                generatedMappingPath,
                generatedReceiptPath,
                mappingSha256,
                CountArray(manifest, "cells"),
                CountArray(manifest, "rooms"),
                CountArray(manifest, "furniture"),
                CountArray(manifest, "openings"));
        }

        internal static void ValidateOnly(VoxelHouseImportRequest request)
        {
            if (request == null) throw new ArgumentNullException(nameof(request));
            var root = RequireAbsoluteDirectory(request.OwnerRoot, "owner_root");
            var manifestPath = ResolveUnderRoot(root, request.ManifestRelativePath, "manifest");
            var schemaPath = ResolveUnderRoot(root, request.SchemaRelativePath, "schema");
            var recipePath = ResolveUnderRoot(root, request.RecipeRelativePath, "recipe");
            var receiptPath = ResolveUnderRoot(root, request.ReceiptRelativePath, "receipt");
            var qualificationPath = ResolveUnderRoot(root, request.QualificationReportRelativePath, "qualification_report");
            var manifestBytes = ReadPackageBytes(manifestPath, "manifest", MaxManifestBytes);
            var schemaBytes = ReadPackageBytes(schemaPath, "schema", MaxSchemaBytes);
            var recipeBytes = ReadPackageBytes(recipePath, "recipe", MaxRecipeBytes);
            var manifest = VoxelJsonParser.Parse(manifestBytes, "manifest");
            var schema = VoxelJsonParser.Parse(schemaBytes, "schema");
            var recipe = VoxelJsonParser.Parse(recipeBytes, "recipe");
            var receipt = VoxelJsonParser.Parse(ReadPackageBytes(receiptPath, "receipt", MaxReceiptBytes), "receipt");
            var qualification = VoxelJsonParser.Parse(ReadPackageBytes(qualificationPath, "qualification_report", MaxQualificationBytes), "qualification_report");
            ValidateSchema(schema, schemaBytes);
            ValidatePackagePaths(root, request, receipt);
            ValidateManifest(manifest, schemaBytes, recipe, receipt, qualification, manifestBytes, recipeBytes);
        }

        private static void ValidateSchema(VoxelJsonValue schema, byte[] schemaBytes)
        {
            schema.RequireKind(VoxelJsonKind.Object, "schema");
            ExactString(schema, "$schema", ExpectedSchemaUri, "schema.$schema");
            ExactString(schema, "title", ExpectedSchemaTitle, "schema.title");
            ExactString(schema, "type", "object", "schema.type");
            ExactBoolean(schema, "additionalProperties", false, "schema.additionalProperties");
            if (schemaBytes == null || schemaBytes.Length == 0)
            {
                throw new VoxelManifestException("schema", "schema file is empty");
            }
        }

        private static void ValidatePackagePaths(string root, VoxelHouseImportRequest request, VoxelJsonValue receipt)
        {
            var manifest = StringAt(receipt, null, "manifest");
            var recipe = StringAt(receipt, null, "surface_recipe");
            if (!string.Equals(NormalizeRelative(manifest), NormalizeRelative(request.ManifestRelativePath), StringComparison.Ordinal) ||
                !string.Equals(NormalizeRelative(recipe), NormalizeRelative(request.RecipeRelativePath), StringComparison.Ordinal))
            {
                throw new VoxelManifestException("receipt", "manifest/recipe paths do not match the requested private package");
            }

            // Receipt paths are metadata, never authority to escape the root.
            ResolveUnderRoot(root, manifest, "receipt.manifest");
            ResolveUnderRoot(root, recipe, "receipt.surface_recipe");
        }

        private static void ValidateManifest(
            VoxelJsonValue manifest,
            byte[] schemaBytes,
            VoxelJsonValue recipe,
            VoxelJsonValue receipt,
            VoxelJsonValue qualification,
            byte[] manifestBytes,
            byte[] recipeBytes)
        {
            manifest.RequireKind(VoxelJsonKind.Object, "manifest");
            var schema = ObjectAt(manifest, "schema", "manifest.schema");
            ExactString(schema, "id", ExpectedSchemaId, "manifest.schema.id");
            ExactInt(schema, "version", 1, "manifest.schema.version");
            ExactString(manifest, "manifest_id", ExpectedManifestId, "manifest.manifest_id");

            var coordinates = ObjectAt(manifest, "coordinate_system", "manifest.coordinate_system");
            ExactString(coordinates, "unit", "m", "manifest.coordinate_system.unit");
            PositiveNumber(coordinates, "cell_size_m", "manifest.coordinate_system.cell_size_m");
            ExactString(coordinates, "handedness", "right", "manifest.coordinate_system.handedness");
            ExactString(coordinates, "up_axis", "+Y", "manifest.coordinate_system.up_axis");
            ExactString(coordinates, "forward_axis", "+Z", "manifest.coordinate_system.forward_axis");
            ExactString(coordinates, "right_axis", "+X", "manifest.coordinate_system.right_axis");
            var anchor = ObjectAt(coordinates, "anchor", "manifest.coordinate_system.anchor");
            ExactString(anchor, "kind", "source_aabb_center_bottom", "manifest.coordinate_system.anchor.kind");
            var anchorCell = Coord(anchor.Require("cell_coord", "manifest.coordinate_system.anchor"), "manifest.coordinate_system.anchor.cell_coord");
            if (anchorCell[0] != 0 || anchorCell[1] != 0 || anchorCell[2] != 0)
                throw new VoxelManifestException("manifest.coordinate_system.anchor.cell_coord", "BUILD-29 anchor must be [0,0,0]");
            var anchorWorld = Vec3(anchor.Require("world_m", "manifest.coordinate_system.anchor"), "manifest.coordinate_system.anchor.world_m", false);
            if (anchorWorld[0] != 0 || anchorWorld[1] != 0 || anchorWorld[2] != 0)
                throw new VoxelManifestException("manifest.coordinate_system.anchor.world_m", "BUILD-29 anchor world must be [0,0,0]");

            var authority = ObjectAt(manifest, "authority", "manifest.authority");
            ExactString(authority, "gameplay", "source_house_state", "manifest.authority.gameplay");
            ExactString(authority, "detail", "voxel_structure_mirror", "manifest.authority.detail");
            ExactString(authority, "targeting", "source_authoritative", "manifest.authority.targeting");
            ExactString(authority, "collision", "source_aabb", "manifest.authority.collision");
            ExactBoolean(authority, "feedback_to_gameplay", false, "manifest.authority.feedback_to_gameplay");
            ExactBoolean(authority, "standalone_town", false, "manifest.authority.standalone_town");

            var profile = ObjectAt(manifest, "profile", "manifest.profile");
            ValidId(StringAt(profile, null, "structure_id"), "manifest.profile.structure_id");
            NonNegativeInt(profile, "generation", "manifest.profile.generation");
            var bounds = ObjectAt(profile, "cell_bounds", "manifest.profile.cell_bounds");
            var lower = Coord(bounds.Require("min", "manifest.profile.cell_bounds"), "manifest.profile.cell_bounds.min");
            var upper = Coord(bounds.Require("max", "manifest.profile.cell_bounds"), "manifest.profile.cell_bounds.max");
            for (var axis = 0; axis < 3; axis++)
            {
                if (lower[axis] > upper[axis]) throw new VoxelManifestException("manifest.profile.cell_bounds", "minimum exceeds maximum");
            }
            var visualBounds = ObjectAt(profile, "visual_bounds_m", "manifest.profile.visual_bounds_m");
            var visualMin = Vec3(visualBounds.Require("min_m", "manifest.profile.visual_bounds_m"), "manifest.profile.visual_bounds_m.min_m", false);
            var visualMax = Vec3(visualBounds.Require("max_m", "manifest.profile.visual_bounds_m"), "manifest.profile.visual_bounds_m.max_m", false);
            for (var axis = 0; axis < 3; axis++)
            {
                if (visualMin[axis] > visualMax[axis]) throw new VoxelManifestException("manifest.profile.visual_bounds_m", "minimum exceeds maximum");
            }
            PositiveNumber(profile, "source_units_per_cell", "manifest.profile.source_units_per_cell");

            var sourceAabb = ObjectAt(manifest, "source_gameplay_aabb", "manifest.source_gameplay_aabb");
            ExactString(sourceAabb, "source_entity_id", ExpectedSourceEntity, "manifest.source_gameplay_aabb.source_entity_id");
            ExactInt(sourceAabb, "building_index", 29, "manifest.source_gameplay_aabb.building_index");
            Vec3(sourceAabb.Require("center_m", "manifest.source_gameplay_aabb"), "manifest.source_gameplay_aabb.center_m", false);
            Vec3(sourceAabb.Require("half_extents_m", "manifest.source_gameplay_aabb"), "manifest.source_gameplay_aabb.half_extents_m", true);
            ExactString(sourceAabb, "source_space", "mc2_source_units", "manifest.source_gameplay_aabb.source_space");
            ExactString(sourceAabb, "removal_authority", "source_house_action_53", "manifest.source_gameplay_aabb.removal_authority");

            var provenance = ObjectAt(manifest, "provenance", "manifest.provenance");
            ExactString(provenance, "class", "owner-derived-private", "manifest.provenance.class");
            NonEmptyString(provenance, "source_ref", "manifest.provenance.source_ref");
            NonEmptyString(provenance, "recipe_ref", "manifest.provenance.recipe_ref");
            NonEmptyString(provenance, "license_ref", "manifest.provenance.license_ref");
            ExactString(provenance, "interior_authorship", "newly_authored", "manifest.provenance.interior_authorship");

            var hashes = ObjectAt(manifest, "hashes", "manifest.hashes");
            foreach (var hashName in new[] { "source_sha256", "recipe_sha256", "schema_sha256", "asset_sha256", "semantic_sha256" })
            {
                ValidHash(StringAt(hashes, null, hashName), "manifest.hashes." + hashName);
            }

            var expectedSchemaHash = Sha256(schemaBytes);
            ExactString(hashes, "schema_sha256", expectedSchemaHash, "manifest.hashes.schema_sha256");
            ExactString(hashes, "semantic_sha256", DigestWithoutSemanticHash(manifest), "manifest.hashes.semantic_sha256");
            ExactString(hashes, "recipe_sha256", DigestRecipePayload(recipe), "manifest.hashes.recipe_sha256");
            ExactString(hashes, "asset_sha256", DigestAssetPayload(manifest), "manifest.hashes.asset_sha256");

            var cells = EntityArray(manifest, "cells");
            var materials = EntityArray(manifest, "materials");
            var parts = EntityArray(manifest, "parts");
            var supports = EntityArray(manifest, "supports");
            var rooms = EntityArray(manifest, "rooms");
            var furniture = EntityArray(manifest, "furniture");
            var openings = EntityArray(manifest, "openings");
            var renderGroups = EntityArray(manifest, "render_groups");
            foreach (var requiredArray in RequiredEntityArrays)
            {
                if (CountArray(manifest, requiredArray) == 0)
                {
                    throw new VoxelManifestException("manifest." + requiredArray, "must not be empty for BUILD-29");
                }
            }

            var entityArrays = new Dictionary<string, List<VoxelJsonValue>>(StringComparer.Ordinal)
            {
                ["materials"] = materials,
                ["parts"] = parts,
                ["cells"] = cells,
                ["supports"] = supports,
                ["rooms"] = rooms,
                ["furniture"] = furniture,
                ["openings"] = openings,
                ["render_groups"] = renderGroups,
            };
            foreach (var entityArray in entityArrays)
            {
                if (entityArray.Value.Count > ArrayCaps[entityArray.Key])
                    throw new VoxelManifestException("manifest." + entityArray.Key, "exceeds private array cap");
                // Every entity list is serialized in stable ID order. This is
                // both a diff/merge invariant and a cheap duplicate-ID guard.
                SortedIds(entityArray.Value, "manifest." + entityArray.Key);
            }

            var materialIds = SortedIds(materials, "manifest.materials");
            if (materialIds.Count > FrozenCaps["materials"]) throw new VoxelManifestException("manifest.materials", "exceeds private cap");
            foreach (var material in materials)
            {
                var path = "manifest.materials[" + materialIds.IndexOf(StringAt(material, null, "id")) + "]";
                ValidId(StringAt(material, null, "id"), path + ".id");
                var kind = StringAt(material, null, "kind");
                if (kind != "masonry" && kind != "wood" && kind != "glass" && kind != "metal" && kind != "debris")
                    throw new VoxelManifestException(path + ".kind", "unknown material kind");
                var density = PositiveNumber(material, "density_kg_m3", path + ".density_kg_m3");
                if (density > 20000) throw new VoxelManifestException(path + ".density_kg_m3", "exceeds 20000 kg/m3");
                PositiveNumber(material, "strength", path + ".strength");
                PositiveNumber(material, "health", path + ".health");
            }

            var cellIds = SortedIds(cells, "manifest.cells");
            if (cellIds.Count > FrozenCaps["cells"]) throw new VoxelManifestException("manifest.cells", "exceeds private cap");
            var cellById = new Dictionary<string, VoxelJsonValue>(StringComparer.Ordinal);
            var cellByCoord = new Dictionary<string, string>(StringComparer.Ordinal);
            foreach (var cell in cells)
            {
                var id = StringAt(cell, null, "id");
                var coord = Coord(cell.Require("coord", "manifest.cells"), "manifest.cells[].coord");
                var stableId = StableCellId(coord);
                if (!string.Equals(id, stableId, StringComparison.Ordinal)) throw new VoxelManifestException("manifest.cells[].id", "must be coordinate-derived stable ID");
                if (!InBounds(coord, lower, upper)) throw new VoxelManifestException("manifest.cells[].coord", "lies outside cell_bounds");
                var coordKey = CoordKey(coord);
                if (!cellByCoord.TryAdd(coordKey, id)) throw new VoxelManifestException("manifest.cells[].coord", "overlaps another occupied cell");
                if (!cell.Require("occupied", "manifest.cells[]").RequireBoolean("manifest.cells[].occupied")) throw new VoxelManifestException("manifest.cells[].occupied", "must be true");
                ValidId(StringAt(cell, null, "material_id"), "manifest.cells[].material_id");
                ValidId(StringAt(cell, null, "part_id"), "manifest.cells[].part_id");
                ValidId(StringAt(cell, null, "render_group_id"), "manifest.cells[].render_group_id");
                var supportIds = ArrayAt(cell, "support_ids", "manifest.cells[].support_ids");
                foreach (var supportId in supportIds) ValidId(supportId.RequireString("manifest.cells[].support_ids[]"), "manifest.cells[].support_ids[]");
                SortedStrings(supportIds, "manifest.cells[].support_ids");
                cellById.Add(id, cell);
            }

            ValidatePartReferences(parts, cellById, materialIds);
            ValidateSupportReferences(supports, cellById);
            ValidateRoomReferences(rooms, lower, upper);
            ValidateFurnitureReferences(furniture, rooms, cellByCoord, lower, upper);
            ValidateOpeningReferences(openings, parts, rooms, cellByCoord, lower, upper);
            ValidateRenderGroups(renderGroups, cellById, materialIds);
            ValidateTopology(manifest, cellById, cellByCoord, supports, lower, upper);
            ValidateVolumeAndBudgets(manifest, cells.Count, materials.Count, lower, upper);
            ValidateReceiptAndQualification(manifest, receipt, qualification, manifestBytes, recipeBytes);
        }

        private static void ValidatePartReferences(List<VoxelJsonValue> parts, Dictionary<string, VoxelJsonValue> cells, List<string> materials)
        {
            var membership = new HashSet<string>(StringComparer.Ordinal);
            foreach (var part in parts)
            {
                var id = StringAt(part, null, "id");
                var kind = StringAt(part, null, "kind");
                if (kind != "shell" && kind != "roof" && kind != "partition" && kind != "fixture") throw new VoxelManifestException("manifest.parts." + id, "unknown part kind");
                if (!materials.Contains(StringAt(part, null, "material_id"))) throw new VoxelManifestException("manifest.parts." + id, "unknown material");
                if (IntAt(part, "thickness_cells", "manifest.parts." + id) < 1) throw new VoxelManifestException("manifest.parts." + id, "thin part");
                var members = ArrayAt(part, "cell_ids", "manifest.parts." + id);
                SortedStrings(members, "manifest.parts." + id + ".cell_ids");
                var memberIds = new HashSet<string>(StringComparer.Ordinal);
                foreach (var member in members)
                {
                    var memberId = member.RequireString("manifest.parts." + id + ".cell_ids[]");
                    if (!cells.TryGetValue(memberId, out var cell) || !memberIds.Add(memberId) || !membership.Add(memberId)) throw new VoxelManifestException("manifest.parts." + id, "invalid or overlapping cell membership");
                    if (!string.Equals(StringAt(cell, null, "part_id"), id, StringComparison.Ordinal)) throw new VoxelManifestException("manifest.parts." + id, "cell part_id does not match membership");
                    if (!string.Equals(StringAt(cell, null, "material_id"), StringAt(part, null, "material_id"), StringComparison.Ordinal)) throw new VoxelManifestException("manifest.parts." + id, "cell material_id does not match part material_id");
                }
            }
            if (membership.Count != cells.Count) throw new VoxelManifestException("manifest.parts", "parts must cover every cell exactly once");
        }

        private static void ValidateSupportReferences(List<VoxelJsonValue> supports, Dictionary<string, VoxelJsonValue> cells)
        {
            var foundations = new HashSet<string>(StringComparer.Ordinal);
            var knownSupportIds = new HashSet<string>(StringComparer.Ordinal);
            var supportMembers = new Dictionary<string, HashSet<string>>(StringComparer.Ordinal);
            foreach (var support in supports)
            {
                var id = StringAt(support, null, "id");
                ValidId(id, "manifest.supports[].id");
                if (!knownSupportIds.Add(id)) throw new VoxelManifestException("manifest.supports", "duplicate support ID");
                var kind = StringAt(support, null, "kind");
                if (kind != "foundation" && kind != "face_connected" && kind != "brace") throw new VoxelManifestException("manifest.supports." + id, "unknown support kind");
                var members = ArrayAt(support, "cell_ids", "manifest.supports." + id);
                SortedStrings(members, "manifest.supports." + id + ".cell_ids");
                var seen = new HashSet<string>(StringComparer.Ordinal);
                supportMembers.Add(id, seen);
                foreach (var member in members)
                {
                    var memberId = member.RequireString("manifest.supports." + id + ".cell_ids[]");
                    if (!cells.ContainsKey(memberId) || !seen.Add(memberId)) throw new VoxelManifestException("manifest.supports." + id, "invalid cell membership");
                    if (kind == "foundation") foundations.Add(memberId);
                }
            }
            if (foundations.Count == 0) throw new VoxelManifestException("manifest.supports", "foundation support is empty");

            foreach (var cell in cells.Values)
            {
                var cellSupportIds = ArrayAt(cell, "support_ids", "manifest.cells[].support_ids");
                var cellId = StringAt(cell, null, "id");
                foreach (var supportIdValue in cellSupportIds)
                {
                    var supportId = supportIdValue.RequireString("manifest.cells[].support_ids[]");
                    if (!knownSupportIds.Contains(supportId) || !supportMembers[supportId].Contains(cellId))
                    {
                        throw new VoxelManifestException("manifest.cells[].support_ids", "references unknown support");
                    }
                }
            }
            foreach (var support in supportMembers)
            {
                foreach (var cellId in support.Value)
                {
                    var cellSupportIds = ArrayAt(cells[cellId], "support_ids", "manifest.cells[].support_ids");
                    var found = false;
                    foreach (var supportIdValue in cellSupportIds)
                    {
                        if (string.Equals(supportIdValue.RequireString("manifest.cells[].support_ids[]"), support.Key, StringComparison.Ordinal))
                        {
                            found = true;
                            break;
                        }
                    }
                    if (!found) throw new VoxelManifestException("manifest.supports." + support.Key, "cell membership is not mirrored by cell.support_ids");
                }
            }
        }

        private static void ValidateRoomReferences(List<VoxelJsonValue> rooms, int[] lower, int[] upper)
        {
            foreach (var room in rooms)
            {
                var id = StringAt(room, null, "id");
                ValidId(StringAt(room, null, "layout_id"), "manifest.rooms." + id + ".layout_id");
                var bounds = ObjectAt(room, "bounds", "manifest.rooms." + id + ".bounds");
                var roomLower = Coord(bounds.Require("min", "manifest.rooms." + id + ".bounds"), "manifest.rooms." + id + ".bounds.min");
                var roomUpper = Coord(bounds.Require("max", "manifest.rooms." + id + ".bounds"), "manifest.rooms." + id + ".bounds.max");
                for (var axis = 0; axis < 3; axis++)
                {
                    if (roomLower[axis] > roomUpper[axis] || roomLower[axis] < lower[axis] || roomUpper[axis] > upper[axis])
                        throw new VoxelManifestException("manifest.rooms." + id + ".bounds", "room lies outside profile bounds");
                }
                SortedStrings(ArrayAt(room, "furniture_ids", "manifest.rooms." + id), "manifest.rooms." + id + ".furniture_ids");
            }
        }

        private static void ValidateFurnitureReferences(List<VoxelJsonValue> furniture, List<VoxelJsonValue> rooms, Dictionary<string, string> occupied, int[] lower, int[] upper)
        {
            var roomIds = new HashSet<string>(StringComparer.Ordinal);
            foreach (var room in rooms) roomIds.Add(StringAt(room, null, "id"));
            var furnitureById = new Dictionary<string, VoxelJsonValue>(StringComparer.Ordinal);
            foreach (var item in furniture) furnitureById.Add(StringAt(item, null, "id"), item);
            var listedFurnitureIds = new HashSet<string>(StringComparer.Ordinal);
            foreach (var room in rooms)
            {
                var roomId = StringAt(room, null, "id");
                foreach (var itemIdValue in ArrayAt(room, "furniture_ids", "manifest.rooms." + roomId + ".furniture_ids"))
                {
                    var itemId = itemIdValue.RequireString("manifest.rooms." + roomId + ".furniture_ids[]");
                    if (!furnitureById.TryGetValue(itemId, out var item) ||
                        !string.Equals(StringAt(item, null, "room_id"), roomId, StringComparison.Ordinal) ||
                        !listedFurnitureIds.Add(itemId))
                    {
                        throw new VoxelManifestException("manifest.rooms." + roomId + ".furniture_ids", "must reference each furniture item exactly once and match room_id");
                    }
                }
            }
            foreach (var item in furniture)
            {
                var id = StringAt(item, null, "id");
                var roomId = StringAt(item, null, "room_id");
                if (!roomIds.Contains(roomId)) throw new VoxelManifestException("manifest.furniture." + id, "unknown room");
                if (!listedFurnitureIds.Contains(id)) throw new VoxelManifestException("manifest.furniture." + id, "is not listed by its room");
                var kind = StringAt(item, null, "kind");
                if (kind != "table" && kind != "bed" && kind != "chest" && kind != "stair" && kind != "lamp") throw new VoxelManifestException("manifest.furniture." + id, "unknown furniture kind");
                ValidId(StringAt(item, null, "material_id"), "manifest.furniture." + id + ".material_id");
                var anchor = Coord(item.Require("anchor_coord", "manifest.furniture." + id), "manifest.furniture." + id + ".anchor_coord");
                if (!InBounds(anchor, lower, upper)) throw new VoxelManifestException("manifest.furniture." + id + ".anchor_coord", "outside profile bounds");
                foreach (var footprint in ArrayAt(item, "footprint_cells", "manifest.furniture." + id))
                {
                    var coord = Coord(footprint, "manifest.furniture." + id + ".footprint_cells[]");
                    if (!InBounds(coord, lower, upper) || occupied.ContainsKey(CoordKey(coord))) throw new VoxelManifestException("manifest.furniture." + id, "footprint conflicts with profile or structure");
                }
            }
        }

        private static void ValidateOpeningReferences(List<VoxelJsonValue> openings, List<VoxelJsonValue> parts, List<VoxelJsonValue> rooms, Dictionary<string, string> occupied, int[] lower, int[] upper)
        {
            var partIds = new HashSet<string>(StringComparer.Ordinal);
            foreach (var part in parts) partIds.Add(StringAt(part, null, "id"));
            var roomIds = new HashSet<string>(StringComparer.Ordinal);
            foreach (var room in rooms) roomIds.Add(StringAt(room, null, "id"));
            var openingCoords = new HashSet<string>(StringComparer.Ordinal);
            foreach (var opening in openings)
            {
                var id = StringAt(opening, null, "id");
                var kind = StringAt(opening, null, "kind");
                if (kind != "window" && kind != "door" && kind != "breach") throw new VoxelManifestException("manifest.openings." + id, "unknown opening kind");
                if (!partIds.Contains(StringAt(opening, null, "host_part_id")) || !roomIds.Contains(StringAt(opening, null, "room_id"))) throw new VoxelManifestException("manifest.openings." + id, "unknown host part or room");
                var coords = ArrayAt(opening, "cell_coords", "manifest.openings." + id);
                if (coords.Count == 0) throw new VoxelManifestException("manifest.openings." + id, "opening has no cells");
                foreach (var item in coords)
                {
                    var coord = Coord(item, "manifest.openings." + id + ".cell_coords[]");
                    var key = CoordKey(coord);
                    if (!InBounds(coord, lower, upper) || occupied.ContainsKey(key) || !openingCoords.Add(key)) throw new VoxelManifestException("manifest.openings." + id, "invalid or overlapping opening cell");
                }
            }
        }

        private static void ValidateRenderGroups(List<VoxelJsonValue> groups, Dictionary<string, VoxelJsonValue> cells, List<string> materials)
        {
            var membership = new HashSet<string>(StringComparer.Ordinal);
            foreach (var group in groups)
            {
                var id = StringAt(group, null, "id");
                var kind = StringAt(group, null, "surface_kind");
                if (kind != "masonry" && kind != "roof" && kind != "interior" && kind != "window_frame" && kind != "furniture") throw new VoxelManifestException("manifest.render_groups." + id, "unknown surface kind");
                var materialId = StringAt(group, null, "material_id");
                if (!materials.Contains(materialId)) throw new VoxelManifestException("manifest.render_groups." + id, "unknown material");
                var members = ArrayAt(group, "cell_ids", "manifest.render_groups." + id);
                SortedStrings(members, "manifest.render_groups." + id + ".cell_ids");
                foreach (var member in members)
                {
                    var cellId = member.RequireString("manifest.render_groups." + id + ".cell_ids[]");
                    if (!cells.TryGetValue(cellId, out var cell) || !membership.Add(cellId)) throw new VoxelManifestException("manifest.render_groups." + id, "invalid or overlapping cell membership");
                    if (!string.Equals(StringAt(cell, null, "render_group_id"), id, StringComparison.Ordinal)) throw new VoxelManifestException("manifest.render_groups." + id, "cell render_group_id does not match membership");
                    if (!string.Equals(StringAt(cell, null, "material_id"), materialId, StringComparison.Ordinal)) throw new VoxelManifestException("manifest.render_groups." + id, "cell material_id does not match render group material_id");
                }
            }
            if (membership.Count != cells.Count) throw new VoxelManifestException("manifest.render_groups", "render groups must cover every cell exactly once");
        }

        private static void ValidateTopology(
            VoxelJsonValue manifest,
            Dictionary<string, VoxelJsonValue> cellsById,
            Dictionary<string, string> cellsByCoord,
            List<VoxelJsonValue> supports,
            int[] lower,
            int[] upper)
        {
            var topology = ObjectAt(manifest, "topology", "manifest.topology");
            var shell = CoordinateMap(topology.Require("shell_required", "manifest.topology"), "manifest.topology.shell_required");
            var foundations = CoordinateMap(topology.Require("foundation_cells", "manifest.topology"), "manifest.topology.foundation_cells");
            var openings = CoordinateMap(topology.Require("opening_cells", "manifest.topology"), "manifest.topology.opening_cells");
            if (shell.Count == 0 || foundations.Count == 0) throw new VoxelManifestException("manifest.topology", "shell and foundations must not be empty");
            foreach (var coordinate in foundations.Keys)
            {
                if (!cellsByCoord.ContainsKey(coordinate)) throw new VoxelManifestException("manifest.topology.foundation_cells", "foundation is not occupied");
            }
            foreach (var coordinate in shell.Keys)
            {
                if (!cellsByCoord.ContainsKey(coordinate) && !openings.ContainsKey(coordinate)) throw new VoxelManifestException("manifest.topology.shell_required", "unrepresented shell coordinate");
            }
            foreach (var coordinate in shell.Keys)
            {
                if (openings.ContainsKey(coordinate)) throw new VoxelManifestException("manifest.topology", "opening overlaps required shell");
            }
            foreach (var pair in openings)
            {
                if (cellsByCoord.ContainsKey(pair.Key) || !InBounds(pair.Value, lower, upper)) throw new VoxelManifestException("manifest.topology.opening_cells", "opening is occupied or outside bounds");
            }

            var supportFoundationCoords = new HashSet<string>(StringComparer.Ordinal);
            foreach (var support in supports)
            {
                if (StringAt(support, null, "kind") != "foundation") continue;
                foreach (var member in ArrayAt(support, "cell_ids", "manifest.supports"))
                {
                    var memberId = member.RequireString("manifest.supports.cell_ids[]");
                    if (!cellsById.TryGetValue(memberId, out var cell)) throw new VoxelManifestException("manifest.supports", "foundation references unknown cell");
                    supportFoundationCoords.Add(CoordKey(Coord(cell.Require("coord", "manifest.cells[]"), "manifest.cells[].coord")));
                }
            }
            if (supportFoundationCoords.Count == 0) throw new VoxelManifestException("manifest.supports", "foundation has no members");
            if (!supportFoundationCoords.SetEquals(foundations.Keys)) throw new VoxelManifestException("manifest.topology.foundation_cells", "does not match foundation support members");

            var openingCoords = new HashSet<string>(StringComparer.Ordinal);
            foreach (var opening in ArrayAt(manifest, "openings", "manifest.openings"))
            {
                foreach (var item in ArrayAt(opening, "cell_coords", "manifest.openings[]"))
                {
                    openingCoords.Add(CoordKey(Coord(item, "manifest.openings[].cell_coords[]")));
                }
            }
            if (!openingCoords.SetEquals(openings.Keys)) throw new VoxelManifestException("manifest.topology.opening_cells", "does not match opening records");
        }

        private static void ValidateVolumeAndBudgets(VoxelJsonValue manifest, int cellCount, int materialCount, int[] lower, int[] upper)
        {
            var volume = ObjectAt(manifest, "volume", "manifest.volume");
            if (IntAt(volume, "occupied_cells", "manifest.volume") != cellCount) throw new VoxelManifestException("manifest.volume.occupied_cells", "does not match cells");
            long envelopeCells = 1;
            for (var axis = 0; axis < 3; axis++)
            {
                var dimension = (long)upper[axis] - lower[axis] + 1L;
                if (dimension <= 0 || envelopeCells > MaxProfileEnvelopeCells / dimension)
                    throw new VoxelManifestException("manifest.profile.cell_bounds", "profile envelope exceeds private cap");
                envelopeCells *= dimension;
            }
            if (IntAt(volume, "envelope_cells", "manifest.volume") != envelopeCells)
                throw new VoxelManifestException("manifest.volume.envelope_cells", "does not match profile bounds");
            if (IntAt(volume, "empty_cells", "manifest.volume") != envelopeCells - cellCount)
                throw new VoxelManifestException("manifest.volume.empty_cells", "does not match envelope minus occupied cells");
            var cellVolume = PositiveNumber(volume, "cell_volume_m3", "manifest.volume.cell_volume_m3");
            var occupiedVolume = PositiveNumber(volume, "occupied_volume_m3", "manifest.volume.occupied_volume_m3");
            if (Math.Abs(occupiedVolume - cellVolume * cellCount) > Math.Max(1e-9, occupiedVolume * 1e-9))
                throw new VoxelManifestException("manifest.volume.occupied_volume_m3", "does not match occupied cells times cell volume");

            var budgets = ObjectAt(manifest, "budgets", "manifest.budgets");
            var limits = ObjectAt(budgets, "limits", "manifest.budgets.limits");
            var observed = ObjectAt(budgets, "observed", "manifest.budgets.observed");
            foreach (var name in BudgetNames)
            {
                var limit = NumberAt(limits, name, "manifest.budgets.limits." + name);
                var value = NumberAt(observed, name, "manifest.budgets.observed." + name);
                if (limit > FrozenCaps[name] || value > limit || value < 0) throw new VoxelManifestException("manifest.budgets." + name, "exceeds private cap or is negative");
                if (IntegerBudgets.Contains(name) && Math.Abs(value - Math.Round(value)) > 1e-9) throw new VoxelManifestException("manifest.budgets." + name, "count budget must be integral");
            }
            if (Math.Abs(NumberAt(observed, "cells", "manifest.budgets.observed.cells") - cellCount) > 1e-9 ||
                Math.Abs(NumberAt(observed, "materials", "manifest.budgets.observed.materials") - materialCount) > 1e-9)
                throw new VoxelManifestException("manifest.budgets.observed", "cells/materials counts do not match arrays");
        }

        private static void ValidateReceiptAndQualification(VoxelJsonValue manifest, VoxelJsonValue receipt, VoxelJsonValue qualification, byte[] manifestBytes, byte[] recipeBytes)
        {
            var manifestHash = Sha256(manifestBytes);
            ExactString(receipt, "provenance_class", "owner-derived-private", "receipt.provenance_class");
            ExactString(receipt, "interior_authorship", "newly_authored", "receipt.interior_authorship");
            ExactString(receipt, "manifest_sha256", manifestHash, "receipt.manifest_sha256");
            ExactString(receipt, "semantic_sha256", StringAt(manifest, "hashes", "semantic_sha256"), "receipt.semantic_sha256");
            ExactString(receipt, "surface_recipe_sha256", Sha256(recipeBytes), "receipt.surface_recipe_sha256");
            ExactString(receipt, "source_profile_sha256", StringAt(manifest, "hashes", "source_sha256"), "receipt.source_profile_sha256");
            var receiptValidator = ObjectAt(receipt, "validator", "receipt.validator");
            ExactBoolean(receiptValidator, "valid", true, "receipt.validator.valid");
            ExactString(receiptValidator, "authority", "source_house_state", "receipt.validator.authority");
            ExactInt(receiptValidator, "building_index", 29, "receipt.validator.building_index");
            ExactInt(receiptValidator, "schema_version", 1, "receipt.validator.schema_version");
            ExactString(qualification, "status", "pass", "qualification_report.status");
            ExactString(qualification, "qualification", ExpectedQualification, "qualification_report.qualification");
            var boundary = ObjectAt(qualification, "level1_boundary", "qualification_report.level1_boundary");
            ExactBoolean(boundary, "qualification_corpus_touched", false, "qualification_report.level1_boundary.qualification_corpus_touched");
            ExactBoolean(boundary, "unity_or_gameplay_placement", false, "qualification_report.level1_boundary.unity_or_gameplay_placement");
            ExactBoolean(boundary, "visual_bounds_replace_source_aabb", false, "qualification_report.level1_boundary.visual_bounds_replace_source_aabb");
        }

        private static VoxelJsonValue BuildMapping(VoxelJsonValue manifest)
        {
            var coordinates = ObjectAt(manifest, "coordinate_system", "manifest.coordinate_system");
            var profile = ObjectAt(manifest, "profile", "manifest.profile");
            var sourceAabb = ObjectAt(manifest, "source_gameplay_aabb", "manifest.source_gameplay_aabb");
            var mapping = new Dictionary<string, VoxelJsonValue>(StringComparer.Ordinal)
            {
                ["mapping_version"] = VoxelJsonValue.Number("1"),
                ["manifest_id"] = VoxelJsonValue.String(StringAt(manifest, null, "manifest_id")),
                ["structure_id"] = VoxelJsonValue.String(StringAt(profile, null, "structure_id")),
                ["generation"] = VoxelJsonValue.Number(IntAt(profile, "generation", "manifest.profile.generation").ToString(CultureInfo.InvariantCulture)),
                ["source_entity_id"] = VoxelJsonValue.String(ExpectedSourceEntity),
                ["source_building_index"] = VoxelJsonValue.Number("29"),
                ["semantic_sha256"] = VoxelJsonValue.String(StringAt(manifest, "hashes", "semantic_sha256")),
                ["schema_sha256"] = VoxelJsonValue.String(StringAt(manifest, "hashes", "schema_sha256")),
                ["source_anchor"] = coordinates.Require("anchor", "manifest.coordinate_system"),
                ["source_gameplay_aabb"] = sourceAabb,
                ["visual_bounds_m"] = profile.Require("visual_bounds_m", "manifest.profile"),
                ["coordinate_system"] = coordinates,
                ["source_to_unity"] = VoxelJsonValue.Object(new Dictionary<string, VoxelJsonValue>(StringComparer.Ordinal)
                {
                    ["conversion"] = VoxelJsonValue.String("SourceToUnity at runtime"),
                    ["authority"] = VoxelJsonValue.String("source_gameplay_aabb"),
                    ["anchor_kind"] = VoxelJsonValue.String("source_aabb_center_bottom"),
                    ["cells_are_data_only"] = VoxelJsonValue.Boolean(true),
                    ["surface_policy"] = VoxelJsonValue.String("batched render groups; no per-cell GameObjects"),
                }),
                ["authority"] = manifest.Require("authority", "manifest"),
                ["materials"] = manifest.Require("materials", "manifest"),
                ["parts"] = manifest.Require("parts", "manifest"),
                ["cells"] = manifest.Require("cells", "manifest"),
                ["supports"] = manifest.Require("supports", "manifest"),
                ["rooms"] = manifest.Require("rooms", "manifest"),
                ["furniture"] = manifest.Require("furniture", "manifest"),
                ["openings"] = manifest.Require("openings", "manifest"),
                ["render_groups"] = manifest.Require("render_groups", "manifest"),
                ["budgets"] = manifest.Require("budgets", "manifest"),
            };
            return VoxelJsonValue.Object(mapping);
        }

        private static string DigestWithoutSemanticHash(VoxelJsonValue manifest)
        {
            var hashes = ObjectAt(manifest, "hashes", "manifest.hashes");
            var semantic = hashes.ObjectValue["semantic_sha256"];
            hashes.ObjectValue.Remove("semantic_sha256");
            try
            {
                return Sha256(VoxelJsonCanonical.Utf8(manifest));
            }
            finally
            {
                hashes.ObjectValue["semantic_sha256"] = semantic;
            }
        }

        private static string DigestRecipePayload(VoxelJsonValue recipe)
        {
            if (recipe.Kind != VoxelJsonKind.Object) throw new VoxelManifestException("recipe", "must be an object");
            recipe.ObjectValue.TryGetValue("recipe_sha256", out var field);
            recipe.ObjectValue.Remove("recipe_sha256");
            try
            {
                return Sha256(VoxelJsonCanonical.Utf8(recipe));
            }
            finally
            {
                if (field != null) recipe.ObjectValue["recipe_sha256"] = field;
            }
        }

        private static string DigestAssetPayload(VoxelJsonValue manifest)
        {
            var payload = VoxelJsonValue.Object(new Dictionary<string, VoxelJsonValue>(StringComparer.Ordinal)
            {
                ["cells"] = manifest.Require("cells", "manifest"),
                ["parts"] = manifest.Require("parts", "manifest"),
                ["supports"] = manifest.Require("supports", "manifest"),
                ["render_groups"] = manifest.Require("render_groups", "manifest"),
            });
            return Sha256(VoxelJsonCanonical.Utf8(payload));
        }

        private static string RequireAbsoluteDirectory(string value, string path)
        {
            if (string.IsNullOrWhiteSpace(value) || !Path.IsPathRooted(value)) throw new VoxelManifestException(path, "must be an absolute directory");
            var full = Path.GetFullPath(value);
            if (!Directory.Exists(full)) throw new VoxelManifestException(path, "directory does not exist");
            return full.TrimEnd(Path.DirectorySeparatorChar, Path.AltDirectorySeparatorChar);
        }

        private static string ResolveUnderRoot(string root, string relative, string path)
        {
            if (string.IsNullOrWhiteSpace(relative) || Path.IsPathRooted(relative)) throw new VoxelManifestException(path, "must be a non-empty relative path");
            var normalized = relative.Replace('\\', '/');
            foreach (var component in normalized.Split('/'))
            {
                if (component == "..") throw new VoxelManifestException(path, "parent traversal is not allowed");
            }

            var candidate = Path.GetFullPath(Path.Combine(root, relative));
            var rootPrefix = root.TrimEnd(Path.DirectorySeparatorChar, Path.AltDirectorySeparatorChar) + Path.DirectorySeparatorChar;
            if (!candidate.StartsWith(rootPrefix, StringComparison.Ordinal) || !File.Exists(candidate))
                throw new VoxelManifestException(path, "file is missing or escapes owner root");
            return candidate;
        }

        private static byte[] ReadPackageBytes(string path, string label, int maxBytes)
        {
            var info = new FileInfo(path);
            if (!info.Exists) throw new VoxelManifestException(label, "file is missing");
            if (info.Length > maxBytes) throw new VoxelManifestException(label, "file exceeds the editor input cap of " + maxBytes.ToString(CultureInfo.InvariantCulture) + " bytes");
            return File.ReadAllBytes(path);
        }

        private static string NormalizeRelative(string value)
        {
            if (string.IsNullOrWhiteSpace(value) || Path.IsPathRooted(value)) throw new VoxelManifestException("path", "receipt path must be relative");
            var normalized = value.Replace('\\', '/');
            foreach (var component in normalized.Split('/')) if (component == "..") throw new VoxelManifestException("path", "receipt path traversal is not allowed");
            return normalized.TrimStart('.', '/');
        }

        private static void ValidateOutputDirectory(string value)
        {
            if (string.IsNullOrWhiteSpace(value) || !Path.IsPathRooted(value)) throw new VoxelManifestException("output_directory", "must be an absolute directory");
            var normalized = Path.GetFullPath(value).Replace('\\', '/').TrimEnd('/');
            var components = normalized.Split(new[] { '/' }, StringSplitOptions.RemoveEmptyEntries);
            for (var index = 0; index + 1 < components.Length; index++)
            {
                if (!string.Equals(components[index], "Assets", StringComparison.OrdinalIgnoreCase)) continue;
                var next = components[index + 1];
                if (string.Equals(next, "Resources", StringComparison.OrdinalIgnoreCase) ||
                    string.Equals(next, "StreamingAssets", StringComparison.OrdinalIgnoreCase) ||
                    string.Equals(next, "OwnerData", StringComparison.OrdinalIgnoreCase) ||
                    string.Equals(next, "OwnerAssets", StringComparison.OrdinalIgnoreCase))
                {
                    throw new VoxelManifestException("output_directory", "owner-derived output cannot be written to Track S Resources/owner payload paths");
                }
            }
        }

        private static List<VoxelJsonValue> EntityArray(VoxelJsonValue root, string key)
        {
            return ArrayAt(root, key, "manifest." + key);
        }

        private static List<VoxelJsonValue> ArrayAt(VoxelJsonValue root, string key, string path)
        {
            return root.Require(key, path).RequireKind(VoxelJsonKind.Array, path).ArrayValue;
        }

        private static VoxelJsonValue ObjectAt(VoxelJsonValue root, string key, string path)
        {
            return root.Require(key, path).RequireKind(VoxelJsonKind.Object, path);
        }

        private static string StringAt(VoxelJsonValue root, string parentKey, string key)
        {
            var value = parentKey == null ? root.Require(key, key) : ObjectAt(root, parentKey, parentKey).Require(key, parentKey).RequireKind(VoxelJsonKind.String, parentKey + "." + key);
            return value.StringValue;
        }

        private static double NumberAt(VoxelJsonValue root, string key, string path)
        {
            return root.Require(key, path).RequireNumber(path);
        }

        private static long IntAt(VoxelJsonValue root, string key, string path)
        {
            return root.Require(key, path).RequireInt64(path);
        }

        private static void ExactString(VoxelJsonValue root, string key, string expected, string path)
        {
            if (!string.Equals(StringAt(root, null, key), expected, StringComparison.Ordinal)) throw new VoxelManifestException(path, "must be '" + expected + "'");
        }

        private static void ExactBoolean(VoxelJsonValue root, string key, bool expected, string path)
        {
            if (root.Require(key, path).RequireBoolean(path) != expected) throw new VoxelManifestException(path, "must be " + expected.ToString().ToLowerInvariant());
        }

        private static void ExactInt(VoxelJsonValue root, string key, long expected, string path)
        {
            if (IntAt(root, key, path) != expected) throw new VoxelManifestException(path, "must be " + expected.ToString(CultureInfo.InvariantCulture));
        }

        private static long NonNegativeInt(VoxelJsonValue root, string key, string path)
        {
            var value = IntAt(root, key, path);
            if (value < 0) throw new VoxelManifestException(path, "must be non-negative");
            return value;
        }

        private static double PositiveNumber(VoxelJsonValue root, string key, string path)
        {
            var value = NumberAt(root, key, path);
            if (!(value > 0)) throw new VoxelManifestException(path, "must be positive and finite");
            return value;
        }

        private static void NonEmptyString(VoxelJsonValue root, string key, string path)
        {
            if (StringAt(root, null, key).Length < 3) throw new VoxelManifestException(path, "must not be empty");
        }

        private static void ValidHash(string value, string path)
        {
            if (value == null || value.Length != 64 || value == new string('0', 64)) throw new VoxelManifestException(path, "must be a non-zero SHA-256 digest");
            foreach (var character in value)
            {
                if (!((character >= '0' && character <= '9') || (character >= 'a' && character <= 'f'))) throw new VoxelManifestException(path, "must be lowercase hexadecimal");
            }
        }

        private static void ValidId(string value, string path)
        {
            if (string.IsNullOrEmpty(value) || value.Length < 2 || value.Length > 96 || value[0] < 'a' || value[0] > 'z') throw new VoxelManifestException(path, "must be a lowercase stable ID");
            var previousHyphen = false;
            foreach (var character in value)
            {
                if (character == '-')
                {
                    if (previousHyphen) throw new VoxelManifestException(path, "must not contain empty ID segments");
                    previousHyphen = true;
                }
                else if (!((character >= 'a' && character <= 'z') || (character >= '0' && character <= '9')))
                {
                    throw new VoxelManifestException(path, "must be lowercase hyphenated");
                }
                else
                {
                    previousHyphen = false;
                }
            }
            if (previousHyphen) throw new VoxelManifestException(path, "must not end with a hyphen");
        }

        private static List<string> SortedIds(List<VoxelJsonValue> values, string path)
        {
            var ids = new List<string>(values.Count);
            foreach (var value in values) ids.Add(StringAt(value, null, "id"));
            SortedStrings(ids, path);
            foreach (var id in ids) ValidId(id, path + ".id");
            return ids;
        }

        private static void SortedStrings(List<VoxelJsonValue> values, string path)
        {
            var strings = new List<string>(values.Count);
            foreach (var value in values) strings.Add(value.RequireString(path + "[]"));
            SortedStrings(strings, path);
        }

        private static void SortedStrings(List<string> values, string path)
        {
            for (var index = 1; index < values.Count; index++)
            {
                if (string.CompareOrdinal(values[index - 1], values[index]) >= 0) throw new VoxelManifestException(path, "items must be sorted and duplicate-free");
            }
        }

        private static int[] Coord(VoxelJsonValue value, string path)
        {
            var values = value.RequireKind(VoxelJsonKind.Array, path).ArrayValue;
            if (values.Count != 3) throw new VoxelManifestException(path, "must contain three integer coordinates");
            var result = new int[3];
            for (var index = 0; index < 3; index++)
            {
                var coordinate = values[index].RequireInt64(path + "[" + index + "]");
                if (coordinate < int.MinValue || coordinate > int.MaxValue) throw new VoxelManifestException(path, "coordinate exceeds Int32");
                result[index] = (int)coordinate;
            }
            return result;
        }

        private static double[] Vec3(VoxelJsonValue value, string path, bool positive)
        {
            var values = value.RequireKind(VoxelJsonKind.Array, path).ArrayValue;
            if (values.Count != 3) throw new VoxelManifestException(path, "must contain three finite numbers");
            var result = new double[3];
            for (var index = 0; index < 3; index++)
            {
                result[index] = values[index].RequireNumber(path + "[" + index + "]");
                if (positive && !(result[index] > 0)) throw new VoxelManifestException(path, "must contain positive extents");
            }
            return result;
        }

        private static Dictionary<string, int[]> CoordinateMap(VoxelJsonValue value, string path)
        {
            var map = new Dictionary<string, int[]>(StringComparer.Ordinal);
            var values = value.RequireKind(VoxelJsonKind.Array, path).ArrayValue;
            foreach (var item in values)
            {
                var coordinate = Coord(item, path + "[]");
                if (!map.TryAdd(CoordKey(coordinate), coordinate)) throw new VoxelManifestException(path, "contains duplicate coordinates");
            }
            return map;
        }

        private static bool InBounds(int[] coordinate, int[] lower, int[] upper)
        {
            for (var axis = 0; axis < 3; axis++) if (coordinate[axis] < lower[axis] || coordinate[axis] > upper[axis]) return false;
            return true;
        }

        private static string CoordKey(int[] coordinate)
        {
            return coordinate[0].ToString(CultureInfo.InvariantCulture) + "," + coordinate[1].ToString(CultureInfo.InvariantCulture) + "," + coordinate[2].ToString(CultureInfo.InvariantCulture);
        }

        private static string StableCellId(int[] coordinate)
        {
            return "cell-" + Component(coordinate[0]) + "-" + Component(coordinate[1]) + "-" + Component(coordinate[2]);
        }

        private static string Component(int value)
        {
            return (value < 0 ? "n" : "p") + Math.Abs((long)value).ToString(CultureInfo.InvariantCulture);
        }

        private static int CountArray(VoxelJsonValue root, string key)
        {
            return ArrayAt(root, key, key).Count;
        }

        private static string Sha256(byte[] bytes)
        {
            using (var sha = SHA256.Create())
            {
                var digest = sha.ComputeHash(bytes);
                var builder = new StringBuilder(digest.Length * 2);
                foreach (var value in digest) builder.Append(value.ToString("x2", CultureInfo.InvariantCulture));
                return builder.ToString();
            }
        }
    }
}
