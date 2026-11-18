using System;
using System.Collections.Generic;
using System.IO;
using System.Text;
using UnityEngine;

namespace MagicCarpet.Native
{
    public sealed partial class MagicCarpetRuntime
    {
        // sub_6DC40_improve_ability -> SetCurrentNotificationMessage_19760.
        // DrawTextPauseEndOfLevel consumes one count per outer draw, including
        // paused draws; multiple Unity IMGUI passes must not consume it twice.
        internal sealed class AbilityImprovedNotification
        {
            private readonly Func<int, string> record;
            private int remaining;
            internal string Text { get; private set; }
            internal bool Visible { get; private set; }

            internal AbilityImprovedNotification(byte[] language)
            {
                var records = Encoding.GetEncoding(28591).GetString(language).Split('\0');
                if (records.Length <= 185 || string.IsNullOrEmpty(records[159]))
                    throw new InvalidDataException("Missing source ability notification language");
                record = index => index >= 0 && index < records.Length ? records[index] : null;
            }

            internal AbilityImprovedNotification(Func<int, string> selectedRecord)
            {
                record = selectedRecord ?? throw new ArgumentNullException(nameof(selectedRecord));
                if (string.IsNullOrEmpty(record(159)))
                    throw new InvalidDataException("Missing source ability notification language");
            }

            internal void Improve(int family)
            {
                if (family < 0 || family >= 26 || string.IsNullOrEmpty(record(160 + family)))
                    throw new InvalidDataException("Unbound source spell notification family");
                var format = record(159);
                var placeholder = format.IndexOf("%s", StringComparison.Ordinal);
                if (placeholder < 0 || format.IndexOf("%s", placeholder + 2, StringComparison.Ordinal) >= 0)
                    throw new InvalidDataException("Invalid source spell notification format");
                var text = format.Substring(0, placeholder) + record(160 + family) + format.Substring(placeholder + 2);
                // Source notification storage is 49 bytes, including NUL.
                Text = text.Length > 48 ? text.Substring(0, 48) : text;
                remaining = 200;
                Visible = false;
            }

            internal void Advance(bool suppressed)
            {
                Visible = !suppressed && remaining > 0;
                if (Visible) remaining--;
            }
        }

        private AbilityImprovedNotification gameRuntimeAbilityNotification;
        // Shared source popup scanner for currently bound ordinary rows and
        // forced objective context. The outer source frame is the only clock.
        internal sealed class ManaHelpHint
        {
            private sealed class RowState
            {
                internal readonly OwnerCampaignMenuCache.GameplayHelpBinding Binding;
                internal int Age;
                internal int Priority;

                internal RowState(OwnerCampaignMenuCache.GameplayHelpBinding binding)
                {
                    Binding = binding ?? throw new InvalidDataException("Missing source help row");
                    Reset();
                }

                internal void Reset()
                {
                    Age = Binding.AgeThreshold;
                    // sub_88B20 initially promotes source type7 context rows.
                    Priority = Binding.Row >= 72 && Binding.Row <= 80 ? 6 : Binding.Priority;
                }
            }

            private readonly Func<int, OwnerCampaignMenuCache.GameplayHelpBinding> rowFor;
            private readonly RowState[] rows;
            private readonly RowState mana;
            private readonly Func<sbyte> sourceObjectiveStageKind;
            // SetTextBoxMinMaxSizes_87090 clears dword_0x72; the 20-pass
            // delay is installed only by suppression or sub_87C10.
            private int pollDelay;
            private int remaining;
            private int idleDraws;
            private RowState currentRow;
            private ushort objectiveHelpSubject;
            private int pendingRow;
            private bool spellMode;
            private bool fireballHelpPending;
            private int spellSlot = -1, spellTier = -1, spellCycle;
            private ushort[] spellRecords;
            private int spellHighlight = -1;
            internal int Priority => mana.Priority;
            internal bool Visible { get; private set; }
            internal int VisibleRow { get; private set; }
            internal int AnchorX { get; private set; } = 540;
            internal int AnchorY { get; private set; }
            internal bool SpellVisible { get; private set; }
            internal bool HasObjectiveMarkHelp { get; }
            internal bool HasLargeMapHelp { get; }
            internal bool HasFireballHelp { get; }
            internal bool PollsObjectiveMarkHelp =>
                HasObjectiveMarkHelp && remaining == 0 && pollDelay <= 1 && pendingRow == 0;
            internal ushort[] VisibleSpellRecords { get; private set; }
            internal int VisibleSpellHighlight { get; private set; } = -1;

            internal ManaHelpHint(Func<int, OwnerCampaignMenuCache.GameplayHelpBinding> binding,
                bool hasMinimapHelp = false, bool hasObjectiveMarkHelp = false, bool hasLargeMapHelp = false,
                bool hasLowLifeHelp = false, bool hasIdleHelp = false, bool hasVitalsHelp = false,
                bool hasCastHelp = false, bool hasSpellSearchHelp = false, bool hasCastleSpellHelp = false,
                bool hasFireballHelp = false, Func<sbyte> sourceObjectiveStageKind = null)
            {
                rowFor = binding ?? throw new ArgumentNullException(nameof(binding));
                HasObjectiveMarkHelp = hasObjectiveMarkHelp;
                HasLargeMapHelp = hasLargeMapHelp;
                HasFireballHelp = hasFireballHelp;
                this.sourceObjectiveStageKind = sourceObjectiveStageKind;
                // Keep source row order: equal overdue ages retain the first
                // eligible row in GetHelpPopupIndex_88450.
                var ids = new List<int> { 1, 3, 40, 41, 42, 69, 70, 71,
                    72, 73, 74, 75, 76, 77, 78, 79, 80, 85 };
                if (hasMinimapHelp) ids.Insert(1, 2);
                if (hasLargeMapHelp) ids.Insert(ids.IndexOf(40), 4);
                if (hasLowLifeHelp) ids.Insert(ids.IndexOf(40), 5);
                if (hasSpellSearchHelp) ids.InsertRange(ids.IndexOf(40), new[] { 6, 8 });
                if (hasCastleSpellHelp) ids.InsertRange(ids.IndexOf(40), new[] { 12, 13 });
                if (hasFireballHelp)
                {
                    ids.Insert(ids.FindIndex(row => row > 7), 7);
                    ids.Insert(ids.FindIndex(row => row > 10), 10);
                }
                if (hasIdleHelp) ids.InsertRange(ids.IndexOf(40), new[] { 15, 16 });
                if (hasVitalsHelp) ids.InsertRange(ids.IndexOf(40), new[] { 36, 37 });
                if (hasCastHelp) ids.InsertRange(ids.IndexOf(40), new[] { 38, 39 });
                if (hasObjectiveMarkHelp) ids.InsertRange(ids.IndexOf(85), new[] { 81, 82 });
                rows = new RowState[ids.Count];
                for (var index = 0; index < ids.Count; index++) rows[index] = new RowState(rowFor(ids[index]));
                mana = FindRow(41);
            }

