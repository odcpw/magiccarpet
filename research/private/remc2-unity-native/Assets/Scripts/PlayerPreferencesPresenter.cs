using System;
using System.Collections.Generic;
using System.IO;
using UnityEngine;

namespace MagicCarpet.Native
{
    [Serializable]
    internal sealed class PlayerPreferences
    {
        public string schema;
        public string player_name;
        public string selected_locale;
        public string[] keys;
        public string input_device;
        public bool invert_mouse;
        public int mouse_sensitivity;
        public int joystick_dead_zone;
        public bool sound_enabled, music_enabled, speech_enabled;
        public int sound_volume, music_volume;
        public bool show_help;
        public bool reflections = true, sky = true, shadows = true;
        public bool dynamic_lighting = true;
        public bool auto_show_objectives_for_foreign_languages;
        public int[] brightness;
        public bool flight_assistance, fullscreen;
    }

    [Serializable]
    internal sealed class InstalledLocaleSummary
    {
        public string id, label;
        public string[] aliases;
        public int non_ascii_record_count;
    }

    [Serializable]
    internal sealed class PlayerLocaleBinding
    {
        public string cache_path, binding_sha256, id, pack_sha256, text_sha256;
        public string[] font_ids;
    }

    [Serializable]
    internal sealed class PlayerPreferencesSnapshot
    {
        public PlayerPreferences preferences;
        public bool installed;
        public string load_status;
        public ulong assistance_sequence;
        public InstalledLocaleSummary[] installed_locales;
        public PlayerLocaleBinding locale_binding;
        public string language_status;
    }

    /// Device and UI adapter only. Rust validates, publishes and retains every
    /// active preference and produces idle assistance in ordinary command order.
    internal sealed class PlayerPreferencesPresenter
    {
        [Serializable] private sealed class InstallCommand
        { public string operation = "install"; public string profile_dir; }
        [Serializable] private sealed class ApplyCommand
        { public string operation = "apply"; public PlayerPreferences preferences; }
        [Serializable] private sealed class ApplyLocaleCommand
        {
            public string operation = "apply_checked";
            public PlayerPreferences preferences;
            public PlayerLocaleBinding expected_locale_binding;
        }
        [Serializable] private sealed class ResetDefaultKeysCommand
        {
            public string operation = "reset_default_keys_checked";
            public PlayerLocaleBinding expected_locale_binding;
        }
        [Serializable] private sealed class InstallLanguagesCommand
        {
            public string operation = "install_languages_checked";
            public string cache_dir;
            public PlayerLocaleBinding expected_locale_binding;
        }
        internal struct InputFrame
        {
            internal bool Forward, Backward, Left, Right;
            internal float Roll, Pitch;
            internal bool CastLeft, CastRight, CastSelected, Recenter, FullStop;
        }

        private readonly Func<string, PlayerPreferencesSnapshot> command;
        private readonly Func<KeyCode, bool> keyHeld;
        private readonly Func<KeyCode, bool> keyPressed;
        private readonly Func<int, bool> mouseHeld;
        private readonly Func<int, bool> mousePressed;
        private readonly Action<OwnerCampaignMenuCache.LocaleTextBinding> localeApplied;
        private OwnerCampaignMenuCache languageCache;
        private readonly HashSet<KeyCode> releaseKeys = new();
        private readonly HashSet<int> releaseMouseButtons = new();
        private PlayerPreferences draft;
        private readonly SourceSettingsDialog sourceSettingsDialog = new();
        private SourceSettingsDialog.Page page = SourceSettingsDialog.Page.Keys;
        private int selectedKeySlot;
        private int captureKey = -1;
        private bool keyCaptureRejected;
        private string status;
        private float roll, pitch;
        private bool fullscreenApplied;
        private bool lastFullscreen;
        internal PlayerPreferencesSnapshot Current { get; private set; }
        internal bool IsOpen => draft != null;
        internal float SteeringRoll => roll;
        internal float SteeringPitch => pitch;

