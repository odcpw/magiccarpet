using System;
using UnityEngine;

namespace MagicCarpet.Native
{
    // UI only: every action is dispatched to the existing Rust runtime owner.
    // The original indexed loading artwork is shared; buttons are Unity UI,
    // not a claim that the original menu layout/animation has been ported.
    internal sealed class CampaignMenuPresenter : IDisposable
    {
        internal enum Action { None, NewGame, Resume, Save, Load, Restart, Continue, Title, Quit, DismissResults, SelectMission, StartMission }
        internal string SelectedMission { get; private set; }
        private readonly Texture2D background;
        private readonly string campaignCachePath;
        private OwnerCampaignMenuCache campaignCache;
        private GUIStyle heading;
        private GUIStyle button;
        private GUIStyle message;
        private PlayerPreferencesPresenter preferences;
        private bool briefingVisible;
        private float briefingElapsed;

        internal void BeginBriefing(bool speechEnabled)
        {
            briefingVisible = !speechEnabled;
            briefingElapsed = 0;
        }

        internal bool StepBriefing(float elapsed, bool toggle)
        {
            var replaySpeech = false;
            if (toggle)
            {
                briefingVisible = !briefingVisible;
                briefingElapsed = 0;
                replaySpeech = briefingVisible;
            }
            if (briefingVisible)
            {
                // NewGameDraw uses hundredths / 100 > 15 for this text box.
                briefingElapsed += elapsed;
                if (briefingElapsed >= 16) briefingVisible = false;
            }
            return replaySpeech;
        }

        internal void AttachPreferences(PlayerPreferencesPresenter owner) { preferences = owner; }

        internal CampaignMenuPresenter(OwnerAssetCache cache, string campaignCachePath = null)
        {
            this.campaignCachePath = campaignCachePath;
            var indices = cache.ReadOutput(CampaignTransitionPresenter.IndicesPath);
            var palette = cache.ReadOutput(CampaignTransitionPresenter.PalettePath);
            if (indices.Length != 320 * 200 || palette.Length != 256 * 3)
                throw new InvalidOperationException("Menu background requires the qualified indexed frame");
            var pixels = new Color32[indices.Length];
            for (var y = 0; y < 200; y++)
                for (var x = 0; x < 320; x++)
                {
                    var offset = indices[y * 320 + x] * 3;
                    pixels[(199 - y) * 320 + x] = new Color32(
                        checked((byte)(palette[offset] * 4)),
                        checked((byte)(palette[offset + 1] * 4)),
                        checked((byte)(palette[offset + 2] * 4)), 255);
                }
            background = new Texture2D(320, 200, TextureFormat.RGBA32, false)
            { name = "Original campaign menu backdrop", filterMode = FilterMode.Point };
            background.SetPixels32(pixels);
            background.Apply(false, true);
        }

        internal static bool Allows(string mode, Action action, bool canSave, bool canLoad, bool objectivesSatisfied)
        {
            if (mode == GameRuntimeTransport.LoadingMode) return false;
            return action switch
            {
                Action.NewGame => mode == GameRuntimeTransport.TitleMode,
                Action.Resume => mode == GameRuntimeTransport.PausedMode,
                Action.Save => mode == GameRuntimeTransport.PausedMode && canSave,
                Action.Load => mode == GameRuntimeTransport.TitleMode && canLoad,
                Action.Restart => mode == GameRuntimeTransport.PausedMode,
                Action.Title => mode == GameRuntimeTransport.PausedMode || mode == GameRuntimeTransport.CampaignMode,
                Action.DismissResults or Action.SelectMission or Action.StartMission => mode == GameRuntimeTransport.CampaignMode,
                Action.Continue => mode == GameRuntimeTransport.PausedMode && objectivesSatisfied,
                Action.Quit => mode == GameRuntimeTransport.TitleMode || mode == GameRuntimeTransport.PausedMode,
                _ => false
            };
        }

