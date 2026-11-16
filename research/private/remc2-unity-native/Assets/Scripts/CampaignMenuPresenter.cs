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
        internal enum Action { None, NewGame, Resume, Save, Load, Restart, Continue, Title, Quit, DismissResults, SelectMission, StartMission, ResetCampaign, ToggleIconsAndMap, TogglePanelTransparency }
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
        private readonly Dictionary<string, SourceSecretPortalAnimation> secretPortalAnimations = new(StringComparer.Ordinal);
        private readonly Dictionary<string, SourceSecretPortalAnimation.Presentation> secretPortalFrames = new(StringComparer.Ordinal);
        private string lastMode, lastCampaignPhase, portalTarget, statisticsLevel;
        private int lastAnimationFrame = -1;
        private bool briefingVisible;
        private float briefingElapsed;
        private bool sourceBriefingSuppressed;
        private SourceScrollDialog scrollDialog;
        private SourceScrollDialog pendingSlotDialog;
        private SourceCampaignSaveSlots.Selection saveSlot;
        private string configuredSavePath;
        private string lastLaunchedMission, lastRuntimeId;
        private bool sourceLeftHeld;
        private bool sourcePauseMenuDrawn;
        private bool sourceMapCursorOwned, previousCursorVisible;
        internal bool IsSourceDialogOpen => scrollDialog != null ||
            campaignCache?.PauseMenu?.IsSourceDialogOpen == true ||
            lastMode == GameRuntimeTransport.CampaignMode && campaignCache?.Campaign?.Motion is { } motion &&
            (!motion.Ready || motion.Travelling);
        internal bool SourceBriefingReady => lastMode == GameRuntimeTransport.CampaignMode &&
            lastCampaignPhase == "mission_select" &&
            campaignCache?.Campaign?.Motion is { Ready: true, Travelling: false } && statisticsLevel == null &&
            !sourceBriefingSuppressed;
        internal void ConfigureSavePath(string path) { configuredSavePath = path; }
        internal bool CanQuickSave => scrollDialog == null && pendingSlotDialog == null &&
            preferences?.IsOpen != true && campaignCache?.PauseMenu?.IsSourceDialogOpen != true &&
            !(lastMode == GameRuntimeTransport.CampaignMode && campaignCache?.Campaign?.Motion is { } motion &&
                (motion.Travelling || (!motion.Ready && lastCampaignPhase != "results")));
        internal void CompleteSlotAction(bool success)
        {
            if (pendingSlotDialog == null) return;
            if (!success)
            {
                pendingSlotDialog.ReopenAfterFailure();
                scrollDialog = pendingSlotDialog;
            }
            pendingSlotDialog = null;
        }
        internal SourceCampaignSaveSlots.Selection TakeSaveSlot()
        { var result = saveSlot; saveSlot = null; return result; }

        internal void ReturnToSecretOrigin(
            string originLevelId,
            GameRuntimeTransport.Identity identity)
        {
            if (identity?.mode != GameRuntimeTransport.CampaignMode ||
                identity.campaign_menu?.phase != "mission_select" ||
                identity.campaign_menu.selected_level_id != originLevelId ||
                string.IsNullOrWhiteSpace(originLevelId))
            {
                throw new InvalidOperationException(
                    "secret return map requires Rust's selected MissionSelect origin");
            }

            // Re-enter the retained original map from Rust's returned
            // selection, clearing any pre-secret portal travel or statistics.
            SelectedMission = originLevelId;
            lastLaunchedMission = originLevelId;
            portalTarget = null;
            statisticsLevel = null;
            scrollDialog = null;
            sourceLeftHeld = false;
            portalStates.Clear();
            lastMode = null;
            lastCampaignPhase = null;
        }

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
        internal OwnerCampaignMenuCache LocaleCache => campaignCache ??=
            new OwnerCampaignMenuCache(campaignCachePath ??
                throw new InvalidOperationException("source language selection requires the menu cache"));
        internal Mc2ObjectiveText CreateObjectiveText() => new Mc2ObjectiveText(LocaleCache);
        internal SourceMenuText GameplayText(string level)
        {
            campaignCache ??= new OwnerCampaignMenuCache(campaignCachePath ??
                throw new InvalidOperationException("source gameplay font requires the menu cache"));
            return campaignCache.PauseMenu.GameplayText(campaignCache.ForMission(level).pause_profile);
        }
        internal OwnerCampaignMenuCache.ObjectiveAssets ObjectiveForProfile(string profile)
        {
            campaignCache ??= new OwnerCampaignMenuCache(campaignCachePath ??
                throw new InvalidOperationException("source objective artwork requires the menu cache"));
            return campaignCache.ObjectiveForProfile(profile);
        }
        internal Texture2D GameplayBitmap(string level, int selector) =>
            campaignCache.PauseMenu.GameplayBitmap(campaignCache.ForMission(level).pause_profile, selector);
        internal Color GameplayColor(string level, byte index) =>
            campaignCache.PauseMenu.GameplayColor(campaignCache.ForMission(level).pause_profile, index);
        internal byte GameplayBlackIndex(string level) =>
            campaignCache.PauseMenu.GameplayBlackIndex(campaignCache.ForMission(level).pause_profile);
        internal Color GameplayNoticeColor(string level) =>
            campaignCache.PauseMenu.GameplayNoticeColor(campaignCache.ForMission(level).pause_profile);
        internal Color GameplayPausedColor(string level) =>
            campaignCache.PauseMenu.GameplayPausedColor(campaignCache.ForMission(level).pause_profile);
        internal string GameplayLanguageText(int selector) => SourceText(selector);
        internal OwnerCampaignMenuCache.GameplayHelpBinding GameplayHelpForRow(int row) =>
            LocaleCache.GameplayHelpForRow(row);
        internal int PauseHoverHelpRow => campaignCache?.PauseMenu?.HoverHelpRow ?? 0;
        internal Vector2Int PauseHoverHelpAnchor => campaignCache?.PauseMenu?.HoverHelpAnchor ?? Vector2Int.zero;
        internal void ClearPauseHoverHelp()
        {
            sourcePauseMenuDrawn = false;
            campaignCache?.PauseMenu?.ClearHoverHelp();
        }
        internal void ResetPauseHoverHelp()
        {
            sourcePauseMenuDrawn = false;
            campaignCache?.PauseMenu?.Reset();
        }
        internal void DrawPauseHoverHelp(GameRuntimeTransport.Identity identity,
            Texture scene, ulong generation, ulong revision, Rect view, byte[] palette, bool sceneTopDown)
        {
            if (!sourcePauseMenuDrawn || identity?.mode != GameRuntimeTransport.PausedMode || preferences?.IsOpen == true)
            {
                ClearPauseHoverHelp();
                return;
            }
            var mission = LocaleCache.ForMission(identity.active_runtime_flow.bound_level_id);
            campaignCache.PauseMenu.DrawHoverHelp(campaignCache, mission.pause_profile,
                scene, generation, revision, view, palette,
                identity.player_preferences?.preferences?.show_help == true, sceneTopDown);
        }
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
                Action.ToggleIconsAndMap => mode == GameRuntimeTransport.PausedMode,
                Action.TogglePanelTransparency => mode == GameRuntimeTransport.PausedMode,
                Action.Save => (mode == GameRuntimeTransport.TitleMode || mode == GameRuntimeTransport.PausedMode ||
                    mode == GameRuntimeTransport.CampaignMode) && canSave,
                Action.Load => (mode == GameRuntimeTransport.TitleMode || mode == GameRuntimeTransport.PausedMode || mode == GameRuntimeTransport.CampaignMode) && canLoad,
                Action.Restart => mode == GameRuntimeTransport.PausedMode,
                Action.Title => mode == GameRuntimeTransport.PausedMode || mode == GameRuntimeTransport.CampaignMode,
                Action.DismissResults or Action.SelectMission or Action.StartMission => mode == GameRuntimeTransport.CampaignMode,
                Action.ResetCampaign => mode == GameRuntimeTransport.CampaignMode,
                Action.Continue => mode == GameRuntimeTransport.PausedMode && objectivesSatisfied,
                Action.Quit => mode == GameRuntimeTransport.TitleMode || mode == GameRuntimeTransport.PausedMode,
                _ => false
            };
        }

        internal Action Draw(GameRuntimeTransport.Identity identity, bool canSave, bool canLoad, bool objectivesSatisfied, string status)
        {
            ClearPauseHoverHelp();
            var mode = identity.mode;
            sourceBriefingSuppressed = false;
            OwnSourceMapCursor(mode == GameRuntimeTransport.CampaignMode && preferences?.IsOpen != true);
            if (mode != GameRuntimeTransport.TitleMode && mode != GameRuntimeTransport.PausedMode && mode != GameRuntimeTransport.CampaignMode)
            {
                ResetPauseHoverHelp();
                return Action.None;
            }
            if (campaignCache == null && (campaignCachePath != null || mode == GameRuntimeTransport.CampaignMode))
                campaignCache = new OwnerCampaignMenuCache(campaignCachePath ??
                    throw new InvalidOperationException("campaign map requires its decoded owner cache"));
            if (mode == GameRuntimeTransport.CampaignMode &&
                !string.IsNullOrEmpty(identity.campaign_menu?.briefing_level_id))
                sourceBriefingSuppressed = SuppressSourceBriefing(
                    identity.campaign_menu.briefing_level_id, identity.secret_campaign);
            if (identity.runtime_id != lastRuntimeId)
            {
                lastRuntimeId = identity.runtime_id;
                secretPortalAnimations.Clear();
                secretPortalFrames.Clear();
                lastLaunchedMission = identity.campaign_menu?.completed_level_id;
                lastMode = null;
            }
            if (mode != lastMode || mode == GameRuntimeTransport.CampaignMode && identity.campaign_menu.phase != lastCampaignPhase)
            {
                if (mode == GameRuntimeTransport.CampaignMode)
                {
                    portalStates.Clear();
                    sourceLeftHeld = false;
                    lastLaunchedMission ??= identity.campaign_menu.completed_level_id;
                    campaignCache.Campaign.Enter(identity.campaign_menu, campaignCache.ForMission, lastLaunchedMission);
                }
                campaignCache?.PauseMenu?.Reset();
                statisticsLevel = null;
                scrollDialog = null;
                lastMode = mode;
                lastCampaignPhase = identity.campaign_menu?.phase;
            }
            var repaint = Event.current.type == EventType.Repaint && lastAnimationFrame != Time.frameCount;
            var now = unchecked((uint)(Time.realtimeSinceStartupAsDouble * 100));
            if (preferences?.IsOpen == true)
            {
                if (mode == GameRuntimeTransport.TitleMode && campaignCache?.Startup != null)
                    DrawStartup(now, repaint, canSave, canLoad, status, false);
                preferences.Draw(campaignCache, PlayClick);
                return Action.None;
            }
            if (mode == GameRuntimeTransport.TitleMode && campaignCache?.Startup != null)
            {
                var result = DrawStartup(now, repaint, canSave, canLoad, status);
                if (scrollDialog != null) result = DrawScrollDialog(identity.campaign_menu, repaint, now);
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
                    sourcePauseMenuDrawn = true;
                    return campaignCache.PauseMenu.Draw(campaignCache.ForMission(level).pause_profile,
                        preferences, canSave, canLoad, identity.display_settings.minimap, identity.display_settings.panels_opaque);
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
                    var sourceMap = campaignCache.Campaign;
                    var sourceView = campaignCache.SourceView(identity.campaign_menu);
                    if (Event.current.type == EventType.MouseDown && Event.current.button == 0) sourceLeftHeld = true;
                    if (Event.current.type == EventType.MouseUp && Event.current.button == 0) sourceLeftHeld = false;
                    string arrival = null;
                    if (repaint)
                    {
                        var mouse = (Event.current.mousePosition - sourceView.position) * (640f / sourceView.width);
                        mouse.x = Mathf.Clamp(mouse.x, 0, 638); mouse.y = Mathf.Clamp(mouse.y, 0, 478);
                        arrival = sourceMap.Step(identity.campaign_menu, campaignCache.ForMission, now,
                            unchecked((uint)(Time.realtimeSinceStartupAsDouble * 30)), mouse, sourceLeftHeld,
                            identity.campaign_menu.phase != "results" && statisticsLevel == null, media,
                            campaignCache.FinalMissionBinding);
                    }
                    UpdatePortals(identity.campaign_menu, now, repaint && sourceMap.SourcePassAdvanced);
                    UpdateSecretPortals(identity, now, repaint && sourceMap.SourcePassAdvanced);
                    var completedSecrets = identity.secret_campaign?.completed_levels ?? Array.Empty<string>();
                    sourceMap.DrawMap(identity.campaign_menu, sourceView, campaignCache.ForMission,
                        PortalSelector, level => Array.IndexOf(completedSecrets, level) >= 0,
                        SecretPortalPresentation);
                    if (repaint) lastAnimationFrame = Time.frameCount;
                    if (arrival != null)
                    {
                        DrawMapCommands(identity.campaign_menu, status, false, false, identity.secret_campaign);
                        portalTarget = arrival; SelectedMission = arrival; lastLaunchedMission = arrival;
                        lastMode = null; // The retained UI is re-entered after mission/abandon.
                        return Action.StartMission;
                    }
                    if (scrollDialog != null)
                    {
                        DrawMapCommands(identity.campaign_menu, status, false, false, identity.secret_campaign);
                        return DrawScrollDialog(identity.campaign_menu, repaint, now);
                    }
                    var showingResults = identity.campaign_menu.phase == "results";
                    if (showingResults || statisticsLevel != null)
                    {
                        var menu = identity.campaign_menu;
                        var level = showingResults ? menu.completed_level_id : statisticsLevel;
                        var stats = StatisticsForLevel(menu, identity.secret_campaign, level);
                        if (campaignCache.Campaign == null)
                            throw new InvalidOperationException("campaign results require the original bank6 artwork; refresh the menu cache");
                        var focus = campaignCache.ForMission(showingResults ? menu.completed_level_id :
                            menu.selected_level_id ?? menu.briefing_level_id ?? menu.completed_level_id);
                        campaignCache.Campaign.DrawStatistics(menu, focus, campaignCache.ForMission(level),
                            stats, campaignCache.SourceView(menu), campaignCache.ForMission,
                            PortalSelector, SourceText);
                        DrawMapCommands(menu, status, false, false, identity.secret_campaign);
                        // F5 is the host save edge, not a Results dismissal.
                        // The runtime owns the save request and consumes it once.
                        if (Event.current.type == EventType.KeyDown &&
                            Event.current.keyCode == KeyCode.F5)
                        {
                            Event.current.Use();
                            return Action.None;
                        }
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
                    var controlAction = DrawMapCommands(identity.campaign_menu, status, canSave, true,
                        identity.secret_campaign);
                    if (controlAction != Action.None || scrollDialog != null) return controlAction;
                    if (sourceMap.Motion.Ready && !sourceMap.Motion.Travelling &&
                        identity.campaign_menu.phase != "results" && Event.current.type == EventType.MouseDown)
                    {
                        var target = campaignCache.HitTestPortal(identity.campaign_menu, Event.current.mousePosition);
                        if (target != null && Event.current.button == 0)
                        {
                            SelectedMission = target;
                            sourceMap.BeginTravel(target, identity.campaign_menu, campaignCache.ForMission, media);
                            Event.current.Use();
                            return Action.None;
                        }
                        if (target != null && Event.current.button == 1 &&
                            CanShowStatistics(identity.campaign_menu, identity.secret_campaign, target))
                        {
                            statisticsLevel = target;
                            Event.current.Use();
                        }
                    }
                    return Action.None;
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
            finally
            {
                if (mode == GameRuntimeTransport.CampaignMode)
                {
                    GUI.color = Color.white;
                    GUI.enabled = true;
                    var view = campaignCache.SourceView(identity.campaign_menu);
                    var pointer = (Event.current.mousePosition - view.position) * (640f / view.width);
                    campaignCache.Campaign.DrawOverlay(identity.campaign_menu, view, pointer,
                        preferences?.Current.preferences.show_help != false, now,
                        scrollDialog == null && statisticsLevel == null && identity.campaign_menu.phase != "results" &&
                        campaignCache.Campaign.Motion.Ready && !campaignCache.Campaign.Motion.Travelling,
                        campaignCache.MissionOrder, SourceText);
                }
                GUI.color = oldColor;
                GUI.enabled = oldEnabled;
            }
        }

        private void OwnSourceMapCursor(bool active)
        {
            if (active)
            {
                if (!sourceMapCursorOwned) previousCursorVisible = Cursor.visible;
                sourceMapCursorOwned = true;
                Cursor.visible = false;
            }
            else if (sourceMapCursorOwned)
            {
                Cursor.visible = previousCursorVisible;
                sourceMapCursorOwned = false;
            }
        }

        private Action DrawStartup(uint now, bool advance, bool canSave, bool canLoad, string status,
            bool acceptInput = true)
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
                        0x259730 => Action.Save,
                        0x2590f0 => Action.Load,
                        0x258980 => Action.Quit,
                        _ => Action.None,
                    };
                    var settings = entry.source_action == 0x259e00 || entry.source_action == 0x25a610 ||
                        entry.source_action == 0x25a160 || entry.source_action == 0x2589e0 &&
                        preferences?.Current.installed_locales?.Length > 0;
                    GUI.enabled = action != Action.None && (action != Action.Save || canSave) ||
                        settings && preferences != null;
                    var hit = new Rect(view.x + entry.x * scale, view.y + entry.y * scale,
                        entry.width * scale, entry.height * scale);
                    if (acceptInput && GUI.enabled && hit.Contains(Event.current.mousePosition))
                        DrawBitmap(entry.hover_selector, entry.x, entry.y);
                    if (scrollDialog != null) continue;
                    if (acceptInput && GUI.Button(hit, GUIContent.none, GUIStyle.none))
                    {
                        PlayClick();
                        if (settings)
                        {
                            preferences.Open(entry.source_action switch {
                                0x2589e0 => SourceSettingsDialog.Page.Language,
                                0x259e00 => SourceSettingsDialog.Page.Name,
                                0x25a610 => SourceSettingsDialog.Page.Keys,
                                _ => SourceSettingsDialog.Page.Joystick
                            });
                            return Action.None;
                        }
                        if (action == Action.Load || action == Action.Save)
                        {
                            scrollDialog = new SourceScrollDialog(action == Action.Load ? SourceScrollDialog.Kind.Load :
                                SourceScrollDialog.Kind.Save, true, configuredSavePath);
                            return Action.None;
                        }
                        return action;
                    }
                }
                GUI.enabled = true;
                var help = scrollDialog == null ? campaignCache.StartupHoverHelp((Event.current.mousePosition - view.position) / scale) : null;
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
                if (campaignCache.ForMission(level).is_secret) continue;
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
                else if (menu.phase != "results" && campaignCache.Campaign.Motion.Ready)
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

        private void UpdateSecretPortals(GameRuntimeTransport.Identity identity, uint now, bool advance)
        {
            var progress = identity.secret_campaign;
            var discovered = progress?.discovered_levels ?? Array.Empty<string>();
            var completed = progress?.completed_levels ?? Array.Empty<string>();
            var secretEntries = identity.active_runtime_flow?.secret_entries ??
                Array.Empty<GameRuntimeTransport.RuntimeFlowSecretEntry>();
            secretPortalFrames.Clear();

            foreach (var entry in secretEntries)
            {
                if (entry == null || string.IsNullOrEmpty(entry.secret_level_id) ||
                    string.IsNullOrEmpty(entry.origin_level_id))
                    throw new InvalidOperationException("Rust secret flow contains an incomplete portal binding");

                var mission = campaignCache.ForMission(entry.secret_level_id);
                var origin = campaignCache.ForMission(entry.origin_level_id);
                if (!mission.is_secret || origin.is_secret ||
                    mission.source_ordinary_index != origin.source_ordinary_index)
                    throw new InvalidOperationException("source secret portal is not bound to its actual ordinary parent");

                if (!secretPortalAnimations.TryGetValue(mission.level_id, out var animation))
                {
                    animation = new SourceSecretPortalAnimation();
                    secretPortalAnimations.Add(mission.level_id, animation);
                }
                var presentation = animation.Step(
                    mission.is_secret,
                    Array.IndexOf(discovered, mission.level_id) >= 0,
                    Array.IndexOf(completed, mission.level_id) >= 0,
                    now,
                    advance,
                    identity.campaign_menu.phase == "mission_select" &&
                    campaignCache.Campaign.Motion.Ready && !campaignCache.Campaign.Motion.Travelling);
                if (presentation.PlayOpeningSound)
                {
                    var binding = campaignCache.PortalAnimation ??
                        throw new InvalidOperationException("secret portal animation requires the source portal cache");
                    var sound = binding.sound ??
                        throw new InvalidOperationException("secret portal opening sound is unbound");
                    if (media == null)
                        throw new InvalidOperationException("secret portal animation requires the frontend audio owner");
                    media.PlayMenuSample(sound.id, campaignCache.ReadPortalOpeningSound(), sound.volume,
                        sound.pan, sound.rate, sound.if_not_playing);
                }
                secretPortalFrames[mission.level_id] = presentation;
            }
        }

        private SourceSecretPortalAnimation.Presentation SecretPortalPresentation(string level) =>
            secretPortalFrames.TryGetValue(level, out var presentation)
                ? presentation : SourceSecretPortalAnimation.Presentation.Hidden;

        private int PortalSelector(string level, bool completed)
        {
            if (portalStates.TryGetValue(level, out var state)) return state.DrawSelector;
            return completed ? 37 : 70;
        }

        internal static GameRuntimeTransport.CompletionStatistics StatisticsForLevel(
            GameRuntimeTransport.CampaignMenuSnapshot menu,
            GameRuntimeTransport.SecretCampaignSnapshot secrets, string level)
        {
            var stats = menu.completed_level_id == level ? menu.completion_statistics : null;
            if (stats == null && menu.level_statistics != null)
                stats = Array.Find(menu.level_statistics, row => row.level_id == level)?.statistics;
            if (stats == null && secrets?.level_statistics != null &&
                secrets.completed_levels != null && Array.IndexOf(secrets.completed_levels, level) >= 0)
                stats = Array.Find(secrets.level_statistics, row => row.level_id == level)?.statistics;
            return stats;
        }

        internal static bool CanShowStatistics(GameRuntimeTransport.CampaignMenuSnapshot menu,
            GameRuntimeTransport.SecretCampaignSnapshot secrets, string level) =>
            menu != null && level != null &&
            ((menu.completed_levels != null && Array.IndexOf(menu.completed_levels, level) >= 0) ||
             (secrets?.completed_levels != null && Array.IndexOf(secrets.completed_levels, level) >= 0 &&
              StatisticsForLevel(menu, secrets, level) != null));

        internal string StatisticsText(GameRuntimeTransport.CampaignMenuSnapshot menu, string level,
            GameRuntimeTransport.SecretCampaignSnapshot secrets = null)
        {
            var stats = StatisticsForLevel(menu, secrets, level);
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

        private Action DrawMapCommands(GameRuntimeTransport.CampaignMenuSnapshot menu, string status, bool canSave,
            bool acceptInput, GameRuntimeTransport.SecretCampaignSnapshot secretCampaign)
        {
            var view = campaignCache.SourceView(menu);
            var scale = view.width / 640f;
            var source = campaignCache.Campaign;
            if (source?.Controls == null) throw new InvalidOperationException("campaign controls require the original source artwork cache");
            foreach (var control in source.Controls)
            {
                var hover = source.Bitmap(control.hover_selector);
                var hit = new Rect(view.x + control.x * scale, view.y + control.y * scale,
                    hover.width * scale, hover.height * scale);
                var interactive = acceptInput && !source.Motion.Travelling;
                source.DrawBitmap(interactive && hit.Contains(Event.current.mousePosition) ? control.hover_selector : control.normal_selector,
                    control.x, control.y, view);
                if (!interactive) continue;
                if (Event.current.type != EventType.MouseDown || Event.current.button != 0 ||
                    !view.Contains(Event.current.mousePosition) || !hit.Contains(Event.current.mousePosition)) continue;
                Event.current.Use();
                PlayClick();
                switch (control.source_action)
                {
                    case 0x25f620: return Action.Title;
                    case 0x25f640:
                        scrollDialog = new SourceScrollDialog(SourceScrollDialog.Kind.Reset, false, configuredSavePath);
                        return Action.None;
                    case 0x25f800:
                        if (canSave) scrollDialog = new SourceScrollDialog(SourceScrollDialog.Kind.Save, false, configuredSavePath);
                        return Action.None;
                    case 0x25f820:
                        scrollDialog = new SourceScrollDialog(SourceScrollDialog.Kind.Load, false, configuredSavePath);
                        return Action.None;
                    default: throw new InvalidOperationException("unbound source campaign control action");
                }
            }
            if (acceptInput && briefingVisible && source.Motion.Ready && !source.Motion.Travelling &&
                !string.IsNullOrEmpty(menu.briefing_level_id) &&
                !SuppressSourceBriefing(menu.briefing_level_id, secretCampaign))
            {
                var mission = campaignCache.ForMission(menu.briefing_level_id);
                source.DrawBriefing(menu, campaignCache.MapFocus(menu), campaignCache.BriefingText(mission),
                    view, campaignCache.ForMission, PortalSelector);
            }
            if (!string.IsNullOrEmpty(status)) source.DialogText.DrawText(status, 130, 10, view, source.NearestColor(63, 63, 63));
            return Action.None;
        }

        private bool SuppressSourceBriefing(string level, GameRuntimeTransport.SecretCampaignSnapshot secretCampaign)
        {
            var mission = campaignCache.ForMission(level);
            if (mission.is_secret) return true;

            foreach (var secretLevel in secretCampaign?.discovered_levels ?? Array.Empty<string>())
            {
                var secret = campaignCache.ForMission(secretLevel);
                if (!secret.is_secret)
                    throw new InvalidOperationException("Rust secret frontier is not bound to source secret artwork");
                if (secret.source_ordinary_index == mission.source_ordinary_index)
                    return true;
            }
            return false;
        }

        private Action DrawScrollDialog(GameRuntimeTransport.CampaignMenuSnapshot menu, bool advance, uint now)
        {
            var scale = Mathf.Min(Screen.width / 640f, Screen.height / 480f);
            var view = menu == null ? new Rect((Screen.width - 640 * scale) / 2, (Screen.height - 480 * scale) / 2,
                640 * scale, 480 * scale) : campaignCache.SourceView(menu);
            var result = scrollDialog.Draw(campaignCache, view, advance, now, PlayClick);
            if (result == null) return Action.None;
            var completedDialog = scrollDialog;
            scrollDialog = null;
            if (!result.Confirmed) return Action.None;
            saveSlot = result.Slot;
            if (result.Operation is SourceScrollDialog.Kind.Save or SourceScrollDialog.Kind.Load)
                pendingSlotDialog = completedDialog;
            return result.Operation switch {
                SourceScrollDialog.Kind.Save => Action.Save,
                SourceScrollDialog.Kind.Load => Action.Load,
                SourceScrollDialog.Kind.Reset => Action.ResetCampaign,
                _ => throw new InvalidOperationException("unbound source scroll result")
            };
        }

        private void PlayClick()
        {
            var sound = campaignCache.ClickSound ??
                throw new InvalidOperationException("source menu controls require their original click sample");
            if (media == null) throw new InvalidOperationException("source menu controls require the frontend audio owner");
            media.PlayMenuSample(sound.id, campaignCache.ReadClickSound(), sound.volume, sound.pan, sound.rate, sound.if_not_playing);
        }

        public void Dispose()
        {
            OwnSourceMapCursor(false);
            campaignCache?.Dispose();
            if (Application.isPlaying) UnityEngine.Object.Destroy(background);
            else UnityEngine.Object.DestroyImmediate(background);
        }
    }
}