        internal PlayerPreferencesPresenter(Func<string, PlayerPreferencesSnapshot> command, string profileDir,
            PlayerPreferencesSnapshot installed = null, Func<KeyCode, bool> keyHeld = null,
            Func<KeyCode, bool> keyPressed = null, Func<int, bool> mouseHeld = null,
            Func<int, bool> mousePressed = null,
            Action<OwnerCampaignMenuCache.LocaleTextBinding> localeApplied = null)
        {
            this.command = command ?? throw new ArgumentNullException(nameof(command));
            this.keyHeld = keyHeld ?? (key => Input.GetKey(key));
            this.keyPressed = keyPressed ?? (key => Input.GetKeyDown(key));
            this.mouseHeld = mouseHeld ?? (button => Input.GetMouseButton(button));
            this.mousePressed = mousePressed ?? (button => Input.GetMouseButtonDown(button));
            this.localeApplied = localeApplied;
            if (installed != null && !installed.installed)
                throw new InvalidOperationException("preferences hydration requires an installed Rust profile");
            // A restored owner may already have its operational profile. Observe
            // that authority; the native one-time installation guard stays intact.
            Synchronize(installed ?? command(JsonUtility.ToJson(new InstallCommand {
                profile_dir = Path.GetFullPath(profileDir) })));
        }

        internal void Synchronize(PlayerPreferencesSnapshot snapshot)
        {
            if (snapshot?.preferences?.keys?.Length != 10 || snapshot.preferences.brightness?.Length != 3)
                throw new InvalidOperationException("Rust preferences observation is incomplete");
            if (Current != null && Current.assistance_sequence != snapshot.assistance_sequence)
            {
                Center();
                Debug.Log($"MC_FLIGHT_ASSIST recenter sequence={snapshot.assistance_sequence}");
            }
            Current = snapshot;
            if (!fullscreenApplied || lastFullscreen != snapshot.preferences.fullscreen)
            {
                Screen.fullScreen = snapshot.preferences.fullscreen;
                lastFullscreen = snapshot.preferences.fullscreen;
                fullscreenApplied = true;
            }
        }

        private OwnerCampaignMenuCache.LocaleTextBinding PrepareInstalledLocale(OwnerCampaignMenuCache cache)
        {
            if (cache == null) throw new ArgumentNullException(nameof(cache));
            try { return cache.PrepareLocale(cache.LanguageCachePath, Current.preferences.selected_locale); }
            catch (ArgumentOutOfRangeException)
            {
                // Match Rust's read-only recovery of a saved ID that is no
                // longer installed. Apply uses the strict lookup below.
                return cache.PrepareLocale(cache.LanguageCachePath, null);
            }
        }

        private static bool SameLocale(PlayerLocaleBinding left, PlayerLocaleBinding right)
        {
            if (left == null || right == null) return left == right;
            if (left.cache_path != right.cache_path || left.binding_sha256 != right.binding_sha256 ||
                left.id != right.id || left.pack_sha256 != right.pack_sha256 || left.text_sha256 != right.text_sha256 ||
                left.font_ids == null || right.font_ids == null || left.font_ids.Length != right.font_ids.Length)
                return false;
            for (var index = 0; index < left.font_ids.Length; index++)
                if (left.font_ids[index] != right.font_ids[index]) return false;
            return true;
        }

        internal bool RequiresLanguageInstall(OwnerCampaignMenuCache cache) =>
            !SameLocale(Current.locale_binding, PrepareInstalledLocale(cache).Identity);

        internal void BindLanguages(OwnerCampaignMenuCache cache)
        {
            var candidate = PrepareInstalledLocale(cache);
            var accepted = SameLocale(Current.locale_binding, candidate.Identity) ? Current :
                command(JsonUtility.ToJson(new InstallLanguagesCommand {
                    cache_dir = cache.LanguageCachePath, expected_locale_binding = candidate.Identity }));
            PublishLocale(cache, candidate, accepted);
            Synchronize(accepted);
        }

        private void PublishLocale(OwnerCampaignMenuCache cache,
            OwnerCampaignMenuCache.LocaleTextBinding candidate, PlayerPreferencesSnapshot accepted)
        {
            if (!ReferenceEquals(candidate, cache.PrepareLocale(accepted.locale_binding)))
                throw new InvalidOperationException("native and prepared source locale owners differ");
            var changed = languageCache != cache || !SameLocale(Current.locale_binding, accepted.locale_binding) ||
                Current.preferences.selected_locale != accepted.preferences.selected_locale;
            cache.PublishLocale(candidate);
            languageCache = cache;
            localeApplied?.Invoke(candidate);
            if (changed)
            {
                var identity = accepted.locale_binding;
                Debug.Log($"MC_SOURCE_LOCALE selection={accepted.preferences.selected_locale ?? "default"} " +
                    $"id={identity.id} binding={identity.binding_sha256} pack={identity.pack_sha256} " +
                    $"text={identity.text_sha256} fonts={string.Join(",", identity.font_ids)}");
            }
        }

