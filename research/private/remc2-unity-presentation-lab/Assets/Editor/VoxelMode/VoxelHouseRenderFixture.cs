using System;
using System.Collections.Generic;
using System.Globalization;
using System.IO;
using System.Security.Cryptography;
using System.Text;

namespace MagicCarpet.PresentationLab.VoxelMode
{
    /// <summary>
    /// Presentation states used by the editor fixture. The gameplay house is
    /// never replaced; these states only choose which deterministic batched
    /// surfaces and metadata would be presented.
    /// </summary>
    public enum VoxelFixtureState
    {
        Intact,
        Opposite,
        NightWindow,
        Cutaway,
        Breach,
    }

    public sealed class VoxelFixtureBatch
    {
        internal VoxelFixtureBatch(string id, string surfaceKind, string materialId, List<string> cellIds)
        {
            Id = id;
            SurfaceKind = surfaceKind;
            MaterialId = materialId;
            CellIds = cellIds.AsReadOnly();
        }

        public string Id { get; }
        public string SurfaceKind { get; }
        public string MaterialId { get; }
        public IReadOnlyList<string> CellIds { get; }
    }

    public sealed class VoxelFixtureFrame
    {
        internal VoxelFixtureFrame(
            VoxelFixtureState state,
            string cameraSide,
            List<string> visibleCellIds,
            List<string> hiddenPartIds,
            List<string> breachCellIds,
            List<string> litWindowIds,
            List<string> roomIds,
            List<string> furnitureIds,
            List<string> exposedRoomIds,
            List<VoxelFixtureBatch> batches,
            string serialized,
            string sha256,
            int persistentGameObjectCount,
            int activeCosmeticDebris)
        {
            State = state;
            CameraSide = cameraSide;
            VisibleCellIds = visibleCellIds.AsReadOnly();
            HiddenPartIds = hiddenPartIds.AsReadOnly();
            BreachCellIds = breachCellIds.AsReadOnly();
            LitWindowIds = litWindowIds.AsReadOnly();
            RoomIds = roomIds.AsReadOnly();
            FurnitureIds = furnitureIds.AsReadOnly();
            ExposedRoomIds = exposedRoomIds.AsReadOnly();
            Batches = batches.AsReadOnly();
            Serialized = serialized;
            Sha256 = sha256;
            PersistentGameObjectCount = persistentGameObjectCount;
            ActiveCosmeticDebris = activeCosmeticDebris;
        }

        public VoxelFixtureState State { get; }
        public string CameraSide { get; }
        public IReadOnlyList<string> VisibleCellIds { get; }
        public IReadOnlyList<string> HiddenPartIds { get; }
        public IReadOnlyList<string> BreachCellIds { get; }
        public IReadOnlyList<string> LitWindowIds { get; }
        public IReadOnlyList<string> RoomIds { get; }
        public IReadOnlyList<string> FurnitureIds { get; }
        public IReadOnlyList<string> ExposedRoomIds { get; }
        public IReadOnlyList<VoxelFixtureBatch> Batches { get; }
        public string Serialized { get; }
        public string Sha256 { get; }
        public int VisibleCellCount => VisibleCellIds.Count;
        public int BatchCount => Batches.Count;
        public int RendererCount => Batches.Count;
        public int MaterialCount
        {
            get
            {
                var materials = new HashSet<string>(StringComparer.Ordinal);
                foreach (var batch in Batches) materials.Add(batch.MaterialId);
                return materials.Count;
            }
        }

        public int PersistentGameObjectCount { get; }
        public int ActiveCosmeticDebris { get; }
    }

    public sealed class VoxelFixtureReplayEvidence
    {
        internal VoxelFixtureReplayEvidence(
            string sequence,
            string firstSha256,
            string resetSha256,
            bool resetReplayMatches,
            string finalState)
        {
            Sequence = sequence;
            FirstSha256 = firstSha256;
            ResetSha256 = resetSha256;
            ResetReplayMatches = resetReplayMatches;
            FinalState = finalState;
        }