            internal void ApplyHelpMode(bool enabled)
            {
                Visible = false;
                VisibleRow = 0;
                SpellVisible = false;
                VisibleSpellRecords = null;
                VisibleSpellHighlight = -1;
                if (!enabled) return;
                // SetTextBoxMinMaxSizes clears the shared popup carrier and
                // restores every retained source row before help resumes.
                foreach (var row in rows) row.Reset();
                pollDelay = 0;
                remaining = 0;
                idleDraws = 0;
                currentRow = null;
                objectiveHelpSubject = 0;
                pendingRow = 0;
                fireballHelpPending = false;
                spellMode = false;
                spellSlot = spellTier = -1;
                spellCycle = 0;
                spellRecords = null;
                spellHighlight = -1;
                AnchorX = 540;
                AnchorY = 0;
            }

            internal void Promote(int hint, int priority)
            {
                if (hint != 41 || priority != 6)
                    throw new InvalidDataException("Unbound campaign help hint");
                mana.Priority = priority;
            }

            internal void ObservePlayerSpeed(short actualSpeed, bool showHelp)
            {
                // sub_872A0 observes real speed before choosing the ordinary,
                // paused, spell, or suppressed help branch.
                if (showHelp) idleDraws = actualSpeed == 0 ? unchecked(idleDraws + 1) : 0;
            }

            internal void Advance(bool leftUnavailable, bool rightUnavailable,
                bool enabled, bool paused, bool suppressed, sbyte wallContactCounter = 0,
                bool levelEnd = false, bool minimapEnabled = true, bool mapOpen = false,
                IReadOnlyList<(byte Row, ushort Subject, int X, int Y)> objectiveMarks = null,
                ushort nearestObjectiveSubject = 0, bool? hasObjectiveHelpSubject = null,
                bool lowLife = false, bool leftAssigned = false, bool rightAssigned = false,
                bool noAcquiredSpells = false, bool missingCastleSpell = false,
                bool hasCastleSpellWithoutCastle = false, int? castleHandAnchorX = null,
                bool? fireballSpellEnabled = null, int? fireballHandAnchorX = null)
            {
                Visible = false;
                VisibleRow = 0;
                SpellVisible = false;
                if (spellMode)
                {
                    spellMode = false;
                    spellSlot = spellTier = -1;
                    spellCycle = 0;
                    if (remaining != 0) Close();
                }
                // DrawHelpPopUps_871F0 enters its pause branch instead of
                // advancing sub_87B30/sub_87B70. Keep the current popup carrier.
                if (!enabled || paused) return;
                if (suppressed)
                {
                    if (remaining != 0) Close();
                    pollDelay = 20;
                    return;
                }
                if (remaining != 0)
                {
                    if (currentRow.Binding.Row == 81 || currentRow.Binding.Row == 82)
                    {
                        if (!RefreshObjectiveMarkAnchor(objectiveMarks))
                        {
                            Close();
                            AgeRows();
                            return;
                        }
                    }
                    // Eligibility is sampled by SetHelpPopupCoords only when
                    // selecting ordinary rows. Type8 also requires the same
                    // subject pointer to have refreshed before each popup draw.
                    Visible = true;
                    VisibleRow = currentRow.Binding.Row;
                    if (--remaining == 0) Close();
                }
                else if (--pollDelay <= 0)
                {
                    // GetHelpPopupTextAndCoords runs before the forced-row
                    // and priority decisions. A masked or losing cue consumes
                    // the request too, provided at least one holder is enabled.
                    var fireballRequested = fireballHelpPending && !noAcquiredSpells;
                    if (fireballRequested) fireballHelpPending = false;
                    if (pendingRow != 0)
                    {
                        currentRow = FindRow(pendingRow);
                        pendingRow = 0;
                    }
                    else
                    {
                        var hasFixedObjectiveMark = false;
                        if (objectiveMarks != null)
                            foreach (var mark in objectiveMarks)
                                hasFixedObjectiveMark |= mark.Row == 81;
                        // Current eligibility is sampled only by the source
                        // poll. A contact event does not promote or select row3.
                        var bestOverdue = 0;
                        for (var priority = 6; priority >= 0 && currentRow == null; priority--)
                        {
                            foreach (var row in rows)
                            {
                                var eligible = row.Binding.Row switch {
                                    1 or 2 => true,
                                    3 => wallContactCounter != 0,
                                    4 => !mapOpen && hasObjectiveHelpSubject == false,
                                    5 => lowLife && !mapOpen,
                                    6 => noAcquiredSpells,
                                    7 => fireballRequested && fireballSpellEnabled == false,
                                    8 => missingCastleSpell,
                                    10 => fireballRequested && fireballSpellEnabled == true && !mapOpen,
                                    12 => hasCastleSpellWithoutCastle && !mapOpen,
                                    13 => hasCastleSpellWithoutCastle && castleHandAnchorX == null,
                                    15 or 16 => idleDraws > 100,
                                    36 or 37 => !mapOpen,
                                    38 => leftAssigned && !mapOpen,
                                    39 => rightAssigned && !mapOpen,
                                    41 => leftUnavailable || rightUnavailable,
                                    81 => hasFixedObjectiveMark,
                                    82 => nearestObjectiveSubject != 0,
                                    _ => false
                                };
                                // The source end mask includes the eligible bit
                                // just set on these rows. Forced word88 bypasses it.
                                if (levelEnd) eligible = false;
                                // The source separately masks minimap-related
                                // rows whenever icons and map are hidden.
                                if (!minimapEnabled && (row.Binding.Flags & 0x2ff) != 0) eligible = false;
                                var overdue = row.Age - row.Binding.AgeThreshold;
                                if (!eligible || row.Priority != priority || overdue <= bestOverdue) continue;
                                currentRow = row;
                                bestOverdue = overdue;
                            }
                        }
                    }
                    if (currentRow != null)
                    {
                        if (currentRow.Binding.Row == 2)
                        {
                            // GetHelpPopupTextAndCoords_87CF0 sets the anchor when polling.
                            AnchorX = mapOpen ? 190 : 64;
                            AnchorY = mapOpen ? 202 : 66;
                        }
                        else if (currentRow.Binding.Row == 10 && fireballHandAnchorX.HasValue)
                            (AnchorX, AnchorY) = (fireballHandAnchorX.Value, 32);
                        else if (currentRow.Binding.Row == 12 && castleHandAnchorX.HasValue)
                            (AnchorX, AnchorY) = (castleHandAnchorX.Value, 32);
                        else if (currentRow.Binding.Row is 5 or 7 or 10 or 12 or 13 or 36 or 37 or 38 or 39)
                            (AnchorX, AnchorY) = (currentRow.Binding.X, currentRow.Binding.Y);
                        else if (currentRow.Binding.Row == 41)
                        {
                            AnchorX = leftUnavailable ? 540 : 600;
                            AnchorY = 48;
                        }
                        else if (currentRow.Binding.Row == 82)
                            objectiveHelpSubject = nearestObjectiveSubject;
                        remaining = currentRow.Binding.Duration;
                    }
                    // SetHelpPopupTextAndCoords_884D0 selects this frame;
                    // DrawPopupTextBox_87610 starts on the following frame.
                }
                // sub_87B70 services every source row after selection/draw,
                // including rows that are not currently eligible.
                AgeRows();
            }