        private PlayerPreferencesSnapshot ApplyPreferences(PlayerPreferences preferences)
        {
            if (languageCache == null)
                return command(JsonUtility.ToJson(new ApplyCommand { preferences = preferences }));
            var candidate = languageCache.PrepareLocale(languageCache.LanguageCachePath, preferences.selected_locale);
            var accepted = command(JsonUtility.ToJson(new ApplyLocaleCommand {
                preferences = preferences, expected_locale_binding = candidate.Identity }));
            PublishLocale(languageCache, candidate, accepted);
            return accepted;
        }

        internal void Open() => Open(SourceSettingsDialog.Page.Keys);

        internal void Open(SourceSettingsDialog.Page requestedPage)
        {
            draft = JsonUtility.FromJson<PlayerPreferences>(JsonUtility.ToJson(Current.preferences));
            // JsonUtility turns a null string into empty; null means the
            // validated source default, whereas an explicit ID must stay exact.
            draft.selected_locale = Current.preferences.selected_locale;
            page = requestedPage;
            sourceSettingsDialog.Open(requestedPage);
            selectedKeySlot = 0;
            captureKey = requestedPage == SourceSettingsDialog.Page.Keys ? 0 : -1;
            keyCaptureRejected = false;
            status = null;
        }

        internal void Cancel()
        {
            try { Synchronize(command("{\"operation\":\"cancel\"}")); }
            catch (Exception error) { status = error.Message; }
            // Closing is always possible, including an unavailable transport.
            ArmInputReleaseBarrier(Current?.preferences, draft);
            draft = null;
            captureKey = -1;
            keyCaptureRejected = false;
            sourceSettingsDialog.Close();
        }

        internal bool Apply()
        {
            if (draft == null || keyCaptureRejected) return false;
            try
            {
                var previous = Current.preferences;
                Synchronize(ApplyPreferences(draft));
                ArmInputReleaseBarrier(previous, Current.preferences);
                draft = null;
                captureKey = -1;
                sourceSettingsDialog.Close();
                Debug.Log($"MC_PREFERENCES_APPLIED player={Current.preferences.player_name} forward={Current.preferences.keys[0]} sound={Current.preferences.sound_volume} help={Current.preferences.show_help}");
                return true;
            }
            catch (Exception error)
            {
                sourceSettingsDialog.RestoreCursor();
                status = error.Message;
                return false;
            }
        }

        internal bool ResetDefaultKeys()
        {
            if (draft == null || page != SourceSettingsDialog.Page.Keys) return false;
            try
            {
                if (languageCache == null || Current.locale_binding == null)
                    throw new InvalidOperationException("source language binding is unavailable");
                var candidate = languageCache.PrepareLocale(Current.locale_binding);
                var accepted = command(JsonUtility.ToJson(new ResetDefaultKeysCommand {
                    expected_locale_binding = candidate.Identity }));
                if (!SameLocale(candidate.Identity, accepted.locale_binding))
                    throw new InvalidOperationException("native and prepared source locale owners differ");
                var previous = Current.preferences;
                Synchronize(accepted);
                ArmInputReleaseBarrier(previous, Current.preferences);
                draft = JsonUtility.FromJson<PlayerPreferences>(JsonUtility.ToJson(Current.preferences));
                draft.selected_locale = Current.preferences.selected_locale;
                selectedKeySlot = 0;
                captureKey = 0;
                keyCaptureRejected = false;
                status = null;
                return true;
            }
            catch (Exception error)
            {
                sourceSettingsDialog.RestoreCursor();
                status = error.Message;
                return false;
            }
        }

        internal bool ApplySource(Action<PlayerPreferences> change)
        {
            if (change == null) throw new ArgumentNullException(nameof(change));
            try
            {
                var next = JsonUtility.FromJson<PlayerPreferences>(JsonUtility.ToJson(Current.preferences));
                next.selected_locale = Current.preferences.selected_locale;
                change(next);
                Synchronize(ApplyPreferences(next));
                return true;
            }
            catch (Exception error)
            {
                Debug.LogWarning("MC_PREFERENCES_REJECTED " + error.Message);
                return false;
            }
        }

        internal void Center() { roll = 0; pitch = 0; }

        internal void BeginKeyCapture(int slot)
        {
            if (draft == null) throw new InvalidOperationException("preferences are not open");
            if (slot < 0 || slot >= draft.keys.Length) throw new ArgumentOutOfRangeException(nameof(slot));
            captureKey = slot;
            keyCaptureRejected = false;
            status = null;
        }