        public string Sequence { get; }
        public string FirstSha256 { get; }
        public string ResetSha256 { get; }
        public bool ResetReplayMatches { get; }
        public string FinalState { get; }
    }

    /// <summary>
    /// Pure/editor fixture for the future Unity surface adapter. It consumes
    /// the `.5.3` mapping, keeps cells as records, and produces deterministic
    /// state frames made of render-group batches. No scene objects are made.
    /// </summary>
    public sealed class VoxelHouseRenderFixture
    {
        private const int MaxMappingBytes = 16 * 1024 * 1024;

        private sealed class Cell
        {
            internal string Id;
            internal string PartId;
            internal string RenderGroupId;
            internal string MaterialId;
            internal int[] Coord;
        }

        private sealed class Part
        {
            internal string Id;
            internal string Kind;
            internal List<string> CellIds;
        }

        private sealed class RenderGroup
        {
            internal string Id;
            internal string SurfaceKind;
            internal string MaterialId;
            internal List<string> CellIds;
        }

        private sealed class Opening
        {
            internal string Id;
            internal string Kind;
            internal List<int[]> Coordinates;
        }

        private readonly Dictionary<string, Cell> cellsById;
        private readonly Dictionary<string, Part> partsById;
        private readonly Dictionary<string, RenderGroup> renderGroupsById;
        private readonly List<Opening> openings;
        private readonly List<string> allCellIds;
        private readonly List<string> roomIds;
        private readonly List<string> furnitureIds;
        private readonly List<string> hiddenCutawayPartIds;
        private readonly List<string> breachCellIds;
        private readonly List<string> litWindowIds;
        private readonly int cellsCap;
        private readonly int renderersCap;
        private readonly int materialsCap;
        private readonly int persistentGameObjectsCap;
        private VoxelFixtureState currentState;

        private VoxelHouseRenderFixture(
            Dictionary<string, Cell> cellsById,
            Dictionary<string, Part> partsById,
            Dictionary<string, RenderGroup> renderGroupsById,
            List<Opening> openings,
            List<string> roomIds,
            List<string> furnitureIds,
            int cellsCap,
            int renderersCap,
            int materialsCap,
            int persistentGameObjectsCap)
        {
            this.cellsById = cellsById;
            this.partsById = partsById;
            this.renderGroupsById = renderGroupsById;
            this.openings = openings;
            this.roomIds = Sorted(roomIds);
            this.furnitureIds = Sorted(furnitureIds);
            this.cellsCap = cellsCap;
            this.renderersCap = renderersCap;
            this.materialsCap = materialsCap;
            this.persistentGameObjectsCap = persistentGameObjectsCap;
            allCellIds = new List<string>(cellsById.Keys);
            allCellIds.Sort(StringComparer.Ordinal);
            hiddenCutawayPartIds = new List<string>();
            foreach (var part in partsById.Values)
            {
                if (part.Kind == "roof") hiddenCutawayPartIds.Add(part.Id);
            }
            hiddenCutawayPartIds.Sort(StringComparer.Ordinal);
            litWindowIds = new List<string>();
            foreach (var opening in openings)
            {
                if (opening.Kind == "window") litWindowIds.Add(opening.Id);
            }
            litWindowIds.Sort(StringComparer.Ordinal);
            breachCellIds = BuildBreachCells(cellsById, openings);
            currentState = VoxelFixtureState.Intact;
        }

        public VoxelFixtureState CurrentState => currentState;
        public int CellCount => cellsById.Count;
        public int RoomCount => roomIds.Count;
        public int FurnitureCount => furnitureIds.Count;
        public IReadOnlyList<string> LitWindowMetadata => litWindowIds.AsReadOnly();

