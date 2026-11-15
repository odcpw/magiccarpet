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
            internal int Priority => mana.Priority;
            internal bool Visible { get; private set; }
            internal int VisibleRow { get; private set; }
            internal int AnchorX { get; private set; } = 540;

            internal ManaHelpHint(Func<int, OwnerCampaignMenuCache.GameplayHelpBinding> binding)
            {
                rowFor = binding ?? throw new ArgumentNullException(nameof(binding));
                // Keep source row order: equal overdue ages retain the first
                // eligible row in GetHelpPopupIndex_88450.
                rows = new RowState[12];
                rows[0] = new RowState(rowFor(1));
                rows[1] = new RowState(rowFor(3));
                rows[2] = mana = new RowState(rowFor(41));
                for (var index = 3; index < rows.Length; index++)
                    rows[index] = new RowState(rowFor(72 + index - 3));
            }

            internal void Promote(int hint, int priority)
            {
                if (hint != 41 || priority != 6)
                    throw new InvalidDataException("Unbound campaign help hint");
                mana.Priority = priority;
            }

            internal void Advance(bool leftUnavailable, bool rightUnavailable,
                bool enabled, bool paused, bool suppressed, sbyte wallContactCounter = 0,
                bool levelEnd = false)
            {
                Visible = false;
                VisibleRow = 0;
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
                pollDelay = 20;
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
            bool showHelp, bool paused, bool suppressed, sbyte wallContactCounter, bool levelEnd)
        {
            gameRuntimeManaHint?.Advance(leftUnavailable, rightUnavailable, showHelp, paused, suppressed,
                wallContactCounter, levelEnd);
        }

        private void DrawGameRuntimeHelpHint()
        {
            if (gameRuntimeManaHint?.Visible != true) return;
            if (Event.current == null || Event.current.type != EventType.Repaint || campaignSession == null) return;
            var identity = session?.ObservedCampaignIdentity();
            var snapshot = gameRuntimeCampaignSnapshot?.value;
            if (identity?.mode != GameRuntimeTransport.PlayingMode ||
                identity.player_preferences?.preferences?.show_help != true || snapshot == null ||
                snapshot.player_vitals.life < 0 || snapshot.objective_textbox_ticks != 0 || snapshot.exit_sequence_active ||
                nativePresentationTutorialOrBriefing || gameRuntimeSpellMenuOpen || gameRuntimeAbandonConfirmationOpen ||
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
            var helpText = gameRuntimeMenu.GameplayLanguageText(helpRow.TextRecord);
            if (helpRow.Row == 41)
                gameRuntimeManaHintPresenter.PresentHint(helpText, campaignSession.Generation, gameRuntimeManaHint.AnchorX);
            else
                gameRuntimeManaHintPresenter.PresentContextHint(helpText, campaignSession.Generation, helpRow.X, helpRow.Y);
            var palette = surface.FramePaletteRgb8;
            if (playerPreferences != null)
                palette = playerPreferences.ApplyBrightnessToPalette(palette, campaignSession.Selection.mapType);
            var scale = Mathf.Min(Screen.width / 640f, Screen.height / 480f);
            gameRuntimeManaHintPresenter.Draw(1, indices, generation, revision,
                new Rect((Screen.width - 640f * scale) * .5f, (Screen.height - 480f * scale) * .5f,
                    640f * scale, 480f * scale), palette, gameRuntimeSourceMenus.MapOpen);
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