            private bool RefreshObjectiveMarkAnchor(
                IReadOnlyList<(byte Row, ushort Subject, int X, int Y)> marks)
            {
                // The source fixed-point hint passes a null subject. It may
                // win the poll but fails sub_872A0's nonnull active Type8 guard.
                if (objectiveHelpSubject == 0 || marks == null) return false;
                foreach (var mark in marks)
                {
                    if (mark.Row != 82 || mark.Subject != objectiveHelpSubject) continue;
                    AnchorX = mark.X;
                    AnchorY = mark.Y;
                    return true;
                }
                return false;
            }

            private void AgeRows()
            {
                foreach (var row in rows)
                {
                    row.Age = unchecked(row.Age + 1);
                    if (row.Age > row.Binding.AgeThreshold * 3)
                    {
                        row.Age = row.Binding.AgeThreshold;
                        row.Priority = Math.Max(0, Math.Min(5, row.Priority + 1));
                    }
                }
            }

            // The same Index_0x86 / dword_0x72 carrier services spell help.
            // This is called once for a real source presentation frame, never
            // from IMGUI repaint or a state query.
            internal void AdvanceSpell(EngineSessionTransport.SpellObservation spell, int slot, int tier,
                bool hasAny, bool enabled, bool suppressed, int panelWidth, int panelHeight,
                int tierWidth, int tierHeight)
            {
                Visible = false; VisibleRow = 0; SpellVisible = false;
                if (!enabled) return;
                spellMode = true;
                if (slot < 0 || slot >= 26 || tier < 0 || tier >= 3 || panelWidth <= 0 ||
                    panelHeight <= 0 || tierWidth <= 0 || tierHeight <= 0)
                    throw new InvalidDataException("invalid source spell help selection or geometry");
                var help = spell?.menu_help;
                var standard = help?.is_standard == true;
                if (slot != spellSlot || tier != spellTier)
                {
                    if (spellCycle > 1 || !standard) Close();
                    spellCycle = 0;
                    SelectSpell(spell, slot, tier, hasAny, panelWidth, panelHeight, tierWidth, tierHeight);
                    spellSlot = slot; spellTier = tier;
                }
                if (suppressed)
                {
                    if (remaining != 0) Close();
                    pollDelay = 6;
                    return;
                }
                if (remaining != 0)
                {
                    Visible = SpellVisible = true;
                    VisibleRow = currentRow.Binding.Row;
                    VisibleSpellRecords = spellRecords;
                    VisibleSpellHighlight = spellHighlight;
                    if (--remaining == 0) Close();
                }
                else if (--pollDelay <= 0)
                {
                    SelectSpell(spell, slot, tier, hasAny, panelWidth, panelHeight, tierWidth, tierHeight);
                }
                AgeRows();
            }

            private void SelectSpell(EngineSessionTransport.SpellObservation spell, int slot, int tier,
                bool hasAny, int panelWidth, int panelHeight, int tierWidth, int tierHeight)
            {
                if (!hasAny) return;
                var help = spell?.menu_help;
                var standard = help?.is_standard == true;
                if (standard && (help.title_records == null || help.title_records.Length != 3 ||
                    help.castle_blocked == null || help.castle_blocked.Length != 3))
                    throw new InvalidDataException("incomplete source spell help observation");
                var top = 480 - 2 * panelHeight;
                var gridX = 8 + panelWidth * (slot % 13) + panelWidth / 2;
                var gridY = top + panelHeight * (slot / 13) + panelHeight / 2;
                var left = Math.Max(0, Math.Min(640 - 3 * tierWidth, gridX - 3 * tierWidth / 2));
                var tierTop = top - tierHeight;
                var tierY = tierTop + tierHeight / 2;
                var row = 0; var x = 0; var y = 0; var stop = false;
                for (var scan = 0; scan < 8 && !stop; scan++)
                {
                    switch (spellCycle)
                    {
                        case 0:
                            if (standard) { row = 85; x = left + 3 * tierWidth / 2; y = tierTop; stop = true; }
                            break;
                        case 1:
                            if (standard) { row = 69; x = left + 33 + (spell.unlocked_tier > 0 ? tierWidth : 0); y = tierTop + 29; stop = true; }
                            break;
                        case 2:
                            if (standard)
                                for (var j = 0; j < 3 && !stop; j++)
                                    if (help.castle_blocked[j]) { row = 70; x = left + j * tierWidth + tierWidth / 2; y = tierY; stop = true; }
                            break;
                        case 3:
                            if (standard && spell.unlocked_tier > 0) { row = 71; x = left + tierWidth / 2 + spell.unlocked_tier * tierWidth; y = tierY; stop = true; }
                            break;
                        case 4:
                            // Source leaves endLoop false here: a later
                            // qualifying case may replace this candidate.
                            if (standard && !help.castle_blocked[tier]) { row = 40; x = gridX; y = (gridY + panelHeight - 8) >> 2; }
                            break;
                        case 5:
                            if (standard) { row = 42; x = left + tierWidth / 2 + tier * tierWidth; y = tierY; stop = true; }
                            break;
                        case 6:
                            if (standard && help.mana_unavailable) { row = 41; x = gridX; y = (gridY + panelHeight - 8) >> 2; stop = true; }
                            break;
                    }
                    spellCycle = (spellCycle + 1) & 7;
                }
                if (row == 0) return;
                currentRow = FindRow(row);
                remaining = currentRow.Binding.Duration;
                AnchorX = x; AnchorY = y;
                spellRecords = row == 85 ? (ushort[])help.title_records.Clone() :
                    new[] { checked((ushort)currentRow.Binding.TextRecord) };
                spellHighlight = row == 85 ? tier : -1;
            }

            private RowState FindRow(int row)
            {
                foreach (var state in rows)
                    if (state.Binding.Row == row) return state;
                throw new InvalidDataException("Unbound source help row");
            }