        public static VoxelHouseRenderFixture LoadFromMapping(string mappingPath)
        {
            if (string.IsNullOrWhiteSpace(mappingPath) || !Path.IsPathRooted(mappingPath))
                throw new VoxelManifestException("mapping_path", "must be an absolute mapping path");
            var info = new FileInfo(mappingPath);
            if (!info.Exists) throw new VoxelManifestException("mapping_path", "mapping file does not exist");
            if (info.Length > MaxMappingBytes) throw new VoxelManifestException("mapping_path", "mapping exceeds editor input cap");
            var mapping = VoxelJsonParser.Parse(File.ReadAllBytes(mappingPath), "mapping");
            return FromMapping(mapping);
        }

        internal static VoxelHouseRenderFixture FromMapping(VoxelJsonValue mapping)
        {
            mapping.RequireKind(VoxelJsonKind.Object, "mapping");
            ExactInt(mapping, "mapping_version", 1, "mapping.mapping_version");
            var sourceAnchor = ObjectAt(mapping, "source_anchor", "mapping.source_anchor");
            ExactString(sourceAnchor, "kind", "source_aabb_center_bottom", "mapping.source_anchor.kind");
            var sourceToUnity = ObjectAt(mapping, "source_to_unity", "mapping.source_to_unity");
            ExactString(sourceToUnity, "conversion", "SourceToUnity at runtime", "mapping.source_to_unity.conversion");
            ExactString(sourceToUnity, "authority", "source_gameplay_aabb", "mapping.source_to_unity.authority");
            ExactString(sourceToUnity, "anchor_kind", "source_aabb_center_bottom", "mapping.source_to_unity.anchor_kind");
            if (!sourceToUnity.Require("cells_are_data_only", "mapping.source_to_unity").RequireBoolean("mapping.source_to_unity.cells_are_data_only"))
                throw new VoxelManifestException("mapping.source_to_unity.cells_are_data_only", "must be true");
            ExactString(sourceToUnity, "surface_policy", "batched render groups; no per-cell GameObjects", "mapping.source_to_unity.surface_policy");

            var cells = ArrayAt(mapping, "cells", "mapping.cells");
            var parts = ArrayAt(mapping, "parts", "mapping.parts");
            var groups = ArrayAt(mapping, "render_groups", "mapping.render_groups");
            var rooms = ArrayAt(mapping, "rooms", "mapping.rooms");
            var furniture = ArrayAt(mapping, "furniture", "mapping.furniture");
            var openings = ArrayAt(mapping, "openings", "mapping.openings");
            var budgets = ObjectAt(mapping, "budgets", "mapping.budgets");
            var limits = ObjectAt(budgets, "limits", "mapping.budgets.limits");

            var cellsById = new Dictionary<string, Cell>(StringComparer.Ordinal);
            foreach (var value in cells)
            {
                var id = StringAt(value, "id", "mapping.cells[].id");
                if (cellsById.ContainsKey(id)) throw new VoxelManifestException("mapping.cells", "duplicate cell ID");
                var coord = Coord(value.Require("coord", "mapping.cells[].coord"), "mapping.cells[].coord");
                cellsById.Add(id, new Cell
                {
                    Id = id,
                    Coord = coord,
                    PartId = StringAt(value, "part_id", "mapping.cells[].part_id"),
                    RenderGroupId = StringAt(value, "render_group_id", "mapping.cells[].render_group_id"),
                    MaterialId = StringAt(value, "material_id", "mapping.cells[].material_id"),
                });
            }

            var partsById = new Dictionary<string, Part>(StringComparer.Ordinal);
            foreach (var value in parts)
            {
                var id = StringAt(value, "id", "mapping.parts[].id");
                if (partsById.ContainsKey(id)) throw new VoxelManifestException("mapping.parts", "duplicate part ID");
                partsById.Add(id, new Part
                {
                    Id = id,
                    Kind = StringAt(value, "kind", "mapping.parts[].kind"),
                    CellIds = StringArray(value, "cell_ids", "mapping.parts[].cell_ids"),
                });
            }

            var renderGroupsById = new Dictionary<string, RenderGroup>(StringComparer.Ordinal);
            foreach (var value in groups)
            {
                var id = StringAt(value, "id", "mapping.render_groups[].id");
                if (renderGroupsById.ContainsKey(id)) throw new VoxelManifestException("mapping.render_groups", "duplicate render-group ID");
                renderGroupsById.Add(id, new RenderGroup
                {
                    Id = id,
                    SurfaceKind = StringAt(value, "surface_kind", "mapping.render_groups[].surface_kind"),
                    MaterialId = StringAt(value, "material_id", "mapping.render_groups[].material_id"),
                    CellIds = StringArray(value, "cell_ids", "mapping.render_groups[].cell_ids"),
                });
            }

            var openingRecords = new List<Opening>();
            foreach (var value in openings)
            {
                openingRecords.Add(new Opening
                {
                    Id = StringAt(value, "id", "mapping.openings[].id"),
                    Kind = StringAt(value, "kind", "mapping.openings[].kind"),
                    Coordinates = CoordinateArray(value.Require("cell_coords", "mapping.openings[].cell_coords"), "mapping.openings[].cell_coords"),
                });
            }

            var roomIds = new List<string>();
            foreach (var room in rooms) roomIds.Add(StringAt(room, "id", "mapping.rooms[].id"));
            var furnitureIds = new List<string>();
            foreach (var item in furniture) furnitureIds.Add(StringAt(item, "id", "mapping.furniture[].id"));

            var fixture = new VoxelHouseRenderFixture(
                cellsById,
                partsById,
                renderGroupsById,
                openingRecords,
                roomIds,
                furnitureIds,
                BudgetLimit(limits, "cells"),
                BudgetLimit(limits, "renderers"),
                BudgetLimit(limits, "materials"),
                BudgetLimit(limits, "persistent_game_objects"));
            fixture.ValidateReferences();
            return fixture;
        }

