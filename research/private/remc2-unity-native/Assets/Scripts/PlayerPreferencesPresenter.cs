using System;
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
        public bool auto_show_objectives_for_foreign_languages;
        public int[] brightness;
        public bool flight_assistance, fullscreen;
    }

    [Serializable]
    internal sealed class PlayerPreferencesSnapshot
    {
        public PlayerPreferences preferences;
        public bool installed;
        public string load_status;
        public ulong assistance_sequence;
    }

    /// Device and UI adapter only. Rust validates, publishes and retains every
    /// active preference and produces idle assistance in ordinary command order.
    internal sealed class PlayerPreferencesPresenter
    {
        [Serializable] private sealed class InstallCommand
        { public string operation = "install"; public string profile_dir; }
        [Serializable] private sealed class ApplyCommand
        { public string operation = "apply"; public PlayerPreferences preferences; }
        internal struct InputFrame
        {
            internal bool Forward, Backward, Left, Right;
            internal float Roll, Pitch;
            internal bool CastLeft, CastRight, CastSelected, Recenter, FullStop;
        }

        private readonly Func<string, PlayerPreferencesSnapshot> command;
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
            PlayerPreferencesSnapshot installed = null)
        {
            this.command = command ?? throw new ArgumentNullException(nameof(command));
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

        internal void Open() => Open(SourceSettingsDialog.Page.Keys);

        internal void Open(SourceSettingsDialog.Page requestedPage)
        {
            draft = JsonUtility.FromJson<PlayerPreferences>(JsonUtility.ToJson(Current.preferences));
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
                Synchronize(command(JsonUtility.ToJson(new ApplyCommand { preferences = draft })));
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

        internal bool ApplySource(Action<PlayerPreferences> change)
        {
            if (change == null) throw new ArgumentNullException(nameof(change));
            try
            {
                var next = JsonUtility.FromJson<PlayerPreferences>(JsonUtility.ToJson(Current.preferences));
                change(next);
                Synchronize(command(JsonUtility.ToJson(new ApplyCommand { preferences = next })));
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
            KeyCode.UpArrow or KeyCode.DownArrow or KeyCode.LeftArrow or KeyCode.RightArrow or
            KeyCode.Return or KeyCode.Space or KeyCode.LeftControl or KeyCode.RightControl or
            KeyCode.LeftShift or KeyCode.RightShift or KeyCode.LeftAlt or KeyCode.RightAlt;

        internal bool Held(int slot) => Enum.TryParse<KeyCode>(Current.preferences.keys[slot], out var key) && Input.GetKey(key);
        internal bool Pressed(int slot) => Enum.TryParse<KeyCode>(Current.preferences.keys[slot], out var key) && Input.GetKeyDown(key);
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
                CastLeft = Input.GetMouseButton(0), CastRight = Input.GetMouseButton(1),
                CastSelected = Input.GetKeyDown(KeyCode.Space),
                Recenter = Input.GetKeyDown(KeyCode.C) || Input.GetMouseButtonDown(2),
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
                frame.CastLeft |= Input.GetKey(KeyCode.JoystickButton0);
                frame.CastRight |= Input.GetKey(KeyCode.JoystickButton1);
            }
            if (frame.Recenter || frame.FullStop) Center();
            roll = Mathf.Clamp(roll, -128, 127);
            pitch = Mathf.Clamp(pitch, -128, 127);
            frame.Roll = roll; frame.Pitch = pitch;
            return frame;
        }

        internal void Draw(OwnerCampaignMenuCache cache)
        {
            if (!IsOpen) return;
            try
            {
                if (page == SourceSettingsDialog.Page.Keys) CaptureKey(Event.current);
                var result = sourceSettingsDialog.Draw(cache, page, draft, captureKey, selectedKeySlot, status);
                if (result.ClearStatus) status = null;
                else if (result.Status != null) status = result.Status;
                if (result.SelectedKeySlot >= 0) selectedKeySlot = result.SelectedKeySlot;
                if (result.BeginKeyCapture >= 0) BeginKeyCapture(result.BeginKeyCapture);
                if (result.Confirm) Apply();
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

        internal void DrawEffects(int terrain = 0, bool playing = false)
        {
            if (Current == null) return;
            if (playing && Current.preferences.show_help)
                GUI.Label(new Rect(12, Screen.height - 44, Screen.width - 24, 40),
                    $"{Current.preferences.player_name} · {Current.preferences.keys[0]}/{Current.preferences.keys[1]} fly · {Current.preferences.keys[2]}/{Current.preferences.keys[3]} strafe · Mouse steer · P pause · Esc abandon · Backspace stop");
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