            private void Close()
            {
                remaining = 0;
                if (currentRow != null)
                {
                    currentRow.Age = 0;
                    currentRow.Priority = currentRow.Binding.Priority;
                    var closedRow = currentRow.Binding.Row;
                    if (HasFireballHelp && (closedRow is 19 or 25 or 26 or 28 or 82 ||
                        closedRow is >= 72 and <= 80 &&
                        sourceObjectiveStageKind?.Invoke() is 1 or 2 or 3 or 7 or 8 or 9))
                        fireballHelpPending = true;
                    if (currentRow.Binding.Row == 15) FindRow(16).Priority = 6;
                    // sub_889F0 promotes the retained minimap rows when the
                    // objective-expiry context closes.
                    if (currentRow.Binding.Row == 79)
                        foreach (var row in rows)
                            if (row.Binding.Row is 2 or 81) row.Priority = 6;
                }
                currentRow = null;
                objectiveHelpSubject = 0;
                pollDelay = spellMode ? 6 : 20;
            }

            internal void RequestObjectiveContext(int row, bool enabled)
            {
                if (!enabled) return;
                // A source kind outside the mapped cases leaves word_0x88
                // unchanged, but still closes the popup with an eight-pass delay.
                if (row != 0)
                {
                    if (row < 72 || row > 80 || rowFor(row) == null)
                        throw new InvalidDataException("Unbound source objective help row");
                    pendingRow = row;
                }
                if (remaining != 0) Close();
                Visible = false;
                VisibleRow = 0;
                pollDelay = 8;
            }

            internal void CloseCurrent(bool enabled)
            {
                // sub_87C10 resets the currently selected row, not row 41
                // when no help popup is selected, but always delays polling.
                if (!enabled) return;
                if (remaining != 0) Close();
                else pollDelay = 20;
                Visible = false;
                VisibleRow = 0;
            }
        }

        private ManaHelpHint gameRuntimeManaHint;
        private Color32[] gameRuntimeSpellPriorPixels;
        private ulong gameRuntimeSpellPriorGeneration;
        private SourceObjectiveTextboxPresenter gameRuntimeManaHintPresenter;
        private SourceObjectiveTextboxPresenter gameRuntimeHudPresenter;
        private Color32[] gameRuntimeHudPixels, gameRuntimeHudScratch;
        private readonly System.Collections.Generic.Dictionary<Texture2D, Color32[]> gameRuntimeHudTierPixels = new();
        private Texture gameRuntimeHudIndices;
        private ulong gameRuntimeHudGeneration, gameRuntimeHudRevision;
        private SourcePlayerNotificationsPresenter.Row[] gameRuntimeMapNoticeRows;

        // DrawTopStatusBar passes a wrapped signed source shift through an
        // int16 argument before DrawBar clamps it to 64 and rejects widths <2.
        private static int SourceTopStatusBarWidth(int value, int denominator)
        {
            if (denominator <= 0) throw new InvalidDataException("source top-status denominator is invalid");
            var width = unchecked((short)(unchecked(value << 6) / denominator));
            return width < 2 ? 0 : Math.Min((int)width, 64);
        }

        private bool TryGetGameRuntimeHudScene(CampaignPresentationSurface surface,
            out Texture indices, out ulong generation, out ulong revision)
        {
            indices = null; generation = revision = 0;
            if (surface == null || campaignSession == null || surface.Generation != campaignSession.Generation ||
                surface.Tick != campaignSession.Tick) return false;
            if (gameRuntimeHudIndices != null && gameRuntimeHudGeneration == campaignSession.Generation)
            {
                indices = gameRuntimeHudIndices; generation = gameRuntimeHudGeneration; revision = gameRuntimeHudRevision;
                return true;
            }
            indices = surface.HudUnderlayIndices;
            generation = surface.HudUnderlayGeneration; revision = surface.HudUnderlayRevision;
            return indices != null && generation == campaignSession.Generation;
        }