        public VoxelFixtureFrame Render(VoxelFixtureState state)
        {
            currentState = state;
            var hiddenParts = state == VoxelFixtureState.Cutaway
                ? new List<string>(hiddenCutawayPartIds)
                : new List<string>();
            var hiddenPartSet = new HashSet<string>(hiddenParts, StringComparer.Ordinal);
            var breachCells = state == VoxelFixtureState.Breach
                ? new List<string>(breachCellIds)
                : new List<string>();
            var breachSet = new HashSet<string>(breachCells, StringComparer.Ordinal);
            var visibleCellIds = new List<string>();
            foreach (var cellId in allCellIds)
            {
                var cell = cellsById[cellId];
                if (hiddenPartSet.Contains(cell.PartId) || breachSet.Contains(cell.Id)) continue;
                visibleCellIds.Add(cell.Id);
            }

            var activeBatches = new List<VoxelFixtureBatch>();
            foreach (var groupId in SortedKeys(renderGroupsById))
            {
                var group = renderGroupsById[groupId];
                var visibleInGroup = new List<string>();
                foreach (var cellId in group.CellIds)
                {
                    if (visibleCellIds.BinarySearch(cellId, StringComparer.Ordinal) >= 0) visibleInGroup.Add(cellId);
                }
                if (visibleInGroup.Count != 0)
                    activeBatches.Add(new VoxelFixtureBatch(group.Id, group.SurfaceKind, group.MaterialId, visibleInGroup));
            }

            var exposedRooms = state == VoxelFixtureState.Cutaway || state == VoxelFixtureState.Breach
                ? new List<string>(roomIds)
                : new List<string>();
            var litWindows = state == VoxelFixtureState.NightWindow
                ? new List<string>(litWindowIds)
                : new List<string>();
            var cameraSide = state == VoxelFixtureState.Opposite ? "opposite" : "source-facing";
            var frameObject = BuildFrameObject(
                state,
                cameraSide,
                visibleCellIds,
                hiddenParts,
                breachCells,
                litWindows,
                roomIds,
                furnitureIds,
                exposedRooms,
                activeBatches);
            var serialized = VoxelJsonCanonical.Write(frameObject) + "\n";
            var sha256 = Sha256(new UTF8Encoding(false).GetBytes(serialized));
            var frame = new VoxelFixtureFrame(
                state,
                cameraSide,
                visibleCellIds,
                hiddenParts,
                breachCells,
                litWindows,
                new List<string>(roomIds),
                new List<string>(furnitureIds),
                exposedRooms,
                activeBatches,
                serialized,
                sha256,
                0,
                0);
            ValidateFrame(frame);
            return frame;
        }