        internal bool CaptureKey(Event input)
        {
            if (captureKey < 0 || input.type != EventType.KeyDown) return false;
            if (input.keyCode == KeyCode.Escape)
            {
                captureKey = -1;
                keyCaptureRejected = false;
                status = null;
                input.Use();
                return true;
            }
            var slot = captureKey;
            if (!IsSupportedPreferenceKey(input.keyCode))
            {
                keyCaptureRejected = true;
                status = "That key is not available in the source settings.";
                input.Use();
                return true;
            }
            var key = input.keyCode.ToString();
            if (slot < 4 && (input.keyCode is KeyCode.C or KeyCode.L or KeyCode.Space))
            {
                keyCaptureRejected = true;
                status = "That key conflicts with a reserved control.";
                input.Use();
                return true;
            }
            var duplicate = false;
            for (var index = 0; index < draft.keys.Length; index++)
                if (index != slot && string.Equals(draft.keys[index], key, StringComparison.OrdinalIgnoreCase) &&
                    (slot != 6 || index < 4))
                { duplicate = true; break; }
            if (duplicate)
            {
                keyCaptureRejected = true;
                status = "That key is already assigned to another control.";
                input.Use();
                return true;
            }
            draft.keys[slot] = key;
            keyCaptureRejected = false;
            selectedKeySlot = Math.Min(slot + 1, draft.keys.Length - 1);
            captureKey = slot + 1 < draft.keys.Length ? slot + 1 : -1;
            status = null;
            input.Use();
            return true;
        }

        private static bool IsSupportedPreferenceKey(KeyCode key) => key is
            KeyCode.A or KeyCode.B or KeyCode.C or KeyCode.D or KeyCode.E or KeyCode.F or
            KeyCode.G or KeyCode.H or KeyCode.I or KeyCode.J or KeyCode.K or KeyCode.L or
            KeyCode.M or KeyCode.N or KeyCode.O or KeyCode.Q or KeyCode.S or KeyCode.T or
            KeyCode.U or KeyCode.V or KeyCode.W or KeyCode.X or KeyCode.Y or KeyCode.Z or
            KeyCode.Minus or KeyCode.Equals or KeyCode.Semicolon or KeyCode.Backslash or
            KeyCode.Comma or KeyCode.Period or KeyCode.Slash or
            KeyCode.UpArrow or KeyCode.DownArrow or KeyCode.LeftArrow or KeyCode.RightArrow or
            KeyCode.Return or KeyCode.Space or KeyCode.LeftControl or KeyCode.RightControl or
            KeyCode.LeftShift or KeyCode.RightShift or KeyCode.LeftAlt or KeyCode.RightAlt;

        private void ArmInputReleaseBarrier(PlayerPreferences previous, PlayerPreferences next)
        {
            void AddKeys(PlayerPreferences preferences)
            {
                if (preferences?.keys == null) return;
                foreach (var name in preferences.keys)
                    if (Enum.TryParse<KeyCode>(name, out var key) && keyHeld(key)) releaseKeys.Add(key);
            }
            AddKeys(previous);
            AddKeys(next);
            foreach (var key in new[] { KeyCode.Space, KeyCode.C, KeyCode.LeftShift, KeyCode.RightShift,
                         KeyCode.LeftAlt, KeyCode.RightAlt, KeyCode.F5, KeyCode.F6,
                         KeyCode.JoystickButton0, KeyCode.JoystickButton1 })
                if (keyHeld(key)) releaseKeys.Add(key);
            for (var button = 0; button < 3; button++)
                if (mouseHeld(button)) releaseMouseButtons.Add(button);
        }

        // Poll even outside flight, so a release in a title or pause menu is
        // observed before a later independent press reaches the consumer.
        internal void AdvanceInputReleaseBarrier()
        {
            releaseKeys.RemoveWhere(key => !keyHeld(key));
            releaseMouseButtons.RemoveWhere(button => !mouseHeld(button));
        }

        internal bool KeyHeld(KeyCode key)
        {
            var held = keyHeld(key);
            if (!held) releaseKeys.Remove(key);
            return held && !releaseKeys.Contains(key);
        }

        internal bool KeyPressed(KeyCode key)
        {
            if (!keyHeld(key)) releaseKeys.Remove(key);
            return !releaseKeys.Contains(key) && keyPressed(key);
        }

        internal bool Held(int slot) => Enum.TryParse<KeyCode>(Current.preferences.keys[slot], out var key) && KeyHeld(key);
        internal bool Pressed(int slot) => Enum.TryParse<KeyCode>(Current.preferences.keys[slot], out var key) && KeyPressed(key);
        internal bool MouseHeld(int button)
        {
            var held = mouseHeld(button);
            if (!held) releaseMouseButtons.Remove(button);
            return held && !releaseMouseButtons.Contains(button);
        }
        private bool MousePressed(int button)
        {
            if (!mouseHeld(button)) releaseMouseButtons.Remove(button);
            return !releaseMouseButtons.Contains(button) && mousePressed(button);
        }
        private static bool HasJoystick()
        {
            foreach (var name in Input.GetJoystickNames()) if (!string.IsNullOrWhiteSpace(name)) return true;
            return false;
        }

