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
            private readonly string[] records;
            private int remaining;
            internal string Text { get; private set; }
            internal bool Visible { get; private set; }

            internal AbilityImprovedNotification(byte[] language)
            {
                records = Encoding.GetEncoding(28591).GetString(language).Split('\0');
                if (records.Length <= 185 || string.IsNullOrEmpty(records[159]))
                    throw new InvalidDataException("Missing source ability notification language");
            }

            internal void Improve(int family)
            {
                if (family < 0 || family >= 26 || string.IsNullOrEmpty(records[160 + family]))
                    throw new InvalidDataException("Unbound source spell notification family");
                var format = records[159];
                var placeholder = format.IndexOf("%s", StringComparison.Ordinal);
                if (placeholder < 0 || format.IndexOf("%s", placeholder + 2, StringComparison.Ordinal) >= 0)
                    throw new InvalidDataException("Invalid source spell notification format");
                var text = format.Substring(0, placeholder) + records[160 + family] + format.Substring(placeholder + 2);
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
        // The only gameplay hint emitted by the opening spell activation path.
        // engine_support.cpp str_E2A74[41]: text 329, duration 50, age 800,
        // base priority 2. sub_88B60 raises priority to 6; it does not create a
        // seconds timer or bypass GetHelpPopupTextAndCoords affordability.
        internal sealed class ManaHelpHint
        {
            private const int Cooldown = 800;
            private int age = Cooldown;
            private int pollDelay = 20;
            private int remaining;
            internal int Priority { get; private set; } = 2;
            internal bool Visible { get; private set; }
            internal int AnchorX { get; private set; } = 540;

            internal void Promote(int hint, int priority)
            {
                if (hint != 41 || priority != 6)
                    throw new InvalidDataException("Unbound campaign help hint");
                Priority = priority;
            }

            internal void Advance(bool leftUnavailable, bool rightUnavailable,
                bool enabled, bool paused, bool suppressed)
            {
                Visible = false;
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
                    if (--remaining == 0) Close();
                }
                else if (--pollDelay <= 0 && age > Cooldown &&
                    (leftUnavailable || rightUnavailable))
                {
                    AnchorX = leftUnavailable ? 540 : 600;
                    remaining = 50;
                    // SetHelpPopupTextAndCoords_884D0 selects this frame;
                    // DrawPopupTextBox_87610 starts on the following frame.
                }
                age = unchecked(age + 1);
                if (age > Cooldown * 3)
                {
                    age = Cooldown;
                    Priority = Math.Min(5, Priority + 1);
                }
            }

            private void Close()
            {
                remaining = 0;
                age = 0;
                Priority = 2;
                pollDelay = 20;
            }
        }

        private ManaHelpHint gameRuntimeManaHint;
        private string gameRuntimeManaHintText;
        private GUIStyle gameRuntimeManaHintStyle;

        private void InitializeGameRuntimeHelpHint(OwnerAssetCache languageCache)
        {
            if (languageCache == null)
                throw new ArgumentNullException(nameof(languageCache));
            // This approved output is the source language record table shared
            // by every opening mission, not an encoded LANGUAGE_L1.TXT file.
            var records = languageCache.ReadOutput("font/language-l1.records-nul.bin");
            gameRuntimeAbilityNotification = new AbilityImprovedNotification(records);
            var record = 0;
            var start = 0;
            gameRuntimeManaHintText = null;
            for (var index = 0; index < records.Length; index++)
            {
                if (records[index] != 0) continue;
                if (record++ == 329)
                {
                    if (index == start)
                        throw new InvalidDataException("Empty source mana help text");
                    gameRuntimeManaHintText = Encoding.ASCII.GetString(records, start, index - start);
                    break;
                }
                start = index + 1;
            }
            if (gameRuntimeManaHintText == null)
                throw new InvalidDataException("Missing source language record 329");
            gameRuntimeManaHint = new ManaHelpHint();
        }

        private void ConsumeGameRuntimeHelpHint(NativeSim.NativeEvent effect)
        {
            Require(effect.kind == NativeSim.CampaignEffectFlight && effect.code == 5,
                "source campaign help event");
            Require(gameRuntimeManaHint != null, "materialized campaign help text");
            gameRuntimeManaHint.Promote(effect.value0, effect.value1);
        }

        private void AdvanceGameRuntimeHelpHint(NativeSim.CampaignDayHudSnapshot hud,
            bool showHelp, bool paused, bool suppressed)
        {
            gameRuntimeManaHint?.Advance(ManaHelpUnavailable(hud.left),
                ManaHelpUnavailable(hud.right), showHelp, paused,
                suppressed || hud.localPlayerLife < 0);
        }

        private static bool ManaHelpUnavailable(NativeSim.CampaignDayHudSpellSlot slot)
        {
            return slot.holderEntityIndex != 0 && slot.maxMana > 0 &&
                slot.casterMana < slot.maxMana;
        }

        private void DrawGameRuntimeHelpHint()
        {
            if (gameRuntimeManaHint?.Visible != true) return;
            gameRuntimeManaHintStyle ??= new GUIStyle(hudStyle) {
                wordWrap = true,
                alignment = TextAnchor.UpperLeft
            };
            var scale = Mathf.Max(1f, Screen.width / 640f);
            var width = Mathf.Min(220f * scale, Screen.width - 40f);
            var height = gameRuntimeManaHintStyle.CalcHeight(
                new GUIContent(gameRuntimeManaHintText), width - 16f) + 16f;
            var anchor = Screen.width - (640 - gameRuntimeManaHint.AnchorX) * scale;
            var x = Mathf.Clamp(anchor - width, 20f, Screen.width - width - 20f);
            var y = 48f * scale;
            var rect = new Rect(x, y, width, height);
            GUI.Box(rect, GUIContent.none);
            GUI.Label(new Rect(rect.x + 8f, rect.y + 8f, width - 16f, height - 16f),
                gameRuntimeManaHintText, gameRuntimeManaHintStyle);
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
            gameRuntimeManaHintText = null;
            gameRuntimeManaHintStyle = null;
        }

        private sealed class Level2HudPresenter
        {
            private readonly MagicCarpetRuntime owner;

            internal Level2HudPresenter(MagicCarpetRuntime owner)
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