        // DrawGameFrame draws the minimap first, then opaque/transparent HUD
        // panel backgrounds, opaque icons and source-colour glyphs/bars. Keep
        // the resulting indices for the objective/help consumers that follow.
        private void DrawGameRuntimeIndexedHud(NativeSim.CampaignDayHudSnapshot hud, bool mapHandsOnly = false)
        {
            if (Event.current == null || Event.current.type != EventType.Repaint || campaignSession == null) return;
            var flashes = gameRuntimeCampaignSnapshot?.value?.player_hand_selection_flash_ticks;
            if (mapHandsOnly && (flashes == null || flashes.Length != 2 || flashes[0] > 8 || flashes[1] > 8))
                throw new InvalidDataException("source map-hand flash counters are invalid");
            if (mapHandsOnly && flashes[0] == 0 && flashes[1] == 0) return;
            var surface = gameRuntimeSecretSurface ?? level3PresentationSurface ??
                level2DayPresentationSurface ?? gameRuntimeNightSurface;
            if (surface == null || surface.Generation != campaignSession.Generation || surface.Tick != campaignSession.Tick) return;
            var underlay = surface.HudUnderlayIndices;
            if (underlay == null || surface.HudUnderlayGeneration != campaignSession.Generation) return;
            EnsureGameRuntimeMenu();
            var profile = campaignSession.Selection.mapType switch {
                0 => "day", 1 => "night", 2 => "cave",
                _ => throw new InvalidDataException("source HUD requires its source environment")
            };
            var assets = gameRuntimeMenu.ObjectiveForProfile(profile);
            var opaque = session.ObservedCampaignIdentity().display_settings.panels_opaque;
            var (castleX, balloonX, playerX) = ComputeLevel2DayHudTopBarLayout(hud);
            var maxY = 48;
            foreach (var selector in new[] { hud.minimapPanelSelector, hud.castlePanelSelector,
                         hud.balloonPanelSelector, hud.playerPanelSelector, hud.playerIconSelector, hud.lifeManaIconSelector })
                if (hudBitmapDescriptors.TryGetValue(selector, out var row) && row != null)
                    maxY = Math.Max(maxY, checked(2 + row.runtime_height));
            foreach (var slot in new[] { hud.left, hud.right })
                if (slot.holderEntityIndex != 0 && slot.drawAdmitted != 0)
                {
                    foreach (var selector in new[] { slot.frameSelector, slot.iconSelector })
                        if (hudBitmapDescriptors.TryGetValue(selector, out var row) && row != null)
                            maxY = Math.Max(maxY, checked(2 + row.runtime_height));
                    if (level2DayHudTierRows.TryGetValue(slot.levelTextSelector, out var tier) && tier != null)
                        maxY = Math.Max(maxY, checked(2 + tier.runtime_height));
                }
            if (maxY > 480) throw new InvalidDataException("source HUD exceeds the indexed viewport");
            var length = checked(640 * maxY);
            if (gameRuntimeHudScratch == null || gameRuntimeHudScratch.Length != length)
                gameRuntimeHudScratch = new Color32[length];
            else Array.Clear(gameRuntimeHudScratch, 0, length);
            void Pixel(int x, int y, byte index, bool blend)
            {
                if (x < 0 || x >= 640 || y < 0 || y >= maxY) return;
                var offset = y * 640 + x;
                var old = gameRuntimeHudScratch[offset];
                if (!blend) { gameRuntimeHudScratch[offset] = new Color32(index, 0, 0, 5); return; }
                if ((old.a & 4) != 0)
                {
                    gameRuntimeHudScratch[offset] = new Color32(assets.Tables[assets.TableOffset + (old.r << 8) + index], 0, 0, 5);
                    return;
                }
                var count = old.a & 3;
                if (count == 3) throw new InvalidDataException("source HUD exceeds indexed blend depth");
                if (count == 0) old.r = index; else if (count == 1) old.g = index; else old.b = index;
                old.a = (byte)(count + 1);
                gameRuntimeHudScratch[offset] = old;
            }
            void Sprite(int selector, int x, int y, bool blend)
            {
                if (!hudBitmapDescriptors.TryGetValue(selector, out var row) || row == null ||
                    !hudBitmapIndices.TryGetValue(selector, out var indices) || indices == null ||
                    !hudBitmapMasks.TryGetValue(selector, out var mask) || mask == null ||
                    row.stored_width <= 0 || row.stored_height <= 0 || row.runtime_width <= 0 || row.runtime_height <= 0 ||
                    indices.Length != checked(row.stored_width * row.stored_height) || mask.Length != indices.Length)
                    throw new InvalidDataException("source indexed HUD sprite is missing or malformed");
                for (var dy = 0; dy < row.runtime_height; dy++)
                for (var dx = 0; dx < row.runtime_width; dx++)
                {
                    var source = dy * row.stored_height / row.runtime_height * row.stored_width +
                        dx * row.stored_width / row.runtime_width;
                    if (mask[source] != 0) Pixel(x + dx, y + dy, indices[source], blend);
                }
            }
            void Fill(int x, int y, int width, int height, byte color)
            {
                for (var dy = 0; dy < height; dy++)
                for (var dx = 0; dx < width; dx++) Pixel(x + dx, y + dy, color, false);
            }
            void Hand(NativeSim.CampaignDayHudSpellSlot slot, int x)
            {
                if (slot.holderEntityIndex == 0 || slot.drawAdmitted == 0) return;
                Sprite(slot.frameSelector, x, 2, !opaque);
                Sprite(slot.iconSelector, x, 2, false);
                var frame = hudBitmapDescriptors[slot.frameSelector];
                var tier = level2DayHudTierRows[slot.levelTextSelector];
                var texture = ownerSpellTierTextures[slot.levelTextSelector];
                if (!gameRuntimeHudTierPixels.TryGetValue(texture, out var tierPixels))
                {
                    tierPixels = texture.GetPixels32();
                    gameRuntimeHudTierPixels.Add(texture, tierPixels);
                }
                var tierX = x + frame.runtime_width - tier.runtime_width - 2;
                for (var dy = 0; dy < tier.runtime_height; dy++)
                for (var dx = 0; dx < tier.runtime_width; dx++)
                {
                    var sx = dx * texture.width / tier.runtime_width;
                    var sy = dy * texture.height / tier.runtime_height;
                    if (tierPixels[(texture.height - 1 - sy) * texture.width + sx].a != 0)
                        Pixel(tierX + dx, 2 + dy, assets.BlendSelector, false);
                }
                if (slot.maxMana <= 0) return;
                Fill(x + 4, 38, (int)(56L * (slot.casterMana % slot.maxMana) / slot.maxMana), 4, hud.manaMaxPaletteIndex);
                var charges = slot.casterMana / slot.maxMana;
                for (var column = 0; column < 27 && charges > 0; column++)
                for (var row = 0; row < 2 && charges > 0; row++, charges--)
                    Fill(x + 2 * (column + 2), 38 + 2 * row, 2, 2, hud.manaPaletteIndex);
            }
            if (mapHandsOnly)
            {
                bool InsideHand(NativeSim.CampaignDayHudSpellSlot slot, int x, int px, int py, byte ticks)
                {
                    if (ticks == 0 || slot.holderEntityIndex == 0 || slot.drawAdmitted == 0 ||
                        !hudBitmapDescriptors.TryGetValue(slot.frameSelector, out var frame) || frame == null) return false;
                    return px >= x && px < x + frame.runtime_width && py >= 2 && py < 2 + frame.runtime_height;
                }
                if (gameRuntimeMapNoticeRows != null)
                {
                    var level = session.ObservedCampaignIdentity().active_runtime_flow.bound_level_id;
                    var pausedColor = gameRuntimeMenu.GameplayPausedColorIndex(level);
                    var lineHeight = gameRuntimeMenu.GameplayText(level).LineAdvance;
                    foreach (var textRow in gameRuntimeMapNoticeRows)
                    {
                        var x = textRow.X; var y = textRow.Y;
                        var color = textRow.PausedLabel ? pausedColor : assets.TextColor;
                        foreach (var value in textRow.Text)
                        {
                            if (value == 0 || x >= 640) break;
                            if (value == 13) continue;
                            if (value == '\n') { x = textRow.X; y += lineHeight; continue; }
                            if (value > byte.MaxValue) throw new InvalidDataException("source notice is not byte text");
                            var glyph = assets.Glyphs[value + 1];
                            if (value != 9 && value != 32)
                                for (var gy = 0; gy < glyph.Height; gy++)
                                for (var gx = 0; gx < glyph.Width; gx++)
                                {
                                    var px = x + gx; var py = y + gy;
                                    if (assets.GlyphMasks[glyph.Offset + gy * glyph.Width + gx] != 0 &&
                                        (InsideHand(hud.left, 510, px, py, flashes[0]) ||
                                         InsideHand(hud.right, 574, px, py, flashes[1])))
                                        Pixel(px, py, color, false);
                                }
                            x += value == 9 || value == 32 ? assets.Glyphs[33].Width : glyph.Width;
                        }
                    }
                }
                if (flashes[0] != 0) Hand(hud.left, 510);
                if (flashes[1] != 0) Hand(hud.right, 574);
            }
            else
            {
                Hand(hud.left, 510); Hand(hud.right, 574);
                Sprite(hud.minimapPanelSelector, 2, 2, !opaque);
                var status = gameRuntimeCampaignSnapshot?.value?.source_top_status ??
                    throw new InvalidDataException("source Castle and balloon status is unbound");
                if (status.castle_rows == null || status.castle_rows.Length > 1 ||
                    (status.castle_rows.Length == 1 && status.castle_rows[0] == null))
                    throw new InvalidDataException("source Castle status rows are invalid");
                var castle = status.castle_rows.Length == 0 ? null : status.castle_rows[0];
                Sprite(castle?.panel_selector ?? 54, castleX, 2, !opaque);
                if (castle != null && castle.stage != 0)
                {
                    Sprite(castle.icon_selector, castleX + 2, 2, false);
                    Sprite(hud.lifeManaIconSelector, castleX + 38, 2, false);
                    var barX = castleX + 58;
                    Fill(barX, 10, SourceTopStatusBarWidth(castle.life, castle.max_life), 10, hud.lifePaletteIndex);
                    var totalMana = unchecked(castle.mana + castle.owner_mana_aggregate);
                    if (totalMana != castle.max_mana)
                        Fill(barX, 28, SourceTopStatusBarWidth(castle.max_mana, hud.globalManaDenominator), 10,
                            hud.manaMaxPaletteIndex);
                    var fullColor = totalMana == castle.max_mana && hud.colorPhase2 == 1;
                    Fill(barX, 28, SourceTopStatusBarWidth(fullColor ? castle.max_mana : totalMana,
                        hud.globalManaDenominator), 10, fullColor ? hud.manaMaxPaletteIndex : hud.manaPaletteIndex);
                    if (status.mana_goal_percent != 0)
                    {
                        var goalX = barX + (status.mana_goal_percent << 6) / 100;
                        Fill(goalX, 26, 2, 2, status.goal_palette_index);
                        Fill(goalX, 38, 2, 2, status.goal_palette_index);
                    }
                }
                Sprite(status.balloon_panel_selector, balloonX, 2, !opaque);
                if (castle != null)
                {
                    Sprite(status.balloon_icon_selector, balloonX + 2, 2, false);
                    Sprite(hud.lifeManaIconSelector, balloonX + 38, 2, false);
                    var count = castle.stage <= 3 ? (castle.stage == 0 ? 0 : 1) : castle.stage <= 5 ? 2 : 3;
                    if (status.balloons == null || status.balloons.Length != 3)
                        throw new InvalidDataException("source balloon rows are incomplete");
                    for (var index = 0; index < count; index++)
                    {
                        var balloon = status.balloons[index];
                        if (balloon == null) throw new InvalidDataException("source balloon row is unbound");
                        if (!balloon.present) continue;
                        if (balloon.life >= 0)
                            Fill(balloonX + 58, 12 + 2 * index,
                                SourceTopStatusBarWidth(balloon.life, balloon.max_life), 2, hud.lifePaletteIndex);
                        Fill(balloonX + 58, 30 + 2 * index,
                            SourceTopStatusBarWidth(balloon.mana, balloon.max_mana), 2, hud.manaPaletteIndex);
                    }
                }
                Sprite(hud.playerPanelSelector, playerX, 2, !opaque);
                Sprite(hud.playerIconSelector, playerX + 2, 2, false);
                Sprite(hud.lifeManaIconSelector, playerX + 38, 2, false);
                Fill(playerX + 58, 10, SourceHudWidth(hud.localPlayerLife, hud.localPlayerMaxLife), 10, hud.lifePaletteIndex);
                Fill(playerX + 58, 28, SourceHudWidth(hud.localPlayerMaxMana, hud.globalManaDenominator), 10, hud.manaMaxPaletteIndex);
                Fill(playerX + 58, 28, SourceHudWidth(hud.localPlayerMana, hud.globalManaDenominator), 10, hud.manaPaletteIndex);
            }
            gameRuntimeHudPresenter ??= new SourceObjectiveTextboxPresenter(assets);
            var previousPixels = gameRuntimeHudPixels;
            var changed = previousPixels == null || previousPixels.Length != length;
            if (previousPixels != null && !changed)
                for (var i = 0; !changed && i < length; i++) changed = !previousPixels[i].Equals(gameRuntimeHudScratch[i]);
            if (changed || gameRuntimeHudGeneration != campaignSession.Generation)
            {
                gameRuntimeHudPresenter.SetSpellPrior(new RectInt(0, 0, 640, maxY), gameRuntimeHudScratch, campaignSession.Generation);
                (gameRuntimeHudPixels, gameRuntimeHudScratch) = (gameRuntimeHudScratch, gameRuntimeHudPixels);
            }
            gameRuntimeHudIndices = gameRuntimeHudPresenter.RenderPriorIndices(underlay, campaignSession.Generation);
            gameRuntimeHudGeneration = surface.HudUnderlayGeneration;
            gameRuntimeHudRevision = surface.HudUnderlayRevision;
            var palette = surface.FramePaletteRgb8;
            var scale = Mathf.Min(Screen.width / 640f, Screen.height / 480f);
            gameRuntimeHudPresenter.DrawPausePrior(underlay, campaignSession.Generation,
                new Rect((Screen.width - 640f * scale) * .5f, (Screen.height - 480f * scale) * .5f,
                    640f * scale, 480f * scale), palette);
        }

