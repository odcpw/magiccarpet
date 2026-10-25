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
        [Serializable] private sealed class PreviewAudioCommand
        {
            public string operation = "preview_audio";
            public bool sound_enabled, music_enabled, speech_enabled;
            public int sound_volume, music_volume;
        }

        internal struct InputFrame
        {
            internal bool Forward, Backward, Left, Right;
            internal float Roll, Pitch;
            internal bool CastLeft, CastRight, CastSelected, Recenter, FullStop;
        }

        private readonly Func<string, PlayerPreferencesSnapshot> command;
        private PlayerPreferences draft;
        private Vector2 scroll;
        private int captureKey = -1;
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

        internal void Open()
        {
            draft = JsonUtility.FromJson<PlayerPreferences>(JsonUtility.ToJson(Current.preferences));
            captureKey = -1;
            status = null;
        }

        internal void Cancel()
        {
            try { Synchronize(command("{\"operation\":\"cancel\"}")); }
            catch (Exception error) { status = error.Message; }
            // Closing is always possible, including an unavailable transport.
            draft = null;
            captureKey = -1;
        }

        internal bool Apply()
        {
            if (draft == null) return false;
            try
            {
                Synchronize(command(JsonUtility.ToJson(new ApplyCommand { preferences = draft })));
                draft = null;
                captureKey = -1;
                Debug.Log($"MC_PREFERENCES_APPLIED player={Current.preferences.player_name} forward={Current.preferences.keys[0]} sound={Current.preferences.sound_volume} help={Current.preferences.show_help}");
                return true;
            }
            catch (Exception error) { status = error.Message; return false; }
        }

        internal void Center() { roll = 0; pitch = 0; }

        internal void BeginKeyCapture(int slot)
        {
            if (draft == null) throw new InvalidOperationException("preferences are not open");
            if (slot < 0 || slot >= draft.keys.Length) throw new ArgumentOutOfRangeException(nameof(slot));
            captureKey = slot;
        }

        internal bool CaptureKey(Event input)
        {
            if (captureKey < 0 || input.type != EventType.KeyDown) return false;
            if (input.keyCode != KeyCode.Escape) draft.keys[captureKey] = input.keyCode.ToString();
            captureKey = -1;
            input.Use();
            return true;
        }

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
            // J/L and I/K remain keyboard steering alongside source flight keys.
            var turn = (Input.GetKey(KeyCode.L) ? 1 : 0) - (Input.GetKey(KeyCode.J) ? 1 : 0);
            var tilt = (Input.GetKey(KeyCode.K) ? 1 : 0) - (Input.GetKey(KeyCode.I) ? 1 : 0);
            roll += turn * 110f * deltaTime;
            pitch += tilt * 110f * deltaTime;
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

        internal void Draw()
        {
            if (!IsOpen) return;
            // A focused TextField consumes character keys during GUILayout.
            // Capture first so rebinding cannot edit the player's name instead.
            CaptureKey(Event.current);
            var width = Mathf.Min(660, Screen.width - 24);
            var height = Mathf.Min(680, Screen.height - 24);
            var rect = new Rect((Screen.width - width) / 2, (Screen.height - height) / 2, width, height);
            GUI.Box(rect, "Player preferences");
            var content = new Rect(rect.x + 12, rect.y + 28, rect.width - 24, rect.height - 90);
            GUILayout.BeginArea(content);
            scroll = GUILayout.BeginScrollView(scroll);
            GUILayout.Label("Player name (12 characters)");
            draft.player_name = GUILayout.TextField(draft.player_name, 12);
            GUILayout.Label("Input device");
            GUILayout.BeginHorizontal();
            if (GUILayout.Toggle(draft.input_device == "mouse_keyboard", "Mouse + keyboard")) draft.input_device = "mouse_keyboard";
            if (GUILayout.Toggle(draft.input_device == "keyboard", "Keyboard")) draft.input_device = "keyboard";
            var wasEnabled = GUI.enabled;
            GUI.enabled = HasJoystick();
            if (GUILayout.Toggle(draft.input_device == "joystick", "Joystick")) draft.input_device = "joystick";
            GUI.enabled = wasEnabled;
            GUILayout.EndHorizontal();
            if (!HasJoystick()) GUILayout.Label("No joystick detected. Keyboard and mouse remain available.");
            draft.invert_mouse = GUILayout.Toggle(draft.invert_mouse, "Invert vertical steering");
            Slider("Mouse sensitivity (%)", ref draft.mouse_sensitivity, 10, 400);
            Slider("Joystick dead zone (%)", ref draft.joystick_dead_zone, 0, 75);
            draft.flight_assistance = GUILayout.Toggle(draft.flight_assistance, "Flight assistance (idle recenter)");
            // Present only current operational consumers. All ten source
            // assignments remain retained in Rust for original-screen owners.
            var labels = new[] { "Forward", "Backward", "Strafe left", "Strafe right", "Spell menu" };
            for (var i = 0; i < labels.Length; i++)
            {
                GUILayout.BeginHorizontal();
                GUILayout.Label(labels[i]);
                if (GUILayout.Button(captureKey == i ? "Press key (Esc cancels)" : draft.keys[i], GUILayout.Width(220)))
                {
                    BeginKeyCapture(i);
                    GUI.FocusControl(null);
                }
                GUILayout.EndHorizontal();
            }
            var priorAudio = (draft.sound_enabled, draft.music_enabled, draft.speech_enabled, draft.sound_volume, draft.music_volume);
            draft.sound_enabled = GUILayout.Toggle(draft.sound_enabled, "Sound effects");
            Slider("Sound volume", ref draft.sound_volume, 0, 127);
            draft.music_enabled = GUILayout.Toggle(draft.music_enabled, "Music");
            Slider("Music volume", ref draft.music_volume, 0, 127);
            draft.speech_enabled = GUILayout.Toggle(draft.speech_enabled, "Speech");
            if (priorAudio != (draft.sound_enabled, draft.music_enabled, draft.speech_enabled, draft.sound_volume, draft.music_volume))
            {
                try { Synchronize(command(JsonUtility.ToJson(new PreviewAudioCommand {
                    sound_enabled = draft.sound_enabled, music_enabled = draft.music_enabled,
                    speech_enabled = draft.speech_enabled, sound_volume = draft.sound_volume,
                    music_volume = draft.music_volume }))); }
                catch (Exception error) { status = error.Message; }
            }
            draft.show_help = GUILayout.Toggle(draft.show_help, "Control help");
            Slider("Day brightness", ref draft.brightness[0], 0, 4);
            Slider("Night brightness", ref draft.brightness[1], 0, 4);
            Slider("Cave brightness", ref draft.brightness[2], 0, 4);
            draft.fullscreen = GUILayout.Toggle(draft.fullscreen, "Fullscreen");
            GUILayout.Label("Escape/P: pause · F5: save · R: restart · F10: quit\nC/middle mouse: recenter · Backspace: full stop\nJ/L, I/K: steer · Space: cast selected spell");
            if (!string.IsNullOrEmpty(status)) GUILayout.Label(status);
            GUILayout.EndScrollView();
            GUILayout.EndArea();
            var y = rect.yMax - 50;
            if (GUI.Button(new Rect(rect.x + 12, y, (width - 36) / 2, 38), "Apply and save"))
                Apply();
            if (GUI.Button(new Rect(rect.x + width / 2 + 6, y, (width - 36) / 2, 38), "Cancel")) Cancel();
        }

        private static void Slider(string label, ref int value, int min, int max)
        {
            GUILayout.Label(label + ": " + value);
            value = Mathf.RoundToInt(GUILayout.HorizontalSlider(value, min, max));
        }

        internal void DrawEffects(int terrain = 0, bool playing = false)
        {
            if (Current == null) return;
            if (playing && Current.preferences.show_help)
                GUI.Label(new Rect(12, Screen.height - 44, Screen.width - 24, 40),
                    $"{Current.preferences.player_name} · {Current.preferences.keys[0]}/{Current.preferences.keys[1]} fly · {Current.preferences.keys[2]}/{Current.preferences.keys[3]} strafe · Mouse steer · Esc pause · Backspace stop");
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