        public void Reset()
        {
            currentState = VoxelFixtureState.Intact;
        }

        public VoxelFixtureReplayEvidence Replay(IReadOnlyList<VoxelFixtureState> sequence)
        {
            var states = sequence == null || sequence.Count == 0
                ? new List<VoxelFixtureState>
                {
                    VoxelFixtureState.Intact,
                    VoxelFixtureState.Opposite,
                    VoxelFixtureState.NightWindow,
                    VoxelFixtureState.Cutaway,
                    VoxelFixtureState.Breach,
                }
                : new List<VoxelFixtureState>(sequence);
            var first = ReplayOnce(states);
            Reset();
            var second = ReplayOnce(states);
            Reset();
            return new VoxelFixtureReplayEvidence(
                string.Join(",", states),
                first.sha256,
                second.sha256,
                string.Equals(first.sha256, second.sha256, StringComparison.Ordinal),
                states[states.Count - 1].ToString());
        }

        public string BuildReportJson(IReadOnlyList<VoxelFixtureState> sequence = null)
        {
            var replay = Replay(sequence);
            var frames = new List<VoxelJsonValue>();
            var states = sequence == null || sequence.Count == 0
                ? new[] { VoxelFixtureState.Intact, VoxelFixtureState.Opposite, VoxelFixtureState.NightWindow, VoxelFixtureState.Cutaway, VoxelFixtureState.Breach }
                : new List<VoxelFixtureState>(sequence).ToArray();
            foreach (var state in states)
            {
                var frame = Render(state);
                frames.Add(VoxelJsonValue.Object(new Dictionary<string, VoxelJsonValue>(StringComparer.Ordinal)
                {
                    ["state"] = VoxelJsonValue.String(state.ToString()),
                    ["sha256"] = VoxelJsonValue.String(frame.Sha256),
                    ["visible_cells"] = VoxelJsonValue.Number(frame.VisibleCellCount.ToString(CultureInfo.InvariantCulture)),
                    ["batches"] = VoxelJsonValue.Number(frame.BatchCount.ToString(CultureInfo.InvariantCulture)),
                    ["rooms"] = VoxelJsonValue.Number(frame.RoomIds.Count.ToString(CultureInfo.InvariantCulture)),
                    ["furniture"] = VoxelJsonValue.Number(frame.FurnitureIds.Count.ToString(CultureInfo.InvariantCulture)),
                    ["lit_windows"] = VoxelJsonValue.Number(frame.LitWindowIds.Count.ToString(CultureInfo.InvariantCulture)),
                    ["persistent_game_objects"] = VoxelJsonValue.Number(frame.PersistentGameObjectCount.ToString(CultureInfo.InvariantCulture)),
                    ["active_cosmetic_debris"] = VoxelJsonValue.Number(frame.ActiveCosmeticDebris.ToString(CultureInfo.InvariantCulture)),
                }));
            }
            var report = VoxelJsonValue.Object(new Dictionary<string, VoxelJsonValue>(StringComparer.Ordinal)
            {
                ["fixture_version"] = VoxelJsonValue.Number("1"),
                ["cells_are_data_only"] = VoxelJsonValue.Boolean(true),
                ["surface_policy"] = VoxelJsonValue.String("batched render groups; no per-cell GameObjects"),
                ["cell_count"] = VoxelJsonValue.Number(CellCount.ToString(CultureInfo.InvariantCulture)),
                ["room_count"] = VoxelJsonValue.Number(RoomCount.ToString(CultureInfo.InvariantCulture)),
                ["furniture_count"] = VoxelJsonValue.Number(FurnitureCount.ToString(CultureInfo.InvariantCulture)),
                ["lit_window_metadata_count"] = VoxelJsonValue.Number(litWindowIds.Count.ToString(CultureInfo.InvariantCulture)),
                ["replay"] = VoxelJsonValue.Object(new Dictionary<string, VoxelJsonValue>(StringComparer.Ordinal)
                {
                    ["sequence"] = VoxelJsonValue.String(replay.Sequence),
                    ["first_sha256"] = VoxelJsonValue.String(replay.FirstSha256),
                    ["reset_sha256"] = VoxelJsonValue.String(replay.ResetSha256),
                    ["reset_replay_matches"] = VoxelJsonValue.Boolean(replay.ResetReplayMatches),
                    ["final_state"] = VoxelJsonValue.String(replay.FinalState),
                }),
                ["frames"] = VoxelJsonValue.Array(frames),
            });
            return VoxelJsonCanonical.Write(report) + "\n";
        }

