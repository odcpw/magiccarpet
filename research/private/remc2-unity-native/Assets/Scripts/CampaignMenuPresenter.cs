using System;
using System.Collections.Generic;
using System.Text;
using UnityEngine;

namespace MagicCarpet.Native
{
    // UI only: every action is dispatched to the existing Rust runtime owner.
    // Source artwork, animation clocks and hit regions are presentation only;
    // every campaign choice still passes through the existing Rust command.
    internal sealed class CampaignMenuPresenter : IDisposable
    {
        internal enum Action { None, NewGame, Resume, Save, Load, Restart, Continue, Title, Quit, DismissResults, SelectMission, StartMission }
        internal string SelectedMission { get; private set; }
        private readonly Texture2D background;
        private readonly OwnerAssetCache ownerCache;
        private byte[] fallbackLanguage;
        private readonly string campaignCachePath;
        private OwnerCampaignMenuCache campaignCache;
        private GUIStyle heading;
        private GUIStyle button;
        private GUIStyle message;
        private PlayerPreferencesPresenter preferences;
        private FrontendMediaPresenter media;
        private MainAnimation[] mainAnimations;
        private readonly Dictionary<string, PortalState> portalStates = new(StringComparer.Ordinal);
        private string lastMode, lastCampaignPhase, portalTarget, statisticsLevel;
        private int lastAnimationFrame = -1;
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
        internal void AttachMedia(FrontendMediaPresenter owner) { media = owner; }
        internal bool DismissPauseSubmenu() => campaignCache?.PauseMenu?.DismissSubmenu() == true;

        internal SourcePauseMenu.ConfirmationResult DrawAbandonConfirmation(
            GameRuntimeTransport.Identity identity, bool mapOpen, bool alive, int sourceTurn,
            bool acceptInput = true)
        {
            campaignCache ??= new OwnerCampaignMenuCache(campaignCachePath ??
                throw new InvalidOperationException("abandon confirmation requires its source menu cache"));
            var level = identity.active_runtime_flow?.bound_level_id;
            if (string.IsNullOrEmpty(level) || campaignCache.PauseMenu == null)
                throw new InvalidOperationException("abandon confirmation requires its bound mission art");
            return campaignCache.PauseMenu.DrawAbandonConfirmation(
                campaignCache.ForMission(level).pause_profile, mapOpen, alive, (sourceTurn / 4 & 1) != 0,
                acceptInput);
        }
        internal string TakePortalMission()
        {
            var target = portalTarget;
            portalTarget = null;
            return target;
        }

        private sealed class MainAnimation
        {
            internal int X, Y, Selector, First, Last, Step;
            internal uint LastTime;
        }

        internal sealed class PortalState
        {
            internal int Selector;
            internal int DrawSelector;
            internal uint LastTime;
            internal byte Phase;

            // DrawFrameAnim advances before drawing, including end+1 until
            // the following gated pass. Long frames advance once, no catchup.
            internal bool Advance(uint now, int first, int last, int step)
            {
                DrawSelector = Selector;
                if (unchecked(now - LastTime) < step) return false;
                var wrapped = Selector > last;
                Selector = wrapped ? first : Selector + 1;
                DrawSelector = Selector;
                LastTime = now;
                return wrapped;
            }
        }