        internal InputFrame SampleInput(float deltaTime, bool active)
        {
            if (!active || IsOpen) { Center(); return default; }
            var prefs = Current.preferences;
            var frame = new InputFrame {
                Forward = Held(0), Backward = Held(1), Left = Held(2), Right = Held(3),
                CastLeft = MouseHeld(0), CastRight = MouseHeld(1),
                CastSelected = KeyPressed(KeyCode.Space),
                Recenter = KeyPressed(KeyCode.C) || MousePressed(2),
                FullStop = Input.GetKeyDown(KeyCode.Backspace),
            };
            var joystick = HasJoystick();
            var mouse = prefs.input_device == "mouse_keyboard" || prefs.input_device == "joystick" && !joystick;
            var gain = prefs.mouse_sensitivity / 100f;
            if (mouse)
            {
                roll = Mathf.Clamp(roll + Input.GetAxisRaw("Mouse X") * 4.5f * gain, -128, 127);
                pitch = Mathf.Clamp(pitch + Input.GetAxisRaw("Mouse Y") * (prefs.invert_mouse ? 4.5f : -4.5f) * gain, -128, 127);
            }
            // Source mouse/keyboard input (EventsFunctions mode7) reads the
            // configured flight slots independently of mouse steering. The
            // letter shortcuts are gameplay commands, not another turn axis.
            if (prefs.input_device == "joystick" && joystick)
            {
                // The installed Unity axes use the first joystick's X/Y axes.
                var x = Input.GetAxisRaw("Joystick X");
                var y = Input.GetAxisRaw("Joystick Y");
                var dead = prefs.joystick_dead_zone / 100f;
                roll = Mathf.Abs(x) <= dead ? 0 : x * 127f;
                pitch = Mathf.Abs(y) <= dead ? 0 : y * (prefs.invert_mouse ? -127f : 127f);
                frame.CastLeft |= KeyHeld(KeyCode.JoystickButton0);
                frame.CastRight |= KeyHeld(KeyCode.JoystickButton1);
            }
            if (frame.Recenter || frame.FullStop) Center();
            roll = Mathf.Clamp(roll, -128, 127);
            pitch = Mathf.Clamp(pitch, -128, 127);
            frame.Roll = roll; frame.Pitch = pitch;
            return frame;
        }

        internal void Draw(OwnerCampaignMenuCache cache, Action playClick = null)
        {
            if (!IsOpen) return;
            try
            {
                if (page == SourceSettingsDialog.Page.Keys) CaptureKey(Event.current);
                var result = sourceSettingsDialog.Draw(cache, page, draft, captureKey, selectedKeySlot, status,
                    Current.installed_locales);
                if (result.ClearStatus) status = null;
                else if (result.Status != null) status = result.Status;
                if (result.SelectedKeySlot >= 0) selectedKeySlot = result.SelectedKeySlot;
                if (result.BeginKeyCapture >= 0) BeginKeyCapture(result.BeginKeyCapture);
                if (result.ResetDefaults)
                {
                    playClick?.Invoke(); // TestMouseRegions plays source sample14 before resetting.
                    ResetDefaultKeys();
                }
                else if (result.Confirm) Apply();
                else if (result.Cancel) Cancel();
            }
            catch (Exception error)
            {
                sourceSettingsDialog.RestoreCursor();
                status = error.Message;
            }
        }

        internal bool ToggleFullscreen()
        {
            if (IsOpen || Current?.preferences == null) return false;
            var fullscreen = Current.preferences.fullscreen;
            return ApplySource(preferences => preferences.fullscreen = !fullscreen);
        }

        /// sub_47650: each six-bit VGA component gains 6*setting and
        /// saturates at 63. Owner caches retain the equivalent RGB8 values.
        internal byte[] ApplyBrightnessToPalette(byte[] rgb8, int terrain)
        {
            var setting = Current.preferences.brightness[Mathf.Clamp(terrain, 0, 2)];
            if (setting == 0) return rgb8;
            var adjusted = new byte[rgb8.Length];
            for (var i = 0; i < adjusted.Length; i++)
                adjusted[i] = checked((byte)Math.Min(252, rgb8[i] + 24 * setting));
            return adjusted;
        }
    }
}