        private (string sha256, VoxelFixtureFrame frame) ReplayOnce(List<VoxelFixtureState> states)
        {
            var trace = new StringBuilder();
            VoxelFixtureFrame finalFrame = null;
            foreach (var state in states)
            {
                finalFrame = Render(state);
                trace.Append(finalFrame.Serialized);
            }
            return (Sha256(new UTF8Encoding(false).GetBytes(trace.ToString())), finalFrame);
        }

        private void ValidateReferences()
        {
            if (cellsById.Count == 0 || cellsById.Count > cellsCap) throw new VoxelManifestException("mapping.cells", "outside renderer cap");
            var partMembership = new HashSet<string>(StringComparer.Ordinal);
            foreach (var part in partsById.Values)
            {
                foreach (var cellId in part.CellIds)
                {
                    if (!cellsById.ContainsKey(cellId) || !partMembership.Add(cellId))
                        throw new VoxelManifestException("mapping.parts." + part.Id, "invalid or overlapping cell membership");
                }
            }
            if (partMembership.Count != cellsById.Count) throw new VoxelManifestException("mapping.parts", "parts do not cover every cell");

            var groupMembership = new HashSet<string>(StringComparer.Ordinal);
            foreach (var group in renderGroupsById.Values)
            {
                foreach (var cellId in group.CellIds)
                {
                    if (!cellsById.ContainsKey(cellId) || !groupMembership.Add(cellId))
                        throw new VoxelManifestException("mapping.render_groups." + group.Id, "invalid or overlapping cell membership");
                }
            }
            if (groupMembership.Count != cellsById.Count) throw new VoxelManifestException("mapping.render_groups", "render groups do not cover every cell");

            foreach (var cell in cellsById.Values)
            {
                if (!partsById.ContainsKey(cell.PartId)) throw new VoxelManifestException("mapping.cells." + cell.Id, "unknown part");
                if (!renderGroupsById.ContainsKey(cell.RenderGroupId)) throw new VoxelManifestException("mapping.cells." + cell.Id, "unknown render group");
                if (!renderGroupsById[cell.RenderGroupId].CellIds.Contains(cell.Id)) throw new VoxelManifestException("mapping.cells." + cell.Id, "render-group membership mismatch");
                if (!partsById[cell.PartId].CellIds.Contains(cell.Id)) throw new VoxelManifestException("mapping.cells." + cell.Id, "part membership mismatch");
                if (!string.Equals(renderGroupsById[cell.RenderGroupId].MaterialId, cell.MaterialId, StringComparison.Ordinal)) throw new VoxelManifestException("mapping.cells." + cell.Id, "render-group material mismatch");
            }
            if (renderGroupsById.Count > renderersCap) throw new VoxelManifestException("mapping.render_groups", "outside renderer cap");
        }

