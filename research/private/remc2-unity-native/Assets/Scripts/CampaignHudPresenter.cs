using System;
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
                    Age = binding.AgeThreshold;
                    // sub_88B20 initially promotes source type7 context rows.
                    Priority = binding.Row >= 72 && binding.Row <= 80 ? 6 : binding.Priority;
                }
            }

            private readonly Func<int, OwnerCampaignMenuCache.GameplayHelpBinding> rowFor;
            private readonly RowState[] rows;
            private readonly RowState mana;
            // SetTextBoxMinMaxSizes_87090 clears dword_0x72; the 20-pass
            // delay is installed only by suppression or sub_87C10.
            private int pollDelay;
            private int remaining;
            private RowState currentRow;
            private int pendingRow;
            private bool spellMode;
            private int spellSlot = -1, spellTier = -1, spellCycle;
            private ushort[] spellRecords;
            private int spellHighlight = -1;
            internal int Priority => mana.Priority;
            internal bool Visible { get; private set; }
            internal int VisibleRow { get; private set; }
            internal int AnchorX { get; private set; } = 540;
            internal int AnchorY { get; private set; }
            internal bool SpellVisible { get; private set; }
            internal ushort[] VisibleSpellRecords { get; private set; }
            internal int VisibleSpellHighlight { get; private set; } = -1;

            internal ManaHelpHint(Func<int, OwnerCampaignMenuCache.GameplayHelpBinding> binding)
            {
                rowFor = binding ?? throw new ArgumentNullException(nameof(binding));
                // Keep source row order: equal overdue ages retain the first
                // eligible row in GetHelpPopupIndex_88450.
                var ids = new[] { 1, 3, 40, 41, 42, 69, 70, 71,
                    72, 73, 74, 75, 76, 77, 78, 79, 80, 85 };
                rows = new RowState[ids.Length];
                for (var index = 0; index < ids.Length; index++) rows[index] = new RowState(rowFor(ids[index]));
                mana = rows[3];
            }

            internal void Promote(int hint, int priority)
            {
                if (hint != 41 || priority != 6)
                    throw new InvalidDataException("Unbound campaign help hint");
                mana.Priority = priority;
            }

            internal void Advance(bool leftUnavailable, bool rightUnavailable,
                bool enabled, bool paused, bool suppressed, sbyte wallContactCounter = 0,
                bool levelEnd = false, bool minimapEnabled = true)
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
                    // Eligibility is sampled by SetHelpPopupCoords only when
                    // selecting a popup, not reevaluated during its 50 draws.
                    Visible = true;
                    VisibleRow = currentRow.Binding.Row;
                    if (--remaining == 0) Close();
                }
                else if (--pollDelay <= 0)
                {
                    if (pendingRow != 0)
                    {
                        currentRow = FindRow(pendingRow);
                        pendingRow = 0;
                    }
                    else
                    {
                        // Current eligibility is sampled only by the source
                        // poll. A contact event does not promote or select row3.
                        var bestOverdue = 0;
                        for (var priority = 6; priority >= 0 && currentRow == null; priority--)
                        {
                            foreach (var row in rows)
                            {
                                var eligible = row.Binding.Row == 1 || (row.Binding.Row == 3 ? wallContactCounter != 0 :
                                    row.Binding.Row == 41 && (leftUnavailable || rightUnavailable));
                                // GetHelpPopupTextAndCoords_87CF0 applies this
                                // mask after normal eligibility when IsLevelEnd
                                // is set. Forced word88 selection bypasses it.
                                if (levelEnd && (row.Binding.Flags & 0x4ff) != 0) eligible = false;
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
                        if (currentRow.Binding.Row == 41) AnchorX = leftUnavailable ? 540 : 600;
                        remaining = currentRow.Binding.Duration;
                    }
                    // SetHelpPopupTextAndCoords_884D0 selects this frame;
                    // DrawPopupTextBox_87610 starts on the following frame.
                }
                // sub_87B70 services every source row after selection/draw,
                // including rows that are not currently eligible.
                AgeRows();
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
                }
                currentRow = null;
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

        private void InitializeGameRuntimeHelpHint(OwnerCampaignMenuCache languageCache)
        {
            if (languageCache == null)
                throw new ArgumentNullException(nameof(languageCache));
            if (string.IsNullOrEmpty(languageCache.LanguageText(329)))
                throw new InvalidDataException("Missing source language record 329");
            gameRuntimeAbilityNotification = new AbilityImprovedNotification(languageCache.LanguageText);
            gameRuntimeManaHint = new ManaHelpHint(languageCache.GameplayHelpForRow);
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

        private void RequestGameRuntimeContextHelpHint(int row, bool showHelp)
        {
            gameRuntimeManaHint?.RequestObjectiveContext(row, showHelp);
        }

        private void AdvanceGameRuntimeHelpHint(bool leftUnavailable, bool rightUnavailable,
            bool showHelp, bool paused, bool suppressed, sbyte wallContactCounter, bool levelEnd,
            bool minimapEnabled)
        {
            gameRuntimeManaHint?.Advance(leftUnavailable, rightUnavailable, showHelp, paused, suppressed,
                wallContactCounter, levelEnd, minimapEnabled);
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
            if (surface == null || surface.Generation != campaignSession.Generation || surface.Tick != campaignSession.Tick) return;
            Texture indices;
            ulong generation, revision;
            if (gameRuntimeSourceMenus.MapOpen)
            {
                indices = surface.WorldIndices; generation = surface.Generation; revision = surface.Tick;
            }
            else
            {
                if (surface.SceneHistoryGeneration != campaignSession.Generation ||
                    surface.SceneHistoryRevision != campaignSession.SceneFrameRevision) return;
                indices = surface.ComposedSceneIndices; generation = surface.SceneHistoryGeneration;
                revision = surface.SceneHistoryRevision;
            }
            if (indices == null) return;
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
                if (helpRow.Row == 41)
                    gameRuntimeManaHintPresenter.PresentHint(helpText, campaignSession.Generation, gameRuntimeManaHint.AnchorX);
                else
                    gameRuntimeManaHintPresenter.PresentContextHint(helpText, campaignSession.Generation, helpRow.X, helpRow.Y);
            }
            var palette = surface.FramePaletteRgb8;
            if (playerPreferences != null)
                palette = playerPreferences.ApplyBrightnessToPalette(palette, campaignSession.Selection.mapType);
            var scale = Mathf.Min(Screen.width / 640f, Screen.height / 480f);
            gameRuntimeManaHintPresenter.Draw(1, indices, generation, revision,
                new Rect((Screen.width - 640f * scale) * .5f, (Screen.height - 480f * scale) * .5f,
                    640f * scale, 480f * scale), palette, gameRuntimeSourceMenus.MapOpen);
        }

        private void DrawGameRuntimeSpellMenuPrior(OwnerCampaignMenuCache.ObjectiveAssets assets)
        {
            if (Event.current == null || Event.current.type != EventType.Repaint || campaignSession == null) return;
            var surface = gameRuntimeSecretSurface ?? level3PresentationSurface ??
                level2DayPresentationSurface ?? gameRuntimeNightSurface;
            if (surface == null || surface.Generation != campaignSession.Generation || surface.Tick != campaignSession.Tick) return;
            Texture scene;
            ulong generation;
            var map = gameRuntimeSourceMenus.MapOpen;
            if (map) { scene = surface.WorldIndices; generation = surface.Generation; }
            else
            {
                if (surface.SceneHistoryGeneration != campaignSession.Generation ||
                    surface.SceneHistoryRevision != campaignSession.SceneFrameRevision) return;
                scene = surface.ComposedSceneIndices; generation = surface.SceneHistoryGeneration;
            }
            if (scene == null) return;
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
            if (playerPreferences != null)
                palette = playerPreferences.ApplyBrightnessToPalette(palette, campaignSession.Selection.mapType);
            var scale = Mathf.Min(Screen.width / 640f, Screen.height / 480f);
            gameRuntimeManaHintPresenter.DrawPausePrior(scene, generation,
                new Rect((Screen.width - 640f * scale) * .5f, (Screen.height - 480f * scale) * .5f,
                    640f * scale, 480f * scale), palette, map);
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