        private void InitializeGameRuntimeHelpHint(OwnerCampaignMenuCache languageCache)
        {
            if (languageCache == null)
                throw new ArgumentNullException(nameof(languageCache));
            if (string.IsNullOrEmpty(languageCache.LanguageText(329)))
                throw new InvalidDataException("Missing source language record 329");
            gameRuntimeAbilityNotification = new AbilityImprovedNotification(languageCache.LanguageText);
            gameRuntimeManaHint = new ManaHelpHint(languageCache.GameplayHelpForRow,
                languageCache.HasMinimapHelp, languageCache.HasObjectiveMarkHelp, languageCache.HasLargeMapHelp,
                languageCache.HasLowLifeHelp, languageCache.HasIdleHelp, languageCache.HasVitalsHelp,
                languageCache.HasCastHelp, languageCache.HasSpellSearchHelp, languageCache.HasCastleSpellHelp,
                languageCache.HasFireballHelp, ReadCurrentHelpObjectiveStageKind);
        }

        private sbyte ReadCurrentHelpObjectiveStageKind()
        {
            // sub_889F0 reads the current objective on closure, which can
            // differ from the context row's original source stage.
            var colour = campaignSession.DayHudState(campaignSession.Tick).localPlayerColorIndex;
            var objective = campaignSession.ObjectiveStageState(colour, 0);
            return campaignSession.ObjectiveStageState(colour, objective.textSelector).stageKind;
        }

        private void ConsumeGameRuntimeHelpHint(NativeSim.NativeEvent effect)
        {
            Require(effect.kind == NativeSim.CampaignEffectFlight && effect.code == 5,
                "source campaign help event");
            Require(gameRuntimeManaHint != null, "materialized campaign help text");
            gameRuntimeManaHint.Promote(effect.value0, effect.value1);
        }

        private void CloseGameRuntimeHelpHint(bool showHelp)
        {
            gameRuntimeManaHint?.CloseCurrent(showHelp);
        }

        private void ApplyGameRuntimeHelpMode(bool enabled)
        {
            Require(gameRuntimeManaHint != null, "materialized campaign help text");
            gameRuntimeManaHint.ApplyHelpMode(enabled);
        }

        private void RequestGameRuntimeContextHelpHint(int row, bool showHelp)
        {
            gameRuntimeManaHint?.RequestObjectiveContext(row, showHelp);
        }