        private void ValidateFrame(VoxelFixtureFrame frame)
        {
            if (frame.VisibleCellCount > cellsCap) throw new VoxelManifestException("fixture.visible_cells", "outside cell cap");
            if (frame.RendererCount > renderersCap) throw new VoxelManifestException("fixture.renderers", "outside renderer cap");
            if (frame.MaterialCount > materialsCap) throw new VoxelManifestException("fixture.materials", "outside material cap");
            if (frame.PersistentGameObjectCount > persistentGameObjectsCap) throw new VoxelManifestException("fixture.persistent_game_objects", "outside cap");
            if (frame.PersistentGameObjectCount != 0) throw new VoxelManifestException("fixture.persistent_game_objects", "per-cell scene objects are forbidden");
            if (frame.ActiveCosmeticDebris < 0) throw new VoxelManifestException("fixture.active_cosmetic_debris", "cannot be negative");
        }

        private VoxelJsonValue BuildFrameObject(
            VoxelFixtureState state,
            string cameraSide,
            List<string> visibleCellIds,
            List<string> hiddenPartIds,
            List<string> breachCells,
            List<string> litWindows,
            List<string> rooms,
            List<string> furniture,
            List<string> exposedRooms,
            List<VoxelFixtureBatch> batches)
        {
            var batchValues = new List<VoxelJsonValue>();
            foreach (var batch in batches)
            {
                batchValues.Add(VoxelJsonValue.Object(new Dictionary<string, VoxelJsonValue>(StringComparer.Ordinal)
                {
                    ["id"] = VoxelJsonValue.String(batch.Id),
                    ["surface_kind"] = VoxelJsonValue.String(batch.SurfaceKind),
                    ["material_id"] = VoxelJsonValue.String(batch.MaterialId),
                    ["cell_ids"] = StringArray(batch.CellIds),
                }));
            }
            return VoxelJsonValue.Object(new Dictionary<string, VoxelJsonValue>(StringComparer.Ordinal)
            {
                ["state"] = VoxelJsonValue.String(state.ToString()),
                ["camera_side"] = VoxelJsonValue.String(cameraSide),
                ["cells_are_data_only"] = VoxelJsonValue.Boolean(true),
                ["visible_cell_ids"] = StringArray(visibleCellIds),
                ["hidden_part_ids"] = StringArray(hiddenPartIds),
                ["breach_cell_ids"] = StringArray(breachCells),
                ["lit_window_ids"] = StringArray(litWindows),
                ["room_ids"] = StringArray(rooms),
                ["furniture_ids"] = StringArray(furniture),
                ["exposed_room_ids"] = StringArray(exposedRooms),
                ["batches"] = VoxelJsonValue.Array(batchValues),
                ["persistent_game_objects"] = VoxelJsonValue.Number("0"),
                ["active_cosmetic_debris"] = VoxelJsonValue.Number("0"),
                ["surface_policy"] = VoxelJsonValue.String("batched render groups; no per-cell GameObjects"),
            });
        }