        internal Action Draw(GameRuntimeTransport.Identity identity, bool canSave, bool canLoad, bool objectivesSatisfied, string status)
        {
            var mode = identity.mode;
            if (mode != GameRuntimeTransport.TitleMode && mode != GameRuntimeTransport.PausedMode && mode != GameRuntimeTransport.CampaignMode)
                return Action.None;
            if (preferences?.IsOpen == true)
            {
                preferences.Draw();
                return Action.None;
            }
            heading ??= new GUIStyle(GUI.skin.label) { alignment = TextAnchor.MiddleCenter, fontSize = 28 };
            button ??= new GUIStyle(GUI.skin.button) { fontSize = 19 };
            message ??= new GUIStyle(GUI.skin.label) { alignment = TextAnchor.MiddleCenter, wordWrap = true };
            var oldColor = GUI.color;
            var oldEnabled = GUI.enabled;
            try
            {
                GUI.color = Color.white;
                if (mode == GameRuntimeTransport.CampaignMode)
                {
                    campaignCache ??= new OwnerCampaignMenuCache(campaignCachePath ?? throw new InvalidOperationException("campaign map requires its decoded owner cache"));
                    campaignCache.DrawMap(identity.campaign_menu);
                }
                else GUI.DrawTexture(new Rect(0, 0, Screen.width, Screen.height), background, ScaleMode.ScaleToFit);
                var width = Mathf.Min(360, Screen.width - 32);
                var x = (Screen.width - width) * .5f;
                var panelHeight = mode == GameRuntimeTransport.PausedMode ? 590 : 480;
                var y = Mathf.Max(16, (Screen.height - panelHeight) * .5f);
                GUI.color = new Color(0, 0, 0, .85f);
                GUI.DrawTexture(new Rect(x - 16, y, width + 32, panelHeight), Texture2D.whiteTexture);
                GUI.color = Color.white;
                GUI.Label(new Rect(x, y + 12, width, 44), mode == GameRuntimeTransport.TitleMode ? "MAGIC CARPET II" : mode == GameRuntimeTransport.CampaignMode ? "CAMPAIGN" : "PAUSED", heading);
                y += 66;
                if (mode == GameRuntimeTransport.CampaignMode)
                {
                    var menu = identity.campaign_menu;
                    if (menu.phase == "results")
                    {
                        GUI.Label(new Rect(x, y, width, 70), "Mission completed\n" + campaignCache.ForMission(menu.completed_level_id).name, message);
                        y += 80;
                        if (GUI.Button(new Rect(x, y, width, 42), "Return to campaign map", button)) return Action.DismissResults;
                        y += 50;
                    }
                    else
                    {
                        foreach (var level in menu.available_levels)
                        {
                            var label = (level == menu.selected_level_id ? "> " : "") + campaignCache.ForMission(level).name;
                            if (GUI.Button(new Rect(x, y, width, 42), label, button))
                            { SelectedMission = level; return Action.SelectMission; }
                            y += 50;
                        }
                        GUI.enabled = !string.IsNullOrEmpty(menu.selected_level_id);
                        if (GUI.Button(new Rect(x, y, width, 42), "Enter selected mission", button)) return Action.StartMission;
                        GUI.enabled = true;
                        y += 50;
                        if (briefingVisible && !string.IsNullOrEmpty(menu.briefing_level_id))
                        {
                            GUI.Label(new Rect(x, y, width, 110), campaignCache.ForMission(menu.briefing_level_id).briefing, message);
                            y += 120;
                        }
                    }
                    if (GUI.Button(new Rect(x, y, width, 42), "Main menu", button)) return Action.Title;
                    if (preferences != null && GUI.Button(new Rect(x, y + 50, width, 42), "Player preferences", button)) preferences.Open();
                    return Action.None;
                }
                var choices = mode == GameRuntimeTransport.TitleMode
                    ? new[] { (Action.NewGame, "New game"), (Action.Load, "Load saved game"), (Action.Quit, "Quit") }
                    : new[] { (Action.Resume, "Resume"), (Action.Save, "Save game"), (Action.Continue, "Return to campaign"), (Action.Restart, "Restart level"), (Action.Title, "Main menu"), (Action.Quit, "Quit") };
                foreach (var (action, label) in choices)
                {
                    GUI.enabled = Allows(mode, action, canSave, canLoad, objectivesSatisfied);
                    if (GUI.Button(new Rect(x, y, width, 42), label, button)) return action;
                    y += 50;
                }
                GUI.enabled = true;
                if (preferences != null)
                {
                    if (GUI.Button(new Rect(x, y, width, 42), "Player preferences", button)) preferences.Open();
                    y += 50;
                }
                var help = mode == GameRuntimeTransport.PausedMode
                    ? $"{preferences?.Current.preferences.keys[0] ?? "W"}/{preferences?.Current.preferences.keys[1] ?? "S"} fly · Mouse steer\n" +
                      "Number keys select spells · Space cast\n" +
                      "Backspace stop · F5 save · R restart"
                    : "";
                if (status != null || preferences?.Current.preferences.show_help != false)
                    GUI.Label(new Rect(x, y + 8, width, 80), status ?? help, message);
                return Action.None;
            }
            finally { GUI.color = oldColor; GUI.enabled = oldEnabled; }
        }

        public void Dispose()
        {
            campaignCache?.Dispose();
            if (Application.isPlaying) UnityEngine.Object.Destroy(background);
            else UnityEngine.Object.DestroyImmediate(background);
        }
    }
}