        private void AdvanceGameRuntimeHelpHint(NativeSim.CampaignDayHudSnapshot hud,
            bool showHelp, bool paused, bool suppressed, sbyte wallContactCounter, bool levelEnd,
            bool minimapEnabled)
        {
            var hint = gameRuntimeManaHint;
            if (hint == null) return;
            IReadOnlyList<(byte Row, ushort Subject, int X, int Y)> marks = null;
            ushort nearest = 0;
            bool? hasObjectiveHelpSubject = null;
            if (hint.HasObjectiveMarkHelp && showHelp && !paused && !suppressed && minimapEnabled)
            {
                var surface = gameRuntimeSecretSurface ?? level3PresentationSurface ??
                    level2DayPresentationSurface ?? gameRuntimeNightSurface;
                if (surface != null && surface.Generation == campaignSession.Generation &&
                    surface.Tick == campaignSession.Tick)
                {
                    marks = surface.ObjectiveHelpMarks;
                    if (hint.PollsObjectiveMarkHelp)
                    {
                        nearest = NearestObjectiveHelpSubject(marks);
                        if (hint.HasLargeMapHelp && surface.LiveMinimapReady)
                            hasObjectiveHelpSubject = surface.HasObjectiveHelpSubject;
                    }
                }
            }
            var observation = gameRuntimeCampaignSnapshot.value;
            var vitals = observation.player_vitals;
            var lowLife = vitals != null && vitals.life <= (vitals.max_life >> 2);
            // Source v29 gates the signed assignments separately from cast readiness.
            var hasSpell = observation.spell_menu_has_any_spell;
            // "not_acquired" is the empty family-2 source slot. An invalid
            // nonempty holder does not trigger the Castle search reminder.
            var castleSpell = hasSpell ? Array.Find(observation.player_spells, spell => spell.id == "spell.castle") : null;
            var missingCastle = castleSpell?.availability == "not_acquired";
            var hasCastleSpellWithoutCastle = castleSpell != null && !missingCastle &&
                observation.source_top_status?.castle_rows?.Length == 0;
            int? castleHandAnchorX = hud.left.rawSpellSelector == 2 ? 540 :
                hud.right.rawSpellSelector == 2 ? 600 : null;
            var fireballSpell = hasSpell && hint.HasFireballHelp
                ? Array.Find(observation.player_spells, spell => spell.id == "spell.fireball") : null;
            bool? fireballSpellEnabled = fireballSpell == null ? null : fireballSpell.availability != "not_acquired";
            int? fireballHandAnchorX = hud.left.rawSpellSelector == 0 ? 540 :
                hud.right.rawSpellSelector == 0 ? 600 : null;
            hint.Advance(hud.left.helpUnavailable != 0, hud.right.helpUnavailable != 0, showHelp, paused, suppressed,
                wallContactCounter, levelEnd, minimapEnabled, gameRuntimeSourceMenus.MapOpen,
                marks, nearest, hasObjectiveHelpSubject, lowLife,
                hasSpell && hud.left.rawSpellSelector >= 0, hasSpell && hud.right.rawSpellSelector >= 0,
                !hasSpell, missingCastle, hasCastleSpellWithoutCastle, castleHandAnchorX,
                fireballSpellEnabled, fireballHandAnchorX);
        }

        private ushort NearestObjectiveHelpSubject(
            IReadOnlyList<(byte Row, ushort Subject, int X, int Y)> marks)
        {
            ushort nearest = 0;
            var distance = int.MaxValue;
            NativeSim.CampaignEntitySnapshot[] entities = null;
            var player = gameRuntimeCampaignSnapshot.value.player_pose;
            foreach (var mark in marks)
            {
                if (mark.Row != 82 || mark.Subject == 0) continue;
                entities ??= campaignSession.Entities();
                var entity = entities[mark.Subject];
                var dx = unchecked((short)(entity.x - player.x));
                var dy = unchecked((short)(entity.y - player.y));
                var current = (int)Math.Sqrt((long)dx * dx + (long)dy * dy);
                if (current >= distance) continue;
                distance = current;
                nearest = mark.Subject;
            }
            return nearest;
        }

        private void DrawGameRuntimeHelpHint()
        {
            if (gameRuntimeManaHint?.Visible != true) return;
            if (Event.current == null || Event.current.type != EventType.Repaint || campaignSession == null) return;
            var identity = session?.ObservedCampaignIdentity();
            var snapshot = gameRuntimeCampaignSnapshot?.value;
            var spellHelp = gameRuntimeSpellMenuOpen && gameRuntimeManaHint.SpellVisible;
            if (spellHelp && gameRuntimeSourceSpellMenu.HoveredSlot < 0) return;
            if ((identity?.mode != GameRuntimeTransport.PlayingMode &&
                 !(spellHelp && identity?.mode == GameRuntimeTransport.PausedMode)) ||
                identity.player_preferences?.preferences?.show_help != true || snapshot == null ||
                snapshot.player_vitals.life < 0 || snapshot.objective_textbox_ticks != 0 || snapshot.exit_sequence_active ||
                nativePresentationTutorialOrBriefing || (gameRuntimeSpellMenuOpen && !spellHelp) || gameRuntimeAbandonConfirmationOpen ||
                playerPreferences?.IsOpen == true) return;
            var surface = gameRuntimeSecretSurface ?? level3PresentationSurface ??
                level2DayPresentationSurface ?? gameRuntimeNightSurface;
            if (!TryGetGameRuntimeHudScene(surface, out var indices, out var generation, out var revision)) return;
            EnsureGameRuntimeMenu();
            var profile = campaignSession.Selection.mapType switch {
                0 => "day", 1 => "night", 2 => "cave",
                _ => throw new InvalidDataException("source hint requires its source environment")
            };
            gameRuntimeManaHintPresenter ??= new SourceObjectiveTextboxPresenter(gameRuntimeMenu.ObjectiveForProfile(profile));
            var helpRow = gameRuntimeMenu.GameplayHelpForRow(gameRuntimeManaHint.VisibleRow);
            if (spellHelp)
            {
                var ids = gameRuntimeManaHint.VisibleSpellRecords;
                if (ids == null || (ids.Length != 1 && ids.Length != 3))
                    throw new InvalidDataException("source spell help has no selected records");
                var text = new string[ids.Length];
                for (var index = 0; index < ids.Length; index++)
                {
                    text[index] = gameRuntimeMenu.GameplayLanguageText(ids[index]);
                    if (string.IsNullOrEmpty(text[index])) throw new InvalidDataException("source spell help record is empty");
                }
                gameRuntimeManaHintPresenter.PresentSpellHint(text, gameRuntimeManaHint.VisibleSpellHighlight,
                    campaignSession.Generation, gameRuntimeManaHint.AnchorX, gameRuntimeManaHint.AnchorY);
            }
            else
            {
                gameRuntimeManaHintPresenter.ClearPrior();
                gameRuntimeSpellPriorPixels = null;
                gameRuntimeSpellPriorGeneration = 0;
                var helpText = gameRuntimeMenu.GameplayLanguageText(helpRow.TextRecord);
                if (helpRow.Row == 82)
                    gameRuntimeManaHintPresenter.PresentObjectiveMarkHint(helpText, campaignSession.Generation,
                        gameRuntimeManaHint.AnchorX, gameRuntimeManaHint.AnchorY, gameRuntimeSourceMenus.MapOpen);
                else if (helpRow.Row is 2 or 5 or 36 or 37 or 38 or 39 or 41 ||
                    (helpRow.Row is 10 or 12 && gameRuntimeManaHint.AnchorY == 32))
                    gameRuntimeManaHintPresenter.PresentHint(helpText, campaignSession.Generation,
                        gameRuntimeManaHint.AnchorX, gameRuntimeManaHint.AnchorY);
                else
                    gameRuntimeManaHintPresenter.PresentContextHint(helpText, campaignSession.Generation, helpRow.X, helpRow.Y);
            }
            var palette = surface.FramePaletteRgb8;
            var scale = Mathf.Min(Screen.width / 640f, Screen.height / 480f);
            gameRuntimeManaHintPresenter.Draw(1, indices, generation, revision,
                new Rect((Screen.width - 640f * scale) * .5f, (Screen.height - 480f * scale) * .5f,
                    640f * scale, 480f * scale), palette);
        }