        private static List<string> BuildBreachCells(Dictionary<string, Cell> cells, List<Opening> openings)
        {
            var result = new HashSet<string>(StringComparer.Ordinal);
            if (openings.Count != 0)
            {
                var target = openings[0];
                foreach (var cell in cells.Values)
                {
                    foreach (var openingCoord in target.Coordinates)
                    {
                        if (Manhattan(cell.Coord, openingCoord) == 1)
                        {
                            result.Add(cell.Id);
                            break;
                        }
                    }
                }
                if (result.Count == 0)
                {
                    var nearestDistance = int.MaxValue;
                    foreach (var cell in cells.Values)
                    {
                        var distance = int.MaxValue;
                        foreach (var openingCoord in target.Coordinates) distance = Math.Min(distance, Manhattan(cell.Coord, openingCoord));
                        nearestDistance = Math.Min(nearestDistance, distance);
                    }
                    foreach (var cell in cells.Values)
                    {
                        foreach (var openingCoord in target.Coordinates)
                        {
                            if (Manhattan(cell.Coord, openingCoord) == nearestDistance) result.Add(cell.Id);
                        }
                    }
                }
            }
            var sorted = new List<string>(result);
            sorted.Sort(StringComparer.Ordinal);
            return sorted;
        }

        private static int Manhattan(int[] left, int[] right)
        {
            return Math.Abs(left[0] - right[0]) + Math.Abs(left[1] - right[1]) + Math.Abs(left[2] - right[2]);
        }

        private static int BudgetLimit(VoxelJsonValue limits, string key)
        {
            var value = limits.Require(key, "mapping.budgets.limits." + key).RequireInt64("mapping.budgets.limits." + key);
            if (value < 0 || value > int.MaxValue) throw new VoxelManifestException("mapping.budgets.limits." + key, "invalid integer cap");
            return (int)value;
        }

        private static VoxelJsonValue ObjectAt(VoxelJsonValue root, string key, string path)
        {
            return root.Require(key, path).RequireKind(VoxelJsonKind.Object, path);
        }

        private static List<VoxelJsonValue> ArrayAt(VoxelJsonValue root, string key, string path)
        {
            return root.Require(key, path).RequireKind(VoxelJsonKind.Array, path).ArrayValue;
        }

        private static string StringAt(VoxelJsonValue root, string key, string path)
        {
            return root.Require(key, path).RequireString(path);
        }

        private static void ExactString(VoxelJsonValue root, string key, string expected, string path)
        {
            if (!string.Equals(StringAt(root, key, path), expected, StringComparison.Ordinal)) throw new VoxelManifestException(path, "must be '" + expected + "'");
        }

        private static void ExactInt(VoxelJsonValue root, string key, long expected, string path)
        {
            if (root.Require(key, path).RequireInt64(path) != expected) throw new VoxelManifestException(path, "must be " + expected.ToString(CultureInfo.InvariantCulture));
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

        private static List<int[]> CoordinateArray(VoxelJsonValue value, string path)
        {
            var coordinates = new List<int[]>();
            foreach (var item in value.RequireKind(VoxelJsonKind.Array, path).ArrayValue) coordinates.Add(Coord(item, path + "[]"));
            if (coordinates.Count == 0) throw new VoxelManifestException(path, "must not be empty");
            return coordinates;
        }

        private static List<string> StringArray(VoxelJsonValue value, string key, string path)
        {
            return StringArray(ArrayAt(value, key, path));
        }

        private static VoxelJsonValue StringArray(IReadOnlyList<string> values)
        {
            var array = new List<VoxelJsonValue>();
            foreach (var value in values) array.Add(VoxelJsonValue.String(value));
            return VoxelJsonValue.Array(array);
        }

        private static List<string> StringArray(List<VoxelJsonValue> values)
        {
            var result = new List<string>();
            foreach (var value in values) result.Add(value.RequireString("mapping.string_array[]"));
            result.Sort(StringComparer.Ordinal);
            return result;
        }

        private static List<string> Sorted(List<string> values)
        {
            var result = new List<string>(values);
            result.Sort(StringComparer.Ordinal);
            return result;
        }

        private static List<string> SortedKeys<T>(Dictionary<string, T> values)
        {
            var keys = new List<string>(values.Keys);
            keys.Sort(StringComparer.Ordinal);
            return keys;
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