        internal CampaignMenuPresenter(OwnerAssetCache cache, string campaignCachePath = null)
        {
            ownerCache = cache;
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
                Action.Save => (mode == GameRuntimeTransport.PausedMode || mode == GameRuntimeTransport.CampaignMode) && canSave,
                Action.Load => (mode == GameRuntimeTransport.TitleMode || mode == GameRuntimeTransport.PausedMode) && canLoad,
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
            if (campaignCache == null && (campaignCachePath != null || mode == GameRuntimeTransport.CampaignMode))
                campaignCache = new OwnerCampaignMenuCache(campaignCachePath ??
                    throw new InvalidOperationException("campaign map requires its decoded owner cache"));
            if (mode != lastMode || mode == GameRuntimeTransport.CampaignMode && identity.campaign_menu.phase != lastCampaignPhase)
            {
                if (mode == GameRuntimeTransport.CampaignMode) portalStates.Clear();
                campaignCache?.PauseMenu?.Reset();
                statisticsLevel = null;
                lastMode = mode;
                lastCampaignPhase = identity.campaign_menu?.phase;
            }
            var repaint = Event.current.type == EventType.Repaint && lastAnimationFrame != Time.frameCount;
            var now = unchecked((uint)(Time.realtimeSinceStartupAsDouble * 100));
            if (preferences?.IsOpen == true)
            {
                preferences.Draw();
                return Action.None;
            }
            if (mode == GameRuntimeTransport.TitleMode && campaignCache?.Startup != null)
            {
                var result = DrawStartup(now, repaint, canLoad, status);
                if (repaint) lastAnimationFrame = Time.frameCount;
                return result;
            }
            if (mode == GameRuntimeTransport.PausedMode)
            {
                if (campaignCache?.PauseMenu == null)
                    throw new InvalidOperationException("pause menu requires its decoded source sprite cache");
                var level = identity.active_runtime_flow?.bound_level_id;
                if (string.IsNullOrEmpty(level)) throw new InvalidOperationException("paused runtime has no bound mission");
                var previousColor = GUI.color;
                var previousEnabled = GUI.enabled;
                try
                {
                    GUI.color = Color.white;
                    GUI.enabled = true;
                    return campaignCache.PauseMenu.Draw(campaignCache.ForMission(level).pause_profile,
                        preferences, canSave, canLoad);
                }
                finally { GUI.color = previousColor; GUI.enabled = previousEnabled; }
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
                    UpdatePortals(identity.campaign_menu, now, repaint);
                    campaignCache.DrawMap(identity.campaign_menu, PortalSelector);
                    if (repaint) lastAnimationFrame = Time.frameCount;
                    var showingResults = identity.campaign_menu.phase == "results";
                    if (showingResults || statisticsLevel != null)
                    {
                        var menu = identity.campaign_menu;
                        var level = showingResults ? menu.completed_level_id : statisticsLevel;
                        var stats = menu.completed_level_id == level ? menu.completion_statistics : null;
                        if (stats == null && menu.level_statistics != null)
                            stats = Array.Find(menu.level_statistics, row => row.level_id == level)?.statistics;
                        if (campaignCache.Campaign == null)
                            throw new InvalidOperationException("campaign results require the original bank6 artwork; refresh the menu cache");
                        var focus = campaignCache.ForMission(showingResults ? menu.completed_level_id :
                            menu.selected_level_id ?? menu.briefing_level_id ?? menu.completed_level_id);
                        campaignCache.Campaign.DrawStatistics(menu, focus, campaignCache.ForMission(level),
                            stats, campaignCache.SourceView(menu), campaignCache.ForMission,
                            PortalSelector, SourceText);
                        // NewGameDraw states4/5 consume any key or mouse
                        // button, with the map itself retained behind the sheet.
                        if (Event.current.type == EventType.KeyDown || Event.current.type == EventType.MouseDown)
                        {
                            statisticsLevel = null;
                            Event.current.Use();
                            if (showingResults) return Action.DismissResults;
                        }
                        return Action.None;
                    }
                    if (identity.campaign_menu.phase != "results" && Event.current.type == EventType.MouseDown)
                    {
                        var target = campaignCache.HitTestPortal(identity.campaign_menu, Event.current.mousePosition);
                        if (target != null && Event.current.button == 0)
                        {
                            portalTarget = target;
                            SelectedMission = target;
                            Event.current.Use();
                            return Action.StartMission;
                        }
                        if (target != null && Event.current.button == 1 &&
                            Array.IndexOf(identity.campaign_menu.completed_levels, target) >= 0)
                        {
                            statisticsLevel = target;
                            Event.current.Use();
                        }
                    }
                    return DrawMapCommands(identity.campaign_menu, status, canSave);
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

        private Action DrawStartup(uint now, bool advance, bool canLoad, string status)
        {
            var startup = campaignCache.Startup;
            if (mainAnimations == null)
            {
                mainAnimations = new MainAnimation[startup.animations.Length];
                for (var i = 0; i < mainAnimations.Length; i++)
                {
                    var row = startup.animations[i];
                    mainAnimations[i] = new MainAnimation { X = row.x, Y = row.y, Selector = row.initial_selector,
                        First = row.first_selector, Last = row.last_selector, Step = row.step_ticks };
                }
            }
            var scale = Mathf.Min(Screen.width / 640f, Screen.height / 480f);
            var view = new Rect((Screen.width - 640 * scale) / 2, (Screen.height - 480 * scale) / 2,
                640 * scale, 480 * scale);
            var oldColor = GUI.color;
            var oldEnabled = GUI.enabled;
            try
            {
                GUI.color = Color.white;
                GUI.DrawTexture(new Rect(0, 0, Screen.width, Screen.height), Texture2D.blackTexture);
                GUI.DrawTexture(view, campaignCache.StartupBackground);
                void DrawBitmap(int selector, int x, int y)
                {
                    var texture = campaignCache.StartupBitmap(selector);
                    GUI.DrawTexture(new Rect(view.x + x * scale, view.y + y * scale,
                        texture.width * scale, texture.height * scale), texture);
                }
                DrawBitmap(startup.logo.selector, startup.logo.x, startup.logo.y);
                foreach (var animation in mainAnimations)
                {
                    DrawBitmap(animation.Selector, animation.X, animation.Y);
                    if (advance && unchecked(now - animation.LastTime) >= animation.Step)
                    {
                        animation.Selector = animation.Selector >= animation.Last ? animation.First : animation.Selector + 1;
                        animation.LastTime = now;
                    }
                }
                foreach (var entry in startup.buttons)
                {
                    var action = entry.source_action switch {
                        0x258350 => Action.NewGame,
                        0x2590f0 => Action.Load,
                        0x258980 => Action.Quit,
                        _ => Action.None,
                    };
                    var settings = entry.source_action == 0x259e00 || entry.source_action == 0x25a610;
                    GUI.enabled = action != Action.None || settings && preferences != null;
                    if (action == Action.Load) GUI.enabled = canLoad;
                    var hit = new Rect(view.x + entry.x * scale, view.y + entry.y * scale,
                        entry.width * scale, entry.height * scale);
                    if (GUI.enabled && hit.Contains(Event.current.mousePosition))
                        DrawBitmap(entry.hover_selector, entry.x, entry.y);
                    if (GUI.Button(hit, GUIContent.none, GUIStyle.none))
                    {
                        if (settings) { preferences.Open(); return Action.None; }
                        return action;
                    }
                }
                GUI.enabled = true;
                var help = campaignCache.StartupHoverHelp((Event.current.mousePosition - view.position) / scale);
                if (help != null && preferences?.Current.preferences.show_help != false)
                    campaignCache.StartupText.DrawHelp(help, campaignCache.LanguageText(help.text_selector), view);
                if (status != null) campaignCache.StartupText.DrawText(status, 10, 440, view);
                return Action.None;
            }
            finally { GUI.color = oldColor; GUI.enabled = oldEnabled; }
        }

        private void UpdatePortals(GameRuntimeTransport.CampaignMenuSnapshot menu, uint now, bool advance)
        {
            var animation = campaignCache.PortalAnimation;
            if (animation == null) return;
            foreach (var level in menu.available_levels)
            {
                var completed = Array.IndexOf(menu.completed_levels, level) >= 0;
                if (!portalStates.TryGetValue(level, out var state))
                {
                    var initial = completed ? animation.completed_first : animation.opening_first;
                    state = new PortalState { Selector = initial, DrawSelector = initial };
                    portalStates.Add(level, state);
                }
                if (!advance) continue;
                if (completed)
                    state.Advance(now, animation.completed_first, animation.completed_last, animation.step_ticks);
                else if (menu.phase != "results")
                {
                    if (state.Phase == 0)
                    {
                        var sound = animation.sound;
                        if (media == null) throw new InvalidOperationException("portal animation requires the retained frontend audio owner");
                        media.PlayMenuSample(sound.id, campaignCache.ReadPortalOpeningSound(), sound.volume,
                            sound.pan, sound.rate, sound.if_not_playing);
                        // The byte19==0 source branch plays once and draws no
                        // portal until the next ordinary presentation pass.
                        state.DrawSelector = -1;
                        state.Phase = 1;
                    }
                    else if (state.Phase == 1)
                    {
                        if (state.Advance(now, animation.opening_first, animation.opening_last, animation.step_ticks))
                        {
                            state.Selector = animation.loop_first;
                            state.Phase = 2;
                        }
                    }
                    else state.Advance(now, animation.loop_first, animation.loop_last, animation.step_ticks);
                }
            }
        }

        private int PortalSelector(string level, bool completed)
        {
            if (portalStates.TryGetValue(level, out var state)) return state.DrawSelector;
            return completed ? 37 : 70;
        }

        internal string StatisticsText(GameRuntimeTransport.CampaignMenuSnapshot menu, string level)
        {
            var stats = menu.completed_level_id == level ? menu.completion_statistics : null;
            if (stats == null && menu.level_statistics != null)
                stats = Array.Find(menu.level_statistics, row => row.level_id == level)?.statistics;
            if (stats == null) return "Statistics are not available for this mission.";
            var seconds = stats.time_taken_seconds;
            return SourceText(386) + ": " + stats.spells_found_percent + "%\n" +
                SourceText(385) + ": " + stats.hit_accuracy_percent + "%\n" +
                SourceText(384) + ": " + stats.creatures_killed_percent + "%\n" +
                SourceText(377) + ": " + stats.mana_collected_percent + "%\n" +
                SourceText(394) + $": {seconds / 3600:D2}:{seconds % 3600 / 60:D2}:{seconds % 60:D2}";
        }

        private string SourceText(int selector)
        {
            if (campaignCache == null && campaignCachePath != null)
                campaignCache = new OwnerCampaignMenuCache(campaignCachePath);
            if (campaignCache?.HasLanguage == true) return campaignCache.LanguageText(selector);
            // Legacy v1 menus use the existing hash-qualified default records,
            // while v2 consumers use their retained language binding above.
            fallbackLanguage ??= ownerCache.ReadOutput("font/language-l1.records-nul.bin");
            var cursor = 0;
            for (var index = 0; index <= selector; index++)
            {
                var end = Array.IndexOf(fallbackLanguage, (byte)0, cursor);
                if (end < 0) throw new InvalidOperationException("source statistics label is missing");
                if (index == selector) return Encoding.GetEncoding(28591).GetString(fallbackLanguage, cursor, end - cursor);
                cursor = end + 1;
            }
            throw new ArgumentOutOfRangeException(nameof(selector));
        }

        private Action DrawMapCommands(GameRuntimeTransport.CampaignMenuSnapshot menu, string status, bool canSave)
        {
            var view = campaignCache.SourceView(menu);
            var scale = view.width / 640f;
            Rect At(float x, float y, float width, float height) =>
                new(view.x + x * scale, view.y + y * scale, width * scale, height * scale);
            if (GUI.Button(At(8, 438, 100, 34), "Main menu", button)) return Action.Title;
            if (preferences != null && GUI.Button(At(118, 438, 150, 34), "Preferences", button)) preferences.Open();
            GUI.enabled = canSave;
            if (GUI.Button(At(278, 438, 100, 34), "Save game", button)) return Action.Save;
            GUI.enabled = true;
            if (!string.IsNullOrEmpty(menu.selected_level_id) &&
                GUI.Button(At(388, 438, 240, 34), "Enter " + campaignCache.ForMission(menu.selected_level_id).name, button))
                return Action.StartMission;
            if (briefingVisible && !string.IsNullOrEmpty(menu.briefing_level_id))
            {
                var mission = campaignCache.ForMission(menu.briefing_level_id);
                GUI.Label(At(130, mission.viewport_y < 478 ? 280 : 60, 380, 110), mission.briefing, message);
            }
            if (!string.IsNullOrEmpty(status)) GUI.Label(At(130, 10, 380, 45), status, message);
            return Action.None;
        }

        public void Dispose()
        {
            campaignCache?.Dispose();
            if (Application.isPlaying) UnityEngine.Object.Destroy(background);
            else UnityEngine.Object.DestroyImmediate(background);
        }
    }
}