        private void DrawGameRuntimeSpellMenuPrior(OwnerCampaignMenuCache.ObjectiveAssets assets)
        {
            if (Event.current == null || Event.current.type != EventType.Repaint || campaignSession == null) return;
            var surface = gameRuntimeSecretSurface ?? level3PresentationSurface ??
                level2DayPresentationSurface ?? gameRuntimeNightSurface;
            if (!TryGetGameRuntimeHudScene(surface, out var scene, out var generation, out _)) return;
            var cache = gameRuntimeEnvironmentCache ?? ownerAssetCache;
            ObjectiveTextboxCompositor.Sprite Indexed(int selector)
            {
                if (!hudBitmapDescriptors.TryGetValue(selector, out var row) || row == null ||
                    !hudBitmapIndices.TryGetValue(selector, out var indices) || indices == null ||
                    !hudBitmapMasks.TryGetValue(selector, out var mask) || mask == null) return null;
                return new ObjectiveTextboxCompositor.Sprite(row.stored_width, row.stored_height, indices, mask);
            }
            if (!gameRuntimeSourceSpellMenu.TryIndexedPrior(cache, Indexed, assets.Tables, assets.TableOffset,
                out var rect, out var pixels)) return;
            gameRuntimeManaHintPresenter ??= new SourceObjectiveTextboxPresenter(assets);
            if (!ReferenceEquals(pixels, gameRuntimeSpellPriorPixels) ||
                gameRuntimeSpellPriorGeneration != campaignSession.Generation)
            {
                gameRuntimeManaHintPresenter.SetSpellPrior(rect, pixels, campaignSession.Generation);
                gameRuntimeSpellPriorPixels = pixels;
                gameRuntimeSpellPriorGeneration = campaignSession.Generation;
            }
            var palette = surface.FramePaletteRgb8;
            var scale = Mathf.Min(Screen.width / 640f, Screen.height / 480f);
            gameRuntimeManaHintPresenter.DrawPausePrior(scene, generation,
                new Rect((Screen.width - 640f * scale) * .5f, (Screen.height - 480f * scale) * .5f,
                    640f * scale, 480f * scale), palette);
            gameRuntimeSourceSpellMenu.DrawCursor();
        }

        private void DrawGameRuntimeAbilityNotification()
        {
            if (gameRuntimeAbilityNotification?.Visible != true) return;
            var scale = Mathf.Max(1f, Screen.width / 640f);
            GUI.Label(new Rect(132 * scale, 50 * scale, 500 * scale, 30 * scale),
                gameRuntimeAbilityNotification.Text, hudStyle);
        }

        private void ResetGameRuntimeHelpHint()
        {
            gameRuntimeHudPresenter?.Dispose();
            gameRuntimeHudPresenter = null;
            gameRuntimeHudPixels = gameRuntimeHudScratch = null;
            gameRuntimeHudTierPixels.Clear();
            gameRuntimeHudIndices = null;
            gameRuntimeHudGeneration = gameRuntimeHudRevision = 0;
            gameRuntimeSpellPriorPixels = null;
            gameRuntimeSpellPriorGeneration = 0;
            gameRuntimeAbilityNotification = null;
            gameRuntimeManaHint = null;
            gameRuntimeManaHintPresenter?.Dispose();
            gameRuntimeManaHintPresenter = null;
        }

        private sealed class CampaignHudPresenter
        {
            private readonly MagicCarpetRuntime owner;

            internal CampaignHudPresenter(MagicCarpetRuntime owner)
            {
                this.owner = owner;
            }

            internal void Present(NativeSim.CampaignDayHudSnapshot hud)
            {
                owner.RecordLevel2Presentation("hud");
                MagicCarpetRuntime.Require(
                    OwnerAssetCache.IsQualifiedDayHudSpriteSelector(
                        hud.minimapPanelSelector) &&
                    OwnerAssetCache.IsQualifiedDayHudSpriteSelector(
                        hud.castlePanelSelector) &&
                    OwnerAssetCache.IsQualifiedDayHudSpriteSelector(
                        hud.balloonPanelSelector) &&
                    OwnerAssetCache.IsQualifiedDayHudSpriteSelector(
                        hud.playerPanelSelector) &&
                    OwnerAssetCache.IsQualifiedDayHudSpriteSelector(
                        hud.playerIconSelector) &&
                    OwnerAssetCache.IsQualifiedDayHudSpriteSelector(
                        hud.lifeManaIconSelector),
                    "qualified Day HUD status roots");
                MagicCarpetRuntime.ValidateLevel2DayHudPresenterSlot(
                    hud.left,
                    hud.localPlayerEntityIndex);
                MagicCarpetRuntime.ValidateLevel2DayHudPresenterSlot(
                    hud.right,
                    hud.localPlayerEntityIndex);

                owner.level2PresentedDayHud = hud;
                owner.level2DayHudPresenterReady = false;
                if (owner.HasMaterializedLevel2DayHudRoots(hud))
                {
                    owner.ComputeLevel2DayHudTopBarLayout(hud);
                    owner.level2DayHudPresenterReady = true;
                }
            }
        }

        private void PresentLevel2DayHud(NativeSim.CampaignDayHudSnapshot hud)
        {
            level2DayHud = hud;
            Level2Hud.Present(hud);
        }
    }
}
